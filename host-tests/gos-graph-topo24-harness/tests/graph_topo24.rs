// gos-graph-topo24-harness — V3.35 NISI + NAZI + NEM1 (Neighborhood S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices24()`:
//   Returns (nisi_ppm, nazi_milli, nem1, edge_count, node_count)
//   - nisi_ppm   = NISI(G) × 10^6 = Σ_{uv∈E} floor(S_u·S_v·10^6/(S_u+S_v))   (floor ppm; S-ISI)
//   - nazi_milli = NAZI(G) × 10^3 = Σ_{uv∈E} floor((S_u·S_v)^3·10^3/(S_u+S_v-2)^3)
//                                    skip edges with S_u+S_v=2                  (floor milli; S-AZI)
//   - nem1        = NEM1(G) = Σ_{uv∈E} (S_u+S_v-2)^2                           (exact u64; S-EM₁)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NISI(G) = Σ_{uv∈E} S_u·S_v/(S_u+S_v)                  (S-analogue of ISI; Sedlar et al. 2011)
//   NAZI(G) = Σ_{uv∈E} (S_u·S_v/(S_u+S_v-2))^3            (S-analogue of AZI; Furtula et al. 2010)
//   NEM1(G) = Σ_{uv∈E} (S_u+S_v-2)^2                      (S-analogue of EM₁; Milićević et al. 2004)
//
// IMPLEMENTATION FORMULAS (no float, no_std safe):
//   NISI per edge = floor(S_u·S_v·10^6 / (S_u+S_v))       [integer division; S_u·S_v·10^6 ≤ 2.6×10^17 ✓]
//   NAZI per edge = floor((S_u·S_v)^3·10^3 / (S_u+S_v-2)^3)  [u128 intermediate; skip when S_u+S_v=2]
//   NEM1 per edge = (S_u+S_v-2)^2                          [exact; max ≈ 10^9 per edge ✓]
//
// KEY INVARIANTS:
//   NISI = |E|×S/2×10^6 for S-regular graphs (S_u=S_v=S everywhere).
//   NAZI = 0 when S_u+S_v=2 for all edges (only K₂: both endpoints S=1).
//   NEM1 = 0 iff (S_u+S_v=2) for every edge (only K₂ type pairs).
//   NAZI = |E| × (S/2)^3 × 10^3 for S-regular S (exact when no rounding): =|E|×(S·10^6/2)^3/10^15.
//
// S VALUES PER GRAPH (same S-variant as topo18/topo21/topo22/topo23):
//   K₂        : S(A)=S(B)=1               → ssum=2, sp=1
//   P₃=A-B-C  : S(A)=S(B)=S(C)=2         → S-uniform S=2
//   K₃        : S(each)=4                 → S-uniform S=4
//   K_{1,4}   : S(hub)=4, S(leaf)=4      → S-uniform S=4
//   P₄=A-B-C-D: S(A)=S(D)=2, S(B)=S(C)=3 → mixed S values
//   K₄        : S(each)=9                 → S-uniform S=9
//   K_{2,3}   : S(all)=6                  → S-uniform S=6
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph        NISI(ppm)    NAZI(milli)   NEM1   edges  nodes
//  Empty               0              0      0       0      0
//  1 node              0              0      0       0      1
//  Edge K₂       500_000              0      0       1      2
//  Path P₃     2_000_000         16_000      8       2      3
//  Triangle K₃ 6_000_000         56_886    108       3      3
//  Star K_{1,4} 8_000_000         75_848    144       4      5
//  Path P₄     3_900_000         27_390     34       3      4
//  Complete K₄ 27_000_000        778_476  1_536       6      4
//  2 isolated          0              0      0       0      2
//  K_{2,3}    18_000_000        279_936    600       6      5
//
// Derivations:
//
//   K₂ (S_A=S_B=1, ssum=2, sp=1, q=0):
//     NISI: floor(1×10^6/2) = 500_000. ✓
//     NAZI: q=0 → skip → 0. ✓
//     NEM1: (1+1-2)^2 = 0. ✓
//
//   P₃ (S-uniform S=2, 2 edges; ssum=4, sp=4, q=2):
//     NISI per edge: floor(4×10^6/4) = 1_000_000. Total: 2_000_000. ✓
//     NAZI per edge: (4)^3×10^3/(2)^3 = 64000/8 = 8000. Total: 16_000. ✓
//     NEM1 per edge: (2+2-2)^2 = 4. Total: 8. ✓
//
//   K₃ (S-uniform S=4, 3 edges; ssum=8, sp=16, q=6):
//     NISI per edge: floor(16×10^6/8) = 2_000_000. Total: 6_000_000. ✓
//     NAZI per edge: floor((16)^3×10^3/(6)^3) = floor(4096000/216) = 18962. Total: 56_886. ✓
//       (4096000 / 216 = 18962.96..., floor = 18962; 3×18962 = 56_886)
//     NEM1 per edge: (4+4-2)^2 = 36. Total: 108. ✓
//
//   K_{1,4} (S-uniform S=4, 4 edges; same per-edge values as K₃):
//     NISI: 4×2_000_000 = 8_000_000. ✓
//     NAZI: 4×18962 = 75_848. ✓
//     NEM1: 4×36 = 144. ✓
//     (S-uniform coincidence: K₃ and K_{1,4} share S=4, same per-edge; differ only in |E|)
//
//   P₄ (S_A=2, S_B=3, S_C=3, S_D=2):
//     Edge A-B (sa=2,sb=3, ssum=5, sp=6, q=3):
//       NISI = floor(6×10^6/5) = 1_200_000.
//       NAZI = floor(6^3×10^3/3^3) = floor(216000/27) = 8000. (exact)
//       NEM1 = (2+3-2)^2 = 9.
//     Edge B-C (sa=3,sb=3, ssum=6, sp=9, q=4):
//       NISI = floor(9×10^6/6) = 1_500_000.
//       NAZI = floor(9^3×10^3/4^3) = floor(729000/64) = 11390. (floor 11390.625)
//       NEM1 = (3+3-2)^2 = 16.
//     Edge C-D: same as A-B by symmetry.
//     NISI total = 1_200_000+1_500_000+1_200_000 = 3_900_000. ✓
//     NAZI total = 8000+11390+8000 = 27_390. ✓
//     NEM1 total = 9+16+9 = 34. ✓
//
//   K₄ (S-uniform S=9, 6 edges; ssum=18, sp=81, q=16):
//     NISI per edge: floor(81×10^6/18) = 4_500_000. Total: 27_000_000. ✓
//     NAZI per edge: floor((81)^3×10^3/(16)^3) = floor(531441000/4096) = 129746. Total: 778_476. ✓
//       (531441000 / 4096 = 129746.337..., floor = 129746; 6×129746 = 778_476)
//     NEM1 per edge: (9+9-2)^2 = 256. Total: 1_536. ✓
//
//   K_{2,3} (S-uniform S=6, 6 edges; ssum=12, sp=36, q=10):
//     NISI per edge: floor(36×10^6/12) = 3_000_000. Total: 18_000_000. ✓
//     NAZI per edge: floor((36)^3×10^3/(10)^3) = floor(46656000/1000) = 46656. Total: 279_936. ✓
//     NEM1 per edge: (6+6-2)^2 = 100. Total: 600. ✓
//     (Note: K₄ and K_{2,3} are DIFFERENT here since S=9 vs S=6)
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (500_000, 0, 0, 1, 2)
//  4.  Path P₃ = A-B-C                   → (2_000_000, 16_000, 8, 2, 3)
//  5.  Triangle K₃                       → (6_000_000, 56_886, 108, 3, 3)
//  6.  Star K_{1,4}                      → (8_000_000, 75_848, 144, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (3_900_000, 27_390, 34, 3, 4)
//  8.  Complete K₄                       → (27_000_000, 778_476, 1_536, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (18_000_000, 279_936, 600, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T24_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_24");
const T24_EXEC:   ExecutorId = ExecutorId::from_ascii("t24.exec");

