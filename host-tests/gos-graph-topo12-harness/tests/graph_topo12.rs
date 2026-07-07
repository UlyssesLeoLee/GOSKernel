// gos-graph-topo12-harness — V3.23 Zagreb eccentricity indices
//
// Verifies `gos_runtime::graph_topo_indices12()`:
//   Returns (m1e, m2e, m3e, edge_count, node_count)
//   - m1e  = M1*(G) = Σ_v ecc(v)²                 (exact u64; Vukičević & Graovac 2010)
//   - m2e  = M2*(G) = Σ_{uv∈E} ecc(u)×ecc(v)      (exact u64; Das et al. 2013)
//   - m3e  = M3*(G) = Σ_{uv∈E} |ecc(u)−ecc(v)|    (exact u64; Farooq & Ali 2021)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// DEFINITIONS:
//   ecc(v) = max{d(v,w) : w reachable from v, w≠v} (0 for isolated/single nodes)
//   M1*(G): sum of squared eccentricities over all nodes
//   M2*(G): sum of eccentricity products over all undirected edges
//   M3*(G): sum of |ecc(u)−ecc(v)| over all undirected edges
//
// KEY INVARIANTS:
//   M3* = 0 iff graph is self-centered (all ecc equal, e.g. Kn, K_{r,s}, even cycles).
//   M1*(Kn) = n (all ecc=1).
//   M2*(Kn) = m = n(n-1)/2 (all ecc=1).
//   For trees: diameter = 2×radius or 2×radius-1 (centre is 1 or 2 nodes).
//   Isolated nodes: ecc=0, contribute 0 to M1*, and no edge contributions.
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph           M1*   M2*   M3*   edges  nodes
//  Empty            0     0     0     0      0
//  1 node           0     0     0     0      1
//  Edge A-B         2     1     0     1      2
//  Path P₃          9     4     2     2      3
//  Triangle K₃      3     3     0     3      3
//  Star K_{1,4}    17     8     4     4      5
//  Path P₄         26    16     2     3      4
//  Complete K₄      4     6     0     6      4
//  Two isolated     0     0     0     0      2
//  K_{2,3}         20    24     0     6      5
//
// Derivations:
//   Edge A-B: ecc(A)=ecc(B)=1. M1*=1+1=2. M2*=1. M3*=0.
//
//   P₃ (A-B-C): ecc(A)=ecc(C)=2; ecc(B)=1.
//     M1*=4+1+4=9. {A,B}: 2×1=2; {B,C}: 1×2=2 → M2*=4.
//     M3*=|2-1|+|1-2|=2.
//
//   K₃: all ecc=1.
//     M1*=3. M2*=3×1=3. M3*=0 (self-centered).
//
//   K_{1,4}: center A ecc=1; each leaf ecc=2.
//     M1*=1²+4×2²=1+16=17. 4 edges {A,leaf}: M2*=4×(1×2)=8. M3*=4×|1-2|=4.
//
//   P₄ (A-B-C-D): ecc(A)=ecc(D)=3; ecc(B)=ecc(C)=2.
//     M1*=9+4+4+9=26. {A,B}:3×2=6; {B,C}:2×2=4; {C,D}:2×3=6 → M2*=16.
//     M3*=|3-2|+|2-2|+|2-3|=2.
//
//   K₄: all ecc=1. M1*=4. M2*=6. M3*=0 (self-centered).
//
//   Two isolated: ecc(A)=ecc(B)=0. M1*=0. No edges → M2*=M3*=0.
//
//   K_{2,3}: left={A,B}, right={C,D,E}. left-right d=1; same-side d=2.
//     ecc(A)=ecc(B)=ecc(C)=ecc(D)=ecc(E)=2 (max dist=2 to same-side node).
//     M1*=5×4=20. 6 cross edges, each ecc=2 on both sides: M2*=6×4=24. M3*=0.
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (2, 1, 0, 1, 2)
//  4.  Path P₃ = A-B-C                    → (9, 4, 2, 2, 3)
//  5.  Triangle K₃                        → (3, 3, 0, 3, 3)
//  6.  Star K_{1,4}                       → (17, 8, 4, 4, 5)
//  7.  Path P₄ = A-B-C-D                  → (26, 16, 2, 3, 4)
//  8.  Complete K₄                        → (4, 6, 0, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check      → (20, 24, 0, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T12_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_12");
const T12_EXEC:   ExecutorId = ExecutorId::from_ascii("t12.exec");

