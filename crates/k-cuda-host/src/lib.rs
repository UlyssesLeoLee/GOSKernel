#![no_std]

mod pre;
mod proc;
mod post;

// ============================================================
// GOS KERNEL TOPOLOGY — k-cuda-host
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_CUDA", name: "k-cuda-host"})
// SET p.executor = "k_cuda_host::EXECUTOR_ID", p.node_type = "Compute", p.state_schema = "0x2016"
//
// -- Dependencies
// MERGE (dep_K_VGA:Plugin {id: "K_VGA"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_VGA)
// MERGE (dep_K_SERIAL:Plugin {id: "K_SERIAL"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_SERIAL)
//
// -- Hardware Resources
//
// -- Exported Capabilities (APIs)
// MERGE (cap_cuda_bridge:Capability {namespace: "cuda", name: "bridge"})
// MERGE (p)-[:EXPORTS]->(cap_cuda_bridge)
//
// -- Imported Capabilities (Dependencies)
// MERGE (cap_console_write:Capability {namespace: "console", name: "write"})
// MERGE (p)-[:IMPORTS]->(cap_console_write)
// MERGE (cap_serial_write:Capability {namespace: "serial", name: "write"})
// MERGE (p)-[:IMPORTS]->(cap_serial_write)
// ============================================================


use gos_protocol::{
    signal_to_packet, ExecStatus, ExecutorContext, ExecutorId, KernelAbi,
    NodeEvent, NodeExecutorVTable, Signal, VectorAddress,
};

pub const NODE_VEC: VectorAddress = VectorAddress::new(6, 7, 0, 0);
const VGA_FALLBACK_VEC: VectorAddress = VectorAddress::new(1, 1, 0, 0);
const MAX_JOB_BYTES: usize = 160;

pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.cuda");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(cuda_host_on_init),
    on_event: Some(cuda_host_on_event),
    on_suspend: Some(cuda_host_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

#[repr(C)]
struct CudaHostState {
    console_target: u64,
    serial_target: u64,
    capture: [u8; MAX_JOB_BYTES],
    capture_len: usize,
    capture_active: u8,
    capture_truncated: u8,
    jobs_submitted: u32,
    last_payload_len: u16,
}

#[derive(Clone, Copy)]
struct ConsoleSink {
    target: u64,
    from: u64,
    abi: &'static KernelAbi,
}

impl ConsoleSink {
    fn emit_to(&self, target: u64, signal: Signal) -> bool {
        if target == 0 {
            return false;
        }
        if let Some(emit_signal) = self.abi.emit_signal {
            unsafe { emit_signal(target, signal_to_packet(signal)) == 0 }
        } else {
            false
        }
    }

    fn emit_console(&self, signal: Signal) -> bool {
        self.emit_to(self.target, signal)
    }
}

struct LineBuf<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> LineBuf<N> {
    fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }

    fn push_byte(&mut self, byte: u8) {
        if self.len < N {
            self.bytes[self.len] = byte;
            self.len += 1;
        }
    }

    fn push_str(&mut self, text: &str) {
        for byte in text.bytes() {
            self.push_byte(byte);
        }
    }

    fn push_dec(&mut self, mut value: u64) {
        let mut buf = [0u8; 20];
        let mut len = 0usize;
        if value == 0 {
            self.push_byte(b'0');
            return;
        }
        while value > 0 {
            buf[len] = b'0' + (value % 10) as u8;
            value /= 10;
            len += 1;
        }
        while len > 0 {
            len -= 1;
            self.push_byte(buf[len]);
        }
    }

    fn push_payload_ascii(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.push_byte(sanitize_host_byte(*byte));
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut CudaHostState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut CudaHostState) }
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

fn emit_console_byte(sink: &ConsoleSink, byte: u8) {
    let _ = sink.emit_console(Signal::Data {
        from: sink.from,
        byte,
    });
}

fn emit_console_str(sink: &ConsoleSink, text: &str) {
    for byte in text.bytes() {
        emit_console_byte(sink, byte);
    }
}

fn set_color(sink: &ConsoleSink, fg: u8, bg: u8) {
    let _ = sink.emit_console(Signal::Control { cmd: 1, val: fg });
    let _ = sink.emit_console(Signal::Control { cmd: 2, val: bg });
}

