// gos-graph-topo8-harness — V3.19 eccentricity-based topological indices
//
// Verifies `gos_runtime::graph_topo_indices8()`:
//   Returns (eci, avg_ecc_ppm, diameter, radius, edge_count, node_count)
//   - eci         = ξ(G) = Σ_v deg(v) × ecc(v)          (exact u64; Sharma, Goswami & Madan 1997)
//   - avg_ecc_ppm = (Σ_v ecc(v)) / n × 10^6             (floor ppm; Buckley & Harary 1990)
//   - diameter    = D(G) = max_{v} ecc(v)                (exact u32)
//   - radius      = R(G) = min{ecc(v) | ecc(v) > 0}     (exact u32; 0 if no connected pairs)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// ecc(v) = max BFS distance from v to any reachable node (0 for isolated/single nodes).
// Undirected projection: A→B + B→A collapses to one undirected edge A-B.
//
// KEY INVARIANTS:
//   Complete K_n: ecc(v)=1 for all v → D=R=1; ECI = n*(n-1); avg_ecc_ppm=1_000_000
//   Path P_n:    ecc(endpoints)=n-1, ecc(center)=⌈(n-1)/2⌉; D=n-1; R=⌈(n-1)/2⌉
//   Star K_{1,k}: D=2 (leaves), R=1 (center); ECI = k*1 + k*(1*2) = 3k? No: ECI=deg(A)*ecc(A) + Σ_leaf deg*ecc
//                 center: deg=k, ecc=1 → contributes k; each leaf: deg=1, ecc=2 → contributes 2
//                 ECI = k + k*2 = 3k
//   Self-centered graph: D = R (e.g. K_n, C_n)
//   Disconnected graph:  D = max ecc of any node; isolated nodes have ecc=0, so D depends on connected part
//   All isolated:        ECI=0, avg=0, D=0, R=0
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph        ECI  avg_ecc_ppm   D   R   edges  nodes
//  Empty          0            0   0   0       0      0
//  1 node         0            0   0   0       0      1
//  Edge A-B       2    1_000_000   1   1       1      2
//  Path P₃        6    1_666_666   2   1       2      3
//  Triangle K₃    6    1_000_000   1   1       3      3
//  Star K_{1,4}  12    1_800_000   2   1       4      5
//  Path P₄       14    2_500_000   3   2       3      4
//  Complete K₄   12    1_000_000   1   1       6      4
//  2 isolated     0            0   0   0       0      2
//  K_{2,3}       24    2_000_000   2   2       6      5
//
// Derivations:
//   Edge A-B: ecc(A)=ecc(B)=1; ECI=1*1+1*1=2; avg=(1+1)/2=1; D=1,R=1
//   P₃ (A-B-C): ecc(A)=2,ecc(B)=1,ecc(C)=2; deg(A)=deg(C)=1,deg(B)=2
//     ECI=1*2+2*1+1*2=6; avg=(2+1+2)/3=5/3→1_666_666; D=2,R=1
//   K₃: all ecc=1, all deg=2; ECI=2*1*3=6; avg=1→1_000_000; D=1,R=1
//   K_{1,4}: center A deg=4,ecc=1; leaves deg=1,ecc=2
//     ECI=4*1+4*(1*2)=4+8=12; avg=(1+2+2+2+2)/5=9/5=1.8→1_800_000; D=2,R=1
//   P₄ (A-B-C-D): ecc(A)=3,ecc(B)=2,ecc(C)=2,ecc(D)=3; deg(A)=deg(D)=1,deg(B)=deg(C)=2
//     ECI=1*3+2*2+2*2+1*3=14; avg=(3+2+2+3)/4=10/4=2.5→2_500_000; D=3,R=2
//   K₄: all ecc=1,deg=3; ECI=3*1*4=12; avg=1→1_000_000; D=1,R=1
//   2 isolated: both ecc=0; ECI=0; avg=0; D=0,R=0
//   K_{2,3}: all ecc=2 (sides {A,B} and {C,D,E}, max dist=2)
//     deg(A)=deg(B)=3,deg(C)=deg(D)=deg(E)=2
//     ECI=3*2+3*2+2*2+2*2+2*2=6+6+4+4+4=24; avg=2→2_000_000; D=2,R=2
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (2, 1_000_000, 1, 1, 1, 2)
//  4.  Path P₃ = A-B-C                    → (6, 1_666_666, 2, 1, 2, 3)
//  5.  Triangle K₃                        → (6, 1_000_000, 1, 1, 3, 3)
//  6.  Star K_{1,4}                       → (12, 1_800_000, 2, 1, 4, 5)
//  7.  Path P₄ = A-B-C-D                  → (14, 2_500_000, 3, 2, 3, 4)
//  8.  Complete K₄                        → (12, 1_000_000, 1, 1, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check      → (24, 2_000_000, 2, 2, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T8_PLUGIN: PluginId   = PluginId::from_ascii("TOPO_IX8");
const T8_EXEC:   ExecutorId = ExecutorId::from_ascii("t8.exec");

