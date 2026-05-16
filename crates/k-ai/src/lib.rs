#![no_std]

mod pre;
mod proc;
mod post;


// ============================================================
// GOS KERNEL TOPOLOGY — k-ai
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_AI", name: "k-ai"})
// SET p.executor = "k_ai::EXECUTOR_ID", p.node_type = "Aggregator", p.state_schema = "0x2011"
//
// -- Dependencies
// MERGE (dep_K_SHELL:Plugin {id: "K_SHELL"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_SHELL)
//
// -- Hardware Resources
//
// -- Exported Capabilities (APIs)
// MERGE (cap_ai_supervisor:Capability {namespace: "ai", name: "supervisor"})
// MERGE (p)-[:EXPORTS]->(cap_ai_supervisor)
// MERGE (cap_graph_orchestrate:Capability {namespace: "graph", name: "orchestrate"})
// MERGE (p)-[:EXPORTS]->(cap_graph_orchestrate)
//
// -- Imported Capabilities (Dependencies)
// MERGE (cap_console_write:Capability {namespace: "console", name: "write"})
// MERGE (p)-[:IMPORTS]->(cap_console_write)
// MERGE (cap_shell_input:Capability {namespace: "shell", name: "input"})
// MERGE (p)-[:IMPORTS]->(cap_shell_input)
// ============================================================


use core::cell::UnsafeCell;

use gos_protocol::{
    signal_to_packet, ControlPlaneMessageKind, ExecStatus,
    ExecutorContext, ExecutorId, KernelAbi, NodeEvent, NodeExecutorVTable, Signal,
    VectorAddress,
};

// ── Phase F.6: direct TCP transport to a local LLM endpoint ─────────────────
//
// Default target: QEMU SLIRP gateway 10.0.2.2 with Ollama on port 11434.
// Users can place any OpenAI-compatible server (Ollama, llama.cpp, vLLM)
// on the host and reach it transparently.

const OLLAMA_IP:   [u8; 4] = [10, 0, 2, 2];
const OLLAMA_PORT: u16      = 11434;
const DEFAULT_MODEL: &[u8]  = b"qwen2.5:7b";

const AI_HTTP_REQ_BUF:  usize = 2048;
const AI_HTTP_RESP_BUF: usize = 8192;

struct ByteBuf<const N: usize>(UnsafeCell<[u8; N]>);
unsafe impl<const N: usize> Sync for ByteBuf<N> {}
static AI_REQ:  ByteBuf<AI_HTTP_REQ_BUF>  = ByteBuf(UnsafeCell::new([0u8; AI_HTTP_REQ_BUF]));
static AI_RESP: ByteBuf<AI_HTTP_RESP_BUF> = ByteBuf(UnsafeCell::new([0u8; AI_HTTP_RESP_BUF]));

fn ai_buf_append(buf: &mut [u8], pos: &mut usize, data: &[u8]) -> bool {
    if *pos + data.len() > buf.len() { return false; }
    buf[*pos..*pos + data.len()].copy_from_slice(data);
    *pos += data.len();
    true
}

fn ai_buf_append_u32(buf: &mut [u8], pos: &mut usize, mut val: u32) -> bool {
    let mut tmp = [0u8; 10];
    let mut len = 0usize;
    if val == 0 { tmp[0] = b'0'; len = 1; }
    else { while val > 0 { tmp[len] = b'0' + (val % 10) as u8; val /= 10; len += 1; } }
    let mut out = [0u8; 10];
    for i in 0..len { out[i] = tmp[len - 1 - i]; }
    ai_buf_append(buf, pos, &out[..len])
}

fn ai_buf_append_json(buf: &mut [u8], pos: &mut usize, s: &[u8]) -> bool {
    for &b in s {
        let ok = match b {
            b'"'  => ai_buf_append(buf, pos, b"\\\""),
            b'\\' => ai_buf_append(buf, pos, b"\\\\"),
            b'\n' => ai_buf_append(buf, pos, b"\\n"),
            b'\r' => ai_buf_append(buf, pos, b"\\r"),
            other => { if *pos < buf.len() { buf[*pos] = other; *pos += 1; true } else { false } }
        };
        if !ok { return false; }
    }
    true
}

