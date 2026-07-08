// gos-graph-topo19-harness — V3.30 Reverse Wiener Λ + RCW + Terminal Wiener TW
//
// Verifies `gos_runtime::graph_topo_indices19()`:
//   Returns (rw, rcw_ppm, tw, edge_count, node_count)
//   - rw      = Λ(G) = Σ_c [C(n_c,2)×D_c − W_c]                      (exact u64)
//   - rcw_ppm = RCW(G) × 10^6 = Σ_{u<v,conn} floor(10^6/(D_c+1−d))   (floor ppm)
//   - tw      = TW(G) = Σ_{u<v, both pendant (deg=1)} d(u,v)           (exact u64)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// DEFINITIONS:
//   W_c   = Σ_{u<v in c} d(u,v)      Wiener index of component c
//   D_c   = max_{u,v in c} d(u,v)    diameter of component c
//   Λ(G)  = Σ_c [C(n_c,2)×D_c − W_c]  Reverse Wiener (Randić et al. 2000)
//   RCW   = Σ_{u<v,conn} 1/(D_c+1−d(u,v))  Reciprocal Complementary Wiener
//   TW    = Σ_{u<v, both deg=1} d(u,v)      Terminal Wiener (Gutman et al. 2004)
//
// KEY INVARIANTS:
//   Λ(G) = 0 iff all components have D_c=1 (complete blocks) or are singletons.
//   RCW: for K_n (D=1), each pair contributes floor(10^6/1)=10^6 → RCW=C(n,2)×10^6.
//   TW = 0 iff fewer than 2 nodes with deg=1 in the graph.
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph     Λ     RCW(ppm)      TW    edges  nodes   W   D   pendant
//  Empty      0          0        0      0      0
//  1 node     0          0        0      0      1
//  Edge A-B   0  1_000_000        1      1      2     1   1   A,B
//  Path P₃    2  2_000_000        2      2      3     4   2   A,C
//  K₃         0  3_000_000        0      3      3     3   1   none
//  K_{1,4}    4  8_000_000       12      4      5    16   2   B,C,D,E
//  Path P₄    8  2_999_999        3      3      4    10   3   A,D
//  K₄         0  6_000_000        0      6      4     6   1   none
//  2 isolated 0          0        0      0      2
//  K_{2,3}    6  7_000_000        0      6      5    14   2   none
//
// Derivations:
//
//   Edge A-B (n=2, m=1, D=1, W=1):
//     Λ = C(2,2)×1−1 = 1×1−1 = 0. ✓
//     RCW: D=1; (A,B): floor(10^6/(1+1−1))=10^6/1=1_000_000. ✓
//     TW: A,B both pendant (deg=1); d=1; TW=1. ✓
//
//   Path P₃ = A−B−C (d_A=d_C=1, d_B=2):
//     D=2, W=1+1+2=4. Λ = 3×2−4=2. ✓
//     RCW: D=2. {A,B}:d=1→10^6/2=500_000; {B,C}:d=1→500_000;
//          {A,C}:d=2→10^6/(2+1−2)=10^6/1=1_000_000. RCW=2_000_000. ✓
//     TW: A,C pendant; d(A,C)=2; TW=2. ✓
//
//   Triangle K₃ (n=3, D=1, W=3):
//     Λ = 3×1−3 = 0. ✓ RCW=3×10^6/1=3_000_000. ✓ TW=0 (no deg-1 nodes). ✓
//
//   Star K_{1,4} (center A:d=4, leaves B−E:d=1; n=5):
//     W = 4×1 + C(4,2)×2 = 4+12 = 16. D=2. Λ=C(5,2)×2−16=10×2−16=4. ✓
//     RCW: D=2. 4 center-leaf (d=1): 4×floor(10^6/2)=4×500_000=2_000_000.
//          6 leaf-leaf (d=2): 6×floor(10^6/1)=6_000_000. RCW=8_000_000. ✓
//     TW: leaves B,C,D,E are pendant. C(4,2)=6 pairs × d=2=12. TW=12. ✓
//
//   Path P₄ = A−B−C−D (d_A=d_D=1, d_B=d_C=2):
//     W=1+1+1+2+2+3=10. D=3. Λ=6×3−10=8. ✓
//     RCW: D=3. {A,B}:d=1→333_333; {B,C}:d=1→333_333; {C,D}:d=1→333_333;
//          {A,C}:d=2→500_000; {B,D}:d=2→500_000; {A,D}:d=3→1_000_000.
//          RCW=999_999+1_000_000+1_000_000=2_999_999. ✓
//     TW: A,D pendant; d(A,D)=3; TW=3. ✓
//
//   Complete K₄ (n=4, D=1, W=6): Λ=6×1−6=0. RCW=6_000_000. TW=0. ✓
//
//   K_{2,3} (left={A,B}:d=3, right={C,D,E}:d=2; n=5):
//     W: 6 L-R × d=1 + 1 L-L × d=2 + 3 R-R × d=2 = 6+2+6=14. D=2.
//     Λ=10×2−14=6. ✓
//     RCW: D=2. 6 L-R(d=1):3_000_000; 1 L-L(d=2):1_000_000; 3 R-R(d=2):3_000_000.
//          RCW=7_000_000. ✓
//     TW: no pendant nodes (L:deg=3, R:deg=2); TW=0. ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (0, 1_000_000, 1, 1, 2)
//  4.  Path P₃ = A-B-C                   → (2, 2_000_000, 2, 2, 3)
//  5.  Triangle K₃                       → (0, 3_000_000, 0, 3, 3)
//  6.  Star K_{1,4}                      → (4, 8_000_000, 12, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (8, 2_999_999, 3, 3, 4)
//  8.  Complete K₄                       → (0, 6_000_000, 0, 6, 4)
//  9.  Two isolated nodes                → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (6, 7_000_000, 0, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T19_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_19");
const T19_EXEC:   ExecutorId = ExecutorId::from_ascii("t19.exec");

