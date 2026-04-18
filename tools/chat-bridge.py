#!/usr/bin/env python3
"""
GOS Chat Bridge — tools/chat-bridge.py
=======================================
Connects to the GOS kernel's COM2 serial port (exposed by QEMU as a TCP
server on 127.0.0.1:14444) and forwards chat messages to a configured AI API.

Usage
-----
  python tools/chat-bridge.py [--api openai|anthropic|ollama] [--model MODEL]
                               [--key API_KEY] [--host HOST] [--port PORT]
                               [--serial-port TCP_PORT]

Defaults
--------
  --api         ollama          (local Ollama running on the host)
  --model       qwen2.5:7b     (fast, capable open-weight model via Ollama)
  --host        127.0.0.1      (API host; 127.0.0.1 works for Ollama on host)
  --port        11434           (Ollama API port)
  --serial-port 14444           (QEMU COM2 TCP port)

Supported back-ends
-------------------
  ollama      http://<host>:<port>/api/chat  (Ollama local inference)
  openai      https://api.openai.com/v1/chat/completions
  anthropic   https://api.anthropic.com/v1/messages

GOS Bridge Protocol (COM2 / TCP 14444)
--------------------------------------
  Kernel → Bridge   GHELO:<id>\\n        handshake request
  Bridge → Kernel   GOKAY:<ver>\\n       handshake acknowledgement
  Kernel → Bridge   GCHAT:<message>\\n   user message
  Bridge → Kernel   GRESP:<text>\\n      one paragraph of AI response
  Bridge → Kernel   GTOOL:<t>:<a>\\n     optional tool invocation request
  Kernel → Bridge   GRSLT:<result>\\n    tool execution result
  Bridge → Kernel   GDONE:\\n            end of AI turn

Supported tools (returned as GTOOL frames)
-------------------------------------------
  ping:<ip>       ask kernel to ping an IP via e1000 NIC
  net:status      ask kernel to print NIC status
  clear           ask kernel to clear the VGA screen

System prompt
-------------
The system prompt instructs the model to:
  - Respond concisely (≤ 3 short paragraphs per turn).
  - Use TOOL: <tool>(<arg>) syntax when it wants to invoke a kernel action.
  - NEVER use markdown headers or code fences in its output.
"""

import argparse
import json
import socket
import sys
import textwrap
import urllib.error
import urllib.request
from typing import Iterator

# ── ANSI colours for host-side logging ────────────────────────────────────────
RESET  = "\033[0m"
CYAN   = "\033[36m"
YELLOW = "\033[33m"
GREEN  = "\033[32m"
RED    = "\033[31m"
GREY   = "\033[90m"


def log(colour: str, tag: str, msg: str) -> None:
    print(f"{colour}[{tag}]{RESET} {msg}", flush=True)


# ── System prompt ──────────────────────────────────────────────────────────────
SYSTEM_PROMPT = textwrap.dedent("""\
    You are an AI assistant embedded in the GOS bare-metal kernel running on
    x86-64 hardware (QEMU).  You have access to a small set of kernel tools:

      TOOL: ping(<ip>)       — send an ICMP echo to <ip> via the e1000 NIC.
      TOOL: net(status)      — print the NIC driver status.
      TOOL: clear()          — clear the VGA screen.

    Rules:
    1. Keep replies concise: at most 3 short paragraphs.
    2. Do NOT use markdown headers, bullet lists, or code fences.
       Plain prose only.
    3. If you want to invoke a tool, include exactly ONE line of the form:
         TOOL: <name>(<arg>)
       at the END of your reply, after the prose. Do not explain the tool call.
    4. Numbers, IPs, and technical terms are fine; emoji are not.
""")

# ── Tool-frame helpers ─────────────────────────────────────────────────────────

def parse_tool_line(line: str):
    """
    Parse a 'TOOL: name(arg)' line from the model output.
    Returns (tool_name, arg) or None.
    """
    line = line.strip()
    if not line.upper().startswith("TOOL:"):
        return None
    body = line[5:].strip()
    if "(" in body and body.endswith(")"):
        name, rest = body.split("(", 1)
        arg = rest[:-1].strip()
        return name.strip().lower(), arg
    return None


