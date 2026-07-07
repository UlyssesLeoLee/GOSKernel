// gos-graph-topo13-harness — V3.24 Transmission Zagreb indices
//
// Verifies `gos_runtime::graph_topo_indices13()`:
//   Returns (tm1, tm2, ga_t, edge_count, node_count)
//   - tm1    = TM₁(G) = Σ_v T_v²                                (exact u64; Xing & Gutman 2012)
//   - tm2    = TM₂(G) = Σ_{uv∈E} T_u·T_v                       (exact u64)
//   - ga_t   = GA_t(G) × 10^6 = Σ_{uv∈E} 2√(T_u·T_v)/(T_u+T_v)(floor ppm; Alizadeh et al. 2013)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// DEFINITIONS:
//   T_v = vertex transmission = Σ_{w reachable, w≠v} d(v,w) (BFS distance sum)
//   Isolated nodes: T_v=0, contribute 0 to TM₁, no edge contribution.
//   TM₁: node-scan index (squared transmission sum)
//   TM₂: edge-scan index (transmission product sum)
//   GA_t: geometric-arithmetic transmission index
//         = |E|×10^6 iff all node transmissions are equal (transmission-regular)
//
// KEY INVARIANTS:
//   GA_t = |E|×10^6 iff graph is transmission-regular (all T_v equal).
//   For Δ-regular graphs: all T_v equal iff graph is vertex-transitive (K_n, cycles, K_{r,s}).
//   TM₁ ≥ TM₂ for graphs with equal transmissions (Cauchy-Schwarz).
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph           TM₁    TM₂    GA_t(ppm)   edges  nodes
//  Empty            0      0      0            0      0
//  1 node           0      0      0            0      1
//  Edge A-B         2      1      1_000_000    1      2
//  Path P₃         22     12      1_959_590    2      3
//  Triangle K₃     12     12      3_000_000    3      3
//  Star K_{1,4}   212    112      3_848_364    4      5
//  Path P₄        104     64      2_959_590    3      4
//  Complete K₄     36     54      6_000_000    6      4
//  Two isolated     0      0      0            0      2
//  K_{2,3}        158    180      5_975_154    6      5
//
// Derivations:
//
//   Edge A-B: T_A=T_B=1.
//     TM₁=1+1=2. TM₂=1. GA_t=isqrt128(4×1×1×10^12)/2=2_000_000/2=1_000_000.
//
//   P₃ (A-B-C): T_A=1+2=3, T_B=1+1=2, T_C=2+1=3.
//     TM₁=9+4+9=22. TM₂={A,B}:3×2=6; {B,C}:2×3=6. TM₂=12.
//     GA_t per {A,B}: isqrt128(4×3×2×10^12)/5 = isqrt128(24×10^12)/5
//       = 4_898_979/5 = 979_795. Both edges same → GA_t=2×979_795=1_959_590.
//
//   K₃: T_A=T_B=T_C=2 (each node sum of 2 unit-distances). TM₁=3×4=12.
//     TM₂=3×4=12. GA_t=3×isqrt128(4×4×10^12)/4=3×4_000_000/4=3×1_000_000=3_000_000.
//
//   K_{1,4}: T_A=4 (center, 4 leaves at d=1).
//     Each leaf: T=1+2+2+2=7 (to center=1, 3 other leaves=2 via center).
//     TM₁=16+4×49=16+196=212. TM₂=4×4×7=4×28=112.
//     GA_t per edge: isqrt128(4×4×7×10^12)/11=isqrt128(112×10^12)/11
//       =10_583_005/11=962_091. GA_t=4×962_091=3_848_364.
//
//   P₄ (A-B-C-D): T_A=1+2+3=6, T_B=1+1+2=4, T_C=2+1+1=4, T_D=3+2+1=6.
//     TM₁=36+16+16+36=104. TM₂={A,B}:6×4=24; {B,C}:4×4=16; {C,D}:4×6=24. TM₂=64.
//     GA_t: {A,B} and {C,D}: isqrt128(96×10^12)/10=9_797_958/10=979_795.
//           {B,C}: isqrt128(64×10^12)/8=8_000_000/8=1_000_000.
//     GA_t=979_795+1_000_000+979_795=2_959_590.
//
//   K₄: all T=3. TM₁=4×9=36. TM₂=6×9=54. GA_t=6×1_000_000=6_000_000.
//
//   Two isolated: T_A=T_B=0. TM₁=0. TM₂=0. GA_t=0.
//
//   K_{2,3}: left={A,B}(T=5), right={C,D,E}(T=6).
//     T_A=d(A,B)+d(A,C)+d(A,D)+d(A,E)=2+1+1+1=5; T_C=1+1+2+2=6.
//     TM₁=2×25+3×36=50+108=158. TM₂=6×5×6=180.
//     GA_t per edge: isqrt128(4×5×6×10^12)/11=isqrt128(120×10^12)/11
//       =10_954_451/11=995_859. GA_t=6×995_859=5_975_154.
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (2, 1, 1_000_000, 1, 2)
//  4.  Path P₃ = A-B-C                    → (22, 12, 1_959_590, 2, 3)
//  5.  Triangle K₃                        → (12, 12, 3_000_000, 3, 3)
//  6.  Star K_{1,4}                       → (212, 112, 3_848_364, 4, 5)
//  7.  Path P₄ = A-B-C-D                  → (104, 64, 2_959_590, 3, 4)
//  8.  Complete K₄                        → (36, 54, 6_000_000, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check      → (158, 180, 5_975_154, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T13_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_13");
const T13_EXEC:   ExecutorId = ExecutorId::from_ascii("t13.exec");

