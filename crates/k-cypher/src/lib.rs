#![no_std]

mod pre;
mod proc;
mod post;

// ============================================================
// GOS KERNEL TOPOLOGY — k-cypher
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_CYPHER", name: "k-cypher"})
// SET p.executor = "k_cypher::EXECUTOR_ID", p.node_type = "Router", p.state_schema = "0x2014"
//
// -- Dependencies
// MERGE (dep_K_VGA:Plugin {id: "K_VGA"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_VGA)
//
// -- Hardware Resources
//
// -- Exported Capabilities (APIs)
// MERGE (cap_cypher_query:Capability {namespace: "cypher", name: "query"})
// MERGE (p)-[:EXPORTS]->(cap_cypher_query)
//
// -- Imported Capabilities (Dependencies)
// MERGE (cap_console_write:Capability {namespace: "console", name: "write"})
// MERGE (p)-[:IMPORTS]->(cap_console_write)
// ============================================================


use gos_protocol::{
    signal_to_packet,
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
    on_telemetry: None,
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
        RuntimeEdgeType::Use => "use",
        RuntimeEdgeType::Link => "link",
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
    // Telemetry: query node executor for live metrics
    if let Some(telemetry) = gos_runtime::node_telemetry(summary.vector) {
        if telemetry.count > 0 {
            set_color(sink, 14, 0);
            print_str(sink, "  telemetry:\n");
            set_color(sink, 7, 0);
            for i in 0..telemetry.count {
                let entry = &telemetry.entries[i];
                if entry.key.is_empty() { break; }
                print_str(sink, "    ");
                print_str(sink, entry.key);
                print_str(sink, ": ");
                print_num(sink, entry.value as usize);
                match entry.unit {
                    gos_protocol::TelemetryUnit::Bytes => print_str(sink, " B"),
                    gos_protocol::TelemetryUnit::KiB => print_str(sink, " KiB"),
                    gos_protocol::TelemetryUnit::MiB => print_str(sink, " MiB"),
                    gos_protocol::TelemetryUnit::Percent => print_str(sink, " %"),
                    _ => {}
                }
                print_byte(sink, b'\n');
            }
        }
    }
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

/// Extract the `n`-th single- or double-quoted literal from the query
/// (0-indexed).  Used by the H.1.x.3 mutation forms which carry two
/// node vectors positionally (`CREATE MOUNT 'V_from' -> 'V_to'`)
/// rather than via the `vector:'...'` key-value syntax of the read
/// side.  Returns the inner text without quotes.
fn extract_quoted_at(query: &str, n: usize) -> Option<&str> {
    let bytes = query.as_bytes();
    let mut idx = 0usize;
    let mut hit = 0usize;
    while idx < bytes.len() {
        let q = bytes[idx];
        if q == b'\'' || q == b'"' {
            let start = idx + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end] != q {
                end += 1;
            }
            if end >= bytes.len() {
                return None;
            }
            if hit == n {
                return query.get(start..end);
            }
            hit += 1;
            idx = end + 1;
        } else {
            idx += 1;
        }
    }
    None
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
    set_color(sink, 11, 0);
    print_str(sink, "cypher> mutations (audited via supervisor gate)\n");
    set_color(sink, 7, 0);
    print_str(sink, "  CREATE MOUNT 'V_from' -> 'V_to'\n");
    print_str(sink, "  CREATE USE 'V_from' -> 'V_to'\n");
    print_str(sink, "  LINK 'V_node' -> 'V_iface'\n");
    print_str(sink, "  DELETE EDGE 'e:V'\n");
    print_str(sink, "  REBIND USE 'V_from' -> 'V_to'\n");
    print_str(sink, "  CREATE (n)\n");
}

/// Source attribution stamped into every audited mutation envelope
/// originating from the cypher shell.  16 bytes ASCII, null-padded.
const CYPHER_AUDIT_SOURCE: [u8; 16] = *b"K_CYPHER\0\0\0\0\0\0\0\0";