const T19_KEY_A: &str = "t19.alpha";
const T19_KEY_B: &str = "t19.beta";
const T19_KEY_C: &str = "t19.gamma";
const T19_KEY_D: &str = "t19.delta";
const T19_KEY_E: &str = "t19.epsilon";

const T19_ID_A: NodeId = derive_node_id(T19_PLUGIN, T19_KEY_A);
const T19_ID_B: NodeId = derive_node_id(T19_PLUGIN, T19_KEY_B);
const T19_ID_C: NodeId = derive_node_id(T19_PLUGIN, T19_KEY_C);
const T19_ID_D: NodeId = derive_node_id(T19_PLUGIN, T19_KEY_D);
const T19_ID_E: NodeId = derive_node_id(T19_PLUGIN, T19_KEY_E);

// L4=106 namespace for this harness.
const T19_VEC_A: VectorAddress = VectorAddress::new(106, 1, 1, 0);
const T19_VEC_B: VectorAddress = VectorAddress::new(106, 1, 2, 0);
const T19_VEC_C: VectorAddress = VectorAddress::new(106, 1, 3, 0);
const T19_VEC_D: VectorAddress = VectorAddress::new(106, 2, 1, 0);
const T19_VEC_E: VectorAddress = VectorAddress::new(106, 2, 2, 0);

