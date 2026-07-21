// gos-graph-topo87-harness — V3.98 NHEXAENACTC + NHHEXAENACTC + NBDSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices87()`:
//   Returns (nhexaenactc, nhhexaenactc, nbdso, edge_count, node_count)
//   - nhexaenactc  = NHEXAENACTC(G) = Σ_v S(v)^61                   (exact u64; S-Hexaencontic vertex sum)
//   - nhhexaenactc = NHHEXAENACTC(G) = Σ_{uv∈E} (S_u+S_v)^60        (exact u64; S-Hexacontic edge-sum)
//   - nbdso        = NBDSO(G)        = Σ_{uv∈E} (S_u²+S_v²)^55      (exact u64; S-Variant Sombor, α=110)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEXAENACTC(G) = Σ_v S(v)^61
//     S-Hexaencontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NHEXAACTC=Σ S⁶⁰ (topo86), NHEXAENACTC=Σ S⁶¹ (topo87).
//     Second of the hexacontic (60-69) series.
//     NHEXAENACTC = n·S^61 for S-regular.
//     Overflow: S^61 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^61 = s32 × s16 × s8 × s4 × s  (61=32+16+8+4+1; 5 mults).
//
//   NHHEXAENACTC(G) = Σ_{uv∈E} (S_u+S_v)^60
//     S-Hexacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHHEXAACTC=Σ(S+S)⁵⁹ (topo86),
//       NHHEXAENACTC=Σ(S+S)⁶⁰ (topo87).
//     NHHEXAENACTC = |E|·(2S)^60 = 1152921504606846976|E|·S^60 for S-regular.
//     Overflow per edge: (2×16129)^60 → saturating u128 accumulator.
//     Implementation: ss^60 = ss32 × ss16 × ss8 × ss4  (60=32+16+8+4; 4 mults — efficient!).
//
//   NBDSO(G) = Σ_{uv∈E} (S_u²+S_v²)^55
//     S-Variant Sombor: generalised Sombor SO^α with α=110 on S-variant.
//     4th of NB series, letter D (after NBASO α=104 topo84, NBBSO α=106 topo85, NBCSO α=108 topo86).
//     NSO(topo21,α=1),..., NBCSO(topo86,α=108), NBDSO(topo87,α=110).
//     NBDSO = |E|·(2S²)^55 = 36028797018963968|E|·S^110 for S-regular.
//     Overflow per edge: (2×16129²)^55 → saturating u128 accumulator.
//     Implementation: s2s^55 = s2s32 × s2s16 × s2s4 × s2s2 × s2s  (55=32+16+4+2+1; 5 mults).
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
//  Graph     NHEXAENACTC(exact)              NHHEXAENACTC(exact)       NBDSO(exact)            edges  nodes
//  Empty                    0                              0                    0                0      0
//  1 node                   0                              0                    0                0      1
//  K₂                       2           1_152_921_504_606_846_976   36_028_797_018_963_968       1      2
//  P₃     6_917_529_027_641_081_856          u64::MAX(sat.)             u64::MAX(sat.)           2      3
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
//     NHEXAENACTC:  1^61 + 1^61 = 2. ✓
//     NHHEXAENACTC: (1+1)^60 = 2^60 = 1_152_921_504_606_846_976. ✓
//     NBDSO:        (1²+1²)^55 = 2^55 = 36_028_797_018_963_968. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEXAENACTC:  3×2^61 = 3×2_305_843_009_213_693_952 = 6_917_529_027_641_081_856. ✓
//     NHHEXAENACTC: 2×(2+2)^60 = 2×4^60 = 2×2^120 → SATURATES. ✓
//     NBDSO:        2×(4+4)^55 = 2×8^55 = 2×2^165 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEXAENACTC:  3×4^61 = 3×2^122 → SATURATES. ✓
//     NHHEXAENACTC: 3×8^60 → SATURATES. ✓
//     NBDSO:        3×32^55 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEXAENACTC:  5×4^61 → SATURATES. ✓
//     NHHEXAENACTC: 4×8^60 → SATURATES. ✓
//     NBDSO:        4×32^55 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEXAENACTC:  2×2^61 + 2×3^61.  3^39>u64::MAX → 3^61 >> u64::MAX → SATURATES. ✓
//     NHHEXAENACTC: 5^60+6^60+5^60 → each term >> u64::MAX → SATURATES. ✓
//     NBDSO:        13^55+18^55+13^55 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEXAENACTC:  4×9^61 → SATURATES. ✓
//     NHHEXAENACTC: 6×18^60 → SATURATES. ✓
//     NBDSO:        6×162^55 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEXAENACTC:  5×6^61 → SATURATES. ✓
//     NHHEXAENACTC: 6×12^60 → SATURATES. ✓
//     NBDSO:        6×72^55 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEXAENACTC  = n·S^61                                                                             for S-regular ✓
//   NHHEXAENACTC = |E|·(2S)^60 = 1152921504606846976|E|·S^60                                         for S-regular ✓
//   NBDSO        = |E|·(2S²)^55 = 36028797018963968|E|·S^110                                         for S-regular ✓
//   Note: ss^60=ss32×ss16×ss8×ss4 is efficient (60=32+16+8+4, four powers of 2, only 4 mults)
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 1_152_921_504_606_846_976, 36_028_797_018_963_968, 1, 2)
//  4.  Path P₃ = A-B-C                   → (6_917_529_027_641_081_856, u64::MAX, u64::MAX, 2, 3)
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

