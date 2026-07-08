// gos-graph-topo16-harness — V3.27 Generalized Randić family + Lanzhou Index
//
// Verifies `gos_runtime::graph_topo_indices16()`:
//   Returns (ir_ppm, rr_ppm, lz, edge_count, node_count)
//   - ir_ppm = R_{1/2}(G)×10^6 = Σ_{uv∈E} √(d_u·d_v)×10^6  (floor ppm; Product Connectivity)
//   - rr_ppm = R_{-1}(G)×10^6  = Σ_{uv∈E} ⌊10^6/(d_u·d_v)⌋  (floor ppm; Reciprocal Randić)
//   - lz     = Lz(G) = Σ_v d_v²·(n−1−d_v)                    (exact u64; Lanzhou Index)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// DEFINITIONS:
//   d_v = degree of v (number of neighbours in undirected projection).
//   R_{1/2}(G) = Σ_{uv∈E} (d_u·d_v)^{1/2}  Bollobás & Erdős 1998.
//   R_{-1}(G)  = Σ_{uv∈E} (d_u·d_v)^{-1}   Bollobás & Erdős 1998.
//   Lz(G)      = Σ_v d_v²·(n−1−d_v)          Xia et al. 2019.
//              = (n−1)·M₁(G) − F(G)           (algebraic identity; M₁=1st Zagreb, F=forgotten index)
//
// KEY INVARIANTS:
//   R_{1/2}: regular Δ-graph → R_{1/2} = m·Δ  (ppm = m·Δ·10^6, exact).
//   R_{-1}:  regular Δ-graph → R_{-1}  = m/Δ² (ppm = floor(m·10^6/Δ²)).
//   Lz = 0 for complete graphs K_n (d_v = n−1 → n−1−d_v = 0 for every v).
//   Lz ≥ 0 always; each term d_v²·(n−1−d_v) ≥ 0 since d_v ≤ n−1.
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph          ir_ppm       rr_ppm   lz    edges  nodes
//  Empty               0            0    0      0      0
//  1 node              0            0    0      0      1
//  Edge A-B    1_000_000    1_000_000    0      1      2
//  Path P₃     2_828_426    1_000_000    2      2      3
//  Triangle K₃  6_000_000      750_000    0      3      3
//  Star K_{1,4} 8_000_000    1_000_000   12      4      5
//  Path P₄     4_828_426    1_250_000   12      3      4
//  Complete K₄ 18_000_000      666_666    0      6      4
//  Two isolated         0            0    0      0      2
//  K_{2,3}    14_696_934      999_996   42      6      5
//
// Derivations:
//
//   Edge A-B (n=2, d_A=d_B=1):
//     IR: √(1·1)·10^6 = 1_000_000.
//     RR: 10^6/(1·1) = 1_000_000.
//     Lz: 1²·(2−1−1)=0 for each node. lz=0.
//
//   Path P₃ = A-B-C (n=3, d_A=d_C=1, d_B=2):
//     IR: {A,B}→isqrt64(1×2×10^12)=isqrt64(2×10^12)=1_414_213;
//         {B,C}→same. ir_ppm=2×1_414_213=2_828_426.
//     RR: {A,B}→10^6/(1×2)=500_000; {B,C}→500_000. rr_ppm=1_000_000.
//     Lz: A:1²×(3−1−1)=1; B:2²×(3−1−2)=0; C:1. lz=2.
//     Identity check: (n−1)·M₁−F = 2·(1+4+1)−(1+8+1) = 12−10 = 2. ✓
//
//   Triangle K₃ (n=3, Δ=2, m=3):
//     IR: 3·√(2·2)·10^6 = 3·2_000_000 = 6_000_000 = m·Δ·10^6. ✓
//     RR: 3·10^6/(2·2) = 3·250_000 = 750_000 = floor(m·10^6/Δ²). ✓
//     Lz: each node: 2²·(3−1−2)=0. lz=0 (complete: K₃=complete). ✓
//
//   Star K_{1,4} (n=5, d_A=4, d_B=d_C=d_D=d_E=1):
//     IR: 4 edges × √(4·1)·10^6 = 4·2_000_000 = 8_000_000.
//     RR: 4 × 10^6/(4·1) = 4·250_000 = 1_000_000.
//     Lz: A:4²·(5−1−4)=0; leaf:1²·(5−1−1)=3 (×4)=12. lz=12.
//     Identity: (n−1)·M₁−F = 4·(16+4)−(64+4) = 80−68 = 12. ✓
//
//   Path P₄ = A-B-C-D (n=4, d_A=d_D=1, d_B=d_C=2):
//     IR: {A,B}→isqrt64(2e12)=1_414_213; {B,C}→isqrt64(4e12)=2_000_000;
//         {C,D}→1_414_213. ir_ppm=4_828_426.
//     RR: {A,B}→500_000; {B,C}→10^6/4=250_000; {C,D}→500_000. rr_ppm=1_250_000.
//     Lz: A:1²·2=2; B:4·1=4; C:4·1=4; D:1²·2=2. lz=12.
//     Identity: (n−1)·M₁−F = 3·(1+4+4+1)−(1+8+8+1) = 30−18 = 12. ✓
//
//   Complete K₄ (n=4, Δ=3, m=6):
//     IR: 6·√(3·3)·10^6 = 6·3_000_000 = 18_000_000 = m·Δ·10^6. ✓
//     RR: 6·10^6/(3·3) = 6·111_111 = 666_666. (floor(10^6/9)=111_111). ✓
//     Lz: each node: 3²·(4−1−3)=0. lz=0. ✓
//
//   K_{2,3} (n=5, left={A,B}:d=3, right={C,D,E}:d=2):
//     IR: 6 edges × √(3·2)·10^6 = 6·isqrt64(6e12) = 6·2_449_489 = 14_696_934.
//         (√6=2.44948974…; floor=2_449_489)
//     RR: 6 × 10^6/(3·2) = 6·166_666 = 999_996. (floor(10^6/6)=166_666)
//     Lz: A,B:3²·(5−1−3)=9·1=9 (×2=18); C,D,E:2²·(5−1−2)=4·2=8 (×3=24). lz=42.
//     Identity: (n−1)·M₁−F = 4·(9+9+4+4+4)−(27+27+8+8+8) = 4·30−78 = 120−78 = 42. ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (1_000_000, 1_000_000, 0, 1, 2)
//  4.  Path P₃ = A-B-C                   → (2_828_426, 1_000_000, 2, 2, 3)
//  5.  Triangle K₃                       → (6_000_000, 750_000, 0, 3, 3)
//  6.  Star K_{1,4}                      → (8_000_000, 1_000_000, 12, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (4_828_426, 1_250_000, 12, 3, 4)
//  8.  Complete K₄                       → (18_000_000, 666_666, 0, 6, 4)
//  9.  Two isolated nodes                → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (14_696_934, 999_996, 42, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T16_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_16");
const T16_EXEC:   ExecutorId = ExecutorId::from_ascii("t16.exec");