fn emit_console_num(sink: &ConsoleSink, mut value: u64) {
    let mut buf = [0u8; 20];
    let mut len = 0usize;
    if value == 0 {
        emit_console_byte(sink, b'0');
        return;
    }
    while value > 0 {
        buf[len] = b'0' + (value % 10) as u8;
        value /= 10;
        len += 1;
    }
    while len > 0 {
        len -= 1;
        emit_console_byte(sink, buf[len]);
    }
}

fn emit_console_vector(sink: &ConsoleSink, vector: VectorAddress) {
    emit_console_num(sink, vector.l4 as u64);
    emit_console_byte(sink, b'.');
    emit_console_num(sink, vector.l3 as u64);
    emit_console_byte(sink, b'.');
    emit_console_num(sink, vector.l2 as u64);
    emit_console_byte(sink, b'.');
    emit_console_num(sink, vector.offset as u64);
}

fn emit_target_bytes(sink: &ConsoleSink, target: u64, bytes: &[u8]) -> bool {
    for byte in bytes {
        if !sink.emit_to(
            target,
            Signal::Data {
                from: sink.from,
                byte: *byte,
            },
        ) {
            return false;
        }
    }
    true
}

fn sanitize_host_byte(byte: u8) -> u8 {
    if byte == b'"' || byte == b'|' || byte == b'\\' {
        b'_'
    } else if byte.is_ascii_graphic() || byte == b' ' {
        byte
    } else if byte.is_ascii_whitespace() {
        b' '
    } else {
        b'#'
    }
}

fn begin_capture(state: &mut CudaHostState) {
    state.capture = [0; MAX_JOB_BYTES];
    state.capture_len = 0;
    state.capture_active = 1;
    state.capture_truncated = 0;
}

fn clear_capture(state: &mut CudaHostState) {
    state.capture = [0; MAX_JOB_BYTES];
    state.capture_len = 0;
    state.capture_active = 0;
    state.capture_truncated = 0;
}

fn append_capture_byte(state: &mut CudaHostState, byte: u8) {
    if state.capture_active == 0 {
        return;
    }
    if state.capture_len < state.capture.len() {
        state.capture[state.capture_len] = byte;
        state.capture_len += 1;
    } else {
        state.capture_truncated = 1;
    }
}

fn emit_serial_hello(sink: &ConsoleSink, state: &CudaHostState) {
    if state.serial_target == 0 {
        return;
    }
    let mut line = LineBuf::<96>::new();
    line.push_str("@gos.cuda hello vector=");
    line.push_dec(NODE_VEC.l4 as u64);
    line.push_byte(b'.');
    line.push_dec(NODE_VEC.l3 as u64);
    line.push_byte(b'.');
    line.push_dec(NODE_VEC.l2 as u64);
    line.push_byte(b'.');
    line.push_dec(NODE_VEC.offset as u64);
    line.push_str(" transport=serial\n");
    let _ = emit_target_bytes(sink, state.serial_target, line.as_slice());
}

fn emit_status_report(sink: &ConsoleSink, state: &CudaHostState) {
    set_color(sink, 11, 0);
    emit_console_str(sink, "cuda> host bridge\n");
    set_color(sink, 7, 0);
    emit_console_str(sink, "  vector: ");
    emit_console_vector(sink, NODE_VEC);
    emit_console_str(sink, "\n  transport: ");
    emit_console_str(sink, if state.serial_target == 0 { "serial-unresolved" } else { "serial-host" });
    emit_console_str(sink, "\n  capture: ");
    emit_console_str(sink, if state.capture_active != 0 { "open" } else { "idle" });
    emit_console_str(sink, "  jobs: ");
    emit_console_num(sink, state.jobs_submitted as u64);
    emit_console_str(sink, "  last-bytes: ");
    emit_console_num(sink, state.last_payload_len as u64);
    emit_console_str(sink, "\n  path: shell -> cuda.bridge -> serial host frame\n");
    emit_console_str(sink, "  cmds: cuda status / cuda submit <job> / cuda demo / cuda reset\n");
    if state.capture_truncated != 0 {
        emit_console_str(sink, "  note: current capture already hit the size cap\n");
    }
}

