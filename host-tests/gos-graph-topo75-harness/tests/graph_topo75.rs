// gos-graph-topo75-harness — V3.86 NNONATETRAACTC + NHNONATETRAACTC + NARSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices75()`:
//   Returns (nnonatetraactc, nhnonatetraactc, narso, edge_count, node_count)
//   - nnonatetraactc  = NNONATETRAACTC(G) = Σ_v S(v)^49                   (exact u64; S-Nonatetracontic vertex sum)
//   - nhnonatetraactc = NHNONATETRAACTC(G)= Σ_{uv∈E} (S_u+S_v)^48         (exact u64; S-Octotetracontic edge-sum)
//   - narso           = NARSO(G)          = Σ_{uv∈E} (S_u²+S_v²)^43       (exact u64; S-Hexaoctacontyl Sombor, α=86)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NNONATETRAACTC(G) = Σ_v S(v)^49
//     S-Nonatetracontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), ...
//       NHEXTETRAACTC=Σ S⁴⁶ (topo72), NHEPTETRAACTC=Σ S⁴⁷ (topo73),
//       NOCTOTETRAACTC=Σ S⁴⁸ (topo74), NNONATETRAACTC=Σ S⁴⁹ (topo75).
//     NNONATETRAACTC = n·S^49 for S-regular.
//     Overflow: S^49 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^49 = s32 × s16 × s  (s32=s16^2; 49=32+16+1; 3 mults — efficient!).
//
//   NHNONATETRAACTC(G) = Σ_{uv∈E} (S_u+S_v)^48
//     S-Octotetracontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHTETRAACTC=Σ(S+S)³⁹ (topo66),
//       NHHENTETRAACTC=Σ(S+S)⁴⁰ (topo67), NHDOTETRAACTC=Σ(S+S)⁴¹ (topo68),
//       NHTRITETRAACTC=Σ(S+S)⁴² (topo69), NHTETRATETRAACTC=Σ(S+S)⁴³ (topo70),
//       NHPENTETRAACTC=Σ(S+S)⁴⁴ (topo71), NHHEXTETRAACTC=Σ(S+S)⁴⁵ (topo72),
//       NHHEPTETRAACTC=Σ(S+S)⁴⁶ (topo73), NHOCTOTETRAACTC=Σ(S+S)⁴⁷ (topo74),
//       NHNONATETRAACTC=Σ(S+S)⁴⁸ (topo75).
//     NHNONATETRAACTC = |E|·(2S)^48 = 281474976710656|E|·S^48 for S-regular.
//     Overflow per edge: (2×16129)^48 → saturating u128 accumulator.
//     Implementation: ss^48 = ss32 × ss16  (ss32=ss16^2; 48=32+16; 2 mults — very efficient!).
//
//   NARSO(G) = Σ_{uv∈E} (S_u²+S_v²)^43
//     S-Hexaoctacontyl Sombor: generalised Sombor SO^α with α=86 on S-variant.
//     3rd-pass double-letter "AR" (after NAQSO α=84, topo74).
//     NSO(topo21,α=1),..., NAASO(topo58,α=52),..., NAQSO(topo74,α=84), NARSO(topo75,α=86).
//     NARSO = |E|·(2S²)^43 = 8796093022208|E|·S^86 for S-regular.
//     Overflow per edge: (2×16129²)^43 → saturating u128 accumulator.
//     Implementation: s2s^43 = s2s32 × s2s8 × s2s2 × s2s  (43=32+8+2+1; 4 mults).
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
//  Graph     NNONATETRAACTC(exact)          NHNONATETRAACTC(exact)        NARSO(exact)              edges  nodes
//  Empty                       0                              0                       0                0      0
//  1 node                      0                              0                       0                0      1
//  K₂                          2             281_474_976_710_656           8_796_093_022_208               1      2
//  P₃        1_688_849_860_263_936              u64::MAX(sat.)              u64::MAX(sat.)              2      3
//  K₃             u64::MAX(sat.)               u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4}        u64::MAX(sat.)               u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄             u64::MAX(sat.)               u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄             u64::MAX(sat.)               u64::MAX(sat.)              u64::MAX(sat.)              6      4
//  2 isolated                  0                              0                       0                0      2
//  K_{2,3}        u64::MAX(sat.)               u64::MAX(sat.)              u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NNONATETRAACTC:  1^49 + 1^49 = 2. ✓
//     NHNONATETRAACTC: (1+1)^48 = 2^48 = 281_474_976_710_656. ✓
//     NARSO:           (1²+1²)^43 = 2^43 = 8_796_093_022_208. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NNONATETRAACTC:  3×2^49 = 3×562_949_953_421_312 = 1_688_849_860_263_936. ✓
//     NHNONATETRAACTC: 2×(2+2)^48 = 2×4^48 = 2×2^96 → SATURATES. ✓
//     NARSO:           2×(4+4)^43 = 2×8^43 = 2×2^129 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NNONATETRAACTC:  3×4^49 = 3×2^98 → SATURATES. ✓
//     NHNONATETRAACTC: 3×8^48 → SATURATES. ✓
//     NARSO:           3×32^43 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NNONATETRAACTC:  5×4^49 → SATURATES. ✓
//     NHNONATETRAACTC: 4×8^48 → SATURATES. ✓
//     NARSO:           4×32^43 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NNONATETRAACTC:  2×2^49 + 2×3^49.
//       3^49>u64::MAX (since 3^41>u64::MAX) → SATURATES. ✓
//     NHNONATETRAACTC: 2×5^48 + 6^48 → each term >> u64::MAX → SATURATES. ✓
//     NARSO:           2×13^43 + 18^43 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NNONATETRAACTC:  4×9^49 → SATURATES. ✓
//     NHNONATETRAACTC: 6×18^48 → SATURATES. ✓
//     NARSO:           6×162^43 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NNONATETRAACTC:  5×6^49 → SATURATES. ✓
//     NHNONATETRAACTC: 6×12^48 → SATURATES. ✓
//     NARSO:           6×72^43 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NNONATETRAACTC  = n·S^49                                                            for S-regular ✓
//   NHNONATETRAACTC = |E|·(2S)^48 = 281474976710656|E|·S^48                             for S-regular ✓
//   NARSO           = |E|·(2S²)^43 = 8796093022208|E|·S^86                              for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 281_474_976_710_656, 8_796_093_022_208, 1, 2)
//  4.  Path P₃ = A-B-C                   → (1_688_849_860_263_936, u64::MAX, u64::MAX, 2, 3)
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

