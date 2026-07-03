// gos-graph-local-eff-harness — V2.76 graph local efficiency API tests
//
// Verifies `gos_runtime::graph_local_efficiency`.
//
// E_loc(G) = (1/n) × Σ_v E(G_v)
// where G_v = directed subgraph induced by undirected neighbours of v.
// E(G_v) = Σ_{i≠j ∈ N(v)} 1/d_{G_v}(i,j) / (|N(v)| × (|N(v)|−1))
// Returns (eloc_ppm: u32, nodes_computed: usize, node_count: usize).
//
//  1. Empty graph                → (0, 0, 0)
//  2. Single isolated node       → (0, 0, 1)
//  3. Two nodes one edge         → (0, 0, 2)  — each has k=1, never computed
//  4. Path A→B→C                → E_loc=0 — G_B={A,C} no edges; nodes_computed=1
//  5. Directed triangle A→B,B→C,A→C → E_loc=500_000 — each G_v has 1 directed pair
//  6. Star A→{B,C,D}            → E_loc=0 — G_A={B,C,D} no internal edges
//  7. Complete K4 (6 directed)  → E_loc=500_000 — each G_v is a 3-node directed chain
//  8. Bidirectional triangle     → E_loc=1_000_000 — each G_v has 2 reachable pairs
//  9. Directed triangle + isolated D → E_loc=375_000 — denominator is n=4
// 10. Four-cycle A→B→C→D→A     → E_loc=0 — each G_v neighbour pair has no directed path

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const ELF_PLUGIN: PluginId   = PluginId::from_ascii("KL_ELF0_00");
const ELF_EXEC:   ExecutorId = ExecutorId::from_ascii("elf.exec");

const ELF_KEY_A: &str = "elf.alpha";
const ELF_KEY_B: &str = "elf.beta";
const ELF_KEY_C: &str = "elf.gamma";
const ELF_KEY_D: &str = "elf.delta";

const ELF_ID_A: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_A);
const ELF_ID_B: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_B);
const ELF_ID_C: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_C);
const ELF_ID_D: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_D);

// L4=52 identifies this harness namespace in the VectorAddress space.
const ELF_VEC_A: VectorAddress = VectorAddress::new(52, 1, 1, 0);
const ELF_VEC_B: VectorAddress = VectorAddress::new(52, 1, 2, 0);
const ELF_VEC_C: VectorAddress = VectorAddress::new(52, 1, 3, 0);
const ELF_VEC_D: VectorAddress = VectorAddress::new(52, 1, 4, 0);

const ELF_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    ELF_PLUGIN,
    name:         "kl-graph-local-eff-harness",
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
        executor_id:       ELF_EXEC,
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
    gos_runtime::discover_plugin(ELF_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(ELF_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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
fn empty_graph_zero_local_eff() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_local_efficiency();
    assert_eq!(node_count, 0,       "empty: no nodes");
    assert_eq!(nodes_computed, 0,   "empty: no computed nodes");
    assert_eq!(ppm, 0,              "empty: E_loc=0");
}

// ── 2. Single isolated node ───────────────────────────────────────────────────

#[test]
fn single_node_zero_local_eff() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xE001);
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_local_efficiency();
    assert_eq!(node_count, 1,       "1 node");
    assert_eq!(nodes_computed, 0,   "k=0: not computed");
    assert_eq!(ppm, 0,              "single node: E_loc=0");
}

// ── 3. Two nodes, one directed edge ──────────────────────────────────────────

#[test]
fn two_nodes_one_edge_zero_local_eff() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xE001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xE002);
    add_edge(ELF_ID_A, ELF_ID_B, "elf.ab.t3");
    // A has k=1, B has k=1 (undirected). Neither has ≥2 neighbours.
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_local_efficiency();
    assert_eq!(node_count, 2,       "2 nodes");
    assert_eq!(nodes_computed, 0,   "k=1 for both: not computed");
    assert_eq!(ppm, 0,              "no k>=2 nodes: E_loc=0");
}

// ── 4. Path A→B→C: G_B={A,C} has no directed edges ──────────────────────────