const T16_KEY_A: &str = "t16.alpha";
const T16_KEY_B: &str = "t16.beta";
const T16_KEY_C: &str = "t16.gamma";
const T16_KEY_D: &str = "t16.delta";
const T16_KEY_E: &str = "t16.epsilon";

const T16_ID_A: NodeId = derive_node_id(T16_PLUGIN, T16_KEY_A);
const T16_ID_B: NodeId = derive_node_id(T16_PLUGIN, T16_KEY_B);
const T16_ID_C: NodeId = derive_node_id(T16_PLUGIN, T16_KEY_C);
const T16_ID_D: NodeId = derive_node_id(T16_PLUGIN, T16_KEY_D);
const T16_ID_E: NodeId = derive_node_id(T16_PLUGIN, T16_KEY_E);

// L4=103 namespace for this harness.
const T16_VEC_A: VectorAddress = VectorAddress::new(103, 1, 1, 0);
const T16_VEC_B: VectorAddress = VectorAddress::new(103, 1, 2, 0);
const T16_VEC_C: VectorAddress = VectorAddress::new(103, 1, 3, 0);
const T16_VEC_D: VectorAddress = VectorAddress::new(103, 2, 1, 0);
const T16_VEC_E: VectorAddress = VectorAddress::new(103, 2, 2, 0);

