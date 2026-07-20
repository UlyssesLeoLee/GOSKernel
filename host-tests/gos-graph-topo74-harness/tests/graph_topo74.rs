// gos-graph-topo74-harness — V3.85 NOCTOTETRAACTC + NHOCTOTETRAACTC + NAQSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices74()`:
//   Returns (noctotetraactc, nhoctotetraactc, naqso, edge_count, node_count)
//   - noctotetraactc  = NOCTOTETRAACTC(G) = Σ_v S(v)^48                   (exact u64; S-Octotetracontic vertex sum)
//   - nhoctotetraactc = NHOCTOTETRAACTC(G)= Σ_{uv∈E} (S_u+S_v)^47         (exact u64; S-Heptotetracontic edge-sum)
//   - naqso           = NAQSO(G)          = Σ_{uv∈E} (S_u²+S_v²)^42       (exact u64; S-Tetrahexacontyl Sombor, α=84)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOCTOTETRAACTC(G) = Σ_v S(v)^48
//     S-Octotetracontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48),
//       NTRICTC=Σ S²³ (topo49), NTETRTC=Σ S²⁴ (topo50), NPENTTC=Σ S²⁵ (topo51),
//       NHEXATC=Σ S²⁶ (topo52), NHEPTATC=Σ S²⁷ (topo53), NOCTATC=Σ S²⁸ (topo54),
//       NNONATC=Σ S²⁹ (topo55), NTRIACTC=Σ S³⁰ (topo56), NHENTRIACTC=Σ S³¹ (topo57),
//       NDOTRIACTC=Σ S³² (topo58), NTRITRIACTC=Σ S³³ (topo59), NTETRTRIACTC=Σ S³⁴ (topo60),
//       NPENTTRIACTC=Σ S³⁵ (topo61), NHEXATRIACTC=Σ S³⁶ (topo62), NHEPTATRIACTC=Σ S³⁷ (topo63),
//       NOCTATRIACTC=Σ S³⁸ (topo64), NNONATRIACTC=Σ S³⁹ (topo65), NTETRAACTC=Σ S⁴⁰ (topo66),
//       NHENTETRAACTC=Σ S⁴¹ (topo67), NDOTETRAACTC=Σ S⁴² (topo68), NTRITETRAACTC=Σ S⁴³ (topo69),
//       NTETRATETRAACTC=Σ S⁴⁴ (topo70), NPENTETRAACTC=Σ S⁴⁵ (topo71),
//       NHEXTETRAACTC=Σ S⁴⁶ (topo72), NHEPTETRAACTC=Σ S⁴⁷ (topo73),
//       NOCTOTETRAACTC=Σ S⁴⁸ (topo74).
//     NOCTOTETRAACTC = n·S^48 for S-regular.
//     Overflow: S^48 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^48 = s32 × s16  (s32=s16^2; 48=32+16; 2 mults — very efficient!).
//
//   NHOCTOTETRAACTC(G) = Σ_{uv∈E} (S_u+S_v)^47
//     S-Heptotetracontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45), NHEICTC=Σ(S+S)¹⁹ (topo46),
//       NHHENTC=Σ(S+S)²⁰ (topo47), NHDOCTC=Σ(S+S)²¹ (topo48), NHTRICTC=Σ(S+S)²² (topo49),
//       NHTETRTC=Σ(S+S)²³ (topo50), NHPENTTC=Σ(S+S)²⁴ (topo51), NHHEXATC=Σ(S+S)²⁵ (topo52),
//       NHHEPTATC=Σ(S+S)²⁶ (topo53), NHOCTATC=Σ(S+S)²⁷ (topo54), NHNONATC=Σ(S+S)²⁸ (topo55),
//       NHTRIACTC=Σ(S+S)²⁹ (topo56), NHHENTRIACTC=Σ(S+S)³⁰ (topo57),
//       NHDOTRIACTC=Σ(S+S)³¹ (topo58), NHTRITRIACTC=Σ(S+S)³² (topo59),
//       NHTETRTRIACTC=Σ(S+S)³³ (topo60), NHPENTTRIACTC=Σ(S+S)³⁴ (topo61),
//       NHHEXATRIACTC=Σ(S+S)³⁵ (topo62), NHHEPTATRIACTC=Σ(S+S)³⁶ (topo63),
//       NHOCTATRIACTC=Σ(S+S)³⁷ (topo64), NHNONATRIACTC=Σ(S+S)³⁸ (topo65),
//       NHTETRAACTC=Σ(S+S)³⁹ (topo66), NHHENTETRAACTC=Σ(S+S)⁴⁰ (topo67),
//       NHDOTETRAACTC=Σ(S+S)⁴¹ (topo68), NHTRITETRAACTC=Σ(S+S)⁴² (topo69),
//       NHTETRATETRAACTC=Σ(S+S)⁴³ (topo70), NHPENTETRAACTC=Σ(S+S)⁴⁴ (topo71),
//       NHHEXTETRAACTC=Σ(S+S)⁴⁵ (topo72), NHHEPTETRAACTC=Σ(S+S)⁴⁶ (topo73),
//       NHOCTOTETRAACTC=Σ(S+S)⁴⁷ (topo74).
//     NHOCTOTETRAACTC = |E|·(2S)^47 = 140737488355328|E|·S^47 for S-regular.
//     Overflow per edge: (2×16129)^47 → saturating u128 accumulator.
//     Implementation: ss^47 = ss32 × ss8 × ss4 × ss2 × ss  (ss32=ss16^2; ss8=ss4^2; 47=32+8+4+2+1; 5 mults).
//
//   NAQSO(G) = Σ_{uv∈E} (S_u²+S_v²)^42
//     S-Tetrahexacontyl Sombor: generalised Sombor SO^α with α=84 on S-variant.
//     3rd-pass double-letter "AQ" (after NAPSO α=82, topo73).
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42), NYSO(topo54,α=44), NZSO(topo55,α=46),
//     NASO(topo56,α=48), NBSO(topo57,α=50), NAASO(topo58,α=52), NABSO(topo59,α=54),
//     NACSO(topo60,α=56), NADSO(topo61,α=58), NAESO(topo62,α=60), NAFSO(topo63,α=62),
//     NAGSO(topo64,α=64), NAHSO(topo65,α=66), NAISO(topo66,α=68), NAJSO(topo67,α=70),
//     NAKSO(topo68,α=72), NALSO(topo69,α=74), NAMSO(topo70,α=76), NANSO(topo71,α=78),
//     NAOSO(topo72,α=80), NAPSO(topo73,α=82), NAQSO(topo74,α=84).
//     NAQSO = |E|·(2S²)^42 = 4398046511104|E|·S^84 for S-regular.
//     Overflow per edge: (2×16129²)^42 → saturating u128 accumulator.
//     Implementation: s2s^42 = s2s32 × s2s8 × s2s2  (s2s32=s2s16^2; s2s8=s2s4^2; 42=32+8+2; 3 mults).
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
//  Graph     NOCTOTETRAACTC(exact)           NHOCTOTETRAACTC(exact)       NAQSO(exact)             edges  nodes
//  Empty                        0                             0                        0               0      0
//  1 node                       0                             0                        0               0      1
//  K₂                           2            140_737_488_355_328           4_398_046_511_104               1      2
//  P₃          844_424_930_131_968              u64::MAX(sat.)              u64::MAX(sat.)              2      3
//  K₃               u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4}          u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄               u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄               u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              6      4
//  2 isolated                   0                             0                        0               0      2
//  K_{2,3}          u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NOCTOTETRAACTC:  1^48 + 1^48 = 2. ✓
//     NHOCTOTETRAACTC: (1+1)^47 = 2^47 = 140_737_488_355_328. ✓
//     NAQSO:           (1²+1²)^42 = 2^42 = 4_398_046_511_104. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOCTOTETRAACTC:  3×2^48 = 3×281_474_976_710_656 = 844_424_930_131_968. ✓
//     NHOCTOTETRAACTC: 2×(2+2)^47 = 2×4^47 = 2×2^94 → SATURATES. ✓
//     NAQSO:           2×(4+4)^42 = 2×8^42 = 2×2^126 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOCTOTETRAACTC:  3×4^48 = 3×2^96 → SATURATES. ✓
//     NHOCTOTETRAACTC: 3×8^47 → SATURATES. ✓
//     NAQSO:           3×32^42 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOCTOTETRAACTC:  5×4^48 → SATURATES. ✓
//     NHOCTOTETRAACTC: 4×8^47 → SATURATES. ✓
//     NAQSO:           4×32^42 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOCTOTETRAACTC:  2×2^48 + 2×3^48.
//       3^48>u64::MAX (since 3^41>u64::MAX) → SATURATES. ✓
//     NHOCTOTETRAACTC: 2×5^47 + 6^47 → each term >> u64::MAX → SATURATES. ✓
//     NAQSO:           2×13^42 + 18^42 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOCTOTETRAACTC:  4×9^48 → SATURATES. ✓
//     NHOCTOTETRAACTC: 6×18^47 → SATURATES. ✓
//     NAQSO:           6×162^42 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOCTOTETRAACTC:  5×6^48 → SATURATES. ✓
//     NHOCTOTETRAACTC: 6×12^47 → SATURATES. ✓
//     NAQSO:           6×72^42 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOCTOTETRAACTC  = n·S^48                                                            for S-regular ✓
//   NHOCTOTETRAACTC = |E|·(2S)^47 = 140737488355328|E|·S^47                             for S-regular ✓
//   NAQSO           = |E|·(2S²)^42 = 4398046511104|E|·S^84                              for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 140_737_488_355_328, 4_398_046_511_104, 1, 2)
//  4.  Path P₃ = A-B-C                   → (844_424_930_131_968, u64::MAX, u64::MAX, 2, 3)
//  5.  Triangle K₃                       → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (u64::MAX, u64::MAX, u64::MAX, 3, 4)
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

