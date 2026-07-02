// gos-graph-path-harness — V2.31 BFS shortest-path API tests
//
// Verifies `gos_runtime::find_graph_path` — the BFS-based graph path-finder
// added to the runtime in V2.31.  Analogous to `traceroute` / `pathping`:
// traces the sequence of graph-node hops between two vector addresses.
//
//  1. Empty graph (no edges) → path length 0.
//  2. Self-path (from == to) → length 1, single-element path.
//  3. Direct edge A→B → path [A, B], length 2.
//  4. path[0] == from_vector for a valid path.
//  5. path[len-1] == to_vector for a valid path.
//  6. Two-hop chain A→B→C, query A→C → length 3, path [A, B, C].
//  7. BFS finds shorter path: A→B→C and A→C direct, query A→C → length 2.
//  8. Unregistered from_vec → length 0.
//  9. Unregistered to_vec → length 0.
// 10. Directed edges: only A→B; query B→A → length 0 (not traversable).

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const GP_PLUGIN: PluginId   = PluginId::from_ascii("KL_GPATH_H");
const GP_EXEC:   ExecutorId = ExecutorId::from_ascii("gp.exec");

const GP_KEY_A: &str = "gp.alpha";
const GP_KEY_B: &str = "gp.beta";
const GP_KEY_C: &str = "gp.gamma";
const GP_KEY_D: &str = "gp.delta";

const GP_ID_A: NodeId = derive_node_id(GP_PLUGIN, GP_KEY_A);
const GP_ID_B: NodeId = derive_node_id(GP_PLUGIN, GP_KEY_B);
const GP_ID_C: NodeId = derive_node_id(GP_PLUGIN, GP_KEY_C);
const GP_ID_D: NodeId = derive_node_id(GP_PLUGIN, GP_KEY_D);

const GP_VEC_A: VectorAddress = VectorAddress::new(10, 3, 1, 0);
const GP_VEC_B: VectorAddress = VectorAddress::new(10, 3, 2, 0);
const GP_VEC_C: VectorAddress = VectorAddress::new(10, 3, 3, 0);
const GP_VEC_D: VectorAddress = VectorAddress::new(10, 3, 4, 0);
const GP_VEC_UNKNOWN: VectorAddress = VectorAddress::new(99, 9, 9, 9);

fn node_spec(key: &'static str, node_id: NodeId, schema: u64) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key: key,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: GP_EXEC,
        state_schema_hash: schema,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    }
}

const GP_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    GP_PLUGIN,
    name:         "kl-graph-path-harness",
    version:      1,
    depends_on:   &[],
    permissions:  &[],
    exports:      &[],
    imports:      &[],
    nodes:        &[],
    edges:        &[],
    signature:    None,
    policy_hash:  [0u8; 16],
};

fn reset() {
    gos_runtime::reset();
}

fn register_abcd() {
    gos_runtime::discover_plugin(GP_MANIFEST).unwrap();
    gos_runtime::register_node(GP_PLUGIN, GP_VEC_A, node_spec(GP_KEY_A, GP_ID_A, 0xA001)).unwrap();
    gos_runtime::register_node(GP_PLUGIN, GP_VEC_B, node_spec(GP_KEY_B, GP_ID_B, 0xA002)).unwrap();
    gos_runtime::register_node(GP_PLUGIN, GP_VEC_C, node_spec(GP_KEY_C, GP_ID_C, 0xA003)).unwrap();
    gos_runtime::register_node(GP_PLUGIN, GP_VEC_D, node_spec(GP_KEY_D, GP_ID_D, 0xA004)).unwrap();
}

fn edge(from: NodeId, to: NodeId, key: &'static str) -> EdgeSpec {
    EdgeSpec {
        edge_id:              derive_edge_id(from, to, key),
        from_node:            from,
        to_node:              to,
        edge_type:            RuntimeEdgeType::Signal,
        weight:               1.0,
        acl_mask:             u64::MAX,
        route_policy:         RoutePolicy::Direct,
        capability_namespace: None,
        capability_binding:   None,
        vector_ref:           None,
    }
}

fn add_edge(from: NodeId, to: NodeId, key: &'static str) {
    gos_runtime::register_edge(edge(from, to, key)).unwrap();
}

fn path_len(from: VectorAddress, to: VectorAddress) -> usize {
    let (_, len) = gos_runtime::find_graph_path::<32>(from, to);
    len
}

fn path_arr(from: VectorAddress, to: VectorAddress) -> ([VectorAddress; 32], usize) {
    gos_runtime::find_graph_path::<32>(from, to)
}

// ── 1. Empty graph (no edges) → path length 0 ────────────────────────────────

