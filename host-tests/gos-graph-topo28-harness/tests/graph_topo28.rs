// gos-graph-topo28-harness — V3.39 NNI + NNMI + NSM1 (Neighborhood Nirmala S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices28()`:
//   Returns (nni_ppm, nnmi_ppm, nsm1, edge_count, node_count)
//   - nni_ppm  = NNI(G) × 10^6  = Σ_{uv∈E} isqrt64((S_u+S_v)×10^12)              (floor ppm; S-Nirmala)
//   - nnmi_ppm = NNMI(G) × 10^6 = Σ_{uv∈E} (S_u+S_v)×isqrt64((S_u+S_v)×10^12)   (floor ppm; S-Modified Nirmala)
//   - nsm1     = NSM1(G)         = Σ_{uv∈E} (S_u+S_v)                             (exact u64; S-edge M₁)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NNI(G)  = Σ_{uv∈E} √(S_u+S_v)              (S-analogue of Nirmala N; Nirmala et al. 2021)
//   NNMI(G) = Σ_{uv∈E} (S_u+S_v)^{3/2}         (S-analogue of Modified Nirmala N*; Kumar et al. 2022)
//   NSM1(G) = Σ_{uv∈E} (S_u+S_v)               (S-analogue of M₁ edge form = Σ_v S(v)·deg(v))
//
// IMPLEMENTATION FORMULAS (no float, no_std safe):
//   NNI  per edge  = isqrt64((S_u+S_v) × 10^12)                    [u64; ssum×10^12 ≤ 3.23×10^16 < u64::MAX]
//   NNMI per edge  = (S_u+S_v) × isqrt64((S_u+S_v) × 10^12)       [reuses NNI per-edge; integer factor out]
//   NSM1 per edge  = S_u + S_v                                      [exact; no sqrt]
//
// NNMI IDENTITY: floor((S_u+S_v)^{3/2} × 10^6) = (S_u+S_v) × floor(√(S_u+S_v) × 10^6)
//   because (S_u+S_v) ∈ ℤ ⟹ it factors out of the floor without error.
//
// KEY INVARIANTS:
//   NNI  = |E|·√(2S)·10^6 for S-regular (all S equal).
//   NNMI = |E|·(2S)·√(2S)·10^6 for S-regular (= |E|·(2S)^{3/2}·10^6).
//   NSM1 = 2|E|·S for S-regular.
//   K₃ and K_{1,4}: both S-uniform S=4 → same per-edge NNI and NNMI values (S-regular coincidence).
//   NNI  = NSC (Neighborhood Sum Connectivity from topo22) is FALSE: NSC=Σ 1/√(S_u+S_v) ≠ NNI.
//
// S VALUES PER GRAPH:
//   K₂        : S(A)=S(B)=1
//   P₃=A-B-C  : S(A)=S(B)=S(C)=2        → S-uniform S=2
//   K₃        : S(each)=4               → S-uniform S=4
//   K_{1,4}   : S(hub)=4, S(leaf)=4     → S-uniform S=4
//   P₄=A-B-C-D: S(A)=S(D)=2, S(B)=S(C)=3 → mixed S values
//   K₄        : S(each)=9               → S-uniform S=9
//   K_{2,3}   : S(all)=6                → S-uniform S=6
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph        NNI(ppm)     NNMI(ppm)    NSM1   edges  nodes
//  Empty                0            0       0       0      0
//  1 node               0            0       0       0      1
//  Edge K₂      1_414_213    2_828_426       2       1      2
//  Path P₃      4_000_000   16_000_000       8       2      3
//  Triangle K₃  8_485_281   67_882_248      24       3      3
//  Star K_{1,4} 11_313_708  90_509_664      32       4      5
//  Path P₄      6_921_623   37_057_604      16       3      4
//  Complete K₄ 25_455_840  458_205_120     108       6      4
//  2 isolated           0            0       0       0      2
//  K_{2,3}     20_784_606  249_415_272      72       6      5
//
// DERIVATIONS:
//
//   K₂ (S_A=S_B=1, ssum=2):
//     NNI:  isqrt64(2×10^12)=1_414_213. (√2×10^6=1_414_213.562...; 1_414_213²=1_999_998_477_369<2×10^12) ✓
//     NNMI: 2×1_414_213=2_828_426. ✓
//     NSM1: 1+1=2. ✓
//
//   P₃ (S-uniform S=2, ssum=4, 2 edges):
//     NNI  per edge: isqrt64(4×10^12)=2_000_000 (exact: √4=2). Total: 4_000_000. ✓
//     NNMI per edge: 4×2_000_000=8_000_000. Total: 16_000_000. ✓
//     NSM1: 2×4=8. ✓
//
//   K₃ (S-uniform S=4, ssum=8, 3 edges):
//     NNI  per edge: isqrt64(8×10^12)=2_828_427. (√8×10^6=2√2×10^6=2_828_427.124...)
//       Total: 8_485_281. ✓
//     NNMI per edge: 8×2_828_427=22_627_416. Total: 67_882_248. ✓
//     NSM1: 3×8=24. ✓
//
//   K_{1,4} (S-uniform S=4, ssum=8, 4 edges — same per-edge as K₃):
//     NNI:  4×2_828_427=11_313_708. ✓
//     NNMI: 4×22_627_416=90_509_664. ✓
//     NSM1: 4×8=32. ✓
//
//   P₄ (S_A=2, S_B=3, S_C=3, S_D=2):
//     Edge A-B (ssum=5): NNI_e=isqrt64(5×10^12)=2_236_067; NNMI_e=5×2_236_067=11_180_335.
//       (√5×10^6=2_236_067.977...; 2_236_067²=4_999_995_628_489<5×10^12<2_236_068²) ✓
//     Edge B-C (ssum=6): NNI_e=isqrt64(6×10^12)=2_449_489; NNMI_e=6×2_449_489=14_696_934.
//       (√6×10^6=2_449_489.742...; 2_449_489²=5_999_996_361_121<6×10^12) ✓
//     Edge C-D: same as A-B.
//     NNI  total = 2_236_067+2_449_489+2_236_067=6_921_623. ✓
//     NNMI total = 11_180_335+14_696_934+11_180_335=37_057_604. ✓
//     NSM1 total = 5+6+5=16. ✓
//
//   K₄ (S-uniform S=9, ssum=18, 6 edges):
//     NNI  per edge: isqrt64(18×10^12)=4_242_640. (√18×10^6=3√2×10^6=4_242_640.687...)
//       Total: 25_455_840. ✓
//     NNMI per edge: 18×4_242_640=76_367_520. Total: 458_205_120. ✓
//     NSM1: 6×18=108. ✓
//
//   K_{2,3} (S-uniform S=6, ssum=12, 6 edges):
//     NNI  per edge: isqrt64(12×10^12)=3_464_101. (√12×10^6=2√3×10^6=3_464_101.615...)
//       Total: 20_784_606. ✓
//     NNMI per edge: 12×3_464_101=41_569_212. Total: 249_415_272. ✓
//     NSM1: 6×12=72. ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (1_414_213, 2_828_426, 2, 1, 2)
//  4.  Path P₃ = A-B-C                   → (4_000_000, 16_000_000, 8, 2, 3)
//  5.  Triangle K₃                       → (8_485_281, 67_882_248, 24, 3, 3)
//  6.  Star K_{1,4}                      → (11_313_708, 90_509_664, 32, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (6_921_623, 37_057_604, 16, 3, 4)
//  8.  Complete K₄                       → (25_455_840, 458_205_120, 108, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (20_784_606, 249_415_272, 72, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T28_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_28");
const T28_EXEC:   ExecutorId = ExecutorId::from_ascii("t28.exec");

