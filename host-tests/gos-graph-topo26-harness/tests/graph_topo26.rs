// gos-graph-topo26-harness — V3.37 NPC + NRM₂ + NRSO (Neighborhood S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices26()`:
//   Returns (npc_ppm, nrm2, nrso_ppm, edge_count, node_count)
//   - npc_ppm  = NPC(G) × 10^6 = Σ_{uv∈E} floor(√(S_u·S_v)·10^6)   (floor ppm; S-R_{1/2})
//   - nrm2     = NRM₂(G) = Σ_{uv∈E} (S_u-1)·(S_v-1)                 (exact u64; S-RM₂)
//   - nrso_ppm = NRSO(G) × 10^6 = Σ_{uv∈E} floor(10^6/√(S_u²+S_v²)) (floor ppm; S-RSO)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NPC(G)  = Σ_{uv∈E} √(S_u·S_v)            (S-analogue of R_{1/2}; Bollobás & Erdős 1998)
//   NRM₂(G) = Σ_{uv∈E} (S_u-1)·(S_v-1)       (S-analogue of RM₂; Furtula, Gutman & Ediz 2014)
//   NRSO(G) = Σ_{uv∈E} 1/√(S_u²+S_v²)        (S-analogue of RSO; Gutman 2022)
//
// IMPLEMENTATION FORMULAS (no float, no_std safe):
//   NPC  per edge  = isqrt128(S_u·S_v·10^12)                 [floor ppm]
//   NRM₂ per edge  = (S_u-1)·(S_v-1)                         [exact u64; 0 if S=1]
//   NRSO per edge  = isqrt64(10^12 / (S_u²+S_v²))            [floor ppm]
//
// KEY INVARIANTS:
//   NPC = |E|·S·10^6 for S-regular (√(S·S)=S); equals |E|·10^6 only when S=1 (K₂ case).
//   NRM₂ = 0 iff all edges have S_u=1 or S_v=1 (K₂-type: both endpoints have S=1).
//   NRSO coincidence: K₂, P₃, K_{2,3} all give 707_106 (floor rounding from different denominators).
//   K₃ and K_{1,4} coincide on all three indices (S-uniform S=4 coincidence, same 4 edges each).
//
// S VALUES PER GRAPH (same S-variant as topo18/topo21–topo26):
//   K₂        : S(A)=S(B)=1
//   P₃=A-B-C  : S(A)=S(B)=S(C)=2        → S-uniform S=2
//   K₃        : S(each)=4               → S-uniform S=4
//   K_{1,4}   : S(hub)=4, S(leaf)=4     → S-uniform S=4
//   P₄=A-B-C-D: S(A)=S(D)=2, S(B)=S(C)=3 → mixed S values
//   K₄        : S(each)=9               → S-uniform S=9
//   K_{2,3}   : S(all)=6                → S-uniform S=6
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph        NPC(ppm)   NRM₂    NRSO(ppm)  edges  nodes
//  Empty               0      0            0      0      0
//  1 node              0      0            0      0      1
//  Edge K₂     1_000_000      0      707_106      1      2
//  Path P₃     4_000_000      2      707_106      2      3
//  Triangle K₃ 12_000_000    27      530_328      3      3
//  Star K_{1,4}16_000_000    36      707_104      4      5
//  Path P₄     7_898_978      8      790_402      3      4
//  Complete K₄ 54_000_000   384      471_402      6      4
//  2 isolated          0      0            0      0      2
//  K_{2,3}     36_000_000   150      707_106      6      5
//
// Derivations:
//
//   K₂ (S_A=S_B=1):
//     NPC:  isqrt128(1·10^12)=1_000_000. ✓
//     NRM₂: (1-1)(1-1)=0. ✓
//     NRSO: isqrt64(10^12/(1+1))=isqrt64(500_000_000_000)=707_106. ✓
//
//   P₃ (S-uniform S=2, 2 edges):
//     NPC  per edge: isqrt128(4·10^12)=2_000_000. Total: 4_000_000. ✓
//       (S-regular: NPC=|E|·S·10^6=2·2·10^6=4_000_000)
//     NRM₂ per edge: (2-1)(2-1)=1. Total: 2. ✓
//     NRSO per edge: isqrt64(10^12/(4+4))=isqrt64(125_000_000_000)=353_553.
//       Total: 2·353_553=707_106. ✓
//       (coincidence: same total as K₂ despite 2 edges at S=2 vs 1 edge at S=1)
//
//   K₃ (S-uniform S=4, 3 edges):
//     NPC  per edge: isqrt128(16·10^12)=4_000_000. Total: 12_000_000. ✓
//     NRM₂ per edge: (4-1)(4-1)=9. Total: 27. ✓
//     NRSO per edge: isqrt64(10^12/(16+16))=isqrt64(31_250_000_000)=176_776.
//       Total: 3·176_776=530_328. ✓
//       (√(1/32)×10^6=176_776.69...; 176_776²=31_249_754_176 ≤ 31_250_000_000 < 176_777²)
//
//   K_{1,4} (S-uniform S=4, 4 edges — same per-edge as K₃):
//     NPC: 4·4_000_000=16_000_000. ✓
//     NRM₂: 4·9=36. ✓
//     NRSO: 4·176_776=707_104. ✓
//       (differs from K₂/P₃/K_{2,3}'s 707_106: 4 edges at 176_776 vs floor rounding at S=1/2/6)
//
//   P₄ (S_A=2, S_B=3, S_C=3, S_D=2):
//     Edge A-B (sa=2, sb=3):
//       NPC:  isqrt128(6·10^12)=2_449_489.
//             (√6·10^6≈2_449_489.74; 2_449_489²·10^0=5_999_997_021_121 ≤ 6·10^12 < 2_449_490²)
//       NRM₂: (2-1)(3-1)=2.
//       NRSO: isqrt64(10^12/13)=isqrt64(76_923_076_923)=277_350.
//             (277_350²=76_903_022_500 ≤ 76_923_076_923 < 277_351²=76_903_577_201 — wait)
//             Actually: 277_350²=76_902_922_500; 277_351²=76_903_477_201; still need to check:
//             277_350²: (277000+350)²=277000²+2×277000×350+350²
//             =76_729_000_000+193_900_000+122_500=76_923_022_500 ≤ 76_923_076_923 ✓
//             277_351²=76_923_022_500+2×277_350+1=76_923_022_500+554_701=76_923_577_201 > 76_923_076_923 ✓
//             So isqrt64(76_923_076_923)=277_350. ✓
//     Edge B-C (sa=3, sb=3):
//       NPC:  isqrt128(9·10^12)=3_000_000. (exact: √9=3)
//       NRM₂: (3-1)(3-1)=4.
//       NRSO: isqrt64(10^12/18)=isqrt64(55_555_555_555).
//             235_702²: (235000+702)²=235000²+2×235000×702+702²
//             =55_225_000_000+329_940_000+492_804=55_555_432_804 ≤ 55_555_555_555 ✓
//             235_703²=55_555_432_804+2×235_702+1=55_555_904_209 > 55_555_555_555 ✓
//             isqrt64(55_555_555_555)=235_702. ✓
//     Edge C-D: same as A-B.
//     NPC  total = 2·2_449_489+3_000_000=7_898_978. ✓
//     NRM₂ total = 2+4+2=8. ✓
//     NRSO total = 2·277_350+235_702=790_402. ✓
//
//   K₄ (S-uniform S=9, 6 edges):
//     NPC  per edge: isqrt128(81·10^12)=9_000_000 (exact). Total: 54_000_000. ✓
//     NRM₂ per edge: (9-1)(9-1)=64. Total: 384. ✓
//     NRSO per edge: isqrt64(10^12/(81+81))=isqrt64(10^12/162)=isqrt64(6_172_839_506).
//       78_567²=(78000+567)²=6_084_000_000+2×78000×567+567²
//             =6_084_000_000+88_452_000+321_489=6_172_773_489 ≤ 6_172_839_506 ✓
//       78_568²=6_172_773_489+2×78_567+1=6_172_930_624 > 6_172_839_506 ✓
//       isqrt64(6_172_839_506)=78_567. Total: 6·78_567=471_402. ✓
//
//   K_{2,3} (S-uniform S=6, 6 edges):
//     NPC  per edge: isqrt128(36·10^12)=6_000_000 (exact). Total: 36_000_000. ✓
//     NRM₂ per edge: (6-1)(6-1)=25. Total: 150. ✓
//     NRSO per edge: isqrt64(10^12/(36+36))=isqrt64(10^12/72)=isqrt64(13_888_888_888).
//       117_851²=(117000+851)²=13_689_000_000+2×117000×851+851²
//              =13_689_000_000+199_134_000+724_201=13_888_858_201 ≤ 13_888_888_888 ✓
//       117_852²=13_888_858_201+2×117_851+1=13_889_093_904 > 13_888_888_888 ✓
//       isqrt64(13_888_888_888)=117_851. Total: 6·117_851=707_106. ✓
//       (NRSO coincidence: same as K₂ and P₃ — all give 707_106 from different denominators)
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (1_000_000, 0, 707_106, 1, 2)
//  4.  Path P₃ = A-B-C                   → (4_000_000, 2, 707_106, 2, 3)
//  5.  Triangle K₃                       → (12_000_000, 27, 530_328, 3, 3)
//  6.  Star K_{1,4}                      → (16_000_000, 36, 707_104, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (7_898_978, 8, 790_402, 3, 4)
//  8.  Complete K₄                       → (54_000_000, 384, 471_402, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (36_000_000, 150, 707_106, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T26_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_26");
const T26_EXEC:   ExecutorId = ExecutorId::from_ascii("t26.exec");