#[test]
fn empty_graph_no_path() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcd();
    // Nodes exist but no edges → no path A→B.
    assert_eq!(path_len(GP_VEC_A, GP_VEC_B), 0, "no edges → path length must be 0");
}

// ── 2. Self-path (from == to) → length 1 ─────────────────────────────────────

#[test]
fn self_path_returns_one_hop() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcd();
    let (p, len) = path_arr(GP_VEC_A, GP_VEC_A);
    assert_eq!(len, 1, "from == to must produce a path of length 1");
    assert_eq!(p[0], GP_VEC_A, "self-path must contain the source vector");
}

// ── 3. Direct edge A→B → path [A, B], length 2 ───────────────────────────────

#[test]
fn direct_edge_returns_two_hops() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcd();
    add_edge(GP_ID_A, GP_ID_B, "gp.ab");
    let (p, len) = path_arr(GP_VEC_A, GP_VEC_B);
    assert_eq!(len, 2, "single direct edge A→B must produce path of length 2");
    assert_eq!(p[0], GP_VEC_A, "hop 0 must be A");
    assert_eq!(p[1], GP_VEC_B, "hop 1 must be B");
}

// ── 4. path[0] == from_vector for a valid path ───────────────────────────────

#[test]
fn path_starts_with_from_vector() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcd();
    add_edge(GP_ID_A, GP_ID_B, "gp.ab2");
    let (p, len) = path_arr(GP_VEC_A, GP_VEC_B);
    assert!(len >= 1, "path must be non-empty");
    assert_eq!(p[0], GP_VEC_A, "first element must be from_vector");
}

// ── 5. path[len-1] == to_vector for a valid path ─────────────────────────────

#[test]
fn path_ends_with_to_vector() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcd();
    add_edge(GP_ID_A, GP_ID_B, "gp.ab3");
    let (p, len) = path_arr(GP_VEC_A, GP_VEC_B);
    assert!(len >= 1, "path must be non-empty");
    assert_eq!(p[len - 1], GP_VEC_B, "last element must be to_vector");
}

// ── 6. Two-hop chain A→B→C, query A→C → length 3, path [A, B, C] ─────────────

#[test]
fn two_hop_chain_path() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcd();
    add_edge(GP_ID_A, GP_ID_B, "gp.ab4");
    add_edge(GP_ID_B, GP_ID_C, "gp.bc4");
    let (p, len) = path_arr(GP_VEC_A, GP_VEC_C);
    assert_eq!(len, 3, "chain A→B→C must produce path of length 3");
    assert_eq!(p[0], GP_VEC_A, "hop 0 must be A");
    assert_eq!(p[1], GP_VEC_B, "hop 1 must be B");
    assert_eq!(p[2], GP_VEC_C, "hop 2 must be C");
}

// ── 7. BFS finds shorter path when direct shortcut exists ─────────────────────

#[test]
fn bfs_finds_shorter_path() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcd();
    // Longer path: A→B→C (3 hops)
    add_edge(GP_ID_A, GP_ID_B, "gp.ab5");
    add_edge(GP_ID_B, GP_ID_C, "gp.bc5");
    // Shorter path: A→C directly (2 hops)
    add_edge(GP_ID_A, GP_ID_C, "gp.ac5");
    let (_, len) = path_arr(GP_VEC_A, GP_VEC_C);
    assert_eq!(len, 2, "BFS must prefer A→C (2 hops) over A→B→C (3 hops)");
}

// ── 8. Unregistered from_vec → length 0 ──────────────────────────────────────

#[test]
fn unregistered_from_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcd();
    add_edge(GP_ID_A, GP_ID_B, "gp.ab6");
    let len = path_len(GP_VEC_UNKNOWN, GP_VEC_B);
    assert_eq!(len, 0, "unknown from_vec must produce path length 0");
}

// ── 9. Unregistered to_vec → length 0 ────────────────────────────────────────

#[test]
fn unregistered_to_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcd();
    add_edge(GP_ID_A, GP_ID_B, "gp.ab7");
    let len = path_len(GP_VEC_A, GP_VEC_UNKNOWN);
    assert_eq!(len, 0, "unknown to_vec must produce path length 0");
}

// ── 10. Directed edges: only A→B, query B→A → length 0 ──────────────────────

#[test]
fn reverse_direction_not_traversable() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcd();
    add_edge(GP_ID_A, GP_ID_B, "gp.ab8");
    // No B→A edge — directed graph.
    let len = path_len(GP_VEC_B, GP_VEC_A);
    assert_eq!(len, 0, "directed edge A→B must not be traversable as B→A");
}
