// gos-graph-topo88-harness — V3.99 NHEXADYACTC + NHHEXADYACTC + NBESO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices88()`:
//   Returns (nhexadyactc, nhhexadyactc, nbeso, edge_count, node_count)
//   - nhexadyactc  = NHEXADYACTC(G) = Σ_v S(v)^62                   (exact u64; S-Hexadycontic vertex sum)
//   - nhhexadyactc = NHHEXADYACTC(G) = Σ_{uv∈E} (S_u+S_v)^61        (exact u64; S-Hexaencontic edge-sum)
//   - nbeso        = NBESO(G)        = Σ_{uv∈E} (S_u²+S_v²)^56      (exact u64; S-Variant Sombor, α=112)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEXADYACTC(G) = Σ_v S(v)^62
//     S-Hexadycontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NHEXAENACTC=Σ S⁶¹ (topo87), NHEXADYACTC=Σ S⁶² (topo88).
//     Third of the hexacontic (60-69) series.
//     NHEXADYACTC = n·S^62 for S-regular.
//     Overflow: S^62 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^62 = s32 × s16 × s8 × s4 × s2  (62=32+16+8+4+2; 5 mults).
//
//   NHHEXADYACTC(G) = Σ_{uv∈E} (S_u+S_v)^61
//     S-Hexaencontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHHEXAENACTC=Σ(S+S)⁶⁰ (topo87),
//       NHHEXADYACTC=Σ(S+S)⁶¹ (topo88).
//     NHHEXADYACTC = |E|·(2S)^61 = 2305843009213693952|E|·S^61 for S-regular.
//     Overflow per edge: (2×16129)^61 → saturating u128 accumulator.
//     Implementation: ss^61 = ss32 × ss16 × ss8 × ss4 × ss  (61=32+16+8+4+1; 5 mults).
//
//   NBESO(G) = Σ_{uv∈E} (S_u²+S_v²)^56
//     S-Variant Sombor: generalised Sombor SO^α with α=112 on S-variant.
//     5th of NB series, letter E (after NBDSO α=110 topo87).
//     NSO(topo21,α=1),..., NBDSO(topo87,α=110), NBESO(topo88,α=112).
//     NBESO = |E|·(2S²)^56 = 72057594037927936|E|·S^112 for S-regular.
//     Overflow per edge: (2×16129²)^56 → saturating u128 accumulator.
//     Implementation: s2s^56 = s2s32 × s2s16 × s2s8  (56=32+16+8; 3 mults — efficient!).
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
//  Graph     NHEXADYACTC(exact)              NHHEXADYACTC(exact)       NBESO(exact)            edges  nodes
//  Empty                    0                              0                    0                0      0
//  1 node                   0                              0                    0                0      1
//  K₂                       2           2_305_843_009_213_693_952   72_057_594_037_927_936       1      2
//  P₃    13_835_058_055_282_163_712          u64::MAX(sat.)             u64::MAX(sat.)           2      3
//  K₃              u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)           3      3
//  K_{1,4}         u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)           4      5
//  P₄              u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)           3      4
//  K₄              u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)           6      4
//  2 isolated               0                              0                    0                0      2
//  K_{2,3}         u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)           6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEXADYACTC:  1^62 + 1^62 = 2. ✓
//     NHHEXADYACTC: (1+1)^61 = 2^61 = 2_305_843_009_213_693_952. ✓
//     NBESO:        (1²+1²)^56 = 2^56 = 72_057_594_037_927_936. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEXADYACTC:  3×2^62 = 3×4_611_686_018_427_387_904 = 13_835_058_055_282_163_712. ✓
//     NHHEXADYACTC: 2×(2+2)^61 = 2×4^61 = 2×2^122 → SATURATES. ✓
//     NBESO:        2×(4+4)^56 = 2×8^56 = 2×2^168 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEXADYACTC:  3×4^62 = 3×2^124 → SATURATES. ✓
//     NHHEXADYACTC: 3×8^61 → SATURATES. ✓
//     NBESO:        3×32^56 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEXADYACTC:  5×4^62 → SATURATES. ✓
//     NHHEXADYACTC: 4×8^61 → SATURATES. ✓
//     NBESO:        4×32^56 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEXADYACTC:  2×2^62 + 2×3^62.  3^40>u64::MAX → 3^62 >> u64::MAX → SATURATES. ✓
//     NHHEXADYACTC: 5^61+6^61+5^61 → each term >> u64::MAX → SATURATES. ✓
//     NBESO:        13^56+18^56+13^56 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEXADYACTC:  4×9^62 → SATURATES. ✓
//     NHHEXADYACTC: 6×18^61 → SATURATES. ✓
//     NBESO:        6×162^56 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEXADYACTC:  5×6^62 → SATURATES. ✓
//     NHHEXADYACTC: 6×12^61 → SATURATES. ✓
//     NBESO:        6×72^56 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEXADYACTC  = n·S^62                                                                             for S-regular ✓
//   NHHEXADYACTC = |E|·(2S)^61 = 2305843009213693952|E|·S^61                                         for S-regular ✓
//   NBESO        = |E|·(2S²)^56 = 72057594037927936|E|·S^112                                         for S-regular ✓
//   Note: s2s^56=s2s32×s2s16×s2s8 is efficient (56=32+16+8, three powers of 2, only 3 mults)
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 2_305_843_009_213_693_952, 72_057_594_037_927_936, 1, 2)
//  4.  Path P₃ = A-B-C                   → (13_835_058_055_282_163_712, u64::MAX, u64::MAX, 2, 3)
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