const T26_KEY_A: &str = "t26.alpha";
const T26_KEY_B: &str = "t26.beta";
const T26_KEY_C: &str = "t26.gamma";
const T26_KEY_D: &str = "t26.delta";
const T26_KEY_E: &str = "t26.epsilon";

const T26_ID_A: NodeId = derive_node_id(T26_PLUGIN, T26_KEY_A);
const T26_ID_B: NodeId = derive_node_id(T26_PLUGIN, T26_KEY_B);
const T26_ID_C: NodeId = derive_node_id(T26_PLUGIN, T26_KEY_C);
const T26_ID_D: NodeId = derive_node_id(T26_PLUGIN, T26_KEY_D);
const T26_ID_E: NodeId = derive_node_id(T26_PLUGIN, T26_KEY_E);

// L4=113 namespace for this harness.
const T26_VEC_A: VectorAddress = VectorAddress::new(113, 1, 1, 0);
const T26_VEC_B: VectorAddress = VectorAddress::new(113, 1, 2, 0);
const T26_VEC_C: VectorAddress = VectorAddress::new(113, 1, 3, 0);
const T26_VEC_D: VectorAddress = VectorAddress::new(113, 2, 1, 0);
const T26_VEC_E: VectorAddress = VectorAddress::new(113, 2, 2, 0);

