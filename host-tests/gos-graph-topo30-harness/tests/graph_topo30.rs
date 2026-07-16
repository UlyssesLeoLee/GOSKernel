// gos-graph-topo30-harness — V3.41 NVQ + NRGS + NHCS (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices30()`:
//   Returns (nvq, nrgs_ppm, nhcs, edge_count, node_count)
//   - nvq      = NVQ(G)        = Σ_v S(v)^4                              (exact u64; S-Quartic vertex sum)
//   - nrgs_ppm = NRGS(G)×10^6 = Σ_{uv∈E} isqrt128((S_u·S_v)^3×10^12)  (floor ppm; S-Generalized Randić 3/2)
//   - nhcs     = NHCS(G)       = Σ_{uv∈E} (S_u+S_v)^3                   (exact u64; S-Cubic edge-sum)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NVQ(G)  = Σ_v S(v)^4
//     S-analogue of 4th-power vertex sum; extends NM₁=Σ S² (topo18) and NF=Σ S³ (topo22).
//     NVQ = n·S^4 for S-regular. S=0 isolated nodes contribute 0.
//
//   NRGS(G) = Σ_{uv∈E} (S_u·S_v)^{3/2}
//     S-analogue of Generalized Randić χ_{3/2}(G) = Σ_{uv∈E} (d_u·d_v)^{3/2}.
//     NRGS = |E|·S^3 for S-regular (since (S²)^{3/2} = S^3; exact when S perfect square).
//     K₃ and K_{1,4}: S-uniform S=4 → same per-edge NRGS=(4·4)^{3/2}=64; totals differ by |E|.
//
//   NHCS(G) = Σ_{uv∈E} (S_u+S_v)^3
//     S-analogue of cubic edge-sum; extends NHM₁=Σ(S+S)² (topo23) to 3rd power.
//     NHCS = |E|·(2S)^3 = 8|E|S^3 for S-regular.
//     K₃ and K_{1,4}: both S-uniform S=4; (4+4)^3=512; totals differ by |E|.
//
// IMPLEMENTATION FORMULAS (no float, no_std safe):
//   NVQ  per vertex = sv^2 * sv^2                            [u64; max S^4 ≤ 16129^4 ≈ 6.77×10^16 < u64::MAX]
//   NRGS per edge   = isqrt128((S_u·S_v)^3 × 10^12) as u64 [u128 intermediate; ≤ ~1.76×10^37 < u128::MAX]
//   NHCS per edge   = (S_u+S_v)^3                           [u64; max per-edge ≈ 3.36×10^13; sum ≤ 2.73×10^17]
//
// KEY INVARIANTS:
//   NVQ  = n·S^4 for S-regular.
//   NRGS = |E|·S^3·10^6 for S-regular (exact; see K₃/K₄/K_{1,4}/K_{2,3} below).
//   NHCS = 8·|E|·S^3 for S-regular.
//   K₃ and K_{1,4}: S-uniform S=4 → same per-edge NRGS (64_000_000) and NHCS (512); differ in NVQ and |E|.
//   K₄ (S=9) and K_{2,3} (S=6): different per-edge values; NRGS = |E|·S^3·10^6 for both.
//
// S VALUES PER GRAPH:
//   K₂        : S(A)=S(B)=1
//   P₃=A-B-C  : S(A)=S(B)=S(C)=2    → S-uniform S=2
//   K₃        : S(each)=4            → S-uniform S=4
//   K_{1,4}   : S(hub)=4, S(leaf)=4  → S-uniform S=4
//   P₄=A-B-C-D: S(A)=S(D)=2, S(B)=S(C)=3 → mixed S
//   K₄        : S(each)=9            → S-uniform S=9
//   K_{2,3}   : S(all)=6             → S-uniform S=6
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph         NVQ(exact)  NRGS(ppm)      NHCS(exact)   edges  nodes
//  Empty                  0            0              0       0      0
//  1 node                 0            0              0       0      1
//  Edge K₂                2    1_000_000              8       1      2
//  Path P₃               48   16_000_000            128       2      3
//  Triangle K₃          768  192_000_000          1_536       3      3
//  Star K_{1,4}       1_280  256_000_000          2_048       4      5
//  Path P₄              194   56_393_876            466       3      4
//  Complete K₄       26_244  4_374_000_000        34_992       6      4
//  2 isolated             0            0              0       0      2
//  K_{2,3}           6_480  1_296_000_000        10_368       6      5
//
// DERIVATIONS:
//
//   K₂ (S_A=S_B=1):
//     NVQ:  1^4 + 1^4 = 2. ✓
//     NRGS: isqrt128((1·1)^3·10^12) = isqrt128(10^12) = 10^6 = 1_000_000. ✓
//     NHCS: (1+1)^3 = 8. ✓
//
//   P₃ (S-uniform S=2, 3 nodes, 2 edges):
//     NVQ:  3 × 2^4 = 3 × 16 = 48. ✓
//     NRGS per edge: isqrt128((2·2)^3·10^12) = isqrt128(64·10^12) = 8·10^6 = 8_000_000.
//       (8·10^6)^2 = 64·10^12 exact. Total = 2 × 8_000_000 = 16_000_000. ✓
//     NHCS: 2 × (2+2)^3 = 2 × 64 = 128. ✓
//
//   K₃ (S-uniform S=4, 3 nodes, 3 edges):
//     NVQ:  3 × 4^4 = 3 × 256 = 768. ✓
//     NRGS per edge: isqrt128((4·4)^3·10^12) = isqrt128(4096·10^12).
//       √(4096·10^12) = 64·10^6 (exact: 64^2·10^12 = 4096·10^12). ✓
//       Total = 3 × 64_000_000 = 192_000_000. ✓
//     NHCS: 3 × (4+4)^3 = 3 × 512 = 1_536. ✓
//
//   K_{1,4} (S-uniform S=4, 5 nodes, 4 edges):
//     NVQ:  5 × 256 = 1_280. ✓
//     NRGS: 4 × 64_000_000 = 256_000_000 (same per-edge as K₃). ✓
//     NHCS: 4 × 512 = 2_048. ✓
//
//   P₄ (S_A=2, S_B=3, S_C=3, S_D=2; 3 edges):
//     NVQ:  2^4 + 3^4 + 3^4 + 2^4 = 16+81+81+16 = 194. ✓
//     NRGS:
//       {A,B} S=2,3: isqrt128((2·3)^3·10^12) = isqrt128(216·10^12).
//         √(216·10^12) = 14.6969...×10^6 → floor = 14_696_938.
//         Check: 14_696_938^2 = 215_999_986_575_844 ≤ 216·10^12;
//                14_696_939^2 = 216_000_015_969_721 > 216·10^12. ✓
//       {B,C} S=3,3: isqrt128((3·3)^3·10^12) = isqrt128(729·10^12).
//         √(729·10^12) = 27·10^6 (exact: 27^2·10^12 = 729·10^12). ✓
//         = 27_000_000.
//       {C,D} S=3,2: same as {A,B} = 14_696_938.
//       Total = 14_696_938 + 27_000_000 + 14_696_938 = 56_393_876. ✓
//     NHCS:
//       {A,B}: (2+3)^3 = 125.
//       {B,C}: (3+3)^3 = 216.
//       {C,D}: (3+2)^3 = 125.
//       Total = 125+216+125 = 466. ✓
//
//   K₄ (S-uniform S=9, 4 nodes, 6 edges):
//     NVQ:  4 × 9^4 = 4 × 6561 = 26_244. ✓
//     NRGS per edge: isqrt128((9·9)^3·10^12) = isqrt128(531441·10^12).
//       √(531441·10^12) = 729·10^6 (exact: 729^2·10^12 = 531441·10^12). ✓
//       Total = 6 × 729_000_000 = 4_374_000_000. ✓
//     NHCS: 6 × (9+9)^3 = 6 × 18^3 = 6 × 5832 = 34_992. ✓
//
//   K_{2,3} (S-uniform S=6, 5 nodes, 6 edges):
//     NVQ:  5 × 6^4 = 5 × 1296 = 6_480. ✓
//     NRGS per edge: isqrt128((6·6)^3·10^12) = isqrt128(46656·10^12).
//       √(46656·10^12) = 216·10^6 (exact: 216^2·10^12 = 46656·10^12). ✓
//       Total = 6 × 216_000_000 = 1_296_000_000. ✓
//     NHCS: 6 × (6+6)^3 = 6 × 12^3 = 6 × 1728 = 10_368. ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 1_000_000, 8, 1, 2)
//  4.  Path P₃ = A-B-C                   → (48, 16_000_000, 128, 2, 3)
//  5.  Triangle K₃                       → (768, 192_000_000, 1_536, 3, 3)
//  6.  Star K_{1,4}                      → (1_280, 256_000_000, 2_048, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (194, 56_393_876, 466, 3, 4)
//  8.  Complete K₄                       → (26_244, 4_374_000_000, 34_992, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (6_480, 1_296_000_000, 10_368, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T30_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_30");
const T30_EXEC:   ExecutorId = ExecutorId::from_ascii("t30.exec");