def tool_to_frame(name: str, arg: str) -> str:
    """Convert a parsed tool to the GTOOL:<t>:<a> wire frame."""
    if name == "ping":
        return f"GTOOL:ping:{arg}"
    elif name == "net":
        return f"GTOOL:net:{arg if arg else 'status'}"
    elif name == "clear":
        return "GTOOL:clear"
    else:
        return f"GTOOL:{name}:{arg}"


# ── AI back-end ────────────────────────────────────────────────────────────────

class AIBackend:
    def __init__(self, api: str, model: str, key: str, host: str, port: int):
        self.api   = api
        self.model = model
        self.key   = key
        self.host  = host
        self.port  = port
        self.history = []   # list of {"role": ..., "content": ...}

    def _ollama_chat(self, user_msg: str) -> str:
        self.history.append({"role": "user", "content": user_msg})
        payload = json.dumps({
            "model": self.model,
            "messages": [{"role": "system", "content": SYSTEM_PROMPT}] + self.history,
            "stream": False,
        }).encode()
        url = f"http://{self.host}:{self.port}/api/chat"
        req = urllib.request.Request(url, data=payload,
                                     headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                data = json.load(resp)
        except Exception as exc:
            return f"[bridge error] Ollama request failed: {exc}"
        reply = data.get("message", {}).get("content", "").strip()
        self.history.append({"role": "assistant", "content": reply})
        return reply

    def _openai_chat(self, user_msg: str) -> str:
        self.history.append({"role": "user", "content": user_msg})
        payload = json.dumps({
            "model": self.model or "gpt-4o-mini",
            "messages": [{"role": "system", "content": SYSTEM_PROMPT}] + self.history,
        }).encode()
        url = "https://api.openai.com/v1/chat/completions"
        req = urllib.request.Request(url, data=payload, headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {self.key}",
        })
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                data = json.load(resp)
        except urllib.error.HTTPError as exc:
            return f"[bridge error] OpenAI HTTP {exc.code}: {exc.read().decode()[:200]}"
        except Exception as exc:
            return f"[bridge error] {exc}"
        reply = data["choices"][0]["message"]["content"].strip()
        self.history.append({"role": "assistant", "content": reply})
        return reply

    def _anthropic_chat(self, user_msg: str) -> str:
        self.history.append({"role": "user", "content": user_msg})
        payload = json.dumps({
            "model": self.model or "claude-haiku-4-5",
            "max_tokens": 1024,
            "system": SYSTEM_PROMPT,
            "messages": self.history,
        }).encode()
        url = "https://api.anthropic.com/v1/messages"
        req = urllib.request.Request(url, data=payload, headers={
            "Content-Type": "application/json",
            "x-api-key": self.key,
            "anthropic-version": "2023-06-01",
        })
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                data = json.load(resp)
        except urllib.error.HTTPError as exc:
            return f"[bridge error] Anthropic HTTP {exc.code}: {exc.read().decode()[:200]}"
        except Exception as exc:
            return f"[bridge error] {exc}"
        reply = data["content"][0]["text"].strip()
        self.history.append({"role": "assistant", "content": reply})
        return reply

    def ask(self, user_msg: str) -> str:
        if self.api == "openai":
            return self._openai_chat(user_msg)
        elif self.api == "anthropic":
            return self._anthropic_chat(user_msg)
        else:
            return self._ollama_chat(user_msg)


# ── Response splitting ─────────────────────────────────────────────────────────

def split_response(text: str):
    """
    Yield (kind, payload) tuples:
      ('text',  line_str)   — prose to send as GRESP:<line>
      ('tool',  frame_str)  — tool call to send as GTOOL:<t>:<a>, then wait for GRSLT
    """
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        parsed = parse_tool_line(line)
        if parsed:
            yield ('tool', tool_to_frame(parsed[0], parsed[1]))
        else:
            # Wrap at ~72 chars to avoid VGA line overflow
            for chunk in textwrap.wrap(line, width=72) or [line]:
                yield ('text', chunk)


# ── Bridge main loop ───────────────────────────────────────────────────────────

