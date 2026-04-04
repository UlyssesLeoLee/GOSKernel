#![no_std]

use gos_protocol::{
    packet_to_signal, signal_to_packet, CYPHER_CONTROL_QUERY_BEGIN, CYPHER_CONTROL_QUERY_COMMIT,
    EdgeVector, ExecStatus, ExecutorContext, ExecutorId, GraphEdgeSummary, GraphNodeSummary,
    KernelAbi, NodeEvent, NodeExecutorVTable, RuntimeEdgeType, Signal, VectorAddress,
};

pub const NODE_VEC: VectorAddress = VectorAddress::new(6, 6, 0, 0);
const VGA_FALLBACK_VEC: VectorAddress = VectorAddress::new(1, 1, 0, 0);
const DEFAULT_LIMIT: usize = 6;
const MAX_LIMIT: usize = 12;

pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.cypher");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(cypher_on_init),
    on_event: Some(cypher_on_event),
    on_suspend: Some(cypher_on_suspend),
    on_resume: None,
    on_teardown: None,
};

#[repr(C)]
struct CypherState {
    console_target: u64,
    query: [u8; 224],
    query_len: usize,
    capture_active: bool,
    executions: usize,
    faults: usize,
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

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut CypherState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut CypherState) }
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

fn emit_console(sink: &ConsoleSink, signal: Signal) {
    sink.emit(signal);
}

fn print_byte(sink: &ConsoleSink, byte: u8) {
    emit_console(
        sink,
        Signal::Data {
            from: sink.from,
            byte,
        },
    );
}

fn print_str(sink: &ConsoleSink, text: &str) {
    for byte in text.bytes() {
        print_byte(sink, byte);
    }
}