fn try_run_mutation(sink: &ConsoleSink, state: &mut CypherState, query: &str) -> bool {
    use gos_cypher_mut::{CypherMutation, ReceptiveEdgeKind};

    let is_create_mount = contains_ci(query, "create mount");
    let is_create_use = contains_ci(query, "create use");
    // LINK is its own top-level verb (no CREATE prefix) so the user-
    // facing syntax matches the README: `LINK 'V_node' -> 'V_iface'`.
    // Match on `link ` (with trailing space) so the keyword isn't
    // confused with any future `*LINK*` literal substring.
    let is_link = starts_with_ci(query, "link ") || starts_with_ci(query, "link\t");
    let is_delete_edge = contains_ci(query, "delete edge");
    let is_rebind_use = contains_ci(query, "rebind use");
    // V2.5e (ADR-005 option A): `CREATE (n)` allocates a fresh provisional
    // node. Handled separately from the edge verbs below: it has no
    // from/to endpoints to gate on, so it calls gos_runtime directly
    // rather than going through the edge-scoped supervisor gate.
    let is_create_node = contains_ci(query, "create (");

    if !(is_create_mount || is_create_use || is_link || is_delete_edge || is_rebind_use || is_create_node) {
        return false;
    }

    state.executions = state.executions.saturating_add(1);

    if is_create_node {
        match gos_runtime::create_provisional_node(
            gos_protocol::RuntimeNodeType::Vector,
            gos_protocol::EntryPolicy::Manual,
            gos_protocol::ExecutorId::ZERO,
        ) {
            Ok((_id, vector)) => {
                set_color(sink, 10, 0);
                print_str(sink, "cypher> created ");
                print_vector(sink, vector);
                print_byte(sink, b'\n');
                set_color(sink, 7, 0);
            }
            Err(_) => {
                set_color(sink, 12, 0);
                print_str(sink, "cypher> create failed\n");
                set_color(sink, 7, 0);
                state.faults = state.faults.saturating_add(1);
            }
        }
        return true;
    }

    if is_delete_edge {
        let Some(literal) = extract_quoted_at(query, 0) else {
            mutation_fail(sink, state, "delete edge requires 'e:V' literal");
            return true;
        };
        let trimmed = if starts_with_ci(literal, "e:") {
            literal.get(2..).unwrap_or(literal)
        } else {
            literal
        };
        let Some(edge_vector) = EdgeVector::parse(trimmed) else {
            mutation_fail(sink, state, "delete edge: bad edge vector");
            return true;
        };
        let Some(edge_id) = gos_runtime::edge_id_for_vector(edge_vector) else {
            mutation_fail(sink, state, "delete edge: not found");
            return true;
        };
        report_mutation_result(
            sink,
            state,
            "delete edge",
            gos_supervisor::apply_cypher_mutation(
                CypherMutation::RemoveEdge { edge_id },
                CYPHER_AUDIT_SOURCE,
            ),
        );
        return true;
    }

    // CREATE / REBIND all take two positional node-vector literals.
    let Some(lit_from) = extract_quoted_at(query, 0) else {
        mutation_fail(sink, state, "mutation requires 'V_from' literal");
        return true;
    };
    let Some(lit_to) = extract_quoted_at(query, 1) else {
        mutation_fail(sink, state, "mutation requires 'V_to' literal");
        return true;
    };
    let Some(vec_from) = VectorAddress::parse(lit_from) else {
        mutation_fail(sink, state, "bad V_from");
        return true;
    };
    let Some(vec_to) = VectorAddress::parse(lit_to) else {
        mutation_fail(sink, state, "bad V_to");
        return true;
    };
    let Some(id_from) = gos_runtime::node_id_for_vec(vec_from) else {
        mutation_fail(sink, state, "V_from node not found");
        return true;
    };
    let Some(id_to) = gos_runtime::node_id_for_vec(vec_to) else {
        mutation_fail(sink, state, "V_to node not found");
        return true;
    };

    let (label, mutation) = if is_create_mount {
        (
            "create mount",
            CypherMutation::AddEdge {
                from: id_from,
                to: id_to,
                edge_kind: ReceptiveEdgeKind::Mount,
            },
        )
    } else if is_create_use {
        (
            "create use",
            CypherMutation::AddEdge {
                from: id_from,
                to: id_to,
                edge_kind: ReceptiveEdgeKind::Use,
            },
        )
    } else if is_link {
        // LINK V_node -> V_iface: establish a declared
        // correspondence between a runtime node and an
        // interface-file node.  Same supervisor gate as Mount/Use;
        // routing semantics are pass-through (see RuntimeEdgeType::Link).
        (
            "link",
            CypherMutation::AddEdge {
                from: id_from,
                to: id_to,
                edge_kind: ReceptiveEdgeKind::Link,
            },
        )
    } else {
        debug_assert!(is_rebind_use);
        (
            "rebind use",
            CypherMutation::RebindUse {
                from: id_from,
                new_target: id_to,
            },
        )
    };

    report_mutation_result(
        sink,
        state,
        label,
        gos_supervisor::apply_cypher_mutation(mutation, CYPHER_AUDIT_SOURCE),
    );
    true
}

fn mutation_fail(sink: &ConsoleSink, state: &mut CypherState, msg: &str) {
    set_color(sink, 12, 0);
    print_str(sink, "cypher> ");
    print_str(sink, msg);
    print_byte(sink, b'\n');
    set_color(sink, 7, 0);
    state.faults = state.faults.saturating_add(1);
}

fn report_mutation_result(
    sink: &ConsoleSink,
    state: &mut CypherState,
    label: &str,
    result: Result<gos_protocol::EdgeId, gos_cypher_mut::MutationError>,
) {
    match result {
        Ok(_edge_id) => {
            set_color(sink, 10, 0);
            print_str(sink, "cypher> ");
            print_str(sink, label);
            print_str(sink, " ok\n");
            set_color(sink, 7, 0);
        }
        Err(err) => {
            set_color(sink, 12, 0);
            print_str(sink, "cypher> ");
            print_str(sink, label);
            print_str(sink, " rejected: ");
            print_mutation_error(sink, err);
            print_byte(sink, b'\n');
            set_color(sink, 7, 0);
            state.faults = state.faults.saturating_add(1);
        }
    }
}

fn print_mutation_error(sink: &ConsoleSink, err: gos_cypher_mut::MutationError) {
    use gos_cypher_mut::MutationError;
    match err {
        MutationError::UnsupportedMutation => print_str(sink, "unsupported"),
        MutationError::UnknownEndpoint(_) => print_str(sink, "unknown endpoint"),
        MutationError::InvalidMountTarget(_) => print_str(sink, "invalid mount target"),
        MutationError::DispatcherRejected(tag) => {
            print_str(sink, "gate(");
            print_num(sink, tag as usize);
            print_str(sink, ")");
        }
    }
}

// ── Phase I.7 — sink-free Cypher dispatch for the boot UI command bar ─
//
// The hypervisor's I.5 command bar wants to forward typed Cypher
// statements directly into the runtime without dragging the
// ConsoleSink / CypherState machinery the in-VM `cypher>` shell uses.
// `dispatch_cypher_text` is a sink-free public entry that performs
// the same parse + endpoint lookup + supervisor gate, returning a
// flat `CypherDispatchOutcome` the caller can rewrite as one line in
// its own scrollback.
//
// Parsing rules mirror the internal `try_run_mutation` exactly so the
// command bar and the in-VM shell accept the same syntax.

