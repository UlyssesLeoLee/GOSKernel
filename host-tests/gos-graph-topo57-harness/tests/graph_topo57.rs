// gos-graph-topo57-harness — V3.68 NHENTRIACTC + NHHENTRIACTC + NBSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices57()`:
//   Returns (nhentriactc, nhhentriactc, nbso, edge_count, node_count)
//   - nhentriactc  = NHENTRIACTC(G) = Σ_v S(v)^31                   (exact u64; S-Hentriacontic vertex sum)
//   - nhhentriactc = NHHENTRIACTC(G)= Σ_{uv∈E} (S_u+S_v)^30         (exact u64; S-Triacontic edge-sum)
//   - nbso         = NBSO(G)        = Σ_{uv∈E} (S_u²+S_v²)^25       (exact u64; S-Pentacontyl Sombor, α=50)
//   - edge_count   = undirected non-self-loop edges
//   - node_count   = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHENTRIACTC(G) = Σ_v S(v)^31
//     S-Hentriacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48),
//       NTRICTC=Σ S²³ (topo49), NTETRTC=Σ S²⁴ (topo50), NPENTTC=Σ S²⁵ (topo51),
//       NHEXATC=Σ S²⁶ (topo52), NHEPTATC=Σ S²⁷ (topo53), NOCTATC=Σ S²⁸ (topo54),
//       NNONATC=Σ S²⁹ (topo55), NTRIACTC=Σ S³⁰ (topo56), NHENTRIACTC=Σ S³¹ (topo57).
//     NHENTRIACTC = n·S^31 for S-regular.
//     Overflow: S^31 ≤ 16129^31 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHHENTRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^30
//     S-Triacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45), NHEICTC=Σ(S+S)¹⁹ (topo46),
//       NHHENTC=Σ(S+S)²⁰ (topo47), NHDOCTC=Σ(S+S)²¹ (topo48), NHTRICTC=Σ(S+S)²² (topo49),
//       NHTETRTC=Σ(S+S)²³ (topo50), NHPENTTC=Σ(S+S)²⁴ (topo51), NHHEXATC=Σ(S+S)²⁵ (topo52),
//       NHHEPTATC=Σ(S+S)²⁶ (topo53), NHOCTATC=Σ(S+S)²⁷ (topo54), NHNONATC=Σ(S+S)²⁸ (topo55),
//       NHTRIACTC=Σ(S+S)²⁹ (topo56), NHHENTRIACTC=Σ(S+S)³⁰ (topo57).
//     NHHENTRIACTC = |E|·(2S)^30 = 1073741824|E|·S^30 for S-regular.
//     Overflow per edge: (2×16129)^30 → saturating u128 accumulator.
//
//   NBSO(G) = Σ_{uv∈E} (S_u²+S_v²)^25
//     S-Pentacontyl Sombor: generalised Sombor SO^α with α=50 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42), NYSO(topo54,α=44), NZSO(topo55,α=46),
//     NASO(topo56,α=48), NBSO(topo57,α=50).
//     NBSO = |E|·(2S²)^25 = 33554432|E|·S^50 for S-regular.
//     Overflow per edge: (2×16129²)^25 → saturating u128 accumulator.
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
//  Graph     NHENTRIACTC(exact)             NHHENTRIACTC(exact)           NBSO(exact)              edges  nodes
//  Empty                   0                             0                         0               0      0
//  1 node                  0                             0                         0               0      1
//  K₂                      2                   1_073_741_824                33_554_432               1      2
//  P₃           6_442_450_944       2_305_843_009_213_693_952           u64::MAX(sat.)              2      3
//  K₃  13_835_058_055_282_163_712       u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4}     u64::MAX(sat.)            u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄       1_235_351_087_535_190         u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄          u64::MAX(sat.)            u64::MAX(sat.)               u64::MAX(sat.)              6      4
//  2 isolated              0                             0                         0               0      2
//  K_{2,3}    u64::MAX(sat.)             u64::MAX(sat.)               u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHENTRIACTC:  1^31 + 1^31 = 2. ✓
//     NHHENTRIACTC: (1+1)^30 = 2^30 = 1_073_741_824. ✓
//     NBSO:         (1²+1²)^25 = 2^25 = 33_554_432. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHENTRIACTC:  3×2^31 = 3×2_147_483_648 = 6_442_450_944. ✓
//     NHHENTRIACTC: 2×(2+2)^30 = 2×4^30 = 2×2^60 = 2^61 = 2_305_843_009_213_693_952. ✓
//       (4^30=2^60=1_152_921_504_606_846_976; 2×4^30=2^61=2_305_843_009_213_693_952)
//     NBSO:         2×(4+4)^25 = 2×8^25 = 2×2^75 → SATURATES (8^25=2^75>u64::MAX per-edge). ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHENTRIACTC:  3×4^31 = 3×2^62 = 3×4_611_686_018_427_387_904 = 13_835_058_055_282_163_712 (fits u64). ✓
//       (4^31=2^62=4_611_686_018_427_387_904; 3×2^62<2^64=18_446_744_073_709_551_616)
//     NHHENTRIACTC: 3×(4+4)^30 = 3×8^30 = 3×2^90 → SATURATES (per-edge >> u64::MAX). ✓
//     NBSO:         3×(16+16)^25 = 3×32^25 = 3×2^125 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHENTRIACTC:  5×4^31 = 5×4_611_686_018_427_387_904 = 23_058_430_092_136_939_520 > u64::MAX → SATURATES. ✓
//     NHHENTRIACTC: 4×8^30 → SATURATES. ✓
//     NBSO:         4×32^25 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHENTRIACTC:  2^31+3^31+3^31+2^31 = 2×2_147_483_648+2×617_673_396_283_947.
//       3^31=3^16×3^8×3^4×3^2×3=43_046_721×6561×81×9×3=617_673_396_283_947.
//       2×2_147_483_648+2×617_673_396_283_947=4_294_967_296+1_235_346_792_567_894=1_235_351_087_535_190. ✓
//     NHHENTRIACTC: (2+3)^30+(3+3)^30+(3+2)^30 = 2×5^30+6^30
//       5^30>>u64::MAX per-edge → SATURATES. ✓
//     NBSO:         13^25+18^25+13^25 — 13^25>>u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHENTRIACTC:  4×9^31 → SATURATES → u64::MAX. ✓
//     NHHENTRIACTC: → SATURATES. ✓
//     NBSO:         → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHENTRIACTC:  5×6^31 → SATURATES → u64::MAX. ✓
//     NHHENTRIACTC: → SATURATES. ✓
//     NBSO:         → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHENTRIACTC  = n·S^31                                                  for S-regular ✓
//   NHHENTRIACTC = |E|·(2S)^30 = 1073741824|E|·S^30                        for S-regular ✓
//   NBSO         = |E|·(2S²)^25 = 33554432|E|·S^50                         for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 1_073_741_824, 33_554_432, 1, 2)
//  4.  Path P₃ = A-B-C                   → (6_442_450_944, 2_305_843_009_213_693_952, u64::MAX, 2, 3)
//  5.  Triangle K₃                       → (13_835_058_055_282_163_712, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (1_239_641_759_535_190, u64::MAX, u64::MAX, 3, 4)
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

