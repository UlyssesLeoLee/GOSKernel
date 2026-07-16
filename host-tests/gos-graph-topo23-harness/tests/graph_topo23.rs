// gos-graph-topo23-harness — V3.34 NHM1 + NSDD + NM3 (Neighborhood S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices23()`:
//   Returns (nhm1, nsdd_ppm, nm3, edge_count, node_count)
//   - nhm1     = NHM1(G) = Σ_{uv∈E} (S_u+S_v)²               (exact u64; S-HM₁)
//   - nsdd_ppm = NSDD(G) × 10^6 = Σ_{uv∈E} (S_u²+S_v²)/(S_u·S_v) × 10^6  (floor ppm; S-SDD)
//   - nm3      = NM3(G)  = Σ_{uv∈E} |S_u−S_v|                (exact u64; S-M₃ irregularity)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHM1(G) = Σ_{uv∈E} (S_u+S_v)²         (S-analogue of HM₁; Shirdel et al. 2013)
//   NSDD(G) = Σ_{uv∈E} (S_u²+S_v²)/(S_u·S_v)  (S-analogue of SDD; Vasilyev 2014)
//   NM3(G)  = Σ_{uv∈E} |S_u−S_v|          (S-analogue of M₃/Albertson irregularity)
//
// IMPLEMENTATION FORMULAS (no float, no_std safe):
//   NHM1 per edge = (S_u+S_v)²                          [exact; max=(32258)²≈10^9 per edge]
//   NSDD per edge = floor((S_u²+S_v²)×10^6 / (S_u·S_v)) [integer division; ≥2×10^6 always]
//   NM3  per edge = |S_u−S_v|                           [exact; =0 when S_u=S_v]
//
// KEY INVARIANTS:
//   NSDD ≥ 2|E|×10^6 always (AM-GM: (S²_u+S²_v)/(S_u·S_v) ≥ 2; equality iff S_u=S_v).
//   NSDD = 2|E|×10^6 iff S-regular (all S values equal across the graph).
//   NM3  = 0 iff S-regular.
//   For S-uniform (S=c): NSDD = m × floor(2c²×10^6/c²) = m × 2_000_000 (exact).
//
// S VALUES PER GRAPH (same S-variant as topo18/topo21/topo22):
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
//  Graph        NHM1      NSDD(ppm)    NM3   edges  nodes
//  Empty           0             0      0       0      0
//  1 node          0             0      0       0      1
//  Edge K₂         4     2_000_000      0       1      2
//  Path P₃        32     4_000_000      0       2      3
//  Triangle K₃   192     6_000_000      0       3      3
//  Star K_{1,4}  256     8_000_000      0       4      5
//  Path P₄        86     6_333_332      2       3      4
//  Complete K₄  1944    12_000_000      0       6      4
//  2 isolated      0             0      0       0      2
//  K_{2,3}       864    12_000_000      0       6      5
//
// Derivations:
//
//   K₂ (S_A=S_B=1, ssum=2, sp=1):
//     NHM1: (1+1)² = 4. ✓
//     NSDD: floor((1+1)×10^6/1) = 2_000_000. ✓  (S-regular: 2|E|×10^6 = 2_000_000 ✓)
//     NM3:  |1-1| = 0. ✓
//
//   P₃ (S-uniform S=2, 2 edges; ssum=4, sp=4):
//     NHM1: 2×(2+2)² = 2×16 = 32. ✓
//     NSDD per edge: floor((4+4)×10^6/4) = floor(8×10^6/4) = 2_000_000.
//            total: 2×2_000_000 = 4_000_000 = 2|E|×10^6 (S-uniform ✓). ✓
//     NM3:  2×|2-2| = 0 (S-uniform). ✓
//
//   K₃ (S=4, ssum=8, sp=16, 3 edges):
//     NHM1: 3×(4+4)² = 3×64 = 192. ✓
//     NSDD per edge: floor((16+16)×10^6/16) = floor(32×10^6/16) = 2_000_000.
//            total: 3×2_000_000 = 6_000_000 = 2|E|×10^6 (S-uniform ✓). ✓
//     NM3:  3×|4-4| = 0. ✓
//
//   K_{1,4} (S=4 for all; ssum=8, sp=16, 4 edges):
//     NHM1: 4×(4+4)² = 4×64 = 256. ✓
//     NSDD: 4×2_000_000 = 8_000_000 = 2|E|×10^6 (S-uniform ✓). ✓
//     NM3:  4×|4-4| = 0. ✓
//
//   P₄ (S_A=2, S_B=3, S_C=3, S_D=2):
//     Edge A-B (sa=2,sb=3): NHM1=(2+3)²=25; NSDD=floor((4+9)×10^6/6)=floor(13_000_000/6)=2_166_666;
//                            NM3=|2-3|=1.
//     Edge B-C (sa=3,sb=3): NHM1=(3+3)²=36; NSDD=floor((9+9)×10^6/9)=floor(18_000_000/9)=2_000_000;
//                            NM3=|3-3|=0.
//     Edge C-D (sa=3,sb=2): same as A-B by symmetry: NHM1=25; NSDD=2_166_666; NM3=1.
//     NHM1 total = 25+36+25 = 86. ✓
//     NSDD total = 2_166_666+2_000_000+2_166_666 = 6_333_332. ✓
//     NM3 total  = 1+0+1 = 2. ✓
//     (NSDD > 2|E|×10^6 = 6_000_000, confirming P₄ is not S-regular ✓)
//
//   K₄ (S=9, ssum=18, sp=81, 6 edges):
//     NHM1: 6×(9+9)² = 6×324 = 1944. ✓
//     NSDD per edge: floor((81+81)×10^6/81) = floor(162_000_000/81) = 2_000_000.
//            total: 6×2_000_000 = 12_000_000 = 2|E|×10^6 (S-uniform ✓). ✓
//     NM3:  6×|9-9| = 0. ✓
//
//   K_{2,3} (S=6 for all, ssum=12, sp=36, 6 edges):
//     NHM1: 6×(6+6)² = 6×144 = 864. ✓
//     NSDD per edge: floor((36+36)×10^6/36) = floor(72_000_000/36) = 2_000_000.
//            total: 6×2_000_000 = 12_000_000 = 2|E|×10^6 (S-uniform ✓). ✓
//     NM3:  6×|6-6| = 0. ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (4, 2_000_000, 0, 1, 2)
//  4.  Path P₃ = A-B-C                   → (32, 4_000_000, 0, 2, 3)
//  5.  Triangle K₃                       → (192, 6_000_000, 0, 3, 3)
//  6.  Star K_{1,4}                      → (256, 8_000_000, 0, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (86, 6_333_332, 2, 3, 4)
//  8.  Complete K₄                       → (1944, 12_000_000, 0, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (864, 12_000_000, 0, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T23_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_23");
const T23_EXEC:   ExecutorId = ExecutorId::from_ascii("t23.exec");