const T12_KEY_A: &str = "t12.alpha";
const T12_KEY_B: &str = "t12.beta";
const T12_KEY_C: &str = "t12.gamma";
const T12_KEY_D: &str = "t12.delta";
const T12_KEY_E: &str = "t12.epsilon";

const T12_ID_A: NodeId = derive_node_id(T12_PLUGIN, T12_KEY_A);
const T12_ID_B: NodeId = derive_node_id(T12_PLUGIN, T12_KEY_B);
const T12_ID_C: NodeId = derive_node_id(T12_PLUGIN, T12_KEY_C);
const T12_ID_D: NodeId = derive_node_id(T12_PLUGIN, T12_KEY_D);
const T12_ID_E: NodeId = derive_node_id(T12_PLUGIN, T12_KEY_E);

// L4=99 namespace for this harness.
const T12_VEC_A: VectorAddress = VectorAddress::new(99, 1, 1, 0);
const T12_VEC_B: VectorAddress = VectorAddress::new(99, 1, 2, 0);
const T12_VEC_C: VectorAddress = VectorAddress::new(99, 1, 3, 0);
const T12_VEC_D: VectorAddress = VectorAddress::new(99, 2, 1, 0);
const T12_VEC_E: VectorAddress = VectorAddress::new(99, 2, 2, 0);

const T12_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T12_PLUGIN,
    name:         "kl-graph-topo12-harness",
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
        executor_id:       T12_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T12_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T12_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
// No nodes. All values are 0.

