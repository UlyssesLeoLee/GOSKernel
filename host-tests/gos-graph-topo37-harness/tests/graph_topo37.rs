// gos-graph-topo37-harness — V3.48 NUC + NHDC + NTSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices37()`:
//   Returns (nuc, nhdc, ntso, edge_count, node_count)
//   - nuc  = NUC(G)  = Σ_v S(v)^11                    (exact u64; S-Undecic vertex sum)
//   - nhdc = NHDC(G) = Σ_{uv∈E} (S_u+S_v)^10          (exact u64; S-Decic edge-sum)
//   - ntso = NTSO(G) = Σ_{uv∈E} (S_u²+S_v²)^5         (exact u64; S-Tenth Sombor, α=10)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NUC(G) = Σ_v S(v)^11
//     S-Undecic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37).
//     NUC = n·S^11 for S-regular.
//     Overflow: S^11 ≤ 16129^11 ≈ 4.2×10^45 > u128::MAX → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHDC(G) = Σ_{uv∈E} (S_u+S_v)^10
//     S-Decic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37).
//     NHDC = |E|·(2S)^10 = 1024|E|S^10 for S-regular.
//     Overflow per edge: (2×16129)^10 ≈ 5.6×10^44 > u128::MAX → saturating u128 accumulator.
//
//   NTSO(G) = Σ_{uv∈E} (S_u²+S_v²)^5
//     S-Tenth Sombor: generalised Sombor SO^α with α=10 on S-variant.
//     NSO(topo21)=Σ(S²+S²)^{1/2} (α=1), NCSO(topo33)=Σ(S²+S²)^{3/2} (α=3),
//     NFSO(topo34)=Σ(S²+S²)^2 (α=4), NHSO(topo35)=Σ(S²+S²)^3 (α=6),
//     NOSO(topo36)=Σ(S²+S²)^4 (α=8), NTSO(topo37)=Σ(S²+S²)^5 (α=10) — exact, no isqrt.
//     NTSO = |E|·(2S²)^5 = 32|E|S^10 for S-regular.
//     Overflow per edge: (2×16129²)^5 ≈ 3.8×10^43 > u128::MAX → saturating u128 accumulator.
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
//  Graph       NUC(exact)          NHDC(exact)         NTSO(exact)     edges  nodes
//  Empty                0                   0                   0          0      0
//  1 node               0                   0                   0          0      1
//  K₂                   2               1_024                  32          1      2
//  P₃               6_144           2_097_152              65_536          2      3
//  K₃          12_582_912       3_221_225_472         100_663_296          3      3
//  K_{1,4}     20_971_520       4_294_967_296         134_217_728          4      5
//  P₄             358_390          79_997_426           2_632_154          3      4
//  K₄     125_524_238_436  21_422_803_359_744     669_462_604_992          6      4
//  2 isolated           0                   0                   0          0      2
//  K_{2,3}  1_813_985_280     371_504_185_344      11_609_505_792          6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NUC:  1^11 + 1^11 = 2. ✓
//     NHDC: (1+1)^10 = 2^10 = 1_024. ✓
//     NTSO: (1²+1²)^5 = 2^5 = 32. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NUC:  3×2^11 = 3×2_048 = 6_144. ✓
//     NHDC: 2×(2+2)^10 = 2×4^10 = 2×1_048_576 = 2_097_152. ✓
//     NTSO: 2×(4+4)^5 = 2×8^5 = 2×32_768 = 65_536. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NUC:  3×4^11 = 3×4_194_304 = 12_582_912. ✓
//     NHDC: 3×(4+4)^10 = 3×8^10 = 3×1_073_741_824 = 3_221_225_472. ✓
//     NTSO: 3×(16+16)^5 = 3×32^5 = 3×33_554_432 = 100_663_296. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NUC:  5×4^11 = 5×4_194_304 = 20_971_520. ✓
//     NHDC: 4×8^10 = 4×1_073_741_824 = 4_294_967_296. ✓
//     NTSO: 4×32^5 = 4×33_554_432 = 134_217_728. ✓
//     Note: K₃ and K_{1,4} share S=4; same per-edge NHDC and NTSO; NUC differs by n.
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NUC:  2^11+3^11+3^11+2^11 = 2_048+177_147+177_147+2_048 = 358_390. ✓
//     NHDC: 5^10+6^10+5^10 = 9_765_625+60_466_176+9_765_625 = 79_997_426. ✓
//       (5^10=9_765_625; 6^10=60_466_176)
//     NTSO: 13^5+18^5+13^5 = 371_293+1_889_568+371_293 = 2_632_154. ✓
//       (S_A²+S_B²=4+9=13; 13^5=371_293; S_B²+S_C²=9+9=18; 18^5=1_889_568)
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NUC:  4×9^11 = 4×31_381_059_609 = 125_524_238_436. ✓
//       (9^5=59_049; 9^11=9^8×9^2×9=43_046_721×81×9=3_138... let's verify:
//        9^1=9; 9^2=81; 9^4=6_561; 9^8=43_046_721; 9^10=43_046_721×81=3_486_784_401;
//        9^11=3_486_784_401×9=31_381_059_609; 4×31_381_059_609=125_524_238_436)
//     NHDC: 6×18^10 = 6×3_570_467_226_624 = 21_422_803_359_744. ✓
//       (18^5=1_889_568; 18^10=1_889_568^2=3_570_467_226_624... let me verify:
//        18^2=324; 18^4=104_976; 18^5=1_889_568; 18^10=(18^5)^2=1_889_568^2;
//        1_889_568^2: ≈1.89×10^6×1.89×10^6≈3.57×10^12;
//        exact: 1_889_568×1_889_568=3_570_467_226_624)
//     NTSO: 6×(81+81)^5 = 6×162^5 = 6×111_577_100_832 = 669_462_604_992. ✓
//       (162^2=26_244; 162^4=26_244^2=688_747_536; 162^5=688_747_536×162=111_577_100_832)
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NUC:  5×6^11 = 5×362_797_056 = 1_813_985_280. ✓
//       (6^5=7_776; 6^10=7_776^2=60_466_176; 6^11=60_466_176×6=362_797_056)
//     NHDC: 6×12^10 = 6×61_917_364_224 = 371_504_185_344. ✓
//       (12^5=248_832; 12^10=248_832^2=61_917_364_224)
//     NTSO: 6×(36+36)^5 = 6×72^5 = 6×1_934_917_632 = 11_609_505_792. ✓
//       (72^2=5_184; 72^4=5_184^2=26_873_856; 72^5=26_873_856×72=1_934_917_632)
//
// S-REGULAR FORMULA VERIFICATION:
//   NUC  = n·S^11                         for S-regular ✓
//   NHDC = |E|·(2S)^10 = 1024|E|·S^10    for S-regular ✓
//   NTSO = |E|·(2S²)^5 = 32|E|·S^10      for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 1_024, 32, 1, 2)
//  4.  Path P₃ = A-B-C                   → (6_144, 2_097_152, 65_536, 2, 3)
//  5.  Triangle K₃                       → (12_582_912, 3_221_225_472, 100_663_296, 3, 3)
//  6.  Star K_{1,4}                      → (20_971_520, 4_294_967_296, 134_217_728, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (358_390, 79_997_426, 2_632_154, 3, 4)
//  8.  Complete K₄                       → (125_524_238_436, 21_422_803_359_744, 669_462_604_992, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (1_813_985_280, 371_504_185_344, 11_609_505_792, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T37_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_37");
const T37_EXEC:   ExecutorId = ExecutorId::from_ascii("t37.exec");