const T24_KEY_A: &str = "t24.alpha";
const T24_KEY_B: &str = "t24.beta";
const T24_KEY_C: &str = "t24.gamma";
const T24_KEY_D: &str = "t24.delta";
const T24_KEY_E: &str = "t24.epsilon";

const T24_ID_A: NodeId = derive_node_id(T24_PLUGIN, T24_KEY_A);
const T24_ID_B: NodeId = derive_node_id(T24_PLUGIN, T24_KEY_B);
const T24_ID_C: NodeId = derive_node_id(T24_PLUGIN, T24_KEY_C);
const T24_ID_D: NodeId = derive_node_id(T24_PLUGIN, T24_KEY_D);
const T24_ID_E: NodeId = derive_node_id(T24_PLUGIN, T24_KEY_E);

// L4=111 namespace for this harness.
const T24_VEC_A: VectorAddress = VectorAddress::new(111, 1, 1, 0);
const T24_VEC_B: VectorAddress = VectorAddress::new(111, 1, 2, 0);
const T24_VEC_C: VectorAddress = VectorAddress::new(111, 1, 3, 0);
const T24_VEC_D: VectorAddress = VectorAddress::new(111, 2, 1, 0);
const T24_VEC_E: VectorAddress = VectorAddress::new(111, 2, 2, 0);