fn build_ai_ollama_body(buf: &mut [u8], model: &[u8], user_msg: &[u8]) -> usize {
    let mut p = 0usize;
    if !ai_buf_append(buf, &mut p, b"{\"model\":\"") { return 0; }
    if !ai_buf_append_json(buf, &mut p, model) { return 0; }
    if !ai_buf_append(buf, &mut p, b"\",\"messages\":[{\"role\":\"system\",\"content\":\"") { return 0; }
    if !ai_buf_append_json(buf, &mut p, b"You are a helpful AI assistant embedded in the GOS graph-native bare-metal kernel. Respond concisely in plain text, no markdown.") { return 0; }
    if !ai_buf_append(buf, &mut p, b"\"},{\"role\":\"user\",\"content\":\"") { return 0; }
    if !ai_buf_append_json(buf, &mut p, user_msg) { return 0; }
    if !ai_buf_append(buf, &mut p, b"\"}],\"stream\":false}") { return 0; }
    p
}

fn build_ai_http_request(
    buf: &mut [u8], path: &[u8], ip: [u8; 4], port: u16,
    auth_hdr: &[u8], body: &[u8],
) -> usize {
    let mut p = 0usize;
    if !ai_buf_append(buf, &mut p, b"POST ") { return 0; }
    if !ai_buf_append(buf, &mut p, path) { return 0; }
    if !ai_buf_append(buf, &mut p, b" HTTP/1.0\r\nHost: ") { return 0; }
    for (i, &o) in ip.iter().enumerate() {
        if i > 0 { buf[p] = b'.'; p += 1; }
        ai_buf_append_u32(buf, &mut p, o as u32);
    }
    buf[p] = b':'; p += 1;
    ai_buf_append_u32(buf, &mut p, port as u32);
    if !ai_buf_append(buf, &mut p, b"\r\nContent-Type: application/json\r\n") { return 0; }
    if !auth_hdr.is_empty() && !ai_buf_append(buf, &mut p, auth_hdr) { return 0; }
    if !ai_buf_append(buf, &mut p, b"Content-Length: ") { return 0; }
    if !ai_buf_append_u32(buf, &mut p, body.len() as u32) { return 0; }
    if !ai_buf_append(buf, &mut p, b"\r\n\r\n") { return 0; }
    if !ai_buf_append(buf, &mut p, body) { return 0; }
    p
}

fn ai_find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() { return Some(0); }
    'outer: for i in 0..hay.len().saturating_sub(needle.len() - 1) {
        for j in 0..needle.len() { if hay[i + j] != needle[j] { continue 'outer; } }
        return Some(i);
    }
    None
}

fn ai_extract_json_str<'a>(json: &'a [u8], key: &[u8]) -> Option<&'a [u8]> {
    let mut pat = [0u8; 128];
    let mut sp = 0usize;
    pat[sp] = b'"'; sp += 1;
    for &b in key { if sp < pat.len() { pat[sp] = b; sp += 1; } }
    if sp + 3 > pat.len() { return None; }
    pat[sp] = b'"'; sp += 1; pat[sp] = b':'; sp += 1; pat[sp] = b'"'; sp += 1;
    let start = ai_find_bytes(json, &pat[..sp])? + sp;
    let mut end = start;
    while end < json.len() {
        if json[end] == b'\\' { end += 2; }
        else if json[end] == b'"' { return Some(&json[start..end]); }
        else { end += 1; }
    }
    None
}

