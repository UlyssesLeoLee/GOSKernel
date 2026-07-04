// gos-graph-link-predict-harness — V2.84 graph link prediction API tests
//
// Verifies `gos_runtime::graph_link_predict` — Common Neighbors (CN), Jaccard
// Coefficient, Adamic-Adar (AA) index, and Resource Allocation (RA) index for
// a directed graph treated as undirected for neighbourhood computation.
//
// Neighbourhood N(u) = {w : edge u→w or w→u, w ≠ u, w ≠ v}.
// u and v are mutually excluded from each other's neighbourhood sets.
//
// Score formulas (AA and RA returned × 1_000_000 as u32 ppm):
//   CN            = |N(u) ∩ N(v)|   (raw count)
//   Jaccard       = CN / |N(u) ∪ N(v)| × 1_000_000
//   Adamic-Adar   = Σ_{w∈CN} 1e12 / LN_TABLE[deg(w)]  (ppm; skips deg≤1)
//   Res. Alloc.   = Σ_{w∈CN} 1_000_000 / deg(w)        (ppm)
//
// OS analogy: LLDP neighbour-discovery prediction — which kernel subsystems
// are structurally primed to form a new dependency edge?
//
//  1. Empty graph: all zeros, node_count=0.
//  2. Single isolated node: CN=0, all zeros, node_count=1.
//  3. Two nodes, no edges: CN=0, all zeros.
//  4. Two nodes A→B, predict (A,B): exclusion removes B from N(A) and A from N(B) → CN=0.
//  5. Path A→B→C, predict (A,C): B is common neighbor, CN=1, Jaccard=1_000_000.
//  6. Degenerate (u == v): all zeros regardless of graph structure.
//  7. Star A→{B,C,D}, predict (B,C): CN=1 (through hub A), Jaccard=1_000_000, deg(A)=3.
//  8. Diamond A→{B,C}→D, predict (A,D): CN=2 (B and C), Jaccard=1_000_000, AA≈2×1.443M.
//  9. Disconnected pair {A→B} ∥ {C→D}, predict (A,D): CN=0, all zeros.
// 10. Predict unknown VectorAddress: CN=0, node_count unchanged.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const LP_PLUGIN: PluginId   = PluginId::from_ascii("KL_LP01_01");
const LP_EXEC:   ExecutorId = ExecutorId::from_ascii("lp.exec");

const LP_KEY_A: &str = "lp.alpha";
const LP_KEY_B: &str = "lp.beta";
const LP_KEY_C: &str = "lp.gamma";
const LP_KEY_D: &str = "lp.delta";

const LP_ID_A: NodeId = derive_node_id(LP_PLUGIN, LP_KEY_A);
const LP_ID_B: NodeId = derive_node_id(LP_PLUGIN, LP_KEY_B);
const LP_ID_C: NodeId = derive_node_id(LP_PLUGIN, LP_KEY_C);
const LP_ID_D: NodeId = derive_node_id(LP_PLUGIN, LP_KEY_D);

// L4=60 identifies this harness namespace.
const LP_VEC_A: VectorAddress = VectorAddress::new(60, 1, 1, 0);
const LP_VEC_B: VectorAddress = VectorAddress::new(60, 1, 2, 0);
const LP_VEC_C: VectorAddress = VectorAddress::new(60, 1, 3, 0);
const LP_VEC_D: VectorAddress = VectorAddress::new(60, 1, 4, 0);

const LP_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    LP_PLUGIN,
    name:         "kl-graph-link-predict-harness",
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

fn node_spec(key: &'static str, id: NodeId) -> NodeSpec {
    NodeSpec {
        node_id:           id,
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       LP_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(LP_PLUGIN, vec, node_spec(key, id)).unwrap();
}

fn add_edge(from: NodeId, to: NodeId, key: &'static str) {
    gos_runtime::register_edge(EdgeSpec {
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
    }).unwrap();
}

fn setup() -> std::sync::MutexGuard<'static, ()> {
    let g = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(LP_MANIFEST).unwrap();
    g
}

// ── Tests ─────────────────────────────────────────────────────────────────────

// 1. Empty graph → all zeros, node_count=0.
#[test]
fn test_01_empty_graph() {
    let _g = setup();
    let (cn, j, aa, ra, nc) = gos_runtime::graph_link_predict(LP_VEC_A, LP_VEC_B);
    assert_eq!(nc, 0, "node_count should be 0 for empty graph");
    assert_eq!(cn, 0);
    assert_eq!(j,  0);
    assert_eq!(aa, 0);
    assert_eq!(ra, 0);
}

