// gos-graph-global-eff-harness — V2.74 global graph efficiency API tests
//
// Verifies `gos_runtime::graph_global_efficiency`.
//
// E(G) = 1/(n*(n-1)) * Σ_{i≠j, d(i,j)<∞} 1/d(i,j)   (scaled to ppm = ×1_000_000)
//
// Returns (efficiency_ppm: u64, pairs_max: usize, node_count: usize).
// Disconnected pairs contribute 0 — metric is well-defined for all graphs.
//
//  1. Empty graph            → ppm=0, pairs_max=0, node_count=0.
//  2. Single node            → ppm=0, pairs_max=0, node_count=1.
//  3. Two nodes, no edges    → ppm=0, pairs_max=2, node_count=2.
//  4. A→B (one directed edge) → sum=1_000_000/1; ppm=1_000_000/2=500_000.
//  5. A↔B (bidirectional)    → sum=2_000_000; ppm=2_000_000/2=1_000_000 (E=1.0).
//  6. Path A→B→C             → sum=1+0.5+1=2.5M; ppm=2_500_000/6=416_666.
//  7. Directed 3-cycle A→B→C→A → sum=3*(1+0.5)=4.5M; ppm=4_500_000/6=750_000.
//  8. Complete K3 A↔B, B↔C, A↔C → 6 pairs d=1; ppm=6_000_000/6=1_000_000 (E=1.0).
//  9. Disconnected {A→B}∥{C→D}  → only 2 pairs reach; ppm=2_000_000/12=166_666.
// 10. Star-out A→{B,C,D}     → 3 reachable pairs d=1; ppm=3_000_000/12=250_000.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const EFF_PLUGIN: PluginId   = PluginId::from_ascii("KL_EFF0_00");
const EFF_EXEC:   ExecutorId = ExecutorId::from_ascii("eff.exec");

const EFF_KEY_A: &str = "eff.alpha";
const EFF_KEY_B: &str = "eff.beta";
const EFF_KEY_C: &str = "eff.gamma";
const EFF_KEY_D: &str = "eff.delta";

const EFF_ID_A: NodeId = derive_node_id(EFF_PLUGIN, EFF_KEY_A);
const EFF_ID_B: NodeId = derive_node_id(EFF_PLUGIN, EFF_KEY_B);
const EFF_ID_C: NodeId = derive_node_id(EFF_PLUGIN, EFF_KEY_C);
const EFF_ID_D: NodeId = derive_node_id(EFF_PLUGIN, EFF_KEY_D);

// L4=50 identifies this harness namespace in the VectorAddress space.
const EFF_VEC_A: VectorAddress = VectorAddress::new(50, 1, 1, 0);
const EFF_VEC_B: VectorAddress = VectorAddress::new(50, 1, 2, 0);
const EFF_VEC_C: VectorAddress = VectorAddress::new(50, 1, 3, 0);
const EFF_VEC_D: VectorAddress = VectorAddress::new(50, 1, 4, 0);

const EFF_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    EFF_PLUGIN,
    name:         "kl-graph-global-eff-harness",
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

fn node_spec(key: &'static str, node_id: NodeId, schema: u64) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       EFF_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() {
    gos_runtime::reset();
}

fn register_plugin() {
    gos_runtime::discover_plugin(EFF_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(EFF_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
}

fn add_edge(from: NodeId, to: NodeId, key: &'static str) {
    let spec = EdgeSpec {
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
    };
    gos_runtime::register_edge(spec).unwrap();
}

// ── 1. Empty graph ────────────────────────────────────────────────────────────

#[test]
fn empty_graph_zero_efficiency() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (ppm, pairs_max, node_count) = gos_runtime::graph_global_efficiency();
    assert_eq!(node_count, 0, "empty: no nodes");
    assert_eq!(pairs_max, 0, "empty: no pairs");
    assert_eq!(ppm, 0, "empty: E(G)=0");
}

// ── 2. Single node ────────────────────────────────────────────────────────────

#[test]
fn single_node_zero_efficiency() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(EFF_VEC_A, EFF_KEY_A, EFF_ID_A, 0xF001);
    let (ppm, pairs_max, node_count) = gos_runtime::graph_global_efficiency();
    assert_eq!(node_count, 1, "1 node");
    assert_eq!(pairs_max, 0, "n<2: no pairs");
    assert_eq!(ppm, 0, "single node: E(G)=0");
}

// ── 3. Two nodes, no edges → E(G)=0 ──────────────────────────────────────────

