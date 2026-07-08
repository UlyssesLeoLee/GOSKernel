// gos-graph-topo14-harness — V3.25 Eccentricity-based topological indices
//
// Verifies `gos_runtime::graph_topo_indices14()`:
//   Returns (te, eds, gea_ppm, edge_count, node_count)
//   - te      = TE(G)  = Σ_v ecc(v)                                   (exact u64; Dankelmann et al. 2004)
//   - eds     = EDS(G) = Σ_v ecc(v)·T_v                               (exact u64; Gupta et al. 2008)
//   - gea_ppm = GEA(G) × 10^6 = Σ_{uv∈E} 2√(ecc(u)·ecc(v))/(ecc(u)+ecc(v)) (floor ppm)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// DEFINITIONS:
//   ecc(v) = max BFS distance from v to any reachable node (0 for isolated).
//   T_v    = vertex transmission = Σ_{w reachable, w≠v} d(v,w).
//   TE:  total eccentricity — aggregate routing reach sum.
//   EDS: eccentric distance sum — ecc × transmission product, amplifies peripheral hubs.
//   GEA: geometric-arithmetic eccentricity index — eccentricity-analog of GA index.
//        = |E|×10^6 iff graph is self-centered (all ecc equal; AM=GM).
//
// KEY INVARIANTS:
//   GEA = |E|×10^6 iff graph is self-centered (K_n, K_{r,s}, even cycles C_{2k}).
//   TE(K_n) = n (all ecc=1).
//   EDS(K_n) = n(n-1) (all ecc=1, T_v=n-1).
//   Isolated nodes: ecc=0, T=0 → contribute 0 to all indices; no edge contribution.
//
// GEA PER-EDGE FORMULA:
//   contribution = isqrt64(4·ea·eb·10^12) / (ea+eb)   where ea=ecc(u), eb=ecc(v)
//   4·ea·eb·10^12 ≤ 4·127²·10^12 ≈ 6.5×10^16 < u64::MAX — no overflow.
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph           TE    EDS   GEA(ppm)    edges  nodes
//  Empty            0     0     0            0      0
//  1 node           0     0     0            0      1
//  Edge A-B         2     2     1_000_000    1      2
//  Path P₃          5    14     1_885_618    2      3
//  Triangle K₃      3     6     3_000_000    3      3
//  Star K_{1,4}     9    60     3_771_236    4      5
//  Path P₄         10    52     2_959_590    3      4
//  Complete K₄      4    12     6_000_000    6      4
//  Two isolated     0     0     0            0      2
//  K_{2,3}         10    56     6_000_000    6      5
//
// Derivations:
//
//   Edge A-B: ecc(A)=ecc(B)=1. T_A=T_B=1.
//     TE=2. EDS=1×1+1×1=2.
//     GEA: isqrt64(4×1×1×10^12)/2 = isqrt64(4e12)/2 = 2_000_000/2 = 1_000_000.
//
//   P₃ (A-B-C): ecc(A)=2, ecc(B)=1, ecc(C)=2. T_A=3, T_B=2, T_C=3.
//     TE=2+1+2=5. EDS=2×3+1×2+2×3=14.
//     GEA per {A,B}: isqrt64(4×2×1×10^12)/3 = isqrt64(8e12)/3.
//       isqrt64(8_000_000_000_000) = 2_828_427 (floor(√8)×10^6).
//       2_828_427/3 = 942_809.  GEA=2×942_809=1_885_618.
//
//   K₃: all ecc=1, T=2. TE=3. EDS=3×(1×2)=6.
//     GEA: per edge isqrt64(4×10^12)/2=2_000_000/2=1_000_000. GEA=3_000_000 (self-centered).
//
//   K_{1,4}: center A ecc=1, T_A=4; each leaf ecc=2, T=7.
//     TE=1+4×2=9. EDS=1×4+4×(2×7)=4+56=60.
//     GEA per edge: isqrt64(4×1×2×10^12)/3 = 2_828_427/3 = 942_809. GEA=4×942_809=3_771_236.
//
//   P₄: ecc A=3,B=2,C=2,D=3. T A=6,B=4,C=4,D=6.
//     TE=10. EDS=3×6+2×4+2×4+3×6=52.
//     {A,B}: isqrt64(24e12)/5 = 4_898_979/5 = 979_795.
//     {B,C}: isqrt64(16e12)/4 = 4_000_000/4 = 1_000_000.
//     {C,D}: 979_795. GEA=2_959_590.
//
//   K₄: all ecc=1, T=3. TE=4. EDS=4×(1×3)=12.
//     GEA: 6×1_000_000=6_000_000 (self-centered).
//
//   Two isolated: ecc=0, T=0. TE=0. EDS=0. GEA=0.
//
//   K_{2,3}: all ecc=2 (self-centered). T_left=5, T_right=6.
//     TE=5×2=10. EDS=2×(2×5)+3×(2×6)=20+36=56.
//     GEA per edge: isqrt64(16e12)/4=1_000_000. GEA=6_000_000 (self-centered).
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (2, 2, 1_000_000, 1, 2)
//  4.  Path P₃ = A-B-C                   → (5, 14, 1_885_618, 2, 3)
//  5.  Triangle K₃                       → (3, 6, 3_000_000, 3, 3)
//  6.  Star K_{1,4}                      → (9, 60, 3_771_236, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (10, 52, 2_959_590, 3, 4)
//  8.  Complete K₄                       → (4, 12, 6_000_000, 6, 4)
//  9.  Two isolated nodes                → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (10, 56, 6_000_000, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T14_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_14");
const T14_EXEC:   ExecutorId = ExecutorId::from_ascii("t14.exec");