#[test]
fn test_01_empty() {
    let _g = setup();

    let (m1e, m2e, m3e, ec, nc) = gos_runtime::graph_topo_indices12();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(m1e, 0, "empty: M1*=0");
    assert_eq!(m2e, 0, "empty: M2*=0");
    assert_eq!(m3e, 0, "empty: M3*=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// ecc(v)=0 for isolated node; no edges. All indices are 0.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T12_VEC_A, T12_KEY_A, T12_ID_A);

    let (m1e, m2e, m3e, ec, nc) = gos_runtime::graph_topo_indices12();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(m1e, 0, "single: M1*=0 (ecc=0 for isolated)");
    assert_eq!(m2e, 0, "single: M2*=0");
    assert_eq!(m3e, 0, "single: M3*=0");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected: {A,B}. ecc(A)=ecc(B)=1.
// M1*=1²+1²=2. M2*=1×1=1. M3*=|1-1|=0.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T12_VEC_A, T12_KEY_A, T12_ID_A);
    add_node(T12_VEC_B, T12_KEY_B, T12_ID_B);
    add_edge(T12_ID_A, T12_ID_B, "t12.e.ab");

    let (m1e, m2e, m3e, ec, nc) = gos_runtime::graph_topo_indices12();
    assert_eq!(nc,  2, "edge: node_count=2");
    assert_eq!(ec,  1, "edge: edge_count=1");
    assert_eq!(m1e, 2, "edge: M1*=2 (ecc(A)=ecc(B)=1, 1+1=2)");
    assert_eq!(m2e, 1, "edge: M2*=1 (1×1)");
    assert_eq!(m3e, 0, "edge: M3*=0 (|1-1|=0; self-centered)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// ecc(A)=ecc(C)=2, ecc(B)=1.
// M1*=4+1+4=9. {A,B}:2×1=2; {B,C}:1×2=2 → M2*=4. M3*=|2-1|+|1-2|=2.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T12_VEC_A, T12_KEY_A, T12_ID_A);
    add_node(T12_VEC_B, T12_KEY_B, T12_ID_B);
    add_node(T12_VEC_C, T12_KEY_C, T12_ID_C);
    add_edge(T12_ID_A, T12_ID_B, "t12.e.ab");
    add_edge(T12_ID_B, T12_ID_C, "t12.e.bc");

    let (m1e, m2e, m3e, ec, nc) = gos_runtime::graph_topo_indices12();
    assert_eq!(nc,  3, "p3: node_count=3");
    assert_eq!(ec,  2, "p3: edge_count=2");
    assert_eq!(m1e, 9, "p3: M1*=9 (4+1+4; ecc_ends=2, ecc_mid=1)");
    assert_eq!(m2e, 4, "p3: M2*=4 (2×1+1×2)");
    assert_eq!(m3e, 2, "p3: M3*=2 (|2-1|+|1-2|)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// All ecc=1 (diameter=1, self-centered).
// M1*=3. M2*=3. M3*=0.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T12_VEC_A, T12_KEY_A, T12_ID_A);
    add_node(T12_VEC_B, T12_KEY_B, T12_ID_B);
    add_node(T12_VEC_C, T12_KEY_C, T12_ID_C);
    add_edge(T12_ID_A, T12_ID_B, "t12.e.ab");
    add_edge(T12_ID_B, T12_ID_A, "t12.e.ba");
    add_edge(T12_ID_B, T12_ID_C, "t12.e.bc");
    add_edge(T12_ID_C, T12_ID_B, "t12.e.cb");
    add_edge(T12_ID_A, T12_ID_C, "t12.e.ac");
    add_edge(T12_ID_C, T12_ID_A, "t12.e.ca");

    let (m1e, m2e, m3e, ec, nc) = gos_runtime::graph_topo_indices12();
    assert_eq!(nc,  3, "k3: node_count=3");
    assert_eq!(ec,  3, "k3: edge_count=3");
    assert_eq!(m1e, 3, "k3: M1*=3 (3 nodes × ecc=1; 1²×3=3)");
    assert_eq!(m2e, 3, "k3: M2*=3 (3 edges × 1×1=1)");
    assert_eq!(m3e, 0, "k3: M3*=0 (self-centered: all ecc=1)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Center A: ecc=1 (all leaves at d=1).
// Each leaf B,C,D,E: ecc=2 (other leaves at d=2 through center).
// M1*=1²+4×2²=1+16=17.
// M2*=4 edges {A,leaf}: 4×(1×2)=8.
// M3*=4×|1-2|=4.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T12_VEC_A, T12_KEY_A, T12_ID_A);
    add_node(T12_VEC_B, T12_KEY_B, T12_ID_B);
    add_node(T12_VEC_C, T12_KEY_C, T12_ID_C);
    add_node(T12_VEC_D, T12_KEY_D, T12_ID_D);
    add_node(T12_VEC_E, T12_KEY_E, T12_ID_E);
    add_edge(T12_ID_A, T12_ID_B, "t12.e.ab");
    add_edge(T12_ID_A, T12_ID_C, "t12.e.ac");
    add_edge(T12_ID_A, T12_ID_D, "t12.e.ad");
    add_edge(T12_ID_A, T12_ID_E, "t12.e.ae");

    let (m1e, m2e, m3e, ec, nc) = gos_runtime::graph_topo_indices12();
    assert_eq!(nc,  5,  "star: node_count=5");
    assert_eq!(ec,  4,  "star: edge_count=4");
    assert_eq!(m1e, 17, "star: M1*=17 (1+16; center ecc=1, leaf ecc=2)");
    assert_eq!(m2e, 8,  "star: M2*=8 (4×(1×2))");
    assert_eq!(m3e, 4,  "star: M3*=4 (4×|1-2|=4; center vs leaf asymmetry)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// ecc(A)=ecc(D)=3; ecc(B)=ecc(C)=2.
// M1*=9+4+4+9=26.
// {A,B}:3×2=6; {B,C}:2×2=4; {C,D}:2×3=6 → M2*=16.
// M3*=|3-2|+|2-2|+|2-3|=1+0+1=2.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T12_VEC_A, T12_KEY_A, T12_ID_A);
    add_node(T12_VEC_B, T12_KEY_B, T12_ID_B);
    add_node(T12_VEC_C, T12_KEY_C, T12_ID_C);
    add_node(T12_VEC_D, T12_KEY_D, T12_ID_D);
    add_edge(T12_ID_A, T12_ID_B, "t12.e.ab");
    add_edge(T12_ID_B, T12_ID_C, "t12.e.bc");
    add_edge(T12_ID_C, T12_ID_D, "t12.e.cd");

    let (m1e, m2e, m3e, ec, nc) = gos_runtime::graph_topo_indices12();
    assert_eq!(nc,  4,  "p4: node_count=4");
    assert_eq!(ec,  3,  "p4: edge_count=3");
    assert_eq!(m1e, 26, "p4: M1*=26 (9+4+4+9; end ecc=3, mid ecc=2)");
    assert_eq!(m2e, 16, "p4: M2*=16 (6+4+6)");
    assert_eq!(m3e, 2,  "p4: M3*=2 (|3-2|+0+|2-3|=2)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// All ecc=1 (diameter=1, self-centered).
// M1*=4. M2*=6. M3*=0.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T12_VEC_A, T12_KEY_A, T12_ID_A);
    add_node(T12_VEC_B, T12_KEY_B, T12_ID_B);
    add_node(T12_VEC_C, T12_KEY_C, T12_ID_C);
    add_node(T12_VEC_D, T12_KEY_D, T12_ID_D);
    add_edge(T12_ID_A, T12_ID_B, "t12.e.ab");
    add_edge(T12_ID_B, T12_ID_A, "t12.e.ba");
    add_edge(T12_ID_A, T12_ID_C, "t12.e.ac");
    add_edge(T12_ID_C, T12_ID_A, "t12.e.ca");
    add_edge(T12_ID_A, T12_ID_D, "t12.e.ad");
    add_edge(T12_ID_D, T12_ID_A, "t12.e.da");
    add_edge(T12_ID_B, T12_ID_C, "t12.e.bc");
    add_edge(T12_ID_C, T12_ID_B, "t12.e.cb");
    add_edge(T12_ID_B, T12_ID_D, "t12.e.bd");
    add_edge(T12_ID_D, T12_ID_B, "t12.e.db");
    add_edge(T12_ID_C, T12_ID_D, "t12.e.cd");
    add_edge(T12_ID_D, T12_ID_C, "t12.e.dc");

    let (m1e, m2e, m3e, ec, nc) = gos_runtime::graph_topo_indices12();
    assert_eq!(nc,  4, "k4: node_count=4");
    assert_eq!(ec,  6, "k4: edge_count=6");
    assert_eq!(m1e, 4, "k4: M1*=4 (4 nodes × ecc=1; M1*(Kn)=n)");
    assert_eq!(m2e, 6, "k4: M2*=6 (6 edges × 1×1; M2*(Kn)=m)");
    assert_eq!(m3e, 0, "k4: M3*=0 (self-centered: all ecc=1)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// n=2, m=0. ecc(A)=ecc(B)=0. All indices are 0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T12_VEC_A, T12_KEY_A, T12_ID_A);
    add_node(T12_VEC_B, T12_KEY_B, T12_ID_B);

    let (m1e, m2e, m3e, ec, nc) = gos_runtime::graph_topo_indices12();
    assert_eq!(nc,  2, "isolated: node_count=2");
    assert_eq!(ec,  0, "isolated: no edges");
    assert_eq!(m1e, 0, "isolated: M1*=0 (ecc=0 for both)");
    assert_eq!(m2e, 0, "isolated: M2*=0");
    assert_eq!(m3e, 0, "isolated: M3*=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}(deg=3), right={C,D,E}(deg=2). Cross edges d=1; same-side d=2.
// ecc(A)=ecc(B)=ecc(C)=ecc(D)=ecc(E)=2 (max dist=2 to same-side node).
// M1*=5×4=20. M2*=6×(2×2)=24. M3*=0 (self-centered: all ecc=2).
// INVARIANT: M3*=0 confirms K_{2,3} is self-centered.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T12_VEC_A, T12_KEY_A, T12_ID_A);
    add_node(T12_VEC_B, T12_KEY_B, T12_ID_B);
    add_node(T12_VEC_C, T12_KEY_C, T12_ID_C);
    add_node(T12_VEC_D, T12_KEY_D, T12_ID_D);
    add_node(T12_VEC_E, T12_KEY_E, T12_ID_E);
    // Left A connects to all right
    add_edge(T12_ID_A, T12_ID_C, "t12.e.ac");
    add_edge(T12_ID_C, T12_ID_A, "t12.e.ca");
    add_edge(T12_ID_A, T12_ID_D, "t12.e.ad");
    add_edge(T12_ID_D, T12_ID_A, "t12.e.da");
    add_edge(T12_ID_A, T12_ID_E, "t12.e.ae");
    add_edge(T12_ID_E, T12_ID_A, "t12.e.ea");
    // Left B connects to all right
    add_edge(T12_ID_B, T12_ID_C, "t12.e.bc");
    add_edge(T12_ID_C, T12_ID_B, "t12.e.cb");
    add_edge(T12_ID_B, T12_ID_D, "t12.e.bd");
    add_edge(T12_ID_D, T12_ID_B, "t12.e.db");
    add_edge(T12_ID_B, T12_ID_E, "t12.e.be");
    add_edge(T12_ID_E, T12_ID_B, "t12.e.eb");

    let (m1e, m2e, m3e, ec, nc) = gos_runtime::graph_topo_indices12();
    assert_eq!(nc,  5,  "k23: node_count=5");
    assert_eq!(ec,  6,  "k23: edge_count=6");
    assert_eq!(m1e, 20, "k23: M1*=20 (5 nodes × ecc²=4; all ecc=2)");
    assert_eq!(m2e, 24, "k23: M2*=24 (6 edges × 2×2=4)");
    assert_eq!(m3e, 0,  "k23: M3*=0 (self-centered: all ecc=2 confirms K_{{2,3}} self-centered)");
}