/// Outcome of a public Cypher text dispatch.
#[derive(Debug, Clone, Copy)]
pub enum CypherDispatchOutcome {
    /// Query didn't start with a recognised Cypher verb.
    NotCypher,
    /// Recognised but the literal arguments were malformed (bad
    /// vector address, missing quotes, etc.).  `&'static str` holds
    /// a short human-readable hint.
    BadSyntax(&'static str),
    /// A `'V_from'` / `'V_to'` / `'e:V'` literal didn't resolve to
    /// an existing runtime node or edge.
    EndpointNotFound(&'static str),
    /// Supervisor gate rejected the mutation.
    DispatchFailed(gos_cypher_mut::MutationError),
    /// Mutation applied; static label of the verb (matches the
    /// strings the in-VM `cypher>` shell prints).
    Applied(&'static str),
}

/// Parse `query` as a Cypher mutation and forward it to the
/// supervisor gate using `source` as the audit attribution.  Sink-
/// free analogue of the internal `try_run_mutation` — returns a flat
/// outcome the caller renders however it wants.
///
/// Recognised verbs (case-insensitive, matches in-VM shell):
///   * `CREATE MOUNT 'V_from' -> 'V_to'`
///   * `CREATE USE 'V_from' -> 'V_to'`
///   * `LINK 'V_node' -> 'V_iface'`
///   * `DELETE EDGE 'e:V'`
///   * `REBIND USE 'V_from' -> 'V_to'`
pub fn dispatch_cypher_text(query: &str, source: [u8; 16]) -> CypherDispatchOutcome {
    use gos_cypher_mut::{CypherMutation, ReceptiveEdgeKind};

    let is_create_mount = contains_ci(query, "create mount");
    let is_create_use = contains_ci(query, "create use");
    let is_link = starts_with_ci(query, "link ") || starts_with_ci(query, "link\t");
    let is_delete_edge = contains_ci(query, "delete edge");
    let is_rebind_use = contains_ci(query, "rebind use");

    if !(is_create_mount || is_create_use || is_link || is_delete_edge || is_rebind_use) {
        return CypherDispatchOutcome::NotCypher;
    }

    if is_delete_edge {
        let Some(literal) = extract_quoted_at(query, 0) else {
            return CypherDispatchOutcome::BadSyntax("delete edge requires 'e:V' literal");
        };
        let trimmed = if starts_with_ci(literal, "e:") {
            literal.get(2..).unwrap_or(literal)
        } else {
            literal
        };
        let Some(edge_vector) = EdgeVector::parse(trimmed) else {
            return CypherDispatchOutcome::BadSyntax("delete edge: bad edge vector");
        };
        let Some(edge_id) = gos_runtime::edge_id_for_vector(edge_vector) else {
            return CypherDispatchOutcome::EndpointNotFound("delete edge: not found");
        };
        return match gos_supervisor::apply_cypher_mutation(
            CypherMutation::RemoveEdge { edge_id },
            source,
        ) {
            Ok(_) => CypherDispatchOutcome::Applied("delete edge"),
            Err(e) => CypherDispatchOutcome::DispatchFailed(e),
        };
    }

    let Some(lit_from) = extract_quoted_at(query, 0) else {
        return CypherDispatchOutcome::BadSyntax("mutation requires 'V_from' literal");
    };
    let Some(lit_to) = extract_quoted_at(query, 1) else {
        return CypherDispatchOutcome::BadSyntax("mutation requires 'V_to' literal");
    };
    let Some(vec_from) = VectorAddress::parse(lit_from) else {
        return CypherDispatchOutcome::BadSyntax("bad V_from");
    };
    let Some(vec_to) = VectorAddress::parse(lit_to) else {
        return CypherDispatchOutcome::BadSyntax("bad V_to");
    };
    let Some(id_from) = gos_runtime::node_id_for_vec(vec_from) else {
        return CypherDispatchOutcome::EndpointNotFound("V_from node not found");
    };
    let Some(id_to) = gos_runtime::node_id_for_vec(vec_to) else {
        return CypherDispatchOutcome::EndpointNotFound("V_to node not found");
    };

    let (label, mutation) = if is_create_mount {
        (
            "create mount",
            CypherMutation::AddEdge {
                from: id_from,
                to: id_to,
                edge_kind: ReceptiveEdgeKind::Mount,
            },
        )
    } else if is_create_use {
        (
            "create use",
            CypherMutation::AddEdge {
                from: id_from,
                to: id_to,
                edge_kind: ReceptiveEdgeKind::Use,
            },
        )
    } else if is_link {
        (
            "link",
            CypherMutation::AddEdge {
                from: id_from,
                to: id_to,
                edge_kind: ReceptiveEdgeKind::Link,
            },
        )
    } else {
        debug_assert!(is_rebind_use);
        (
            "rebind use",
            CypherMutation::RebindUse {
                from: id_from,
                new_target: id_to,
            },
        )
    };

    match gos_supervisor::apply_cypher_mutation(mutation, source) {
        Ok(_) => CypherDispatchOutcome::Applied(label),
        Err(e) => CypherDispatchOutcome::DispatchFailed(e),
    }
}

// ══════════════════════════════════════════════════════════════════
// Phase J.1 — read-side Cypher
// ══════════════════════════════════════════════════════════════════
//
// `dispatch_cypher_text` covered the WRITE half (CREATE / LINK /
// DELETE / REBIND).  J.1 adds the READ half so introspection is
// symmetric with mutation — every other kernel feature can query the
// live graph through one canonical surface.
//
// Recognised verbs (case-insensitive, both `SHOW` and `MATCH`
// accepted because users coming from real Cypher expect `MATCH`):
//
//   SHOW NODES                                — every node, one per row
//   SHOW NODES OF CLASS Driver                — filtered by sub-domain
//   SHOW NODES WHERE class=Service            — same, alternate syntax
//   SHOW EDGES                                — every edge, one per row
//   SHOW EDGES OF KIND Use                    — filtered by edge type
//   SHOW EDGES WHERE kind=Mount               — same, alternate syntax
//   SHOW EDGES FROM 'V'                       — outgoing edges from V
//   SHOW EDGES TO 'V'                         — incoming edges to V
//   MATCH NODES … / MATCH EDGES …             — aliases for SHOW
//
// Sink-free design mirrors `dispatch_cypher_text`: callers pass a
// `QueryEmitter` and we push one formatted row per match.  The
// hypervisor's `interpret_command` emitter writes into the chat HUD;
// harness tests collect into a Vec for assertion.

/// One outcome of a public Cypher query dispatch.
#[derive(Debug, Clone, Copy)]
pub enum CypherQueryOutcome {
    /// Query didn't start with a recognised read verb.
    NotQuery,
    /// Recognised but the argument was malformed (bad class name,
    /// bad vector, etc.).
    BadSyntax(&'static str),
    /// A `'V'` literal didn't resolve to an existing runtime node.
    EndpointNotFound(&'static str),
    /// Query executed; `count` is the number of rows emitted via the
    /// caller's `QueryEmitter`.  Zero is a legitimate empty result.
    Rows { count: usize },
}

/// Caller-supplied sink for query rows.  Each call to `emit_row`
/// receives one formatted line of ASCII text.  Implementation can
/// log into a scrollback, push into a Vec, or do anything else.
pub trait QueryEmitter {
    fn emit_row(&mut self, row: &str);
}

/// Parse + execute a Cypher read query against the live runtime.
/// See the verb table above for accepted forms.
pub fn dispatch_cypher_query<E: QueryEmitter>(
    query: &str,
    emitter: &mut E,
) -> CypherQueryOutcome {
    let is_show_nodes = starts_with_ci(query, "show nodes") || starts_with_ci(query, "match nodes");
    let is_show_edges = starts_with_ci(query, "show edges") || starts_with_ci(query, "match edges");
    let is_show_journal = starts_with_ci(query, "show journal") || starts_with_ci(query, "match journal");
    let is_show_plugins = starts_with_ci(query, "show plugins") || starts_with_ci(query, "match plugins");
    let is_show_stats = starts_with_ci(query, "show stats") || starts_with_ci(query, "stats");
    let is_show_capabilities = starts_with_ci(query, "show capabilities")
        || starts_with_ci(query, "show caps");
    let is_set_priority = starts_with_ci(query, "set priority");
    let is_show_priority = starts_with_ci(query, "show priority");
    let is_reset_priority = starts_with_ci(query, "reset priority");
    let is_set_deadline = starts_with_ci(query, "set deadline");
    let is_show_deadline = starts_with_ci(query, "show deadline");
    let is_invoke = starts_with_ci(query, "invoke ") || starts_with_ci(query, "invoke\t");

    if !(is_show_nodes || is_show_edges || is_show_journal || is_show_plugins || is_show_stats || is_show_capabilities || is_set_priority || is_show_priority || is_reset_priority || is_set_deadline || is_show_deadline || is_invoke) {
        return CypherQueryOutcome::NotQuery;
    }

    if is_show_stats {
        return show_stats(emitter);
    }
    if is_show_capabilities {
        return show_capabilities(emitter);
    }
    if is_set_priority {
        return set_priority_action(query, emitter);
    }
    if is_show_priority {
        return show_priority_action(query, emitter);
    }
    if is_reset_priority {
        return reset_priority_action(query, emitter);
    }
    if is_set_deadline {
        return set_deadline_action(query, emitter);
    }
    if is_show_deadline {
        return show_deadline_action(query, emitter);
    }
    if is_invoke {
        return invoke_action(query, emitter);
    }
    if is_show_journal {
        return show_journal(emitter, parse_query_limit(query));
    }
    if is_show_plugins {
        return show_plugins(emitter);
    }

    if is_show_nodes {
        let class_filter = match parse_class_filter(query) {
            Ok(opt) => opt,
            Err(msg) => return CypherQueryOutcome::BadSyntax(msg),
        };
        return show_nodes(emitter, class_filter);
    }

    debug_assert!(is_show_edges);
    // EDGES support three optional filters: kind, FROM 'V', TO 'V'.
    let kind_filter = match parse_kind_filter(query) {
        Ok(opt) => opt,
        Err(msg) => return CypherQueryOutcome::BadSyntax(msg),
    };
    let from_filter = match parse_endpoint_filter(query, "from") {
        Ok(opt) => opt,
        Err(out) => return out,
    };
    let to_filter = match parse_endpoint_filter(query, "to") {
        Ok(opt) => opt,
        Err(out) => return out,
    };
    show_edges(emitter, kind_filter, from_filter, to_filter)
}

// ── Phase L.12 — list all exported capabilities ─────────────────
fn show_capabilities<E: QueryEmitter>(emitter: &mut E) -> CypherQueryOutcome {
    use core::sync::atomic::{AtomicUsize, Ordering as AtomicOrd};
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    COUNT.store(0, AtomicOrd::Relaxed);

    // We need to emit through the trait while iterating gos_runtime;
    // the closure captures &mut E.  Iterate the runtime which calls
    // back into our closure.  Use the emitter inside.
    let mut closure_count = 0usize;
    gos_runtime::for_each_exported_capability(|_pid, plugin_name, cap| {
        let mut row = RowBuf::<80>::new();
        row.push_str(cap.namespace);
        row.push_str("/");
        row.push_str(cap.name);
        row.push_str("@v");
        row.push_dec(cap.version as u64);
        // Pad to ~28 cols before the provider name.
        let used = cap.namespace.len() + cap.name.len() + 4 + decimal_width(cap.version as u64);
        let pad = 32usize.saturating_sub(used);
        for _ in 0..pad {
            row.push_str(" ");
        }
        row.push_str("← ");
        let nm = trim_k_prefix(plugin_name);
        let take = nm.len().min(16);
        row.push_str(&nm[..take]);
        emitter.emit_row(row.as_str());
        closure_count += 1;
    });

    CypherQueryOutcome::Rows { count: closure_count }
}

fn decimal_width(n: u64) -> usize {
    if n == 0 {
        return 1;
    }
    let mut w = 0;
    let mut v = n;
    while v > 0 {
        w += 1;
        v /= 10;
    }
    w
}

// ── Phase K.1 — comprehensive runtime statistics view ────────────
fn show_stats<E: QueryEmitter>(emitter: &mut E) -> CypherQueryOutcome {
    let snap = gos_runtime::snapshot();
    let journal_len = gos_runtime::journal_len();
    let journal_lifetime = gos_runtime::journal_lifetime();
    let generation = gos_runtime::graph_generation();
    let tick = gos_runtime::current_tick();
    let stable = gos_runtime::is_stable();

    let mut row = RowBuf::<80>::new();
    row.push_str("plugins   ");
    row.push_dec(snap.plugin_count as u64);
    row.push_str(" / 32");
    emitter.emit_row(row.as_str());

    let mut row = RowBuf::<80>::new();
    row.push_str("nodes     ");
    row.push_dec(snap.node_count as u64);
    row.push_str(" / 128");
    emitter.emit_row(row.as_str());

    let mut row = RowBuf::<80>::new();
    row.push_str("edges     ");
    row.push_dec(snap.edge_count as u64);
    row.push_str(" / 512");
    emitter.emit_row(row.as_str());

    let mut row = RowBuf::<80>::new();
    row.push_str("ready=");
    row.push_dec(snap.ready_queue_len as u64);
    row.push_str("  signals=");
    row.push_dec(snap.signal_queue_len as u64);
    emitter.emit_row(row.as_str());

    let mut row = RowBuf::<80>::new();
    row.push_str("journal   ");
    row.push_dec(journal_len as u64);
    row.push_str(" stored / ");
    row.push_dec(journal_lifetime);
    row.push_str(" lifetime");
    emitter.emit_row(row.as_str());

    let mut row = RowBuf::<80>::new();
    row.push_str("generation G");
    row.push_dec(generation);
    row.push_str("  tick=");
    row.push_dec(tick);
    emitter.emit_row(row.as_str());

    let mut row = RowBuf::<80>::new();
    row.push_str("stable    ");
    row.push_str(if stable { "yes" } else { "no" });
    emitter.emit_row(row.as_str());

    // L.8 — RPC call counters
    let rpc_word = gos_runtime::rpc_call_count_word();
    let rpc_buf = gos_runtime::rpc_call_count_buf();
    let mut row = RowBuf::<80>::new();
    row.push_str("rpc       word=");
    row.push_dec(rpc_word);
    row.push_str(" buf=");
    row.push_dec(rpc_buf);
    emitter.emit_row(row.as_str());

    // L.4 — deadline overrun counter
    let overruns = gos_runtime::deadline_overrun_count();
    let mut row = RowBuf::<80>::new();
    row.push_str("deadlines overruns=");
    row.push_dec(overruns);
    emitter.emit_row(row.as_str());

    CypherQueryOutcome::Rows { count: 9 }
}

// ── Phase K.2 — set node priority via Cypher ─────────────────────
fn set_priority_action<E: QueryEmitter>(
    query: &str,
    emitter: &mut E,
) -> CypherQueryOutcome {
    let Some(literal) = extract_quoted_at(query, 0) else {
        return CypherQueryOutcome::BadSyntax("set priority requires 'V' literal");
    };
    let Some(vec) = VectorAddress::parse(literal) else {
        return CypherQueryOutcome::BadSyntax("bad vector literal");
    };
    if gos_runtime::node_id_for_vec(vec).is_none() {
        return CypherQueryOutcome::EndpointNotFound("set priority: node not found");
    }
    // Find the value token after '=' or after the literal.
    let n_value = parse_priority_value(query);
    let Some(n) = n_value else {
        return CypherQueryOutcome::BadSyntax("set priority requires '= N' with N in 0..=255");
    };
    if let Err(_) = gos_runtime::set_node_priority(vec, n) {
        return CypherQueryOutcome::EndpointNotFound("set priority: runtime rejected");
    }
    let mut row = RowBuf::<80>::new();
    row.push_str("set priority '");
    row.push_str(literal);
    row.push_str("' = ");
    row.push_dec(n as u64);
    emitter.emit_row(row.as_str());
    CypherQueryOutcome::Rows { count: 1 }
}

// ── K.8 — read / reset a node's priority ─────────────────────────
fn show_priority_action<E: QueryEmitter>(
    query: &str,
    emitter: &mut E,
) -> CypherQueryOutcome {
    let Some(literal) = extract_quoted_at(query, 0) else {
        return CypherQueryOutcome::BadSyntax("show priority requires 'V' literal");
    };
    let Some(vec) = VectorAddress::parse(literal) else {
        return CypherQueryOutcome::BadSyntax("bad vector literal");
    };
    let Some(p) = gos_runtime::node_priority(vec) else {
        return CypherQueryOutcome::EndpointNotFound("show priority: node not found");
    };
    let mut row = RowBuf::<80>::new();
    row.push_str("'");
    row.push_str(literal);
    row.push_str("' priority = ");
    row.push_dec(p as u64);
    // Add a human-readable tier label.
    let tier = if p >= 192 { "HIGH" } else if p >= 128 { "DEFAULT" } else if p >= 64 { "BACKGROUND" } else { "IDLE" };
    row.push_str(" (");
    row.push_str(tier);
    row.push_str(")");
    emitter.emit_row(row.as_str());
    CypherQueryOutcome::Rows { count: 1 }
}

fn reset_priority_action<E: QueryEmitter>(
    query: &str,
    emitter: &mut E,
) -> CypherQueryOutcome {
    let Some(literal) = extract_quoted_at(query, 0) else {
        return CypherQueryOutcome::BadSyntax("reset priority requires 'V' literal");
    };
    let Some(vec) = VectorAddress::parse(literal) else {
        return CypherQueryOutcome::BadSyntax("bad vector literal");
    };
    if gos_runtime::node_id_for_vec(vec).is_none() {
        return CypherQueryOutcome::EndpointNotFound("reset priority: node not found");
    }
    if gos_runtime::set_node_priority(vec, gos_runtime::NODE_PRIORITY_DEFAULT).is_err() {
        return CypherQueryOutcome::EndpointNotFound("reset priority: runtime rejected");
    }
    let mut row = RowBuf::<80>::new();
    row.push_str("reset priority '");
    row.push_str(literal);
    row.push_str("' = 128 (DEFAULT)");
    emitter.emit_row(row.as_str());
    CypherQueryOutcome::Rows { count: 1 }
}

// ── L.4 — Cypher deadline operations ─────────────────────────────
fn set_deadline_action<E: QueryEmitter>(
    query: &str,
    emitter: &mut E,
) -> CypherQueryOutcome {
    let Some(literal) = extract_quoted_at(query, 0) else {
        return CypherQueryOutcome::BadSyntax("set deadline requires 'V' literal");
    };
    let Some(vec) = VectorAddress::parse(literal) else {
        return CypherQueryOutcome::BadSyntax("bad vector literal");
    };
    if gos_runtime::node_id_for_vec(vec).is_none() {
        return CypherQueryOutcome::EndpointNotFound("set deadline: node not found");
    }
    let Some(n) = parse_deadline_value(query) else {
        return CypherQueryOutcome::BadSyntax("set deadline requires '= N' (cycles, 0 disables)");
    };
    if gos_runtime::set_node_deadline(vec, n).is_err() {
        return CypherQueryOutcome::EndpointNotFound("set deadline: runtime rejected");
    }
    let mut row = RowBuf::<80>::new();
    row.push_str("set deadline '");
    row.push_str(literal);
    row.push_str("' = ");
    row.push_dec(n);
    row.push_str(" cycles");
    if n == 0 {
        row.push_str(" (disabled)");
    }
    emitter.emit_row(row.as_str());
    CypherQueryOutcome::Rows { count: 1 }
}

fn show_deadline_action<E: QueryEmitter>(
    query: &str,
    emitter: &mut E,
) -> CypherQueryOutcome {
    let Some(literal) = extract_quoted_at(query, 0) else {
        return CypherQueryOutcome::BadSyntax("show deadline requires 'V' literal");
    };
    let Some(vec) = VectorAddress::parse(literal) else {
        return CypherQueryOutcome::BadSyntax("bad vector literal");
    };
    let Some(cycles) = gos_runtime::node_deadline(vec) else {
        return CypherQueryOutcome::EndpointNotFound("show deadline: node not found");
    };
    let mut row = RowBuf::<80>::new();
    row.push_str("'");
    row.push_str(literal);
    row.push_str("' deadline = ");
    if cycles == 0 {
        row.push_str("(disabled)");
    } else {
        row.push_dec(cycles);
        row.push_str(" cycles");
    }
    emitter.emit_row(row.as_str());
    CypherQueryOutcome::Rows { count: 1 }
}

fn parse_deadline_value(query: &str) -> Option<u64> {
    let eq_pos = query.as_bytes().iter().position(|&b| b == b'=')?;
    let rest = &query[eq_pos + 1..];
    let token = next_token(rest)?;
    token.parse::<u64>().ok()
}

fn parse_priority_value(query: &str) -> Option<u8> {
    // Strategy: find the '=' character, take the next token, parse as decimal.
    let eq_pos = query.as_bytes().iter().position(|&b| b == b'=')?;
    let rest = &query[eq_pos + 1..];
    let token = next_token(rest)?;
    token.parse::<u8>().ok()
}

// ── Phase K.3 — RPC invoke via Cypher ────────────────────────────
fn invoke_action<E: QueryEmitter>(
    query: &str,
    emitter: &mut E,
) -> CypherQueryOutcome {
    let Some(literal) = extract_quoted_at(query, 0) else {
        return CypherQueryOutcome::BadSyntax("invoke requires 'V' literal");
    };
    let Some(vec) = VectorAddress::parse(literal) else {
        return CypherQueryOutcome::BadSyntax("bad vector literal");
    };
    // Parse `WITH N` value (defaults to 0).
    let request_word = parse_with_value(query).unwrap_or(0);
    match gos_runtime::rpc_invoke(vec, request_word) {
        Ok(response) => {
            let mut row = RowBuf::<80>::new();
            row.push_str("invoke '");
            row.push_str(literal);
            row.push_str("' with ");
            row.push_dec(request_word);
            row.push_str(" -> ");
            row.push_dec(response);
            emitter.emit_row(row.as_str());
            CypherQueryOutcome::Rows { count: 1 }
        }
        Err(gos_runtime::RpcError::NoReply) => {
            let mut row = RowBuf::<80>::new();
            row.push_str("invoke '");
            row.push_str(literal);
            row.push_str("': target ran but no rpc_reply");
            emitter.emit_row(row.as_str());
            CypherQueryOutcome::Rows { count: 0 }
        }
        Err(gos_runtime::RpcError::BadStatus) => {
            let mut row = RowBuf::<80>::new();
            row.push_str("invoke '");
            row.push_str(literal);
            row.push_str("': target dispatch returned non-OK");
            emitter.emit_row(row.as_str());
            CypherQueryOutcome::Rows { count: 0 }
        }
        Err(gos_runtime::RpcError::Runtime(_)) => CypherQueryOutcome::EndpointNotFound(
            "invoke: target node not found or runtime error",
        ),
    }
}

fn parse_with_value(query: &str) -> Option<u64> {
    let idx = find_ci(query, "with")?;
    let rest = &query[idx + "with".len()..];
    let token = next_token(rest)?;
    token.parse::<u64>().ok()
}

fn show_journal<E: QueryEmitter>(
    emitter: &mut E,
    limit: usize,
) -> CypherQueryOutcome {
    let stored = gos_runtime::journal_len();
    let lifetime = gos_runtime::journal_lifetime();
    let mut header = RowBuf::<80>::new();
    header.push_str("journal stored=");
    header.push_dec(stored as u64);
    header.push_str(" lifetime=");
    header.push_dec(lifetime);
    emitter.emit_row(header.as_str());

    // Show the most recent `limit` entries (newest at bottom).
    let take = limit.min(stored);
    let start = stored - take;
    let mut count = 1usize; // 1 for the header
    for i in start..stored {
        if let Some(env) = gos_runtime::journal_envelope_at(i) {
            let mut row = RowBuf::<80>::new();
            row.push_str("  ");
            row.push_dec(i as u64);
            row.push_str(": ");
            row.push_str(envelope_kind_label(env.kind));
            row.push_str(" arg0=");
            row.push_dec(env.arg0);
            row.push_str(" arg1=");
            row.push_dec(env.arg1);
            emitter.emit_row(row.as_str());
            count += 1;
        }
    }
    CypherQueryOutcome::Rows { count }
}

fn show_plugins<E: QueryEmitter>(emitter: &mut E) -> CypherQueryOutcome {
    // Walk node_page and tally plugin distinct ids.  Same approach
    // as the hypervisor's `ps` command, but emitted via the Cypher
    // sink so any caller can consume it.
    let mut buf = [GraphNodeSummary::EMPTY; 64];
    let (_total, returned) = gos_runtime::node_page(0, &mut buf);
    let mut seen_ids: [Option<gos_protocol::PluginId>; 64] = [None; 64];
    let mut seen_count: [usize; 64] = [0; 64];
    let mut seen_name: [&'static str; 64] = [""; 64];
    let mut n_seen = 0usize;
    for node in &buf[..returned] {
        let mut idx = None;
        for s in 0..n_seen {
            if seen_ids[s] == Some(node.plugin_id) {
                idx = Some(s);
                break;
            }
        }
        match idx {
            Some(s) => seen_count[s] += 1,
            None => {
                seen_ids[n_seen] = Some(node.plugin_id);
                seen_count[n_seen] = 1;
                seen_name[n_seen] = node.plugin_name;
                n_seen += 1;
            }
        }
    }
    for s in 0..n_seen {
        let mut row = RowBuf::<80>::new();
        let nm = trim_k_prefix(seen_name[s]);
        let take = nm.len().min(14);
        row.push_str(&nm[..take]);
        for _ in take..14 {
            row.push_str(" ");
        }
        row.push_str("x");
        row.push_dec(seen_count[s] as u64);
        emitter.emit_row(row.as_str());
    }
    CypherQueryOutcome::Rows { count: n_seen }
}

fn envelope_kind_label(kind: gos_protocol::ControlPlaneMessageKind) -> &'static str {
    use gos_protocol::ControlPlaneMessageKind::*;
    match kind {
        Hello => "Hello",
        PluginDiscovered => "PluginDiscovered",
        NodeUpsert => "NodeUpsert",
        EdgeUpsert => "EdgeUpsert",
        StateDelta => "StateDelta",
        SnapshotChunk => "SnapshotChunk",
        Fault => "Fault",
        Metric => "Metric",
        MutationAudit => "MutationAudit",
        CausalOverflow => "CausalOverflow",
        RuleApplied => "RuleApplied",
        SubscribeTriggered => "SubscribeTriggered",
    }
}

fn parse_query_limit(query: &str) -> usize {
    // Look for "limit N" or just default to 10.
    if let Some(idx) = find_ci(query, "limit") {
        let rest = &query[idx + "limit".len()..];
        if let Some(tok) = next_token(rest) {
            if let Ok(n) = tok.parse::<usize>() {
                return n;
            }
        }
    }
    10
}

fn show_nodes<E: QueryEmitter>(
    emitter: &mut E,
    class_filter: Option<gos_protocol::NodeSubDomain>,
) -> CypherQueryOutcome {
    let mut buf = [GraphNodeSummary::EMPTY; 64];
    let (_total, returned) = gos_runtime::node_page(0, &mut buf);
    let mut count = 0usize;
    for n in &buf[..returned] {
        if let Some(cls) = class_filter {
            if n.sub_domain != cls {
                continue;
            }
        }
        let mut row = RowBuf::<80>::new();
        push_vec(&mut row, n.vector);
        row.push_str("  ");
        // Pad plugin name to 14 chars.
        let nm = trim_k_prefix(n.plugin_name);
        let take = nm.len().min(14);
        row.push_str(&nm[..take]);
        for _ in take..14 {
            row.push_str(" ");
        }
        row.push_str(node_type_label(n.node_type));
        row.push_str(" / ");
        row.push_str(sub_domain_label(n.sub_domain));
        emitter.emit_row(row.as_str());
        count += 1;
    }
    CypherQueryOutcome::Rows { count }
}

fn show_edges<E: QueryEmitter>(
    emitter: &mut E,
    kind_filter: Option<RuntimeEdgeType>,
    from_filter: Option<VectorAddress>,
    to_filter: Option<VectorAddress>,
) -> CypherQueryOutcome {
    let mut buf = [GraphEdgeSummary::EMPTY; 128];
    let (_total, returned) = gos_runtime::edge_page(0, &mut buf);
    let mut count = 0usize;
    for e in &buf[..returned] {
        if let Some(kind) = kind_filter {
            if e.edge_type != kind {
                continue;
            }
        }
        if let Some(from) = from_filter {
            if e.from_vector != from {
                continue;
            }
        }
        if let Some(to) = to_filter {
            if e.to_vector != to {
                continue;
            }
        }
        let mut row = RowBuf::<80>::new();
        push_vec(&mut row, e.from_vector);
        row.push_str(" -> ");
        push_vec(&mut row, e.to_vector);
        row.push_str("  ");
        row.push_str(edge_type_label(e.edge_type));
        if let Some(ns) = e.capability_namespace {
            row.push_str("  [");
            let take = ns.len().min(20);
            row.push_str(&ns[..take]);
            if let Some(cap) = e.capability_binding {
                row.push_str("/");
                let take2 = cap.len().min(20);
                row.push_str(&cap[..take2]);
            }
            row.push_str("]");
        }
        emitter.emit_row(row.as_str());
        count += 1;
    }
    CypherQueryOutcome::Rows { count }
}

fn parse_class_filter(query: &str) -> Result<Option<gos_protocol::NodeSubDomain>, &'static str> {
    let key = if let Some(idx) = find_ci(query, "of class") {
        idx + "of class".len()
    } else if let Some(idx) = find_ci(query, "where class") {
        let after = idx + "where class".len();
        // Skip optional '=' or ':'
        skip_eq_colon_ws(query, after)
    } else {
        return Ok(None);
    };
    let token = next_token(&query[key..]).ok_or("class filter missing argument")?;
    Ok(Some(match_class_token(token).ok_or("unknown class (try Hardware/Driver/Service/Compute/Routing/Vector)")?))
}

fn parse_kind_filter(query: &str) -> Result<Option<RuntimeEdgeType>, &'static str> {
    let key = if let Some(idx) = find_ci(query, "of kind") {
        idx + "of kind".len()
    } else if let Some(idx) = find_ci(query, "where kind") {
        let after = idx + "where kind".len();
        skip_eq_colon_ws(query, after)
    } else {
        return Ok(None);
    };
    let token = next_token(&query[key..]).ok_or("kind filter missing argument")?;
    Ok(Some(match_kind_token(token).ok_or("unknown kind (try Mount/Use/Signal/Call/Spawn/Link/Depend)")?))
}

fn parse_endpoint_filter(
    query: &str,
    key: &str,
) -> Result<Option<VectorAddress>, CypherQueryOutcome> {
    let Some(literal) = extract_quoted_value_word(query, key) else {
        return Ok(None);
    };
    let Some(vec) = VectorAddress::parse(literal) else {
        return Err(CypherQueryOutcome::BadSyntax("bad vector literal for FROM/TO filter"));
    };
    if gos_runtime::node_id_for_vec(vec).is_none() {
        return Err(CypherQueryOutcome::EndpointNotFound("FROM/TO endpoint not found"));
    }
    Ok(Some(vec))
}

fn skip_eq_colon_ws(query: &str, mut idx: usize) -> usize {
    let bytes = query.as_bytes();
    while idx < bytes.len() && (bytes[idx] == b'=' || bytes[idx] == b':' || bytes[idx].is_ascii_whitespace()) {
        idx += 1;
    }
    idx
}

fn next_token(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let mut start = 0;
    while start < bytes.len() && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    let mut end = start;
    while end < bytes.len() && !bytes[end].is_ascii_whitespace() && bytes[end] != b',' && bytes[end] != b';' {
        end += 1;
    }
    if start == end {
        None
    } else {
        s.get(start..end)
    }
}

fn match_class_token(tok: &str) -> Option<gos_protocol::NodeSubDomain> {
    use gos_protocol::NodeSubDomain;
    let lower_match = |needle: &str| eq_ci(tok, needle);
    if lower_match("hw") || lower_match("hardware") {
        Some(NodeSubDomain::Hardware)
    } else if lower_match("drv") || lower_match("driver") || lower_match("kerneldriver") {
        Some(NodeSubDomain::KernelDriver)
    } else if lower_match("svc") || lower_match("service") {
        Some(NodeSubDomain::Service)
    } else if lower_match("cpu") || lower_match("compute") {
        Some(NodeSubDomain::Compute)
    } else if lower_match("rtr") || lower_match("routing") || lower_match("router") {
        Some(NodeSubDomain::Routing)
    } else if lower_match("vec") || lower_match("vector") {
        Some(NodeSubDomain::Vector)
    } else {
        None
    }
}

fn match_kind_token(tok: &str) -> Option<RuntimeEdgeType> {
    let lower_match = |needle: &str| eq_ci(tok, needle);
    if lower_match("call") {
        Some(RuntimeEdgeType::Call)
    } else if lower_match("spawn") {
        Some(RuntimeEdgeType::Spawn)
    } else if lower_match("depend") {
        Some(RuntimeEdgeType::Depend)
    } else if lower_match("signal") {
        Some(RuntimeEdgeType::Signal)
    } else if lower_match("return") {
        Some(RuntimeEdgeType::Return)
    } else if lower_match("mount") {
        Some(RuntimeEdgeType::Mount)
    } else if lower_match("sync") {
        Some(RuntimeEdgeType::Sync)
    } else if lower_match("stream") {
        Some(RuntimeEdgeType::Stream)
    } else if lower_match("use") {
        Some(RuntimeEdgeType::Use)
    } else if lower_match("link") {
        Some(RuntimeEdgeType::Link)
    } else {
        None
    }
}

fn eq_ci(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    for i in 0..a.len() {
        if ascii_lower(a[i]) != ascii_lower(b[i]) {
            return false;
        }
    }
    true
}

fn extract_quoted_value_word<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    // Look for `key 'V'` (no = required, FROM/TO style).
    let idx = find_ci(query, key)?;
    let after = idx + key.len();
    let bytes = query.as_bytes();
    let mut cursor = after;
    while cursor < bytes.len() && (bytes[cursor].is_ascii_whitespace() || bytes[cursor] == b'=' || bytes[cursor] == b':') {
        cursor += 1;
    }
    if cursor >= bytes.len() {
        return None;
    }
    let q = bytes[cursor];
    if q != b'\'' && q != b'"' {
        return None;
    }
    let start = cursor + 1;
    let mut end = start;
    while end < bytes.len() && bytes[end] != q {
        end += 1;
    }
    query.get(start..end)
}

fn trim_k_prefix(s: &str) -> &str {
    s.strip_prefix("K_").unwrap_or(s)
}

fn node_type_label(t: gos_protocol::RuntimeNodeType) -> &'static str {
    use gos_protocol::RuntimeNodeType;
    match t {
        RuntimeNodeType::Hardware => "HW",
        RuntimeNodeType::Driver => "DRV",
        RuntimeNodeType::Service => "SVC",
        RuntimeNodeType::PluginEntry => "PE",
        RuntimeNodeType::Compute => "CPU",
        RuntimeNodeType::Router => "RTR",
        RuntimeNodeType::Aggregator => "AGG",
        RuntimeNodeType::Vector => "VEC",
    }
}

fn sub_domain_label(s: gos_protocol::NodeSubDomain) -> &'static str {
    use gos_protocol::NodeSubDomain;
    match s {
        NodeSubDomain::Hardware => "Hardware",
        NodeSubDomain::KernelDriver => "Driver",
        NodeSubDomain::Service => "Service",
        NodeSubDomain::Compute => "Compute",
        NodeSubDomain::Routing => "Routing",
        NodeSubDomain::Vector => "Vector",
    }
}

