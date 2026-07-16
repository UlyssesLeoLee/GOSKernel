// gos-graph-topo40-harness — V3.51 NQTC + NHTC + NGSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices40()`:
//   Returns (nqtc, nhtc, ngso, edge_count, node_count)
//   - nqtc = NQTC(G) = Σ_v S(v)^14                   (exact u64; S-Tetradecic vertex sum)
//   - nhtc = NHTC(G) = Σ_{uv∈E} (S_u+S_v)^13         (exact u64; S-Tridecic edge-sum)
//   - ngso = NGSO(G) = Σ_{uv∈E} (S_u²+S_v²)^8        (exact u64; S-Hexadecic Sombor, α=16)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NQTC(G) = Σ_v S(v)^14
//     S-Tetradecic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40).
//     NQTC = n·S^14 for S-regular.
//     Overflow: S^14 ≤ 16129^14 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHTC(G) = Σ_{uv∈E} (S_u+S_v)^13
//     S-Tridecic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40).
//     NHTC = |E|·(2S)^13 = 8192|E|S^13 for S-regular.
//     Overflow per edge: (2×16129)^13 → saturating u128 accumulator.
//
//   NGSO(G) = Σ_{uv∈E} (S_u²+S_v²)^8
//     S-Hexadecic Sombor: generalised Sombor SO^α with α=16 on S-variant.
//     NSO(topo21)=Σ(S²+S²)^{1/2} (α=1), NCSO(topo33)=Σ(S²+S²)^{3/2} (α=3),
//     NFSO(topo34)=Σ(S²+S²)^2 (α=4), NHSO(topo35)=Σ(S²+S²)^3 (α=6),
//     NOSO(topo36)=Σ(S²+S²)^4 (α=8), NTSO(topo37)=Σ(S²+S²)^5 (α=10),
//     NDSO(topo38)=Σ(S²+S²)^6 (α=12), NESO(topo39)=Σ(S²+S²)^7 (α=14),
//     NGSO(topo40)=Σ(S²+S²)^8 (α=16) — exact, no isqrt.
//     NGSO = |E|·(2S²)^8 = 256|E|S^16 for S-regular.
//     Overflow per edge: (2×16129²)^8 → saturating u128 accumulator.
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
//  Graph       NQTC(exact)              NHTC(exact)               NGSO(exact)       edges  nodes
//  Empty                    0                        0                        0           0      0
//  1 node                   0                        0                        0           0      1
//  K₂                       2                    8_192                      256           1      2
//  P₃                  49_152              134_217_728               33_554_432           2      3
//  K₃             805_306_368        1_649_267_441_664        3_298_534_883_328           3      3
//  K_{1,4}      1_342_177_280        2_199_023_255_552        4_398_046_511_104           4      5
//  P₄               9_598_706           15_502_100_266           12_651_422_018           3      4
//  K₄      91_507_169_819_844  124_937_789_194_027_008    2_846_239_010_076_427_776       6      4
//  2 isolated               0                        0                        0           0      2
//  K_{2,3}    391_820_820_480      641_959_232_274_432    4_333_224_817_852_416           6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NQTC:  1^14 + 1^14 = 2. ✓
//     NHTC:  (1+1)^13 = 2^13 = 8_192. ✓
//     NGSO:  (1²+1²)^8 = 2^8 = 256. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NQTC:  3×2^14 = 3×16_384 = 49_152. ✓
//     NHTC:  2×(2+2)^13 = 2×4^13 = 2×67_108_864 = 134_217_728. ✓
//     NGSO:  2×(4+4)^8 = 2×8^8 = 2×16_777_216 = 33_554_432. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NQTC:  3×4^14 = 3×268_435_456 = 805_306_368. ✓
//       (4^7=16_384; 4^14=16_384²=268_435_456)
//     NHTC:  3×(4+4)^13 = 3×8^13 = 3×549_755_813_888 = 1_649_267_441_664. ✓
//       (8^12=68_719_476_736; 8^13=68_719_476_736×8=549_755_813_888)
//     NGSO:  3×(16+16)^8 = 3×32^8 = 3×1_099_511_627_776 = 3_298_534_883_328. ✓
//       (32^8=2^40=1_099_511_627_776)
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NQTC:  5×4^14 = 5×268_435_456 = 1_342_177_280. ✓
//     NHTC:  4×8^13 = 4×549_755_813_888 = 2_199_023_255_552. ✓
//     NGSO:  4×32^8 = 4×1_099_511_627_776 = 4_398_046_511_104. ✓
//     Note: K₃ and K_{1,4} share S=4; same per-edge NHTC and NGSO; NQTC differs by n.
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NQTC:  2^14+3^14+3^14+2^14 = 16_384+4_782_969+4_782_969+16_384 = 9_598_706. ✓
//       (3^7=2_187; 3^14=2_187²=4_782_969)
//     NHTC:  5^13+6^13+5^13 = 1_220_703_125+13_060_694_016+1_220_703_125 = 15_502_100_266. ✓
//       (5^12=244_140_625; 5^13=244_140_625×5=1_220_703_125)
//       (6^12=2_176_782_336; 6^13=2_176_782_336×6=13_060_694_016)
//     NGSO:  13^8+18^8+13^8 = 815_730_721+11_019_960_576+815_730_721 = 12_651_422_018. ✓
//       (S_A²+S_B²=4+9=13; 13^4=28_561; 13^8=28_561²=815_730_721)
//       (S_B²+S_C²=9+9=18; 18^4=104_976; 18^8=104_976²=11_019_960_576)
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NQTC:  4×9^14 = 4×22_876_792_454_961 = 91_507_169_819_844. ✓
//       (9^7=4_782_969; 9^14=4_782_969²=22_876_792_454_961)
//     NHTC:  6×18^13 = 6×20_822_964_865_671_168 = 124_937_789_194_027_008. ✓
//       (18^12=1_156_831_381_426_176; 18^13=1_156_831_381_426_176×18=20_822_964_865_671_168)
//     NGSO:  6×162^8 = 6×474_373_168_346_071_296 = 2_846_239_010_076_427_776. ✓
//       (162^7=2_928_229_434_235_008; 162^8=2_928_229_434_235_008×162=474_373_168_346_071_296)
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NQTC:  5×6^14 = 5×78_364_164_096 = 391_820_820_480. ✓
//       (6^7=279_936; 6^14=279_936²=78_364_164_096)
//     NHTC:  6×12^13 = 6×106_993_205_379_072 = 641_959_232_274_432. ✓
//       (12^12=8_916_100_448_256; 12^13=8_916_100_448_256×12=106_993_205_379_072)
//     NGSO:  6×72^8 = 6×722_204_136_308_736 = 4_333_224_817_852_416. ✓
//       (72^7=10_030_613_004_288; 72^8=10_030_613_004_288×72=722_204_136_308_736)
//
// S-REGULAR FORMULA VERIFICATION:
//   NQTC = n·S^14                           for S-regular ✓
//   NHTC = |E|·(2S)^13 = 8192|E|·S^13      for S-regular ✓
//   NGSO = |E|·(2S²)^8 = 256|E|·S^16       for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 8_192, 256, 1, 2)
//  4.  Path P₃ = A-B-C                   → (49_152, 134_217_728, 33_554_432, 2, 3)
//  5.  Triangle K₃                       → (805_306_368, 1_649_267_441_664, 3_298_534_883_328, 3, 3)
//  6.  Star K_{1,4}                      → (1_342_177_280, 2_199_023_255_552, 4_398_046_511_104, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (9_598_706, 15_502_100_266, 12_651_422_018, 3, 4)
//  8.  Complete K₄                       → (91_507_169_819_844, 124_937_789_194_027_008, 2_846_239_010_076_427_776, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (391_820_820_480, 641_959_232_274_432, 4_333_224_817_852_416, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T40_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_40");
const T40_EXEC:   ExecutorId = ExecutorId::from_ascii("t40.exec");

