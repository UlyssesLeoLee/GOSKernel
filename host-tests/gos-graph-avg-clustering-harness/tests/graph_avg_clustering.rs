// gos-graph-avg-clustering-harness — V2.75 average clustering coefficient API tests
//
// Verifies `gos_runtime::graph_avg_clustering`.
//
// avg_CC = (1/n) × Σ CC(v)   where CC(v) = triangles(v) / C(k_v, 2)
// Nodes with undirected degree k_v < 2 contribute CC(v) = 0.
// Returns (avg_ppm: u32, nodes_computed: usize, node_count: usize).
// Differs from graph_clustering (V2.61) which is the global transitivity ratio.
//
//  1. Empty graph               → ppm=0, nodes_computed=0, node_count=0.
//  2. Single isolated node      → ppm=0, nodes_computed=0, node_count=1.
//  3. Two nodes, no edges       → ppm=0, nodes_computed=0, node_count=2.
//  4. Path A→B→C               → B has k=2 but no triangle; ppm=0, nodes_computed=1.
//  5. Triangle A→B,B→C,A→C     → CC(each)=1.0; avg=1.0; ppm=1_000_000, nodes_computed=3.
//  6. Star A→{B,C,D}           → CC(A)=0 (no edges among {B,C,D}); ppm=0, nodes_computed=1.
//  7. Triangle + tail (A→B,B→C,A→C + D→A) → avg=2_333_333/4=583_333, nodes_computed=3.
//  8. Triangle + isolated D     → avg=3_000_000/4=750_000, nodes_computed=3.
//  9. Two triangles sharing edge (A→B,B→C,A→C + B→D,C→D) → avg=3_333_332/4=833_333, nodes_computed=4.
// 10. Complete directed K4 (A→B,A→C,A→D,B→C,B→D,C→D) → CC(each)=1.0; ppm=1_000_000, nodes_computed=4.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const ACC_PLUGIN: PluginId   = PluginId::from_ascii("KL_ACC0_00");
const ACC_EXEC:   ExecutorId = ExecutorId::from_ascii("acc.exec");

const ACC_KEY_A: &str = "acc.alpha";
const ACC_KEY_B: &str = "acc.beta";
const ACC_KEY_C: &str = "acc.gamma";
const ACC_KEY_D: &str = "acc.delta";

const ACC_ID_A: NodeId = derive_node_id(ACC_PLUGIN, ACC_KEY_A);
const ACC_ID_B: NodeId = derive_node_id(ACC_PLUGIN, ACC_KEY_B);
const ACC_ID_C: NodeId = derive_node_id(ACC_PLUGIN, ACC_KEY_C);
const ACC_ID_D: NodeId = derive_node_id(ACC_PLUGIN, ACC_KEY_D);

// L4=51 identifies this harness namespace in the VectorAddress space.
const ACC_VEC_A: VectorAddress = VectorAddress::new(51, 1, 1, 0);
const ACC_VEC_B: VectorAddress = VectorAddress::new(51, 1, 2, 0);
const ACC_VEC_C: VectorAddress = VectorAddress::new(51, 1, 3, 0);
const ACC_VEC_D: VectorAddress = VectorAddress::new(51, 1, 4, 0);

const ACC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    ACC_PLUGIN,
    name:         "kl-graph-avg-clustering-harness",
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
        executor_id:       ACC_EXEC,
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
    gos_runtime::discover_plugin(ACC_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(ACC_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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
fn empty_graph_zero_avg_cc() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_avg_clustering();
    assert_eq!(node_count, 0,       "empty: no nodes");
    assert_eq!(nodes_computed, 0,   "empty: no k>=2 nodes");
    assert_eq!(ppm, 0,              "empty: avg_CC=0");
}

// ── 2. Single isolated node ───────────────────────────────────────────────────

#[test]
fn single_node_zero_avg_cc() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ACC_VEC_A, ACC_KEY_A, ACC_ID_A, 0xA001);
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_avg_clustering();
    assert_eq!(node_count, 1,       "1 node");
    assert_eq!(nodes_computed, 0,   "k=0 < 2: not computed");
    assert_eq!(ppm, 0,              "single node: avg_CC=0");
}