const T37_KEY_A: &str = "t37.alpha";
const T37_KEY_B: &str = "t37.beta";
const T37_KEY_C: &str = "t37.gamma";
const T37_KEY_D: &str = "t37.delta";
const T37_KEY_E: &str = "t37.epsilon";

const T37_ID_A: NodeId = derive_node_id(T37_PLUGIN, T37_KEY_A);
const T37_ID_B: NodeId = derive_node_id(T37_PLUGIN, T37_KEY_B);
const T37_ID_C: NodeId = derive_node_id(T37_PLUGIN, T37_KEY_C);
const T37_ID_D: NodeId = derive_node_id(T37_PLUGIN, T37_KEY_D);
const T37_ID_E: NodeId = derive_node_id(T37_PLUGIN, T37_KEY_E);

// L4=124 namespace for this harness.
const T37_VEC_A: VectorAddress = VectorAddress::new(124, 1, 1, 0);
const T37_VEC_B: VectorAddress = VectorAddress::new(124, 1, 2, 0);
const T37_VEC_C: VectorAddress = VectorAddress::new(124, 1, 3, 0);
const T37_VEC_D: VectorAddress = VectorAddress::new(124, 2, 1, 0);
const T37_VEC_E: VectorAddress = VectorAddress::new(124, 2, 2, 0);