const T75_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_75");
const T75_EXEC:   ExecutorId = ExecutorId::from_ascii("t75.exec");

const T75_KEY_A: &str = "t75.alpha";
const T75_KEY_B: &str = "t75.beta";
const T75_KEY_C: &str = "t75.gamma";
const T75_KEY_D: &str = "t75.delta";
const T75_KEY_E: &str = "t75.epsilon";

const T75_ID_A: NodeId = derive_node_id(T75_PLUGIN, T75_KEY_A);
const T75_ID_B: NodeId = derive_node_id(T75_PLUGIN, T75_KEY_B);
const T75_ID_C: NodeId = derive_node_id(T75_PLUGIN, T75_KEY_C);
const T75_ID_D: NodeId = derive_node_id(T75_PLUGIN, T75_KEY_D);
const T75_ID_E: NodeId = derive_node_id(T75_PLUGIN, T75_KEY_E);

// L4=162 namespace for this harness.
const T75_VEC_A: VectorAddress = VectorAddress::new(162, 1, 1, 0);
const T75_VEC_B: VectorAddress = VectorAddress::new(162, 1, 2, 0);
const T75_VEC_C: VectorAddress = VectorAddress::new(162, 1, 3, 0);
const T75_VEC_D: VectorAddress = VectorAddress::new(162, 2, 1, 0);
const T75_VEC_E: VectorAddress = VectorAddress::new(162, 2, 2, 0);