#[test]
fn two_nodes_no_edges_zero_efficiency() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(EFF_VEC_A, EFF_KEY_A, EFF_ID_A, 0xF001);
    add_node(EFF_VEC_B, EFF_KEY_B, EFF_ID_B, 0xF002);
    let (ppm, pairs_max, node_count) = gos_runtime::graph_global_efficiency();
    assert_eq!(node_count, 2, "2 nodes");
    assert_eq!(pairs_max, 2, "pairs_max = 2*(2-1) = 2");
    assert_eq!(ppm, 0, "no paths: E(G)=0");
}

// ── 4. A→B: sum=1_000_000, pairs_max=2, ppm=500_000 ─────────────────────────

#[test]
fn one_directed_edge_half_efficiency() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(EFF_VEC_A, EFF_KEY_A, EFF_ID_A, 0xF001);
    add_node(EFF_VEC_B, EFF_KEY_B, EFF_ID_B, 0xF002);
    add_edge(EFF_ID_A, EFF_ID_B, "eff.ab.t4");
    let (ppm, pairs_max, node_count) = gos_runtime::graph_global_efficiency();
    // Only pair A→B is reachable (d=1). B→A is not reachable (no back-edge).
    // sum_recip = 1_000_000 / 1 = 1_000_000.
    // ppm = 1_000_000 / 2 = 500_000.
    assert_eq!(node_count, 2, "2 nodes");
    assert_eq!(pairs_max, 2, "pairs_max=2");
    assert_eq!(ppm, 500_000, "E(G) = 0.5 for one-way edge");
}

// ── 5. A↔B bidirectional → E(G)=1.0, ppm=1_000_000 ─────────────────────────

#[test]
fn bidirectional_two_nodes_full_efficiency() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(EFF_VEC_A, EFF_KEY_A, EFF_ID_A, 0xF001);
    add_node(EFF_VEC_B, EFF_KEY_B, EFF_ID_B, 0xF002);
    add_edge(EFF_ID_A, EFF_ID_B, "eff.ab.t5");
    add_edge(EFF_ID_B, EFF_ID_A, "eff.ba.t5");
    let (ppm, pairs_max, node_count) = gos_runtime::graph_global_efficiency();
    // Both pairs reach each other at d=1.
    // sum_recip = 2 * (1_000_000/1) = 2_000_000. pairs_max=2. ppm=1_000_000.
    assert_eq!(node_count, 2, "2 nodes");
    assert_eq!(pairs_max, 2, "pairs_max=2");
    assert_eq!(ppm, 1_000_000, "complete 2-node graph: E(G)=1.0");
}

// ── 6. Path A→B→C: ppm = 2_500_000/6 = 416_666 ─────────────────────────────

#[test]
fn path_three_nodes_efficiency() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(EFF_VEC_A, EFF_KEY_A, EFF_ID_A, 0xF001);
    add_node(EFF_VEC_B, EFF_KEY_B, EFF_ID_B, 0xF002);
    add_node(EFF_VEC_C, EFF_KEY_C, EFF_ID_C, 0xF003);
    add_edge(EFF_ID_A, EFF_ID_B, "eff.ab.t6");
    add_edge(EFF_ID_B, EFF_ID_C, "eff.bc.t6");
    let (ppm, pairs_max, node_count) = gos_runtime::graph_global_efficiency();
    // Reachable: A→B(d=1), A→C(d=2), B→C(d=1). Unreachable: B→A, C→A, C→B.
    // sum = 1_000_000 + 500_000 + 1_000_000 = 2_500_000. pairs_max=6. ppm=416_666.
    assert_eq!(node_count, 3, "3 nodes");
    assert_eq!(pairs_max, 6, "pairs_max=6");
    assert_eq!(ppm, 416_666, "path A→B→C: E(G)≈0.4167");
}

// ── 7. Directed 3-cycle A→B→C→A: ppm = 4_500_000/6 = 750_000 ───────────────

#[test]
fn directed_cycle_three_nodes_efficiency() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(EFF_VEC_A, EFF_KEY_A, EFF_ID_A, 0xF001);
    add_node(EFF_VEC_B, EFF_KEY_B, EFF_ID_B, 0xF002);
    add_node(EFF_VEC_C, EFF_KEY_C, EFF_ID_C, 0xF003);
    add_edge(EFF_ID_A, EFF_ID_B, "eff.ab.t7");
    add_edge(EFF_ID_B, EFF_ID_C, "eff.bc.t7");
    add_edge(EFF_ID_C, EFF_ID_A, "eff.ca.t7");
    let (ppm, pairs_max, node_count) = gos_runtime::graph_global_efficiency();
    // All 6 directed pairs are reachable.
    // d=1 for consecutive pairs (A→B, B→C, C→A), d=2 for skipped pairs (A→C, B→A, C→B).
    // sum = 3*(1_000_000/1) + 3*(1_000_000/2) = 3_000_000 + 1_500_000 = 4_500_000.
    // ppm = 4_500_000 / 6 = 750_000.
    assert_eq!(node_count, 3, "3 nodes");
    assert_eq!(pairs_max, 6, "pairs_max=6");
    assert_eq!(ppm, 750_000, "directed 3-cycle: E(G)=0.75");
}