fn set_color(sink: &ConsoleSink, fg: u8, bg: u8) {
    emit_console(sink, Signal::Control { cmd: 1, val: fg });
    emit_console(sink, Signal::Control { cmd: 2, val: bg });
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

fn print_vector(sink: &ConsoleSink, vector: VectorAddress) {
    print_num(sink, vector.l4 as usize);
    print_byte(sink, b'.');
    print_num(sink, vector.l3 as usize);
    print_byte(sink, b'.');
    print_num(sink, vector.l2 as usize);
    print_byte(sink, b'.');
    print_num(sink, vector.offset as usize);
}

fn print_edge_vector(sink: &ConsoleSink, vector: EdgeVector) {
    print_str(sink, "e:");
    print_num(sink, vector.l4 as usize);
    print_byte(sink, b'.');
    print_num(sink, vector.l3 as usize);
    print_byte(sink, b'.');
    print_num(sink, vector.l2 as usize);
    print_byte(sink, b'.');
    print_num(sink, vector.offset as usize);
}

fn print_runtime_edge_type(sink: &ConsoleSink, edge_type: RuntimeEdgeType) {
    let label = match edge_type {
        RuntimeEdgeType::Call => "call",
        RuntimeEdgeType::Spawn => "spawn",
        RuntimeEdgeType::Depend => "depend",
        RuntimeEdgeType::Signal => "signal",
        RuntimeEdgeType::Return => "return",
        RuntimeEdgeType::Mount => "mount",
        RuntimeEdgeType::Sync => "sync",
        RuntimeEdgeType::Stream => "stream",
    };
    print_str(sink, label);
}

fn print_node_type(sink: &ConsoleSink, node_type: gos_protocol::RuntimeNodeType) {
    let label = match node_type {
        gos_protocol::RuntimeNodeType::Hardware => "hw",
        gos_protocol::RuntimeNodeType::Driver => "drv",
        gos_protocol::RuntimeNodeType::Service => "svc",
        gos_protocol::RuntimeNodeType::PluginEntry => "entry",
        gos_protocol::RuntimeNodeType::Compute => "compute",
        gos_protocol::RuntimeNodeType::Router => "router",
        gos_protocol::RuntimeNodeType::Aggregator => "agg",
        gos_protocol::RuntimeNodeType::Vector => "vector",
    };
    print_str(sink, label);
}

fn print_lifecycle(sink: &ConsoleSink, lifecycle: gos_protocol::NodeLifecycle) {
    let label = match lifecycle {
        gos_protocol::NodeLifecycle::Discovered => "discover",
        gos_protocol::NodeLifecycle::Loaded => "loaded",
        gos_protocol::NodeLifecycle::Registered => "register",
        gos_protocol::NodeLifecycle::Allocated => "alloc",
        gos_protocol::NodeLifecycle::Ready => "ready",
        gos_protocol::NodeLifecycle::Running => "run",
        gos_protocol::NodeLifecycle::Waiting => "wait",
        gos_protocol::NodeLifecycle::Suspended => "suspend",
        gos_protocol::NodeLifecycle::Terminated => "term",
        gos_protocol::NodeLifecycle::Faulted => "fault",
    };
    print_str(sink, label);
}

fn print_node_brief(sink: &ConsoleSink, summary: &GraphNodeSummary) {
    print_vector(sink, summary.vector);
    print_byte(sink, b' ');
    print_str(sink, summary.plugin_name);
    print_byte(sink, b'/');
    print_str(sink, summary.local_node_key);
    print_byte(sink, b' ');
    print_node_type(sink, summary.node_type);
    print_byte(sink, b' ');
    print_lifecycle(sink, summary.lifecycle);
    print_byte(sink, b'\n');
}

fn print_node_detail(sink: &ConsoleSink, summary: &GraphNodeSummary) {
    set_color(sink, 11, 0);
    print_str(sink, "cypher> node\n");
    set_color(sink, 7, 0);
    print_str(sink, "  vector: ");
    print_vector(sink, summary.vector);
    print_str(sink, "\n  plugin: ");
    print_str(sink, summary.plugin_name);
    print_str(sink, "\n  key: ");
    print_str(sink, summary.local_node_key);
    print_str(sink, "\n  type: ");
    print_node_type(sink, summary.node_type);
    print_str(sink, "\n  state: ");
    print_lifecycle(sink, summary.lifecycle);
    print_str(sink, "\n  exports: ");
    print_num(sink, summary.export_count);
    print_byte(sink, b'\n');
}

fn print_edge_brief(sink: &ConsoleSink, summary: &GraphEdgeSummary) {
    print_edge_vector(sink, summary.edge_vector);
    print_byte(sink, b' ');
    print_runtime_edge_type(sink, summary.edge_type);
    print_byte(sink, b' ');
    print_vector(sink, summary.from_vector);
    print_str(sink, " -> ");
    print_vector(sink, summary.to_vector);
    if let (Some(namespace), Some(binding)) = (summary.capability_namespace, summary.capability_binding)
    {
        print_str(sink, " cap=");
        print_str(sink, namespace);
        print_byte(sink, b'/');
        print_str(sink, binding);
    }
    print_byte(sink, b'\n');
}

fn print_edge_detail(sink: &ConsoleSink, summary: &GraphEdgeSummary) {
    set_color(sink, 11, 0);
    print_str(sink, "cypher> edge\n");
    set_color(sink, 7, 0);
    print_str(sink, "  vector: ");
    print_edge_vector(sink, summary.edge_vector);
    print_str(sink, "\n  type: ");
    print_runtime_edge_type(sink, summary.edge_type);
    print_str(sink, "\n  from: ");
    print_vector(sink, summary.from_vector);
    print_str(sink, " (");
    print_str(sink, summary.from_key);
    print_str(sink, ")\n  to: ");
    print_vector(sink, summary.to_vector);
    print_str(sink, " (");
    print_str(sink, summary.to_key);
    print_str(sink, ")");
    if let (Some(namespace), Some(binding)) = (summary.capability_namespace, summary.capability_binding)
    {
        print_str(sink, "\n  cap: ");
        print_str(sink, namespace);
        print_byte(sink, b'/');
        print_str(sink, binding);
    }
    print_byte(sink, b'\n');
}

fn ascii_lower(byte: u8) -> u8 {
    byte.to_ascii_lowercase()
}

fn starts_with_ci(text: &str, needle: &str) -> bool {
    let text = text.as_bytes();
    let needle = needle.as_bytes();
    if needle.len() > text.len() {
        return false;
    }
    for idx in 0..needle.len() {
        if ascii_lower(text[idx]) != ascii_lower(needle[idx]) {
            return false;
        }
    }
    true
}

fn find_ci(text: &str, needle: &str) -> Option<usize> {
    let text = text.as_bytes();
    let needle = needle.as_bytes();
    if needle.is_empty() || needle.len() > text.len() {
        return None;
    }
    let end = text.len() - needle.len();
    for start in 0..=end {
        let mut matched = true;
        for idx in 0..needle.len() {
            if ascii_lower(text[start + idx]) != ascii_lower(needle[idx]) {
                matched = false;
                break;
            }
        }
        if matched {
            return Some(start);
        }
    }
    None
}

fn contains_ci(text: &str, needle: &str) -> bool {
    find_ci(text, needle).is_some()
}

fn extract_quoted_value_ci<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let bytes = text.as_bytes();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let slice = text.get(cursor..)?;
        let relative = find_ci(slice, key)?;
        let mut idx = cursor + relative + key.len();
        while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        if idx >= bytes.len() || (bytes[idx] != b':' && bytes[idx] != b'=') {
            cursor = cursor + relative + key.len();
            continue;
        }
        idx += 1;
        while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        if idx >= bytes.len() {
            return None;
        }
        let quote = bytes[idx];
        if quote != b'\'' && quote != b'"' {
            return None;
        }
        let start = idx + 1;
        let mut end = start;
        while end < bytes.len() && bytes[end] != quote {
            end += 1;
        }
        return text.get(start..end);
    }
    None
}

