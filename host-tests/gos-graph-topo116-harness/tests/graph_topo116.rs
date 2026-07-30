// gos-graph-topo116-harness — V3.127 NENNAACTC + NHENNAACTC + NBGGSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices116()`:
//   Returns (nennaactc, nhennaactc, nbggso, edge_count, node_count)
//   - nennaactc   = NENNAACTC(G)  = Σ_v S(v)^90                          (saturating u64)
//   - nhennaactc  = NHENNAACTC(G) = Σ_{uv∈E} (S_u+S_v)^89              (saturating u64)
//   - nbggso      = NBGGSO(G)     = Σ_{uv∈E} (S_u²+S_v²)^84            (saturating u64)
//
// NENNAACTC: FIRST of ennacontic (90-99) series. Extends NOCTAENNACTC=Σ S^89 (topo115).
//   s^90 = s64 × s16 × s8 × s2  (90=64+16+8+2; 9 mults).
// NHENNAACTC: ss^89 = ss64 × ss16 × ss8 × ss  (9 mults).
// NBGGSO: α=168, 33rd of NB series. s2s^84 = s2s64 × s2s16 × s2s4  (8 mults).
//
// ANALYTICAL CROSS-CHECK TABLE:
//  Graph     NENNAACTC(exact)       NHENNAACTC(exact)      NBGGSO(exact)   edges nodes
//  Empty                  0                        0                 0          0     0
//  1 node                 0                        0                 0          0     1
//  K₂ (S=1)               2        u64::MAX(sat.)    u64::MAX(sat.)             1     2
//  P₃ (S=2)  u64::MAX(sat.)        u64::MAX(sat.)    u64::MAX(sat.)             2     3
//  K₃ (S=4)  u64::MAX(sat.)        u64::MAX(sat.)    u64::MAX(sat.)             3     3
//  K_{1,4}   u64::MAX(sat.)        u64::MAX(sat.)    u64::MAX(sat.)             4     5
//  P₄ mixed  u64::MAX(sat.)        u64::MAX(sat.)    u64::MAX(sat.)             3     4
//  K₄ (S=9)  u64::MAX(sat.)        u64::MAX(sat.)    u64::MAX(sat.)             6     4
//  2 isolated             0                        0                 0          0     2
//  K_{2,3}   u64::MAX(sat.)        u64::MAX(sat.)    u64::MAX(sat.)             6     5
//
// K₂ derivation (S=1 uniform):
//   NENNAACTC  = 2 × 1^90 = 2.  (exact)
//   NHENNAACTC = 1 × (1+1)^89 = 2^89 > u64::MAX. (saturated)
//   NBGGSO     = 1 × (1²+1²)^84 = 2^84 > u64::MAX. (saturated)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const T116_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX116");
const T116_EXEC:   ExecutorId = ExecutorId::from_ascii("t116.exec");

const T116_KEY_A: &str = "t116.alpha";
const T116_KEY_B: &str = "t116.beta";
const T116_KEY_C: &str = "t116.gamma";
const T116_KEY_D: &str = "t116.delta";
const T116_KEY_E: &str = "t116.epsilon";

const T116_ID_A: NodeId = derive_node_id(T116_PLUGIN, T116_KEY_A);
const T116_ID_B: NodeId = derive_node_id(T116_PLUGIN, T116_KEY_B);
const T116_ID_C: NodeId = derive_node_id(T116_PLUGIN, T116_KEY_C);
const T116_ID_D: NodeId = derive_node_id(T116_PLUGIN, T116_KEY_D);
const T116_ID_E: NodeId = derive_node_id(T116_PLUGIN, T116_KEY_E);

// L4=203 namespace for this harness.
const T116_VEC_A: VectorAddress = VectorAddress::new(203, 1, 1, 0);
const T116_VEC_B: VectorAddress = VectorAddress::new(203, 1, 2, 0);
const T116_VEC_C: VectorAddress = VectorAddress::new(203, 1, 3, 0);
const T116_VEC_D: VectorAddress = VectorAddress::new(203, 2, 1, 0);
const T116_VEC_E: VectorAddress = VectorAddress::new(203, 2, 2, 0);