const T14_KEY_A: &str = "t14.alpha";
const T14_KEY_B: &str = "t14.beta";
const T14_KEY_C: &str = "t14.gamma";
const T14_KEY_D: &str = "t14.delta";
const T14_KEY_E: &str = "t14.epsilon";

const T14_ID_A: NodeId = derive_node_id(T14_PLUGIN, T14_KEY_A);
const T14_ID_B: NodeId = derive_node_id(T14_PLUGIN, T14_KEY_B);
const T14_ID_C: NodeId = derive_node_id(T14_PLUGIN, T14_KEY_C);
const T14_ID_D: NodeId = derive_node_id(T14_PLUGIN, T14_KEY_D);
const T14_ID_E: NodeId = derive_node_id(T14_PLUGIN, T14_KEY_E);

// L4=101 namespace for this harness.
const T14_VEC_A: VectorAddress = VectorAddress::new(101, 1, 1, 0);
const T14_VEC_B: VectorAddress = VectorAddress::new(101, 1, 2, 0);
const T14_VEC_C: VectorAddress = VectorAddress::new(101, 1, 3, 0);
const T14_VEC_D: VectorAddress = VectorAddress::new(101, 2, 1, 0);
const T14_VEC_E: VectorAddress = VectorAddress::new(101, 2, 2, 0);

