// gos-graph-topo56-harness — V3.67 NTRIACTC + NHTRIACTC + NASO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices56()`:
//   Returns (ntriactc, nhtriactc, naso, edge_count, node_count)
//   - ntriactc  = NTRIACTC(G)  = Σ_v S(v)^30                   (exact u64; S-Triacontyl vertex sum)
//   - nhtriactc = NHTRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^29         (exact u64; S-Nonacosic edge-sum)
//   - naso      = NASO(G)      = Σ_{uv∈E} (S_u²+S_v²)^24       (exact u64; S-Octatetracontyl Sombor, α=48)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NTRIACTC(G) = Σ_v S(v)^30
//     S-Triacontyl vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48),
//       NTRICTC=Σ S²³ (topo49), NTETRTC=Σ S²⁴ (topo50), NPENTTC=Σ S²⁵ (topo51),
//       NHEXATC=Σ S²⁶ (topo52), NHEPTATC=Σ S²⁷ (topo53), NOCTATC=Σ S²⁸ (topo54),
//       NNONATC=Σ S²⁹ (topo55), NTRIACTC=Σ S³⁰ (topo56).
//     NTRIACTC = n·S^30 for S-regular.
//     Overflow: S^30 ≤ 16129^30 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHTRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^29
//     S-Nonacosic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45), NHEICTC=Σ(S+S)¹⁹ (topo46),
//       NHHENTC=Σ(S+S)²⁰ (topo47), NHDOCTC=Σ(S+S)²¹ (topo48), NHTRICTC=Σ(S+S)²² (topo49),
//       NHTETRTC=Σ(S+S)²³ (topo50), NHPENTTC=Σ(S+S)²⁴ (topo51), NHHEXATC=Σ(S+S)²⁵ (topo52),
//       NHHEPTATC=Σ(S+S)²⁶ (topo53), NHOCTATC=Σ(S+S)²⁷ (topo54), NHNONATC=Σ(S+S)²⁸ (topo55),
//       NHTRIACTC=Σ(S+S)²⁹ (topo56).
//     NHTRIACTC = |E|·(2S)^29 = 536870912|E|·S^29 for S-regular.
//     Overflow per edge: (2×16129)^29 → saturating u128 accumulator.
//
//   NASO(G) = Σ_{uv∈E} (S_u²+S_v²)^24
//     S-Octatetracontyl Sombor: generalised Sombor SO^α with α=48 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42), NYSO(topo54,α=44), NZSO(topo55,α=46),
//     NASO(topo56,α=48).
//     NASO = |E|·(2S²)^24 = 16777216|E|·S^48 for S-regular.
//     Overflow per edge: (2×16129²)^24 → saturating u128 accumulator.
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
//  Graph     NTRIACTC(exact)               NHTRIACTC(exact)              NASO(exact)              edges  nodes
//  Empty                   0                             0                         0               0      0
//  1 node                  0                             0                         0               0      1
//  K₂                      2                   536_870_912                16_777_216               1      2
//  P₃           3_221_225_472       576_460_752_303_423_488           u64::MAX(sat.)              2      3
//  K₃   3_458_764_513_820_540_928       u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4} 5_764_607_523_034_234_880    u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄       411_784_411_672_946          u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄          u64::MAX(sat.)            u64::MAX(sat.)               u64::MAX(sat.)              6      4
//  2 isolated              0                             0                         0               0      2
//  K_{2,3}    u64::MAX(sat.)             u64::MAX(sat.)               u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NTRIACTC:  1^30 + 1^30 = 2. ✓
//     NHTRIACTC: (1+1)^29 = 2^29 = 536_870_912. ✓
//     NASO:      (1²+1²)^24 = 2^24 = 16_777_216. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NTRIACTC:  3×2^30 = 3×1_073_741_824 = 3_221_225_472. ✓
//     NHTRIACTC: 2×(2+2)^29 = 2×4^29 = 2×2^58 = 2^59 = 576_460_752_303_423_488. ✓
//       (4^29=2^58=288_230_376_151_711_744; 2×4^29=576_460_752_303_423_488)
//     NASO:      2×(4+4)^24 = 2×8^24 = 2×2^72 → SATURATES (8^24=2^72>u64::MAX per-edge). ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NTRIACTC:  3×4^30 = 3×2^60 = 3×1_152_921_504_606_846_976 = 3_458_764_513_820_540_928 (fits u64). ✓
//       (4^30=4^29×4=288_230_376_151_711_744×4=1_152_921_504_606_846_976; 3×2^60<2^64)
//     NHTRIACTC: 3×(4+4)^29 = 3×8^29 = 3×2^87 → SATURATES (per-edge >> u64::MAX). ✓
//     NASO:      3×(16+16)^24 = 3×32^24 = 3×2^120 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NTRIACTC:  5×4^30 = 5×1_152_921_504_606_846_976 = 5_764_607_523_034_234_880 (fits u64). ✓
//     NHTRIACTC: 4×8^29 → SATURATES. ✓
//     NASO:      4×32^24 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NTRIACTC:  2^30+3^30+3^30+2^30 = 2×1_073_741_824+2×205_891_132_094_649.
//       3^30=3^29×3=68_630_377_364_883×3=205_891_132_094_649
//       2×1_073_741_824+2×205_891_132_094_649=2_147_483_648+411_782_264_189_298=411_784_411_672_946. ✓
//     NHTRIACTC: (2+3)^29+(3+3)^29+(3+2)^29 = 2×5^29+6^29
//       5^29>>u64::MAX per-edge → SATURATES. ✓
//     NASO:      13^24+18^24+13^24 — 13^24>>u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NTRIACTC:  4×9^30 → 9^30>>u64::MAX → SATURATES. ✓
//     NHTRIACTC: → SATURATES. ✓
//     NASO:      → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NTRIACTC:  5×6^30 → 6^30>>u64::MAX per-node → SATURATES. ✓
//     NHTRIACTC: → SATURATES. ✓
//     NASO:      → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NTRIACTC  = n·S^30                                         for S-regular ✓
//   NHTRIACTC = |E|·(2S)^29 = 536870912|E|·S^29               for S-regular ✓
//   NASO      = |E|·(2S²)^24 = 16777216|E|·S^48               for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 536_870_912, 16_777_216, 1, 2)
//  4.  Path P₃ = A-B-C                   → (3_221_225_472, 576_460_752_303_423_488, u64::MAX, 2, 3)
//  5.  Triangle K₃                       → (3_458_764_513_820_540_928, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (5_764_607_523_034_234_880, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (411_784_411_672_946, u64::MAX, u64::MAX, 3, 4)
//  8.  Complete K₄                       → (u64::MAX, u64::MAX, u64::MAX, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (u64::MAX, u64::MAX, u64::MAX, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T56_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_56");
const T56_EXEC:   ExecutorId = ExecutorId::from_ascii("t56.exec");

