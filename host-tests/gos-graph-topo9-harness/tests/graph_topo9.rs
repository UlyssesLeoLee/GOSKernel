// gos-graph-topo9-harness — V3.20 degree-distance hybrid topological indices
//
// Verifies `gos_runtime::graph_topo_indices9()`:
//   Returns (ws, wg, cxe_ppm, edge_count, node_count)
//   - ws      = W_S(G) = Σ_{u<v} (deg(u)+deg(v))×d(u,v)    (exact u64; Schultz 1989)
//   - wg      = W_G(G) = Σ_{u<v} deg(u)×deg(v)×d(u,v)      (exact u64; Gutman 1994)
//   - cxe_ppm = CξE(G)×10^6 = Σ_v deg(v)/ecc(v) × 10^6     (floor ppm; Gupta et al. 2000)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// Undirected projection: A→B + B→A collapses to one undirected edge A-B.
// Disconnected pairs (d=∞): contribute 0 to W_S and W_G.
// Isolated nodes (ecc=0 ⇒ deg=0): contribute 0 to CξE.
//
// KEY INVARIANTS:
//   Regular Δ-graph, n nodes, m edges:
//     W_S = 2Δ × W(G)  (since deg(u)+deg(v) = 2Δ for all edges; W = Wiener index)
//     W_G = Δ² × W(G)
//     CξE = n × Δ/D × 10^6  where D = diameter (if self-centered)
//   Complete K_n (all d=1, deg=n-1):
//     W_S = 2(n-1) × m = 2(n-1) × n(n-1)/2 = n(n-1)²
//     W_G = (n-1)² × m = (n-1)² × n(n-1)/2 = n(n-1)³/2
//     CξE = n × (n-1)/1 × 10^6 = n(n-1) × 10^6
//   Star K_{1,k}: W_S = 5k(k+3)/2 - k? Let's just use exact values below.
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph          W_S   W_G  CxiE_ppm  edges  nodes
//  Empty            0     0         0      0      0
//  1 node           0     0         0      0      1
//  Edge A-B         2     1 2_000_000      1      2
//  Path P₃         10     6 3_000_000      2      3
//  Triangle K₃     12    12 6_000_000      3      3
//  Star K_{1,4}    44    28 6_000_000      4      5
//  Path P₄         28    19 2_666_666      3      4
//  Complete K₄     36    54 12_000_000     6      4
//  2 isolated       0     0         0      0      2
//  K_{2,3}         66    78 6_000_000      6      5
//
// Derivations:
//   Edge A-B: 1 pair d=1; deg=[1,1]; W_S=(1+1)×1=2; W_G=1×1×1=1; CξE=(1/1+1/1)×10^6=2_000_000
//   P₃ (A-B-C): deg=[1,2,1], ecc=[2,1,2]
//     A-B d=1: W_S+=(1+2)=3; W_G+=1×2=2
//     B-C d=1: W_S+=(2+1)=3; W_G+=2×1=2
//     A-C d=2: W_S+=(1+1)×2=4; W_G+=1×1×2=2
//     W_S=10; W_G=6; CξE=(1/2+2/1+1/2)×10^6=3_000_000
//   K₃: all d=1, deg=2, ecc=1; 3 pairs each (2+2)×1=4; W_S=12; W_G=3×4=12; CξE=3×2×10^6=6_000_000
//   K_{1,4}: center A(deg=4,ecc=1), leaves B..E(deg=1,ecc=2)
//     4 center-leaf d=1: W_S+=(4+1)×1=5 each→20; W_G+=4×1×1=4 each→16
//     6 leaf-leaf d=2: W_S+=(1+1)×2=4 each→24; W_G+=1×1×2=2 each→12
//     W_S=44; W_G=28; CξE=4_000_000+4×500_000=6_000_000
//   P₄ (A-B-C-D): deg=[1,2,2,1], ecc=[3,2,2,3]
//     A-B d=1: W_S+=3; W_G+=2
//     A-C d=2: W_S+=6; W_G+=4
//     A-D d=3: W_S+=6; W_G+=3
//     B-C d=1: W_S+=4; W_G+=4
//     B-D d=2: W_S+=6; W_G+=4
//     C-D d=1: W_S+=3; W_G+=2
//     W_S=28; W_G=19; CξE=floor(1/3×10^6)×2+1×10^6×2=333_333×2+2_000_000=2_666_666
//   K₄: all d=1, deg=3, ecc=1; 6 pairs each (3+3)×1=6; W_S=36; W_G=6×9=54; CξE=4×3×10^6=12_000_000
//   K_{2,3}: left={A,B}(deg=3,ecc=2), right={C,D,E}(deg=2,ecc=2)
//     A-B same-left d=2: W_S+=(3+3)×2=12; W_G+=3×3×2=18
//     C-D,C-E,D-E same-right d=2: W_S+=(2+2)×2=8 each→24; W_G+=2×2×2=8 each→24
//     6 cross d=1: W_S+=(3+2)×1=5 each→30; W_G+=3×2×1=6 each→36
//     W_S=66; W_G=78; CξE=(3/2+3/2+2/2+2/2+2/2)×10^6=6_000_000
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (2, 1, 2_000_000, 1, 2)
//  4.  Path P₃ = A-B-C                    → (10, 6, 3_000_000, 2, 3)
//  5.  Triangle K₃                        → (12, 12, 6_000_000, 3, 3)
//  6.  Star K_{1,4}                       → (44, 28, 6_000_000, 4, 5)
//  7.  Path P₄ = A-B-C-D                  → (28, 19, 2_666_666, 3, 4)
//  8.  Complete K₄                        → (36, 54, 12_000_000, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check      → (66, 78, 6_000_000, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T9_PLUGIN: PluginId   = PluginId::from_ascii("TOPO_IX9");
const T9_EXEC:   ExecutorId = ExecutorId::from_ascii("t9.exec");

