// gos-graph-topo27-harness — V3.38 NRR + NSO* + NrSO (Neighborhood S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices27()`:
//   Returns (nrr_ppm, nsos_ppm, nrso_ppm, edge_count, node_count)
//   - nrr_ppm  = NRR(G) × 10^6  = Σ_{uv∈E} floor(10^6/(S_u·S_v))                     (floor ppm; S-R_{-1})
//   - nsos_ppm = NSO*(G) × 10^6 = Σ_{uv∈E} isqrt128(S_u²·S_v²·10^12/(S_u²+S_v²))    (floor ppm; S-SO*)
//   - nrso_ppm = NrSO(G) × 10^6 = Σ_{uv∈E} isqrt128(((S_u-1)²+(S_v-1)²)·10^12)     (floor ppm; S-rSO)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NRR(G)  = Σ_{uv∈E} 1/(S_u·S_v)              (S-analogue of R_{-1}; Bollobás & Erdős 1998)
//   NSO*(G) = Σ_{uv∈E} S_u·S_v/√(S_u²+S_v²)    (S-analogue of SO*; Ghanbari & Rajabi-Parsa 2021)
//   NrSO(G) = Σ_{uv∈E} √((S_u-1)²+(S_v-1)²)    (S-analogue of rSO; Doslic et al. 2022)
//
// IMPLEMENTATION FORMULAS (no float, no_std safe):
//   NRR  per edge  = floor(10^6 / (S_u·S_v))
//   NSO* per edge  = isqrt128(S_u²·S_v²·10^12 / (S_u²+S_v²))           [u128 intermediate]
//   NrSO per edge  = isqrt128(((S_u-1)²+(S_v-1)²)·10^12)               [u128 intermediate; result fits u64]
//
// KEY INVARIANTS:
//   NRR  = |E|×10^6 only when all S=1 (K₂ case); NRR=|E|×floor(10^6/S²) for S-regular.
//   NSO* = NrSO for S=2 uniform (P₃ edges: both give √2·10^6=1_414_213 per edge).
//   NrSO = 0 iff all S=1 (K₂-type: both endpoints S=1, (0²+0²)=0).
//   K₃ and K_{1,4} coincide on all three indices (S-uniform S=4 coincidence).
//   K_{2,3} NSO* coincides with K₃ NrSO (both compute isqrt128(18·10^12)=4_242_640 per edge).
//
// S VALUES PER GRAPH:
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
//  Graph        NRR(ppm)   NSO*(ppm)   NrSO(ppm)  edges  nodes
//  Empty               0           0           0      0      0
//  1 node              0           0           0      0      1
//  Edge K₂     1_000_000     707_106           0      1      2
//  Path P₃       500_000   2_828_426   2_828_426      2      3
//  Triangle K₃   187_500   8_485_281  12_727_920      3      3
//  Star K_{1,4}  250_000  11_313_708  16_970_560      4      5
//  Path P₄       444_443   5_449_520   7_300_561      3      4
//  Complete K₄    74_070  38_183_766  67_882_248      6      4
//  2 isolated          0           0           0      0      2
//  K_{2,3}       166_662  25_455_840  42_426_402      6      5
//
// DERIVATIONS:
//
//   K₂ (S_A=S_B=1):
//     NRR:  floor(10^6/1)=1_000_000. ✓
//     NSO*: isqrt128(1·1·10^12/(1+1))=isqrt128(5×10^11)=707_106. ✓
//           (√(1/2)×10^6=707_106.78...; 707_106²=499_998_971_236 ≤ 5×10^11 < 707_107²)
//     NrSO: isqrt128((0²+0²)·10^12)=isqrt128(0)=0. ✓
//
//   P₃ (S-uniform S=2, 2 edges):
//     NRR  per edge: floor(10^6/4)=250_000. Total: 500_000. ✓
//     NSO* per edge: isqrt128(4·4·10^12/(4+4))=isqrt128(2×10^12)=1_414_213.
//       Total: 2_828_426. ✓
//       (NSO*=NrSO per edge at S=2: S·S/√(S²+S²)=S/√2=√2=√((S-1)²+(S-1)²)=√2)
//     NrSO per edge: isqrt128((1²+1²)·10^12)=isqrt128(2×10^12)=1_414_213.
//       Total: 2_828_426. ✓
//
//   K₃ (S-uniform S=4, 3 edges):
//     NRR  per edge: floor(10^6/16)=62_500. Total: 187_500. ✓
//     NSO* per edge: isqrt128(16·16·10^12/(16+16))=isqrt128(8×10^12)=2_828_427.
//       Total: 8_485_281. ✓
//       (2_828_427²=7_999_999_294_329 ≤ 8×10^12 < 2_828_428²=8_000_004_951_184)
//     NrSO per edge: isqrt128((3²+3²)·10^12)=isqrt128(18×10^12)=4_242_640.
//       Total: 12_727_920. ✓
//       (4_242_640²=17_999_994_169_600 ≤ 18×10^12 < 4_242_641²=18_000_002_654_881)
//
//   K_{1,4} (S-uniform S=4, 4 edges — same per-edge as K₃):
//     NRR:  4×62_500=250_000. ✓
//     NSO*: 4×2_828_427=11_313_708. ✓
//     NrSO: 4×4_242_640=16_970_560. ✓
//
//   P₄ (S_A=2, S_B=3, S_C=3, S_D=2):
//     Edge A-B (sa=2, sb=3):
//       NRR:  floor(10^6/6)=166_666.
//       NSO*: isqrt128(4·9·10^12/13)=isqrt128(36×10^12/13)=isqrt128(2_769_230_769_230)=1_664_100.
//             (1_664_100²=2_769_228_810_000 ≤ 2_769_230_769_230 < 1_664_101²=2_769_232_138_201)
//       NrSO: isqrt128((1²+2²)·10^12)=isqrt128(5×10^12)=2_236_067.
//             (2_236_067²=4_999_995_628_489 ≤ 5×10^12 < 2_236_068²=5_000_000_100_624)
//     Edge B-C (sa=3, sb=3):
//       NRR:  floor(10^6/9)=111_111.
//       NSO*: isqrt128(9·9·10^12/18)=isqrt128(4_500_000_000_000)=2_121_320.
//             (2_121_320²=4_499_998_542_400 ≤ 4.5×10^12 < 2_121_321²=4_500_002_785_041)
//       NrSO: isqrt128((2²+2²)·10^12)=isqrt128(8×10^12)=2_828_427.
//     Edge C-D: same as A-B.
//     NRR  total = 166_666+111_111+166_666=444_443. ✓
//     NSO* total = 1_664_100+2_121_320+1_664_100=5_449_520. ✓
//     NrSO total = 2_236_067+2_828_427+2_236_067=7_300_561. ✓
//
//   K₄ (S-uniform S=9, 6 edges):
//     NRR  per edge: floor(10^6/81)=12_345 (10^6/81=12_345.679...). Total: 74_070. ✓
//     NSO* per edge: isqrt128(81·81·10^12/162)=isqrt128(40_500_000_000_000)=6_363_961.
//       Total: 38_183_766. ✓
//       (6_363_961²=40_499_999_609_521 ≤ 40.5×10^12 < 6_363_962²=40_500_012_337_444)
//     NrSO per edge: isqrt128((8²+8²)·10^12)=isqrt128(128×10^12)=11_313_708.
//       Total: 67_882_248. ✓
//       (11_313_708²=127_999_997_109_264 ≤ 128×10^12 < 11_313_709²=128_000_019_736_681)
//
//   K_{2,3} (S-uniform S=6, 6 edges):
//     NRR  per edge: floor(10^6/36)=27_777 (10^6/36=27_777.777...). Total: 166_662. ✓
//     NSO* per edge: isqrt128(36·36·10^12/72)=isqrt128(18×10^12)=4_242_640.
//       Total: 25_455_840. ✓
//       (coincidence: NSO* per edge = K₃ NrSO per edge = isqrt128(18×10^12)=4_242_640)
//     NrSO per edge: isqrt128((5²+5²)·10^12)=isqrt128(50×10^12)=7_071_067.
//       Total: 42_426_402. ✓
//       (7_071_067²=49_999_988_518_489 ≤ 50×10^12 < 7_071_068²=50_000_002_660_624)
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (1_000_000, 707_106, 0, 1, 2)
//  4.  Path P₃ = A-B-C                   → (500_000, 2_828_426, 2_828_426, 2, 3)
//  5.  Triangle K₃                       → (187_500, 8_485_281, 12_727_920, 3, 3)
//  6.  Star K_{1,4}                      → (250_000, 11_313_708, 16_970_560, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (444_443, 5_449_520, 7_300_561, 3, 4)
//  8.  Complete K₄                       → (74_070, 38_183_766, 67_882_248, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (166_662, 25_455_840, 42_426_402, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T27_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_27");
const T27_EXEC:   ExecutorId = ExecutorId::from_ascii("t27.exec");

