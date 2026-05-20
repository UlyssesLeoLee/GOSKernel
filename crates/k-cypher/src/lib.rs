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

    if !(is_create_mount || is_create_use || is_link || is_delete_edge || is_rebind_use) {
        return false;
    }

    state.executions = state.executions.saturating_add(1);

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
