// gos-graph-topo31-harness — V3.42 NSig + NHQS + NPS (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices31()`:
//   Returns (nsig, nhqs, nps, edge_count, node_count)
//   - nsig = NSig(G) = Σ_{uv∈E} (S_u−S_v)²          (exact u64; S-Sigma irregularity; =0 iff S-regular)
//   - nhqs = NHQS(G) = Σ_{uv∈E} (S_u+S_v)^4          (exact u64; S-quartic edge-sum)
//   - nps  = NPS(G)  = Σ_v S(v)^5                     (exact u64; S-penta vertex sum)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NSig(G) = Σ_{uv∈E} (S_u−S_v)²
//     S-variant of the Sigma irregularity index σ(G) = Σ_{uv∈E} (d_u−d_v)².
//     NSig = 0 iff S-regular. NSig > 0 iff any edge has differing endpoint S values.
//
//   NHQS(G) = Σ_{uv∈E} (S_u+S_v)^4
//     S-quartic edge-sum; extends NHM1=Σ(S+S)² (topo23) and NHCS=Σ(S+S)³ (topo30) to 4th power.
//     NHQS = |E|·(2S)^4 = 16|E|S^4 for S-regular.
//     K₃ and K_{1,4}: both S-uniform S=4 → same per-edge NHQS (8^4=4096); totals differ by |E|.
//
//   NPS(G) = Σ_v S(v)^5
//     S-penta vertex sum; extends NVQ=Σ S⁴ (topo30) to 5th power.
//     NPS = n·S^5 for S-regular.
//     Vertex-power series: NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31).
//
// IMPLEMENTATION FORMULAS (no float, no_std safe):
//   NSig per edge = (S_u−S_v)²                        [u64; max ≤ 16129² ≈ 2.60×10^8; sum < u64::MAX]
//   NHQS per edge = (S_u+S_v)^4 (u128 accumulator)   [per-edge ≤ 32258^4 ≈ 1.08×10^18 < u64::MAX]
//   NPS  per node = S^5 (u128 accumulator, min u64::MAX) [S^5 ≤ 16129^5 ≈ 1.09×10^21 > u64::MAX]
//
// KEY INVARIANTS:
//   NSig = 0 iff S-regular (certified by annotation "NSig=0: S-regular").
//   NHQS = 16|E|S^4 for S-regular (= |E|·(2S)^4).
//   NPS  = n·S^5 for S-regular.
//   K₃ and K_{1,4}: S-uniform S=4 → same per-edge NHQS (4096); differ in NPS (|nodes|) and NHQS total (|E|).
//
// S VALUES PER GRAPH:
//   K₂        : S(A)=S(B)=1
//   P₃=A-B-C  : S(A)=S(B)=S(C)=2    → S-uniform S=2
//   K₃        : S(each)=4            → S-uniform S=4
//   K_{1,4}   : S(hub)=4, S(leaf)=4  → S-uniform S=4
//   P₄=A-B-C-D: S(A)=S(D)=2, S(B)=S(C)=3 → mixed S; NSig > 0
//   K₄        : S(each)=9            → S-uniform S=9
//   K_{2,3}   : S(all)=6             → S-uniform S=6
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph         NSig(exact)  NHQS(exact)   NPS(exact)    edges  nodes
//  Empty                   0            0            0       0      0
//  1 node                  0            0            0       0      1
//  Edge K₂                 0           16            2       1      2
//  Path P₃                 0          512           96       2      3
//  Triangle K₃             0       12_288        3_072       3      3
//  Star K_{1,4}            0       16_384        5_120       4      5
//  Path P₄                 2        2_546          550       3      4
//  Complete K₄             0      629_856      236_196       6      4
//  2 isolated              0            0            0       0      2
//  K_{2,3}                 0      124_416       38_880       6      5
//
// DERIVATIONS:
//
//   K₂ (S_A=S_B=1):
//     NSig:  (1−1)² = 0. ✓
//     NHQS: (1+1)^4 = 16. ✓
//     NPS:  1^5 + 1^5 = 2. ✓
//
//   P₃ = A-B-C (S-uniform S=2, 3 nodes, 2 edges):
//     S(A)=deg(B)=2; S(B)=deg(A)+deg(C)=1+1=2; S(C)=deg(B)=2.
//     NSig: 2 × (2−2)² = 0. ✓
//     NHQS: 2 × (2+2)^4 = 2 × 256 = 512. ✓
//     NPS:  3 × 2^5 = 3 × 32 = 96. ✓
//
//   K₃ (S-uniform S=4, 3 nodes, 3 edges):
//     S(each) = 2+2 = 4. NSig: 3 × (4−4)² = 0. ✓
//     NHQS: 3 × (4+4)^4 = 3 × 8^4 = 3 × 4096 = 12_288. ✓
//     NPS:  3 × 4^5 = 3 × 1024 = 3_072. ✓
//
//   K_{1,4} (S-uniform S=4, 5 nodes, 4 edges):
//     S(hub)=4×1=4; S(leaf)=deg(hub)=4. S-uniform S=4.
//     NSig: 4 × (4−4)² = 0. ✓
//     NHQS: 4 × 4096 = 16_384. ✓ (same per-edge as K₃)
//     NPS:  5 × 1024 = 5_120. ✓ (more nodes than K₃)
//
//   P₄ = A-B-C-D (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges):
//     S(A)=deg(B)=2; S(B)=deg(A)+deg(C)=1+2=3; S(C)=deg(B)+deg(D)=2+1=3; S(D)=deg(C)=2.
//     NSig: {A,B}=(2−3)²=1; {B,C}=(3−3)²=0; {C,D}=(3−2)²=1. Total=2. ✓
//     NHQS: (2+3)^4+(3+3)^4+(3+2)^4 = 5^4+6^4+5^4 = 625+1296+625 = 2_546. ✓
//     NPS:  2^5+3^5+3^5+2^5 = 32+243+243+32 = 550. ✓
//
//   K₄ (S-uniform S=9, 4 nodes, 6 edges):
//     S(each) = 3×3 = 9. NSig: 6 × (9−9)² = 0. ✓
//     NHQS: 6 × (9+9)^4 = 6 × 18^4 = 6 × 104_976 = 629_856. ✓
//     NPS:  4 × 9^5 = 4 × 59_049 = 236_196. ✓
//
//   K_{2,3} (S-uniform S=6, 5 nodes, 6 edges):
//     S(left: d=3) = 3×2=6; S(right: d=2) = 2×3=6. S-uniform S=6.
//     NSig: 6 × (6−6)² = 0. ✓
//     NHQS: 6 × (6+6)^4 = 6 × 12^4 = 6 × 20_736 = 124_416. ✓
//     NPS:  5 × 6^5 = 5 × 7_776 = 38_880. ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (0, 16, 2, 1, 2)
//  4.  Path P₃ = A-B-C                   → (0, 512, 96, 2, 3)
//  5.  Triangle K₃                       → (0, 12_288, 3_072, 3, 3)
//  6.  Star K_{1,4}                      → (0, 16_384, 5_120, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (2, 2_546, 550, 3, 4)
//  8.  Complete K₄                       → (0, 629_856, 236_196, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (0, 124_416, 38_880, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T31_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_31");
const T31_EXEC:   ExecutorId = ExecutorId::from_ascii("t31.exec");

