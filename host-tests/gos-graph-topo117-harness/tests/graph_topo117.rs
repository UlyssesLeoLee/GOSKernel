// gos-graph-topo117-harness — V3.128 NENNAMONOACTC + NHENNAMONOACTC + NBHHSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices117()`:
//   Returns (nennamonoactc, nhennamonoactc, nbhhso, edge_count, node_count)
//   - nennamonoactc   = NENNAMONOACTC(G) = Σ_v S(v)^91                        (saturating u64)
//   - nhennamonoactc  = NHENNAMONOACTC(G)= Σ_{uv∈E} (S_u+S_v)^90            (saturating u64)
//   - nbhhso          = NBHHSO(G)        = Σ_{uv∈E} (S_u²+S_v²)^85          (saturating u64)
//
// NENNAMONOACTC: 2nd of ennacontic (90-99) series. Extends NENNAACTC=Σ S^90 (topo116).
//   s^91 = s90×s = s88×s2×s  (91=64+16+8+2+1; 10 mults).
// NHENNAMONOACTC: ss^90 = ss88×ss2  (90=64+16+8+2; 9 mults).
// NBHHSO: α=170, 34th of NB series. s2s^85 = s2s84×s2s  (85=64+16+4+1; 9 mults).
//
// ANALYTICAL CROSS-CHECK TABLE:
//  Graph     NENNAMONOACTC        NHENNAMONOACTC        NBHHSO      edges nodes
//  Empty                 0                     0             0           0     0
//  1 node                0                     0             0           0     1
//  K₂ (S=1)              2       u64::MAX(sat.)  u64::MAX(sat.)          1     2
//  P₃ (S=2)  u64::MAX(sat.)      u64::MAX(sat.)  u64::MAX(sat.)          2     3
//  Others    u64::MAX(sat.)      u64::MAX(sat.)  u64::MAX(sat.)         ...   ...
//
// K₂ derivation:
//   NENNAMONOACTC  = 2×1^91 = 2.  (exact; odd exponent on S=1)
//   NHENNAMONOACTC = 1×(1+1)^90 = 2^90 > u64::MAX. (saturated)
//   NBHHSO         = 1×(1²+1²)^85 = 2^85 > u64::MAX. (saturated)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const T117_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX117");
const T117_EXEC:   ExecutorId = ExecutorId::from_ascii("t117.exec");

const T117_KEY_A: &str = "t117.alpha";
const T117_KEY_B: &str = "t117.beta";
const T117_KEY_C: &str = "t117.gamma";
const T117_KEY_D: &str = "t117.delta";
const T117_KEY_E: &str = "t117.epsilon";

const T117_ID_A: NodeId = derive_node_id(T117_PLUGIN, T117_KEY_A);
const T117_ID_B: NodeId = derive_node_id(T117_PLUGIN, T117_KEY_B);
const T117_ID_C: NodeId = derive_node_id(T117_PLUGIN, T117_KEY_C);
const T117_ID_D: NodeId = derive_node_id(T117_PLUGIN, T117_KEY_D);
const T117_ID_E: NodeId = derive_node_id(T117_PLUGIN, T117_KEY_E);

// L4=204 namespace for this harness.
const T117_VEC_A: VectorAddress = VectorAddress::new(204, 1, 1, 0);
const T117_VEC_B: VectorAddress = VectorAddress::new(204, 1, 2, 0);
const T117_VEC_C: VectorAddress = VectorAddress::new(204, 1, 3, 0);
const T117_VEC_D: VectorAddress = VectorAddress::new(204, 2, 1, 0);
const T117_VEC_E: VectorAddress = VectorAddress::new(204, 2, 2, 0);

const T117_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T117_PLUGIN,
    name:         "kl-graph-topo117-harness",
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
        executor_id:       T117_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T117_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T117_MANIFEST).unwrap();
    g
}