// ── 3. Two nodes, no edges ────────────────────────────────────────────────────

#[test]
fn two_nodes_no_edges_zero_avg_cc() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ACC_VEC_A, ACC_KEY_A, ACC_ID_A, 0xA001);
    add_node(ACC_VEC_B, ACC_KEY_B, ACC_ID_B, 0xA002);
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_avg_clustering();
    assert_eq!(node_count, 2,       "2 nodes");
    assert_eq!(nodes_computed, 0,   "k=0 for both: not computed");
    assert_eq!(ppm, 0,              "no edges: avg_CC=0");
}

// ── 4. Path A→B→C: CC(B)=0 (no triangle), avg=0/3=0 ─────────────────────────

#[test]
fn path_three_nodes_zero_avg_cc() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ACC_VEC_A, ACC_KEY_A, ACC_ID_A, 0xA001);
    add_node(ACC_VEC_B, ACC_KEY_B, ACC_ID_B, 0xA002);
    add_node(ACC_VEC_C, ACC_KEY_C, ACC_ID_C, 0xA003);
    add_edge(ACC_ID_A, ACC_ID_B, "acc.ab.t4");
    add_edge(ACC_ID_B, ACC_ID_C, "acc.bc.t4");
    // Undirected: A={B}(k=1), B={A,C}(k=2), C={B}(k=1).
    // CC(B): neighbors {A,C}; edge A-C? no → triangles=0 → CC(B)=0.
    // avg = (0+0+0)/3 = 0. nodes_computed=1 (B has k=2).
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_avg_clustering();
    assert_eq!(node_count, 3,       "3 nodes");
    assert_eq!(nodes_computed, 1,   "only B has k>=2");
    assert_eq!(ppm, 0,              "path: no triangles, avg_CC=0");
}

// ── 5. Triangle A→B, B→C, A→C: CC(each)=1.0, avg=1.0, ppm=1_000_000 ─────────

#[test]
fn triangle_full_avg_cc() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ACC_VEC_A, ACC_KEY_A, ACC_ID_A, 0xA001);
    add_node(ACC_VEC_B, ACC_KEY_B, ACC_ID_B, 0xA002);
    add_node(ACC_VEC_C, ACC_KEY_C, ACC_ID_C, 0xA003);
    add_edge(ACC_ID_A, ACC_ID_B, "acc.ab.t5");
    add_edge(ACC_ID_B, ACC_ID_C, "acc.bc.t5");
    add_edge(ACC_ID_A, ACC_ID_C, "acc.ac.t5");
    // Undirected: A={B,C}(k=2), B={A,C}(k=2), C={A,B}(k=2).
    // CC(A): edge B-C? B→C yes → 1 triangle / 1 triplet = 1.0 → ppm=1_000_000
    // CC(B): edge A-C? A→C yes → 1_000_000
    // CC(C): edge A-B? A→B yes → 1_000_000
    // avg = 3_000_000 / 3 = 1_000_000
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_avg_clustering();
    assert_eq!(node_count, 3,         "3 nodes");
    assert_eq!(nodes_computed, 3,     "all 3 have k=2");
    assert_eq!(ppm, 1_000_000,        "triangle: avg_CC=1.0");
}

// ── 6. Star A→{B,C,D}: CC(A)=0 (leaves not connected), ppm=0 ─────────────────