def run_bridge(ai: AIBackend, serial_port: int) -> None:
    log(CYAN, "BRIDGE", f"Connecting to QEMU COM2 at 127.0.0.1:{serial_port} ...")

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.connect(("127.0.0.1", serial_port))
        sock.settimeout(None)
        f = sock.makefile("rwb", buffering=0)

        log(GREEN, "BRIDGE", "Connected. Waiting for kernel handshake ...")

        for raw in f:
            line = raw.rstrip(b"\r\n").decode("utf-8", errors="replace")

            if line.startswith("GHELO:"):
                kernel_id = line[6:]
                log(GREEN, "HELO", f"Kernel identified as: {kernel_id!r}")
                f.write(b"GOKAY:chat-bridge-v1\n")
                f.flush()
                log(GREEN, "BRIDGE", "Handshake complete. Listening for GCHAT frames ...")

            elif line.startswith("GCHAT:"):
                user_msg = line[6:].strip()
                if not user_msg:
                    f.write(b"GDONE:\n")
                    f.flush()
                    continue

                log(YELLOW, "USER", user_msg)

                try:
                    reply_text = ai.ask(user_msg)
                except Exception as exc:
                    reply_text = f"[bridge error] {exc}"

                log(GREEN, "AI", reply_text[:120] + ("..." if len(reply_text) > 120 else ""))

                for kind, payload in split_response(reply_text):
                    if kind == "text":
                        frame = f"GRESP:{payload}\n".encode()
                        f.write(frame)
                        f.flush()
                    elif kind == "tool":
                        log(CYAN, "TOOL", f"Requesting: {payload}")
                        f.write(f"{payload}\n".encode())
                        f.flush()
                        # Wait for GRSLT
                        try:
                            result_raw = f.readline()
                            result = result_raw.rstrip(b"\r\n").decode("utf-8", errors="replace")
                            if result.startswith("GRSLT:"):
                                log(GREY, "RSLT", result[6:])
                        except Exception:
                            pass

                f.write(b"GDONE:\n")
                f.flush()

            elif line.startswith("GRSLT:"):
                # Unexpected result frame — log and ignore
                log(GREY, "RSLT?", line[6:])

            else:
                if line:
                    log(GREY, "RAW", line)


# ── Entry point ────────────────────────────────────────────────────────────────

def main() -> None:
    p = argparse.ArgumentParser(description="GOS Chat Bridge")
    p.add_argument("--api",         default="ollama",
                   choices=["ollama", "openai", "anthropic"],
                   help="AI API back-end (default: ollama)")
    p.add_argument("--model",       default="",
                   help="Model name (default: qwen2.5:7b for Ollama)")
    p.add_argument("--key",         default="",
                   help="API key (required for openai/anthropic)")
    p.add_argument("--host",        default="127.0.0.1",
                   help="API server host (default: 127.0.0.1)")
    p.add_argument("--port",        default=11434, type=int,
                   help="API server port (default: 11434 for Ollama)")
    p.add_argument("--serial-port", default=14444, type=int,
                   help="QEMU COM2 TCP port (default: 14444)")
    args = p.parse_args()

    # Resolve default model per back-end
    model_defaults = {
        "ollama":    "qwen2.5:7b",
        "openai":    "gpt-4o-mini",
        "anthropic": "claude-haiku-4-5",
    }
    model = args.model or model_defaults[args.api]

    log(CYAN, "CONFIG", f"api={args.api}  model={model}  "
        f"host={args.host}:{args.port}  com2-tcp={args.serial_port}")

    ai = AIBackend(args.api, model, args.key, args.host, args.port)

    while True:
        try:
            run_bridge(ai, args.serial_port)
        except ConnectionRefusedError:
            log(RED, "ERROR", "Connection refused — is QEMU running with -serial tcp:127.0.0.1:14444,server,nowait?")
            sys.exit(1)
        except (BrokenPipeError, ConnectionResetError, EOFError):
            log(YELLOW, "BRIDGE", "Kernel disconnected. Waiting for next boot ...")
            # Loop and reconnect on next QEMU boot
        except KeyboardInterrupt:
            log(YELLOW, "BRIDGE", "Shutting down.")
            sys.exit(0)
        except Exception as exc:
            log(RED, "ERROR", str(exc))
            sys.exit(1)


if __name__ == "__main__":
    main()