const T14_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T14_PLUGIN,
    name:         "kl-graph-topo14-harness",
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
        executor_id:       T14_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T14_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T14_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (te, eds, gea, ec, nc) = gos_runtime::graph_topo_indices14();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(te,  0, "empty: TE=0");
    assert_eq!(eds, 0, "empty: EDS=0");
    assert_eq!(gea, 0, "empty: GEA=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// ecc=0 (no reachable nodes). T=0. All indices zero.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T14_VEC_A, T14_KEY_A, T14_ID_A);

    let (te, eds, gea, ec, nc) = gos_runtime::graph_topo_indices14();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(te,  0, "single: TE=0 (ecc=0 for isolated)");
    assert_eq!(eds, 0, "single: EDS=0");
    assert_eq!(gea, 0, "single: GEA=0");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected {A,B}. ecc(A)=ecc(B)=1. T_A=T_B=1.
// TE=2. EDS=1×1+1×1=2.
// GEA: isqrt64(4×1×1×10^12)/2 = 2_000_000/2 = 1_000_000.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T14_VEC_A, T14_KEY_A, T14_ID_A);
    add_node(T14_VEC_B, T14_KEY_B, T14_ID_B);
    add_edge(T14_ID_A, T14_ID_B, "t14.e.ab");

    let (te, eds, gea, ec, nc) = gos_runtime::graph_topo_indices14();
    assert_eq!(nc,  2,         "edge: node_count=2");
    assert_eq!(ec,  1,         "edge: edge_count=1");
    assert_eq!(te,  2,         "edge: TE=2 (ecc(A)=ecc(B)=1; 1+1)");
    assert_eq!(eds, 2,         "edge: EDS=2 (1×1+1×1)");
    assert_eq!(gea, 1_000_000, "edge: GEA=1_000_000 (self-centered; ea=eb=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// ecc(A)=2, ecc(B)=1, ecc(C)=2. T_A=3, T_B=2, T_C=3.
// TE=5. EDS=2×3+1×2+2×3=14.
// GEA {A,B}: isqrt64(8e12)/3 = 2_828_427/3 = 942_809.
// GEA {B,C}: same = 942_809. GEA=1_885_618.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T14_VEC_A, T14_KEY_A, T14_ID_A);
    add_node(T14_VEC_B, T14_KEY_B, T14_ID_B);
    add_node(T14_VEC_C, T14_KEY_C, T14_ID_C);
    add_edge(T14_ID_A, T14_ID_B, "t14.e.ab");
    add_edge(T14_ID_B, T14_ID_C, "t14.e.bc");

    let (te, eds, gea, ec, nc) = gos_runtime::graph_topo_indices14();
    assert_eq!(nc,  3,         "p3: node_count=3");
    assert_eq!(ec,  2,         "p3: edge_count=2");
    assert_eq!(te,  5,         "p3: TE=5 (ecc:2+1+2)");
    assert_eq!(eds, 14,        "p3: EDS=14 (2×3+1×2+2×3)");
    assert_eq!(gea, 1_885_618, "p3: GEA=1_885_618 (2×942_809; isqrt64(8e12)/3)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// All ecc=1 (self-centered), T=2. TE=3. EDS=3×(1×2)=6.
// GEA: 3 edges × 1_000_000 = 3_000_000 (self-centered).

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T14_VEC_A, T14_KEY_A, T14_ID_A);
    add_node(T14_VEC_B, T14_KEY_B, T14_ID_B);
    add_node(T14_VEC_C, T14_KEY_C, T14_ID_C);
    add_edge(T14_ID_A, T14_ID_B, "t14.e.ab");
    add_edge(T14_ID_B, T14_ID_A, "t14.e.ba");
    add_edge(T14_ID_B, T14_ID_C, "t14.e.bc");
    add_edge(T14_ID_C, T14_ID_B, "t14.e.cb");
    add_edge(T14_ID_A, T14_ID_C, "t14.e.ac");
    add_edge(T14_ID_C, T14_ID_A, "t14.e.ca");

    let (te, eds, gea, ec, nc) = gos_runtime::graph_topo_indices14();
    assert_eq!(nc,  3,         "k3: node_count=3");
    assert_eq!(ec,  3,         "k3: edge_count=3");
    assert_eq!(te,  3,         "k3: TE=3 (all ecc=1; 3×1)");
    assert_eq!(eds, 6,         "k3: EDS=6 (3×(1×2); ecc=1, T=2)");
    assert_eq!(gea, 3_000_000, "k3: GEA=3_000_000 (self-centered; all ecc=1)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Center A: ecc=1, T_A=4. Each leaf: ecc=2, T=7.
// TE=1+4×2=9. EDS=1×4+4×(2×7)=4+56=60.
// GEA per edge (center-leaf): isqrt64(4×1×2×10^12)/3 = 2_828_427/3 = 942_809.
// GEA=4×942_809=3_771_236.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T14_VEC_A, T14_KEY_A, T14_ID_A);
    add_node(T14_VEC_B, T14_KEY_B, T14_ID_B);
    add_node(T14_VEC_C, T14_KEY_C, T14_ID_C);
    add_node(T14_VEC_D, T14_KEY_D, T14_ID_D);
    add_node(T14_VEC_E, T14_KEY_E, T14_ID_E);
    add_edge(T14_ID_A, T14_ID_B, "t14.e.ab");
    add_edge(T14_ID_A, T14_ID_C, "t14.e.ac");
    add_edge(T14_ID_A, T14_ID_D, "t14.e.ad");
    add_edge(T14_ID_A, T14_ID_E, "t14.e.ae");

    let (te, eds, gea, ec, nc) = gos_runtime::graph_topo_indices14();
    assert_eq!(nc,  5,         "star: node_count=5");
    assert_eq!(ec,  4,         "star: edge_count=4");
    assert_eq!(te,  9,         "star: TE=9 (center ecc=1, 4 leaves ecc=2; 1+8)");
    assert_eq!(eds, 60,        "star: EDS=60 (1×4+4×14; center=4, leaf=2×7=14)");
    assert_eq!(gea, 3_771_236, "star: GEA=3_771_236 (4×942_809; isqrt64(8e12)/3)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// ecc: A=3, B=2, C=2, D=3. T: A=6, B=4, C=4, D=6.
// TE=10. EDS=3×6+2×4+2×4+3×6=52.
// {A,B}: isqrt64(24e12)/5 = 4_898_979/5 = 979_795.
// {B,C}: isqrt64(16e12)/4 = 1_000_000.
// {C,D}: 979_795. GEA=2_959_590.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T14_VEC_A, T14_KEY_A, T14_ID_A);
    add_node(T14_VEC_B, T14_KEY_B, T14_ID_B);
    add_node(T14_VEC_C, T14_KEY_C, T14_ID_C);
    add_node(T14_VEC_D, T14_KEY_D, T14_ID_D);
    add_edge(T14_ID_A, T14_ID_B, "t14.e.ab");
    add_edge(T14_ID_B, T14_ID_C, "t14.e.bc");
    add_edge(T14_ID_C, T14_ID_D, "t14.e.cd");

    let (te, eds, gea, ec, nc) = gos_runtime::graph_topo_indices14();
    assert_eq!(nc,  4,         "p4: node_count=4");
    assert_eq!(ec,  3,         "p4: edge_count=3");
    assert_eq!(te,  10,        "p4: TE=10 (3+2+2+3)");
    assert_eq!(eds, 52,        "p4: EDS=52 (18+8+8+18; ecc×T per node)");
    assert_eq!(gea, 2_959_590, "p4: GEA=2_959_590 (979_795+1_000_000+979_795)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// All ecc=1 (self-centered), T=3. TE=4. EDS=4×(1×3)=12.
// GEA: 6 edges × 1_000_000 = 6_000_000 (self-centered).

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T14_VEC_A, T14_KEY_A, T14_ID_A);
    add_node(T14_VEC_B, T14_KEY_B, T14_ID_B);
    add_node(T14_VEC_C, T14_KEY_C, T14_ID_C);
    add_node(T14_VEC_D, T14_KEY_D, T14_ID_D);
    add_edge(T14_ID_A, T14_ID_B, "t14.e.ab");
    add_edge(T14_ID_B, T14_ID_A, "t14.e.ba");
    add_edge(T14_ID_A, T14_ID_C, "t14.e.ac");
    add_edge(T14_ID_C, T14_ID_A, "t14.e.ca");
    add_edge(T14_ID_A, T14_ID_D, "t14.e.ad");
    add_edge(T14_ID_D, T14_ID_A, "t14.e.da");
    add_edge(T14_ID_B, T14_ID_C, "t14.e.bc");
    add_edge(T14_ID_C, T14_ID_B, "t14.e.cb");
    add_edge(T14_ID_B, T14_ID_D, "t14.e.bd");
    add_edge(T14_ID_D, T14_ID_B, "t14.e.db");
    add_edge(T14_ID_C, T14_ID_D, "t14.e.cd");
    add_edge(T14_ID_D, T14_ID_C, "t14.e.dc");

    let (te, eds, gea, ec, nc) = gos_runtime::graph_topo_indices14();
    assert_eq!(nc,  4,         "k4: node_count=4");
    assert_eq!(ec,  6,         "k4: edge_count=6");
    assert_eq!(te,  4,         "k4: TE=4 (all ecc=1; n=4)");
    assert_eq!(eds, 12,        "k4: EDS=12 (4×(1×3); ecc=1, T=3)");
    assert_eq!(gea, 6_000_000, "k4: GEA=6_000_000 (self-centered; all ecc=1)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// n=2, m=0. ecc=0, T=0 for both. All indices zero.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T14_VEC_A, T14_KEY_A, T14_ID_A);
    add_node(T14_VEC_B, T14_KEY_B, T14_ID_B);

    let (te, eds, gea, ec, nc) = gos_runtime::graph_topo_indices14();
    assert_eq!(nc,  2, "isolated: node_count=2");
    assert_eq!(ec,  0, "isolated: no edges");
    assert_eq!(te,  0, "isolated: TE=0 (ecc=0 for isolated)");
    assert_eq!(eds, 0, "isolated: EDS=0");
    assert_eq!(gea, 0, "isolated: GEA=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}, right={C,D,E}. All left-right edges present.
// K_{2,3} is self-centered: all ecc=2 (farthest is across the bipartite partition = d=2).
// T_A=T_B: d(A,B)=2, d(A,C)=d(A,D)=d(A,E)=1. T_A=5.
// T_C=T_D=T_E: d(C,A)=d(C,B)=1, d(C,D)=d(C,E)=2. T_C=6.
// TE=5×2=10.
// EDS=2×(2×5)+3×(2×6)=20+36=56.
// GEA per edge: ea=eb=2 → isqrt64(16e12)/4=4_000_000/4=1_000_000.
// GEA=6_000_000 (self-centered — confirms K_{2,3} is self-centered).

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T14_VEC_A, T14_KEY_A, T14_ID_A);
    add_node(T14_VEC_B, T14_KEY_B, T14_ID_B);
    add_node(T14_VEC_C, T14_KEY_C, T14_ID_C);
    add_node(T14_VEC_D, T14_KEY_D, T14_ID_D);
    add_node(T14_VEC_E, T14_KEY_E, T14_ID_E);
    // Left A connects to all right
    add_edge(T14_ID_A, T14_ID_C, "t14.e.ac");
    add_edge(T14_ID_C, T14_ID_A, "t14.e.ca");
    add_edge(T14_ID_A, T14_ID_D, "t14.e.ad");
    add_edge(T14_ID_D, T14_ID_A, "t14.e.da");
    add_edge(T14_ID_A, T14_ID_E, "t14.e.ae");
    add_edge(T14_ID_E, T14_ID_A, "t14.e.ea");
    // Left B connects to all right
    add_edge(T14_ID_B, T14_ID_C, "t14.e.bc");
    add_edge(T14_ID_C, T14_ID_B, "t14.e.cb");
    add_edge(T14_ID_B, T14_ID_D, "t14.e.bd");
    add_edge(T14_ID_D, T14_ID_B, "t14.e.db");
    add_edge(T14_ID_B, T14_ID_E, "t14.e.be");
    add_edge(T14_ID_E, T14_ID_B, "t14.e.eb");

    let (te, eds, gea, ec, nc) = gos_runtime::graph_topo_indices14();
    assert_eq!(nc,  5,         "k23: node_count=5");
    assert_eq!(ec,  6,         "k23: edge_count=6");
    assert_eq!(te,  10,        "k23: TE=10 (all ecc=2; 5×2)");
    assert_eq!(eds, 56,        "k23: EDS=56 (2×(2×5)+3×(2×6)=20+36)");
    assert_eq!(gea, 6_000_000, "k23: GEA=6_000_000 (self-centered; all ecc=2)");
}
