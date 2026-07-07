// gos-graph-topo7-harness — V3.18 Wiener W + Harary H + Hyper-Wiener WW distance-based indices
//
// Verifies `gos_runtime::graph_topo_indices7()`:
//   Returns (wiener, harary_ppm, hyper_wiener, edge_count, node_count)
//   - wiener       = W(G)  = Σ_{u<v} d(u,v)                           (exact u64; Wiener 1947)
//   - harary_ppm   = H(G)  = Σ_{u<v} 1/d(u,v) × 10^6                 (floor ppm; Plavšić 1993)
//   - hyper_wiener = WW(G) = (1/2) Σ_{u<v} [d(u,v) + d(u,v)²]        (exact u64; Klein 1993)
//   - edge_count   = undirected non-self-loop edges
//   - node_count   = live node count
//
// Integer precision:
//   W:  exact integer (sum of BFS distances)
//   H:  floor(1/d × 10^6) per connected pair; error ≤ 1 ppm per pair
//   WW: = Σ_{u<v} d*(d+1)/2  (always integer since d*(d+1) is even for all d≥1)
//
// KEY INVARIANTS:
//   W(K_n)  = n*(n-1)/2              (all pairs d=1)
//   H(K_n)  = n*(n-1)/2 × 10^6      (all pairs d=1; H_ppm = W × 10^6)
//   WW(K_n) = n*(n-1)/2              (d=1: WW per-pair=1, same as W for complete graphs)
//   W(P_n)  = n*(n²-1)/6             (standard formula for path graphs)
//   Disconnected pairs: d=∞ → contribute 0 to W, H, WW
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph           W        H_ppm    WW       edge_count  node_count
//  Empty           0            0     0        0           0
//  1 node          0            0     0        0           1
//  Edge A-B        1    1_000_000     1        1           2
//  Path P₃         4    2_500_000     5        2           3
//  Triangle K₃     3    3_000_000     3        3           3
//  Star K_{1,4}   16    7_000_000    22        4           5
//  Path P₄        10    4_333_333    15        3           4
//  Complete K₄     6    6_000_000     6        6           4
//  2 isolated      0            0     0        0           2
//  K_{2,3}        14    8_000_000    18        6           5
//
// Per-pair derivations:
//   d=1: W+=1; H+=1_000_000; WW+=1*(2)/2=1
//   d=2: W+=2; H+=500_000;   WW+=2*(3)/2=3
//   d=3: W+=3; H+=333_333;   WW+=3*(4)/2=6
//
// P₃ (A-B-C): pairs (A,B)=1,(A,C)=2,(B,C)=1 → W=4; H=1_000_000+500_000+1_000_000=2_500_000; WW=1+3+1=5
// K_{1,4} (center=A, leaves B,C,D,E):
//   4 center-leaf pairs d=1: W+=4; H+=4_000_000; WW+=4
//   6 leaf-leaf pairs d=2: W+=12; H+=3_000_000; WW+=18
//   Total: W=16; H=7_000_000; WW=22
// K_{2,3} (sides {A,B},{C,D,E}):
//   1 same-side pair d=2 (A,B): W+=2; H+=500_000; WW+=3
//   6 cross pairs d=1: W+=6; H+=6_000_000; WW+=6
//   3 same-side pairs d=2 (C,D),(C,E),(D,E): W+=6; H+=1_500_000; WW+=9
//   Total: W=14; H=8_000_000; WW=18
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (1, 1_000_000, 1, 1, 2)
//  4.  Path P₃ = A-B-C                    → (4, 2_500_000, 5, 2, 3)
//  5.  Triangle K₃                        → (3, 3_000_000, 3, 3, 3)
//  6.  Star K_{1,4}                       → (16, 7_000_000, 22, 4, 5)
//  7.  Path P₄ = A-B-C-D                  → (10, 4_333_333, 15, 3, 4)
//  8.  Complete K₄                        → (6, 6_000_000, 6, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check      → (14, 8_000_000, 18, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T7_PLUGIN: PluginId   = PluginId::from_ascii("TOPO_IX7");
const T7_EXEC:   ExecutorId = ExecutorId::from_ascii("t7.exec");