const T88_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_88");
const T88_EXEC:   ExecutorId = ExecutorId::from_ascii("t88.exec");

const T88_KEY_A: &str = "t88.alpha";
const T88_KEY_B: &str = "t88.beta";
const T88_KEY_C: &str = "t88.gamma";
const T88_KEY_D: &str = "t88.delta";
const T88_KEY_E: &str = "t88.epsilon";

const T88_ID_A: NodeId = derive_node_id(T88_PLUGIN, T88_KEY_A);
const T88_ID_B: NodeId = derive_node_id(T88_PLUGIN, T88_KEY_B);
const T88_ID_C: NodeId = derive_node_id(T88_PLUGIN, T88_KEY_C);
const T88_ID_D: NodeId = derive_node_id(T88_PLUGIN, T88_KEY_D);
const T88_ID_E: NodeId = derive_node_id(T88_PLUGIN, T88_KEY_E);

// L4=175 namespace for this harness.
const T88_VEC_A: VectorAddress = VectorAddress::new(175, 1, 1, 0);
const T88_VEC_B: VectorAddress = VectorAddress::new(175, 1, 2, 0);
const T88_VEC_C: VectorAddress = VectorAddress::new(175, 1, 3, 0);
const T88_VEC_D: VectorAddress = VectorAddress::new(175, 2, 1, 0);
const T88_VEC_E: VectorAddress = VectorAddress::new(175, 2, 2, 0);

