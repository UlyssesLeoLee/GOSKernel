#!/usr/bin/env python3
"""Smoke test for chat-bridge.py's Gemini backend.

Usage (PowerShell):
    $env:GEMINI_API_KEY = "<your key>"
    python tools/test-gemini-bridge.py

Usage (bash):
    GEMINI_API_KEY=<your key> python tools/test-gemini-bridge.py

Reads the key from GEMINI_API_KEY or GOOGLE_API_KEY; never prints it.
Sends one trivial prompt to confirm the endpoint/model/auth/response
parsing in chat-bridge.py's _gemini_chat are wired correctly.
"""
import importlib.util
import os
import sys

spec = importlib.util.spec_from_file_location("chat_bridge", "tools/chat-bridge.py")
cb = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cb)

key = os.environ.get("GEMINI_API_KEY") or os.environ.get("GOOGLE_API_KEY") or ""
if not key:
    print("[test] set GEMINI_API_KEY or GOOGLE_API_KEY first")
    sys.exit(1)

ai = cb.AIBackend("gemini", "gemini-2.5-flash", key, "127.0.0.1", 0)
reply = ai.ask("Reply with exactly these three words: GOS bridge online")
print(f"[test] reply: {reply!r}")