#[test]
fn star_zero_avg_cc() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ACC_VEC_A, ACC_KEY_A, ACC_ID_A, 0xA001);
    add_node(ACC_VEC_B, ACC_KEY_B, ACC_ID_B, 0xA002);
    add_node(ACC_VEC_C, ACC_KEY_C, ACC_ID_C, 0xA003);
    add_node(ACC_VEC_D, ACC_KEY_D, ACC_ID_D, 0xA004);
    add_edge(ACC_ID_A, ACC_ID_B, "acc.ab.t6");
    add_edge(ACC_ID_A, ACC_ID_C, "acc.ac.t6");
    add_edge(ACC_ID_A, ACC_ID_D, "acc.ad.t6");
    // Undirected: A={B,C,D}(k=3), B={A}(k=1), C={A}(k=1), D={A}(k=1).
    // CC(A): neighbors {B,C,D}; B-C? no. B-D? no. C-D? no → triangles=0 → CC(A)=0.
    // avg = (0+0+0+0)/4 = 0. nodes_computed=1.
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_avg_clustering();
    assert_eq!(node_count, 4,       "4 nodes");
    assert_eq!(nodes_computed, 1,   "only A has k>=2");
    assert_eq!(ppm, 0,              "star hub: no triangles, avg_CC=0");
}

// ── 7. Triangle + tail: A→B,B→C,A→C + D→A; avg=2_333_333/4=583_333 ──────────

#[test]
fn triangle_with_tail_avg_cc() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ACC_VEC_A, ACC_KEY_A, ACC_ID_A, 0xA001);
    add_node(ACC_VEC_B, ACC_KEY_B, ACC_ID_B, 0xA002);
    add_node(ACC_VEC_C, ACC_KEY_C, ACC_ID_C, 0xA003);
    add_node(ACC_VEC_D, ACC_KEY_D, ACC_ID_D, 0xA004);
    add_edge(ACC_ID_A, ACC_ID_B, "acc.ab.t7");
    add_edge(ACC_ID_B, ACC_ID_C, "acc.bc.t7");
    add_edge(ACC_ID_A, ACC_ID_C, "acc.ac.t7");
    add_edge(ACC_ID_D, ACC_ID_A, "acc.da.t7");
    // Undirected: A={B,C,D}(k=3), B={A,C}(k=2), C={A,B}(k=2), D={A}(k=1).
    // CC(A): neighbors {B,C,D}; B-C? yes. B-D? no. C-D? no → triangles=1; triplets=3 → 1_000_000/3=333_333
    // CC(B): neighbors {A,C}; A-C? yes → triangles=1; triplets=1 → 1_000_000
    // CC(C): neighbors {A,B}; A-B? yes → triangles=1; triplets=1 → 1_000_000
    // D: k=1, not computed.
    // sum = 333_333 + 1_000_000 + 1_000_000 = 2_333_333
    // avg = 2_333_333 / 4 = 583_333
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_avg_clustering();
    assert_eq!(node_count, 4,       "4 nodes");
    assert_eq!(nodes_computed, 3,   "A,B,C have k>=2; D has k=1");
    assert_eq!(ppm, 583_333,        "triangle+tail: avg_CC=2_333_333/4=583_333");
}

// ── 8. Triangle + isolated D: avg=3_000_000/4=750_000 ────────────────────────

#[test]
fn triangle_plus_isolated_avg_cc() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ACC_VEC_A, ACC_KEY_A, ACC_ID_A, 0xA001);
    add_node(ACC_VEC_B, ACC_KEY_B, ACC_ID_B, 0xA002);
    add_node(ACC_VEC_C, ACC_KEY_C, ACC_ID_C, 0xA003);
    add_node(ACC_VEC_D, ACC_KEY_D, ACC_ID_D, 0xA004);
    add_edge(ACC_ID_A, ACC_ID_B, "acc.ab.t8");
    add_edge(ACC_ID_B, ACC_ID_C, "acc.bc.t8");
    add_edge(ACC_ID_A, ACC_ID_C, "acc.ac.t8");
    // Triangle A,B,C each have CC=1.0; D isolated (k=0, CC=0).
    // sum = 3_000_000. avg = 3_000_000/4 = 750_000.
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_avg_clustering();
    assert_eq!(node_count, 4,       "4 nodes (triangle + isolated)");
    assert_eq!(nodes_computed, 3,   "A,B,C have k>=2; D isolated");
    assert_eq!(ppm, 750_000,        "triangle+isolated: avg_CC=3_000_000/4=750_000");
}