const T28_KEY_A: &str = "t28.alpha";
const T28_KEY_B: &str = "t28.beta";
const T28_KEY_C: &str = "t28.gamma";
const T28_KEY_D: &str = "t28.delta";
const T28_KEY_E: &str = "t28.epsilon";

const T28_ID_A: NodeId = derive_node_id(T28_PLUGIN, T28_KEY_A);
const T28_ID_B: NodeId = derive_node_id(T28_PLUGIN, T28_KEY_B);
const T28_ID_C: NodeId = derive_node_id(T28_PLUGIN, T28_KEY_C);
const T28_ID_D: NodeId = derive_node_id(T28_PLUGIN, T28_KEY_D);
const T28_ID_E: NodeId = derive_node_id(T28_PLUGIN, T28_KEY_E);

// L4=115 namespace for this harness.
const T28_VEC_A: VectorAddress = VectorAddress::new(115, 1, 1, 0);
const T28_VEC_B: VectorAddress = VectorAddress::new(115, 1, 2, 0);
const T28_VEC_C: VectorAddress = VectorAddress::new(115, 1, 3, 0);
const T28_VEC_D: VectorAddress = VectorAddress::new(115, 2, 1, 0);
const T28_VEC_E: VectorAddress = VectorAddress::new(115, 2, 2, 0);