// 2. Single isolated node → CN=0, node_count=1.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(LP_VEC_A, LP_KEY_A, LP_ID_A);
    let (cn, j, aa, ra, nc) = gos_runtime::graph_link_predict(LP_VEC_A, LP_VEC_B);
    assert_eq!(nc, 1, "node_count should be 1");
    assert_eq!(cn, 0, "CN=0: B is not registered");
    assert_eq!(j,  0);
    assert_eq!(aa, 0);
    assert_eq!(ra, 0);
}

// 3. Two registered nodes, no edges → CN=0.
#[test]
fn test_03_two_nodes_no_edges() {
    let _g = setup();
    add_node(LP_VEC_A, LP_KEY_A, LP_ID_A);
    add_node(LP_VEC_B, LP_KEY_B, LP_ID_B);
    let (cn, j, aa, ra, nc) = gos_runtime::graph_link_predict(LP_VEC_A, LP_VEC_B);
    assert_eq!(nc, 2);
    assert_eq!(cn, 0, "no common neighbours when no edges");
    assert_eq!(j,  0);
    assert_eq!(aa, 0);
    assert_eq!(ra, 0);
}

// 4. Two nodes A→B, predict (A, B):
//    exclusion removes B from N(A) and A from N(B) → CN=0.
#[test]
fn test_04_direct_edge_no_common_neighbor() {
    let _g = setup();
    add_node(LP_VEC_A, LP_KEY_A, LP_ID_A);
    add_node(LP_VEC_B, LP_KEY_B, LP_ID_B);
    add_edge(LP_ID_A, LP_ID_B, "ab");
    let (cn, j, aa, ra, nc) = gos_runtime::graph_link_predict(LP_VEC_A, LP_VEC_B);
    assert_eq!(nc, 2);
    assert_eq!(cn, 0, "u and v excluded from each other's N → CN=0 even with direct edge");
    assert_eq!(j,  0);
    assert_eq!(aa, 0);
    assert_eq!(ra, 0);
}

// 5. Path A→B→C, predict (A, C):
//    B is the only common neighbour; deg(B)=2 (edges A→B and B→C).
//    CN=1, Jaccard=1_000_000, AA≈1.443M (1/ln(2)×1e6), RA=500_000 (1/2×1e6).
#[test]
fn test_05_path_one_common_neighbor() {
    let _g = setup();
    add_node(LP_VEC_A, LP_KEY_A, LP_ID_A);
    add_node(LP_VEC_B, LP_KEY_B, LP_ID_B);
    add_node(LP_VEC_C, LP_KEY_C, LP_ID_C);
    add_edge(LP_ID_A, LP_ID_B, "ab");
    add_edge(LP_ID_B, LP_ID_C, "bc");

    let (cn, j, aa, ra, nc) = gos_runtime::graph_link_predict(LP_VEC_A, LP_VEC_C);
    assert_eq!(nc, 3);
    assert_eq!(cn, 1, "B is the single common neighbour");
    assert_eq!(j, 1_000_000, "N(A)={{B}}, N(C)={{B}} → union=1, CN=1 → Jaccard=1.0");
    // AA: deg(B)=2, LN_TABLE[2]=693_147, aa = 1e12/693_147 = 1_442_695
    assert!(aa > 1_440_000 && aa < 1_450_000,
        "AA should be ~1.443M (1/ln(2)×1e6), got {aa}");
    assert_eq!(ra, 500_000, "RA: 1/2 × 1e6 = 500_000, deg(B)=2");
}

// 6. Degenerate: predict (u, u) → all zeros.
#[test]
fn test_06_degenerate_self_prediction() {
    let _g = setup();
    add_node(LP_VEC_A, LP_KEY_A, LP_ID_A);
    add_node(LP_VEC_B, LP_KEY_B, LP_ID_B);
    add_edge(LP_ID_A, LP_ID_B, "ab");
    let (cn, j, aa, ra, _nc) = gos_runtime::graph_link_predict(LP_VEC_A, LP_VEC_A);
    assert_eq!(cn, 0, "self-prediction is degenerate → CN=0");
    assert_eq!(j,  0);
    assert_eq!(aa, 0);
    assert_eq!(ra, 0);
}