const T23_KEY_A: &str = "t23.alpha";
const T23_KEY_B: &str = "t23.beta";
const T23_KEY_C: &str = "t23.gamma";
const T23_KEY_D: &str = "t23.delta";
const T23_KEY_E: &str = "t23.epsilon";

const T23_ID_A: NodeId = derive_node_id(T23_PLUGIN, T23_KEY_A);
const T23_ID_B: NodeId = derive_node_id(T23_PLUGIN, T23_KEY_B);
const T23_ID_C: NodeId = derive_node_id(T23_PLUGIN, T23_KEY_C);
const T23_ID_D: NodeId = derive_node_id(T23_PLUGIN, T23_KEY_D);
const T23_ID_E: NodeId = derive_node_id(T23_PLUGIN, T23_KEY_E);

// L4=110 namespace for this harness.
const T23_VEC_A: VectorAddress = VectorAddress::new(110, 1, 1, 0);
const T23_VEC_B: VectorAddress = VectorAddress::new(110, 1, 2, 0);
const T23_VEC_C: VectorAddress = VectorAddress::new(110, 1, 3, 0);
const T23_VEC_D: VectorAddress = VectorAddress::new(110, 2, 1, 0);
const T23_VEC_E: VectorAddress = VectorAddress::new(110, 2, 2, 0);