const T40_KEY_A: &str = "t40.alpha";
const T40_KEY_B: &str = "t40.beta";
const T40_KEY_C: &str = "t40.gamma";
const T40_KEY_D: &str = "t40.delta";
const T40_KEY_E: &str = "t40.epsilon";

const T40_ID_A: NodeId = derive_node_id(T40_PLUGIN, T40_KEY_A);
const T40_ID_B: NodeId = derive_node_id(T40_PLUGIN, T40_KEY_B);
const T40_ID_C: NodeId = derive_node_id(T40_PLUGIN, T40_KEY_C);
const T40_ID_D: NodeId = derive_node_id(T40_PLUGIN, T40_KEY_D);
const T40_ID_E: NodeId = derive_node_id(T40_PLUGIN, T40_KEY_E);

// L4=127 namespace for this harness.
const T40_VEC_A: VectorAddress = VectorAddress::new(127, 1, 1, 0);
const T40_VEC_B: VectorAddress = VectorAddress::new(127, 1, 2, 0);
const T40_VEC_C: VectorAddress = VectorAddress::new(127, 1, 3, 0);
const T40_VEC_D: VectorAddress = VectorAddress::new(127, 2, 1, 0);
const T40_VEC_E: VectorAddress = VectorAddress::new(127, 2, 2, 0);