const T13_KEY_A: &str = "t13.alpha";
const T13_KEY_B: &str = "t13.beta";
const T13_KEY_C: &str = "t13.gamma";
const T13_KEY_D: &str = "t13.delta";
const T13_KEY_E: &str = "t13.epsilon";

const T13_ID_A: NodeId = derive_node_id(T13_PLUGIN, T13_KEY_A);
const T13_ID_B: NodeId = derive_node_id(T13_PLUGIN, T13_KEY_B);
const T13_ID_C: NodeId = derive_node_id(T13_PLUGIN, T13_KEY_C);
const T13_ID_D: NodeId = derive_node_id(T13_PLUGIN, T13_KEY_D);
const T13_ID_E: NodeId = derive_node_id(T13_PLUGIN, T13_KEY_E);

// L4=100 namespace for this harness.
const T13_VEC_A: VectorAddress = VectorAddress::new(100, 1, 1, 0);
const T13_VEC_B: VectorAddress = VectorAddress::new(100, 1, 2, 0);
const T13_VEC_C: VectorAddress = VectorAddress::new(100, 1, 3, 0);
const T13_VEC_D: VectorAddress = VectorAddress::new(100, 2, 1, 0);
const T13_VEC_E: VectorAddress = VectorAddress::new(100, 2, 2, 0);