const T23_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T23_PLUGIN,
    name:         "kl-graph-topo23-harness",
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
        executor_id:       T23_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T23_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T23_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nhm1, nsdd, nm3, ec, nc) = gos_runtime::graph_topo_indices23();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(nhm1, 0, "empty: NHM1=0");
    assert_eq!(nsdd, 0, "empty: NSDD=0");
    assert_eq!(nm3,  0, "empty: NM3=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T23_VEC_A, T23_KEY_A, T23_ID_A);

    let (nhm1, nsdd, nm3, ec, nc) = gos_runtime::graph_topo_indices23();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(nhm1, 0, "single: NHM1=0 (no edges)");
    assert_eq!(nsdd, 0, "single: NSDD=0 (no edges)");
    assert_eq!(nm3,  0, "single: NM3=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. ssum=2, sp=1.
// NHM1: (1+1)² = 4.
// NSDD: floor((1²+1²)×10^6/1) = floor(2_000_000) = 2_000_000.  (=2|E|×10^6 ✓)
// NM3:  |1-1| = 0.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T23_VEC_A, T23_KEY_A, T23_ID_A);
    add_node(T23_VEC_B, T23_KEY_B, T23_ID_B);
    add_edge(T23_ID_A, T23_ID_B, "t23.e.ab");

    let (nhm1, nsdd, nm3, ec, nc) = gos_runtime::graph_topo_indices23();
    assert_eq!(nc,   2,         "k2: node_count=2");
    assert_eq!(ec,   1,         "k2: edge_count=1");
    assert_eq!(nhm1, 4,         "k2: NHM1=4 ((1+1)\u{00b2}=4)");
    assert_eq!(nsdd, 2_000_000, "k2: NSDD=2_000_000 (S-regular: 2\u{00d7}|E|\u{00d7}10\u{2076})");
    assert_eq!(nm3,  0,         "k2: NM3=0 (S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S(A)=S(B)=S(C)=2 (S-uniform S=2).
// NHM1: 2×(2+2)² = 2×16 = 32.
// NSDD per edge: floor((4+4)×10^6/4) = 2_000_000. Total: 4_000_000. (=2|E|×10^6 ✓)
// NM3: 2×|2-2| = 0.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T23_VEC_A, T23_KEY_A, T23_ID_A);
    add_node(T23_VEC_B, T23_KEY_B, T23_ID_B);
    add_node(T23_VEC_C, T23_KEY_C, T23_ID_C);
    add_edge(T23_ID_A, T23_ID_B, "t23.e.ab");
    add_edge(T23_ID_B, T23_ID_C, "t23.e.bc");

    let (nhm1, nsdd, nm3, ec, nc) = gos_runtime::graph_topo_indices23();
    assert_eq!(nc,   3,         "p3: node_count=3");
    assert_eq!(ec,   2,         "p3: edge_count=2");
    assert_eq!(nhm1, 32,        "p3: NHM1=32 (2\u{00d7}(2+2)\u{00b2}=2\u{00d7}16; S-uniform S=2)");
    assert_eq!(nsdd, 4_000_000, "p3: NSDD=4_000_000 (2\u{00d7}2_000_000; S-uniform, \u{2261}2|E|\u{00d7}10\u{2076})");
    assert_eq!(nm3,  0,         "p3: NM3=0 (S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. ssum=8, sp=16.
// NHM1: 3×(4+4)² = 3×64 = 192.
// NSDD per edge: floor((16+16)×10^6/16) = 2_000_000. Total: 6_000_000. (=2|E|×10^6 ✓)
// NM3: 3×|4-4| = 0.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T23_VEC_A, T23_KEY_A, T23_ID_A);
    add_node(T23_VEC_B, T23_KEY_B, T23_ID_B);
    add_node(T23_VEC_C, T23_KEY_C, T23_ID_C);
    add_edge(T23_ID_A, T23_ID_B, "t23.e.ab");
    add_edge(T23_ID_B, T23_ID_A, "t23.e.ba");
    add_edge(T23_ID_B, T23_ID_C, "t23.e.bc");
    add_edge(T23_ID_C, T23_ID_B, "t23.e.cb");
    add_edge(T23_ID_A, T23_ID_C, "t23.e.ac");
    add_edge(T23_ID_C, T23_ID_A, "t23.e.ca");

    let (nhm1, nsdd, nm3, ec, nc) = gos_runtime::graph_topo_indices23();
    assert_eq!(nc,   3,         "k3: node_count=3");
    assert_eq!(ec,   3,         "k3: edge_count=3");
    assert_eq!(nhm1, 192,       "k3: NHM1=192 (3\u{00d7}(4+4)\u{00b2}=3\u{00d7}64; S-uniform S=4)");
    assert_eq!(nsdd, 6_000_000, "k3: NSDD=6_000_000 (3\u{00d7}2_000_000; \u{2261}2|E|\u{00d7}10\u{2076})");
    assert_eq!(nm3,  0,         "k3: NM3=0 (S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4×1=4, S(leaf)=deg(hub)=4. S-uniform S=4.
// NHM1: 4×(4+4)² = 4×64 = 256.
// NSDD: 4×2_000_000 = 8_000_000. (=2|E|×10^6 ✓)
// NM3: 4×|4-4| = 0.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T23_VEC_A, T23_KEY_A, T23_ID_A);
    add_node(T23_VEC_B, T23_KEY_B, T23_ID_B);
    add_node(T23_VEC_C, T23_KEY_C, T23_ID_C);
    add_node(T23_VEC_D, T23_KEY_D, T23_ID_D);
    add_node(T23_VEC_E, T23_KEY_E, T23_ID_E);
    add_edge(T23_ID_A, T23_ID_B, "t23.e.ab");
    add_edge(T23_ID_A, T23_ID_C, "t23.e.ac");
    add_edge(T23_ID_A, T23_ID_D, "t23.e.ad");
    add_edge(T23_ID_A, T23_ID_E, "t23.e.ae");

    let (nhm1, nsdd, nm3, ec, nc) = gos_runtime::graph_topo_indices23();
    assert_eq!(nc,   5,         "star: node_count=5");
    assert_eq!(ec,   4,         "star: edge_count=4");
    assert_eq!(nhm1, 256,       "star: NHM1=256 (4\u{00d7}64; S-uniform S=4, same as K\u{2083})");
    assert_eq!(nsdd, 8_000_000, "star: NSDD=8_000_000 (4\u{00d7}2_000_000; \u{2261}2|E|\u{00d7}10\u{2076})");
    assert_eq!(nm3,  0,         "star: NM3=0 (S-uniform S=4)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2.
// S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=3, S(C)=deg(B)+deg(D)=3, S(D)=deg(C)=2.
// Edge A-B (sa=2,sb=3): NHM1=25; NSDD=floor(13×10^6/6)=2_166_666; NM3=1.
// Edge B-C (sa=3,sb=3): NHM1=36; NSDD=floor(18×10^6/9)=2_000_000; NM3=0.
// Edge C-D (sa=3,sb=2): NHM1=25; NSDD=2_166_666; NM3=1.
// NHM1 = 25+36+25 = 86.
// NSDD = 2_166_666+2_000_000+2_166_666 = 6_333_332.
// NM3  = 1+0+1 = 2.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T23_VEC_A, T23_KEY_A, T23_ID_A);
    add_node(T23_VEC_B, T23_KEY_B, T23_ID_B);
    add_node(T23_VEC_C, T23_KEY_C, T23_ID_C);
    add_node(T23_VEC_D, T23_KEY_D, T23_ID_D);
    add_edge(T23_ID_A, T23_ID_B, "t23.e.ab");
    add_edge(T23_ID_B, T23_ID_C, "t23.e.bc");
    add_edge(T23_ID_C, T23_ID_D, "t23.e.cd");

    let (nhm1, nsdd, nm3, ec, nc) = gos_runtime::graph_topo_indices23();
    assert_eq!(nc,   4,         "p4: node_count=4");
    assert_eq!(ec,   3,         "p4: edge_count=3");
    assert_eq!(nhm1, 86,        "p4: NHM1=86 (25+36+25; mixed S=2,3)");
    assert_eq!(nsdd, 6_333_332, "p4: NSDD=6_333_332 (2_166_666+2_000_000+2_166_666; >2|E|\u{00d7}10\u{2076}=6_000_000)");
    assert_eq!(nm3,  2,         "p4: NM3=2 (2 edges with |S_u-S_v|=1; B-C contributes 0)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=3×3=9. S-uniform S=9. ssum=18, sp=81.
// NHM1: 6×(9+9)² = 6×324 = 1944.
// NSDD per edge: floor((81+81)×10^6/81) = floor(162_000_000/81) = 2_000_000.
//        total: 6×2_000_000 = 12_000_000. (=2|E|×10^6 ✓)
// NM3:  6×|9-9| = 0.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T23_VEC_A, T23_KEY_A, T23_ID_A);
    add_node(T23_VEC_B, T23_KEY_B, T23_ID_B);
    add_node(T23_VEC_C, T23_KEY_C, T23_ID_C);
    add_node(T23_VEC_D, T23_KEY_D, T23_ID_D);
    add_edge(T23_ID_A, T23_ID_B, "t23.e.ab");
    add_edge(T23_ID_B, T23_ID_A, "t23.e.ba");
    add_edge(T23_ID_A, T23_ID_C, "t23.e.ac");
    add_edge(T23_ID_C, T23_ID_A, "t23.e.ca");
    add_edge(T23_ID_A, T23_ID_D, "t23.e.ad");
    add_edge(T23_ID_D, T23_ID_A, "t23.e.da");
    add_edge(T23_ID_B, T23_ID_C, "t23.e.bc");
    add_edge(T23_ID_C, T23_ID_B, "t23.e.cb");
    add_edge(T23_ID_B, T23_ID_D, "t23.e.bd");
    add_edge(T23_ID_D, T23_ID_B, "t23.e.db");
    add_edge(T23_ID_C, T23_ID_D, "t23.e.cd");
    add_edge(T23_ID_D, T23_ID_C, "t23.e.dc");

    let (nhm1, nsdd, nm3, ec, nc) = gos_runtime::graph_topo_indices23();
    assert_eq!(nc,   4,          "k4: node_count=4");
    assert_eq!(ec,   6,          "k4: edge_count=6");
    assert_eq!(nhm1, 1944,       "k4: NHM1=1944 (6\u{00d7}(9+9)\u{00b2}=6\u{00d7}324; S-uniform S=9)");
    assert_eq!(nsdd, 12_000_000, "k4: NSDD=12_000_000 (6\u{00d7}2_000_000; \u{2261}2|E|\u{00d7}10\u{2076})");
    assert_eq!(nm3,  0,          "k4: NM3=0 (S-uniform S=9)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T23_VEC_A, T23_KEY_A, T23_ID_A);
    add_node(T23_VEC_B, T23_KEY_B, T23_ID_B);

    let (nhm1, nsdd, nm3, ec, nc) = gos_runtime::graph_topo_indices23();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(nhm1, 0, "isolated: NHM1=0 (no edges)");
    assert_eq!(nsdd, 0, "isolated: NSDD=0 (no edges)");
    assert_eq!(nm3,  0, "isolated: NM3=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}:d=3. Right={C,D,E}:d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. ssum=12, sp=36.
// NHM1: 6×(6+6)² = 6×144 = 864.
// NSDD per edge: floor((36+36)×10^6/36) = floor(72_000_000/36) = 2_000_000.
//        total: 6×2_000_000 = 12_000_000. (=2|E|×10^6 ✓)
// NM3:  6×|6-6| = 0.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T23_VEC_A, T23_KEY_A, T23_ID_A);
    add_node(T23_VEC_B, T23_KEY_B, T23_ID_B);
    add_node(T23_VEC_C, T23_KEY_C, T23_ID_C);
    add_node(T23_VEC_D, T23_KEY_D, T23_ID_D);
    add_node(T23_VEC_E, T23_KEY_E, T23_ID_E);
    add_edge(T23_ID_A, T23_ID_C, "t23.e.ac");
    add_edge(T23_ID_C, T23_ID_A, "t23.e.ca");
    add_edge(T23_ID_A, T23_ID_D, "t23.e.ad");
    add_edge(T23_ID_D, T23_ID_A, "t23.e.da");
    add_edge(T23_ID_A, T23_ID_E, "t23.e.ae");
    add_edge(T23_ID_E, T23_ID_A, "t23.e.ea");
    add_edge(T23_ID_B, T23_ID_C, "t23.e.bc");
    add_edge(T23_ID_C, T23_ID_B, "t23.e.cb");
    add_edge(T23_ID_B, T23_ID_D, "t23.e.bd");
    add_edge(T23_ID_D, T23_ID_B, "t23.e.db");
    add_edge(T23_ID_B, T23_ID_E, "t23.e.be");
    add_edge(T23_ID_E, T23_ID_B, "t23.e.eb");

    let (nhm1, nsdd, nm3, ec, nc) = gos_runtime::graph_topo_indices23();
    assert_eq!(nc,   5,          "k23: node_count=5");
    assert_eq!(ec,   6,          "k23: edge_count=6");
    assert_eq!(nhm1, 864,        "k23: NHM1=864 (6\u{00d7}(6+6)\u{00b2}=6\u{00d7}144; S-uniform S=6)");
    assert_eq!(nsdd, 12_000_000, "k23: NSDD=12_000_000 (6\u{00d7}2_000_000; \u{2261}2|E|\u{00d7}10\u{2076}; same as K\u{2084})");
    assert_eq!(nm3,  0,          "k23: NM3=0 (S-uniform S=6)");
}