const T56_KEY_A: &str = "t56.alpha";
const T56_KEY_B: &str = "t56.beta";
const T56_KEY_C: &str = "t56.gamma";
const T56_KEY_D: &str = "t56.delta";
const T56_KEY_E: &str = "t56.epsilon";

const T56_ID_A: NodeId = derive_node_id(T56_PLUGIN, T56_KEY_A);
const T56_ID_B: NodeId = derive_node_id(T56_PLUGIN, T56_KEY_B);
const T56_ID_C: NodeId = derive_node_id(T56_PLUGIN, T56_KEY_C);
const T56_ID_D: NodeId = derive_node_id(T56_PLUGIN, T56_KEY_D);
const T56_ID_E: NodeId = derive_node_id(T56_PLUGIN, T56_KEY_E);

// L4=143 namespace for this harness.
const T56_VEC_A: VectorAddress = VectorAddress::new(143, 1, 1, 0);
const T56_VEC_B: VectorAddress = VectorAddress::new(143, 1, 2, 0);
const T56_VEC_C: VectorAddress = VectorAddress::new(143, 1, 3, 0);
const T56_VEC_D: VectorAddress = VectorAddress::new(143, 2, 1, 0);
const T56_VEC_E: VectorAddress = VectorAddress::new(143, 2, 2, 0);