const T30_KEY_A: &str = "t30.alpha";
const T30_KEY_B: &str = "t30.beta";
const T30_KEY_C: &str = "t30.gamma";
const T30_KEY_D: &str = "t30.delta";
const T30_KEY_E: &str = "t30.epsilon";

const T30_ID_A: NodeId = derive_node_id(T30_PLUGIN, T30_KEY_A);
const T30_ID_B: NodeId = derive_node_id(T30_PLUGIN, T30_KEY_B);
const T30_ID_C: NodeId = derive_node_id(T30_PLUGIN, T30_KEY_C);
const T30_ID_D: NodeId = derive_node_id(T30_PLUGIN, T30_KEY_D);
const T30_ID_E: NodeId = derive_node_id(T30_PLUGIN, T30_KEY_E);

// L4=117 namespace for this harness.
const T30_VEC_A: VectorAddress = VectorAddress::new(117, 1, 1, 0);
const T30_VEC_B: VectorAddress = VectorAddress::new(117, 1, 2, 0);
const T30_VEC_C: VectorAddress = VectorAddress::new(117, 1, 3, 0);
const T30_VEC_D: VectorAddress = VectorAddress::new(117, 2, 1, 0);
const T30_VEC_E: VectorAddress = VectorAddress::new(117, 2, 2, 0);