const T24_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T24_PLUGIN,
    name:         "kl-graph-topo24-harness",
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
        executor_id:       T24_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T24_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T24_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nisi, nazi, nem1, ec, nc) = gos_runtime::graph_topo_indices24();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(nisi, 0, "empty: NISI=0");
    assert_eq!(nazi, 0, "empty: NAZI=0");
    assert_eq!(nem1, 0, "empty: NEM1=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T24_VEC_A, T24_KEY_A, T24_ID_A);

    let (nisi, nazi, nem1, ec, nc) = gos_runtime::graph_topo_indices24();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(nisi, 0, "single: NISI=0 (no edges)");
    assert_eq!(nazi, 0, "single: NAZI=0 (no edges)");
    assert_eq!(nem1, 0, "single: NEM1=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. ssum=2, sp=1, q=0.
// NISI: floor(1×10^6/2) = 500_000.
// NAZI: q=0 → skip → 0.
// NEM1: (1+1-2)^2 = 0.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T24_VEC_A, T24_KEY_A, T24_ID_A);
    add_node(T24_VEC_B, T24_KEY_B, T24_ID_B);
    add_edge(T24_ID_A, T24_ID_B, "t24.e.ab");

    let (nisi, nazi, nem1, ec, nc) = gos_runtime::graph_topo_indices24();
    assert_eq!(nc,   2,       "k2: node_count=2");
    assert_eq!(ec,   1,       "k2: edge_count=1");
    assert_eq!(nisi, 500_000, "k2: NISI=500_000 (floor(1\u{00d7}10\u{2076}/2); S=1)");
    assert_eq!(nazi, 0,       "k2: NAZI=0 (q=S_u+S_v-2=0; skip pendant-pair)");
    assert_eq!(nem1, 0,       "k2: NEM1=0 ((1+1-2)\u{00b2}=0)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. ssum=4, sp=4, q=2.
// NISI per edge: floor(4×10^6/4) = 1_000_000. Total: 2_000_000.
// NAZI per edge: (4)^3×10^3/(2)^3 = 64000/8 = 8000. Total: 16_000.
// NEM1 per edge: (2+2-2)^2 = 4. Total: 8.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T24_VEC_A, T24_KEY_A, T24_ID_A);
    add_node(T24_VEC_B, T24_KEY_B, T24_ID_B);
    add_node(T24_VEC_C, T24_KEY_C, T24_ID_C);
    add_edge(T24_ID_A, T24_ID_B, "t24.e.ab");
    add_edge(T24_ID_B, T24_ID_C, "t24.e.bc");

    let (nisi, nazi, nem1, ec, nc) = gos_runtime::graph_topo_indices24();
    assert_eq!(nc,   3,         "p3: node_count=3");
    assert_eq!(ec,   2,         "p3: edge_count=2");
    assert_eq!(nisi, 2_000_000, "p3: NISI=2_000_000 (2\u{00d7}1_000_000; S-uniform S=2)");
    assert_eq!(nazi, 16_000,    "p3: NAZI=16_000 (2\u{00d7}8000; (4\u{00b3}\u{00d7}1000/8)=8000)");
    assert_eq!(nem1, 8,         "p3: NEM1=8 (2\u{00d7}4; (2+2-2)\u{00b2}=4)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. ssum=8, sp=16, q=6.
// NISI per edge: floor(16×10^6/8) = 2_000_000. Total: 6_000_000.
// NAZI per edge: floor(16^3×10^3/6^3) = floor(4096000/216) = 18962. Total: 56_886.
// NEM1 per edge: (4+4-2)^2 = 36. Total: 108.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T24_VEC_A, T24_KEY_A, T24_ID_A);
    add_node(T24_VEC_B, T24_KEY_B, T24_ID_B);
    add_node(T24_VEC_C, T24_KEY_C, T24_ID_C);
    add_edge(T24_ID_A, T24_ID_B, "t24.e.ab");
    add_edge(T24_ID_B, T24_ID_A, "t24.e.ba");
    add_edge(T24_ID_B, T24_ID_C, "t24.e.bc");
    add_edge(T24_ID_C, T24_ID_B, "t24.e.cb");
    add_edge(T24_ID_A, T24_ID_C, "t24.e.ac");
    add_edge(T24_ID_C, T24_ID_A, "t24.e.ca");

    let (nisi, nazi, nem1, ec, nc) = gos_runtime::graph_topo_indices24();
    assert_eq!(nc,   3,         "k3: node_count=3");
    assert_eq!(ec,   3,         "k3: edge_count=3");
    assert_eq!(nisi, 6_000_000, "k3: NISI=6_000_000 (3\u{00d7}2_000_000; S-uniform S=4)");
    assert_eq!(nazi, 56_886,    "k3: NAZI=56_886 (3\u{00d7}18962; floor(4096000/216)=18962)");
    assert_eq!(nem1, 108,       "k3: NEM1=108 (3\u{00d7}36; (4+4-2)\u{00b2}=36)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4×1=4, S(leaf)=deg(hub)=4. S-uniform S=4.
// Same per-edge values as K₃ (S-uniform S=4 coincidence), but 4 edges.
// NISI: 4×2_000_000 = 8_000_000.
// NAZI: 4×18962 = 75_848.
// NEM1: 4×36 = 144.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T24_VEC_A, T24_KEY_A, T24_ID_A);
    add_node(T24_VEC_B, T24_KEY_B, T24_ID_B);
    add_node(T24_VEC_C, T24_KEY_C, T24_ID_C);
    add_node(T24_VEC_D, T24_KEY_D, T24_ID_D);
    add_node(T24_VEC_E, T24_KEY_E, T24_ID_E);
    add_edge(T24_ID_A, T24_ID_B, "t24.e.ab");
    add_edge(T24_ID_A, T24_ID_C, "t24.e.ac");
    add_edge(T24_ID_A, T24_ID_D, "t24.e.ad");
    add_edge(T24_ID_A, T24_ID_E, "t24.e.ae");

    let (nisi, nazi, nem1, ec, nc) = gos_runtime::graph_topo_indices24();
    assert_eq!(nc,   5,         "star: node_count=5");
    assert_eq!(ec,   4,         "star: edge_count=4");
    assert_eq!(nisi, 8_000_000, "star: NISI=8_000_000 (4\u{00d7}2_000_000; S-uniform S=4, same as K\u{2083})");
    assert_eq!(nazi, 75_848,    "star: NAZI=75_848 (4\u{00d7}18962; S-uniform S=4 coincidence with K\u{2083})");
    assert_eq!(nem1, 144,       "star: NEM1=144 (4\u{00d7}36; S-uniform S=4)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2.
// S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=3, S(C)=deg(B)+deg(D)=3, S(D)=deg(C)=2.
// Edge A-B (sa=2,sb=3, q=3): NISI=1_200_000; NAZI=8000 (exact); NEM1=9.
// Edge B-C (sa=3,sb=3, q=4): NISI=1_500_000; NAZI=11390 (floor 11390.625); NEM1=16.
// Edge C-D (sa=3,sb=2, q=3): same as A-B.
// NISI=3_900_000. NAZI=27_390. NEM1=34.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T24_VEC_A, T24_KEY_A, T24_ID_A);
    add_node(T24_VEC_B, T24_KEY_B, T24_ID_B);
    add_node(T24_VEC_C, T24_KEY_C, T24_ID_C);
    add_node(T24_VEC_D, T24_KEY_D, T24_ID_D);
    add_edge(T24_ID_A, T24_ID_B, "t24.e.ab");
    add_edge(T24_ID_B, T24_ID_C, "t24.e.bc");
    add_edge(T24_ID_C, T24_ID_D, "t24.e.cd");

    let (nisi, nazi, nem1, ec, nc) = gos_runtime::graph_topo_indices24();
    assert_eq!(nc,   4,         "p4: node_count=4");
    assert_eq!(ec,   3,         "p4: edge_count=3");
    assert_eq!(nisi, 3_900_000, "p4: NISI=3_900_000 (1_200_000+1_500_000+1_200_000)");
    assert_eq!(nazi, 27_390,    "p4: NAZI=27_390 (8000+11390+8000; floor(729000/64)=11390)");
    assert_eq!(nem1, 34,        "p4: NEM1=34 (9+16+9; mixed S=2,3)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=3×3=9. S-uniform S=9. ssum=18, sp=81, q=16.
// NISI per edge: floor(81×10^6/18) = 4_500_000. Total: 27_000_000.
// NAZI per edge: floor(81^3×10^3/16^3) = floor(531441000/4096) = 129_746. Total: 778_476.
// NEM1 per edge: (9+9-2)^2 = 256. Total: 1_536.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T24_VEC_A, T24_KEY_A, T24_ID_A);
    add_node(T24_VEC_B, T24_KEY_B, T24_ID_B);
    add_node(T24_VEC_C, T24_KEY_C, T24_ID_C);
    add_node(T24_VEC_D, T24_KEY_D, T24_ID_D);
    add_edge(T24_ID_A, T24_ID_B, "t24.e.ab");
    add_edge(T24_ID_B, T24_ID_A, "t24.e.ba");
    add_edge(T24_ID_A, T24_ID_C, "t24.e.ac");
    add_edge(T24_ID_C, T24_ID_A, "t24.e.ca");
    add_edge(T24_ID_A, T24_ID_D, "t24.e.ad");
    add_edge(T24_ID_D, T24_ID_A, "t24.e.da");
    add_edge(T24_ID_B, T24_ID_C, "t24.e.bc");
    add_edge(T24_ID_C, T24_ID_B, "t24.e.cb");
    add_edge(T24_ID_B, T24_ID_D, "t24.e.bd");
    add_edge(T24_ID_D, T24_ID_B, "t24.e.db");
    add_edge(T24_ID_C, T24_ID_D, "t24.e.cd");
    add_edge(T24_ID_D, T24_ID_C, "t24.e.dc");

    let (nisi, nazi, nem1, ec, nc) = gos_runtime::graph_topo_indices24();
    assert_eq!(nc,   4,          "k4: node_count=4");
    assert_eq!(ec,   6,          "k4: edge_count=6");
    assert_eq!(nisi, 27_000_000, "k4: NISI=27_000_000 (6\u{00d7}4_500_000; S-uniform S=9)");
    assert_eq!(nazi, 778_476,    "k4: NAZI=778_476 (6\u{00d7}129746; floor(531441000/4096)=129746)");
    assert_eq!(nem1, 1_536,      "k4: NEM1=1_536 (6\u{00d7}256; (9+9-2)\u{00b2}=256)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T24_VEC_A, T24_KEY_A, T24_ID_A);
    add_node(T24_VEC_B, T24_KEY_B, T24_ID_B);

    let (nisi, nazi, nem1, ec, nc) = gos_runtime::graph_topo_indices24();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(nisi, 0, "isolated: NISI=0 (no edges)");
    assert_eq!(nazi, 0, "isolated: NAZI=0 (no edges)");
    assert_eq!(nem1, 0, "isolated: NEM1=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}:d=3. Right={C,D,E}:d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. ssum=12, sp=36, q=10.
// NISI per edge: floor(36×10^6/12) = 3_000_000. Total: 18_000_000.
// NAZI per edge: floor(36^3×10^3/10^3) = floor(46656000/1000) = 46_656 (exact). Total: 279_936.
// NEM1 per edge: (6+6-2)^2 = 100. Total: 600.
// (Note: K₄ and K_{2,3} give DIFFERENT values: S=9 vs S=6)

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T24_VEC_A, T24_KEY_A, T24_ID_A);
    add_node(T24_VEC_B, T24_KEY_B, T24_ID_B);
    add_node(T24_VEC_C, T24_KEY_C, T24_ID_C);
    add_node(T24_VEC_D, T24_KEY_D, T24_ID_D);
    add_node(T24_VEC_E, T24_KEY_E, T24_ID_E);
    add_edge(T24_ID_A, T24_ID_C, "t24.e.ac");
    add_edge(T24_ID_C, T24_ID_A, "t24.e.ca");
    add_edge(T24_ID_A, T24_ID_D, "t24.e.ad");
    add_edge(T24_ID_D, T24_ID_A, "t24.e.da");
    add_edge(T24_ID_A, T24_ID_E, "t24.e.ae");
    add_edge(T24_ID_E, T24_ID_A, "t24.e.ea");
    add_edge(T24_ID_B, T24_ID_C, "t24.e.bc");
    add_edge(T24_ID_C, T24_ID_B, "t24.e.cb");
    add_edge(T24_ID_B, T24_ID_D, "t24.e.bd");
    add_edge(T24_ID_D, T24_ID_B, "t24.e.db");
    add_edge(T24_ID_B, T24_ID_E, "t24.e.be");
    add_edge(T24_ID_E, T24_ID_B, "t24.e.eb");

    let (nisi, nazi, nem1, ec, nc) = gos_runtime::graph_topo_indices24();
    assert_eq!(nc,   5,          "k23: node_count=5");
    assert_eq!(ec,   6,          "k23: edge_count=6");
    assert_eq!(nisi, 18_000_000, "k23: NISI=18_000_000 (6\u{00d7}3_000_000; S-uniform S=6; \u{2260}K\u{2084})");
    assert_eq!(nazi, 279_936,    "k23: NAZI=279_936 (6\u{00d7}46656; 36\u{00b3}\u{00d7}1000/10\u{00b3}=46656 exact)");
    assert_eq!(nem1, 600,        "k23: NEM1=600 (6\u{00d7}100; (6+6-2)\u{00b2}=100)");
}