const T56_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T56_PLUGIN,
    name:         "kl-graph-topo56-harness",
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
        executor_id:       T56_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T56_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T56_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (ntriactc, nhtriactc, naso, ec, nc) = gos_runtime::graph_topo_indices56();
    assert_eq!(nc,        0, "empty: node_count=0");
    assert_eq!(ec,        0, "empty: edge_count=0");
    assert_eq!(ntriactc,  0, "empty: NTRIACTC=0");
    assert_eq!(nhtriactc, 0, "empty: NHTRIACTC=0");
    assert_eq!(naso,      0, "empty: NASO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T56_VEC_A, T56_KEY_A, T56_ID_A);

    let (ntriactc, nhtriactc, naso, ec, nc) = gos_runtime::graph_topo_indices56();
    assert_eq!(nc,        1, "single: node_count=1");
    assert_eq!(ec,        0, "single: edge_count=0");
    assert_eq!(ntriactc,  0, "single: NTRIACTC=0");
    assert_eq!(nhtriactc, 0, "single: NHTRIACTC=0");
    assert_eq!(naso,      0, "single: NASO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NTRIACTC:  1^30+1^30 = 2.
// NHTRIACTC: (1+1)^29 = 2^29 = 536_870_912.
// NASO:      (1²+1²)^24 = 2^24 = 16_777_216.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T56_VEC_A, T56_KEY_A, T56_ID_A);
    add_node(T56_VEC_B, T56_KEY_B, T56_ID_B);
    add_edge(T56_ID_A, T56_ID_B, "t56.e.ab");

    let (ntriactc, nhtriactc, naso, ec, nc) = gos_runtime::graph_topo_indices56();
    assert_eq!(nc,        2,           "k2: node_count=2");
    assert_eq!(ec,        1,           "k2: edge_count=1");
    assert_eq!(ntriactc,  2,           "k2: NTRIACTC=2 (1\u{00b3}\u{2070}+1\u{00b3}\u{2070}=2)");
    assert_eq!(nhtriactc, 536_870_912, "k2: NHTRIACTC=536_870_912 (2\u{00b2}\u{2079}=2^29)");
    assert_eq!(naso,      16_777_216,  "k2: NASO=16_777_216 (2\u{00b2}\u{2074}=2^24)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NTRIACTC:  3×2^30 = 3×1_073_741_824 = 3_221_225_472.
// NHTRIACTC: 2×(2+2)^29 = 2×4^29 = 2×2^58 = 2^59 = 576_460_752_303_423_488.
// NASO:      2×(4+4)^24 = 2×8^24 = 2×2^72 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T56_VEC_A, T56_KEY_A, T56_ID_A);
    add_node(T56_VEC_B, T56_KEY_B, T56_ID_B);
    add_node(T56_VEC_C, T56_KEY_C, T56_ID_C);
    add_edge(T56_ID_A, T56_ID_B, "t56.e.ab");
    add_edge(T56_ID_B, T56_ID_C, "t56.e.bc");

    let (ntriactc, nhtriactc, naso, ec, nc) = gos_runtime::graph_topo_indices56();
    assert_eq!(nc,        3,                         "p3: node_count=3");
    assert_eq!(ec,        2,                         "p3: edge_count=2");
    assert_eq!(ntriactc,  3_221_225_472,             "p3: NTRIACTC=3_221_225_472 (3\u{00d7}2\u{00b3}\u{2070})");
    assert_eq!(nhtriactc, 576_460_752_303_423_488,   "p3: NHTRIACTC=576_460_752_303_423_488 (2\u{00d7}4\u{00b2}\u{2079}=2^59)");
    assert_eq!(naso,      u64::MAX,                  "p3: NASO=u64::MAX (8\u{00b2}\u{2074}=2^72>u64::MAX per-edge; saturated)");
}

// ── Test 5: Triangle K₃ ─────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NTRIACTC:  3×4^30 = 3×2^60 = 3_458_764_513_820_540_928 (fits u64).
// NHTRIACTC: 3×(4+4)^29 = 3×8^29 = 3×2^87 → SATURATES.
// NASO:      3×(16+16)^24 = 3×32^24 = 3×2^120 → SATURATES.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T56_VEC_A, T56_KEY_A, T56_ID_A);
    add_node(T56_VEC_B, T56_KEY_B, T56_ID_B);
    add_node(T56_VEC_C, T56_KEY_C, T56_ID_C);
    add_edge(T56_ID_A, T56_ID_B, "t56.e.ab");
    add_edge(T56_ID_B, T56_ID_A, "t56.e.ba");
    add_edge(T56_ID_B, T56_ID_C, "t56.e.bc");
    add_edge(T56_ID_C, T56_ID_B, "t56.e.cb");
    add_edge(T56_ID_A, T56_ID_C, "t56.e.ac");
    add_edge(T56_ID_C, T56_ID_A, "t56.e.ca");

    let (ntriactc, nhtriactc, naso, ec, nc) = gos_runtime::graph_topo_indices56();
    assert_eq!(nc,        3,                           "k3: node_count=3");
    assert_eq!(ec,        3,                           "k3: edge_count=3");
    assert_eq!(ntriactc,  3_458_764_513_820_540_928,   "k3: NTRIACTC=3_458_764_513_820_540_928 (3\u{00d7}4\u{00b3}\u{2070}=3\u{00d7}2^60)");
    assert_eq!(nhtriactc, u64::MAX,                    "k3: NHTRIACTC=u64::MAX (3\u{00d7}8\u{00b2}\u{2079}=3\u{00d7}2^87>>u64::MAX; saturated)");
    assert_eq!(naso,      u64::MAX,                    "k3: NASO=u64::MAX (3\u{00d7}32\u{00b2}\u{2074}>>u64::MAX; saturated)");
}

// ── Test 6: Star K_{1,4} ────────────────────────────────────────────────────
// Center A: d=4. Leaves B,C,D,E: d=1.
// S(center)=4×1=4. S(leaf)=1×4=4. S-uniform S=4. 4 edges, 5 nodes.
// NTRIACTC:  5×4^30 = 5×2^60 = 5_764_607_523_034_234_880 (fits u64).
// NHTRIACTC: 4×(4+4)^29 = 4×8^29 → SATURATES.
// NASO:      4×(16+16)^24 → SATURATES.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T56_VEC_A, T56_KEY_A, T56_ID_A);
    add_node(T56_VEC_B, T56_KEY_B, T56_ID_B);
    add_node(T56_VEC_C, T56_KEY_C, T56_ID_C);
    add_node(T56_VEC_D, T56_KEY_D, T56_ID_D);
    add_node(T56_VEC_E, T56_KEY_E, T56_ID_E);
    add_edge(T56_ID_A, T56_ID_B, "t56.e.ab");
    add_edge(T56_ID_A, T56_ID_C, "t56.e.ac");
    add_edge(T56_ID_A, T56_ID_D, "t56.e.ad");
    add_edge(T56_ID_A, T56_ID_E, "t56.e.ae");

    let (ntriactc, nhtriactc, naso, ec, nc) = gos_runtime::graph_topo_indices56();
    assert_eq!(nc,        5,                           "k14: node_count=5");
    assert_eq!(ec,        4,                           "k14: edge_count=4");
    assert_eq!(ntriactc,  5_764_607_523_034_234_880,   "k14: NTRIACTC=5_764_607_523_034_234_880 (5\u{00d7}4\u{00b3}\u{2070}=5\u{00d7}2^60)");
    assert_eq!(nhtriactc, u64::MAX,                    "k14: NHTRIACTC=u64::MAX (4\u{00d7}8\u{00b2}\u{2079}>>u64::MAX; saturated)");
    assert_eq!(naso,      u64::MAX,                    "k14: NASO=u64::MAX (4\u{00d7}32\u{00b2}\u{2074}>>u64::MAX; saturated)");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1. S: S(A)=2,S(B)=3,S(C)=3,S(D)=2. 3 edges, 4 nodes.
// NTRIACTC:  2^30+3^30+3^30+2^30 = 2×1_073_741_824+2×205_891_132_094_649 = 413_929_747_672_946.
// NHTRIACTC: (2+3)^29+(3+3)^29+(3+2)^29 = 2×5^29+6^29 → 5^29>>u64::MAX → SATURATES.
// NASO:      13^24+18^24+13^24 — 13^24>>u64::MAX per-edge → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T56_VEC_A, T56_KEY_A, T56_ID_A);
    add_node(T56_VEC_B, T56_KEY_B, T56_ID_B);
    add_node(T56_VEC_C, T56_KEY_C, T56_ID_C);
    add_node(T56_VEC_D, T56_KEY_D, T56_ID_D);
    add_edge(T56_ID_A, T56_ID_B, "t56.e.ab");
    add_edge(T56_ID_B, T56_ID_C, "t56.e.bc");
    add_edge(T56_ID_C, T56_ID_D, "t56.e.cd");

    let (ntriactc, nhtriactc, naso, ec, nc) = gos_runtime::graph_topo_indices56();
    assert_eq!(nc,        4,                    "p4: node_count=4");
    assert_eq!(ec,        3,                    "p4: edge_count=3");
    assert_eq!(ntriactc,  411_784_411_672_946,  "p4: NTRIACTC=411_784_411_672_946 (2\u{00d7}2\u{00b3}\u{2070}+2\u{00d7}3\u{00b3}\u{2070}; 3\u{00b3}\u{2070}=205_891_132_094_649)");
    assert_eq!(nhtriactc, u64::MAX,             "p4: NHTRIACTC=u64::MAX (5\u{00b2}\u{2079}>>u64::MAX per-edge; saturated)");
    assert_eq!(naso,      u64::MAX,             "p4: NASO=u64::MAX (13\u{00b2}\u{2074}>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NTRIACTC:  4×9^30 → SATURATES → u64::MAX.
// NHTRIACTC: 6×18^29 → SATURATES → u64::MAX.
// NASO:      6×162^24 → SATURATES → u64::MAX.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T56_VEC_A, T56_KEY_A, T56_ID_A);
    add_node(T56_VEC_B, T56_KEY_B, T56_ID_B);
    add_node(T56_VEC_C, T56_KEY_C, T56_ID_C);
    add_node(T56_VEC_D, T56_KEY_D, T56_ID_D);
    add_edge(T56_ID_A, T56_ID_B, "t56.e.ab");
    add_edge(T56_ID_B, T56_ID_A, "t56.e.ba");
    add_edge(T56_ID_A, T56_ID_C, "t56.e.ac");
    add_edge(T56_ID_C, T56_ID_A, "t56.e.ca");
    add_edge(T56_ID_A, T56_ID_D, "t56.e.ad");
    add_edge(T56_ID_D, T56_ID_A, "t56.e.da");
    add_edge(T56_ID_B, T56_ID_C, "t56.e.bc");
    add_edge(T56_ID_C, T56_ID_B, "t56.e.cb");
    add_edge(T56_ID_B, T56_ID_D, "t56.e.bd");
    add_edge(T56_ID_D, T56_ID_B, "t56.e.db");
    add_edge(T56_ID_C, T56_ID_D, "t56.e.cd");
    add_edge(T56_ID_D, T56_ID_C, "t56.e.dc");

    let (ntriactc, nhtriactc, naso, ec, nc) = gos_runtime::graph_topo_indices56();
    assert_eq!(nc,        4,        "k4: node_count=4");
    assert_eq!(ec,        6,        "k4: edge_count=6");
    assert_eq!(ntriactc,  u64::MAX, "k4: NTRIACTC=u64::MAX (4\u{00d7}9\u{00b3}\u{2070} >> u64::MAX; saturated)");
    assert_eq!(nhtriactc, u64::MAX, "k4: NHTRIACTC=u64::MAX (6\u{00d7}18\u{00b2}\u{2079} >> u64::MAX; saturated)");
    assert_eq!(naso,      u64::MAX, "k4: NASO=u64::MAX (6\u{00d7}162\u{00b2}\u{2074} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NTRIACTC=0; NHTRIACTC=0; NASO=0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T56_VEC_A, T56_KEY_A, T56_ID_A);
    add_node(T56_VEC_B, T56_KEY_B, T56_ID_B);

    let (ntriactc, nhtriactc, naso, ec, nc) = gos_runtime::graph_topo_indices56();
    assert_eq!(nc,        2, "two-iso: node_count=2");
    assert_eq!(ec,        0, "two-iso: edge_count=0");
    assert_eq!(ntriactc,  0, "two-iso: NTRIACTC=0");
    assert_eq!(nhtriactc, 0, "two-iso: NHTRIACTC=0");
    assert_eq!(naso,      0, "two-iso: NASO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NTRIACTC:  5×6^30 → 6^30>>u64::MAX per-node → SATURATES.
// NHTRIACTC: 6×12^29 → SATURATES (12^29>>u64::MAX per-edge).
// NASO:      6×72^24 → SATURATES (per-edge >> u64::MAX).
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T56_VEC_A, T56_KEY_A, T56_ID_A);
    add_node(T56_VEC_B, T56_KEY_B, T56_ID_B);
    add_node(T56_VEC_C, T56_KEY_C, T56_ID_C);
    add_node(T56_VEC_D, T56_KEY_D, T56_ID_D);
    add_node(T56_VEC_E, T56_KEY_E, T56_ID_E);
    add_edge(T56_ID_A, T56_ID_C, "t56.e.ac");
    add_edge(T56_ID_A, T56_ID_D, "t56.e.ad");
    add_edge(T56_ID_A, T56_ID_E, "t56.e.ae");
    add_edge(T56_ID_B, T56_ID_C, "t56.e.bc");
    add_edge(T56_ID_B, T56_ID_D, "t56.e.bd");
    add_edge(T56_ID_B, T56_ID_E, "t56.e.be");

    let (ntriactc, nhtriactc, naso, ec, nc) = gos_runtime::graph_topo_indices56();
    assert_eq!(nc,        5,        "k23: node_count=5");
    assert_eq!(ec,        6,        "k23: edge_count=6");
    assert_eq!(ntriactc,  u64::MAX, "k23: NTRIACTC=u64::MAX (5\u{00d7}6\u{00b3}\u{2070}; 6\u{00b3}\u{2070}>>u64::MAX per-node; saturated)");
    assert_eq!(nhtriactc, u64::MAX, "k23: NHTRIACTC=u64::MAX (6\u{00d7}12\u{00b2}\u{2079} >> u64::MAX; per-edge saturates)");
    assert_eq!(naso,      u64::MAX, "k23: NASO=u64::MAX (6\u{00d7}72\u{00b2}\u{2074} >> u64::MAX; per-edge saturates)");
}