fn emit_reset_frame(sink: &ConsoleSink, state: &CudaHostState) {
    if state.serial_target == 0 {
        return;
    }
    let mut line = LineBuf::<80>::new();
    line.push_str("@gos.cuda reset submitted=");
    line.push_dec(state.jobs_submitted as u64);
    line.push_str("\n");
    let _ = emit_target_bytes(sink, state.serial_target, line.as_slice());
}

fn commit_capture(sink: &ConsoleSink, state: &mut CudaHostState) {
    state.capture_active = 0;

    if state.capture_len == 0 {
        set_color(sink, 12, 0);
        emit_console_str(sink, "cuda> empty job payload\n");
        set_color(sink, 7, 0);
        return;
    }
    if state.serial_target == 0 {
        set_color(sink, 12, 0);
        emit_console_str(sink, "cuda> serial host bridge unresolved\n");
        set_color(sink, 7, 0);
        return;
    }

    state.jobs_submitted = state.jobs_submitted.wrapping_add(1);
    state.last_payload_len = state.capture_len as u16;

    let mut line = LineBuf::<320>::new();
    line.push_str("@gos.cuda submit id=");
    line.push_dec(state.jobs_submitted as u64);
    line.push_str(" bytes=");
    line.push_dec(state.capture_len as u64);
    line.push_str(" trunc=");
    line.push_dec(state.capture_truncated as u64);
    line.push_str(" payload=\"");
    line.push_payload_ascii(&state.capture[..state.capture_len]);
    line.push_str("\"\n");

    if emit_target_bytes(sink, state.serial_target, line.as_slice()) {
        set_color(sink, 10, 0);
        emit_console_str(sink, "cuda> host job submitted #");
        emit_console_num(sink, state.jobs_submitted as u64);
        set_color(sink, 7, 0);
        emit_console_str(sink, " bytes=");
        emit_console_num(sink, state.capture_len as u64);
        if state.capture_truncated != 0 {
            emit_console_str(sink, " trunc=1");
        }
        emit_console_str(sink, "\n");
    } else {
        set_color(sink, 12, 0);
        emit_console_str(sink, "cuda> serial host bridge write failed\n");
        set_color(sink, 7, 0);
    }

    clear_capture(state);
}

unsafe extern "C" fn cuda_host_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
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

    let serial_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"serial".as_ptr(),
                    b"serial".len(),
                    b"write".as_ptr(),
                    b"write".len(),
                )
            }
        } else {
            0
        }
    };

    unsafe {
        core::ptr::write(
            (*ctx).state_ptr as *mut CudaHostState,
            CudaHostState {
                console_target: if console_target == 0 {
                    VGA_FALLBACK_VEC.as_u64()
                } else {
                    console_target
                },
                serial_target,
                capture: [0; MAX_JOB_BYTES],
                capture_len: 0,
                capture_active: 0,
                capture_truncated: 0,
                jobs_submitted: 0,
                last_payload_len: 0,
            },
        );
    }

    ExecStatus::Done
}

unsafe extern "C" fn cuda_host_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    let Some(input)  = (unsafe { pre::prepare(ctx, event) })  else { return ExecStatus::Done; };
    let Some(output) = (unsafe { proc::process(ctx, input) }) else { return ExecStatus::Done; };
    unsafe { post::emit(ctx, output) }
}

unsafe extern "C" fn cuda_host_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

#[cfg(test)]
mod tests {
    use super::{sanitize_host_byte, LineBuf};

    #[test]
    fn host_frame_sanitizes_reserved_bytes() {
        assert_eq!(sanitize_host_byte(b'"'), b'_');
        assert_eq!(sanitize_host_byte(b'|'), b'_');
        assert_eq!(sanitize_host_byte(0xF0), b'#');
    }

    #[test]
    fn line_buf_serializes_decimal_and_payload() {
        let mut line = LineBuf::<64>::new();
        line.push_str("id=");
        line.push_dec(42);
        line.push_str(" payload=\"");
        line.push_payload_ascii(b"a|b");
        line.push_str("\"");
        assert_eq!(line.as_slice(), b"id=42 payload=\"a_b\"");
    }
}