fn extract_node_vector(query: &str) -> Option<VectorAddress> {
    let literal = extract_quoted_value_ci(query, "vector")?.trim();
    if starts_with_ci(literal, "e:") {
        return None;
    }
    VectorAddress::parse(literal)
}

fn extract_edge_vector(query: &str) -> Option<EdgeVector> {
    let literal = extract_quoted_value_ci(query, "vector")?.trim();
    let trimmed = if starts_with_ci(literal, "e:") {
        literal.get(2..)?
    } else {
        literal
    };
    EdgeVector::parse(trimmed)
}

fn parse_limit(query: &str) -> usize {
    let Some(start) = find_ci(query, "limit") else {
        return DEFAULT_LIMIT;
    };
    let bytes = query.as_bytes();
    let mut idx = start + 5;
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }

    let mut value = 0usize;
    let mut seen_digit = false;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        seen_digit = true;
        value = value
            .saturating_mul(10)
            .saturating_add(usize::from(bytes[idx] - b'0'));
        idx += 1;
    }

    if !seen_digit {
        DEFAULT_LIMIT
    } else {
        value.clamp(1, MAX_LIMIT)
    }
}

fn edge_signal(edge_type: RuntimeEdgeType) -> Signal {
    match edge_type {
        RuntimeEdgeType::Call => Signal::Call {
            from: NODE_VEC.as_u64(),
        },
        _ => Signal::Spawn { payload: 0 },
    }
}

fn print_help(sink: &ConsoleSink) {
    set_color(sink, 11, 0);
    print_str(sink, "cypher> supported subset\n");
    set_color(sink, 7, 0);
    print_str(sink, "  MATCH (n) RETURN n [LIMIT 6]\n");
    print_str(sink, "  MATCH (n {vector:'6.1.0.0'}) RETURN n\n");
    print_str(sink, "  MATCH ()-[e]-() RETURN e [LIMIT 6]\n");
    print_str(sink, "  MATCH (n {vector:'6.1.0.0'})-[e]-() RETURN e\n");
    print_str(sink, "  MATCH ()-[e {vector:'e:6.1.0.0'}]-() RETURN e\n");
    print_str(sink, "  MATCH (n {vector:'6.1.0.0'}) CALL activate(n)\n");
    print_str(sink, "  MATCH (n {vector:'6.1.0.0'}) CALL spawn(n)\n");
    print_str(sink, "  MATCH ()-[e {vector:'e:6.1.0.0'}]-() CALL route(e)\n");
}