const T87_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_87");
const T87_EXEC:   ExecutorId = ExecutorId::from_ascii("t87.exec");

const T87_KEY_A: &str = "t87.alpha";
const T87_KEY_B: &str = "t87.beta";
const T87_KEY_C: &str = "t87.gamma";
const T87_KEY_D: &str = "t87.delta";
const T87_KEY_E: &str = "t87.epsilon";

const T87_ID_A: NodeId = derive_node_id(T87_PLUGIN, T87_KEY_A);
const T87_ID_B: NodeId = derive_node_id(T87_PLUGIN, T87_KEY_B);
const T87_ID_C: NodeId = derive_node_id(T87_PLUGIN, T87_KEY_C);
const T87_ID_D: NodeId = derive_node_id(T87_PLUGIN, T87_KEY_D);
const T87_ID_E: NodeId = derive_node_id(T87_PLUGIN, T87_KEY_E);

// L4=174 namespace for this harness.
const T87_VEC_A: VectorAddress = VectorAddress::new(174, 1, 1, 0);
const T87_VEC_B: VectorAddress = VectorAddress::new(174, 1, 2, 0);
const T87_VEC_C: VectorAddress = VectorAddress::new(174, 1, 3, 0);
const T87_VEC_D: VectorAddress = VectorAddress::new(174, 2, 1, 0);
const T87_VEC_E: VectorAddress = VectorAddress::new(174, 2, 2, 0);