const T9_KEY_A: &str = "t9.alpha";
const T9_KEY_B: &str = "t9.beta";
const T9_KEY_C: &str = "t9.gamma";
const T9_KEY_D: &str = "t9.delta";
const T9_KEY_E: &str = "t9.epsilon";

const T9_ID_A: NodeId = derive_node_id(T9_PLUGIN, T9_KEY_A);
const T9_ID_B: NodeId = derive_node_id(T9_PLUGIN, T9_KEY_B);
const T9_ID_C: NodeId = derive_node_id(T9_PLUGIN, T9_KEY_C);
const T9_ID_D: NodeId = derive_node_id(T9_PLUGIN, T9_KEY_D);
const T9_ID_E: NodeId = derive_node_id(T9_PLUGIN, T9_KEY_E);

// L4=96 namespace for this harness.
const T9_VEC_A: VectorAddress = VectorAddress::new(96, 1, 1, 0);
const T9_VEC_B: VectorAddress = VectorAddress::new(96, 1, 2, 0);
const T9_VEC_C: VectorAddress = VectorAddress::new(96, 1, 3, 0);
const T9_VEC_D: VectorAddress = VectorAddress::new(96, 2, 1, 0);
const T9_VEC_E: VectorAddress = VectorAddress::new(96, 2, 2, 0);

const T9_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T9_PLUGIN,
    name:         "kl-graph-topo9-harness",
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
        executor_id:       T9_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T9_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T9_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
// No nodes. All values are 0.