fn run_query(sink: &ConsoleSink, state: &mut CypherState, query: &str) {
    let query = query.trim();
    if query.is_empty() {
        print_help(sink);
        return;
    }

    if !starts_with_ci(query, "match") {
        set_color(sink, 12, 0);
        print_str(sink, "cypher> only MATCH-based queries are supported in v1\n");
        set_color(sink, 7, 0);
        print_help(sink);
        state.faults = state.faults.saturating_add(1);
        return;
    }

    state.executions = state.executions.saturating_add(1);

    if contains_ci(query, "call activate(n)") {
        let Some(vector) = extract_node_vector(query) else {
            set_color(sink, 12, 0);
            print_str(sink, "cypher> activate requires node vector filter\n");
            return;
        };
        match gos_runtime::activate(vector) {
            Ok(_) => {
                set_color(sink, 10, 0);
                print_str(sink, "cypher> activate ok ");
                print_vector(sink, vector);
                print_byte(sink, b'\n');
            }
            Err(_) => {
                set_color(sink, 12, 0);
                print_str(sink, "cypher> activate failed ");
                print_vector(sink, vector);
                print_byte(sink, b'\n');
                state.faults = state.faults.saturating_add(1);
            }
        }
        set_color(sink, 7, 0);
        return;
    }

    if contains_ci(query, "call spawn(n)") {
        let Some(vector) = extract_node_vector(query) else {
            set_color(sink, 12, 0);
            print_str(sink, "cypher> spawn requires node vector filter\n");
            return;
        };
        match gos_runtime::post_signal(vector, Signal::Spawn { payload: 0 }) {
            Ok(_) => {
                gos_runtime::pump();
                set_color(sink, 10, 0);
                print_str(sink, "cypher> spawn ok ");
                print_vector(sink, vector);
                print_byte(sink, b'\n');
            }
            Err(_) => {
                set_color(sink, 12, 0);
                print_str(sink, "cypher> spawn failed ");
                print_vector(sink, vector);
                print_byte(sink, b'\n');
                state.faults = state.faults.saturating_add(1);
            }
        }
        set_color(sink, 7, 0);
        return;
    }

    if contains_ci(query, "call route(e)") {
        let Some(edge_vector) = extract_edge_vector(query) else {
            set_color(sink, 12, 0);
            print_str(sink, "cypher> route requires edge vector filter\n");
            return;
        };
        let Some(summary) = gos_runtime::edge_summary(edge_vector) else {
            set_color(sink, 12, 0);
            print_str(sink, "cypher> edge not found ");
            print_edge_vector(sink, edge_vector);
            print_byte(sink, b'\n');
            state.faults = state.faults.saturating_add(1);
            return;
        };
        let Some(edge_id) = gos_runtime::edge_id_for_vector(edge_vector) else {
            set_color(sink, 12, 0);
            print_str(sink, "cypher> edge id missing ");
            print_edge_vector(sink, edge_vector);
            print_byte(sink, b'\n');
            state.faults = state.faults.saturating_add(1);
            return;
        };
        match gos_runtime::route_edge(edge_id, edge_signal(summary.edge_type)) {
            Ok(_) => {
                gos_runtime::pump();
                set_color(sink, 10, 0);
                print_str(sink, "cypher> routed ");
                print_edge_vector(sink, edge_vector);
                print_str(sink, " as ");
                print_runtime_edge_type(sink, summary.edge_type);
                print_byte(sink, b'\n');
            }
            Err(_) => {
                set_color(sink, 12, 0);
                print_str(sink, "cypher> route failed ");
                print_edge_vector(sink, edge_vector);
                print_byte(sink, b'\n');
                state.faults = state.faults.saturating_add(1);
            }
        }
        set_color(sink, 7, 0);
        return;
    }

    if contains_ci(query, "return e") {
        if let Some(edge_vector) = extract_edge_vector(query) {
            if let Some(summary) = gos_runtime::edge_summary(edge_vector) {
                print_edge_detail(sink, &summary);
            } else {
                set_color(sink, 12, 0);
                print_str(sink, "cypher> edge not found ");
                print_edge_vector(sink, edge_vector);
                print_byte(sink, b'\n');
                state.faults = state.faults.saturating_add(1);
            }
            set_color(sink, 7, 0);
            return;
        }

        let limit = parse_limit(query);
        let mut edges = [GraphEdgeSummary::EMPTY; MAX_LIMIT];

        if let Some(node_vector) = extract_node_vector(query) {
            match gos_runtime::edge_page_for_node(node_vector, 0, &mut edges) {
                Ok((total, returned)) => {
                    set_color(sink, 11, 0);
                    print_str(sink, "cypher> edges for ");
                    print_vector(sink, node_vector);
                    print_str(sink, " returned ");
                    print_num(sink, returned.min(limit));
                    print_str(sink, " of ");
                    print_num(sink, total);
                    print_byte(sink, b'\n');
                    set_color(sink, 7, 0);
                    for summary in edges.iter().take(returned.min(limit)) {
                        print_edge_brief(sink, summary);
                    }
                }
                Err(_) => {
                    set_color(sink, 12, 0);
                    print_str(sink, "cypher> node not found ");
                    print_vector(sink, node_vector);
                    print_byte(sink, b'\n');
                    state.faults = state.faults.saturating_add(1);
                }
            }
            set_color(sink, 7, 0);
            return;
        }

        let (total, returned) = gos_runtime::edge_page(0, &mut edges);
        set_color(sink, 11, 0);
        print_str(sink, "cypher> edge list returned ");
        print_num(sink, returned.min(limit));
        print_str(sink, " of ");
        print_num(sink, total);
        print_byte(sink, b'\n');
        set_color(sink, 7, 0);
        for summary in edges.iter().take(returned.min(limit)) {
            print_edge_brief(sink, summary);
        }
        return;
    }

    if contains_ci(query, "return n") {
        if let Some(vector) = extract_node_vector(query) {
            if let Some(summary) = gos_runtime::node_summary(vector) {
                print_node_detail(sink, &summary);
            } else {
                set_color(sink, 12, 0);
                print_str(sink, "cypher> node not found ");
                print_vector(sink, vector);
                print_byte(sink, b'\n');
                state.faults = state.faults.saturating_add(1);
            }
            set_color(sink, 7, 0);
            return;
        }

        let limit = parse_limit(query);
        let mut nodes = [GraphNodeSummary::EMPTY; MAX_LIMIT];
        let (total, returned) = gos_runtime::node_page(0, &mut nodes);
        set_color(sink, 11, 0);
        print_str(sink, "cypher> node list returned ");
        print_num(sink, returned.min(limit));
        print_str(sink, " of ");
        print_num(sink, total);
        print_byte(sink, b'\n');
        set_color(sink, 7, 0);
        for summary in nodes.iter().take(returned.min(limit)) {
            print_node_brief(sink, summary);
        }
        return;
    }

    set_color(sink, 12, 0);
    print_str(sink, "cypher> unsupported MATCH clause\n");
    set_color(sink, 7, 0);
    print_help(sink);
    state.faults = state.faults.saturating_add(1);
}