const T31_KEY_A: &str = "t31.alpha";
const T31_KEY_B: &str = "t31.beta";
const T31_KEY_C: &str = "t31.gamma";
const T31_KEY_D: &str = "t31.delta";
const T31_KEY_E: &str = "t31.epsilon";

const T31_ID_A: NodeId = derive_node_id(T31_PLUGIN, T31_KEY_A);
const T31_ID_B: NodeId = derive_node_id(T31_PLUGIN, T31_KEY_B);
const T31_ID_C: NodeId = derive_node_id(T31_PLUGIN, T31_KEY_C);
const T31_ID_D: NodeId = derive_node_id(T31_PLUGIN, T31_KEY_D);
const T31_ID_E: NodeId = derive_node_id(T31_PLUGIN, T31_KEY_E);

// L4=118 namespace for this harness.
const T31_VEC_A: VectorAddress = VectorAddress::new(118, 1, 1, 0);
const T31_VEC_B: VectorAddress = VectorAddress::new(118, 1, 2, 0);
const T31_VEC_C: VectorAddress = VectorAddress::new(118, 1, 3, 0);
const T31_VEC_D: VectorAddress = VectorAddress::new(118, 2, 1, 0);
const T31_VEC_E: VectorAddress = VectorAddress::new(118, 2, 2, 0);