const T116_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T116_PLUGIN,
    name:         "kl-graph-topo116-harness",
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
        executor_id:       T116_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T116_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    let g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    gos_runtime::reset();
    gos_runtime::discover_plugin(T116_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nennaactc, nhennaactc, nbggso, ec, nc) = gos_runtime::graph_topo_indices116();
    assert_eq!(nc,         0, "empty: node_count=0");
    assert_eq!(ec,         0, "empty: edge_count=0");
    assert_eq!(nennaactc,  0, "empty: NENNAACTC=0");
    assert_eq!(nhennaactc, 0, "empty: NHENNAACTC=0");
    assert_eq!(nbggso,     0, "empty: NBGGSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T116_VEC_A, T116_KEY_A, T116_ID_A);
    let (nennaactc, nhennaactc, nbggso, ec, nc) = gos_runtime::graph_topo_indices116();
    assert_eq!(nc,         1, "single: node_count=1");
    assert_eq!(ec,         0, "single: edge_count=0");
    assert_eq!(nennaactc,  0, "single: NENNAACTC=0");
    assert_eq!(nhennaactc, 0, "single: NHENNAACTC=0");
    assert_eq!(nbggso,     0, "single: NBGGSO=0");
}

// ── Test 3: K₂ single edge A→B ───────────────────────────────────────────────
// deg(A)=deg(B)=1, S(A)=S(B)=1.
// NENNAACTC = 2 × 1^90 = 2.
// NHENNAACTC = (1+1)^89 = 2^89 > u64::MAX → SATURATES.
// NBGGSO = (1²+1²)^84 = 2^84 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T116_VEC_A, T116_KEY_A, T116_ID_A);
    add_node(T116_VEC_B, T116_KEY_B, T116_ID_B);
    add_edge(T116_ID_A, T116_ID_B, "t116.e.ab");
    let (nennaactc, nhennaactc, nbggso, ec, nc) = gos_runtime::graph_topo_indices116();
    assert_eq!(nc,         2,        "k2: node_count=2");
    assert_eq!(ec,         1,        "k2: edge_count=1");
    assert_eq!(nennaactc,  2,        "k2: NENNAACTC=2 (1^90+1^90=2)");
    assert_eq!(nhennaactc, u64::MAX, "k2: NHENNAACTC=SAT (2^89>u64::MAX)");
    assert_eq!(nbggso,     u64::MAX, "k2: NBGGSO=SAT (2^84>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T116_VEC_A, T116_KEY_A, T116_ID_A);
    add_node(T116_VEC_B, T116_KEY_B, T116_ID_B);
    add_node(T116_VEC_C, T116_KEY_C, T116_ID_C);
    add_edge(T116_ID_A, T116_ID_B, "t116.e.ab");
    add_edge(T116_ID_B, T116_ID_C, "t116.e.bc");
    let (nennaactc, nhennaactc, nbggso, ec, nc) = gos_runtime::graph_topo_indices116();
    assert_eq!(nc, 3, "p3: node_count=3");
    assert_eq!(ec, 2, "p3: edge_count=2");
    assert_eq!(nennaactc,  u64::MAX, "p3: NENNAACTC=SAT");
    assert_eq!(nhennaactc, u64::MAX, "p3: NHENNAACTC=SAT");
    assert_eq!(nbggso,     u64::MAX, "p3: NBGGSO=SAT");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T116_VEC_A, T116_KEY_A, T116_ID_A);
    add_node(T116_VEC_B, T116_KEY_B, T116_ID_B);
    add_node(T116_VEC_C, T116_KEY_C, T116_ID_C);
    add_edge(T116_ID_A, T116_ID_B, "t116.e.ab");
    add_edge(T116_ID_B, T116_ID_C, "t116.e.bc");
    add_edge(T116_ID_C, T116_ID_A, "t116.e.ca");
    let (nennaactc, nhennaactc, nbggso, ec, nc) = gos_runtime::graph_topo_indices116();
    assert_eq!(nc, 3, "k3: node_count=3");
    assert_eq!(ec, 3, "k3: edge_count=3");
    assert_eq!(nennaactc,  u64::MAX, "k3: NENNAACTC=SAT");
    assert_eq!(nhennaactc, u64::MAX, "k3: NHOCTA=SAT");
    assert_eq!(nbggso,     u64::MAX, "k3: NBGGSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T116_VEC_A, T116_KEY_A, T116_ID_A); // hub
    add_node(T116_VEC_B, T116_KEY_B, T116_ID_B);
    add_node(T116_VEC_C, T116_KEY_C, T116_ID_C);
    add_node(T116_VEC_D, T116_KEY_D, T116_ID_D);
    add_node(T116_VEC_E, T116_KEY_E, T116_ID_E);
    add_edge(T116_ID_A, T116_ID_B, "t116.e.ab");
    add_edge(T116_ID_A, T116_ID_C, "t116.e.ac");
    add_edge(T116_ID_A, T116_ID_D, "t116.e.ad");
    add_edge(T116_ID_A, T116_ID_E, "t116.e.ae");
    let (nennaactc, nhennaactc, nbggso, ec, nc) = gos_runtime::graph_topo_indices116();
    assert_eq!(nc, 5, "k14: node_count=5");
    assert_eq!(ec, 4, "k14: edge_count=4");
    assert_eq!(nennaactc,  u64::MAX, "k14: SAT");
    assert_eq!(nhennaactc, u64::MAX, "k14: SAT");
    assert_eq!(nbggso,     u64::MAX, "k14: SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T116_VEC_A, T116_KEY_A, T116_ID_A);
    add_node(T116_VEC_B, T116_KEY_B, T116_ID_B);
    add_node(T116_VEC_C, T116_KEY_C, T116_ID_C);
    add_node(T116_VEC_D, T116_KEY_D, T116_ID_D);
    add_edge(T116_ID_A, T116_ID_B, "t116.e.ab");
    add_edge(T116_ID_B, T116_ID_C, "t116.e.bc");
    add_edge(T116_ID_C, T116_ID_D, "t116.e.cd");
    let (nennaactc, nhennaactc, nbggso, ec, nc) = gos_runtime::graph_topo_indices116();
    assert_eq!(nc, 4, "p4: node_count=4");
    assert_eq!(ec, 3, "p4: edge_count=3");
    assert_eq!(nennaactc,  u64::MAX, "p4: SAT");
    assert_eq!(nhennaactc, u64::MAX, "p4: SAT");
    assert_eq!(nbggso,     u64::MAX, "p4: SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T116_VEC_A, T116_KEY_A, T116_ID_A);
    add_node(T116_VEC_B, T116_KEY_B, T116_ID_B);
    add_node(T116_VEC_C, T116_KEY_C, T116_ID_C);
    add_node(T116_VEC_D, T116_KEY_D, T116_ID_D);
    add_edge(T116_ID_A, T116_ID_B, "t116.e.ab");
    add_edge(T116_ID_A, T116_ID_C, "t116.e.ac");
    add_edge(T116_ID_A, T116_ID_D, "t116.e.ad");
    add_edge(T116_ID_B, T116_ID_C, "t116.e.bc");
    add_edge(T116_ID_B, T116_ID_D, "t116.e.bd");
    add_edge(T116_ID_C, T116_ID_D, "t116.e.cd");
    let (nennaactc, nhennaactc, nbggso, ec, nc) = gos_runtime::graph_topo_indices116();
    assert_eq!(nc, 4, "k4: node_count=4");
    assert_eq!(ec, 6, "k4: edge_count=6");
    assert_eq!(nennaactc,  u64::MAX, "k4: SAT");
    assert_eq!(nhennaactc, u64::MAX, "k4: SAT");
    assert_eq!(nbggso,     u64::MAX, "k4: SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T116_VEC_A, T116_KEY_A, T116_ID_A);
    add_node(T116_VEC_B, T116_KEY_B, T116_ID_B);
    let (nennaactc, nhennaactc, nbggso, ec, nc) = gos_runtime::graph_topo_indices116();
    assert_eq!(nc,         2, "2iso: node_count=2");
    assert_eq!(ec,         0, "2iso: edge_count=0");
    assert_eq!(nennaactc,  0, "2iso: NENNAACTC=0");
    assert_eq!(nhennaactc, 0, "2iso: NHENNAACTC=0");
    assert_eq!(nbggso,     0, "2iso: NBGGSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    add_node(T116_VEC_A, T116_KEY_A, T116_ID_A);
    add_node(T116_VEC_B, T116_KEY_B, T116_ID_B);
    add_node(T116_VEC_C, T116_KEY_C, T116_ID_C);
    add_node(T116_VEC_D, T116_KEY_D, T116_ID_D);
    add_node(T116_VEC_E, T116_KEY_E, T116_ID_E);
    add_edge(T116_ID_A, T116_ID_C, "t116.e.ac");
    add_edge(T116_ID_A, T116_ID_D, "t116.e.ad");
    add_edge(T116_ID_A, T116_ID_E, "t116.e.ae");
    add_edge(T116_ID_B, T116_ID_C, "t116.e.bc");
    add_edge(T116_ID_B, T116_ID_D, "t116.e.bd");
    add_edge(T116_ID_B, T116_ID_E, "t116.e.be");
    let (nennaactc, nhennaactc, nbggso, ec, nc) = gos_runtime::graph_topo_indices116();
    assert_eq!(nc, 5, "k23: node_count=5");
    assert_eq!(ec, 6, "k23: edge_count=6");
    assert_eq!(nennaactc,  u64::MAX, "k23: SAT");
    assert_eq!(nhennaactc, u64::MAX, "k23: SAT");
    assert_eq!(nbggso,     u64::MAX, "k23: SAT");
}