const T37_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T37_PLUGIN,
    name:         "kl-graph-topo37-harness",
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
        executor_id:       T37_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T37_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T37_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nuc, nhdc, ntso, ec, nc) = gos_runtime::graph_topo_indices37();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(nuc,  0, "empty: NUC=0");
    assert_eq!(nhdc, 0, "empty: NHDC=0");
    assert_eq!(ntso, 0, "empty: NTSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NUC: 0^11=0; NHDC: no edges; NTSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T37_VEC_A, T37_KEY_A, T37_ID_A);

    let (nuc, nhdc, ntso, ec, nc) = gos_runtime::graph_topo_indices37();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(nuc,  0, "single: NUC=0 (S=0; 0^11=0)");
    assert_eq!(nhdc, 0, "single: NHDC=0 (no edges)");
    assert_eq!(ntso, 0, "single: NTSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NUC:  1^11+1^11 = 2.
// NHDC: (1+1)^10 = 2^10 = 1_024.
// NTSO: (1²+1²)^5 = 2^5 = 32.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T37_VEC_A, T37_KEY_A, T37_ID_A);
    add_node(T37_VEC_B, T37_KEY_B, T37_ID_B);
    add_edge(T37_ID_A, T37_ID_B, "t37.e.ab");

    let (nuc, nhdc, ntso, ec, nc) = gos_runtime::graph_topo_indices37();
    assert_eq!(nc,   2,     "k2: node_count=2");
    assert_eq!(ec,   1,     "k2: edge_count=1");
    assert_eq!(nuc,  2,     "k2: NUC=2 (1\u{00b9}\u{00b9}+1\u{00b9}\u{00b9}=2; S-uniform S=1)");
    assert_eq!(nhdc, 1_024, "k2: NHDC=1_024 ((1+1)\u{00b9}\u{2070}=2\u{00b9}\u{2070}=1_024; S-uniform S=1)");
    assert_eq!(ntso, 32,    "k2: NTSO=32 ((1\u{00b2}+1\u{00b2})\u{2075}=2\u{2075}=32; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NUC:  3×2^11 = 3×2_048 = 6_144.
// NHDC: 2×(2+2)^10 = 2×4^10 = 2×1_048_576 = 2_097_152.
// NTSO: 2×(4+4)^5 = 2×8^5 = 2×32_768 = 65_536.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T37_VEC_A, T37_KEY_A, T37_ID_A);
    add_node(T37_VEC_B, T37_KEY_B, T37_ID_B);
    add_node(T37_VEC_C, T37_KEY_C, T37_ID_C);
    add_edge(T37_ID_A, T37_ID_B, "t37.e.ab");
    add_edge(T37_ID_B, T37_ID_C, "t37.e.bc");

    let (nuc, nhdc, ntso, ec, nc) = gos_runtime::graph_topo_indices37();
    assert_eq!(nc,   3,         "p3: node_count=3");
    assert_eq!(ec,   2,         "p3: edge_count=2");
    assert_eq!(nuc,  6_144,     "p3: NUC=6_144 (3\u{00d7}2_048; 2\u{00b9}\u{00b9}=2_048; S-uniform S=2)");
    assert_eq!(nhdc, 2_097_152, "p3: NHDC=2_097_152 (2\u{00d7}1_048_576; (2+2)\u{00b9}\u{2070}=4\u{00b9}\u{2070}=1_048_576; S-uniform S=2)");
    assert_eq!(ntso, 65_536,    "p3: NTSO=65_536 (2\u{00d7}32_768; (4+4)\u{2075}=8\u{2075}=32_768; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NUC:  3×4^11 = 3×4_194_304 = 12_582_912.
// NHDC: 3×(4+4)^10 = 3×8^10 = 3×1_073_741_824 = 3_221_225_472.
// NTSO: 3×(16+16)^5 = 3×32^5 = 3×33_554_432 = 100_663_296.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T37_VEC_A, T37_KEY_A, T37_ID_A);
    add_node(T37_VEC_B, T37_KEY_B, T37_ID_B);
    add_node(T37_VEC_C, T37_KEY_C, T37_ID_C);
    add_edge(T37_ID_A, T37_ID_B, "t37.e.ab");
    add_edge(T37_ID_B, T37_ID_A, "t37.e.ba");
    add_edge(T37_ID_B, T37_ID_C, "t37.e.bc");
    add_edge(T37_ID_C, T37_ID_B, "t37.e.cb");
    add_edge(T37_ID_A, T37_ID_C, "t37.e.ac");
    add_edge(T37_ID_C, T37_ID_A, "t37.e.ca");

    let (nuc, nhdc, ntso, ec, nc) = gos_runtime::graph_topo_indices37();
    assert_eq!(nc,   3,             "k3: node_count=3");
    assert_eq!(ec,   3,             "k3: edge_count=3");
    assert_eq!(nuc,  12_582_912,    "k3: NUC=12_582_912 (3\u{00d7}4_194_304; 4\u{00b9}\u{00b9}=4_194_304; S-uniform S=4)");
    assert_eq!(nhdc, 3_221_225_472, "k3: NHDC=3_221_225_472 (3\u{00d7}1_073_741_824; (4+4)\u{00b9}\u{2070}=8\u{00b9}\u{2070}=1_073_741_824; S-uniform S=4)");
    assert_eq!(ntso, 100_663_296,   "k3: NTSO=100_663_296 (3\u{00d7}33_554_432; (16+16)\u{2075}=32\u{2075}=33_554_432; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-edge NHDC (1_073_741_824) and NTSO (33_554_432) as K₃; NUC and totals differ.
// NUC:  5×4^11 = 5×4_194_304 = 20_971_520.
// NHDC: 4×8^10 = 4×1_073_741_824 = 4_294_967_296.
// NTSO: 4×32^5 = 4×33_554_432 = 134_217_728.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T37_VEC_A, T37_KEY_A, T37_ID_A);
    add_node(T37_VEC_B, T37_KEY_B, T37_ID_B);
    add_node(T37_VEC_C, T37_KEY_C, T37_ID_C);
    add_node(T37_VEC_D, T37_KEY_D, T37_ID_D);
    add_node(T37_VEC_E, T37_KEY_E, T37_ID_E);
    add_edge(T37_ID_A, T37_ID_B, "t37.e.ab");
    add_edge(T37_ID_A, T37_ID_C, "t37.e.ac");
    add_edge(T37_ID_A, T37_ID_D, "t37.e.ad");
    add_edge(T37_ID_A, T37_ID_E, "t37.e.ae");

    let (nuc, nhdc, ntso, ec, nc) = gos_runtime::graph_topo_indices37();
    assert_eq!(nc,   5,             "star: node_count=5");
    assert_eq!(ec,   4,             "star: edge_count=4");
    assert_eq!(nuc,  20_971_520,    "star: NUC=20_971_520 (5\u{00d7}4_194_304; same S as K\u{2083})");
    assert_eq!(nhdc, 4_294_967_296, "star: NHDC=4_294_967_296 (4\u{00d7}1_073_741_824; same per-edge as K\u{2083})");
    assert_eq!(ntso, 134_217_728,   "star: NTSO=134_217_728 (4\u{00d7}33_554_432; same per-edge as K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NUC:  2^11+3^11+3^11+2^11 = 2_048+177_147+177_147+2_048 = 358_390.
// NHDC: (2+3)^10+(3+3)^10+(3+2)^10 = 5^10+6^10+5^10 = 9_765_625+60_466_176+9_765_625 = 79_997_426.
// NTSO: 13^5+18^5+13^5 = 371_293+1_889_568+371_293 = 2_632_154.
//   (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T37_VEC_A, T37_KEY_A, T37_ID_A);
    add_node(T37_VEC_B, T37_KEY_B, T37_ID_B);
    add_node(T37_VEC_C, T37_KEY_C, T37_ID_C);
    add_node(T37_VEC_D, T37_KEY_D, T37_ID_D);
    add_edge(T37_ID_A, T37_ID_B, "t37.e.ab");
    add_edge(T37_ID_B, T37_ID_C, "t37.e.bc");
    add_edge(T37_ID_C, T37_ID_D, "t37.e.cd");

    let (nuc, nhdc, ntso, ec, nc) = gos_runtime::graph_topo_indices37();
    assert_eq!(nc,   4,          "p4: node_count=4");
    assert_eq!(ec,   3,          "p4: edge_count=3");
    assert_eq!(nuc,  358_390,    "p4: NUC=358_390 (2_048+177_147+177_147+2_048; 2\u{00b9}\u{00b9}+3\u{00b9}\u{00b9}+3\u{00b9}\u{00b9}+2\u{00b9}\u{00b9})");
    assert_eq!(nhdc, 79_997_426, "p4: NHDC=79_997_426 (9_765_625+60_466_176+9_765_625; 5\u{00b9}\u{2070}+6\u{00b9}\u{2070}+5\u{00b9}\u{2070})");
    assert_eq!(ntso, 2_632_154,  "p4: NTSO=2_632_154 (371_293+1_889_568+371_293; 13\u{2075}+18\u{2075}+13\u{2075})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NUC:  4×9^11 = 4×31_381_059_609 = 125_524_238_436.
// NHDC: 6×18^10 = 6×3_570_467_226_624 = 21_422_803_359_744.
// NTSO: 6×(81+81)^5 = 6×162^5 = 6×111_577_100_832 = 669_462_604_992.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T37_VEC_A, T37_KEY_A, T37_ID_A);
    add_node(T37_VEC_B, T37_KEY_B, T37_ID_B);
    add_node(T37_VEC_C, T37_KEY_C, T37_ID_C);
    add_node(T37_VEC_D, T37_KEY_D, T37_ID_D);
    add_edge(T37_ID_A, T37_ID_B, "t37.e.ab");
    add_edge(T37_ID_B, T37_ID_A, "t37.e.ba");
    add_edge(T37_ID_A, T37_ID_C, "t37.e.ac");
    add_edge(T37_ID_C, T37_ID_A, "t37.e.ca");
    add_edge(T37_ID_A, T37_ID_D, "t37.e.ad");
    add_edge(T37_ID_D, T37_ID_A, "t37.e.da");
    add_edge(T37_ID_B, T37_ID_C, "t37.e.bc");
    add_edge(T37_ID_C, T37_ID_B, "t37.e.cb");
    add_edge(T37_ID_B, T37_ID_D, "t37.e.bd");
    add_edge(T37_ID_D, T37_ID_B, "t37.e.db");
    add_edge(T37_ID_C, T37_ID_D, "t37.e.cd");
    add_edge(T37_ID_D, T37_ID_C, "t37.e.dc");

    let (nuc, nhdc, ntso, ec, nc) = gos_runtime::graph_topo_indices37();
    assert_eq!(nc,   4,                    "k4: node_count=4");
    assert_eq!(ec,   6,                    "k4: edge_count=6");
    assert_eq!(nuc,  125_524_238_436,      "k4: NUC=125_524_238_436 (4\u{00d7}31_381_059_609; 9\u{00b9}\u{00b9}=31_381_059_609; S-uniform S=9)");
    assert_eq!(nhdc, 21_422_803_359_744,   "k4: NHDC=21_422_803_359_744 (6\u{00d7}3_570_467_226_624; 18\u{00b9}\u{2070}=3_570_467_226_624; S-uniform S=9)");
    assert_eq!(ntso, 669_462_604_992,      "k4: NTSO=669_462_604_992 (6\u{00d7}111_577_100_832; 162\u{2075}=111_577_100_832; S-uniform S=9)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NUC=0; NHDC=0; NTSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T37_VEC_A, T37_KEY_A, T37_ID_A);
    add_node(T37_VEC_B, T37_KEY_B, T37_ID_B);

    let (nuc, nhdc, ntso, ec, nc) = gos_runtime::graph_topo_indices37();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(nuc,  0, "isolated: NUC=0 (S=0; 0^11=0)");
    assert_eq!(nhdc, 0, "isolated: NHDC=0 (no edges)");
    assert_eq!(ntso, 0, "isolated: NTSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NUC:  5×6^11 = 5×362_797_056 = 1_813_985_280.
// NHDC: 6×12^10 = 6×61_917_364_224 = 371_504_185_344.
// NTSO: 6×(36+36)^5 = 6×72^5 = 6×1_934_917_632 = 11_609_505_792.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T37_VEC_A, T37_KEY_A, T37_ID_A);
    add_node(T37_VEC_B, T37_KEY_B, T37_ID_B);
    add_node(T37_VEC_C, T37_KEY_C, T37_ID_C);
    add_node(T37_VEC_D, T37_KEY_D, T37_ID_D);
    add_node(T37_VEC_E, T37_KEY_E, T37_ID_E);
    add_edge(T37_ID_A, T37_ID_C, "t37.e.ac");
    add_edge(T37_ID_C, T37_ID_A, "t37.e.ca");
    add_edge(T37_ID_A, T37_ID_D, "t37.e.ad");
    add_edge(T37_ID_D, T37_ID_A, "t37.e.da");
    add_edge(T37_ID_A, T37_ID_E, "t37.e.ae");
    add_edge(T37_ID_E, T37_ID_A, "t37.e.ea");
    add_edge(T37_ID_B, T37_ID_C, "t37.e.bc");
    add_edge(T37_ID_C, T37_ID_B, "t37.e.cb");
    add_edge(T37_ID_B, T37_ID_D, "t37.e.bd");
    add_edge(T37_ID_D, T37_ID_B, "t37.e.db");
    add_edge(T37_ID_B, T37_ID_E, "t37.e.be");
    add_edge(T37_ID_E, T37_ID_B, "t37.e.eb");

    let (nuc, nhdc, ntso, ec, nc) = gos_runtime::graph_topo_indices37();
    assert_eq!(nc,   5,               "k23: node_count=5");
    assert_eq!(ec,   6,               "k23: edge_count=6");
    assert_eq!(nuc,  1_813_985_280,   "k23: NUC=1_813_985_280 (5\u{00d7}362_797_056; 6\u{00b9}\u{00b9}=362_797_056; S-uniform S=6)");
    assert_eq!(nhdc, 371_504_185_344, "k23: NHDC=371_504_185_344 (6\u{00d7}61_917_364_224; 12\u{00b9}\u{2070}=61_917_364_224; S-uniform S=6)");
    assert_eq!(ntso, 11_609_505_792,  "k23: NTSO=11_609_505_792 (6\u{00d7}1_934_917_632; 72\u{2075}=1_934_917_632; S-uniform S=6)");
}