const T31_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T31_PLUGIN,
    name:         "kl-graph-topo31-harness",
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
        executor_id:       T31_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T31_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T31_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nsig, nhqs, nps, ec, nc) = gos_runtime::graph_topo_indices31();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(nsig, 0, "empty: NSig=0");
    assert_eq!(nhqs, 0, "empty: NHQS=0");
    assert_eq!(nps,  0, "empty: NPS=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NSig: no edges; NHQS: no edges; NPS: 0^5=0.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T31_VEC_A, T31_KEY_A, T31_ID_A);

    let (nsig, nhqs, nps, ec, nc) = gos_runtime::graph_topo_indices31();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(nsig, 0, "single: NSig=0 (no edges)");
    assert_eq!(nhqs, 0, "single: NHQS=0 (no edges)");
    assert_eq!(nps,  0, "single: NPS=0 (S=0; 0^5=0)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NSig: (1−1)² = 0.
// NHQS: (1+1)^4 = 2^4 = 16.
// NPS:  1^5 + 1^5 = 2.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T31_VEC_A, T31_KEY_A, T31_ID_A);
    add_node(T31_VEC_B, T31_KEY_B, T31_ID_B);
    add_edge(T31_ID_A, T31_ID_B, "t31.e.ab");

    let (nsig, nhqs, nps, ec, nc) = gos_runtime::graph_topo_indices31();
    assert_eq!(nc,   2,  "k2: node_count=2");
    assert_eq!(ec,   1,  "k2: edge_count=1");
    assert_eq!(nsig, 0,  "k2: NSig=0 ((1\u{2212}1)\u{00b2}=0; S-uniform S=1)");
    assert_eq!(nhqs, 16, "k2: NHQS=16 ((1+1)\u{2074}=2\u{2074}=16; S-uniform S=1)");
    assert_eq!(nps,  2,  "k2: NPS=2 (1\u{2075}+1\u{2075}=2; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NSig: 0. NHQS: 2×(2+2)^4=2×256=512. NPS: 3×32=96.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T31_VEC_A, T31_KEY_A, T31_ID_A);
    add_node(T31_VEC_B, T31_KEY_B, T31_ID_B);
    add_node(T31_VEC_C, T31_KEY_C, T31_ID_C);
    add_edge(T31_ID_A, T31_ID_B, "t31.e.ab");
    add_edge(T31_ID_B, T31_ID_C, "t31.e.bc");

    let (nsig, nhqs, nps, ec, nc) = gos_runtime::graph_topo_indices31();
    assert_eq!(nc,   3,   "p3: node_count=3");
    assert_eq!(ec,   2,   "p3: edge_count=2");
    assert_eq!(nsig, 0,   "p3: NSig=0 (S-uniform S=2; (2\u{2212}2)\u{00b2}=0)");
    assert_eq!(nhqs, 512, "p3: NHQS=512 (2\u{00d7}256; (2+2)\u{2074}=256; S-uniform S=2)");
    assert_eq!(nps,  96,  "p3: NPS=96 (3\u{00d7}32; 2\u{2075}=32; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NSig: 0. NHQS: 3×(4+4)^4=3×4096=12_288. NPS: 3×4^5=3×1024=3_072.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T31_VEC_A, T31_KEY_A, T31_ID_A);
    add_node(T31_VEC_B, T31_KEY_B, T31_ID_B);
    add_node(T31_VEC_C, T31_KEY_C, T31_ID_C);
    add_edge(T31_ID_A, T31_ID_B, "t31.e.ab");
    add_edge(T31_ID_B, T31_ID_A, "t31.e.ba");
    add_edge(T31_ID_B, T31_ID_C, "t31.e.bc");
    add_edge(T31_ID_C, T31_ID_B, "t31.e.cb");
    add_edge(T31_ID_A, T31_ID_C, "t31.e.ac");
    add_edge(T31_ID_C, T31_ID_A, "t31.e.ca");

    let (nsig, nhqs, nps, ec, nc) = gos_runtime::graph_topo_indices31();
    assert_eq!(nc,   3,      "k3: node_count=3");
    assert_eq!(ec,   3,      "k3: edge_count=3");
    assert_eq!(nsig, 0,      "k3: NSig=0 (S-uniform S=4; (4\u{2212}4)\u{00b2}=0)");
    assert_eq!(nhqs, 12_288, "k3: NHQS=12_288 (3\u{00d7}4096; (4+4)\u{2074}=8\u{2074}=4096; S-uniform S=4)");
    assert_eq!(nps,  3_072,  "k3: NPS=3_072 (3\u{00d7}1024; 4\u{2075}=1024; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-edge NSig (0) and NHQS (4096) as K₃; NPS and totals differ by node/edge count.
// NSig: 0. NHQS: 4×4096=16_384. NPS: 5×1024=5_120.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T31_VEC_A, T31_KEY_A, T31_ID_A);
    add_node(T31_VEC_B, T31_KEY_B, T31_ID_B);
    add_node(T31_VEC_C, T31_KEY_C, T31_ID_C);
    add_node(T31_VEC_D, T31_KEY_D, T31_ID_D);
    add_node(T31_VEC_E, T31_KEY_E, T31_ID_E);
    add_edge(T31_ID_A, T31_ID_B, "t31.e.ab");
    add_edge(T31_ID_A, T31_ID_C, "t31.e.ac");
    add_edge(T31_ID_A, T31_ID_D, "t31.e.ad");
    add_edge(T31_ID_A, T31_ID_E, "t31.e.ae");

    let (nsig, nhqs, nps, ec, nc) = gos_runtime::graph_topo_indices31();
    assert_eq!(nc,   5,      "star: node_count=5");
    assert_eq!(ec,   4,      "star: edge_count=4");
    assert_eq!(nsig, 0,      "star: NSig=0 (S-uniform S=4; same S as K\u{2083})");
    assert_eq!(nhqs, 16_384, "star: NHQS=16_384 (4\u{00d7}4096; same per-edge as K\u{2083})");
    assert_eq!(nps,  5_120,  "star: NPS=5_120 (5\u{00d7}1024; more nodes than K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S (NSig > 0).
// NSig: (2−3)²+(3−3)²+(3−2)²=1+0+1=2.
// NHQS: 5^4+6^4+5^4=625+1296+625=2_546.
// NPS:  2^5+3^5+3^5+2^5=32+243+243+32=550.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T31_VEC_A, T31_KEY_A, T31_ID_A);
    add_node(T31_VEC_B, T31_KEY_B, T31_ID_B);
    add_node(T31_VEC_C, T31_KEY_C, T31_ID_C);
    add_node(T31_VEC_D, T31_KEY_D, T31_ID_D);
    add_edge(T31_ID_A, T31_ID_B, "t31.e.ab");
    add_edge(T31_ID_B, T31_ID_C, "t31.e.bc");
    add_edge(T31_ID_C, T31_ID_D, "t31.e.cd");

    let (nsig, nhqs, nps, ec, nc) = gos_runtime::graph_topo_indices31();
    assert_eq!(nc,   4,     "p4: node_count=4");
    assert_eq!(ec,   3,     "p4: edge_count=3");
    assert_eq!(nsig, 2,     "p4: NSig=2 ((2\u{2212}3)\u{00b2}+(3\u{2212}3)\u{00b2}+(3\u{2212}2)\u{00b2}=1+0+1; S values 2,3,3,2)");
    assert_eq!(nhqs, 2_546, "p4: NHQS=2_546 (625+1296+625; (2+3)\u{2074}+(3+3)\u{2074}+(3+2)\u{2074})");
    assert_eq!(nps,  550,   "p4: NPS=550 (32+243+243+32; 2\u{2075}+3\u{2075}+3\u{2075}+2\u{2075})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NSig: 0. NHQS: 6×(9+9)^4=6×18^4=6×104_976=629_856. NPS: 4×9^5=4×59_049=236_196.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T31_VEC_A, T31_KEY_A, T31_ID_A);
    add_node(T31_VEC_B, T31_KEY_B, T31_ID_B);
    add_node(T31_VEC_C, T31_KEY_C, T31_ID_C);
    add_node(T31_VEC_D, T31_KEY_D, T31_ID_D);
    add_edge(T31_ID_A, T31_ID_B, "t31.e.ab");
    add_edge(T31_ID_B, T31_ID_A, "t31.e.ba");
    add_edge(T31_ID_A, T31_ID_C, "t31.e.ac");
    add_edge(T31_ID_C, T31_ID_A, "t31.e.ca");
    add_edge(T31_ID_A, T31_ID_D, "t31.e.ad");
    add_edge(T31_ID_D, T31_ID_A, "t31.e.da");
    add_edge(T31_ID_B, T31_ID_C, "t31.e.bc");
    add_edge(T31_ID_C, T31_ID_B, "t31.e.cb");
    add_edge(T31_ID_B, T31_ID_D, "t31.e.bd");
    add_edge(T31_ID_D, T31_ID_B, "t31.e.db");
    add_edge(T31_ID_C, T31_ID_D, "t31.e.cd");
    add_edge(T31_ID_D, T31_ID_C, "t31.e.dc");

    let (nsig, nhqs, nps, ec, nc) = gos_runtime::graph_topo_indices31();
    assert_eq!(nc,   4,       "k4: node_count=4");
    assert_eq!(ec,   6,       "k4: edge_count=6");
    assert_eq!(nsig, 0,       "k4: NSig=0 (S-uniform S=9; (9\u{2212}9)\u{00b2}=0)");
    assert_eq!(nhqs, 629_856, "k4: NHQS=629_856 (6\u{00d7}104_976; (9+9)\u{2074}=18\u{2074}=104_976; S-uniform S=9)");
    assert_eq!(nps,  236_196, "k4: NPS=236_196 (4\u{00d7}59_049; 9\u{2075}=59_049; S-uniform S=9)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NSig=0 (no edges); NHQS=0 (no edges); NPS=0 (0^5=0).

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T31_VEC_A, T31_KEY_A, T31_ID_A);
    add_node(T31_VEC_B, T31_KEY_B, T31_ID_B);

    let (nsig, nhqs, nps, ec, nc) = gos_runtime::graph_topo_indices31();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(nsig, 0, "isolated: NSig=0 (no edges)");
    assert_eq!(nhqs, 0, "isolated: NHQS=0 (no edges)");
    assert_eq!(nps,  0, "isolated: NPS=0 (S=0; 0^5=0)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NSig: 6×(6−6)²=0. NHQS: 6×(6+6)^4=6×20_736=124_416. NPS: 5×7_776=38_880.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T31_VEC_A, T31_KEY_A, T31_ID_A);
    add_node(T31_VEC_B, T31_KEY_B, T31_ID_B);
    add_node(T31_VEC_C, T31_KEY_C, T31_ID_C);
    add_node(T31_VEC_D, T31_KEY_D, T31_ID_D);
    add_node(T31_VEC_E, T31_KEY_E, T31_ID_E);
    add_edge(T31_ID_A, T31_ID_C, "t31.e.ac");
    add_edge(T31_ID_C, T31_ID_A, "t31.e.ca");
    add_edge(T31_ID_A, T31_ID_D, "t31.e.ad");
    add_edge(T31_ID_D, T31_ID_A, "t31.e.da");
    add_edge(T31_ID_A, T31_ID_E, "t31.e.ae");
    add_edge(T31_ID_E, T31_ID_A, "t31.e.ea");
    add_edge(T31_ID_B, T31_ID_C, "t31.e.bc");
    add_edge(T31_ID_C, T31_ID_B, "t31.e.cb");
    add_edge(T31_ID_B, T31_ID_D, "t31.e.bd");
    add_edge(T31_ID_D, T31_ID_B, "t31.e.db");
    add_edge(T31_ID_B, T31_ID_E, "t31.e.be");
    add_edge(T31_ID_E, T31_ID_B, "t31.e.eb");

    let (nsig, nhqs, nps, ec, nc) = gos_runtime::graph_topo_indices31();
    assert_eq!(nc,   5,       "k23: node_count=5");
    assert_eq!(ec,   6,       "k23: edge_count=6");
    assert_eq!(nsig, 0,       "k23: NSig=0 (S-uniform S=6; (6\u{2212}6)\u{00b2}=0)");
    assert_eq!(nhqs, 124_416, "k23: NHQS=124_416 (6\u{00d7}20_736; (6+6)\u{2074}=12\u{2074}=20_736; S-uniform S=6)");
    assert_eq!(nps,  38_880,  "k23: NPS=38_880 (5\u{00d7}7_776; 6\u{2075}=7_776; S-uniform S=6)");
}