#[test]
fn path_three_nodes_zero_local_eff() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xE001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xE002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xE003);
    add_edge(ELF_ID_A, ELF_ID_B, "elf.ab.t4");
    add_edge(ELF_ID_B, ELF_ID_C, "elf.bc.t4");
    // Undirected: A={B}(k=1), B={A,C}(k=2), C={B}(k=1).
    // G_B = {A,C}: directed edges A→C? no. C→A? no. E(G_B)=0.
    // E_loc = 0/3 = 0. nodes_computed=1 (only B).
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_local_efficiency();
    assert_eq!(node_count, 3,       "3 nodes");
    assert_eq!(nodes_computed, 1,   "only B has k=2");
    assert_eq!(ppm, 0,              "path: no subgraph edges, E_loc=0");
}

// ── 5. Directed triangle A→B, B→C, A→C: E_loc=500_000 ───────────────────────

#[test]
fn directed_triangle_local_eff() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xE001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xE002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xE003);
    add_edge(ELF_ID_A, ELF_ID_B, "elf.ab.t5");
    add_edge(ELF_ID_B, ELF_ID_C, "elf.bc.t5");
    add_edge(ELF_ID_A, ELF_ID_C, "elf.ac.t5");
    // Undirected: A={B,C}(k=2), B={A,C}(k=2), C={A,B}(k=2).
    // G_A={B,C}: B→C yes, C→B no. sum_recip=1_000_000. pairs_max=2. E(G_A)=500_000.
    // G_B={A,C}: A→C yes, C→A no. sum_recip=1_000_000. E(G_B)=500_000.
    // G_C={A,B}: A→B yes, B→A no. sum_recip=1_000_000. E(G_C)=500_000.
    // E_loc = (500_000+500_000+500_000)/3 = 500_000. nodes_computed=3.
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_local_efficiency();
    assert_eq!(node_count, 3,       "3 nodes");
    assert_eq!(nodes_computed, 3,   "all 3 have k=2");
    assert_eq!(ppm, 500_000,        "directed triangle: E_loc=0.5");
}

// ── 6. Star A→{B,C,D}: G_A has no internal edges ────────────────────────────

#[test]
fn star_zero_local_eff() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xE001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xE002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xE003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xE004);
    add_edge(ELF_ID_A, ELF_ID_B, "elf.ab.t6");
    add_edge(ELF_ID_A, ELF_ID_C, "elf.ac.t6");
    add_edge(ELF_ID_A, ELF_ID_D, "elf.ad.t6");
    // Undirected: A={B,C,D}(k=3), B={A}(k=1), C={A}(k=1), D={A}(k=1).
    // G_A = {B,C,D}: no directed edges among leaves. E(G_A)=0.
    // B,C,D have k=1. E_loc = 0/4 = 0. nodes_computed=1.
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_local_efficiency();
    assert_eq!(node_count, 4,       "4 nodes");
    assert_eq!(nodes_computed, 1,   "only A has k>=2");
    assert_eq!(ppm, 0,              "star: no subgraph edges, E_loc=0");
}

// ── 7. Complete directed K4 (6 one-way edges): E_loc=500_000 ─────────────────

#[test]
fn complete_k4_local_eff() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xE001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xE002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xE003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xE004);
    add_edge(ELF_ID_A, ELF_ID_B, "elf.ab.t7");
    add_edge(ELF_ID_A, ELF_ID_C, "elf.ac.t7");
    add_edge(ELF_ID_A, ELF_ID_D, "elf.ad.t7");
    add_edge(ELF_ID_B, ELF_ID_C, "elf.bc.t7");
    add_edge(ELF_ID_B, ELF_ID_D, "elf.bd.t7");
    add_edge(ELF_ID_C, ELF_ID_D, "elf.cd.t7");
    // Each node has k=3 (all others as undirected neighbours).
    // G_A={B,C,D}: B→C, B→D, C→D. BFS from B: d(C)=1,d(D)=1. BFS from C: d(D)=1.
    //   sum_recip = 3×1_000_000 = 3_000_000. pairs_max=6. E(G_A)=500_000.
    // By symmetry G_B, G_C, G_D each = 500_000.
    // E_loc = (4×500_000)/4 = 500_000. nodes_computed=4.
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_local_efficiency();
    assert_eq!(node_count, 4,       "4 nodes");
    assert_eq!(nodes_computed, 4,   "all 4 have k=3");
    assert_eq!(ppm, 500_000,        "complete K4: E_loc=0.5");
}