const T74_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_74");
const T74_EXEC:   ExecutorId = ExecutorId::from_ascii("t74.exec");

const T74_KEY_A: &str = "t74.alpha";
const T74_KEY_B: &str = "t74.beta";
const T74_KEY_C: &str = "t74.gamma";
const T74_KEY_D: &str = "t74.delta";
const T74_KEY_E: &str = "t74.epsilon";

const T74_ID_A: NodeId = derive_node_id(T74_PLUGIN, T74_KEY_A);
const T74_ID_B: NodeId = derive_node_id(T74_PLUGIN, T74_KEY_B);
const T74_ID_C: NodeId = derive_node_id(T74_PLUGIN, T74_KEY_C);
const T74_ID_D: NodeId = derive_node_id(T74_PLUGIN, T74_KEY_D);
const T74_ID_E: NodeId = derive_node_id(T74_PLUGIN, T74_KEY_E);

// L4=161 namespace for this harness.
const T74_VEC_A: VectorAddress = VectorAddress::new(161, 1, 1, 0);
const T74_VEC_B: VectorAddress = VectorAddress::new(161, 1, 2, 0);
const T74_VEC_C: VectorAddress = VectorAddress::new(161, 1, 3, 0);
const T74_VEC_D: VectorAddress = VectorAddress::new(161, 2, 1, 0);
const T74_VEC_E: VectorAddress = VectorAddress::new(161, 2, 2, 0);