const T7_KEY_A: &str = "t7.alpha";
const T7_KEY_B: &str = "t7.beta";
const T7_KEY_C: &str = "t7.gamma";
const T7_KEY_D: &str = "t7.delta";
const T7_KEY_E: &str = "t7.epsilon";

const T7_ID_A: NodeId = derive_node_id(T7_PLUGIN, T7_KEY_A);
const T7_ID_B: NodeId = derive_node_id(T7_PLUGIN, T7_KEY_B);
const T7_ID_C: NodeId = derive_node_id(T7_PLUGIN, T7_KEY_C);
const T7_ID_D: NodeId = derive_node_id(T7_PLUGIN, T7_KEY_D);
const T7_ID_E: NodeId = derive_node_id(T7_PLUGIN, T7_KEY_E);

// L4=94 namespace for this harness.
const T7_VEC_A: VectorAddress = VectorAddress::new(94, 1, 1, 0);
const T7_VEC_B: VectorAddress = VectorAddress::new(94, 1, 2, 0);
const T7_VEC_C: VectorAddress = VectorAddress::new(94, 1, 3, 0);
const T7_VEC_D: VectorAddress = VectorAddress::new(94, 2, 1, 0);
const T7_VEC_E: VectorAddress = VectorAddress::new(94, 2, 2, 0);

const T7_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T7_PLUGIN,
    name:         "kl-graph-topo7-harness",
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
        executor_id:       T7_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T7_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T7_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
// No nodes, no edges. All indices and counts are 0.

#[test]
fn test_01_empty() {
    let _g = setup();

    let (w, h, ww, ec, nc) = gos_runtime::graph_topo_indices7();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(w,   0, "empty: W=0");
    assert_eq!(h,   0, "empty: H=0");
    assert_eq!(ww,  0, "empty: WW=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// One node, no pairs exist. All indices are 0.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T7_VEC_A, T7_KEY_A, T7_ID_A);

    let (w, h, ww, ec, nc) = gos_runtime::graph_topo_indices7();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(w,   0, "single: W=0 (no pairs)");
    assert_eq!(h,   0, "single: H=0 (no pairs)");
    assert_eq!(ww,  0, "single: WW=0 (no pairs)");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected A-B: d(A,B)=1. One connected pair.
//
// W  = 1
// H  = 1/1 × 10^6 = 1_000_000
// WW = 1*(1+1)/2 = 1

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T7_VEC_A, T7_KEY_A, T7_ID_A);
    add_node(T7_VEC_B, T7_KEY_B, T7_ID_B);
    add_edge(T7_ID_A, T7_ID_B, "ab");

    let (w, h, ww, ec, nc) = gos_runtime::graph_topo_indices7();
    assert_eq!(nc,  2,         "edge: node_count=2");
    assert_eq!(ec,  1,         "edge: edge_count=1");
    assert_eq!(w,   1,         "edge: W=1");
    assert_eq!(h,   1_000_000, "edge: H=1.000");
    assert_eq!(ww,  1,         "edge: WW=1");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// Pairs: (A,B)=1, (A,C)=2, (B,C)=1.
//
// W  = 1+2+1 = 4
// H  = 1_000_000 + 500_000 + 1_000_000 = 2_500_000
// WW = 1*(2)/2 + 2*(3)/2 + 1*(2)/2 = 1 + 3 + 1 = 5

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T7_VEC_A, T7_KEY_A, T7_ID_A);
    add_node(T7_VEC_B, T7_KEY_B, T7_ID_B);
    add_node(T7_VEC_C, T7_KEY_C, T7_ID_C);
    add_edge(T7_ID_A, T7_ID_B, "ab");
    add_edge(T7_ID_B, T7_ID_C, "bc");

    let (w, h, ww, ec, nc) = gos_runtime::graph_topo_indices7();
    assert_eq!(nc,  3,         "P3: node_count=3");
    assert_eq!(ec,  2,         "P3: edge_count=2");
    assert_eq!(w,   4,         "P3: W=4");
    assert_eq!(h,   2_500_000, "P3: H=2.500");
    assert_eq!(ww,  5,         "P3: WW=5");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// All 3 pairs have d=1.