const T8_KEY_A: &str = "t8.alpha";
const T8_KEY_B: &str = "t8.beta";
const T8_KEY_C: &str = "t8.gamma";
const T8_KEY_D: &str = "t8.delta";
const T8_KEY_E: &str = "t8.epsilon";

const T8_ID_A: NodeId = derive_node_id(T8_PLUGIN, T8_KEY_A);
const T8_ID_B: NodeId = derive_node_id(T8_PLUGIN, T8_KEY_B);
const T8_ID_C: NodeId = derive_node_id(T8_PLUGIN, T8_KEY_C);
const T8_ID_D: NodeId = derive_node_id(T8_PLUGIN, T8_KEY_D);
const T8_ID_E: NodeId = derive_node_id(T8_PLUGIN, T8_KEY_E);

// L4=95 namespace for this harness.
const T8_VEC_A: VectorAddress = VectorAddress::new(95, 1, 1, 0);
const T8_VEC_B: VectorAddress = VectorAddress::new(95, 1, 2, 0);
const T8_VEC_C: VectorAddress = VectorAddress::new(95, 1, 3, 0);
const T8_VEC_D: VectorAddress = VectorAddress::new(95, 2, 1, 0);
const T8_VEC_E: VectorAddress = VectorAddress::new(95, 2, 2, 0);

const T8_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T8_PLUGIN,
    name:         "kl-graph-topo8-harness",
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
        executor_id:       T8_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T8_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T8_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
// No nodes. All values are 0.

#[test]
fn test_01_empty() {
    let _g = setup();

    let (eci, avg_ecc, d, r, ec, nc) = gos_runtime::graph_topo_indices8();
    assert_eq!(nc,     0, "empty: node_count=0");
    assert_eq!(ec,     0, "empty: edge_count=0");
    assert_eq!(eci,    0, "empty: ECI=0");
    assert_eq!(avg_ecc, 0, "empty: avg_ecc=0");
    assert_eq!(d,      0, "empty: D=0");
    assert_eq!(r,      0, "empty: R=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// One node with no edges: ecc(v)=0 (no other reachable node).

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T8_VEC_A, T8_KEY_A, T8_ID_A);

    let (eci, avg_ecc, d, r, ec, nc) = gos_runtime::graph_topo_indices8();
    assert_eq!(nc,     1, "single: node_count=1");
    assert_eq!(ec,     0, "single: no edges");
    assert_eq!(eci,    0, "single: ECI=0 (ecc=0)");
    assert_eq!(avg_ecc, 0, "single: avg_ecc=0");
    assert_eq!(d,      0, "single: D=0");
    assert_eq!(r,      0, "single: R=0");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected A-B: ecc(A)=ecc(B)=1; deg(A)=deg(B)=1.
//
// ECI      = 1*1 + 1*1 = 2
// avg_ecc  = (1+1)/2 = 1 → 1_000_000
// D        = 1
// R        = 1

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T8_VEC_A, T8_KEY_A, T8_ID_A);
    add_node(T8_VEC_B, T8_KEY_B, T8_ID_B);
    add_edge(T8_ID_A, T8_ID_B, "ab");

    let (eci, avg_ecc, d, r, ec, nc) = gos_runtime::graph_topo_indices8();
    assert_eq!(nc,      2,         "edge: node_count=2");
    assert_eq!(ec,      1,         "edge: edge_count=1");
    assert_eq!(eci,     2,         "edge: ECI=2");
    assert_eq!(avg_ecc, 1_000_000, "edge: avg_ecc=1.000");
    assert_eq!(d,       1,         "edge: D=1");
    assert_eq!(r,       1,         "edge: R=1");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// ecc(A)=2, ecc(B)=1, ecc(C)=2; deg(A)=deg(C)=1, deg(B)=2.