unsafe extern "C" fn cypher_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
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

    unsafe {
        core::ptr::write(
            (*ctx).state_ptr as *mut CypherState,
            CypherState {
                console_target: if console_target == 0 {
                    VGA_FALLBACK_VEC.as_u64()
                } else {
                    console_target
                },
                query: [0; 224],
                query_len: 0,
                capture_active: false,
                executions: 0,
                faults: 0,
            },
        );
    }

    ExecStatus::Done
}

unsafe extern "C" fn cypher_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    let sink = sink_from_ctx(ctx);
    let state = unsafe { state_mut(ctx) };
    let signal = packet_to_signal(unsafe { (*event).signal });

    match signal {
        Signal::Spawn { .. } => ExecStatus::Done,
        Signal::Control { cmd, .. } if cmd == CYPHER_CONTROL_QUERY_BEGIN => {
            state.query = [0; 224];
            state.query_len = 0;
            state.capture_active = true;
            ExecStatus::Done
        }
        Signal::Control { cmd, .. } if cmd == CYPHER_CONTROL_QUERY_COMMIT => {
            state.capture_active = false;
            let query_len = state.query_len.min(state.query.len());
            let mut query_buf = [0u8; 224];
            query_buf[..query_len].copy_from_slice(&state.query[..query_len]);
            if let Ok(query) = core::str::from_utf8(&query_buf[..query_len]) {
                run_query(&sink, state, query);
            } else {
                set_color(&sink, 12, 0);
                print_str(&sink, "cypher> query payload must be utf-8 ascii subset\n");
                set_color(&sink, 7, 0);
                state.faults = state.faults.saturating_add(1);
            }
            ExecStatus::Done
        }
        Signal::Data { byte, .. } => {
            if state.capture_active && state.query_len < state.query.len() {
                state.query[state.query_len] = byte;
                state.query_len += 1;
            }
            ExecStatus::Done
        }
        _ => ExecStatus::Done,
    }
}

unsafe extern "C" fn cypher_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}