//
// W  = 3 = K₃ invariant (n*(n-1)/2 for complete)
// H  = 3_000_000
// WW = 3  (WW = W for complete graphs since d=1 → WW per-pair = 1)

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T7_VEC_A, T7_KEY_A, T7_ID_A);
    add_node(T7_VEC_B, T7_KEY_B, T7_ID_B);
    add_node(T7_VEC_C, T7_KEY_C, T7_ID_C);
    add_edge(T7_ID_A, T7_ID_B, "ab");
    add_edge(T7_ID_B, T7_ID_C, "bc");
    add_edge(T7_ID_A, T7_ID_C, "ac");

    let (w, h, ww, ec, nc) = gos_runtime::graph_topo_indices7();
    assert_eq!(nc,  3,         "K3: node_count=3");
    assert_eq!(ec,  3,         "K3: edge_count=3");
    assert_eq!(w,   3,         "K3: W=3=n(n-1)/2");
    assert_eq!(h,   3_000_000, "K3: H=3.000 (all d=1)");
    assert_eq!(ww,  3,         "K3: WW=3 (=W for complete)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Center=A, leaves=B,C,D,E.
// 4 center-leaf pairs d=1; 6 leaf-leaf pairs d=2.
//
// W  = 4*1 + 6*2 = 16
// H  = 4*1_000_000 + 6*500_000 = 4_000_000 + 3_000_000 = 7_000_000
// WW = 4*1*(2)/2 + 6*2*(3)/2 = 4*1 + 6*3 = 4 + 18 = 22

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T7_VEC_A, T7_KEY_A, T7_ID_A);
    add_node(T7_VEC_B, T7_KEY_B, T7_ID_B);
    add_node(T7_VEC_C, T7_KEY_C, T7_ID_C);
    add_node(T7_VEC_D, T7_KEY_D, T7_ID_D);
    add_node(T7_VEC_E, T7_KEY_E, T7_ID_E);
    add_edge(T7_ID_A, T7_ID_B, "ab");
    add_edge(T7_ID_A, T7_ID_C, "ac");
    add_edge(T7_ID_A, T7_ID_D, "ad");
    add_edge(T7_ID_A, T7_ID_E, "ae");

    let (w, h, ww, ec, nc) = gos_runtime::graph_topo_indices7();
    assert_eq!(nc,  5,         "K14: node_count=5");
    assert_eq!(ec,  4,         "K14: edge_count=4");
    assert_eq!(w,   16,        "K14: W=16");
    assert_eq!(h,   7_000_000, "K14: H=7.000");
    assert_eq!(ww,  22,        "K14: WW=22");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// Pairs: (A,B)=1,(A,C)=2,(A,D)=3,(B,C)=1,(B,D)=2,(C,D)=1
//
// W  = 1+2+3+1+2+1 = 10  (formula: n*(n²-1)/6 = 4*15/6 = 10)
// H  = 1_000_000+500_000+333_333+1_000_000+500_000+1_000_000 = 4_333_333
// WW = 1+3+6+1+3+1 = 15

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T7_VEC_A, T7_KEY_A, T7_ID_A);
    add_node(T7_VEC_B, T7_KEY_B, T7_ID_B);
    add_node(T7_VEC_C, T7_KEY_C, T7_ID_C);
    add_node(T7_VEC_D, T7_KEY_D, T7_ID_D);
    add_edge(T7_ID_A, T7_ID_B, "ab");
    add_edge(T7_ID_B, T7_ID_C, "bc");
    add_edge(T7_ID_C, T7_ID_D, "cd");

    let (w, h, ww, ec, nc) = gos_runtime::graph_topo_indices7();
    assert_eq!(nc,  4,         "P4: node_count=4");
    assert_eq!(ec,  3,         "P4: edge_count=3");
    assert_eq!(w,   10,        "P4: W=10=n(n²-1)/6");
    assert_eq!(h,   4_333_333, "P4: H=4.333");
    assert_eq!(ww,  15,        "P4: WW=15");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// All 6 pairs have d=1.