const T75_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T75_PLUGIN,
    name:         "kl-graph-topo75-harness",
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
        executor_id:       T75_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T75_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T75_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nnonatetraactc, nhnonatetraactc, narso, ec, nc) = gos_runtime::graph_topo_indices75();
    assert_eq!(nc,                0, "empty: node_count=0");
    assert_eq!(ec,                0, "empty: edge_count=0");
    assert_eq!(nnonatetraactc,    0, "empty: NNONATETRAACTC=0");
    assert_eq!(nhnonatetraactc,   0, "empty: NHNONATETRAACTC=0");
    assert_eq!(narso,             0, "empty: NARSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T75_VEC_A, T75_KEY_A, T75_ID_A);

    let (nnonatetraactc, nhnonatetraactc, narso, ec, nc) = gos_runtime::graph_topo_indices75();
    assert_eq!(nc,                1, "single: node_count=1");
    assert_eq!(ec,                0, "single: edge_count=0");
    assert_eq!(nnonatetraactc,    0, "single: NNONATETRAACTC=0");
    assert_eq!(nhnonatetraactc,   0, "single: NHNONATETRAACTC=0");
    assert_eq!(narso,             0, "single: NARSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NNONATETRAACTC:  1^49+1^49 = 2.
// NHNONATETRAACTC: (1+1)^48 = 2^48 = 281_474_976_710_656.
// NARSO:           (1²+1²)^43 = 2^43 = 8_796_093_022_208.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T75_VEC_A, T75_KEY_A, T75_ID_A);
    add_node(T75_VEC_B, T75_KEY_B, T75_ID_B);
    add_edge(T75_ID_A, T75_ID_B, "t75.e.ab");

    let (nnonatetraactc, nhnonatetraactc, narso, ec, nc) = gos_runtime::graph_topo_indices75();
    assert_eq!(nc,                2,                      "k2: node_count=2");
    assert_eq!(ec,                1,                      "k2: edge_count=1");
    assert_eq!(nnonatetraactc,    2,                      "k2: NNONATETRAACTC=2 (1\u{2074}\u{2079}+1\u{2074}\u{2079}=2)");
    assert_eq!(nhnonatetraactc,   281_474_976_710_656,    "k2: NHNONATETRAACTC=281_474_976_710_656 (2\u{2074}\u{2078}=2^48)");
    assert_eq!(narso,             8_796_093_022_208,      "k2: NARSO=8_796_093_022_208 (2\u{2074}\u{00b3}=2^43)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NNONATETRAACTC:  3×2^49 = 3×562_949_953_421_312 = 1_688_849_860_263_936.
// NHNONATETRAACTC: 2×(2+2)^48 = 2×4^48 = 2×2^96 → SATURATES.
// NARSO:           2×(4+4)^43 = 2×8^43 = 2×2^129 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T75_VEC_A, T75_KEY_A, T75_ID_A);
    add_node(T75_VEC_B, T75_KEY_B, T75_ID_B);
    add_node(T75_VEC_C, T75_KEY_C, T75_ID_C);
    add_edge(T75_ID_A, T75_ID_B, "t75.e.ab");
    add_edge(T75_ID_B, T75_ID_C, "t75.e.bc");

    let (nnonatetraactc, nhnonatetraactc, narso, ec, nc) = gos_runtime::graph_topo_indices75();
    assert_eq!(nc,                3,                        "p3: node_count=3");
    assert_eq!(ec,                2,                        "p3: edge_count=2");
    assert_eq!(nnonatetraactc,    1_688_849_860_263_936,    "p3: NNONATETRAACTC=1_688_849_860_263_936 (3\u{00d7}2\u{2074}\u{2079})");
    assert_eq!(nhnonatetraactc,   u64::MAX,                 "p3: NHNONATETRAACTC=SAT (4\u{2074}\u{2078}>u64)");
    assert_eq!(narso,             u64::MAX,                 "p3: NARSO=SAT (8\u{2074}\u{00b3}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T75_VEC_A, T75_KEY_A, T75_ID_A);
    add_node(T75_VEC_B, T75_KEY_B, T75_ID_B);
    add_node(T75_VEC_C, T75_KEY_C, T75_ID_C);
    add_edge(T75_ID_A, T75_ID_B, "t75.e.ab");
    add_edge(T75_ID_B, T75_ID_C, "t75.e.bc");
    add_edge(T75_ID_C, T75_ID_A, "t75.e.ca");

    let (nnonatetraactc, nhnonatetraactc, narso, ec, nc) = gos_runtime::graph_topo_indices75();
    assert_eq!(nc,                3,        "k3: node_count=3");
    assert_eq!(ec,                3,        "k3: edge_count=3");
    assert_eq!(nnonatetraactc,    u64::MAX, "k3: NNONATETRAACTC=SAT");
    assert_eq!(nhnonatetraactc,   u64::MAX, "k3: NHNONATETRAACTC=SAT");
    assert_eq!(narso,             u64::MAX, "k3: NARSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T75_VEC_A, T75_KEY_A, T75_ID_A); // hub
    add_node(T75_VEC_B, T75_KEY_B, T75_ID_B);
    add_node(T75_VEC_C, T75_KEY_C, T75_ID_C);
    add_node(T75_VEC_D, T75_KEY_D, T75_ID_D);
    add_node(T75_VEC_E, T75_KEY_E, T75_ID_E);
    add_edge(T75_ID_A, T75_ID_B, "t75.e.ab");
    add_edge(T75_ID_A, T75_ID_C, "t75.e.ac");
    add_edge(T75_ID_A, T75_ID_D, "t75.e.ad");
    add_edge(T75_ID_A, T75_ID_E, "t75.e.ae");

    let (nnonatetraactc, nhnonatetraactc, narso, ec, nc) = gos_runtime::graph_topo_indices75();
    assert_eq!(nc,                5,        "k14: node_count=5");
    assert_eq!(ec,                4,        "k14: edge_count=4");
    assert_eq!(nnonatetraactc,    u64::MAX, "k14: NNONATETRAACTC=SAT");
    assert_eq!(nhnonatetraactc,   u64::MAX, "k14: NHNONATETRAACTC=SAT");
    assert_eq!(narso,             u64::MAX, "k14: NARSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NNONATETRAACTC: 2×2^49 + 2×3^49.  3^49>u64::MAX → SATURATES.
// NHNONATETRAACTC: 5^48+6^48+5^48 → SATURATES.
// NARSO: 13^43+18^43+13^43 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T75_VEC_A, T75_KEY_A, T75_ID_A);
    add_node(T75_VEC_B, T75_KEY_B, T75_ID_B);
    add_node(T75_VEC_C, T75_KEY_C, T75_ID_C);
    add_node(T75_VEC_D, T75_KEY_D, T75_ID_D);
    add_edge(T75_ID_A, T75_ID_B, "t75.e.ab");
    add_edge(T75_ID_B, T75_ID_C, "t75.e.bc");
    add_edge(T75_ID_C, T75_ID_D, "t75.e.cd");

    let (nnonatetraactc, nhnonatetraactc, narso, ec, nc) = gos_runtime::graph_topo_indices75();
    assert_eq!(nc,                4,        "p4: node_count=4");
    assert_eq!(ec,                3,        "p4: edge_count=3");
    assert_eq!(nnonatetraactc,    u64::MAX, "p4: NNONATETRAACTC=SAT");
    assert_eq!(nhnonatetraactc,   u64::MAX, "p4: NHNONATETRAACTC=SAT");
    assert_eq!(narso,             u64::MAX, "p4: NARSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T75_VEC_A, T75_KEY_A, T75_ID_A);
    add_node(T75_VEC_B, T75_KEY_B, T75_ID_B);
    add_node(T75_VEC_C, T75_KEY_C, T75_ID_C);
    add_node(T75_VEC_D, T75_KEY_D, T75_ID_D);
    add_edge(T75_ID_A, T75_ID_B, "t75.e.ab");
    add_edge(T75_ID_A, T75_ID_C, "t75.e.ac");
    add_edge(T75_ID_A, T75_ID_D, "t75.e.ad");
    add_edge(T75_ID_B, T75_ID_C, "t75.e.bc");
    add_edge(T75_ID_B, T75_ID_D, "t75.e.bd");
    add_edge(T75_ID_C, T75_ID_D, "t75.e.cd");

    let (nnonatetraactc, nhnonatetraactc, narso, ec, nc) = gos_runtime::graph_topo_indices75();
    assert_eq!(nc,                4,        "k4: node_count=4");
    assert_eq!(ec,                6,        "k4: edge_count=6");
    assert_eq!(nnonatetraactc,    u64::MAX, "k4: NNONATETRAACTC=SAT");
    assert_eq!(nhnonatetraactc,   u64::MAX, "k4: NHNONATETRAACTC=SAT");
    assert_eq!(narso,             u64::MAX, "k4: NARSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T75_VEC_A, T75_KEY_A, T75_ID_A);
    add_node(T75_VEC_B, T75_KEY_B, T75_ID_B);

    let (nnonatetraactc, nhnonatetraactc, narso, ec, nc) = gos_runtime::graph_topo_indices75();
    assert_eq!(nc,                2, "isolated: node_count=2");
    assert_eq!(ec,                0, "isolated: edge_count=0");
    assert_eq!(nnonatetraactc,    0, "isolated: NNONATETRAACTC=0");
    assert_eq!(nhnonatetraactc,   0, "isolated: NHNONATETRAACTC=0");
    assert_eq!(narso,             0, "isolated: NARSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NNONATETRAACTC=5×6^49 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T75_VEC_A, T75_KEY_A, T75_ID_A);
    add_node(T75_VEC_B, T75_KEY_B, T75_ID_B);
    add_node(T75_VEC_C, T75_KEY_C, T75_ID_C);
    add_node(T75_VEC_D, T75_KEY_D, T75_ID_D);
    add_node(T75_VEC_E, T75_KEY_E, T75_ID_E);
    add_edge(T75_ID_A, T75_ID_C, "t75.e.ac");
    add_edge(T75_ID_A, T75_ID_D, "t75.e.ad");
    add_edge(T75_ID_A, T75_ID_E, "t75.e.ae");
    add_edge(T75_ID_B, T75_ID_C, "t75.e.bc");
    add_edge(T75_ID_B, T75_ID_D, "t75.e.bd");
    add_edge(T75_ID_B, T75_ID_E, "t75.e.be");

    let (nnonatetraactc, nhnonatetraactc, narso, ec, nc) = gos_runtime::graph_topo_indices75();
    assert_eq!(nc,                5,        "k23: node_count=5");
    assert_eq!(ec,                6,        "k23: edge_count=6");
    assert_eq!(nnonatetraactc,    u64::MAX, "k23: NNONATETRAACTC=SAT (5\u{00d7}6\u{2074}\u{2079})");
    assert_eq!(nhnonatetraactc,   u64::MAX, "k23: NHNONATETRAACTC=SAT");
    assert_eq!(narso,             u64::MAX, "k23: NARSO=SAT");
}