const T27_KEY_A: &str = "t27.alpha";
const T27_KEY_B: &str = "t27.beta";
const T27_KEY_C: &str = "t27.gamma";
const T27_KEY_D: &str = "t27.delta";
const T27_KEY_E: &str = "t27.epsilon";

const T27_ID_A: NodeId = derive_node_id(T27_PLUGIN, T27_KEY_A);
const T27_ID_B: NodeId = derive_node_id(T27_PLUGIN, T27_KEY_B);
const T27_ID_C: NodeId = derive_node_id(T27_PLUGIN, T27_KEY_C);
const T27_ID_D: NodeId = derive_node_id(T27_PLUGIN, T27_KEY_D);
const T27_ID_E: NodeId = derive_node_id(T27_PLUGIN, T27_KEY_E);

// L4=114 namespace for this harness.
const T27_VEC_A: VectorAddress = VectorAddress::new(114, 1, 1, 0);
const T27_VEC_B: VectorAddress = VectorAddress::new(114, 1, 2, 0);
const T27_VEC_C: VectorAddress = VectorAddress::new(114, 1, 3, 0);
const T27_VEC_D: VectorAddress = VectorAddress::new(114, 2, 1, 0);
const T27_VEC_E: VectorAddress = VectorAddress::new(114, 2, 2, 0);