#[test]
fn test_01_empty() {
    let _g = setup();
    let (a, b, c, ec, nc) = gos_runtime::graph_topo_indices117();
    assert_eq!(nc, 0); assert_eq!(ec, 0);
    assert_eq!(a, 0, "empty: NENNAMONOACTC=0");
    assert_eq!(b, 0, "empty: NHENNAMONOACTC=0");
    assert_eq!(c, 0, "empty: NBHHSO=0");
}

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T117_VEC_A, T117_KEY_A, T117_ID_A);
    let (a, b, c, ec, nc) = gos_runtime::graph_topo_indices117();
    assert_eq!(nc, 1); assert_eq!(ec, 0);
    assert_eq!(a, 0, "single: NENNAMONOACTC=0");
    assert_eq!(b, 0, "single: NHENNAMONOACTC=0");
    assert_eq!(c, 0, "single: NBHHSO=0");
}

#[test]
fn test_03_k2_edge() {
    // K₂: S=1. NENNAMONOACTC=2×1^91=2. Others saturate.
    let _g = setup();
    add_node(T117_VEC_A, T117_KEY_A, T117_ID_A);
    add_node(T117_VEC_B, T117_KEY_B, T117_ID_B);
    add_edge(T117_ID_A, T117_ID_B, "t117.e.ab");
    let (nennamonoactc, nhennamonoactc, nbhhso, ec, nc) = gos_runtime::graph_topo_indices117();
    assert_eq!(nc, 2); assert_eq!(ec, 1);
    assert_eq!(nennamonoactc,  2,        "k2: NENNAMONOACTC=2 (1^91+1^91=2)");
    assert_eq!(nhennamonoactc, u64::MAX, "k2: NHENNAMONOACTC=SAT (2^90>u64::MAX)");
    assert_eq!(nbhhso,         u64::MAX, "k2: NBHHSO=SAT (2^85>u64::MAX)");
}

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T117_VEC_A, T117_KEY_A, T117_ID_A);
    add_node(T117_VEC_B, T117_KEY_B, T117_ID_B);
    add_node(T117_VEC_C, T117_KEY_C, T117_ID_C);
    add_edge(T117_ID_A, T117_ID_B, "t117.e.ab");
    add_edge(T117_ID_B, T117_ID_C, "t117.e.bc");
    let (a, b, c, ec, nc) = gos_runtime::graph_topo_indices117();
    assert_eq!(nc, 3); assert_eq!(ec, 2);
    assert_eq!(a, u64::MAX, "p3: SAT"); assert_eq!(b, u64::MAX, "p3: SAT"); assert_eq!(c, u64::MAX, "p3: SAT");
}

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T117_VEC_A, T117_KEY_A, T117_ID_A);
    add_node(T117_VEC_B, T117_KEY_B, T117_ID_B);
    add_node(T117_VEC_C, T117_KEY_C, T117_ID_C);
    add_edge(T117_ID_A, T117_ID_B, "t117.e.ab");
    add_edge(T117_ID_B, T117_ID_C, "t117.e.bc");
    add_edge(T117_ID_C, T117_ID_A, "t117.e.ca");
    let (a, b, c, ec, nc) = gos_runtime::graph_topo_indices117();
    assert_eq!(nc, 3); assert_eq!(ec, 3);
    assert_eq!(a, u64::MAX); assert_eq!(b, u64::MAX); assert_eq!(c, u64::MAX);
}

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T117_VEC_A, T117_KEY_A, T117_ID_A);
    add_node(T117_VEC_B, T117_KEY_B, T117_ID_B);
    add_node(T117_VEC_C, T117_KEY_C, T117_ID_C);
    add_node(T117_VEC_D, T117_KEY_D, T117_ID_D);
    add_node(T117_VEC_E, T117_KEY_E, T117_ID_E);
    add_edge(T117_ID_A, T117_ID_B, "t117.e.ab");
    add_edge(T117_ID_A, T117_ID_C, "t117.e.ac");
    add_edge(T117_ID_A, T117_ID_D, "t117.e.ad");
    add_edge(T117_ID_A, T117_ID_E, "t117.e.ae");
    let (a, b, c, ec, nc) = gos_runtime::graph_topo_indices117();
    assert_eq!(nc, 5); assert_eq!(ec, 4);
    assert_eq!(a, u64::MAX); assert_eq!(b, u64::MAX); assert_eq!(c, u64::MAX);
}

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T117_VEC_A, T117_KEY_A, T117_ID_A);
    add_node(T117_VEC_B, T117_KEY_B, T117_ID_B);
    add_node(T117_VEC_C, T117_KEY_C, T117_ID_C);
    add_node(T117_VEC_D, T117_KEY_D, T117_ID_D);
    add_edge(T117_ID_A, T117_ID_B, "t117.e.ab");
    add_edge(T117_ID_B, T117_ID_C, "t117.e.bc");
    add_edge(T117_ID_C, T117_ID_D, "t117.e.cd");
    let (a, b, c, ec, nc) = gos_runtime::graph_topo_indices117();
    assert_eq!(nc, 4); assert_eq!(ec, 3);
    assert_eq!(a, u64::MAX); assert_eq!(b, u64::MAX); assert_eq!(c, u64::MAX);
}

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T117_VEC_A, T117_KEY_A, T117_ID_A);
    add_node(T117_VEC_B, T117_KEY_B, T117_ID_B);
    add_node(T117_VEC_C, T117_KEY_C, T117_ID_C);
    add_node(T117_VEC_D, T117_KEY_D, T117_ID_D);
    add_edge(T117_ID_A, T117_ID_B, "t117.e.ab");
    add_edge(T117_ID_A, T117_ID_C, "t117.e.ac");
    add_edge(T117_ID_A, T117_ID_D, "t117.e.ad");
    add_edge(T117_ID_B, T117_ID_C, "t117.e.bc");
    add_edge(T117_ID_B, T117_ID_D, "t117.e.bd");
    add_edge(T117_ID_C, T117_ID_D, "t117.e.cd");
    let (a, b, c, ec, nc) = gos_runtime::graph_topo_indices117();
    assert_eq!(nc, 4); assert_eq!(ec, 6);
    assert_eq!(a, u64::MAX); assert_eq!(b, u64::MAX); assert_eq!(c, u64::MAX);
}

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T117_VEC_A, T117_KEY_A, T117_ID_A);
    add_node(T117_VEC_B, T117_KEY_B, T117_ID_B);
    let (a, b, c, ec, nc) = gos_runtime::graph_topo_indices117();
    assert_eq!(nc, 2); assert_eq!(ec, 0);
    assert_eq!(a, 0, "2iso: NENNAMONOACTC=0");
    assert_eq!(b, 0, "2iso: NHENNAMONOACTC=0");
    assert_eq!(c, 0, "2iso: NBHHSO=0");
}

#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    add_node(T117_VEC_A, T117_KEY_A, T117_ID_A);
    add_node(T117_VEC_B, T117_KEY_B, T117_ID_B);
    add_node(T117_VEC_C, T117_KEY_C, T117_ID_C);
    add_node(T117_VEC_D, T117_KEY_D, T117_ID_D);
    add_node(T117_VEC_E, T117_KEY_E, T117_ID_E);
    add_edge(T117_ID_A, T117_ID_C, "t117.e.ac");
    add_edge(T117_ID_A, T117_ID_D, "t117.e.ad");
    add_edge(T117_ID_A, T117_ID_E, "t117.e.ae");
    add_edge(T117_ID_B, T117_ID_C, "t117.e.bc");
    add_edge(T117_ID_B, T117_ID_D, "t117.e.bd");
    add_edge(T117_ID_B, T117_ID_E, "t117.e.be");
    let (a, b, c, ec, nc) = gos_runtime::graph_topo_indices117();
    assert_eq!(nc, 5); assert_eq!(ec, 6);
    assert_eq!(a, u64::MAX); assert_eq!(b, u64::MAX); assert_eq!(c, u64::MAX);
}