//
// ECI      = 1*2 + 2*1 + 1*2 = 6
// avg_ecc  = (2+1+2)/3 = 5/3 → floor(5_000_000/3) = 1_666_666
// D        = 2
// R        = 1  (centre node B)

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T8_VEC_A, T8_KEY_A, T8_ID_A);
    add_node(T8_VEC_B, T8_KEY_B, T8_ID_B);
    add_node(T8_VEC_C, T8_KEY_C, T8_ID_C);
    add_edge(T8_ID_A, T8_ID_B, "ab");
    add_edge(T8_ID_B, T8_ID_C, "bc");

    let (eci, avg_ecc, d, r, ec, nc) = gos_runtime::graph_topo_indices8();
    assert_eq!(nc,      3,         "P3: node_count=3");
    assert_eq!(ec,      2,         "P3: edge_count=2");
    assert_eq!(eci,     6,         "P3: ECI=6");
    assert_eq!(avg_ecc, 1_666_666, "P3: avg_ecc=1.666");
    assert_eq!(d,       2,         "P3: D=2");
    assert_eq!(r,       1,         "P3: R=1 (centre)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// All ecc=1 (complete → D=R=1, self-centered). deg=2 for all.
//
// ECI      = 2*1 * 3 = 6
// avg_ecc  = 1 → 1_000_000
// D        = 1
// R        = 1  (self-centered)

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T8_VEC_A, T8_KEY_A, T8_ID_A);
    add_node(T8_VEC_B, T8_KEY_B, T8_ID_B);
    add_node(T8_VEC_C, T8_KEY_C, T8_ID_C);
    add_edge(T8_ID_A, T8_ID_B, "ab");
    add_edge(T8_ID_B, T8_ID_C, "bc");
    add_edge(T8_ID_A, T8_ID_C, "ac");

    let (eci, avg_ecc, d, r, ec, nc) = gos_runtime::graph_topo_indices8();
    assert_eq!(nc,      3,         "K3: node_count=3");
    assert_eq!(ec,      3,         "K3: edge_count=3");
    assert_eq!(eci,     6,         "K3: ECI=6");
    assert_eq!(avg_ecc, 1_000_000, "K3: avg_ecc=1.000 (all ecc=1)");
    assert_eq!(d,       1,         "K3: D=1 (complete)");
    assert_eq!(r,       1,         "K3: R=1 (self-centered)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Center=A (deg=4, ecc=1); leaves=B,C,D,E (deg=1, ecc=2 each).
//
// ECI      = 4*1 + 4*(1*2) = 4 + 8 = 12
// avg_ecc  = (1+2+2+2+2)/5 = 9/5 = 1.8 → 1_800_000
// D        = 2  (leaf-to-leaf max dist)
// R        = 1  (center has min ecc)

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T8_VEC_A, T8_KEY_A, T8_ID_A);
    add_node(T8_VEC_B, T8_KEY_B, T8_ID_B);
    add_node(T8_VEC_C, T8_KEY_C, T8_ID_C);
    add_node(T8_VEC_D, T8_KEY_D, T8_ID_D);
    add_node(T8_VEC_E, T8_KEY_E, T8_ID_E);
    add_edge(T8_ID_A, T8_ID_B, "ab");
    add_edge(T8_ID_A, T8_ID_C, "ac");
    add_edge(T8_ID_A, T8_ID_D, "ad");
    add_edge(T8_ID_A, T8_ID_E, "ae");

    let (eci, avg_ecc, d, r, ec, nc) = gos_runtime::graph_topo_indices8();
    assert_eq!(nc,      5,         "K14: node_count=5");
    assert_eq!(ec,      4,         "K14: edge_count=4");
    assert_eq!(eci,     12,        "K14: ECI=12");
    assert_eq!(avg_ecc, 1_800_000, "K14: avg_ecc=1.800");
    assert_eq!(d,       2,         "K14: D=2");
    assert_eq!(r,       1,         "K14: R=1 (center)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// ecc(A)=3, ecc(B)=2, ecc(C)=2, ecc(D)=3; deg(A)=deg(D)=1, deg(B)=deg(C)=2.
//
// ECI      = 1*3 + 2*2 + 2*2 + 1*3 = 3+4+4+3 = 14
// avg_ecc  = (3+2+2+3)/4 = 10/4 = 2.5 → 2_500_000
// D        = 3
// R        = 2  (inner nodes B and C)

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T8_VEC_A, T8_KEY_A, T8_ID_A);
    add_node(T8_VEC_B, T8_KEY_B, T8_ID_B);
    add_node(T8_VEC_C, T8_KEY_C, T8_ID_C);
    add_node(T8_VEC_D, T8_KEY_D, T8_ID_D);
    add_edge(T8_ID_A, T8_ID_B, "ab");
    add_edge(T8_ID_B, T8_ID_C, "bc");
    add_edge(T8_ID_C, T8_ID_D, "cd");

    let (eci, avg_ecc, d, r, ec, nc) = gos_runtime::graph_topo_indices8();
    assert_eq!(nc,      4,         "P4: node_count=4");
    assert_eq!(ec,      3,         "P4: edge_count=3");
    assert_eq!(eci,     14,        "P4: ECI=14");
    assert_eq!(avg_ecc, 2_500_000, "P4: avg_ecc=2.500");
    assert_eq!(d,       3,         "P4: D=3");
    assert_eq!(r,       2,         "P4: R=2 (inner nodes)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// All 4 nodes connected to each other: all ecc=1, all deg=3 (self-centered).
//
// ECI      = 3*1 * 4 = 12
// avg_ecc  = 1 → 1_000_000
// D        = 1
// R        = 1  (self-centered)

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T8_VEC_A, T8_KEY_A, T8_ID_A);
    add_node(T8_VEC_B, T8_KEY_B, T8_ID_B);
    add_node(T8_VEC_C, T8_KEY_C, T8_ID_C);
    add_node(T8_VEC_D, T8_KEY_D, T8_ID_D);
    add_edge(T8_ID_A, T8_ID_B, "ab");
    add_edge(T8_ID_A, T8_ID_C, "ac");
    add_edge(T8_ID_A, T8_ID_D, "ad");
    add_edge(T8_ID_B, T8_ID_C, "bc");
    add_edge(T8_ID_B, T8_ID_D, "bd");
    add_edge(T8_ID_C, T8_ID_D, "cd");

    let (eci, avg_ecc, d, r, ec, nc) = gos_runtime::graph_topo_indices8();
    assert_eq!(nc,      4,         "K4: node_count=4");
    assert_eq!(ec,      6,         "K4: edge_count=6");
    assert_eq!(eci,     12,        "K4: ECI=12=3*1*4");
    assert_eq!(avg_ecc, 1_000_000, "K4: avg_ecc=1.000");
    assert_eq!(d,       1,         "K4: D=1 (complete)");
    assert_eq!(r,       1,         "K4: R=1 (self-centered)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// A and B with no edges: ecc(A)=ecc(B)=0 (no reachable neighbours).
// ECI=0; avg_ecc=0; D=0; R=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T8_VEC_A, T8_KEY_A, T8_ID_A);
    add_node(T8_VEC_B, T8_KEY_B, T8_ID_B);

    let (eci, avg_ecc, d, r, ec, nc) = gos_runtime::graph_topo_indices8();
    assert_eq!(nc,      2, "isolated: node_count=2");
    assert_eq!(ec,      0, "isolated: edge_count=0");
    assert_eq!(eci,     0, "isolated: ECI=0");
    assert_eq!(avg_ecc, 0, "isolated: avg_ecc=0 (all disconnected)");
    assert_eq!(d,       0, "isolated: D=0 (no reachable pairs)");
    assert_eq!(r,       0, "isolated: R=0 (no connected node)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Sides {A,B} and {C,D,E} with all 6 cross-edges.
// All shortest paths:
//   Same-side A↔B: d=2 (go through any C/D/E)
//   Same-side C↔D, C↔E, D↔E: d=2 (go through A or B)
//   Cross-side: d=1
// All ecc(v) = 2 (max dist for any node is 2, either same-side or cross-side all at d≤2)
//   Verify for A: max(d(A,B)=2, d(A,C)=1, d(A,D)=1, d(A,E)=1) = 2 ✓
//   Verify for C: max(d(C,A)=1, d(C,B)=1, d(C,D)=2, d(C,E)=2) = 2 ✓
//
// deg(A)=deg(B)=3; deg(C)=deg(D)=deg(E)=2
// ECI = 3*2 + 3*2 + 2*2 + 2*2 + 2*2 = 6+6+4+4+4 = 24
// avg_ecc = (2+2+2+2+2)/5 = 2 → 2_000_000
// D = 2; R = 2  (self-centered: all ecc equal)

#[test]
fn test_10_k23_cross_check() {
    let _g = setup();
    // Side A: A, B  Side B: C, D, E
    add_node(T8_VEC_A, T8_KEY_A, T8_ID_A);
    add_node(T8_VEC_B, T8_KEY_B, T8_ID_B);
    add_node(T8_VEC_C, T8_KEY_C, T8_ID_C);
    add_node(T8_VEC_D, T8_KEY_D, T8_ID_D);
    add_node(T8_VEC_E, T8_KEY_E, T8_ID_E);
    // All 6 cross-edges
    add_edge(T8_ID_A, T8_ID_C, "ac");
    add_edge(T8_ID_A, T8_ID_D, "ad");
    add_edge(T8_ID_A, T8_ID_E, "ae");
    add_edge(T8_ID_B, T8_ID_C, "bc");
    add_edge(T8_ID_B, T8_ID_D, "bd");
    add_edge(T8_ID_B, T8_ID_E, "be");

    let (eci, avg_ecc, d, r, ec, nc) = gos_runtime::graph_topo_indices8();
    assert_eq!(nc,      5,         "K23: node_count=5");
    assert_eq!(ec,      6,         "K23: edge_count=6");
    assert_eq!(eci,     24,        "K23: ECI=24");
    assert_eq!(avg_ecc, 2_000_000, "K23: avg_ecc=2.000 (all ecc=2)");
    assert_eq!(d,       2,         "K23: D=2");
    assert_eq!(r,       2,         "K23: R=2 (self-centered)");
}