const T27_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T27_PLUGIN,
    name:         "kl-graph-topo27-harness",
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
        executor_id:       T27_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T27_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T27_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nrr, nsos, nrso, ec, nc) = gos_runtime::graph_topo_indices27();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(nrr,  0, "empty: NRR=0");
    assert_eq!(nsos, 0, "empty: NSO*=0");
    assert_eq!(nrso, 0, "empty: NrSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T27_VEC_A, T27_KEY_A, T27_ID_A);

    let (nrr, nsos, nrso, ec, nc) = gos_runtime::graph_topo_indices27();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(nrr,  0, "single: NRR=0 (no edges)");
    assert_eq!(nsos, 0, "single: NSO*=0 (no edges)");
    assert_eq!(nrso, 0, "single: NrSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1.
// NRR:  floor(10^6/(1×1))=1_000_000.
// NSO*: isqrt128(1·1·10^12/(1+1))=isqrt128(5×10^11)=707_106.
// NrSO: isqrt128((0²+0²)·10^12)=0.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T27_VEC_A, T27_KEY_A, T27_ID_A);
    add_node(T27_VEC_B, T27_KEY_B, T27_ID_B);
    add_edge(T27_ID_A, T27_ID_B, "t27.e.ab");

    let (nrr, nsos, nrso, ec, nc) = gos_runtime::graph_topo_indices27();
    assert_eq!(nc,   2,         "k2: node_count=2");
    assert_eq!(ec,   1,         "k2: edge_count=1");
    assert_eq!(nrr,  1_000_000, "k2: NRR=1_000_000 (floor(10\u{2076}/(1\u{00b7}1))=10\u{2076}; S=1)");
    assert_eq!(nsos, 707_106,   "k2: NSO*=707_106 (isqrt128(5\u{00d7}10\u{00b9}\u{00b9})=707_106; S_u\u{00b2}+S_v\u{00b2}=2)");
    assert_eq!(nrso, 0,         "k2: NrSO=0 (isqrt128(0)=0; S=1: (S-1)\u{00b2}=0)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 2 edges.
// NRR  per edge: floor(10^6/4)=250_000. Total: 500_000.
// NSO* per edge: isqrt128(4·4·10^12/8)=isqrt128(2×10^12)=1_414_213. Total: 2_828_426.
// NrSO per edge: isqrt128((1+1)·10^12)=isqrt128(2×10^12)=1_414_213. Total: 2_828_426.
// NSO* = NrSO for S=2 uniform (coincidence: S·S/√(S²+S²) = √((S-1)²+(S-1)²) = √2 at S=2).

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T27_VEC_A, T27_KEY_A, T27_ID_A);
    add_node(T27_VEC_B, T27_KEY_B, T27_ID_B);
    add_node(T27_VEC_C, T27_KEY_C, T27_ID_C);
    add_edge(T27_ID_A, T27_ID_B, "t27.e.ab");
    add_edge(T27_ID_B, T27_ID_C, "t27.e.bc");

    let (nrr, nsos, nrso, ec, nc) = gos_runtime::graph_topo_indices27();
    assert_eq!(nc,   3,         "p3: node_count=3");
    assert_eq!(ec,   2,         "p3: edge_count=2");
    assert_eq!(nrr,  500_000,   "p3: NRR=500_000 (2\u{00d7}250_000; floor(10\u{2076}/4)=250_000; S-uniform S=2)");
    assert_eq!(nsos, 2_828_426, "p3: NSO*=2_828_426 (2\u{00d7}1_414_213; isqrt128(2\u{00d7}10\u{00b9}\u{00b2})=1_414_213)");
    assert_eq!(nrso, 2_828_426, "p3: NrSO=2_828_426 (2\u{00d7}1_414_213; NSO*=NrSO coincidence at S=2 uniform)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 edges.
// NRR  per edge: floor(10^6/16)=62_500. Total: 187_500.
// NSO* per edge: isqrt128(16·16·10^12/32)=isqrt128(8×10^12)=2_828_427. Total: 8_485_281.
// NrSO per edge: isqrt128((9+9)·10^12)=isqrt128(18×10^12)=4_242_640. Total: 12_727_920.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T27_VEC_A, T27_KEY_A, T27_ID_A);
    add_node(T27_VEC_B, T27_KEY_B, T27_ID_B);
    add_node(T27_VEC_C, T27_KEY_C, T27_ID_C);
    add_edge(T27_ID_A, T27_ID_B, "t27.e.ab");
    add_edge(T27_ID_B, T27_ID_A, "t27.e.ba");
    add_edge(T27_ID_B, T27_ID_C, "t27.e.bc");
    add_edge(T27_ID_C, T27_ID_B, "t27.e.cb");
    add_edge(T27_ID_A, T27_ID_C, "t27.e.ac");
    add_edge(T27_ID_C, T27_ID_A, "t27.e.ca");

    let (nrr, nsos, nrso, ec, nc) = gos_runtime::graph_topo_indices27();
    assert_eq!(nc,   3,          "k3: node_count=3");
    assert_eq!(ec,   3,          "k3: edge_count=3");
    assert_eq!(nrr,  187_500,    "k3: NRR=187_500 (3\u{00d7}62_500; floor(10\u{2076}/16)=62_500; S-uniform S=4)");
    assert_eq!(nsos, 8_485_281,  "k3: NSO*=8_485_281 (3\u{00d7}2_828_427; isqrt128(8\u{00d7}10\u{00b9}\u{00b2})=2_828_427)");
    assert_eq!(nrso, 12_727_920, "k3: NrSO=12_727_920 (3\u{00d7}4_242_640; isqrt128(18\u{00d7}10\u{00b9}\u{00b2})=4_242_640)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 4 edges.
// Same per-edge values as K₃ (S-uniform S=4 coincidence).
// NRR: 4×62_500=250_000. NSO*: 4×2_828_427=11_313_708. NrSO: 4×4_242_640=16_970_560.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T27_VEC_A, T27_KEY_A, T27_ID_A);
    add_node(T27_VEC_B, T27_KEY_B, T27_ID_B);
    add_node(T27_VEC_C, T27_KEY_C, T27_ID_C);
    add_node(T27_VEC_D, T27_KEY_D, T27_ID_D);
    add_node(T27_VEC_E, T27_KEY_E, T27_ID_E);
    add_edge(T27_ID_A, T27_ID_B, "t27.e.ab");
    add_edge(T27_ID_A, T27_ID_C, "t27.e.ac");
    add_edge(T27_ID_A, T27_ID_D, "t27.e.ad");
    add_edge(T27_ID_A, T27_ID_E, "t27.e.ae");

    let (nrr, nsos, nrso, ec, nc) = gos_runtime::graph_topo_indices27();
    assert_eq!(nc,   5,          "star: node_count=5");
    assert_eq!(ec,   4,          "star: edge_count=4");
    assert_eq!(nrr,  250_000,    "star: NRR=250_000 (4\u{00d7}62_500; S-uniform S=4 coincides with K\u{2083} per-edge)");
    assert_eq!(nsos, 11_313_708, "star: NSO*=11_313_708 (4\u{00d7}2_828_427; S-uniform S=4 coincidence)");
    assert_eq!(nrso, 16_970_560, "star: NrSO=16_970_560 (4\u{00d7}4_242_640; S-uniform S=4 coincidence)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2.
// S(A)=2, S(B)=3, S(C)=3, S(D)=2.
// Edge A-B (sa=2,sb=3): NRR=166_666; NSO*=1_664_100; NrSO=2_236_067.
// Edge B-C (sa=3,sb=3): NRR=111_111; NSO*=2_121_320; NrSO=2_828_427.
// Edge C-D: same as A-B.
// NRR=444_443. NSO*=5_449_520. NrSO=7_300_561.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T27_VEC_A, T27_KEY_A, T27_ID_A);
    add_node(T27_VEC_B, T27_KEY_B, T27_ID_B);
    add_node(T27_VEC_C, T27_KEY_C, T27_ID_C);
    add_node(T27_VEC_D, T27_KEY_D, T27_ID_D);
    add_edge(T27_ID_A, T27_ID_B, "t27.e.ab");
    add_edge(T27_ID_B, T27_ID_C, "t27.e.bc");
    add_edge(T27_ID_C, T27_ID_D, "t27.e.cd");

    let (nrr, nsos, nrso, ec, nc) = gos_runtime::graph_topo_indices27();
    assert_eq!(nc,   4,         "p4: node_count=4");
    assert_eq!(ec,   3,         "p4: edge_count=3");
    assert_eq!(nrr,  444_443,   "p4: NRR=444_443 (166_666+111_111+166_666; S values 2,3,3,2)");
    assert_eq!(nsos, 5_449_520, "p4: NSO*=5_449_520 (1_664_100+2_121_320+1_664_100; mixed S)");
    assert_eq!(nrso, 7_300_561, "p4: NrSO=7_300_561 (2_236_067+2_828_427+2_236_067; mixed S)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=3×3=9. S-uniform S=9. 6 edges.
// NRR  per edge: floor(10^6/81)=12_345. Total: 74_070.
// NSO* per edge: isqrt128(81²·10^12/162)=isqrt128(40_500_000_000_000)=6_363_961. Total: 38_183_766.
// NrSO per edge: isqrt128((64+64)·10^12)=isqrt128(128×10^12)=11_313_708. Total: 67_882_248.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T27_VEC_A, T27_KEY_A, T27_ID_A);
    add_node(T27_VEC_B, T27_KEY_B, T27_ID_B);
    add_node(T27_VEC_C, T27_KEY_C, T27_ID_C);
    add_node(T27_VEC_D, T27_KEY_D, T27_ID_D);
    add_edge(T27_ID_A, T27_ID_B, "t27.e.ab");
    add_edge(T27_ID_B, T27_ID_A, "t27.e.ba");
    add_edge(T27_ID_A, T27_ID_C, "t27.e.ac");
    add_edge(T27_ID_C, T27_ID_A, "t27.e.ca");
    add_edge(T27_ID_A, T27_ID_D, "t27.e.ad");
    add_edge(T27_ID_D, T27_ID_A, "t27.e.da");
    add_edge(T27_ID_B, T27_ID_C, "t27.e.bc");
    add_edge(T27_ID_C, T27_ID_B, "t27.e.cb");
    add_edge(T27_ID_B, T27_ID_D, "t27.e.bd");
    add_edge(T27_ID_D, T27_ID_B, "t27.e.db");
    add_edge(T27_ID_C, T27_ID_D, "t27.e.cd");
    add_edge(T27_ID_D, T27_ID_C, "t27.e.dc");

    let (nrr, nsos, nrso, ec, nc) = gos_runtime::graph_topo_indices27();
    assert_eq!(nc,   4,          "k4: node_count=4");
    assert_eq!(ec,   6,          "k4: edge_count=6");
    assert_eq!(nrr,  74_070,     "k4: NRR=74_070 (6\u{00d7}12_345; floor(10\u{2076}/81)=12_345; S-uniform S=9)");
    assert_eq!(nsos, 38_183_766, "k4: NSO*=38_183_766 (6\u{00d7}6_363_961; isqrt128(40.5\u{00d7}10\u{00b9}\u{00b2})=6_363_961)");
    assert_eq!(nrso, 67_882_248, "k4: NrSO=67_882_248 (6\u{00d7}11_313_708; isqrt128(128\u{00d7}10\u{00b9}\u{00b2})=11_313_708)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T27_VEC_A, T27_KEY_A, T27_ID_A);
    add_node(T27_VEC_B, T27_KEY_B, T27_ID_B);

    let (nrr, nsos, nrso, ec, nc) = gos_runtime::graph_topo_indices27();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(nrr,  0, "isolated: NRR=0 (no edges)");
    assert_eq!(nsos, 0, "isolated: NSO*=0 (no edges)");
    assert_eq!(nrso, 0, "isolated: NrSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 6 edges.
// NRR  per edge: floor(10^6/36)=27_777. Total: 166_662.
// NSO* per edge: isqrt128(36·36·10^12/72)=isqrt128(18×10^12)=4_242_640. Total: 25_455_840.
//   (NSO* per edge = K₃ NrSO per edge = isqrt128(18·10^12)=4_242_640 — coincidence)
// NrSO per edge: isqrt128((25+25)·10^12)=isqrt128(50×10^12)=7_071_067. Total: 42_426_402.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T27_VEC_A, T27_KEY_A, T27_ID_A);
    add_node(T27_VEC_B, T27_KEY_B, T27_ID_B);
    add_node(T27_VEC_C, T27_KEY_C, T27_ID_C);
    add_node(T27_VEC_D, T27_KEY_D, T27_ID_D);
    add_node(T27_VEC_E, T27_KEY_E, T27_ID_E);
    add_edge(T27_ID_A, T27_ID_C, "t27.e.ac");
    add_edge(T27_ID_C, T27_ID_A, "t27.e.ca");
    add_edge(T27_ID_A, T27_ID_D, "t27.e.ad");
    add_edge(T27_ID_D, T27_ID_A, "t27.e.da");
    add_edge(T27_ID_A, T27_ID_E, "t27.e.ae");
    add_edge(T27_ID_E, T27_ID_A, "t27.e.ea");
    add_edge(T27_ID_B, T27_ID_C, "t27.e.bc");
    add_edge(T27_ID_C, T27_ID_B, "t27.e.cb");
    add_edge(T27_ID_B, T27_ID_D, "t27.e.bd");
    add_edge(T27_ID_D, T27_ID_B, "t27.e.db");
    add_edge(T27_ID_B, T27_ID_E, "t27.e.be");
    add_edge(T27_ID_E, T27_ID_B, "t27.e.eb");

    let (nrr, nsos, nrso, ec, nc) = gos_runtime::graph_topo_indices27();
    assert_eq!(nc,   5,          "k23: node_count=5");
    assert_eq!(ec,   6,          "k23: edge_count=6");
    assert_eq!(nrr,  166_662,    "k23: NRR=166_662 (6\u{00d7}27_777; floor(10\u{2076}/36)=27_777; S-uniform S=6)");
    assert_eq!(nsos, 25_455_840, "k23: NSO*=25_455_840 (6\u{00d7}4_242_640; isqrt128(18\u{00d7}10\u{00b9}\u{00b2})=4_242_640; coincides with K\u{2083} NrSO per-edge)");
    assert_eq!(nrso, 42_426_402, "k23: NrSO=42_426_402 (6\u{00d7}7_071_067; isqrt128(50\u{00d7}10\u{00b9}\u{00b2})=7_071_067)");
}