// ── 8. Complete K3 bidirectional: E(G)=1.0, ppm=1_000_000 ───────────────────

#[test]
fn complete_k3_full_efficiency() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(EFF_VEC_A, EFF_KEY_A, EFF_ID_A, 0xF001);
    add_node(EFF_VEC_B, EFF_KEY_B, EFF_ID_B, 0xF002);
    add_node(EFF_VEC_C, EFF_KEY_C, EFF_ID_C, 0xF003);
    add_edge(EFF_ID_A, EFF_ID_B, "eff.ab.t8");
    add_edge(EFF_ID_B, EFF_ID_A, "eff.ba.t8");
    add_edge(EFF_ID_B, EFF_ID_C, "eff.bc.t8");
    add_edge(EFF_ID_C, EFF_ID_B, "eff.cb.t8");
    add_edge(EFF_ID_A, EFF_ID_C, "eff.ac.t8");
    add_edge(EFF_ID_C, EFF_ID_A, "eff.ca.t8");
    let (ppm, pairs_max, node_count) = gos_runtime::graph_global_efficiency();
    // All 6 pairs d=1. sum = 6_000_000. pairs_max=6. ppm=1_000_000.
    assert_eq!(node_count, 3, "3 nodes");
    assert_eq!(pairs_max, 6, "pairs_max=6");
    assert_eq!(ppm, 1_000_000, "complete K3: E(G)=1.0");
}

// ── 9. Disconnected {A→B}∥{C→D}: ppm = 2_000_000/12 = 166_666 ──────────────

#[test]
fn disconnected_two_components_low_efficiency() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(EFF_VEC_A, EFF_KEY_A, EFF_ID_A, 0xF001);
    add_node(EFF_VEC_B, EFF_KEY_B, EFF_ID_B, 0xF002);
    add_node(EFF_VEC_C, EFF_KEY_C, EFF_ID_C, 0xF003);
    add_node(EFF_VEC_D, EFF_KEY_D, EFF_ID_D, 0xF004);
    add_edge(EFF_ID_A, EFF_ID_B, "eff.ab.t9");
    add_edge(EFF_ID_C, EFF_ID_D, "eff.cd.t9");
    let (ppm, pairs_max, node_count) = gos_runtime::graph_global_efficiency();
    // Only A→B and C→D reachable (d=1 each). Cross-component pairs unreachable.
    // sum = 2 * 1_000_000 = 2_000_000. pairs_max = 4*3 = 12. ppm = 166_666.
    assert_eq!(node_count, 4, "4 nodes");
    assert_eq!(pairs_max, 12, "pairs_max=12");
    assert_eq!(ppm, 166_666, "disconnected pair of edges: E(G)≈0.167");
}

// ── 10. Star-out A→{B,C,D}: ppm = 3_000_000/12 = 250_000 ───────────────────

#[test]
fn star_out_efficiency() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(EFF_VEC_A, EFF_KEY_A, EFF_ID_A, 0xF001);
    add_node(EFF_VEC_B, EFF_KEY_B, EFF_ID_B, 0xF002);
    add_node(EFF_VEC_C, EFF_KEY_C, EFF_ID_C, 0xF003);
    add_node(EFF_VEC_D, EFF_KEY_D, EFF_ID_D, 0xF004);
    add_edge(EFF_ID_A, EFF_ID_B, "eff.ab.t10");
    add_edge(EFF_ID_A, EFF_ID_C, "eff.ac.t10");
    add_edge(EFF_ID_A, EFF_ID_D, "eff.ad.t10");
    let (ppm, pairs_max, node_count) = gos_runtime::graph_global_efficiency();
    // 3 reachable pairs: A→B, A→C, A→D (each d=1). All others unreachable.
    // sum = 3 * 1_000_000 = 3_000_000. pairs_max=12. ppm=250_000.
    assert_eq!(node_count, 4, "4 nodes");
    assert_eq!(pairs_max, 12, "pairs_max=12");
    assert_eq!(ppm, 250_000, "star-out hub: E(G)=0.25");
}