const T30_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T30_PLUGIN,
    name:         "kl-graph-topo30-harness",
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
        executor_id:       T30_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T30_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T30_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nvq, nrgs, nhcs, ec, nc) = gos_runtime::graph_topo_indices30();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(nvq,  0, "empty: NVQ=0");
    assert_eq!(nrgs, 0, "empty: NRGS=0");
    assert_eq!(nhcs, 0, "empty: NHCS=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NVQ: 0^4=0; NRGS: no edges; NHCS: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T30_VEC_A, T30_KEY_A, T30_ID_A);

    let (nvq, nrgs, nhcs, ec, nc) = gos_runtime::graph_topo_indices30();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(nvq,  0, "single: NVQ=0 (S=0; 0^4=0)");
    assert_eq!(nrgs, 0, "single: NRGS=0 (no edges)");
    assert_eq!(nhcs, 0, "single: NHCS=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NVQ:  1^4 + 1^4 = 2.
// NRGS: isqrt128((1·1)^3·10^12)=isqrt128(10^12)=10^6=1_000_000.
// NHCS: (1+1)^3 = 8.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T30_VEC_A, T30_KEY_A, T30_ID_A);
    add_node(T30_VEC_B, T30_KEY_B, T30_ID_B);
    add_edge(T30_ID_A, T30_ID_B, "t30.e.ab");

    let (nvq, nrgs, nhcs, ec, nc) = gos_runtime::graph_topo_indices30();
    assert_eq!(nc,   2,         "k2: node_count=2");
    assert_eq!(ec,   1,         "k2: edge_count=1");
    assert_eq!(nvq,  2,         "k2: NVQ=2 (1\u{2074}+1\u{2074}; S-uniform S=1)");
    assert_eq!(nrgs, 1_000_000, "k2: NRGS=1_000_000 (isqrt128(10\u{00b9}\u{00b2})=10\u{2076}; (1\u{00b7}1)^{{3/2}}=1)");
    assert_eq!(nhcs, 8,         "k2: NHCS=8 ((1+1)\u{00b3}=8)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NVQ:  3×2^4=48. NRGS: 2×8_000_000=16_000_000. NHCS: 2×64=128.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T30_VEC_A, T30_KEY_A, T30_ID_A);
    add_node(T30_VEC_B, T30_KEY_B, T30_ID_B);
    add_node(T30_VEC_C, T30_KEY_C, T30_ID_C);
    add_edge(T30_ID_A, T30_ID_B, "t30.e.ab");
    add_edge(T30_ID_B, T30_ID_C, "t30.e.bc");

    let (nvq, nrgs, nhcs, ec, nc) = gos_runtime::graph_topo_indices30();
    assert_eq!(nc,   3,          "p3: node_count=3");
    assert_eq!(ec,   2,          "p3: edge_count=2");
    assert_eq!(nvq,  48,         "p3: NVQ=48 (3\u{00d7}16; 2\u{2074}=16; S-uniform S=2)");
    assert_eq!(nrgs, 16_000_000, "p3: NRGS=16_000_000 (2\u{00d7}8_000_000; (2\u{00b7}2)^{{3/2}}=8; per-edge=8_000_000)");
    assert_eq!(nhcs, 128,        "p3: NHCS=128 (2\u{00d7}64; (2+2)\u{00b3}=64; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NVQ:  3×256=768. NRGS: 3×64_000_000=192_000_000. NHCS: 3×512=1_536.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T30_VEC_A, T30_KEY_A, T30_ID_A);
    add_node(T30_VEC_B, T30_KEY_B, T30_ID_B);
    add_node(T30_VEC_C, T30_KEY_C, T30_ID_C);
    add_edge(T30_ID_A, T30_ID_B, "t30.e.ab");
    add_edge(T30_ID_B, T30_ID_A, "t30.e.ba");
    add_edge(T30_ID_B, T30_ID_C, "t30.e.bc");
    add_edge(T30_ID_C, T30_ID_B, "t30.e.cb");
    add_edge(T30_ID_A, T30_ID_C, "t30.e.ac");
    add_edge(T30_ID_C, T30_ID_A, "t30.e.ca");

    let (nvq, nrgs, nhcs, ec, nc) = gos_runtime::graph_topo_indices30();
    assert_eq!(nc,   3,           "k3: node_count=3");
    assert_eq!(ec,   3,           "k3: edge_count=3");
    assert_eq!(nvq,  768,         "k3: NVQ=768 (3\u{00d7}256; 4\u{2074}=256; S-uniform S=4)");
    assert_eq!(nrgs, 192_000_000, "k3: NRGS=192_000_000 (3\u{00d7}64_000_000; (4\u{00b7}4)^{{3/2}}=64; S-uniform S=4)");
    assert_eq!(nhcs, 1_536,       "k3: NHCS=1_536 (3\u{00d7}512; (4+4)\u{00b3}=512; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-edge NRGS and NHCS as K₃ (S-uniform S=4 coincidence); NVQ and totals differ.
// NVQ: 5×256=1_280. NRGS: 4×64_000_000=256_000_000. NHCS: 4×512=2_048.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T30_VEC_A, T30_KEY_A, T30_ID_A);
    add_node(T30_VEC_B, T30_KEY_B, T30_ID_B);
    add_node(T30_VEC_C, T30_KEY_C, T30_ID_C);
    add_node(T30_VEC_D, T30_KEY_D, T30_ID_D);
    add_node(T30_VEC_E, T30_KEY_E, T30_ID_E);
    add_edge(T30_ID_A, T30_ID_B, "t30.e.ab");
    add_edge(T30_ID_A, T30_ID_C, "t30.e.ac");
    add_edge(T30_ID_A, T30_ID_D, "t30.e.ad");
    add_edge(T30_ID_A, T30_ID_E, "t30.e.ae");

    let (nvq, nrgs, nhcs, ec, nc) = gos_runtime::graph_topo_indices30();
    assert_eq!(nc,   5,           "star: node_count=5");
    assert_eq!(ec,   4,           "star: edge_count=4");
    assert_eq!(nvq,  1_280,       "star: NVQ=1_280 (5\u{00d7}256; S-uniform S=4; more nodes than K\u{2083})");
    assert_eq!(nrgs, 256_000_000, "star: NRGS=256_000_000 (4\u{00d7}64_000_000; (4\u{00b7}4)^{{3/2}}=64; same per-edge as K\u{2083})");
    assert_eq!(nhcs, 2_048,       "star: NHCS=2_048 (4\u{00d7}512; same per-edge as K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NVQ:  16+81+81+16=194.
// NRGS: {A,B}=14_696_938 + {B,C}=27_000_000 + {C,D}=14_696_938 = 56_393_876.
// NHCS: 125+216+125=466.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T30_VEC_A, T30_KEY_A, T30_ID_A);
    add_node(T30_VEC_B, T30_KEY_B, T30_ID_B);
    add_node(T30_VEC_C, T30_KEY_C, T30_ID_C);
    add_node(T30_VEC_D, T30_KEY_D, T30_ID_D);
    add_edge(T30_ID_A, T30_ID_B, "t30.e.ab");
    add_edge(T30_ID_B, T30_ID_C, "t30.e.bc");
    add_edge(T30_ID_C, T30_ID_D, "t30.e.cd");

    let (nvq, nrgs, nhcs, ec, nc) = gos_runtime::graph_topo_indices30();
    assert_eq!(nc,   4,          "p4: node_count=4");
    assert_eq!(ec,   3,          "p4: edge_count=3");
    assert_eq!(nvq,  194,        "p4: NVQ=194 (16+81+81+16; S values 2,3,3,2)");
    assert_eq!(nrgs, 56_393_876, "p4: NRGS=56_393_876 (14_696_938+27_000_000+14_696_938; {{A,B}}=(6)^{{3/2}}\u{00d7}10\u{2076}; {{B,C}}=27_000_000)");
    assert_eq!(nhcs, 466,        "p4: NHCS=466 (125+216+125; (2+3)\u{00b3}+(3+3)\u{00b3}+(3+2)\u{00b3})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NVQ:  4×9^4=4×6561=26_244.
// NRGS: 6×729_000_000=4_374_000_000.  (729^2·10^12=531441·10^12; √=729·10^6; exact)
// NHCS: 6×18^3=6×5832=34_992.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T30_VEC_A, T30_KEY_A, T30_ID_A);
    add_node(T30_VEC_B, T30_KEY_B, T30_ID_B);
    add_node(T30_VEC_C, T30_KEY_C, T30_ID_C);
    add_node(T30_VEC_D, T30_KEY_D, T30_ID_D);
    add_edge(T30_ID_A, T30_ID_B, "t30.e.ab");
    add_edge(T30_ID_B, T30_ID_A, "t30.e.ba");
    add_edge(T30_ID_A, T30_ID_C, "t30.e.ac");
    add_edge(T30_ID_C, T30_ID_A, "t30.e.ca");
    add_edge(T30_ID_A, T30_ID_D, "t30.e.ad");
    add_edge(T30_ID_D, T30_ID_A, "t30.e.da");
    add_edge(T30_ID_B, T30_ID_C, "t30.e.bc");
    add_edge(T30_ID_C, T30_ID_B, "t30.e.cb");
    add_edge(T30_ID_B, T30_ID_D, "t30.e.bd");
    add_edge(T30_ID_D, T30_ID_B, "t30.e.db");
    add_edge(T30_ID_C, T30_ID_D, "t30.e.cd");
    add_edge(T30_ID_D, T30_ID_C, "t30.e.dc");

    let (nvq, nrgs, nhcs, ec, nc) = gos_runtime::graph_topo_indices30();
    assert_eq!(nc,   4,               "k4: node_count=4");
    assert_eq!(ec,   6,               "k4: edge_count=6");
    assert_eq!(nvq,  26_244,          "k4: NVQ=26_244 (4\u{00d7}6561; 9\u{2074}=6561; S-uniform S=9)");
    assert_eq!(nrgs, 4_374_000_000,   "k4: NRGS=4_374_000_000 (6\u{00d7}729_000_000; (9\u{00b7}9)^{{3/2}}=729; S-uniform S=9)");
    assert_eq!(nhcs, 34_992,          "k4: NHCS=34_992 (6\u{00d7}5832; 18\u{00b3}=5832; S-uniform S=9)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NVQ=0 (0^4=0); NRGS=0 (no edges); NHCS=0 (no edges).

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T30_VEC_A, T30_KEY_A, T30_ID_A);
    add_node(T30_VEC_B, T30_KEY_B, T30_ID_B);

    let (nvq, nrgs, nhcs, ec, nc) = gos_runtime::graph_topo_indices30();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(nvq,  0, "isolated: NVQ=0 (S=0; 0^4=0)");
    assert_eq!(nrgs, 0, "isolated: NRGS=0 (no edges)");
    assert_eq!(nhcs, 0, "isolated: NHCS=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NVQ:  5×6^4=5×1296=6_480.
// NRGS: 6×isqrt128((6·6)^3·10^12)=6×216_000_000=1_296_000_000. (216^2=46656 exact ✓)
// NHCS: 6×12^3=6×1728=10_368.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T30_VEC_A, T30_KEY_A, T30_ID_A);
    add_node(T30_VEC_B, T30_KEY_B, T30_ID_B);
    add_node(T30_VEC_C, T30_KEY_C, T30_ID_C);
    add_node(T30_VEC_D, T30_KEY_D, T30_ID_D);
    add_node(T30_VEC_E, T30_KEY_E, T30_ID_E);
    add_edge(T30_ID_A, T30_ID_C, "t30.e.ac");
    add_edge(T30_ID_C, T30_ID_A, "t30.e.ca");
    add_edge(T30_ID_A, T30_ID_D, "t30.e.ad");
    add_edge(T30_ID_D, T30_ID_A, "t30.e.da");
    add_edge(T30_ID_A, T30_ID_E, "t30.e.ae");
    add_edge(T30_ID_E, T30_ID_A, "t30.e.ea");
    add_edge(T30_ID_B, T30_ID_C, "t30.e.bc");
    add_edge(T30_ID_C, T30_ID_B, "t30.e.cb");
    add_edge(T30_ID_B, T30_ID_D, "t30.e.bd");
    add_edge(T30_ID_D, T30_ID_B, "t30.e.db");
    add_edge(T30_ID_B, T30_ID_E, "t30.e.be");
    add_edge(T30_ID_E, T30_ID_B, "t30.e.eb");

    let (nvq, nrgs, nhcs, ec, nc) = gos_runtime::graph_topo_indices30();
    assert_eq!(nc,   5,               "k23: node_count=5");
    assert_eq!(ec,   6,               "k23: edge_count=6");
    assert_eq!(nvq,  6_480,           "k23: NVQ=6_480 (5\u{00d7}1296; 6\u{2074}=1296; S-uniform S=6)");
    assert_eq!(nrgs, 1_296_000_000,   "k23: NRGS=1_296_000_000 (6\u{00d7}216_000_000; (6\u{00b7}6)^{{3/2}}=216; S-uniform S=6)");
    assert_eq!(nhcs, 10_368,          "k23: NHCS=10_368 (6\u{00d7}1728; 12\u{00b3}=1728; S-uniform S=6)");
}