unsafe fn do_llm_request(sink: &ConsoleSink, state: &mut AiState) {
    let req_buf  = unsafe { &mut *AI_REQ.0.get() };
    let resp_buf = unsafe { &mut *AI_RESP.0.get() };

    let user_msg = &state.prompt[..state.prompt_len];

    // Build body into second half of req_buf as scratch
    let body_start = AI_HTTP_REQ_BUF / 2;
    let body_len = build_ai_ollama_body(&mut req_buf[body_start..], DEFAULT_MODEL, user_msg);
    if body_len == 0 {
        emit_shell_chat_line(sink, state, "ai> prompt too large for HTTP body");
        return;
    }
    // Copy body out to avoid aliasing req_buf
    let mut body_scratch = [0u8; 1024];
    let body_len = body_len.min(body_scratch.len());
    body_scratch[..body_len].copy_from_slice(&req_buf[body_start..body_start + body_len]);

    // Optional Bearer auth if api_key is set
    let mut auth_hdr = [0u8; 192];
    let auth_len = if state.api_ready && state.api_len > 0 {
        let mut p = 0usize;
        let ok = ai_buf_append(&mut auth_hdr, &mut p, b"Authorization: Bearer ")
            && ai_buf_append(&mut auth_hdr, &mut p, &state.api_key[..state.api_len])
            && ai_buf_append(&mut auth_hdr, &mut p, b"\r\n");
        if ok { p } else { 0 }
    } else {
        0
    };

    let req_len = build_ai_http_request(
        req_buf,
        b"/api/chat",
        OLLAMA_IP,
        OLLAMA_PORT,
        &auth_hdr[..auth_len],
        &body_scratch[..body_len],
    );
    if req_len == 0 {
        emit_shell_chat_line(sink, state, "ai> HTTP request overflow");
        return;
    }

    emit_shell_chat_str(sink, state, "ai> connecting to llm... ");

    let result = unsafe {
        k_net::net_http_post_sync(OLLAMA_IP, OLLAMA_PORT, &req_buf[..req_len], resp_buf)
    };

    match result {
        None => {
            emit_shell_chat_line(sink, state, "failed (network unavailable)");
        }
        Some(n) => {
            let raw = &resp_buf[..n];
            let body_off = ai_find_bytes(raw, b"\r\n\r\n").map(|i| i + 4).unwrap_or(0);
            let json = &raw[body_off..];
            if let Some(content) = ai_extract_json_str(json, b"content") {
                emit_shell_chat_byte(sink, state, b'\n');
                emit_shell_chat_str(sink, state, "ai> ");
                emit_shell_chat_bytes(sink, state, content, content.len().min(512));
                emit_shell_chat_byte(sink, state, b'\n');
            } else {
                emit_shell_chat_str(sink, state, "ok (");
                emit_shell_chat_num(sink, state, n.saturating_sub(body_off));
                emit_shell_chat_str(sink, state, "b)\n");
            }
        }
    }
}

pub const NODE_VEC: VectorAddress = VectorAddress::new(6, 2, 0, 0);
const VGA_FALLBACK_VEC: VectorAddress = VectorAddress::new(1, 1, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.ai");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(ai_on_init),
    on_event: Some(ai_on_event),
    on_suspend: Some(ai_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

#[repr(C)]
struct AiState {
    console_target: u64,
    shell_target: u64,
    drained_messages: usize,
    plugin_events: usize,
    node_events: usize,
    edge_events: usize,
    state_deltas: usize,
    fault_events: usize,
    shell_handoff_complete: bool,
    api_key: [u8; 128],
    api_len: usize,
    api_capture_active: bool,
    api_ready: bool,
    prompt: [u8; 160],
    prompt_len: usize,
    prompt_capture_active: bool,
}

#[derive(Clone, Copy)]
struct ConsoleSink {
    target: u64,
    from: u64,
    abi: &'static KernelAbi,
}

impl ConsoleSink {
    fn emit(&self, signal: Signal) {
        if let Some(emit_signal) = self.abi.emit_signal {
            unsafe {
                let _ = emit_signal(self.target, signal_to_packet(signal));
            }
        }
    }
}

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut AiState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut AiState) }
}

fn sink_from_ctx(ctx: *mut ExecutorContext) -> ConsoleSink {
    let ctx_ref = unsafe { &*ctx };
    let abi = unsafe { &*ctx_ref.abi };
    let state = unsafe { state_mut(ctx) };
    ConsoleSink {
        target: if state.console_target == 0 {
            VGA_FALLBACK_VEC.as_u64()
        } else {
            state.console_target
        },
        from: ctx_ref.vector.as_u64(),
        abi,
    }
}

fn emit_vga(sink: &ConsoleSink, signal: Signal) {
    sink.emit(signal);
}

fn emit_target_signal(sink: &ConsoleSink, target: u64, signal: Signal) -> bool {
    if target == 0 {
        return false;
    }

    if let Some(emit_signal) = sink.abi.emit_signal {
        unsafe { emit_signal(target, signal_to_packet(signal)) == 0 }
    } else {
        false
    }
}

fn print_byte(sink: &ConsoleSink, byte: u8) {
    emit_vga(sink, Signal::Data { from: sink.from, byte });
}

fn print_str(sink: &ConsoleSink, s: &str) {
    for byte in s.bytes() {
        print_byte(sink, byte);
    }
}

fn set_color(sink: &ConsoleSink, fg: u8, bg: u8) {
    emit_vga(sink, Signal::Control { cmd: 1, val: fg });
    emit_vga(sink, Signal::Control { cmd: 2, val: bg });
}

fn print_num(sink: &ConsoleSink, mut value: usize) {
    let mut buf = [0u8; 20];
    let mut len = 0usize;
    if value == 0 {
        buf[0] = b'0';
        len = 1;
    } else {
        while value > 0 {
            buf[len] = b'0' + (value % 10) as u8;
            value /= 10;
            len += 1;
        }
    }

    while len > 0 {
        len -= 1;
        print_byte(sink, buf[len]);
    }
}