const T16_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T16_PLUGIN,
    name:         "kl-graph-topo16-harness",
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
        executor_id:       T16_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T16_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T16_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (ir, rr, lz, ec, nc) = gos_runtime::graph_topo_indices16();
    assert_eq!(nc, 0, "empty: node_count=0");
    assert_eq!(ec, 0, "empty: edge_count=0");
    assert_eq!(ir, 0, "empty: IR=0");
    assert_eq!(rr, 0, "empty: RR=0");
    assert_eq!(lz, 0, "empty: Lz=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T16_VEC_A, T16_KEY_A, T16_ID_A);

    let (ir, rr, lz, ec, nc) = gos_runtime::graph_topo_indices16();
    assert_eq!(nc, 1, "single: node_count=1");
    assert_eq!(ec, 0, "single: no edges");
    assert_eq!(ir, 0, "single: IR=0 (no edges)");
    assert_eq!(rr, 0, "single: RR=0 (no edges)");
    assert_eq!(lz, 0, "single: Lz=0 (d=0; 0²·0=0)");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected {A,B}. d_A=d_B=1.
// IR: √(1·1)·10^6 = 1_000_000.
// RR: 10^6/(1·1) = 1_000_000.
// Lz: each node: 1²·(2−1−1)=0. lz=0.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T16_VEC_A, T16_KEY_A, T16_ID_A);
    add_node(T16_VEC_B, T16_KEY_B, T16_ID_B);
    add_edge(T16_ID_A, T16_ID_B, "t16.e.ab");

    let (ir, rr, lz, ec, nc) = gos_runtime::graph_topo_indices16();
    assert_eq!(nc,        2, "edge: node_count=2");
    assert_eq!(ec,        1, "edge: edge_count=1");
    assert_eq!(ir, 1_000_000, "edge: IR=1_000_000 (sqrt(1·1)·10^6)");
    assert_eq!(rr, 1_000_000, "edge: RR=1_000_000 (10^6/(1·1))");
    assert_eq!(lz,        0, "edge: Lz=0 (n=2; 1²·(2−1−1)=0)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d_A=d_C=1, d_B=2. n=3.
// IR: {A,B}→isqrt64(2e12)=1_414_213; {B,C}→1_414_213. ir=2_828_426.
// RR: {A,B}→500_000; {B,C}→500_000. rr=1_000_000.
// Lz: A:1·1=1; B:4·0=0; C:1·1=1. lz=2.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T16_VEC_A, T16_KEY_A, T16_ID_A);
    add_node(T16_VEC_B, T16_KEY_B, T16_ID_B);
    add_node(T16_VEC_C, T16_KEY_C, T16_ID_C);
    add_edge(T16_ID_A, T16_ID_B, "t16.e.ab");
    add_edge(T16_ID_B, T16_ID_C, "t16.e.bc");

    let (ir, rr, lz, ec, nc) = gos_runtime::graph_topo_indices16();
    assert_eq!(nc,        3, "p3: node_count=3");
    assert_eq!(ec,        2, "p3: edge_count=2");
    assert_eq!(ir, 2_828_426, "p3: IR=2_828_426 (2×isqrt64(2e12)=2×1_414_213)");
    assert_eq!(rr, 1_000_000, "p3: RR=1_000_000 (2×500_000)");
    assert_eq!(lz,        2, "p3: Lz=2 (ends:1²×1=1 each; center:2²×0=0)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// Δ=2, m=3, n=3. Regular.
// IR: 3·√(4)·10^6 = 6_000_000 = m·Δ·10^6 (regular). ✓
// RR: 3·10^6/4 = 750_000. ✓
// Lz: each node: 4·(3−1−2)=0. lz=0. ✓

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T16_VEC_A, T16_KEY_A, T16_ID_A);
    add_node(T16_VEC_B, T16_KEY_B, T16_ID_B);
    add_node(T16_VEC_C, T16_KEY_C, T16_ID_C);
    add_edge(T16_ID_A, T16_ID_B, "t16.e.ab");
    add_edge(T16_ID_B, T16_ID_A, "t16.e.ba");
    add_edge(T16_ID_B, T16_ID_C, "t16.e.bc");
    add_edge(T16_ID_C, T16_ID_B, "t16.e.cb");
    add_edge(T16_ID_A, T16_ID_C, "t16.e.ac");
    add_edge(T16_ID_C, T16_ID_A, "t16.e.ca");

    let (ir, rr, lz, ec, nc) = gos_runtime::graph_topo_indices16();
    assert_eq!(nc,        3, "k3: node_count=3");
    assert_eq!(ec,        3, "k3: edge_count=3");
    assert_eq!(ir, 6_000_000, "k3: IR=6_000_000 (regular Δ=2: 3·2·10^6)");
    assert_eq!(rr,   750_000, "k3: RR=750_000 (3·10^6/4)");
    assert_eq!(lz,         0, "k3: Lz=0 (complete K₃: n−1−d=0)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d_A=4 (center), d_BCDE=1 (leaves). n=5.
// IR: 4·√(4·1)·10^6 = 4·2_000_000 = 8_000_000.
// RR: 4·10^6/(4·1) = 4·250_000 = 1_000_000.
// Lz: center:4²·0=0; leaf:1²·3=3 (×4=12). lz=12.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T16_VEC_A, T16_KEY_A, T16_ID_A);
    add_node(T16_VEC_B, T16_KEY_B, T16_ID_B);
    add_node(T16_VEC_C, T16_KEY_C, T16_ID_C);
    add_node(T16_VEC_D, T16_KEY_D, T16_ID_D);
    add_node(T16_VEC_E, T16_KEY_E, T16_ID_E);
    add_edge(T16_ID_A, T16_ID_B, "t16.e.ab");
    add_edge(T16_ID_A, T16_ID_C, "t16.e.ac");
    add_edge(T16_ID_A, T16_ID_D, "t16.e.ad");
    add_edge(T16_ID_A, T16_ID_E, "t16.e.ae");

    let (ir, rr, lz, ec, nc) = gos_runtime::graph_topo_indices16();
    assert_eq!(nc,        5, "star: node_count=5");
    assert_eq!(ec,        4, "star: edge_count=4");
    assert_eq!(ir, 8_000_000, "star: IR=8_000_000 (4·sqrt(4)·10^6)");
    assert_eq!(rr, 1_000_000, "star: RR=1_000_000 (4·10^6/4)");
    assert_eq!(lz,       12, "star: Lz=12 (center:0; leaves:4×3)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d_A=d_D=1, d_B=d_C=2. n=4.
// IR: {A,B}→1_414_213; {B,C}→2_000_000; {C,D}→1_414_213. ir=4_828_426.
// RR: {A,B}→500_000; {B,C}→250_000; {C,D}→500_000. rr=1_250_000.
// Lz: ends:1²·2=2 (×2=4); inner:2²·1=4 (×2=8). lz=12.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T16_VEC_A, T16_KEY_A, T16_ID_A);
    add_node(T16_VEC_B, T16_KEY_B, T16_ID_B);
    add_node(T16_VEC_C, T16_KEY_C, T16_ID_C);
    add_node(T16_VEC_D, T16_KEY_D, T16_ID_D);
    add_edge(T16_ID_A, T16_ID_B, "t16.e.ab");
    add_edge(T16_ID_B, T16_ID_C, "t16.e.bc");
    add_edge(T16_ID_C, T16_ID_D, "t16.e.cd");

    let (ir, rr, lz, ec, nc) = gos_runtime::graph_topo_indices16();
    assert_eq!(nc,        4, "p4: node_count=4");
    assert_eq!(ec,        3, "p4: edge_count=3");
    assert_eq!(ir, 4_828_426, "p4: IR=4_828_426 (1_414_213+2_000_000+1_414_213)");
    assert_eq!(rr, 1_250_000, "p4: RR=1_250_000 (500_000+250_000+500_000)");
    assert_eq!(lz,       12, "p4: Lz=12 (ends:2×2; inner:2×4)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// Δ=3, m=6, n=4. Regular.
// IR: 6·√(9)·10^6 = 6·3_000_000 = 18_000_000 = m·Δ·10^6. ✓
// RR: 6·10^6/9 = 6·111_111 = 666_666. (floor(10^6/9)=111_111) ✓
// Lz: each: 3²·(4−1−3)=0. lz=0. ✓

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T16_VEC_A, T16_KEY_A, T16_ID_A);
    add_node(T16_VEC_B, T16_KEY_B, T16_ID_B);
    add_node(T16_VEC_C, T16_KEY_C, T16_ID_C);
    add_node(T16_VEC_D, T16_KEY_D, T16_ID_D);
    add_edge(T16_ID_A, T16_ID_B, "t16.e.ab");
    add_edge(T16_ID_B, T16_ID_A, "t16.e.ba");
    add_edge(T16_ID_A, T16_ID_C, "t16.e.ac");
    add_edge(T16_ID_C, T16_ID_A, "t16.e.ca");
    add_edge(T16_ID_A, T16_ID_D, "t16.e.ad");
    add_edge(T16_ID_D, T16_ID_A, "t16.e.da");
    add_edge(T16_ID_B, T16_ID_C, "t16.e.bc");
    add_edge(T16_ID_C, T16_ID_B, "t16.e.cb");
    add_edge(T16_ID_B, T16_ID_D, "t16.e.bd");
    add_edge(T16_ID_D, T16_ID_B, "t16.e.db");
    add_edge(T16_ID_C, T16_ID_D, "t16.e.cd");
    add_edge(T16_ID_D, T16_ID_C, "t16.e.dc");

    let (ir, rr, lz, ec, nc) = gos_runtime::graph_topo_indices16();
    assert_eq!(nc,         4, "k4: node_count=4");
    assert_eq!(ec,         6, "k4: edge_count=6");
    assert_eq!(ir, 18_000_000, "k4: IR=18_000_000 (regular Δ=3: 6·3·10^6)");
    assert_eq!(rr,    666_666, "k4: RR=666_666 (6·floor(10^6/9)=6·111_111)");
    assert_eq!(lz,          0, "k4: Lz=0 (complete K₄: n−1−d=0)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges. d=0 for both. All indices zero.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T16_VEC_A, T16_KEY_A, T16_ID_A);
    add_node(T16_VEC_B, T16_KEY_B, T16_ID_B);

    let (ir, rr, lz, ec, nc) = gos_runtime::graph_topo_indices16();
    assert_eq!(nc, 2, "isolated: node_count=2");
    assert_eq!(ec, 0, "isolated: no edges");
    assert_eq!(ir, 0, "isolated: IR=0");
    assert_eq!(rr, 0, "isolated: RR=0");
    assert_eq!(lz, 0, "isolated: Lz=0 (d=0; 0²·0=0)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}:d=3. Right={C,D,E}:d=2. n=5.
// IR: 6·isqrt64(6e12)=6·2_449_489=14_696_934.
//     (√6=2.44948974…; isqrt64(6_000_000_000_000)=2_449_489)
// RR: 6·floor(10^6/6)=6·166_666=999_996.
// Lz: A,B:9·1=9 (×2=18); C,D,E:4·2=8 (×3=24). lz=42.
//     Identity: (5−1)·(9+9+4+4+4)−(27+27+8+8+8) = 4·30−78 = 120−78 = 42.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T16_VEC_A, T16_KEY_A, T16_ID_A);
    add_node(T16_VEC_B, T16_KEY_B, T16_ID_B);
    add_node(T16_VEC_C, T16_KEY_C, T16_ID_C);
    add_node(T16_VEC_D, T16_KEY_D, T16_ID_D);
    add_node(T16_VEC_E, T16_KEY_E, T16_ID_E);
    // Left A connects to all right
    add_edge(T16_ID_A, T16_ID_C, "t16.e.ac");
    add_edge(T16_ID_C, T16_ID_A, "t16.e.ca");
    add_edge(T16_ID_A, T16_ID_D, "t16.e.ad");
    add_edge(T16_ID_D, T16_ID_A, "t16.e.da");
    add_edge(T16_ID_A, T16_ID_E, "t16.e.ae");
    add_edge(T16_ID_E, T16_ID_A, "t16.e.ea");
    // Left B connects to all right
    add_edge(T16_ID_B, T16_ID_C, "t16.e.bc");
    add_edge(T16_ID_C, T16_ID_B, "t16.e.cb");
    add_edge(T16_ID_B, T16_ID_D, "t16.e.bd");
    add_edge(T16_ID_D, T16_ID_B, "t16.e.db");
    add_edge(T16_ID_B, T16_ID_E, "t16.e.be");
    add_edge(T16_ID_E, T16_ID_B, "t16.e.eb");

    let (ir, rr, lz, ec, nc) = gos_runtime::graph_topo_indices16();
    assert_eq!(nc,         5, "k23: node_count=5");
    assert_eq!(ec,         6, "k23: edge_count=6");
    assert_eq!(ir, 14_696_934, "k23: IR=14_696_934 (6·isqrt64(6e12)=6·2_449_489)");
    assert_eq!(rr,    999_996, "k23: RR=999_996 (6·floor(10^6/6)=6·166_666)");
    assert_eq!(lz,        42, "k23: Lz=42 (left:2×9; right:3×8)");
}