const T87_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T87_PLUGIN,
    name:         "kl-graph-topo87-harness",
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
        executor_id:       T87_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T87_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T87_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nhexaenactc, nhhexaenactc, nbdso, ec, nc) = gos_runtime::graph_topo_indices87();
    assert_eq!(nc,            0, "empty: node_count=0");
    assert_eq!(ec,            0, "empty: edge_count=0");
    assert_eq!(nhexaenactc,   0, "empty: NHEXAENACTC=0");
    assert_eq!(nhhexaenactc,  0, "empty: NHHEXAENACTC=0");
    assert_eq!(nbdso,         0, "empty: NBDSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T87_VEC_A, T87_KEY_A, T87_ID_A);

    let (nhexaenactc, nhhexaenactc, nbdso, ec, nc) = gos_runtime::graph_topo_indices87();
    assert_eq!(nc,            1, "single: node_count=1");
    assert_eq!(ec,            0, "single: edge_count=0");
    assert_eq!(nhexaenactc,   0, "single: NHEXAENACTC=0");
    assert_eq!(nhhexaenactc,  0, "single: NHHEXAENACTC=0");
    assert_eq!(nbdso,         0, "single: NBDSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEXAENACTC:  1^61+1^61 = 2.
// NHHEXAENACTC: (1+1)^60 = 2^60 = 1_152_921_504_606_846_976.
// NBDSO:        (1²+1²)^55 = 2^55 = 36_028_797_018_963_968.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T87_VEC_A, T87_KEY_A, T87_ID_A);
    add_node(T87_VEC_B, T87_KEY_B, T87_ID_B);
    add_edge(T87_ID_A, T87_ID_B, "t87.e.ab");

    let (nhexaenactc, nhhexaenactc, nbdso, ec, nc) = gos_runtime::graph_topo_indices87();
    assert_eq!(nc,            2,                           "k2: node_count=2");
    assert_eq!(ec,            1,                           "k2: edge_count=1");
    assert_eq!(nhexaenactc,   2,                           "k2: NHEXAENACTC=2 (1\u{2076}\u{00b9}+1\u{2076}\u{00b9}=2)");
    assert_eq!(nhhexaenactc,  1_152_921_504_606_846_976,   "k2: NHHEXAENACTC=1_152_921_504_606_846_976 (2\u{2076}\u{2070}=2^60)");
    assert_eq!(nbdso,         36_028_797_018_963_968,      "k2: NBDSO=36_028_797_018_963_968 (2\u{2075}\u{2075}=2^55)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NHEXAENACTC:  3×2^61 = 3×2_305_843_009_213_693_952 = 6_917_529_027_641_081_856.
// NHHEXAENACTC: 2×(2+2)^60 = 2×4^60 = 2×2^120 → SATURATES.
// NBDSO:        2×(4+4)^55 = 2×8^55 = 2×2^165 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T87_VEC_A, T87_KEY_A, T87_ID_A);
    add_node(T87_VEC_B, T87_KEY_B, T87_ID_B);
    add_node(T87_VEC_C, T87_KEY_C, T87_ID_C);
    add_edge(T87_ID_A, T87_ID_B, "t87.e.ab");
    add_edge(T87_ID_B, T87_ID_C, "t87.e.bc");

    let (nhexaenactc, nhhexaenactc, nbdso, ec, nc) = gos_runtime::graph_topo_indices87();
    assert_eq!(nc,            3,                           "p3: node_count=3");
    assert_eq!(ec,            2,                           "p3: edge_count=2");
    assert_eq!(nhexaenactc,   6_917_529_027_641_081_856,   "p3: NHEXAENACTC=6_917_529_027_641_081_856 (3\u{00d7}2\u{2076}\u{00b9})");
    assert_eq!(nhhexaenactc,  u64::MAX,                    "p3: NHHEXAENACTC=SAT (4\u{2076}\u{2070}>u64)");
    assert_eq!(nbdso,         u64::MAX,                    "p3: NBDSO=SAT (8\u{2075}\u{2075}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T87_VEC_A, T87_KEY_A, T87_ID_A);
    add_node(T87_VEC_B, T87_KEY_B, T87_ID_B);
    add_node(T87_VEC_C, T87_KEY_C, T87_ID_C);
    add_edge(T87_ID_A, T87_ID_B, "t87.e.ab");
    add_edge(T87_ID_B, T87_ID_C, "t87.e.bc");
    add_edge(T87_ID_C, T87_ID_A, "t87.e.ca");

    let (nhexaenactc, nhhexaenactc, nbdso, ec, nc) = gos_runtime::graph_topo_indices87();
    assert_eq!(nc,           3,        "k3: node_count=3");
    assert_eq!(ec,           3,        "k3: edge_count=3");
    assert_eq!(nhexaenactc,  u64::MAX, "k3: NHEXAENACTC=SAT");
    assert_eq!(nhhexaenactc, u64::MAX, "k3: NHHEXAENACTC=SAT");
    assert_eq!(nbdso,        u64::MAX, "k3: NBDSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T87_VEC_A, T87_KEY_A, T87_ID_A); // hub
    add_node(T87_VEC_B, T87_KEY_B, T87_ID_B);
    add_node(T87_VEC_C, T87_KEY_C, T87_ID_C);
    add_node(T87_VEC_D, T87_KEY_D, T87_ID_D);
    add_node(T87_VEC_E, T87_KEY_E, T87_ID_E);
    add_edge(T87_ID_A, T87_ID_B, "t87.e.ab");
    add_edge(T87_ID_A, T87_ID_C, "t87.e.ac");
    add_edge(T87_ID_A, T87_ID_D, "t87.e.ad");
    add_edge(T87_ID_A, T87_ID_E, "t87.e.ae");

    let (nhexaenactc, nhhexaenactc, nbdso, ec, nc) = gos_runtime::graph_topo_indices87();
    assert_eq!(nc,           5,        "k14: node_count=5");
    assert_eq!(ec,           4,        "k14: edge_count=4");
    assert_eq!(nhexaenactc,  u64::MAX, "k14: NHEXAENACTC=SAT");
    assert_eq!(nhhexaenactc, u64::MAX, "k14: NHHEXAENACTC=SAT");
    assert_eq!(nbdso,        u64::MAX, "k14: NBDSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NHEXAENACTC:  2×2^61 + 2×3^61.  3^39>u64::MAX → SATURATES.
// NHHEXAENACTC: 5^60+6^60+5^60 → SATURATES.
// NBDSO:        13^55+18^55+13^55 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T87_VEC_A, T87_KEY_A, T87_ID_A);
    add_node(T87_VEC_B, T87_KEY_B, T87_ID_B);
    add_node(T87_VEC_C, T87_KEY_C, T87_ID_C);
    add_node(T87_VEC_D, T87_KEY_D, T87_ID_D);
    add_edge(T87_ID_A, T87_ID_B, "t87.e.ab");
    add_edge(T87_ID_B, T87_ID_C, "t87.e.bc");
    add_edge(T87_ID_C, T87_ID_D, "t87.e.cd");

    let (nhexaenactc, nhhexaenactc, nbdso, ec, nc) = gos_runtime::graph_topo_indices87();
    assert_eq!(nc,           4,        "p4: node_count=4");
    assert_eq!(ec,           3,        "p4: edge_count=3");
    assert_eq!(nhexaenactc,  u64::MAX, "p4: NHEXAENACTC=SAT");
    assert_eq!(nhhexaenactc, u64::MAX, "p4: NHHEXAENACTC=SAT");
    assert_eq!(nbdso,        u64::MAX, "p4: NBDSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T87_VEC_A, T87_KEY_A, T87_ID_A);
    add_node(T87_VEC_B, T87_KEY_B, T87_ID_B);
    add_node(T87_VEC_C, T87_KEY_C, T87_ID_C);
    add_node(T87_VEC_D, T87_KEY_D, T87_ID_D);
    add_edge(T87_ID_A, T87_ID_B, "t87.e.ab");
    add_edge(T87_ID_A, T87_ID_C, "t87.e.ac");
    add_edge(T87_ID_A, T87_ID_D, "t87.e.ad");
    add_edge(T87_ID_B, T87_ID_C, "t87.e.bc");
    add_edge(T87_ID_B, T87_ID_D, "t87.e.bd");
    add_edge(T87_ID_C, T87_ID_D, "t87.e.cd");

    let (nhexaenactc, nhhexaenactc, nbdso, ec, nc) = gos_runtime::graph_topo_indices87();
    assert_eq!(nc,           4,        "k4: node_count=4");
    assert_eq!(ec,           6,        "k4: edge_count=6");
    assert_eq!(nhexaenactc,  u64::MAX, "k4: NHEXAENACTC=SAT");
    assert_eq!(nhhexaenactc, u64::MAX, "k4: NHHEXAENACTC=SAT");
    assert_eq!(nbdso,        u64::MAX, "k4: NBDSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T87_VEC_A, T87_KEY_A, T87_ID_A);
    add_node(T87_VEC_B, T87_KEY_B, T87_ID_B);

    let (nhexaenactc, nhhexaenactc, nbdso, ec, nc) = gos_runtime::graph_topo_indices87();
    assert_eq!(nc,            2, "isolated: node_count=2");
    assert_eq!(ec,            0, "isolated: edge_count=0");
    assert_eq!(nhexaenactc,   0, "isolated: NHEXAENACTC=0");
    assert_eq!(nhhexaenactc,  0, "isolated: NHHEXAENACTC=0");
    assert_eq!(nbdso,         0, "isolated: NBDSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NHEXAENACTC=5×6^61 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T87_VEC_A, T87_KEY_A, T87_ID_A);
    add_node(T87_VEC_B, T87_KEY_B, T87_ID_B);
    add_node(T87_VEC_C, T87_KEY_C, T87_ID_C);
    add_node(T87_VEC_D, T87_KEY_D, T87_ID_D);
    add_node(T87_VEC_E, T87_KEY_E, T87_ID_E);
    add_edge(T87_ID_A, T87_ID_C, "t87.e.ac");
    add_edge(T87_ID_A, T87_ID_D, "t87.e.ad");
    add_edge(T87_ID_A, T87_ID_E, "t87.e.ae");
    add_edge(T87_ID_B, T87_ID_C, "t87.e.bc");
    add_edge(T87_ID_B, T87_ID_D, "t87.e.bd");
    add_edge(T87_ID_B, T87_ID_E, "t87.e.be");

    let (nhexaenactc, nhhexaenactc, nbdso, ec, nc) = gos_runtime::graph_topo_indices87();
    assert_eq!(nc,           5,        "k23: node_count=5");
    assert_eq!(ec,           6,        "k23: edge_count=6");
    assert_eq!(nhexaenactc,  u64::MAX, "k23: NHEXAENACTC=SAT (5\u{00d7}6\u{2076}\u{00b9})");
    assert_eq!(nhhexaenactc, u64::MAX, "k23: NHHEXAENACTC=SAT");
    assert_eq!(nbdso,        u64::MAX, "k23: NBDSO=SAT");
}