const T28_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T28_PLUGIN,
    name:         "kl-graph-topo28-harness",
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
        executor_id:       T28_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T28_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T28_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nni, nnmi, nsm1, ec, nc) = gos_runtime::graph_topo_indices28();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(nni,  0, "empty: NNI=0");
    assert_eq!(nnmi, 0, "empty: NNMI=0");
    assert_eq!(nsm1, 0, "empty: NSM1=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T28_VEC_A, T28_KEY_A, T28_ID_A);

    let (nni, nnmi, nsm1, ec, nc) = gos_runtime::graph_topo_indices28();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(nni,  0, "single: NNI=0 (no edges)");
    assert_eq!(nnmi, 0, "single: NNMI=0 (no edges)");
    assert_eq!(nsm1, 0, "single: NSM1=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. ssum=2.
// NNI:  isqrt64(2×10^12)=1_414_213.
// NNMI: 2×1_414_213=2_828_426.
// NSM1: 1+1=2.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T28_VEC_A, T28_KEY_A, T28_ID_A);
    add_node(T28_VEC_B, T28_KEY_B, T28_ID_B);
    add_edge(T28_ID_A, T28_ID_B, "t28.e.ab");

    let (nni, nnmi, nsm1, ec, nc) = gos_runtime::graph_topo_indices28();
    assert_eq!(nc,   2,         "k2: node_count=2");
    assert_eq!(ec,   1,         "k2: edge_count=1");
    assert_eq!(nni,  1_414_213, "k2: NNI=1_414_213 (isqrt64(2\u{00d7}10\u{00b9}\u{00b2})=1_414_213; S=1 uniform)");
    assert_eq!(nnmi, 2_828_426, "k2: NNMI=2_828_426 (2\u{00d7}1_414_213; ssum\u{00d7}NNI_e at ssum=2)");
    assert_eq!(nsm1, 2,         "k2: NSM1=2 (1+1; S=1 uniform)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 2 edges, ssum=4 each.
// NNI  per edge: isqrt64(4×10^12)=2_000_000. Total: 4_000_000.
// NNMI per edge: 4×2_000_000=8_000_000. Total: 16_000_000.
// NSM1: 2×4=8.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T28_VEC_A, T28_KEY_A, T28_ID_A);
    add_node(T28_VEC_B, T28_KEY_B, T28_ID_B);
    add_node(T28_VEC_C, T28_KEY_C, T28_ID_C);
    add_edge(T28_ID_A, T28_ID_B, "t28.e.ab");
    add_edge(T28_ID_B, T28_ID_C, "t28.e.bc");

    let (nni, nnmi, nsm1, ec, nc) = gos_runtime::graph_topo_indices28();
    assert_eq!(nc,   3,          "p3: node_count=3");
    assert_eq!(ec,   2,          "p3: edge_count=2");
    assert_eq!(nni,  4_000_000,  "p3: NNI=4_000_000 (2\u{00d7}2_000_000; isqrt64(4\u{00d7}10\u{00b9}\u{00b2})=2_000_000; S-uniform S=2)");
    assert_eq!(nnmi, 16_000_000, "p3: NNMI=16_000_000 (2\u{00d7}8_000_000; 4\u{00d7}2_000_000=8_000_000 per edge)");
    assert_eq!(nsm1, 8,          "p3: NSM1=8 (2\u{00d7}4; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 edges, ssum=8 each.
// NNI  per edge: isqrt64(8×10^12)=2_828_427. Total: 8_485_281.
// NNMI per edge: 8×2_828_427=22_627_416. Total: 67_882_248.
// NSM1: 3×8=24.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T28_VEC_A, T28_KEY_A, T28_ID_A);
    add_node(T28_VEC_B, T28_KEY_B, T28_ID_B);
    add_node(T28_VEC_C, T28_KEY_C, T28_ID_C);
    add_edge(T28_ID_A, T28_ID_B, "t28.e.ab");
    add_edge(T28_ID_B, T28_ID_A, "t28.e.ba");
    add_edge(T28_ID_B, T28_ID_C, "t28.e.bc");
    add_edge(T28_ID_C, T28_ID_B, "t28.e.cb");
    add_edge(T28_ID_A, T28_ID_C, "t28.e.ac");
    add_edge(T28_ID_C, T28_ID_A, "t28.e.ca");

    let (nni, nnmi, nsm1, ec, nc) = gos_runtime::graph_topo_indices28();
    assert_eq!(nc,   3,          "k3: node_count=3");
    assert_eq!(ec,   3,          "k3: edge_count=3");
    assert_eq!(nni,  8_485_281,  "k3: NNI=8_485_281 (3\u{00d7}2_828_427; isqrt64(8\u{00d7}10\u{00b9}\u{00b2})=2_828_427; S-uniform S=4)");
    assert_eq!(nnmi, 67_882_248, "k3: NNMI=67_882_248 (3\u{00d7}22_627_416; 8\u{00d7}2_828_427=22_627_416 per edge)");
    assert_eq!(nsm1, 24,         "k3: NSM1=24 (3\u{00d7}8; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 4 edges, ssum=8 each.
// Same per-edge NNI and NNMI as K₃ (S-uniform S=4 coincidence).
// NNI: 4×2_828_427=11_313_708. NNMI: 4×22_627_416=90_509_664. NSM1: 4×8=32.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T28_VEC_A, T28_KEY_A, T28_ID_A);
    add_node(T28_VEC_B, T28_KEY_B, T28_ID_B);
    add_node(T28_VEC_C, T28_KEY_C, T28_ID_C);
    add_node(T28_VEC_D, T28_KEY_D, T28_ID_D);
    add_node(T28_VEC_E, T28_KEY_E, T28_ID_E);
    add_edge(T28_ID_A, T28_ID_B, "t28.e.ab");
    add_edge(T28_ID_A, T28_ID_C, "t28.e.ac");
    add_edge(T28_ID_A, T28_ID_D, "t28.e.ad");
    add_edge(T28_ID_A, T28_ID_E, "t28.e.ae");

    let (nni, nnmi, nsm1, ec, nc) = gos_runtime::graph_topo_indices28();
    assert_eq!(nc,   5,          "star: node_count=5");
    assert_eq!(ec,   4,          "star: edge_count=4");
    assert_eq!(nni,  11_313_708, "star: NNI=11_313_708 (4\u{00d7}2_828_427; S-uniform S=4 coincides with K\u{2083} per-edge)");
    assert_eq!(nnmi, 90_509_664, "star: NNMI=90_509_664 (4\u{00d7}22_627_416; S-uniform S=4 coincidence)");
    assert_eq!(nsm1, 32,         "star: NSM1=32 (4\u{00d7}8; S-uniform S=4)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2.
// S(A)=2, S(B)=3, S(C)=3, S(D)=2.
// Edge A-B (ssum=5): NNI_e=2_236_067; NNMI_e=11_180_335.
// Edge B-C (ssum=6): NNI_e=2_449_489; NNMI_e=14_696_934.
// Edge C-D: same as A-B.
// NNI=6_921_623. NNMI=37_057_604. NSM1=16.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T28_VEC_A, T28_KEY_A, T28_ID_A);
    add_node(T28_VEC_B, T28_KEY_B, T28_ID_B);
    add_node(T28_VEC_C, T28_KEY_C, T28_ID_C);
    add_node(T28_VEC_D, T28_KEY_D, T28_ID_D);
    add_edge(T28_ID_A, T28_ID_B, "t28.e.ab");
    add_edge(T28_ID_B, T28_ID_C, "t28.e.bc");
    add_edge(T28_ID_C, T28_ID_D, "t28.e.cd");

    let (nni, nnmi, nsm1, ec, nc) = gos_runtime::graph_topo_indices28();
    assert_eq!(nc,   4,          "p4: node_count=4");
    assert_eq!(ec,   3,          "p4: edge_count=3");
    assert_eq!(nni,  6_921_623,  "p4: NNI=6_921_623 (2_236_067+2_449_489+2_236_067; mixed S sums 5,6,5)");
    assert_eq!(nnmi, 37_057_604, "p4: NNMI=37_057_604 (11_180_335+14_696_934+11_180_335; 5\u{00d7}NNI_e each)");
    assert_eq!(nsm1, 16,         "p4: NSM1=16 (5+6+5; S values 2,3,3,2)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=3×3=9. S-uniform S=9. 6 edges, ssum=18 each.
// NNI  per edge: isqrt64(18×10^12)=4_242_640. Total: 25_455_840.
// NNMI per edge: 18×4_242_640=76_367_520. Total: 458_205_120.
// NSM1: 6×18=108.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T28_VEC_A, T28_KEY_A, T28_ID_A);
    add_node(T28_VEC_B, T28_KEY_B, T28_ID_B);
    add_node(T28_VEC_C, T28_KEY_C, T28_ID_C);
    add_node(T28_VEC_D, T28_KEY_D, T28_ID_D);
    add_edge(T28_ID_A, T28_ID_B, "t28.e.ab");
    add_edge(T28_ID_B, T28_ID_A, "t28.e.ba");
    add_edge(T28_ID_A, T28_ID_C, "t28.e.ac");
    add_edge(T28_ID_C, T28_ID_A, "t28.e.ca");
    add_edge(T28_ID_A, T28_ID_D, "t28.e.ad");
    add_edge(T28_ID_D, T28_ID_A, "t28.e.da");
    add_edge(T28_ID_B, T28_ID_C, "t28.e.bc");
    add_edge(T28_ID_C, T28_ID_B, "t28.e.cb");
    add_edge(T28_ID_B, T28_ID_D, "t28.e.bd");
    add_edge(T28_ID_D, T28_ID_B, "t28.e.db");
    add_edge(T28_ID_C, T28_ID_D, "t28.e.cd");
    add_edge(T28_ID_D, T28_ID_C, "t28.e.dc");

    let (nni, nnmi, nsm1, ec, nc) = gos_runtime::graph_topo_indices28();
    assert_eq!(nc,   4,           "k4: node_count=4");
    assert_eq!(ec,   6,           "k4: edge_count=6");
    assert_eq!(nni,  25_455_840,  "k4: NNI=25_455_840 (6\u{00d7}4_242_640; isqrt64(18\u{00d7}10\u{00b9}\u{00b2})=4_242_640; S-uniform S=9)");
    assert_eq!(nnmi, 458_205_120, "k4: NNMI=458_205_120 (6\u{00d7}76_367_520; 18\u{00d7}4_242_640=76_367_520 per edge)");
    assert_eq!(nsm1, 108,         "k4: NSM1=108 (6\u{00d7}18; S-uniform S=9)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T28_VEC_A, T28_KEY_A, T28_ID_A);
    add_node(T28_VEC_B, T28_KEY_B, T28_ID_B);

    let (nni, nnmi, nsm1, ec, nc) = gos_runtime::graph_topo_indices28();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(nni,  0, "isolated: NNI=0 (no edges)");
    assert_eq!(nnmi, 0, "isolated: NNMI=0 (no edges)");
    assert_eq!(nsm1, 0, "isolated: NSM1=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 6 edges, ssum=12 each.
// NNI  per edge: isqrt64(12×10^12)=3_464_101. Total: 20_784_606.
//   (√12×10^6=2√3×10^6=3_464_101.615...; 3_464_101²=11_999_995_666_201<12×10^12) ✓
// NNMI per edge: 12×3_464_101=41_569_212. Total: 249_415_272.
// NSM1: 6×12=72.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T28_VEC_A, T28_KEY_A, T28_ID_A);
    add_node(T28_VEC_B, T28_KEY_B, T28_ID_B);
    add_node(T28_VEC_C, T28_KEY_C, T28_ID_C);
    add_node(T28_VEC_D, T28_KEY_D, T28_ID_D);
    add_node(T28_VEC_E, T28_KEY_E, T28_ID_E);
    add_edge(T28_ID_A, T28_ID_C, "t28.e.ac");
    add_edge(T28_ID_C, T28_ID_A, "t28.e.ca");
    add_edge(T28_ID_A, T28_ID_D, "t28.e.ad");
    add_edge(T28_ID_D, T28_ID_A, "t28.e.da");
    add_edge(T28_ID_A, T28_ID_E, "t28.e.ae");
    add_edge(T28_ID_E, T28_ID_A, "t28.e.ea");
    add_edge(T28_ID_B, T28_ID_C, "t28.e.bc");
    add_edge(T28_ID_C, T28_ID_B, "t28.e.cb");
    add_edge(T28_ID_B, T28_ID_D, "t28.e.bd");
    add_edge(T28_ID_D, T28_ID_B, "t28.e.db");
    add_edge(T28_ID_B, T28_ID_E, "t28.e.be");
    add_edge(T28_ID_E, T28_ID_B, "t28.e.eb");

    let (nni, nnmi, nsm1, ec, nc) = gos_runtime::graph_topo_indices28();
    assert_eq!(nc,   5,           "k23: node_count=5");
    assert_eq!(ec,   6,           "k23: edge_count=6");
    assert_eq!(nni,  20_784_606,  "k23: NNI=20_784_606 (6\u{00d7}3_464_101; isqrt64(12\u{00d7}10\u{00b9}\u{00b2})=3_464_101; S-uniform S=6)");
    assert_eq!(nnmi, 249_415_272, "k23: NNMI=249_415_272 (6\u{00d7}41_569_212; 12\u{00d7}3_464_101=41_569_212 per edge)");
    assert_eq!(nsm1, 72,          "k23: NSM1=72 (6\u{00d7}12; S-uniform S=6)");
}