const T74_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T74_PLUGIN,
    name:         "kl-graph-topo74-harness",
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
        executor_id:       T74_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T74_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T74_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (noctotetraactc, nhoctotetraactc, naqso, ec, nc) = gos_runtime::graph_topo_indices74();
    assert_eq!(nc,                0, "empty: node_count=0");
    assert_eq!(ec,                0, "empty: edge_count=0");
    assert_eq!(noctotetraactc,    0, "empty: NOCTOTETRAACTC=0");
    assert_eq!(nhoctotetraactc,   0, "empty: NHOCTOTETRAACTC=0");
    assert_eq!(naqso,             0, "empty: NAQSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T74_VEC_A, T74_KEY_A, T74_ID_A);

    let (noctotetraactc, nhoctotetraactc, naqso, ec, nc) = gos_runtime::graph_topo_indices74();
    assert_eq!(nc,                1, "single: node_count=1");
    assert_eq!(ec,                0, "single: edge_count=0");
    assert_eq!(noctotetraactc,    0, "single: NOCTOTETRAACTC=0");
    assert_eq!(nhoctotetraactc,   0, "single: NHOCTOTETRAACTC=0");
    assert_eq!(naqso,             0, "single: NAQSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NOCTOTETRAACTC:  1^48+1^48 = 2.
// NHOCTOTETRAACTC: (1+1)^47 = 2^47 = 140_737_488_355_328.
// NAQSO:           (1²+1²)^42 = 2^42 = 4_398_046_511_104.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T74_VEC_A, T74_KEY_A, T74_ID_A);
    add_node(T74_VEC_B, T74_KEY_B, T74_ID_B);
    add_edge(T74_ID_A, T74_ID_B, "t74.e.ab");

    let (noctotetraactc, nhoctotetraactc, naqso, ec, nc) = gos_runtime::graph_topo_indices74();
    assert_eq!(nc,                2,                     "k2: node_count=2");
    assert_eq!(ec,                1,                     "k2: edge_count=1");
    assert_eq!(noctotetraactc,    2,                     "k2: NOCTOTETRAACTC=2 (1\u{2074}\u{2078}+1\u{2074}\u{2078}=2)");
    assert_eq!(nhoctotetraactc,   140_737_488_355_328,   "k2: NHOCTOTETRAACTC=140_737_488_355_328 (2\u{2074}\u{2077}=2^47)");
    assert_eq!(naqso,             4_398_046_511_104,     "k2: NAQSO=4_398_046_511_104 (2\u{2074}\u{00b2}=2^42)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NOCTOTETRAACTC:  3×2^48 = 3×281_474_976_710_656 = 844_424_930_131_968.
// NHOCTOTETRAACTC: 2×(2+2)^47 = 2×4^47 = 2×2^94 → SATURATES.
// NAQSO:           2×(4+4)^42 = 2×8^42 = 2×2^126 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T74_VEC_A, T74_KEY_A, T74_ID_A);
    add_node(T74_VEC_B, T74_KEY_B, T74_ID_B);
    add_node(T74_VEC_C, T74_KEY_C, T74_ID_C);
    add_edge(T74_ID_A, T74_ID_B, "t74.e.ab");
    add_edge(T74_ID_B, T74_ID_C, "t74.e.bc");

    let (noctotetraactc, nhoctotetraactc, naqso, ec, nc) = gos_runtime::graph_topo_indices74();
    assert_eq!(nc,                3,                       "p3: node_count=3");
    assert_eq!(ec,                2,                       "p3: edge_count=2");
    assert_eq!(noctotetraactc,    844_424_930_131_968,     "p3: NOCTOTETRAACTC=844_424_930_131_968 (3\u{00d7}2\u{2074}\u{2078})");
    assert_eq!(nhoctotetraactc,   u64::MAX,                "p3: NHOCTOTETRAACTC=SAT (4\u{2074}\u{2077}>u64)");
    assert_eq!(naqso,             u64::MAX,                "p3: NAQSO=SAT (8\u{2074}\u{00b2}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T74_VEC_A, T74_KEY_A, T74_ID_A);
    add_node(T74_VEC_B, T74_KEY_B, T74_ID_B);
    add_node(T74_VEC_C, T74_KEY_C, T74_ID_C);
    add_edge(T74_ID_A, T74_ID_B, "t74.e.ab");
    add_edge(T74_ID_B, T74_ID_C, "t74.e.bc");
    add_edge(T74_ID_C, T74_ID_A, "t74.e.ca");

    let (noctotetraactc, nhoctotetraactc, naqso, ec, nc) = gos_runtime::graph_topo_indices74();
    assert_eq!(nc,               3,        "k3: node_count=3");
    assert_eq!(ec,               3,        "k3: edge_count=3");
    assert_eq!(noctotetraactc,   u64::MAX, "k3: NOCTOTETRAACTC=SAT");
    assert_eq!(nhoctotetraactc,  u64::MAX, "k3: NHOCTOTETRAACTC=SAT");
    assert_eq!(naqso,            u64::MAX, "k3: NAQSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T74_VEC_A, T74_KEY_A, T74_ID_A); // hub
    add_node(T74_VEC_B, T74_KEY_B, T74_ID_B);
    add_node(T74_VEC_C, T74_KEY_C, T74_ID_C);
    add_node(T74_VEC_D, T74_KEY_D, T74_ID_D);
    add_node(T74_VEC_E, T74_KEY_E, T74_ID_E);
    add_edge(T74_ID_A, T74_ID_B, "t74.e.ab");
    add_edge(T74_ID_A, T74_ID_C, "t74.e.ac");
    add_edge(T74_ID_A, T74_ID_D, "t74.e.ad");
    add_edge(T74_ID_A, T74_ID_E, "t74.e.ae");

    let (noctotetraactc, nhoctotetraactc, naqso, ec, nc) = gos_runtime::graph_topo_indices74();
    assert_eq!(nc,               5,        "k14: node_count=5");
    assert_eq!(ec,               4,        "k14: edge_count=4");
    assert_eq!(noctotetraactc,   u64::MAX, "k14: NOCTOTETRAACTC=SAT");
    assert_eq!(nhoctotetraactc,  u64::MAX, "k14: NHOCTOTETRAACTC=SAT");
    assert_eq!(naqso,            u64::MAX, "k14: NAQSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NOCTOTETRAACTC: 2×2^48 + 2×3^48.  3^48>u64::MAX → SATURATES.
// NHOCTOTETRAACTC: 5^47+6^47+5^47 → SATURATES.
// NAQSO: 13^42+18^42+13^42 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T74_VEC_A, T74_KEY_A, T74_ID_A);
    add_node(T74_VEC_B, T74_KEY_B, T74_ID_B);
    add_node(T74_VEC_C, T74_KEY_C, T74_ID_C);
    add_node(T74_VEC_D, T74_KEY_D, T74_ID_D);
    add_edge(T74_ID_A, T74_ID_B, "t74.e.ab");
    add_edge(T74_ID_B, T74_ID_C, "t74.e.bc");
    add_edge(T74_ID_C, T74_ID_D, "t74.e.cd");

    let (noctotetraactc, nhoctotetraactc, naqso, ec, nc) = gos_runtime::graph_topo_indices74();
    assert_eq!(nc,               4,        "p4: node_count=4");
    assert_eq!(ec,               3,        "p4: edge_count=3");
    assert_eq!(noctotetraactc,   u64::MAX, "p4: NOCTOTETRAACTC=SAT");
    assert_eq!(nhoctotetraactc,  u64::MAX, "p4: NHOCTOTETRAACTC=SAT");
    assert_eq!(naqso,            u64::MAX, "p4: NAQSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T74_VEC_A, T74_KEY_A, T74_ID_A);
    add_node(T74_VEC_B, T74_KEY_B, T74_ID_B);
    add_node(T74_VEC_C, T74_KEY_C, T74_ID_C);
    add_node(T74_VEC_D, T74_KEY_D, T74_ID_D);
    add_edge(T74_ID_A, T74_ID_B, "t74.e.ab");
    add_edge(T74_ID_A, T74_ID_C, "t74.e.ac");
    add_edge(T74_ID_A, T74_ID_D, "t74.e.ad");
    add_edge(T74_ID_B, T74_ID_C, "t74.e.bc");
    add_edge(T74_ID_B, T74_ID_D, "t74.e.bd");
    add_edge(T74_ID_C, T74_ID_D, "t74.e.cd");

    let (noctotetraactc, nhoctotetraactc, naqso, ec, nc) = gos_runtime::graph_topo_indices74();
    assert_eq!(nc,               4,        "k4: node_count=4");
    assert_eq!(ec,               6,        "k4: edge_count=6");
    assert_eq!(noctotetraactc,   u64::MAX, "k4: NOCTOTETRAACTC=SAT");
    assert_eq!(nhoctotetraactc,  u64::MAX, "k4: NHOCTOTETRAACTC=SAT");
    assert_eq!(naqso,            u64::MAX, "k4: NAQSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T74_VEC_A, T74_KEY_A, T74_ID_A);
    add_node(T74_VEC_B, T74_KEY_B, T74_ID_B);

    let (noctotetraactc, nhoctotetraactc, naqso, ec, nc) = gos_runtime::graph_topo_indices74();
    assert_eq!(nc,                2, "isolated: node_count=2");
    assert_eq!(ec,                0, "isolated: edge_count=0");
    assert_eq!(noctotetraactc,    0, "isolated: NOCTOTETRAACTC=0");
    assert_eq!(nhoctotetraactc,   0, "isolated: NHOCTOTETRAACTC=0");
    assert_eq!(naqso,             0, "isolated: NAQSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NOCTOTETRAACTC=5×6^48 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T74_VEC_A, T74_KEY_A, T74_ID_A);
    add_node(T74_VEC_B, T74_KEY_B, T74_ID_B);
    add_node(T74_VEC_C, T74_KEY_C, T74_ID_C);
    add_node(T74_VEC_D, T74_KEY_D, T74_ID_D);
    add_node(T74_VEC_E, T74_KEY_E, T74_ID_E);
    add_edge(T74_ID_A, T74_ID_C, "t74.e.ac");
    add_edge(T74_ID_A, T74_ID_D, "t74.e.ad");
    add_edge(T74_ID_A, T74_ID_E, "t74.e.ae");
    add_edge(T74_ID_B, T74_ID_C, "t74.e.bc");
    add_edge(T74_ID_B, T74_ID_D, "t74.e.bd");
    add_edge(T74_ID_B, T74_ID_E, "t74.e.be");

    let (noctotetraactc, nhoctotetraactc, naqso, ec, nc) = gos_runtime::graph_topo_indices74();
    assert_eq!(nc,               5,        "k23: node_count=5");
    assert_eq!(ec,               6,        "k23: edge_count=6");
    assert_eq!(noctotetraactc,   u64::MAX, "k23: NOCTOTETRAACTC=SAT (5\u{00d7}6\u{2074}\u{2078})");
    assert_eq!(nhoctotetraactc,  u64::MAX, "k23: NHOCTOTETRAACTC=SAT");
    assert_eq!(naqso,            u64::MAX, "k23: NAQSO=SAT");
}