fn edge_type_label(e: RuntimeEdgeType) -> &'static str {
    match e {
        RuntimeEdgeType::Call => "Call",
        RuntimeEdgeType::Spawn => "Spawn",
        RuntimeEdgeType::Depend => "Depend",
        RuntimeEdgeType::Signal => "Signal",
        RuntimeEdgeType::Return => "Return",
        RuntimeEdgeType::Mount => "Mount",
        RuntimeEdgeType::Sync => "Sync",
        RuntimeEdgeType::Stream => "Stream",
        RuntimeEdgeType::Use => "Use",
        RuntimeEdgeType::Link => "Link",
    }
}

fn push_vec<const N: usize>(row: &mut RowBuf<N>, v: VectorAddress) {
    row.push_dec(v.l4 as u64);
    row.push_str(".");
    row.push_dec(v.l3 as u64);
    row.push_str(".");
    row.push_dec(v.l2 as u64);
    row.push_str(".");
    row.push_dec(v.offset as u64);
}

/// Tiny stack-only string builder for query rows.  Truncates silently
/// past `N` bytes; caller's emitter sees the truncated &str.
pub struct RowBuf<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> RowBuf<N> {
    pub fn new() -> Self {
        Self { buf: [0; N], len: 0 }
    }

    pub fn push_str(&mut self, s: &str) {
        for &b in s.as_bytes() {
            if self.len >= N {
                return;
            }
            self.buf[self.len] = b;
            self.len += 1;
        }
    }