// ── 8. Bidirectional triangle (6 edges): E_loc=1_000_000 ─────────────────────

#[test]
fn bidirectional_triangle_local_eff() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xE001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xE002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xE003);
    add_edge(ELF_ID_A, ELF_ID_B, "elf.ab.t8");
    add_edge(ELF_ID_B, ELF_ID_A, "elf.ba.t8");
    add_edge(ELF_ID_B, ELF_ID_C, "elf.bc.t8");
    add_edge(ELF_ID_C, ELF_ID_B, "elf.cb.t8");
    add_edge(ELF_ID_A, ELF_ID_C, "elf.ac.t8");
    add_edge(ELF_ID_C, ELF_ID_A, "elf.ca.t8");
    // Undirected: A={B,C}(k=2), B={A,C}(k=2), C={A,B}(k=2).
    // G_A={B,C}: B→C yes, C→B yes. BFS from B: d(C)=1. BFS from C: d(B)=1.
    //   sum_recip = 2_000_000. pairs_max=2. E(G_A)=1_000_000.
    // By symmetry E(G_B)=E(G_C)=1_000_000.
    // E_loc = 3_000_000/3 = 1_000_000. nodes_computed=3.
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_local_efficiency();
    assert_eq!(node_count, 3,       "3 nodes");
    assert_eq!(nodes_computed, 3,   "all 3 have k=2");
    assert_eq!(ppm, 1_000_000,      "bidirectional triangle: E_loc=1.0");
}

// ── 9. Directed triangle + isolated D: E_loc=375_000 ─────────────────────────

#[test]
fn triangle_plus_isolated_local_eff() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xE001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xE002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xE003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xE004);
    add_edge(ELF_ID_A, ELF_ID_B, "elf.ab.t9");
    add_edge(ELF_ID_B, ELF_ID_C, "elf.bc.t9");
    add_edge(ELF_ID_A, ELF_ID_C, "elf.ac.t9");
    // Triangle A,B,C: each contributes E(G_v)=500_000 (same as test 5).
    // D: isolated, k=0, not computed. Contributes 0.
    // E_loc = (500_000×3 + 0)/4 = 1_500_000/4 = 375_000. nodes_computed=3.
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_local_efficiency();
    assert_eq!(node_count, 4,       "4 nodes (triangle + isolated)");
    assert_eq!(nodes_computed, 3,   "A,B,C have k>=2; D isolated");
    assert_eq!(ppm, 375_000,        "triangle+isolated: E_loc=1_500_000/4=375_000");
}

// ── 10. Four-cycle A→B→C→D→A: each G_v pair has no directed path ─────────────

#[test]
fn four_cycle_zero_local_eff() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xE001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xE002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xE003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xE004);
    add_edge(ELF_ID_A, ELF_ID_B, "elf.ab.t10");
    add_edge(ELF_ID_B, ELF_ID_C, "elf.bc.t10");
    add_edge(ELF_ID_C, ELF_ID_D, "elf.cd.t10");
    add_edge(ELF_ID_D, ELF_ID_A, "elf.da.t10");
    // Undirected: A={B,D}(k=2), B={A,C}(k=2), C={B,D}(k=2), D={C,A}(k=2).
    // G_A={B,D}: B→D? no. D→B? no. E(G_A)=0.
    // G_B={A,C}: A→C? no. C→A? no. E(G_B)=0.
    // G_C={B,D}: B→D? no. D→B? no. E(G_C)=0.
    // G_D={C,A}: C→A? no. A→C? no. E(G_D)=0.
    // E_loc = 0/4 = 0. nodes_computed=4.
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_local_efficiency();
    assert_eq!(node_count, 4,       "4 nodes");
    assert_eq!(nodes_computed, 4,   "all have k=2");
    assert_eq!(ppm, 0,              "4-cycle: no subgraph paths, E_loc=0");
}