#[test]
fn test_01_empty() {
    let _g = setup();

    let (ws, wg, cxe, ec, nc) = gos_runtime::graph_topo_indices9();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(ws,  0, "empty: W_S=0");
    assert_eq!(wg,  0, "empty: W_G=0");
    assert_eq!(cxe, 0, "empty: CxiE=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// deg=0, ecc=0. All indices are 0.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T9_VEC_A, T9_KEY_A, T9_ID_A);

    let (ws, wg, cxe, ec, nc) = gos_runtime::graph_topo_indices9();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(ws,  0, "single: W_S=0");
    assert_eq!(wg,  0, "single: W_G=0");
    assert_eq!(cxe, 0, "single: CxiE=0 (ecc=0)");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected A-B: deg=[1,1], ecc=[1,1].
//
// W_S  = (1+1)×1 = 2
// W_G  = 1×1×1   = 1
// CξE  = (1/1 + 1/1) × 10^6 = 2_000_000

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T9_VEC_A, T9_KEY_A, T9_ID_A);
    add_node(T9_VEC_B, T9_KEY_B, T9_ID_B);
    add_edge(T9_ID_A, T9_ID_B, "ab");

    let (ws, wg, cxe, ec, nc) = gos_runtime::graph_topo_indices9();
    assert_eq!(nc,  2,         "edge: node_count=2");
    assert_eq!(ec,  1,         "edge: edge_count=1");
    assert_eq!(ws,  2,         "edge: W_S=2");
    assert_eq!(wg,  1,         "edge: W_G=1");
    assert_eq!(cxe, 2_000_000, "edge: CxiE=2.000");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// deg=[1,2,1], ecc=[2,1,2].
//
// W_S  = (1+2)×1 + (2+1)×1 + (1+1)×2 = 3+3+4 = 10
// W_G  = 1×2×1  + 2×1×1  + 1×1×2   = 2+2+2  = 6
// CξE  = (1/2 + 2/1 + 1/2) × 10^6 = 3_000_000

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T9_VEC_A, T9_KEY_A, T9_ID_A);
    add_node(T9_VEC_B, T9_KEY_B, T9_ID_B);
    add_node(T9_VEC_C, T9_KEY_C, T9_ID_C);
    add_edge(T9_ID_A, T9_ID_B, "ab");
    add_edge(T9_ID_B, T9_ID_C, "bc");

    let (ws, wg, cxe, ec, nc) = gos_runtime::graph_topo_indices9();
    assert_eq!(nc,  3,         "P3: node_count=3");
    assert_eq!(ec,  2,         "P3: edge_count=2");
    assert_eq!(ws,  10,        "P3: W_S=10");
    assert_eq!(wg,  6,         "P3: W_G=6");
    assert_eq!(cxe, 3_000_000, "P3: CxiE=3.000");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// All deg=2, all ecc=1. 3 pairs, each d=1.
//
// W_S  = 3 × (2+2)×1 = 12
// W_G  = 3 × 2×2×1   = 12
// CξE  = 3 × 2/1 × 10^6 = 6_000_000

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T9_VEC_A, T9_KEY_A, T9_ID_A);
    add_node(T9_VEC_B, T9_KEY_B, T9_ID_B);
    add_node(T9_VEC_C, T9_KEY_C, T9_ID_C);
    add_edge(T9_ID_A, T9_ID_B, "ab");
    add_edge(T9_ID_B, T9_ID_C, "bc");
    add_edge(T9_ID_A, T9_ID_C, "ac");

    let (ws, wg, cxe, ec, nc) = gos_runtime::graph_topo_indices9();
    assert_eq!(nc,  3,         "K3: node_count=3");
    assert_eq!(ec,  3,         "K3: edge_count=3");
    assert_eq!(ws,  12,        "K3: W_S=12");
    assert_eq!(wg,  12,        "K3: W_G=12");
    assert_eq!(cxe, 6_000_000, "K3: CxiE=6.000 (all deg=2, ecc=1)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Center=A (deg=4, ecc=1); leaves=B,C,D,E (deg=1, ecc=2 each).
//
// 4 center-leaf pairs (d=1): W_S += (4+1)×1=5 each → 20; W_G += 4×1×1=4 each → 16
// 6 leaf-leaf pairs (d=2):  W_S += (1+1)×2=4 each → 24; W_G += 1×1×2=2 each → 12
// W_S  = 20+24 = 44
// W_G  = 16+12 = 28
// CξE  = 4/1×10^6 + 4×(1/2×10^6) = 4_000_000 + 2_000_000 = 6_000_000

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T9_VEC_A, T9_KEY_A, T9_ID_A);
    add_node(T9_VEC_B, T9_KEY_B, T9_ID_B);
    add_node(T9_VEC_C, T9_KEY_C, T9_ID_C);
    add_node(T9_VEC_D, T9_KEY_D, T9_ID_D);
    add_node(T9_VEC_E, T9_KEY_E, T9_ID_E);
    add_edge(T9_ID_A, T9_ID_B, "ab");
    add_edge(T9_ID_A, T9_ID_C, "ac");
    add_edge(T9_ID_A, T9_ID_D, "ad");
    add_edge(T9_ID_A, T9_ID_E, "ae");

    let (ws, wg, cxe, ec, nc) = gos_runtime::graph_topo_indices9();
    assert_eq!(nc,  5,         "K14: node_count=5");
    assert_eq!(ec,  4,         "K14: edge_count=4");
    assert_eq!(ws,  44,        "K14: W_S=44");
    assert_eq!(wg,  28,        "K14: W_G=28");
    assert_eq!(cxe, 6_000_000, "K14: CxiE=6.000");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// deg=[1,2,2,1], ecc=[3,2,2,3]. 6 pairs.