// 7. Star A→{B,C,D}, predict (B, C):
//    A is the common hub; deg(A)=3; CN=1, Jaccard=1_000_000.
//    AA≈0.910M (1/ln(3)×1e6), RA=333_333 (1/3×1e6 integer division).
#[test]
fn test_07_star_topology_predict_leaves() {
    let _g = setup();
    add_node(LP_VEC_A, LP_KEY_A, LP_ID_A);
    add_node(LP_VEC_B, LP_KEY_B, LP_ID_B);
    add_node(LP_VEC_C, LP_KEY_C, LP_ID_C);
    add_node(LP_VEC_D, LP_KEY_D, LP_ID_D);
    add_edge(LP_ID_A, LP_ID_B, "ab");
    add_edge(LP_ID_A, LP_ID_C, "ac");
    add_edge(LP_ID_A, LP_ID_D, "ad");

    let (cn, j, aa, ra, nc) = gos_runtime::graph_link_predict(LP_VEC_B, LP_VEC_C);
    assert_eq!(nc, 4);
    assert_eq!(cn, 1, "A is the only common neighbour of B and C");
    assert_eq!(j, 1_000_000, "Jaccard = 1/1 = 1.0 since N(B)={{A}}, N(C)={{A}}");
    // deg(A)=3, LN_TABLE[3]=1_098_612 → 1e12/1_098_612 ≈ 910_239
    assert!(aa > 905_000 && aa < 915_000,
        "AA should be ~0.910M (1/ln(3)×1e6), got {aa}");
    assert_eq!(ra, 333_333, "RA: 1/3 × 1e6 = 333_333 (integer division), deg(A)=3");
}

// 8. Diamond A→{B,C}→D, predict (A, D):
//    B and C are both common neighbours; deg(B)=deg(C)=2; CN=2.
//    Jaccard=1_000_000, AA≈2×1.443M=2.885M, RA=1_000_000 (2×500_000).
#[test]
fn test_08_diamond_two_common_neighbors() {
    let _g = setup();
    add_node(LP_VEC_A, LP_KEY_A, LP_ID_A);
    add_node(LP_VEC_B, LP_KEY_B, LP_ID_B);
    add_node(LP_VEC_C, LP_KEY_C, LP_ID_C);
    add_node(LP_VEC_D, LP_KEY_D, LP_ID_D);
    add_edge(LP_ID_A, LP_ID_B, "ab");
    add_edge(LP_ID_A, LP_ID_C, "ac");
    add_edge(LP_ID_B, LP_ID_D, "bd");
    add_edge(LP_ID_C, LP_ID_D, "cd");

    let (cn, j, aa, ra, nc) = gos_runtime::graph_link_predict(LP_VEC_A, LP_VEC_D);
    assert_eq!(nc, 4);
    assert_eq!(cn, 2, "B and C are both common neighbours of A and D");
    assert_eq!(j, 1_000_000, "Jaccard = 2/2 = 1.0");
    // deg(B)=deg(C)=2, AA = 2 × 1e12/693_147 = 2 × 1_442_695 = 2_885_390
    assert!(aa > 2_880_000 && aa < 2_892_000,
        "AA should be ~2.885M (2×1/ln(2)×1e6), got {aa}");
    assert_eq!(ra, 1_000_000, "RA: 2 × (1/2 × 1e6) = 1_000_000");
}

// 9. Disconnected pair {A→B} ∥ {C→D}, predict (A, D):
//    No common neighbours across components → CN=0.
#[test]
fn test_09_disconnected_components() {
    let _g = setup();
    add_node(LP_VEC_A, LP_KEY_A, LP_ID_A);
    add_node(LP_VEC_B, LP_KEY_B, LP_ID_B);
    add_node(LP_VEC_C, LP_KEY_C, LP_ID_C);
    add_node(LP_VEC_D, LP_KEY_D, LP_ID_D);
    add_edge(LP_ID_A, LP_ID_B, "ab");
    add_edge(LP_ID_C, LP_ID_D, "cd");

    let (cn, j, aa, ra, nc) = gos_runtime::graph_link_predict(LP_VEC_A, LP_VEC_D);
    assert_eq!(nc, 4);
    assert_eq!(cn, 0, "A and D are in different components, no common neighbours");
    assert_eq!(j,  0, "Jaccard=0 when union is empty");
    assert_eq!(aa, 0);
    assert_eq!(ra, 0);
}

// 10. Predict with unregistered VectorAddress → all zeros, node_count unaffected.
#[test]
fn test_10_unknown_vector_address() {
    let _g = setup();
    add_node(LP_VEC_A, LP_KEY_A, LP_ID_A);
    add_node(LP_VEC_B, LP_KEY_B, LP_ID_B);
    add_edge(LP_ID_A, LP_ID_B, "ab");

    let unknown = VectorAddress::new(60, 9, 9, 0);
    let (cn, j, aa, ra, nc) = gos_runtime::graph_link_predict(LP_VEC_A, unknown);
    assert_eq!(nc, 2, "node_count reflects actual live nodes, not query arguments");
    assert_eq!(cn, 0, "unknown vector → no neighbourhood → CN=0");
    assert_eq!(j,  0);
    assert_eq!(aa, 0);
    assert_eq!(ra, 0);
}