const T40_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T40_PLUGIN,
    name:         "kl-graph-topo40-harness",
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
        executor_id:       T40_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T40_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T40_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nqtc, nhtc, ngso, ec, nc) = gos_runtime::graph_topo_indices40();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(nqtc, 0, "empty: NQTC=0");
    assert_eq!(nhtc, 0, "empty: NHTC=0");
    assert_eq!(ngso, 0, "empty: NGSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NQTC: 0^14=0; NHTC: no edges; NGSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T40_VEC_A, T40_KEY_A, T40_ID_A);

    let (nqtc, nhtc, ngso, ec, nc) = gos_runtime::graph_topo_indices40();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(nqtc, 0, "single: NQTC=0 (S=0; 0^14=0)");
    assert_eq!(nhtc, 0, "single: NHTC=0 (no edges)");
    assert_eq!(ngso, 0, "single: NGSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NQTC: 1^14+1^14 = 2.
// NHTC: (1+1)^13 = 2^13 = 8_192.
// NGSO: (1²+1²)^8 = 2^8 = 256.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T40_VEC_A, T40_KEY_A, T40_ID_A);
    add_node(T40_VEC_B, T40_KEY_B, T40_ID_B);
    add_edge(T40_ID_A, T40_ID_B, "t40.e.ab");

    let (nqtc, nhtc, ngso, ec, nc) = gos_runtime::graph_topo_indices40();
    assert_eq!(nc,   2,     "k2: node_count=2");
    assert_eq!(ec,   1,     "k2: edge_count=1");
    assert_eq!(nqtc, 2,     "k2: NQTC=2 (1\u{00b9}\u{2074}+1\u{00b9}\u{2074}=2; S-uniform S=1)");
    assert_eq!(nhtc, 8_192, "k2: NHTC=8_192 ((1+1)\u{00b9}\u{00b3}=2\u{00b9}\u{00b3}=8_192; S-uniform S=1)");
    assert_eq!(ngso, 256,   "k2: NGSO=256 ((1\u{00b2}+1\u{00b2})\u{2078}=2\u{2078}=256; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NQTC: 3×2^14 = 3×16_384 = 49_152.
// NHTC: 2×(2+2)^13 = 2×4^13 = 2×67_108_864 = 134_217_728.
// NGSO: 2×(4+4)^8 = 2×8^8 = 2×16_777_216 = 33_554_432.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T40_VEC_A, T40_KEY_A, T40_ID_A);
    add_node(T40_VEC_B, T40_KEY_B, T40_ID_B);
    add_node(T40_VEC_C, T40_KEY_C, T40_ID_C);
    add_edge(T40_ID_A, T40_ID_B, "t40.e.ab");
    add_edge(T40_ID_B, T40_ID_C, "t40.e.bc");

    let (nqtc, nhtc, ngso, ec, nc) = gos_runtime::graph_topo_indices40();
    assert_eq!(nc,   3,           "p3: node_count=3");
    assert_eq!(ec,   2,           "p3: edge_count=2");
    assert_eq!(nqtc, 49_152,      "p3: NQTC=49_152 (3\u{00d7}16_384; 2\u{00b9}\u{2074}=16_384; S-uniform S=2)");
    assert_eq!(nhtc, 134_217_728, "p3: NHTC=134_217_728 (2\u{00d7}67_108_864; (2+2)\u{00b9}\u{00b3}=4\u{00b9}\u{00b3}=67_108_864; S-uniform S=2)");
    assert_eq!(ngso, 33_554_432,  "p3: NGSO=33_554_432 (2\u{00d7}16_777_216; (4+4)\u{2078}=8\u{2078}=16_777_216; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NQTC: 3×4^14 = 3×268_435_456 = 805_306_368.
// NHTC: 3×(4+4)^13 = 3×8^13 = 3×549_755_813_888 = 1_649_267_441_664.
// NGSO: 3×(16+16)^8 = 3×32^8 = 3×1_099_511_627_776 = 3_298_534_883_328.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T40_VEC_A, T40_KEY_A, T40_ID_A);
    add_node(T40_VEC_B, T40_KEY_B, T40_ID_B);
    add_node(T40_VEC_C, T40_KEY_C, T40_ID_C);
    add_edge(T40_ID_A, T40_ID_B, "t40.e.ab");
    add_edge(T40_ID_B, T40_ID_A, "t40.e.ba");
    add_edge(T40_ID_B, T40_ID_C, "t40.e.bc");
    add_edge(T40_ID_C, T40_ID_B, "t40.e.cb");
    add_edge(T40_ID_A, T40_ID_C, "t40.e.ac");
    add_edge(T40_ID_C, T40_ID_A, "t40.e.ca");

    let (nqtc, nhtc, ngso, ec, nc) = gos_runtime::graph_topo_indices40();
    assert_eq!(nc,   3,                  "k3: node_count=3");
    assert_eq!(ec,   3,                  "k3: edge_count=3");
    assert_eq!(nqtc, 805_306_368,        "k3: NQTC=805_306_368 (3\u{00d7}268_435_456; 4\u{00b9}\u{2074}=268_435_456; S-uniform S=4)");
    assert_eq!(nhtc, 1_649_267_441_664,  "k3: NHTC=1_649_267_441_664 (3\u{00d7}549_755_813_888; (4+4)\u{00b9}\u{00b3}=8\u{00b9}\u{00b3}=549_755_813_888; S-uniform S=4)");
    assert_eq!(ngso, 3_298_534_883_328,  "k3: NGSO=3_298_534_883_328 (3\u{00d7}1_099_511_627_776; (16+16)\u{2078}=32\u{2078}=1_099_511_627_776; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-edge NHTC (549_755_813_888) and NGSO (1_099_511_627_776) as K₃; NQTC and totals differ.
// NQTC: 5×4^14 = 5×268_435_456 = 1_342_177_280.
// NHTC: 4×8^13 = 4×549_755_813_888 = 2_199_023_255_552.
// NGSO: 4×32^8 = 4×1_099_511_627_776 = 4_398_046_511_104.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T40_VEC_A, T40_KEY_A, T40_ID_A);
    add_node(T40_VEC_B, T40_KEY_B, T40_ID_B);
    add_node(T40_VEC_C, T40_KEY_C, T40_ID_C);
    add_node(T40_VEC_D, T40_KEY_D, T40_ID_D);
    add_node(T40_VEC_E, T40_KEY_E, T40_ID_E);
    add_edge(T40_ID_A, T40_ID_B, "t40.e.ab");
    add_edge(T40_ID_A, T40_ID_C, "t40.e.ac");
    add_edge(T40_ID_A, T40_ID_D, "t40.e.ad");
    add_edge(T40_ID_A, T40_ID_E, "t40.e.ae");

    let (nqtc, nhtc, ngso, ec, nc) = gos_runtime::graph_topo_indices40();
    assert_eq!(nc,   5,                  "star: node_count=5");
    assert_eq!(ec,   4,                  "star: edge_count=4");
    assert_eq!(nqtc, 1_342_177_280,      "star: NQTC=1_342_177_280 (5\u{00d7}268_435_456; same S as K\u{2083})");
    assert_eq!(nhtc, 2_199_023_255_552,  "star: NHTC=2_199_023_255_552 (4\u{00d7}549_755_813_888; same per-edge as K\u{2083})");
    assert_eq!(ngso, 4_398_046_511_104,  "star: NGSO=4_398_046_511_104 (4\u{00d7}1_099_511_627_776; same per-edge as K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NQTC: 2^14+3^14+3^14+2^14 = 16_384+4_782_969+4_782_969+16_384 = 9_598_706.
// NHTC: (2+3)^13+(3+3)^13+(3+2)^13 = 5^13+6^13+5^13
//       = 1_220_703_125+13_060_694_016+1_220_703_125 = 15_502_100_266.
// NGSO: 13^8+18^8+13^8 = 815_730_721+11_019_960_576+815_730_721 = 12_651_422_018.
//   (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T40_VEC_A, T40_KEY_A, T40_ID_A);
    add_node(T40_VEC_B, T40_KEY_B, T40_ID_B);
    add_node(T40_VEC_C, T40_KEY_C, T40_ID_C);
    add_node(T40_VEC_D, T40_KEY_D, T40_ID_D);
    add_edge(T40_ID_A, T40_ID_B, "t40.e.ab");
    add_edge(T40_ID_B, T40_ID_C, "t40.e.bc");
    add_edge(T40_ID_C, T40_ID_D, "t40.e.cd");

    let (nqtc, nhtc, ngso, ec, nc) = gos_runtime::graph_topo_indices40();
    assert_eq!(nc,   4,              "p4: node_count=4");
    assert_eq!(ec,   3,              "p4: edge_count=3");
    assert_eq!(nqtc, 9_598_706,      "p4: NQTC=9_598_706 (16_384+4_782_969+4_782_969+16_384; 2\u{00b9}\u{2074}+3\u{00b9}\u{2074}+3\u{00b9}\u{2074}+2\u{00b9}\u{2074})");
    assert_eq!(nhtc, 15_502_100_266, "p4: NHTC=15_502_100_266 (1_220_703_125+13_060_694_016+1_220_703_125; 5\u{00b9}\u{00b3}+6\u{00b9}\u{00b3}+5\u{00b9}\u{00b3})");
    assert_eq!(ngso, 12_651_422_018, "p4: NGSO=12_651_422_018 (815_730_721+11_019_960_576+815_730_721; 13\u{2078}+18\u{2078}+13\u{2078})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NQTC: 4×9^14 = 4×22_876_792_454_961 = 91_507_169_819_844.
// NHTC: 6×18^13 = 6×20_822_964_865_671_168 = 124_937_789_194_027_008.
// NGSO: 6×162^8 = 6×474_373_168_346_071_296 = 2_846_239_010_076_427_776.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T40_VEC_A, T40_KEY_A, T40_ID_A);
    add_node(T40_VEC_B, T40_KEY_B, T40_ID_B);
    add_node(T40_VEC_C, T40_KEY_C, T40_ID_C);
    add_node(T40_VEC_D, T40_KEY_D, T40_ID_D);
    add_edge(T40_ID_A, T40_ID_B, "t40.e.ab");
    add_edge(T40_ID_B, T40_ID_A, "t40.e.ba");
    add_edge(T40_ID_A, T40_ID_C, "t40.e.ac");
    add_edge(T40_ID_C, T40_ID_A, "t40.e.ca");
    add_edge(T40_ID_A, T40_ID_D, "t40.e.ad");
    add_edge(T40_ID_D, T40_ID_A, "t40.e.da");
    add_edge(T40_ID_B, T40_ID_C, "t40.e.bc");
    add_edge(T40_ID_C, T40_ID_B, "t40.e.cb");
    add_edge(T40_ID_B, T40_ID_D, "t40.e.bd");
    add_edge(T40_ID_D, T40_ID_B, "t40.e.db");
    add_edge(T40_ID_C, T40_ID_D, "t40.e.cd");
    add_edge(T40_ID_D, T40_ID_C, "t40.e.dc");

    let (nqtc, nhtc, ngso, ec, nc) = gos_runtime::graph_topo_indices40();
    assert_eq!(nc,   4,                          "k4: node_count=4");
    assert_eq!(ec,   6,                          "k4: edge_count=6");
    assert_eq!(nqtc, 91_507_169_819_844,         "k4: NQTC=91_507_169_819_844 (4\u{00d7}22_876_792_454_961; 9\u{00b9}\u{2074}=22_876_792_454_961; S-uniform S=9)");
    assert_eq!(nhtc, 124_937_789_194_027_008,    "k4: NHTC=124_937_789_194_027_008 (6\u{00d7}20_822_964_865_671_168; 18\u{00b9}\u{00b3}=20_822_964_865_671_168; S-uniform S=9)");
    assert_eq!(ngso, 2_846_239_010_076_427_776,  "k4: NGSO=2_846_239_010_076_427_776 (6\u{00d7}474_373_168_346_071_296; 162\u{2078}=474_373_168_346_071_296; S-uniform S=9)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NQTC=0; NHTC=0; NGSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T40_VEC_A, T40_KEY_A, T40_ID_A);
    add_node(T40_VEC_B, T40_KEY_B, T40_ID_B);

    let (nqtc, nhtc, ngso, ec, nc) = gos_runtime::graph_topo_indices40();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(nqtc, 0, "isolated: NQTC=0 (S=0; 0^14=0)");
    assert_eq!(nhtc, 0, "isolated: NHTC=0 (no edges)");
    assert_eq!(ngso, 0, "isolated: NGSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NQTC: 5×6^14 = 5×78_364_164_096 = 391_820_820_480.
// NHTC: 6×12^13 = 6×106_993_205_379_072 = 641_959_232_274_432.
// NGSO: 6×72^8 = 6×722_204_136_308_736 = 4_333_224_817_852_416.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T40_VEC_A, T40_KEY_A, T40_ID_A);
    add_node(T40_VEC_B, T40_KEY_B, T40_ID_B);
    add_node(T40_VEC_C, T40_KEY_C, T40_ID_C);
    add_node(T40_VEC_D, T40_KEY_D, T40_ID_D);
    add_node(T40_VEC_E, T40_KEY_E, T40_ID_E);
    add_edge(T40_ID_A, T40_ID_C, "t40.e.ac");
    add_edge(T40_ID_C, T40_ID_A, "t40.e.ca");
    add_edge(T40_ID_A, T40_ID_D, "t40.e.ad");
    add_edge(T40_ID_D, T40_ID_A, "t40.e.da");
    add_edge(T40_ID_A, T40_ID_E, "t40.e.ae");
    add_edge(T40_ID_E, T40_ID_A, "t40.e.ea");
    add_edge(T40_ID_B, T40_ID_C, "t40.e.bc");
    add_edge(T40_ID_C, T40_ID_B, "t40.e.cb");
    add_edge(T40_ID_B, T40_ID_D, "t40.e.bd");
    add_edge(T40_ID_D, T40_ID_B, "t40.e.db");
    add_edge(T40_ID_B, T40_ID_E, "t40.e.be");
    add_edge(T40_ID_E, T40_ID_B, "t40.e.eb");

    let (nqtc, nhtc, ngso, ec, nc) = gos_runtime::graph_topo_indices40();
    assert_eq!(nc,   5,                    "k23: node_count=5");
    assert_eq!(ec,   6,                    "k23: edge_count=6");
    assert_eq!(nqtc, 391_820_820_480,      "k23: NQTC=391_820_820_480 (5\u{00d7}78_364_164_096; 6\u{00b9}\u{2074}=78_364_164_096; S-uniform S=6)");
    assert_eq!(nhtc, 641_959_232_274_432,  "k23: NHTC=641_959_232_274_432 (6\u{00d7}106_993_205_379_072; 12\u{00b9}\u{00b3}=106_993_205_379_072; S-uniform S=6)");
    assert_eq!(ngso, 4_333_224_817_852_416,"k23: NGSO=4_333_224_817_852_416 (6\u{00d7}722_204_136_308_736; 72\u{2078}=722_204_136_308_736; S-uniform S=6)");
}