fn print_runtime_brief(sink: &ConsoleSink, state: &AiState) {
    let snapshot = gos_runtime::snapshot();
    set_color(sink, 13, 0);
    print_str(sink, "\n[AI] supervisor online\n");
    set_color(sink, 7, 0);
    print_str(sink, "     mode: kernel-authoritative graph supervisor\n");
    print_str(sink, "     plugins: ");
    print_num(sink, snapshot.plugin_count);
    print_str(sink, " nodes: ");
    print_num(sink, snapshot.node_count);
    print_str(sink, " edges: ");
    print_num(sink, snapshot.edge_count);
    print_str(sink, "\n     control-plane drained: ");
    print_num(sink, state.drained_messages);
    print_str(sink, " api: ");
    print_str(sink, if state.api_ready { "armed" } else { "missing" });
    if state.api_ready {
        print_str(sink, " bytes: ");
        print_num(sink, state.api_len);
    }
    print_str(sink, " stable: ");
    print_str(sink, if gos_runtime::is_stable() { "yes" } else { "no" });
    print_str(sink, "\n");
}

fn drain_control_plane_into(state: &mut AiState) {
    while let Some(message) = gos_runtime::drain_control_plane() {
        state.drained_messages += 1;
        match message.kind {
            ControlPlaneMessageKind::PluginDiscovered => state.plugin_events += 1,
            ControlPlaneMessageKind::NodeUpsert => state.node_events += 1,
            ControlPlaneMessageKind::EdgeUpsert => state.edge_events += 1,
            ControlPlaneMessageKind::StateDelta => state.state_deltas += 1,
            ControlPlaneMessageKind::Fault => state.fault_events += 1,
            _ => {}
        }
    }
}

fn handoff_shell(sink: &ConsoleSink, state: &mut AiState) {
    if state.shell_handoff_complete {
        return;
    }

    if let Some(emit_signal) = sink.abi.emit_signal {
        unsafe {
            let _ = emit_signal(
                k_shell::NODE_VEC.as_u64(),
                signal_to_packet(Signal::Spawn { payload: 0 }),
            );
        }
    }
    state.shell_handoff_complete = true;
    set_color(sink, 10, 0);
    print_str(sink, "     shell handoff: granted by ai supervisor\n");
    set_color(sink, 7, 0);
}

fn begin_api_capture(state: &mut AiState) {
    state.api_key = [0; 128];
    state.api_len = 0;
    state.api_capture_active = true;
    state.api_ready = false;
}

fn append_api_byte(state: &mut AiState, byte: u8) {
    if state.api_capture_active && state.api_len < state.api_key.len() {
        state.api_key[state.api_len] = byte;
        state.api_len += 1;
    }
}

fn commit_api_capture(state: &mut AiState) {
    state.api_capture_active = false;
    state.api_ready = state.api_len > 0;
}

fn begin_chat_capture(state: &mut AiState) {
    state.prompt = [0; 160];
    state.prompt_len = 0;
    state.prompt_capture_active = true;
}

fn append_chat_byte(state: &mut AiState, byte: u8) {
    if state.prompt_capture_active && state.prompt_len < state.prompt.len() {
        state.prompt[state.prompt_len] = byte;
        state.prompt_len += 1;
    }
}

fn ascii_fold(byte: u8) -> u8 {
    byte.to_ascii_lowercase()
}

fn prompt_contains(prompt: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > prompt.len() {
        return false;
    }

    let end = prompt.len() - needle.len();
    for start in 0..=end {
        let mut matched = true;
        for (idx, expected) in needle.iter().enumerate() {
            if ascii_fold(prompt[start + idx]) != *expected {
                matched = false;
                break;
            }
        }
        if matched {
            return true;
        }
    }
    false
}

fn emit_shell_chat_byte(sink: &ConsoleSink, state: &AiState, byte: u8) {
    let _ = emit_target_signal(
        sink,
        state.shell_target,
        Signal::Data {
            from: sink.from,
            byte,
        },
    );
}

fn emit_shell_chat_str(sink: &ConsoleSink, state: &AiState, text: &str) {
    for byte in text.bytes() {
        emit_shell_chat_byte(sink, state, byte);
    }
}

fn emit_shell_chat_bytes(sink: &ConsoleSink, state: &AiState, bytes: &[u8], limit: usize) {
    for byte in bytes.iter().copied().take(limit) {
        let mapped = if byte.is_ascii_graphic() || byte == b' ' {
            byte
        } else if byte >= 0x80 {
            b'#'
        } else {
            b' '
        };
        emit_shell_chat_byte(sink, state, mapped);
    }
}