//
// W  = 6 = K₄ invariant n*(n-1)/2 = 4*3/2
// H  = 6_000_000  (= W × 10^6 for complete graphs)
// WW = 6  (WW = W for complete graphs)

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T7_VEC_A, T7_KEY_A, T7_ID_A);
    add_node(T7_VEC_B, T7_KEY_B, T7_ID_B);
    add_node(T7_VEC_C, T7_KEY_C, T7_ID_C);
    add_node(T7_VEC_D, T7_KEY_D, T7_ID_D);
    add_edge(T7_ID_A, T7_ID_B, "ab");
    add_edge(T7_ID_A, T7_ID_C, "ac");
    add_edge(T7_ID_A, T7_ID_D, "ad");
    add_edge(T7_ID_B, T7_ID_C, "bc");
    add_edge(T7_ID_B, T7_ID_D, "bd");
    add_edge(T7_ID_C, T7_ID_D, "cd");

    let (w, h, ww, ec, nc) = gos_runtime::graph_topo_indices7();
    assert_eq!(nc,  4,         "K4: node_count=4");
    assert_eq!(ec,  6,         "K4: edge_count=6");
    assert_eq!(w,   6,         "K4: W=6=n(n-1)/2");
    assert_eq!(h,   6_000_000, "K4: H=6.000 (=W×10^6)");
    assert_eq!(ww,  6,         "K4: WW=6 (=W for complete)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// A and B with no edges. The pair (A,B) is disconnected: d=∞ → contributes 0.
//
// W = H = WW = 0  (no connected pairs)

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T7_VEC_A, T7_KEY_A, T7_ID_A);
    add_node(T7_VEC_B, T7_KEY_B, T7_ID_B);

    let (w, h, ww, ec, nc) = gos_runtime::graph_topo_indices7();
    assert_eq!(nc,  2, "isolated: node_count=2");
    assert_eq!(ec,  0, "isolated: edge_count=0");
    assert_eq!(w,   0, "isolated: W=0 (disconnected)");
    assert_eq!(h,   0, "isolated: H=0 (disconnected)");
    assert_eq!(ww,  0, "isolated: WW=0 (disconnected)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Sides {A,B} and {C,D,E} with all 6 cross-edges.
// d(A,B)=2; d(A,C)=d(A,D)=d(A,E)=d(B,C)=d(B,D)=d(B,E)=1; d(C,D)=d(C,E)=d(D,E)=2
//
// W  = 1*(2) + 6*(1) + 3*(2) = 2+6+6 = 14
// H  = 1*(500_000) + 6*(1_000_000) + 3*(500_000) = 500_000+6_000_000+1_500_000 = 8_000_000
// WW = 1*3 + 6*1 + 3*3 = 3+6+9 = 18
//
// Cross-invariant: W(K_{2,3}) = 14 (well-known result for K_{m,n}: m*n*1 + [m*(m-1)/2+n*(n-1)/2]*2)
//   = 2*3*1 + [1+3]*2 = 6+8 = 14 ✓

#[test]
fn test_10_k23_cross_check() {
    let _g = setup();
    // Side A: A, B  Side B: C, D, E
    add_node(T7_VEC_A, T7_KEY_A, T7_ID_A);
    add_node(T7_VEC_B, T7_KEY_B, T7_ID_B);
    add_node(T7_VEC_C, T7_KEY_C, T7_ID_C);
    add_node(T7_VEC_D, T7_KEY_D, T7_ID_D);
    add_node(T7_VEC_E, T7_KEY_E, T7_ID_E);
    // All 6 cross-edges
    add_edge(T7_ID_A, T7_ID_C, "ac");
    add_edge(T7_ID_A, T7_ID_D, "ad");
    add_edge(T7_ID_A, T7_ID_E, "ae");
    add_edge(T7_ID_B, T7_ID_C, "bc");
    add_edge(T7_ID_B, T7_ID_D, "bd");
    add_edge(T7_ID_B, T7_ID_E, "be");

    let (w, h, ww, ec, nc) = gos_runtime::graph_topo_indices7();
    assert_eq!(nc,  5,         "K23: node_count=5");
    assert_eq!(ec,  6,         "K23: edge_count=6");
    assert_eq!(w,   14,        "K23: W=14");
    assert_eq!(h,   8_000_000, "K23: H=8.000");
    assert_eq!(ww,  18,        "K23: WW=18");
}