const T26_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T26_PLUGIN,
    name:         "kl-graph-topo26-harness",
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
        executor_id:       T26_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T26_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T26_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (npc, nrm2, nrso, ec, nc) = gos_runtime::graph_topo_indices26();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(npc,  0, "empty: NPC=0");
    assert_eq!(nrm2, 0, "empty: NRM\u{2082}=0");
    assert_eq!(nrso, 0, "empty: NRSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T26_VEC_A, T26_KEY_A, T26_ID_A);

    let (npc, nrm2, nrso, ec, nc) = gos_runtime::graph_topo_indices26();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(npc,  0, "single: NPC=0 (no edges)");
    assert_eq!(nrm2, 0, "single: NRM\u{2082}=0 (no edges)");
    assert_eq!(nrso, 0, "single: NRSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1.
// NPC:  isqrt128(1·10^12)=1_000_000.
// NRM₂: (1-1)(1-1)=0.
// NRSO: isqrt64(10^12/(1²+1²))=isqrt64(500_000_000_000)=707_106.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T26_VEC_A, T26_KEY_A, T26_ID_A);
    add_node(T26_VEC_B, T26_KEY_B, T26_ID_B);
    add_edge(T26_ID_A, T26_ID_B, "t26.e.ab");

    let (npc, nrm2, nrso, ec, nc) = gos_runtime::graph_topo_indices26();
    assert_eq!(nc,   2,         "k2: node_count=2");
    assert_eq!(ec,   1,         "k2: edge_count=1");
    assert_eq!(npc,  1_000_000, "k2: NPC=1_000_000 (isqrt128(1\u{00b7}10\u{00b9}\u{00b2})=10\u{2076}; S=1)");
    assert_eq!(nrm2, 0,         "k2: NRM\u{2082}=0 ((1-1)(1-1)=0; S=1 both endpoints)");
    assert_eq!(nrso, 707_106,   "k2: NRSO=707_106 (isqrt64(5\u{00d7}10\u{00b9}\u{00b9})=707_106; S\u{00b2}+S\u{00b2}=2)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. ssum=4, sp=4.
// NPC  per edge: isqrt128(4·10^12)=2_000_000. Total: 4_000_000.
// NRM₂ per edge: (2-1)(2-1)=1. Total: 2.
// NRSO per edge: isqrt64(10^12/8)=isqrt64(125_000_000_000)=353_553. Total: 707_106.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T26_VEC_A, T26_KEY_A, T26_ID_A);
    add_node(T26_VEC_B, T26_KEY_B, T26_ID_B);
    add_node(T26_VEC_C, T26_KEY_C, T26_ID_C);
    add_edge(T26_ID_A, T26_ID_B, "t26.e.ab");
    add_edge(T26_ID_B, T26_ID_C, "t26.e.bc");

    let (npc, nrm2, nrso, ec, nc) = gos_runtime::graph_topo_indices26();
    assert_eq!(nc,   3,         "p3: node_count=3");
    assert_eq!(ec,   2,         "p3: edge_count=2");
    assert_eq!(npc,  4_000_000, "p3: NPC=4_000_000 (2\u{00d7}2_000_000; S-regular S=2: NPC=|E|\u{00b7}S\u{00b7}10\u{2076})");
    assert_eq!(nrm2, 2,         "p3: NRM\u{2082}=2 (2\u{00d7}1; (2-1)(2-1)=1 per edge; S-uniform S=2)");
    assert_eq!(nrso, 707_106,   "p3: NRSO=707_106 (2\u{00d7}353_553; isqrt64(125_000_000_000)=353_553)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 edges.
// NPC  per edge: isqrt128(16·10^12)=4_000_000. Total: 12_000_000.
// NRM₂ per edge: (4-1)(4-1)=9. Total: 27.
// NRSO per edge: isqrt64(10^12/32)=isqrt64(31_250_000_000)=176_776. Total: 530_328.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T26_VEC_A, T26_KEY_A, T26_ID_A);
    add_node(T26_VEC_B, T26_KEY_B, T26_ID_B);
    add_node(T26_VEC_C, T26_KEY_C, T26_ID_C);
    add_edge(T26_ID_A, T26_ID_B, "t26.e.ab");
    add_edge(T26_ID_B, T26_ID_A, "t26.e.ba");
    add_edge(T26_ID_B, T26_ID_C, "t26.e.bc");
    add_edge(T26_ID_C, T26_ID_B, "t26.e.cb");
    add_edge(T26_ID_A, T26_ID_C, "t26.e.ac");
    add_edge(T26_ID_C, T26_ID_A, "t26.e.ca");

    let (npc, nrm2, nrso, ec, nc) = gos_runtime::graph_topo_indices26();
    assert_eq!(nc,   3,          "k3: node_count=3");
    assert_eq!(ec,   3,          "k3: edge_count=3");
    assert_eq!(npc,  12_000_000, "k3: NPC=12_000_000 (3\u{00d7}4_000_000; S-regular S=4: NPC=3\u{00d7}4\u{00d7}10\u{2076})");
    assert_eq!(nrm2, 27,         "k3: NRM\u{2082}=27 (3\u{00d7}9; (4-1)(4-1)=9 per edge; S-uniform S=4)");
    assert_eq!(nrso, 530_328,    "k3: NRSO=530_328 (3\u{00d7}176_776; isqrt64(31_250_000_000)=176_776)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4×1=4, S(leaf)=deg(hub)=4. S-uniform S=4. 4 edges.
// Same per-edge values as K₃ (S-uniform S=4 coincidence), but 4 edges.
// NPC: 4×4_000_000=16_000_000. NRM₂: 4×9=36. NRSO: 4×176_776=707_104.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T26_VEC_A, T26_KEY_A, T26_ID_A);
    add_node(T26_VEC_B, T26_KEY_B, T26_ID_B);
    add_node(T26_VEC_C, T26_KEY_C, T26_ID_C);
    add_node(T26_VEC_D, T26_KEY_D, T26_ID_D);
    add_node(T26_VEC_E, T26_KEY_E, T26_ID_E);
    add_edge(T26_ID_A, T26_ID_B, "t26.e.ab");
    add_edge(T26_ID_A, T26_ID_C, "t26.e.ac");
    add_edge(T26_ID_A, T26_ID_D, "t26.e.ad");
    add_edge(T26_ID_A, T26_ID_E, "t26.e.ae");

    let (npc, nrm2, nrso, ec, nc) = gos_runtime::graph_topo_indices26();
    assert_eq!(nc,   5,          "star: node_count=5");
    assert_eq!(ec,   4,          "star: edge_count=4");
    assert_eq!(npc,  16_000_000, "star: NPC=16_000_000 (4\u{00d7}4_000_000; S-uniform S=4 same as K\u{2083})");
    assert_eq!(nrm2, 36,         "star: NRM\u{2082}=36 (4\u{00d7}9; S-uniform S=4 coincidence with K\u{2083})");
    assert_eq!(nrso, 707_104,    "star: NRSO=707_104 (4\u{00d7}176_776; differs from K\u{2082}/P\u{2083}/K\u{2082}\u{2083} by 2 floor units)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2.
// S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=3, S(C)=deg(B)+deg(D)=3, S(D)=deg(C)=2.
// Edge A-B (sa=2,sb=3): NPC=2_449_489; NRM₂=2; NRSO=277_350.
// Edge B-C (sa=3,sb=3): NPC=3_000_000; NRM₂=4; NRSO=235_702.
// Edge C-D: same as A-B.
// NPC=7_898_978. NRM₂=8. NRSO=790_402.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T26_VEC_A, T26_KEY_A, T26_ID_A);
    add_node(T26_VEC_B, T26_KEY_B, T26_ID_B);
    add_node(T26_VEC_C, T26_KEY_C, T26_ID_C);
    add_node(T26_VEC_D, T26_KEY_D, T26_ID_D);
    add_edge(T26_ID_A, T26_ID_B, "t26.e.ab");
    add_edge(T26_ID_B, T26_ID_C, "t26.e.bc");
    add_edge(T26_ID_C, T26_ID_D, "t26.e.cd");

    let (npc, nrm2, nrso, ec, nc) = gos_runtime::graph_topo_indices26();
    assert_eq!(nc,   4,         "p4: node_count=4");
    assert_eq!(ec,   3,         "p4: edge_count=3");
    assert_eq!(npc,  7_898_978, "p4: NPC=7_898_978 (2\u{00d7}2_449_489+3_000_000; isqrt128(6\u{00b7}10\u{00b9}\u{00b2})=2_449_489)");
    assert_eq!(nrm2, 8,         "p4: NRM\u{2082}=8 (2+4+2; edges AB,CD: (2-1)(3-1)=2; BC: (3-1)(3-1)=4)");
    assert_eq!(nrso, 790_402,   "p4: NRSO=790_402 (2\u{00d7}277_350+235_702; mixed S=2,3)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=3×3=9. S-uniform S=9. 6 edges.
// NPC  per edge: isqrt128(81·10^12)=9_000_000 (exact). Total: 54_000_000.
// NRM₂ per edge: (9-1)(9-1)=64. Total: 384.
// NRSO per edge: isqrt64(10^12/162)=isqrt64(6_172_839_506)=78_567. Total: 471_402.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T26_VEC_A, T26_KEY_A, T26_ID_A);
    add_node(T26_VEC_B, T26_KEY_B, T26_ID_B);
    add_node(T26_VEC_C, T26_KEY_C, T26_ID_C);
    add_node(T26_VEC_D, T26_KEY_D, T26_ID_D);
    add_edge(T26_ID_A, T26_ID_B, "t26.e.ab");
    add_edge(T26_ID_B, T26_ID_A, "t26.e.ba");
    add_edge(T26_ID_A, T26_ID_C, "t26.e.ac");
    add_edge(T26_ID_C, T26_ID_A, "t26.e.ca");
    add_edge(T26_ID_A, T26_ID_D, "t26.e.ad");
    add_edge(T26_ID_D, T26_ID_A, "t26.e.da");
    add_edge(T26_ID_B, T26_ID_C, "t26.e.bc");
    add_edge(T26_ID_C, T26_ID_B, "t26.e.cb");
    add_edge(T26_ID_B, T26_ID_D, "t26.e.bd");
    add_edge(T26_ID_D, T26_ID_B, "t26.e.db");
    add_edge(T26_ID_C, T26_ID_D, "t26.e.cd");
    add_edge(T26_ID_D, T26_ID_C, "t26.e.dc");

    let (npc, nrm2, nrso, ec, nc) = gos_runtime::graph_topo_indices26();
    assert_eq!(nc,   4,          "k4: node_count=4");
    assert_eq!(ec,   6,          "k4: edge_count=6");
    assert_eq!(npc,  54_000_000, "k4: NPC=54_000_000 (6\u{00d7}9_000_000 exact; S-regular S=9: NPC=6\u{00d7}9\u{00d7}10\u{2076})");
    assert_eq!(nrm2, 384,        "k4: NRM\u{2082}=384 (6\u{00d7}64; (9-1)(9-1)=64 per edge; S-uniform S=9)");
    assert_eq!(nrso, 471_402,    "k4: NRSO=471_402 (6\u{00d7}78_567; isqrt64(6_172_839_506)=78_567; denom=162)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T26_VEC_A, T26_KEY_A, T26_ID_A);
    add_node(T26_VEC_B, T26_KEY_B, T26_ID_B);

    let (npc, nrm2, nrso, ec, nc) = gos_runtime::graph_topo_indices26();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(npc,  0, "isolated: NPC=0 (no edges)");
    assert_eq!(nrm2, 0, "isolated: NRM\u{2082}=0 (no edges)");
    assert_eq!(nrso, 0, "isolated: NRSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}:d=3. Right={C,D,E}:d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 6 edges.
// NPC  per edge: isqrt128(36·10^12)=6_000_000 (exact). Total: 36_000_000.
// NRM₂ per edge: (6-1)(6-1)=25. Total: 150.
// NRSO per edge: isqrt64(10^12/72)=isqrt64(13_888_888_888)=117_851. Total: 707_106.
//   (NRSO coincidence: same 707_106 as K₂ and P₃ — different denominators, same floor total)

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T26_VEC_A, T26_KEY_A, T26_ID_A);
    add_node(T26_VEC_B, T26_KEY_B, T26_ID_B);
    add_node(T26_VEC_C, T26_KEY_C, T26_ID_C);
    add_node(T26_VEC_D, T26_KEY_D, T26_ID_D);
    add_node(T26_VEC_E, T26_KEY_E, T26_ID_E);
    add_edge(T26_ID_A, T26_ID_C, "t26.e.ac");
    add_edge(T26_ID_C, T26_ID_A, "t26.e.ca");
    add_edge(T26_ID_A, T26_ID_D, "t26.e.ad");
    add_edge(T26_ID_D, T26_ID_A, "t26.e.da");
    add_edge(T26_ID_A, T26_ID_E, "t26.e.ae");
    add_edge(T26_ID_E, T26_ID_A, "t26.e.ea");
    add_edge(T26_ID_B, T26_ID_C, "t26.e.bc");
    add_edge(T26_ID_C, T26_ID_B, "t26.e.cb");
    add_edge(T26_ID_B, T26_ID_D, "t26.e.bd");
    add_edge(T26_ID_D, T26_ID_B, "t26.e.db");
    add_edge(T26_ID_B, T26_ID_E, "t26.e.be");
    add_edge(T26_ID_E, T26_ID_B, "t26.e.eb");

    let (npc, nrm2, nrso, ec, nc) = gos_runtime::graph_topo_indices26();
    assert_eq!(nc,   5,          "k23: node_count=5");
    assert_eq!(ec,   6,          "k23: edge_count=6");
    assert_eq!(npc,  36_000_000, "k23: NPC=36_000_000 (6\u{00d7}6_000_000 exact; S-regular S=6: NPC=6\u{00d7}6\u{00d7}10\u{2076})");
    assert_eq!(nrm2, 150,        "k23: NRM\u{2082}=150 (6\u{00d7}25; (6-1)(6-1)=25 per edge; S-uniform S=6)");
    assert_eq!(nrso, 707_106,    "k23: NRSO=707_106 (6\u{00d7}117_851; isqrt64(13_888_888_888)=117_851; NRSO coincidence with K\u{2082}/P\u{2083})");
}