fn emit_shell_chat_line(sink: &ConsoleSink, state: &AiState, text: &str) {
    emit_shell_chat_str(sink, state, text);
    emit_shell_chat_byte(sink, state, b'\n');
}

fn emit_status_summary(sink: &ConsoleSink, state: &AiState) {
    let snapshot = gos_runtime::snapshot();
    emit_shell_chat_str(sink, state, "ai> graph ");
    emit_shell_chat_str(sink, state, if gos_runtime::is_stable() { "stable " } else { "live " });
    emit_shell_chat_str(sink, state, "p=");
    emit_shell_chat_num(sink, state, snapshot.plugin_count);
    emit_shell_chat_str(sink, state, " n=");
    emit_shell_chat_num(sink, state, snapshot.node_count);
    emit_shell_chat_str(sink, state, " e=");
    emit_shell_chat_num(sink, state, snapshot.edge_count);
    emit_shell_chat_byte(sink, state, b'\n');
}

fn emit_shell_chat_num(sink: &ConsoleSink, state: &AiState, mut value: usize) {
    let mut buf = [0u8; 20];
    let mut len = 0usize;
    if value == 0 {
        buf[0] = b'0';
        len = 1;
    } else {
        while value > 0 {
            buf[len] = b'0' + (value % 10) as u8;
            value /= 10;
            len += 1;
        }
    }

    while len > 0 {
        len -= 1;
        emit_shell_chat_byte(sink, state, buf[len]);
    }
}

fn commit_chat_capture(sink: &ConsoleSink, state: &mut AiState) {
    state.prompt_capture_active = false;

    if state.prompt_len == 0 {
        emit_shell_chat_line(sink, state, "ai> say something after ask");
        return;
    }

    if !state.api_ready {
        emit_shell_chat_line(sink, state, "ai> add an api key first");
        return;
    }

    let prompt = &state.prompt[..state.prompt_len];
    if prompt_contains(prompt, b"graph") || prompt_contains(prompt, b"status") {
        emit_status_summary(sink, state);
    } else if prompt_contains(prompt, b"cuda")
        || prompt_contains(prompt, b"gpu")
        || prompt_contains(prompt, b"nvidia")
    {
        emit_shell_chat_line(
            sink,
            state,
            "ai> cuda bridge is graph-native; use cuda status / cuda submit <job> in shell",
        );
    } else if prompt_contains(prompt, b"net")
        || prompt_contains(prompt, b"wifi")
        || prompt_contains(prompt, b"network")
    {
        emit_shell_chat_line(
            sink,
            state,
            "ai> uplink node can probe/reset e1000 and read mac/link; dhcp/ip/tcp still pending",
        );
    } else if prompt_contains(prompt, b"api") || prompt_contains(prompt, b"key") {
        emit_shell_chat_line(sink, state, "ai> api key is armed for this boot session only");
    } else {
        unsafe { do_llm_request(sink, state); }
    }
}

unsafe extern "C" fn ai_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    let console_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"console".as_ptr(),
                    b"console".len(),
                    b"write".as_ptr(),
                    b"write".len(),
                )
            }
        } else {
            0
        }
    };

    let shell_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"shell".as_ptr(),
                    b"shell".len(),
                    b"input".as_ptr(),
                    b"input".len(),
                )
            }
        } else {
            0
        }
    };

    unsafe {
        core::ptr::write(
            (*ctx).state_ptr as *mut AiState,
            AiState {
                console_target: if console_target == 0 {
                    VGA_FALLBACK_VEC.as_u64()
                } else {
                    console_target
                },
                shell_target,
                drained_messages: 0,
                plugin_events: 0,
                node_events: 0,
                edge_events: 0,
                state_deltas: 0,
                fault_events: 0,
                shell_handoff_complete: false,
                api_key: [0; 128],
                api_len: 0,
                api_capture_active: false,
                api_ready: false,
                prompt: [0; 160],
                prompt_len: 0,
                prompt_capture_active: false,
            },
        );
    }

    ExecStatus::Done
}

unsafe extern "C" fn ai_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    let Some(input)  = (unsafe { pre::prepare(ctx, event) })   else { return ExecStatus::Done; };
    let Some(output) = (unsafe { proc::process(ctx, input) })  else { return ExecStatus::Done; };
    unsafe { post::emit(ctx, output) }
}

unsafe extern "C" fn ai_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}