    pub fn push_dec(&mut self, n: u64) {
        let mut buf = [0u8; 20];
        let mut i = buf.len();
        let mut v = n;
        if v == 0 {
            self.push_str("0");
            return;
        }
        while v > 0 && i > 0 {
            i -= 1;
            buf[i] = b'0' + (v % 10) as u8;
            v /= 10;
        }
        // SAFETY: ASCII digits.
        let s = unsafe { core::str::from_utf8_unchecked(&buf[i..]) };
        self.push_str(s);
    }

    pub fn as_str(&self) -> &str {
        // SAFETY: only ASCII written via push_str / push_dec.
        unsafe { core::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

fn run_query(sink: &ConsoleSink, state: &mut CypherState, query: &str) {
    let query = query.trim();
    if query.is_empty() {
        print_help(sink);
        return;
    }

    // Phase H.1.x.3 — Cypher writes (CREATE / DELETE / REBIND) are
    // dispatched first.  Anything that matches the mutation prefix
    // never reaches the MATCH gate below.  The audited envelope path
    // (supervisor gate) handles attribution + telemetry.
    if try_run_mutation(sink, state, query) {
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

    // V2.5e (ADR-005 option A): `CREATE (n)` allocates a fresh provisional
    // node directly via gos_runtime — same direct-call style as the CALL
    // verbs above. `register_node` bumps `graph_epoch` synchronously, so the
    // next `vk_auto_refresh` poll (V2.5c) picks the new node up with no pump.
    if contains_ci(query, "create (") {
        match gos_runtime::create_provisional_node(
            gos_protocol::RuntimeNodeType::Vector,
            gos_protocol::EntryPolicy::Manual,
            gos_protocol::ExecutorId::ZERO,
        ) {
            Ok((_id, vector)) => {
                set_color(sink, 10, 0);
                print_str(sink, "cypher> created ");
                print_vector(sink, vector);
                print_byte(sink, b'\n');
            }
            Err(_) => {
                set_color(sink, 12, 0);
                print_str(sink, "cypher> create failed\n");
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
    let Some(input)  = (unsafe { pre::prepare(ctx, event) })  else { return ExecStatus::Done; };
    let Some(output) = (unsafe { proc::process(ctx, input) }) else { return ExecStatus::Done; };
    unsafe { post::emit(ctx, output) }
}

unsafe extern "C" fn cypher_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}