const T13_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T13_PLUGIN,
    name:         "kl-graph-topo13-harness",
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
        executor_id:       T13_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T13_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T13_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (tm1, tm2, ga_t, ec, nc) = gos_runtime::graph_topo_indices13();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(tm1,  0, "empty: TM1=0");
    assert_eq!(tm2,  0, "empty: TM2=0");
    assert_eq!(ga_t, 0, "empty: GA_t=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// T_v=0 for isolated node; no edges. All indices are 0.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T13_VEC_A, T13_KEY_A, T13_ID_A);

    let (tm1, tm2, ga_t, ec, nc) = gos_runtime::graph_topo_indices13();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(tm1,  0, "single: TM1=0 (T=0 for isolated)");
    assert_eq!(tm2,  0, "single: TM2=0");
    assert_eq!(ga_t, 0, "single: GA_t=0");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected: {A,B}. T_A=T_B=1.
// TM1=1+1=2. TM2=1×1=1.
// GA_t: isqrt128(4×1×1×10^12)/(1+1) = 2_000_000/2 = 1_000_000.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T13_VEC_A, T13_KEY_A, T13_ID_A);
    add_node(T13_VEC_B, T13_KEY_B, T13_ID_B);
    add_edge(T13_ID_A, T13_ID_B, "t13.e.ab");

    let (tm1, tm2, ga_t, ec, nc) = gos_runtime::graph_topo_indices13();
    assert_eq!(nc,        2,         "edge: node_count=2");
    assert_eq!(ec,        1,         "edge: edge_count=1");
    assert_eq!(tm1,       2,         "edge: TM1=2 (T_A=T_B=1; 1²+1²)");
    assert_eq!(tm2,       1,         "edge: TM2=1 (1×1)");
    assert_eq!(ga_t,      1_000_000, "edge: GA_t=1_000_000 (trans-regular)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// T_A=3, T_B=2, T_C=3.
// TM1=9+4+9=22. TM2=3×2+2×3=12.
// GA_t: each edge isqrt128(4×3×2×10^12)/5 = 4_898_979/5 = 979_795.
// GA_t=2×979_795=1_959_590.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T13_VEC_A, T13_KEY_A, T13_ID_A);
    add_node(T13_VEC_B, T13_KEY_B, T13_ID_B);
    add_node(T13_VEC_C, T13_KEY_C, T13_ID_C);
    add_edge(T13_ID_A, T13_ID_B, "t13.e.ab");
    add_edge(T13_ID_B, T13_ID_C, "t13.e.bc");

    let (tm1, tm2, ga_t, ec, nc) = gos_runtime::graph_topo_indices13();
    assert_eq!(nc,   3,         "p3: node_count=3");
    assert_eq!(ec,   2,         "p3: edge_count=2");
    assert_eq!(tm1,  22,        "p3: TM1=22 (9+4+9; T_end=3, T_mid=2)");
    assert_eq!(tm2,  12,        "p3: TM2=12 (3×2+2×3)");
    assert_eq!(ga_t, 1_959_590, "p3: GA_t=1_959_590 (2×979_795; isqrt128(24×10^12)/5)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// All T=2 (each node: distances to 2 neighbours = 1+1=2).
// TM1=3×4=12. TM2=3×4=12.
// GA_t: each edge isqrt128(4×2×2×10^12)/4=4_000_000/4=1_000_000.
// GA_t=3×1_000_000=3_000_000 (transmission-regular).

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T13_VEC_A, T13_KEY_A, T13_ID_A);
    add_node(T13_VEC_B, T13_KEY_B, T13_ID_B);
    add_node(T13_VEC_C, T13_KEY_C, T13_ID_C);
    add_edge(T13_ID_A, T13_ID_B, "t13.e.ab");
    add_edge(T13_ID_B, T13_ID_A, "t13.e.ba");
    add_edge(T13_ID_B, T13_ID_C, "t13.e.bc");
    add_edge(T13_ID_C, T13_ID_B, "t13.e.cb");
    add_edge(T13_ID_A, T13_ID_C, "t13.e.ac");
    add_edge(T13_ID_C, T13_ID_A, "t13.e.ca");

    let (tm1, tm2, ga_t, ec, nc) = gos_runtime::graph_topo_indices13();
    assert_eq!(nc,   3,         "k3: node_count=3");
    assert_eq!(ec,   3,         "k3: edge_count=3");
    assert_eq!(tm1,  12,        "k3: TM1=12 (3×4; all T=2)");
    assert_eq!(tm2,  12,        "k3: TM2=12 (3×4; 3 edges × 2×2=4)");
    assert_eq!(ga_t, 3_000_000, "k3: GA_t=3_000_000 (transmission-regular; all T=2)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Center A: T_A=4 (4 leaves at d=1).
// Each leaf B,C,D,E: T=1+2+2+2=7 (to center=1, 3 other leaves=2).
// TM1=16+4×49=212. TM2=4×4×7=112.
// GA_t per edge: isqrt128(4×4×7×10^12)/11=10_583_005/11=962_091.
// GA_t=4×962_091=3_848_364.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T13_VEC_A, T13_KEY_A, T13_ID_A);
    add_node(T13_VEC_B, T13_KEY_B, T13_ID_B);
    add_node(T13_VEC_C, T13_KEY_C, T13_ID_C);
    add_node(T13_VEC_D, T13_KEY_D, T13_ID_D);
    add_node(T13_VEC_E, T13_KEY_E, T13_ID_E);
    add_edge(T13_ID_A, T13_ID_B, "t13.e.ab");
    add_edge(T13_ID_A, T13_ID_C, "t13.e.ac");
    add_edge(T13_ID_A, T13_ID_D, "t13.e.ad");
    add_edge(T13_ID_A, T13_ID_E, "t13.e.ae");

    let (tm1, tm2, ga_t, ec, nc) = gos_runtime::graph_topo_indices13();
    assert_eq!(nc,   5,         "star: node_count=5");
    assert_eq!(ec,   4,         "star: edge_count=4");
    assert_eq!(tm1,  212,       "star: TM1=212 (T_center=4, T_leaf=7; 16+196)");
    assert_eq!(tm2,  112,       "star: TM2=112 (4 edges × 4×7=28)");
    assert_eq!(ga_t, 3_848_364, "star: GA_t=3_848_364 (4×962_091; isqrt128(112×10^12)/11)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// T_A=1+2+3=6, T_B=1+1+2=4, T_C=2+1+1=4, T_D=3+2+1=6.
// TM1=36+16+16+36=104. TM2=6×4+4×4+4×6=24+16+24=64.
// GA_t: {A,B}: isqrt128(96×10^12)/10=9_797_958/10=979_795.
//        {B,C}: isqrt128(64×10^12)/8=8_000_000/8=1_000_000.
//        {C,D}: 979_795. GA_t=979_795+1_000_000+979_795=2_959_590.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T13_VEC_A, T13_KEY_A, T13_ID_A);
    add_node(T13_VEC_B, T13_KEY_B, T13_ID_B);
    add_node(T13_VEC_C, T13_KEY_C, T13_ID_C);
    add_node(T13_VEC_D, T13_KEY_D, T13_ID_D);
    add_edge(T13_ID_A, T13_ID_B, "t13.e.ab");
    add_edge(T13_ID_B, T13_ID_C, "t13.e.bc");
    add_edge(T13_ID_C, T13_ID_D, "t13.e.cd");

    let (tm1, tm2, ga_t, ec, nc) = gos_runtime::graph_topo_indices13();
    assert_eq!(nc,   4,         "p4: node_count=4");
    assert_eq!(ec,   3,         "p4: edge_count=3");
    assert_eq!(tm1,  104,       "p4: TM1=104 (36+16+16+36; T_end=6, T_mid=4)");
    assert_eq!(tm2,  64,        "p4: TM2=64 (24+16+24)");
    assert_eq!(ga_t, 2_959_590, "p4: GA_t=2_959_590 (979_795+1_000_000+979_795)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// All T=3 (each node reaches 3 others at d=1).
// TM1=4×9=36. TM2=6×9=54.
// GA_t=6×1_000_000=6_000_000 (transmission-regular).

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T13_VEC_A, T13_KEY_A, T13_ID_A);
    add_node(T13_VEC_B, T13_KEY_B, T13_ID_B);
    add_node(T13_VEC_C, T13_KEY_C, T13_ID_C);
    add_node(T13_VEC_D, T13_KEY_D, T13_ID_D);
    add_edge(T13_ID_A, T13_ID_B, "t13.e.ab");
    add_edge(T13_ID_B, T13_ID_A, "t13.e.ba");
    add_edge(T13_ID_A, T13_ID_C, "t13.e.ac");
    add_edge(T13_ID_C, T13_ID_A, "t13.e.ca");
    add_edge(T13_ID_A, T13_ID_D, "t13.e.ad");
    add_edge(T13_ID_D, T13_ID_A, "t13.e.da");
    add_edge(T13_ID_B, T13_ID_C, "t13.e.bc");
    add_edge(T13_ID_C, T13_ID_B, "t13.e.cb");
    add_edge(T13_ID_B, T13_ID_D, "t13.e.bd");
    add_edge(T13_ID_D, T13_ID_B, "t13.e.db");
    add_edge(T13_ID_C, T13_ID_D, "t13.e.cd");
    add_edge(T13_ID_D, T13_ID_C, "t13.e.dc");

    let (tm1, tm2, ga_t, ec, nc) = gos_runtime::graph_topo_indices13();
    assert_eq!(nc,   4,         "k4: node_count=4");
    assert_eq!(ec,   6,         "k4: edge_count=6");
    assert_eq!(tm1,  36,        "k4: TM1=36 (4×9; all T=3)");
    assert_eq!(tm2,  54,        "k4: TM2=54 (6×9; 6 edges × 3×3)");
    assert_eq!(ga_t, 6_000_000, "k4: GA_t=6_000_000 (transmission-regular; all T=3)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// n=2, m=0. T_A=T_B=0. All indices are 0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T13_VEC_A, T13_KEY_A, T13_ID_A);
    add_node(T13_VEC_B, T13_KEY_B, T13_ID_B);

    let (tm1, tm2, ga_t, ec, nc) = gos_runtime::graph_topo_indices13();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(tm1,  0, "isolated: TM1=0 (T=0 for both)");
    assert_eq!(tm2,  0, "isolated: TM2=0");
    assert_eq!(ga_t, 0, "isolated: GA_t=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}, right={C,D,E}. All left-right edges present.
// T_A=T_B: d(A,B)=2, d(A,C)=d(A,D)=d(A,E)=1. T_A=5.
// T_C=T_D=T_E: d(C,A)=d(C,B)=1, d(C,D)=d(C,E)=2. T_C=6.
// TM1=2×25+3×36=50+108=158.
// TM2=6 edges × 5×6=6×30=180.
// GA_t per edge: isqrt128(4×5×6×10^12)/11=isqrt128(120×10^12)/11=10_954_451/11=995_859.
// GA_t=6×995_859=5_975_154.
// INVARIANT: GA_t < 6_000_000 confirms K_{2,3} is NOT transmission-regular.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T13_VEC_A, T13_KEY_A, T13_ID_A);
    add_node(T13_VEC_B, T13_KEY_B, T13_ID_B);
    add_node(T13_VEC_C, T13_KEY_C, T13_ID_C);
    add_node(T13_VEC_D, T13_KEY_D, T13_ID_D);
    add_node(T13_VEC_E, T13_KEY_E, T13_ID_E);
    // Left A connects to all right
    add_edge(T13_ID_A, T13_ID_C, "t13.e.ac");
    add_edge(T13_ID_C, T13_ID_A, "t13.e.ca");
    add_edge(T13_ID_A, T13_ID_D, "t13.e.ad");
    add_edge(T13_ID_D, T13_ID_A, "t13.e.da");
    add_edge(T13_ID_A, T13_ID_E, "t13.e.ae");
    add_edge(T13_ID_E, T13_ID_A, "t13.e.ea");
    // Left B connects to all right
    add_edge(T13_ID_B, T13_ID_C, "t13.e.bc");
    add_edge(T13_ID_C, T13_ID_B, "t13.e.cb");
    add_edge(T13_ID_B, T13_ID_D, "t13.e.bd");
    add_edge(T13_ID_D, T13_ID_B, "t13.e.db");
    add_edge(T13_ID_B, T13_ID_E, "t13.e.be");
    add_edge(T13_ID_E, T13_ID_B, "t13.e.eb");

    let (tm1, tm2, ga_t, ec, nc) = gos_runtime::graph_topo_indices13();
    assert_eq!(nc,   5,         "k23: node_count=5");
    assert_eq!(ec,   6,         "k23: edge_count=6");
    assert_eq!(tm1,  158,       "k23: TM1=158 (T_left=5 → 2×25=50; T_right=6 → 3×36=108)");
    assert_eq!(tm2,  180,       "k23: TM2=180 (6 edges × 5×6=30)");
    assert_eq!(ga_t, 5_975_154, "k23: GA_t=5_975_154 (6×995_859; isqrt128(120×10^12)/11)");
}