//
// A-B d=1: W_S+=(1+2)=3;   W_G+=1×2=2
// A-C d=2: W_S+=(1+2)×2=6; W_G+=1×2×2=4
// A-D d=3: W_S+=(1+1)×3=6; W_G+=1×1×3=3
// B-C d=1: W_S+=(2+2)=4;   W_G+=2×2=4
// B-D d=2: W_S+=(2+1)×2=6; W_G+=2×1×2=4
// C-D d=1: W_S+=(2+1)=3;   W_G+=2×1=2
// W_S  = 3+6+6+4+6+3 = 28
// W_G  = 2+4+3+4+4+2 = 19
// CξE  = floor(1/3×10^6)×2 + 1×10^6×2 = 333_333×2 + 2_000_000 = 2_666_666

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T9_VEC_A, T9_KEY_A, T9_ID_A);
    add_node(T9_VEC_B, T9_KEY_B, T9_ID_B);
    add_node(T9_VEC_C, T9_KEY_C, T9_ID_C);
    add_node(T9_VEC_D, T9_KEY_D, T9_ID_D);
    add_edge(T9_ID_A, T9_ID_B, "ab");
    add_edge(T9_ID_B, T9_ID_C, "bc");
    add_edge(T9_ID_C, T9_ID_D, "cd");

    let (ws, wg, cxe, ec, nc) = gos_runtime::graph_topo_indices9();
    assert_eq!(nc,  4,         "P4: node_count=4");
    assert_eq!(ec,  3,         "P4: edge_count=3");
    assert_eq!(ws,  28,        "P4: W_S=28");
    assert_eq!(wg,  19,        "P4: W_G=19");
    assert_eq!(cxe, 2_666_666, "P4: CxiE=2.666 (floor 1/3 per endpoint)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// All 4 nodes connected to each other: all d=1, deg=3, ecc=1 (self-centered).
//
// 6 pairs each d=1:
// W_S  = 6 × (3+3)×1 = 36
// W_G  = 6 × 3×3×1   = 54
// CξE  = 4 × 3/1 × 10^6 = 12_000_000

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T9_VEC_A, T9_KEY_A, T9_ID_A);
    add_node(T9_VEC_B, T9_KEY_B, T9_ID_B);
    add_node(T9_VEC_C, T9_KEY_C, T9_ID_C);
    add_node(T9_VEC_D, T9_KEY_D, T9_ID_D);
    add_edge(T9_ID_A, T9_ID_B, "ab");
    add_edge(T9_ID_A, T9_ID_C, "ac");
    add_edge(T9_ID_A, T9_ID_D, "ad");
    add_edge(T9_ID_B, T9_ID_C, "bc");
    add_edge(T9_ID_B, T9_ID_D, "bd");
    add_edge(T9_ID_C, T9_ID_D, "cd");

    let (ws, wg, cxe, ec, nc) = gos_runtime::graph_topo_indices9();
    assert_eq!(nc,  4,          "K4: node_count=4");
    assert_eq!(ec,  6,          "K4: edge_count=6");
    assert_eq!(ws,  36,         "K4: W_S=36=6×6");
    assert_eq!(wg,  54,         "K4: W_G=54=6×9");
    assert_eq!(cxe, 12_000_000, "K4: CxiE=12.000 (all deg=3, ecc=1)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// A and B with no edges: deg=0, ecc=0. All indices are 0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T9_VEC_A, T9_KEY_A, T9_ID_A);
    add_node(T9_VEC_B, T9_KEY_B, T9_ID_B);

    let (ws, wg, cxe, ec, nc) = gos_runtime::graph_topo_indices9();
    assert_eq!(nc,  2, "isolated: node_count=2");
    assert_eq!(ec,  0, "isolated: edge_count=0");
    assert_eq!(ws,  0, "isolated: W_S=0");
    assert_eq!(wg,  0, "isolated: W_G=0");
    assert_eq!(cxe, 0, "isolated: CxiE=0 (all disconnected)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Sides {A,B} and {C,D,E} with all 6 cross-edges.
// left(A,B) deg=3; right(C,D,E) deg=2. All ecc=2 (self-centered bipartite).
//
// Distances: cross-pairs d=1; same-side pairs d=2.
//
// A-B   d=2: W_S+=(3+3)×2=12; W_G+=3×3×2=18
// C-D   d=2: W_S+=(2+2)×2=8;  W_G+=2×2×2=8
// C-E   d=2: W_S+=(2+2)×2=8;  W_G+=2×2×2=8
// D-E   d=2: W_S+=(2+2)×2=8;  W_G+=2×2×2=8
// A-C   d=1: W_S+=(3+2)=5;    W_G+=3×2=6
// A-D   d=1: W_S+=(3+2)=5;    W_G+=3×2=6
// A-E   d=1: W_S+=(3+2)=5;    W_G+=3×2=6
// B-C   d=1: W_S+=(3+2)=5;    W_G+=3×2=6
// B-D   d=1: W_S+=(3+2)=5;    W_G+=3×2=6
// B-E   d=1: W_S+=(3+2)=5;    W_G+=3×2=6
// W_S = 12+8+8+8+5+5+5+5+5+5 = 66
// W_G = 18+8+8+8+6+6+6+6+6+6 = 78
// CξE = (floor(3/2)+floor(3/2)+floor(2/2)+floor(2/2)+floor(2/2)) × 10^6
//     = (1_500_000×2 + 1_000_000×3) = 6_000_000

#[test]
fn test_10_k23_cross_check() {
    let _g = setup();
    add_node(T9_VEC_A, T9_KEY_A, T9_ID_A);
    add_node(T9_VEC_B, T9_KEY_B, T9_ID_B);
    add_node(T9_VEC_C, T9_KEY_C, T9_ID_C);
    add_node(T9_VEC_D, T9_KEY_D, T9_ID_D);
    add_node(T9_VEC_E, T9_KEY_E, T9_ID_E);
    add_edge(T9_ID_A, T9_ID_C, "ac");
    add_edge(T9_ID_A, T9_ID_D, "ad");
    add_edge(T9_ID_A, T9_ID_E, "ae");
    add_edge(T9_ID_B, T9_ID_C, "bc");
    add_edge(T9_ID_B, T9_ID_D, "bd");
    add_edge(T9_ID_B, T9_ID_E, "be");

    let (ws, wg, cxe, ec, nc) = gos_runtime::graph_topo_indices9();
    assert_eq!(nc,  5,         "K23: node_count=5");
    assert_eq!(ec,  6,         "K23: edge_count=6");
    assert_eq!(ws,  66,        "K23: W_S=66");
    assert_eq!(wg,  78,        "K23: W_G=78");
    assert_eq!(cxe, 6_000_000, "K23: CxiE=6.000 (all ecc=2)");
}