// ── 9. Two triangles sharing edge (A→B,B→C,A→C + B→D,C→D): avg=833_333 ──────

#[test]
fn two_triangles_sharing_edge_avg_cc() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ACC_VEC_A, ACC_KEY_A, ACC_ID_A, 0xA001);
    add_node(ACC_VEC_B, ACC_KEY_B, ACC_ID_B, 0xA002);
    add_node(ACC_VEC_C, ACC_KEY_C, ACC_ID_C, 0xA003);
    add_node(ACC_VEC_D, ACC_KEY_D, ACC_ID_D, 0xA004);
    add_edge(ACC_ID_A, ACC_ID_B, "acc.ab.t9");
    add_edge(ACC_ID_B, ACC_ID_C, "acc.bc.t9");
    add_edge(ACC_ID_A, ACC_ID_C, "acc.ac.t9");
    add_edge(ACC_ID_B, ACC_ID_D, "acc.bd.t9");
    add_edge(ACC_ID_C, ACC_ID_D, "acc.cd.t9");
    // Undirected: A={B,C}(k=2), B={A,C,D}(k=3), C={A,B,D}(k=3), D={B,C}(k=2).
    // CC(A): edge B-C? yes → 1/1 = 1_000_000
    // CC(B): edges among {A,C,D}: A-C? yes; A-D? no; C-D? yes → 2 triangles/3 triplets = 666_666
    // CC(C): edges among {A,B,D}: A-B? yes; A-D? no; B-D? yes → 2/3 = 666_666
    // CC(D): edge B-C? yes → 1/1 = 1_000_000
    // sum = 1_000_000 + 666_666 + 666_666 + 1_000_000 = 3_333_332
    // avg = 3_333_332 / 4 = 833_333
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_avg_clustering();
    assert_eq!(node_count, 4,       "4 nodes");
    assert_eq!(nodes_computed, 4,   "all 4 have k>=2");
    assert_eq!(ppm, 833_333,        "two triangles sharing edge: avg=3_333_332/4=833_333");
}

// ── 10. Complete directed K4 (6 one-way edges): CC(each)=1.0, ppm=1_000_000 ──

#[test]
fn complete_k4_full_avg_cc() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ACC_VEC_A, ACC_KEY_A, ACC_ID_A, 0xA001);
    add_node(ACC_VEC_B, ACC_KEY_B, ACC_ID_B, 0xA002);
    add_node(ACC_VEC_C, ACC_KEY_C, ACC_ID_C, 0xA003);
    add_node(ACC_VEC_D, ACC_KEY_D, ACC_ID_D, 0xA004);
    add_edge(ACC_ID_A, ACC_ID_B, "acc.ab.t10");
    add_edge(ACC_ID_A, ACC_ID_C, "acc.ac.t10");
    add_edge(ACC_ID_A, ACC_ID_D, "acc.ad.t10");
    add_edge(ACC_ID_B, ACC_ID_C, "acc.bc.t10");
    add_edge(ACC_ID_B, ACC_ID_D, "acc.bd.t10");
    add_edge(ACC_ID_C, ACC_ID_D, "acc.cd.t10");
    // Undirected neighbors: each node has k=3 (all others are neighbours).
    // CC(A): {B,C,D}; B-C? yes. B-D? yes. C-D? yes → 3/3=1_000_000
    // CC(B): {A,C,D}; A-C? yes. A-D? yes. C-D? yes → 3/3=1_000_000
    // CC(C): {A,B,D}; A-B? yes. A-D? yes. B-D? yes → 3/3=1_000_000
    // CC(D): {A,B,C}; A-B? yes. A-C? yes. B-C? yes → 3/3=1_000_000
    // sum = 4_000_000. avg = 4_000_000/4 = 1_000_000.
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_avg_clustering();
    assert_eq!(node_count, 4,         "4 nodes");
    assert_eq!(nodes_computed, 4,     "all 4 have k=3");
    assert_eq!(ppm, 1_000_000,        "complete K4: avg_CC=1.0");
}