const T88_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T88_PLUGIN,
    name:         "kl-graph-topo88-harness",
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
        executor_id:       T88_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T88_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T88_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nhexadyactc, nhhexadyactc, nbeso, ec, nc) = gos_runtime::graph_topo_indices88();
    assert_eq!(nc,            0, "empty: node_count=0");
    assert_eq!(ec,            0, "empty: edge_count=0");
    assert_eq!(nhexadyactc,   0, "empty: NHEXADYACTC=0");
    assert_eq!(nhhexadyactc,  0, "empty: NHHEXADYACTC=0");
    assert_eq!(nbeso,         0, "empty: NBESO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T88_VEC_A, T88_KEY_A, T88_ID_A);

    let (nhexadyactc, nhhexadyactc, nbeso, ec, nc) = gos_runtime::graph_topo_indices88();
    assert_eq!(nc,            1, "single: node_count=1");
    assert_eq!(ec,            0, "single: edge_count=0");
    assert_eq!(nhexadyactc,   0, "single: NHEXADYACTC=0");
    assert_eq!(nhhexadyactc,  0, "single: NHHEXADYACTC=0");
    assert_eq!(nbeso,         0, "single: NBESO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEXADYACTC:  1^62+1^62 = 2.
// NHHEXADYACTC: (1+1)^61 = 2^61 = 2_305_843_009_213_693_952.
// NBESO:        (1²+1²)^56 = 2^56 = 72_057_594_037_927_936.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T88_VEC_A, T88_KEY_A, T88_ID_A);
    add_node(T88_VEC_B, T88_KEY_B, T88_ID_B);
    add_edge(T88_ID_A, T88_ID_B, "t88.e.ab");

    let (nhexadyactc, nhhexadyactc, nbeso, ec, nc) = gos_runtime::graph_topo_indices88();
    assert_eq!(nc,            2,                           "k2: node_count=2");
    assert_eq!(ec,            1,                           "k2: edge_count=1");
    assert_eq!(nhexadyactc,   2,                           "k2: NHEXADYACTC=2 (1\u{2076}\u{00b2}+1\u{2076}\u{00b2}=2)");
    assert_eq!(nhhexadyactc,  2_305_843_009_213_693_952,   "k2: NHHEXADYACTC=2_305_843_009_213_693_952 (2\u{2076}\u{00b9}=2^61)");
    assert_eq!(nbeso,         72_057_594_037_927_936,      "k2: NBESO=72_057_594_037_927_936 (2\u{2075}\u{2076}=2^56)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NHEXADYACTC:  3×2^62 = 3×4_611_686_018_427_387_904 = 13_835_058_055_282_163_712.
// NHHEXADYACTC: 2×(2+2)^61 = 2×4^61 = 2×2^122 → SATURATES.
// NBESO:        2×(4+4)^56 = 2×8^56 = 2×2^168 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T88_VEC_A, T88_KEY_A, T88_ID_A);
    add_node(T88_VEC_B, T88_KEY_B, T88_ID_B);
    add_node(T88_VEC_C, T88_KEY_C, T88_ID_C);
    add_edge(T88_ID_A, T88_ID_B, "t88.e.ab");
    add_edge(T88_ID_B, T88_ID_C, "t88.e.bc");

    let (nhexadyactc, nhhexadyactc, nbeso, ec, nc) = gos_runtime::graph_topo_indices88();
    assert_eq!(nc,            3,                            "p3: node_count=3");
    assert_eq!(ec,            2,                            "p3: edge_count=2");
    assert_eq!(nhexadyactc,   13_835_058_055_282_163_712,   "p3: NHEXADYACTC=13_835_058_055_282_163_712 (3\u{00d7}2\u{2076}\u{00b2})");
    assert_eq!(nhhexadyactc,  u64::MAX,                     "p3: NHHEXADYACTC=SAT (4\u{2076}\u{00b9}>u64)");
    assert_eq!(nbeso,         u64::MAX,                     "p3: NBESO=SAT (8\u{2075}\u{2076}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T88_VEC_A, T88_KEY_A, T88_ID_A);
    add_node(T88_VEC_B, T88_KEY_B, T88_ID_B);
    add_node(T88_VEC_C, T88_KEY_C, T88_ID_C);
    add_edge(T88_ID_A, T88_ID_B, "t88.e.ab");
    add_edge(T88_ID_B, T88_ID_C, "t88.e.bc");
    add_edge(T88_ID_C, T88_ID_A, "t88.e.ca");

    let (nhexadyactc, nhhexadyactc, nbeso, ec, nc) = gos_runtime::graph_topo_indices88();
    assert_eq!(nc,           3,        "k3: node_count=3");
    assert_eq!(ec,           3,        "k3: edge_count=3");
    assert_eq!(nhexadyactc,  u64::MAX, "k3: NHEXADYACTC=SAT");
    assert_eq!(nhhexadyactc, u64::MAX, "k3: NHHEXADYACTC=SAT");
    assert_eq!(nbeso,        u64::MAX, "k3: NBESO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T88_VEC_A, T88_KEY_A, T88_ID_A); // hub
    add_node(T88_VEC_B, T88_KEY_B, T88_ID_B);
    add_node(T88_VEC_C, T88_KEY_C, T88_ID_C);
    add_node(T88_VEC_D, T88_KEY_D, T88_ID_D);
    add_node(T88_VEC_E, T88_KEY_E, T88_ID_E);
    add_edge(T88_ID_A, T88_ID_B, "t88.e.ab");
    add_edge(T88_ID_A, T88_ID_C, "t88.e.ac");
    add_edge(T88_ID_A, T88_ID_D, "t88.e.ad");
    add_edge(T88_ID_A, T88_ID_E, "t88.e.ae");

    let (nhexadyactc, nhhexadyactc, nbeso, ec, nc) = gos_runtime::graph_topo_indices88();
    assert_eq!(nc,           5,        "k14: node_count=5");
    assert_eq!(ec,           4,        "k14: edge_count=4");
    assert_eq!(nhexadyactc,  u64::MAX, "k14: NHEXADYACTC=SAT");
    assert_eq!(nhhexadyactc, u64::MAX, "k14: NHHEXADYACTC=SAT");
    assert_eq!(nbeso,        u64::MAX, "k14: NBESO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NHEXADYACTC:  2×2^62 + 2×3^62.  3^40>u64::MAX → 3^62 >> u64::MAX → SATURATES.
// NHHEXADYACTC: 5^61+6^61+5^61 → SATURATES.
// NBESO:        13^56+18^56+13^56 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T88_VEC_A, T88_KEY_A, T88_ID_A);
    add_node(T88_VEC_B, T88_KEY_B, T88_ID_B);
    add_node(T88_VEC_C, T88_KEY_C, T88_ID_C);
    add_node(T88_VEC_D, T88_KEY_D, T88_ID_D);
    add_edge(T88_ID_A, T88_ID_B, "t88.e.ab");
    add_edge(T88_ID_B, T88_ID_C, "t88.e.bc");
    add_edge(T88_ID_C, T88_ID_D, "t88.e.cd");

    let (nhexadyactc, nhhexadyactc, nbeso, ec, nc) = gos_runtime::graph_topo_indices88();
    assert_eq!(nc,           4,        "p4: node_count=4");
    assert_eq!(ec,           3,        "p4: edge_count=3");
    assert_eq!(nhexadyactc,  u64::MAX, "p4: NHEXADYACTC=SAT");
    assert_eq!(nhhexadyactc, u64::MAX, "p4: NHHEXADYACTC=SAT");
    assert_eq!(nbeso,        u64::MAX, "p4: NBESO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T88_VEC_A, T88_KEY_A, T88_ID_A);
    add_node(T88_VEC_B, T88_KEY_B, T88_ID_B);
    add_node(T88_VEC_C, T88_KEY_C, T88_ID_C);
    add_node(T88_VEC_D, T88_KEY_D, T88_ID_D);
    add_edge(T88_ID_A, T88_ID_B, "t88.e.ab");
    add_edge(T88_ID_A, T88_ID_C, "t88.e.ac");
    add_edge(T88_ID_A, T88_ID_D, "t88.e.ad");
    add_edge(T88_ID_B, T88_ID_C, "t88.e.bc");
    add_edge(T88_ID_B, T88_ID_D, "t88.e.bd");
    add_edge(T88_ID_C, T88_ID_D, "t88.e.cd");

    let (nhexadyactc, nhhexadyactc, nbeso, ec, nc) = gos_runtime::graph_topo_indices88();
    assert_eq!(nc,           4,        "k4: node_count=4");
    assert_eq!(ec,           6,        "k4: edge_count=6");
    assert_eq!(nhexadyactc,  u64::MAX, "k4: NHEXADYACTC=SAT");
    assert_eq!(nhhexadyactc, u64::MAX, "k4: NHHEXADYACTC=SAT");
    assert_eq!(nbeso,        u64::MAX, "k4: NBESO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T88_VEC_A, T88_KEY_A, T88_ID_A);
    add_node(T88_VEC_B, T88_KEY_B, T88_ID_B);

    let (nhexadyactc, nhhexadyactc, nbeso, ec, nc) = gos_runtime::graph_topo_indices88();
    assert_eq!(nc,            2, "isolated: node_count=2");
    assert_eq!(ec,            0, "isolated: edge_count=0");
    assert_eq!(nhexadyactc,   0, "isolated: NHEXADYACTC=0");
    assert_eq!(nhhexadyactc,  0, "isolated: NHHEXADYACTC=0");
    assert_eq!(nbeso,         0, "isolated: NBESO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NHEXADYACTC=5×6^62 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T88_VEC_A, T88_KEY_A, T88_ID_A);
    add_node(T88_VEC_B, T88_KEY_B, T88_ID_B);
    add_node(T88_VEC_C, T88_KEY_C, T88_ID_C);
    add_node(T88_VEC_D, T88_KEY_D, T88_ID_D);
    add_node(T88_VEC_E, T88_KEY_E, T88_ID_E);
    add_edge(T88_ID_A, T88_ID_C, "t88.e.ac");
    add_edge(T88_ID_A, T88_ID_D, "t88.e.ad");
    add_edge(T88_ID_A, T88_ID_E, "t88.e.ae");
    add_edge(T88_ID_B, T88_ID_C, "t88.e.bc");
    add_edge(T88_ID_B, T88_ID_D, "t88.e.bd");
    add_edge(T88_ID_B, T88_ID_E, "t88.e.be");

    let (nhexadyactc, nhhexadyactc, nbeso, ec, nc) = gos_runtime::graph_topo_indices88();
    assert_eq!(nc,           5,        "k23: node_count=5");
    assert_eq!(ec,           6,        "k23: edge_count=6");
    assert_eq!(nhexadyactc,  u64::MAX, "k23: NHEXADYACTC=SAT (5\u{00d7}6\u{2076}\u{00b2})");
    assert_eq!(nhhexadyactc, u64::MAX, "k23: NHHEXADYACTC=SAT");
    assert_eq!(nbeso,        u64::MAX, "k23: NBESO=SAT");
}