const T57_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_57");
const T57_EXEC:   ExecutorId = ExecutorId::from_ascii("t57.exec");

const T57_KEY_A: &str = "t57.alpha";
const T57_KEY_B: &str = "t57.beta";
const T57_KEY_C: &str = "t57.gamma";
const T57_KEY_D: &str = "t57.delta";
const T57_KEY_E: &str = "t57.epsilon";

const T57_ID_A: NodeId = derive_node_id(T57_PLUGIN, T57_KEY_A);
const T57_ID_B: NodeId = derive_node_id(T57_PLUGIN, T57_KEY_B);
const T57_ID_C: NodeId = derive_node_id(T57_PLUGIN, T57_KEY_C);
const T57_ID_D: NodeId = derive_node_id(T57_PLUGIN, T57_KEY_D);
const T57_ID_E: NodeId = derive_node_id(T57_PLUGIN, T57_KEY_E);

// L4=144 namespace for this harness.
const T57_VEC_A: VectorAddress = VectorAddress::new(144, 1, 1, 0);
const T57_VEC_B: VectorAddress = VectorAddress::new(144, 1, 2, 0);
const T57_VEC_C: VectorAddress = VectorAddress::new(144, 1, 3, 0);
const T57_VEC_D: VectorAddress = VectorAddress::new(144, 2, 1, 0);
const T57_VEC_E: VectorAddress = VectorAddress::new(144, 2, 2, 0);