const T19_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T19_PLUGIN,
    name:         "kl-graph-topo19-harness",
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
        executor_id:       T19_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T19_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T19_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (rw, rcw, tw, ec, nc) = gos_runtime::graph_topo_indices19();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(rw,  0, "empty: \u{039b}=0");
    assert_eq!(rcw, 0, "empty: RCW=0");
    assert_eq!(tw,  0, "empty: TW=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T19_VEC_A, T19_KEY_A, T19_ID_A);

    let (rw, rcw, tw, ec, nc) = gos_runtime::graph_topo_indices19();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(rw,  0, "single: \u{039b}=0 (no pairs)");
    assert_eq!(rcw, 0, "single: RCW=0 (no pairs)");
    assert_eq!(tw,  0, "single: TW=0 (no pairs)");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// n=2, D=1, W=1. Λ=C(2,2)×1−1=0. Both pendant.
// RCW: floor(10^6/(1+1−1))=10^6. TW=d(A,B)=1.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T19_VEC_A, T19_KEY_A, T19_ID_A);
    add_node(T19_VEC_B, T19_KEY_B, T19_ID_B);
    add_edge(T19_ID_A, T19_ID_B, "t19.e.ab");

    let (rw, rcw, tw, ec, nc) = gos_runtime::graph_topo_indices19();
    assert_eq!(nc,  2,           "edge: node_count=2");
    assert_eq!(ec,  1,           "edge: edge_count=1");
    assert_eq!(rw,  0,           "edge: \u{039b}=0 (C(2,2)\u{00d7}1−1=0)");
    assert_eq!(rcw, 1_000_000,   "edge: RCW=1_000_000 (D=1; floor(10^6/1)=1_000_000)");
    assert_eq!(tw,  1,           "edge: TW=1 (both pendant, d=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// D=2, W=4. Λ=3×2−4=2. A,C pendant.
// RCW: {A,B}:500_000; {B,C}:500_000; {A,C}:1_000_000. Sum=2_000_000.
// TW: d(A,C)=2 → TW=2.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T19_VEC_A, T19_KEY_A, T19_ID_A);
    add_node(T19_VEC_B, T19_KEY_B, T19_ID_B);
    add_node(T19_VEC_C, T19_KEY_C, T19_ID_C);
    add_edge(T19_ID_A, T19_ID_B, "t19.e.ab");
    add_edge(T19_ID_B, T19_ID_C, "t19.e.bc");

    let (rw, rcw, tw, ec, nc) = gos_runtime::graph_topo_indices19();
    assert_eq!(nc,  3,         "p3: node_count=3");
    assert_eq!(ec,  2,         "p3: edge_count=2");
    assert_eq!(rw,  2,         "p3: \u{039b}=2 (3\u{00d7}2−4=2)");
    assert_eq!(rcw, 2_000_000, "p3: RCW=2_000_000 (500k+500k+1M)");
    assert_eq!(tw,  2,         "p3: TW=2 (A,C pendant; d(A,C)=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// D=1, W=3. Λ=3×1−3=0. No pendant nodes (all deg=2).
// RCW: 3 pairs × floor(10^6/1) = 3_000_000.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T19_VEC_A, T19_KEY_A, T19_ID_A);
    add_node(T19_VEC_B, T19_KEY_B, T19_ID_B);
    add_node(T19_VEC_C, T19_KEY_C, T19_ID_C);
    add_edge(T19_ID_A, T19_ID_B, "t19.e.ab");
    add_edge(T19_ID_B, T19_ID_A, "t19.e.ba");
    add_edge(T19_ID_B, T19_ID_C, "t19.e.bc");
    add_edge(T19_ID_C, T19_ID_B, "t19.e.cb");
    add_edge(T19_ID_A, T19_ID_C, "t19.e.ac");
    add_edge(T19_ID_C, T19_ID_A, "t19.e.ca");

    let (rw, rcw, tw, ec, nc) = gos_runtime::graph_topo_indices19();
    assert_eq!(nc,  3,         "k3: node_count=3");
    assert_eq!(ec,  3,         "k3: edge_count=3");
    assert_eq!(rw,  0,         "k3: \u{039b}=0 (\u{039b}=0: D=1=complete)");
    assert_eq!(rcw, 3_000_000, "k3: RCW=3_000_000 (3 pairs \u{00d7} 10^6/1)");
    assert_eq!(tw,  0,         "k3: TW=0 (no pendant nodes)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// center A (deg=4), leaves B,C,D,E (deg=1). n=5, D=2.
// W = 4×1 + 6×2 = 16. Λ = 10×2−16 = 4.
// RCW: D=2. 4 center-leaf (d=1): 4×500_000=2_000_000.
//           6 leaf-leaf   (d=2): 6×1_000_000=6_000_000. RCW=8_000_000.
// TW: C(4,2)=6 pendant pairs × d=2 = 12.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T19_VEC_A, T19_KEY_A, T19_ID_A);
    add_node(T19_VEC_B, T19_KEY_B, T19_ID_B);
    add_node(T19_VEC_C, T19_KEY_C, T19_ID_C);
    add_node(T19_VEC_D, T19_KEY_D, T19_ID_D);
    add_node(T19_VEC_E, T19_KEY_E, T19_ID_E);
    add_edge(T19_ID_A, T19_ID_B, "t19.e.ab");
    add_edge(T19_ID_A, T19_ID_C, "t19.e.ac");
    add_edge(T19_ID_A, T19_ID_D, "t19.e.ad");
    add_edge(T19_ID_A, T19_ID_E, "t19.e.ae");

    let (rw, rcw, tw, ec, nc) = gos_runtime::graph_topo_indices19();
    assert_eq!(nc,  5,         "star: node_count=5");
    assert_eq!(ec,  4,         "star: edge_count=4");
    assert_eq!(rw,  4,         "star: \u{039b}=4 (10\u{00d7}2−16=4)");
    assert_eq!(rcw, 8_000_000, "star: RCW=8_000_000 (2M center-leaf + 6M leaf-leaf)");
    assert_eq!(tw,  12,        "star: TW=12 (6 pendant pairs \u{00d7} d=2)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d_A=d_D=1, d_B=d_C=2. D=3, W=10. Λ=6×3−10=8.
// RCW: D=3. {A,B}:333_333; {B,C}:333_333; {C,D}:333_333;
//           {A,C}:500_000; {B,D}:500_000; {A,D}:1_000_000.
//           Sum=999_999+1_000_000+1_000_000=2_999_999.
// TW: A,D pendant; d(A,D)=3; TW=3.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T19_VEC_A, T19_KEY_A, T19_ID_A);
    add_node(T19_VEC_B, T19_KEY_B, T19_ID_B);
    add_node(T19_VEC_C, T19_KEY_C, T19_ID_C);
    add_node(T19_VEC_D, T19_KEY_D, T19_ID_D);
    add_edge(T19_ID_A, T19_ID_B, "t19.e.ab");
    add_edge(T19_ID_B, T19_ID_C, "t19.e.bc");
    add_edge(T19_ID_C, T19_ID_D, "t19.e.cd");

    let (rw, rcw, tw, ec, nc) = gos_runtime::graph_topo_indices19();
    assert_eq!(nc,  4,         "p4: node_count=4");
    assert_eq!(ec,  3,         "p4: edge_count=3");
    assert_eq!(rw,  8,         "p4: \u{039b}=8 (6\u{00d7}3−10=8)");
    assert_eq!(rcw, 2_999_999, "p4: RCW=2_999_999 (333333\u{00d7}3+500000\u{00d7}2+1000000)");
    assert_eq!(tw,  3,         "p4: TW=3 (A,D pendant; d(A,D)=3)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// n=4, D=1, W=6. Λ=6×1−6=0. No pendant. RCW=6×10^6=6_000_000. TW=0.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T19_VEC_A, T19_KEY_A, T19_ID_A);
    add_node(T19_VEC_B, T19_KEY_B, T19_ID_B);
    add_node(T19_VEC_C, T19_KEY_C, T19_ID_C);
    add_node(T19_VEC_D, T19_KEY_D, T19_ID_D);
    add_edge(T19_ID_A, T19_ID_B, "t19.e.ab");
    add_edge(T19_ID_B, T19_ID_A, "t19.e.ba");
    add_edge(T19_ID_A, T19_ID_C, "t19.e.ac");
    add_edge(T19_ID_C, T19_ID_A, "t19.e.ca");
    add_edge(T19_ID_A, T19_ID_D, "t19.e.ad");
    add_edge(T19_ID_D, T19_ID_A, "t19.e.da");
    add_edge(T19_ID_B, T19_ID_C, "t19.e.bc");
    add_edge(T19_ID_C, T19_ID_B, "t19.e.cb");
    add_edge(T19_ID_B, T19_ID_D, "t19.e.bd");
    add_edge(T19_ID_D, T19_ID_B, "t19.e.db");
    add_edge(T19_ID_C, T19_ID_D, "t19.e.cd");
    add_edge(T19_ID_D, T19_ID_C, "t19.e.dc");

    let (rw, rcw, tw, ec, nc) = gos_runtime::graph_topo_indices19();
    assert_eq!(nc,  4,         "k4: node_count=4");
    assert_eq!(ec,  6,         "k4: edge_count=6");
    assert_eq!(rw,  0,         "k4: \u{039b}=0 (complete: C(4,2)\u{00d7}1−6=0)");
    assert_eq!(rcw, 6_000_000, "k4: RCW=6_000_000 (6 pairs \u{00d7} 10^6/1)");
    assert_eq!(tw,  0,         "k4: TW=0 (no pendant nodes, all deg=3)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// Two singleton components; no pairs within any component. All 0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T19_VEC_A, T19_KEY_A, T19_ID_A);
    add_node(T19_VEC_B, T19_KEY_B, T19_ID_B);

    let (rw, rcw, tw, ec, nc) = gos_runtime::graph_topo_indices19();
    assert_eq!(nc,  2, "isolated: node_count=2");
    assert_eq!(ec,  0, "isolated: no edges");
    assert_eq!(rw,  0, "isolated: \u{039b}=0 (no pairs in any component)");
    assert_eq!(rcw, 0, "isolated: RCW=0 (no pairs)");
    assert_eq!(tw,  0, "isolated: TW=0 (no reachable pendant pairs)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}:deg=3. Right={C,D,E}:deg=2. n=5, m=6.
// W = 6(L-R,d=1) + 2(L-L,d=2) + 6(R-R,d=2) = 14. D=2.
// Λ = C(5,2)×2−14 = 10×2−14 = 6.
// RCW: D=2. 6 L-R(d=1):3_000_000; 1 L-L(d=2):1_000_000; 3 R-R(d=2):3_000_000.
//      RCW=7_000_000.
// TW=0: deg(A)=deg(B)=3, deg(C)=deg(D)=deg(E)=2 → no pendant nodes.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T19_VEC_A, T19_KEY_A, T19_ID_A);
    add_node(T19_VEC_B, T19_KEY_B, T19_ID_B);
    add_node(T19_VEC_C, T19_KEY_C, T19_ID_C);
    add_node(T19_VEC_D, T19_KEY_D, T19_ID_D);
    add_node(T19_VEC_E, T19_KEY_E, T19_ID_E);
    // Left A connects to all right
    add_edge(T19_ID_A, T19_ID_C, "t19.e.ac");
    add_edge(T19_ID_C, T19_ID_A, "t19.e.ca");
    add_edge(T19_ID_A, T19_ID_D, "t19.e.ad");
    add_edge(T19_ID_D, T19_ID_A, "t19.e.da");
    add_edge(T19_ID_A, T19_ID_E, "t19.e.ae");
    add_edge(T19_ID_E, T19_ID_A, "t19.e.ea");
    // Left B connects to all right
    add_edge(T19_ID_B, T19_ID_C, "t19.e.bc");
    add_edge(T19_ID_C, T19_ID_B, "t19.e.cb");
    add_edge(T19_ID_B, T19_ID_D, "t19.e.bd");
    add_edge(T19_ID_D, T19_ID_B, "t19.e.db");
    add_edge(T19_ID_B, T19_ID_E, "t19.e.be");
    add_edge(T19_ID_E, T19_ID_B, "t19.e.eb");

    let (rw, rcw, tw, ec, nc) = gos_runtime::graph_topo_indices19();
    assert_eq!(nc,   5,        "k23: node_count=5");
    assert_eq!(ec,   6,        "k23: edge_count=6");
    assert_eq!(rw,   6,        "k23: \u{039b}=6 (10\u{00d7}2−14=6)");
    assert_eq!(rcw,  7_000_000,"k23: RCW=7_000_000 (3M+1M+3M)");
    assert_eq!(tw,   0,        "k23: TW=0 (no pendant nodes)");
}