const T57_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T57_PLUGIN,
    name:         "kl-graph-topo57-harness",
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
        executor_id:       T57_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T57_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T57_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nhentriactc, nhhentriactc, nbso, ec, nc) = gos_runtime::graph_topo_indices57();
    assert_eq!(nc,           0, "empty: node_count=0");
    assert_eq!(ec,           0, "empty: edge_count=0");
    assert_eq!(nhentriactc,  0, "empty: NHENTRIACTC=0");
    assert_eq!(nhhentriactc, 0, "empty: NHHENTRIACTC=0");
    assert_eq!(nbso,         0, "empty: NBSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T57_VEC_A, T57_KEY_A, T57_ID_A);

    let (nhentriactc, nhhentriactc, nbso, ec, nc) = gos_runtime::graph_topo_indices57();
    assert_eq!(nc,           1, "single: node_count=1");
    assert_eq!(ec,           0, "single: edge_count=0");
    assert_eq!(nhentriactc,  0, "single: NHENTRIACTC=0");
    assert_eq!(nhhentriactc, 0, "single: NHHENTRIACTC=0");
    assert_eq!(nbso,         0, "single: NBSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHENTRIACTC:  1^31+1^31 = 2.
// NHHENTRIACTC: (1+1)^30 = 2^30 = 1_073_741_824.
// NBSO:         (1²+1²)^25 = 2^25 = 33_554_432.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T57_VEC_A, T57_KEY_A, T57_ID_A);
    add_node(T57_VEC_B, T57_KEY_B, T57_ID_B);
    add_edge(T57_ID_A, T57_ID_B, "t57.e.ab");

    let (nhentriactc, nhhentriactc, nbso, ec, nc) = gos_runtime::graph_topo_indices57();
    assert_eq!(nc,           2,             "k2: node_count=2");
    assert_eq!(ec,           1,             "k2: edge_count=1");
    assert_eq!(nhentriactc,  2,             "k2: NHENTRIACTC=2 (1\u{00b3}\u{00b9}+1\u{00b3}\u{00b9}=2)");
    assert_eq!(nhhentriactc, 1_073_741_824, "k2: NHHENTRIACTC=1_073_741_824 (2\u{00b3}\u{2070}=2^30)");
    assert_eq!(nbso,         33_554_432,    "k2: NBSO=33_554_432 (2\u{00b2}\u{2075}=2^25)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NHENTRIACTC:  3×2^31 = 3×2_147_483_648 = 6_442_450_944.
// NHHENTRIACTC: 2×(2+2)^30 = 2×4^30 = 2×2^60 = 2^61 = 2_305_843_009_213_693_952.
// NBSO:         2×(4+4)^25 = 2×8^25 = 2×2^75 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T57_VEC_A, T57_KEY_A, T57_ID_A);
    add_node(T57_VEC_B, T57_KEY_B, T57_ID_B);
    add_node(T57_VEC_C, T57_KEY_C, T57_ID_C);
    add_edge(T57_ID_A, T57_ID_B, "t57.e.ab");
    add_edge(T57_ID_B, T57_ID_C, "t57.e.bc");

    let (nhentriactc, nhhentriactc, nbso, ec, nc) = gos_runtime::graph_topo_indices57();
    assert_eq!(nc,           3,                           "p3: node_count=3");
    assert_eq!(ec,           2,                           "p3: edge_count=2");
    assert_eq!(nhentriactc,  6_442_450_944,               "p3: NHENTRIACTC=6_442_450_944 (3\u{00d7}2\u{00b3}\u{00b9})");
    assert_eq!(nhhentriactc, 2_305_843_009_213_693_952,   "p3: NHHENTRIACTC=2_305_843_009_213_693_952 (2\u{00d7}4\u{00b3}\u{2070}=2^61)");
    assert_eq!(nbso,         u64::MAX,                    "p3: NBSO=u64::MAX (8\u{00b2}\u{2075}=2^75>u64::MAX per-edge; saturated)");
}

// ── Test 5: Triangle K₃ ─────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NHENTRIACTC:  3×4^31 = 3×2^62 = 13_835_058_055_282_163_712 (fits u64).
// NHHENTRIACTC: 3×(4+4)^30 = 3×8^30 = 3×2^90 → SATURATES.
// NBSO:         3×(16+16)^25 = 3×32^25 = 3×2^125 → SATURATES.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T57_VEC_A, T57_KEY_A, T57_ID_A);
    add_node(T57_VEC_B, T57_KEY_B, T57_ID_B);
    add_node(T57_VEC_C, T57_KEY_C, T57_ID_C);
    add_edge(T57_ID_A, T57_ID_B, "t57.e.ab");
    add_edge(T57_ID_B, T57_ID_A, "t57.e.ba");
    add_edge(T57_ID_B, T57_ID_C, "t57.e.bc");
    add_edge(T57_ID_C, T57_ID_B, "t57.e.cb");
    add_edge(T57_ID_A, T57_ID_C, "t57.e.ac");
    add_edge(T57_ID_C, T57_ID_A, "t57.e.ca");

    let (nhentriactc, nhhentriactc, nbso, ec, nc) = gos_runtime::graph_topo_indices57();
    assert_eq!(nc,           3,                             "k3: node_count=3");
    assert_eq!(ec,           3,                             "k3: edge_count=3");
    assert_eq!(nhentriactc,  13_835_058_055_282_163_712,    "k3: NHENTRIACTC=13_835_058_055_282_163_712 (3\u{00d7}4\u{00b3}\u{00b9}=3\u{00d7}2^62)");
    assert_eq!(nhhentriactc, u64::MAX,                      "k3: NHHENTRIACTC=u64::MAX (3\u{00d7}8\u{00b3}\u{2070}=3\u{00d7}2^90>>u64::MAX; saturated)");
    assert_eq!(nbso,         u64::MAX,                      "k3: NBSO=u64::MAX (3\u{00d7}32\u{00b2}\u{2075}>>u64::MAX; saturated)");
}

// ── Test 6: Star K_{1,4} ────────────────────────────────────────────────────
// Center A: d=4. Leaves B,C,D,E: d=1.
// S(center)=4×1=4. S(leaf)=1×4=4. S-uniform S=4. 4 edges, 5 nodes.
// NHENTRIACTC:  5×4^31 = 5×2^62 = 23_058_430_092_136_939_520 > u64::MAX → SATURATES.
// NHHENTRIACTC: 4×(4+4)^30 = 4×8^30 → SATURATES.
// NBSO:         4×(16+16)^25 → SATURATES.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T57_VEC_A, T57_KEY_A, T57_ID_A);
    add_node(T57_VEC_B, T57_KEY_B, T57_ID_B);
    add_node(T57_VEC_C, T57_KEY_C, T57_ID_C);
    add_node(T57_VEC_D, T57_KEY_D, T57_ID_D);
    add_node(T57_VEC_E, T57_KEY_E, T57_ID_E);
    add_edge(T57_ID_A, T57_ID_B, "t57.e.ab");
    add_edge(T57_ID_A, T57_ID_C, "t57.e.ac");
    add_edge(T57_ID_A, T57_ID_D, "t57.e.ad");
    add_edge(T57_ID_A, T57_ID_E, "t57.e.ae");

    let (nhentriactc, nhhentriactc, nbso, ec, nc) = gos_runtime::graph_topo_indices57();
    assert_eq!(nc,           5,        "k14: node_count=5");
    assert_eq!(ec,           4,        "k14: edge_count=4");
    assert_eq!(nhentriactc,  u64::MAX, "k14: NHENTRIACTC=u64::MAX (5\u{00d7}4\u{00b3}\u{00b9}>u64::MAX; saturated)");
    assert_eq!(nhhentriactc, u64::MAX, "k14: NHHENTRIACTC=u64::MAX (4\u{00d7}8\u{00b3}\u{2070}>>u64::MAX; saturated)");
    assert_eq!(nbso,         u64::MAX, "k14: NBSO=u64::MAX (4\u{00d7}32\u{00b2}\u{2075}>>u64::MAX; saturated)");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1. S: S(A)=2,S(B)=3,S(C)=3,S(D)=2. 3 edges, 4 nodes.
// NHENTRIACTC:  2^31+3^31+3^31+2^31 = 2×2_147_483_648+2×617_673_396_283_947 = 1_235_351_087_535_190.
// NHHENTRIACTC: (2+3)^30+(3+3)^30+(3+2)^30 = 2×5^30+6^30 → 5^30>>u64::MAX → SATURATES.
// NBSO:         13^25+18^25+13^25 — 13^25>>u64::MAX per-edge → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T57_VEC_A, T57_KEY_A, T57_ID_A);
    add_node(T57_VEC_B, T57_KEY_B, T57_ID_B);
    add_node(T57_VEC_C, T57_KEY_C, T57_ID_C);
    add_node(T57_VEC_D, T57_KEY_D, T57_ID_D);
    add_edge(T57_ID_A, T57_ID_B, "t57.e.ab");
    add_edge(T57_ID_B, T57_ID_C, "t57.e.bc");
    add_edge(T57_ID_C, T57_ID_D, "t57.e.cd");

    let (nhentriactc, nhhentriactc, nbso, ec, nc) = gos_runtime::graph_topo_indices57();
    assert_eq!(nc,           4,                     "p4: node_count=4");
    assert_eq!(ec,           3,                     "p4: edge_count=3");
    assert_eq!(nhentriactc,  1_235_351_087_535_190, "p4: NHENTRIACTC=1_235_351_087_535_190 (2\u{00d7}2\u{00b3}\u{00b9}+2\u{00d7}3\u{00b3}\u{00b9}; 3\u{00b3}\u{00b9}=617_673_396_283_947)");
    assert_eq!(nhhentriactc, u64::MAX,              "p4: NHHENTRIACTC=u64::MAX (5\u{00b3}\u{2070}>>u64::MAX per-edge; saturated)");
    assert_eq!(nbso,         u64::MAX,              "p4: NBSO=u64::MAX (13\u{00b2}\u{2075}>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NHENTRIACTC:  4×9^31 → SATURATES → u64::MAX.
// NHHENTRIACTC: 6×18^30 → SATURATES → u64::MAX.
// NBSO:         6×162^25 → SATURATES → u64::MAX.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T57_VEC_A, T57_KEY_A, T57_ID_A);
    add_node(T57_VEC_B, T57_KEY_B, T57_ID_B);
    add_node(T57_VEC_C, T57_KEY_C, T57_ID_C);
    add_node(T57_VEC_D, T57_KEY_D, T57_ID_D);
    add_edge(T57_ID_A, T57_ID_B, "t57.e.ab");
    add_edge(T57_ID_B, T57_ID_A, "t57.e.ba");
    add_edge(T57_ID_A, T57_ID_C, "t57.e.ac");
    add_edge(T57_ID_C, T57_ID_A, "t57.e.ca");
    add_edge(T57_ID_A, T57_ID_D, "t57.e.ad");
    add_edge(T57_ID_D, T57_ID_A, "t57.e.da");
    add_edge(T57_ID_B, T57_ID_C, "t57.e.bc");
    add_edge(T57_ID_C, T57_ID_B, "t57.e.cb");
    add_edge(T57_ID_B, T57_ID_D, "t57.e.bd");
    add_edge(T57_ID_D, T57_ID_B, "t57.e.db");
    add_edge(T57_ID_C, T57_ID_D, "t57.e.cd");
    add_edge(T57_ID_D, T57_ID_C, "t57.e.dc");

    let (nhentriactc, nhhentriactc, nbso, ec, nc) = gos_runtime::graph_topo_indices57();
    assert_eq!(nc,           4,        "k4: node_count=4");
    assert_eq!(ec,           6,        "k4: edge_count=6");
    assert_eq!(nhentriactc,  u64::MAX, "k4: NHENTRIACTC=u64::MAX (4\u{00d7}9\u{00b3}\u{00b9} >> u64::MAX; saturated)");
    assert_eq!(nhhentriactc, u64::MAX, "k4: NHHENTRIACTC=u64::MAX (6\u{00d7}18\u{00b3}\u{2070} >> u64::MAX; saturated)");
    assert_eq!(nbso,         u64::MAX, "k4: NBSO=u64::MAX (6\u{00d7}162\u{00b2}\u{2075} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NHENTRIACTC=0; NHHENTRIACTC=0; NBSO=0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T57_VEC_A, T57_KEY_A, T57_ID_A);
    add_node(T57_VEC_B, T57_KEY_B, T57_ID_B);

    let (nhentriactc, nhhentriactc, nbso, ec, nc) = gos_runtime::graph_topo_indices57();
    assert_eq!(nc,           2, "two-iso: node_count=2");
    assert_eq!(ec,           0, "two-iso: edge_count=0");
    assert_eq!(nhentriactc,  0, "two-iso: NHENTRIACTC=0");
    assert_eq!(nhhentriactc, 0, "two-iso: NHHENTRIACTC=0");
    assert_eq!(nbso,         0, "two-iso: NBSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NHENTRIACTC:  5×6^31 → SATURATES (6^31 >> u64::MAX per-node).
// NHHENTRIACTC: 6×12^30 → SATURATES (12^30>>u64::MAX per-edge).
// NBSO:         6×72^25 → SATURATES (per-edge >> u64::MAX).
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T57_VEC_A, T57_KEY_A, T57_ID_A);
    add_node(T57_VEC_B, T57_KEY_B, T57_ID_B);
    add_node(T57_VEC_C, T57_KEY_C, T57_ID_C);
    add_node(T57_VEC_D, T57_KEY_D, T57_ID_D);
    add_node(T57_VEC_E, T57_KEY_E, T57_ID_E);
    add_edge(T57_ID_A, T57_ID_C, "t57.e.ac");
    add_edge(T57_ID_A, T57_ID_D, "t57.e.ad");
    add_edge(T57_ID_A, T57_ID_E, "t57.e.ae");
    add_edge(T57_ID_B, T57_ID_C, "t57.e.bc");
    add_edge(T57_ID_B, T57_ID_D, "t57.e.bd");
    add_edge(T57_ID_B, T57_ID_E, "t57.e.be");

    let (nhentriactc, nhhentriactc, nbso, ec, nc) = gos_runtime::graph_topo_indices57();
    assert_eq!(nc,           5,        "k23: node_count=5");
    assert_eq!(ec,           6,        "k23: edge_count=6");
    assert_eq!(nhentriactc,  u64::MAX, "k23: NHENTRIACTC=u64::MAX (5\u{00d7}6\u{00b3}\u{00b9}; 6\u{00b3}\u{00b9}>>u64::MAX per-node; saturated)");
    assert_eq!(nhhentriactc, u64::MAX, "k23: NHHENTRIACTC=u64::MAX (6\u{00d7}12\u{00b3}\u{2070} >> u64::MAX; per-edge saturates)");
    assert_eq!(nbso,         u64::MAX, "k23: NBSO=u64::MAX (6\u{00d7}72\u{00b2}\u{2075} >> u64::MAX; per-edge saturates)");
}
