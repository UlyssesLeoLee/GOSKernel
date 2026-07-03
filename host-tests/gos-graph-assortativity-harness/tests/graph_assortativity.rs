// gos-graph-assortativity-harness — V2.65 degree assortativity coefficient
//
// Verifies `gos_runtime::graph_assortativity` — Newman (2002) degree assortativity.
//
// Measures the tendency of nodes to connect to other nodes of similar degree.
// Uses undirected neighbour count as the degree of each node (same definition as
// graph_clustering / graph_transitivity); iterates each stored directed edge once.
//
// Formula (integer arithmetic, no float):
//   M  = stored directed edge count
//   For each edge (u→v): j = undirected_deg(u), k = undirected_deg(v)
//     S1 += j*k,  T += j+k,  Q += j²+k²
//   Numerator   = 4·M·S1 − T²
//   Denominator = 2·M·Q  − T²
//   r_ppm = Numerator·1_000_000 / Denominator   (clamped to [−1e6, +1e6])
//
// Return value: (assortativity_ppm, edge_count, node_count)
//   +1_000_000 → perfectly assortative (hubs link to hubs only)
//   −1_000_000 → perfectly disassortative (hubs link to leaves only)
//        0     → uncorrelated or undefined (no edges / regular graph)
//
// Key invariants:
//   No edges                            → ppm=0, edges=0
//   Regular graph (all same degree)     → ppm=0 (denominator=0 → undefined)
//   Perfect disassortative: path A→B→C → ppm=−1_000_000 (hub B connects only to deg-1 endpoints)
//   Perfect disassortative: star hub→3  → ppm=−1_000_000
//   Perfect assortative: disjoint cliques of distinct sizes → ppm=+1_000_000
//   Isolated nodes counted in node_count but not in edge sums
//
// Test matrix:
//  1.  Empty graph                      → (0, 0, 0)
//  2.  Nodes but no edges               → ppm=0, edges=0
//  3.  Single directed edge A→B         → ppm=0 (both deg=1, regular → denom=0)
//  4.  Path A→B→C                       → ppm=−1_000_000 (perfectly disassortative)
//  5.  Directed triangle cycle          → ppm=0 (regular, all deg=2)
//  6.  Star hub→B,C,D (3 leaves)        → ppm=−1_000_000 (perfectly disassortative)
//  7.  Two disjoint triangles           → ppm=0 (regular, all deg=2)
//  8.  Disjoint K3-cycle + K2-pair      → ppm=+1_000_000 (perfectly assortative)
//  9.  Edge count returned correctly    → edges=M for a known graph
// 10.  Node count includes isolated     → nodes=N even for isolated nodes

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const AS_PLUGIN: PluginId   = PluginId::from_ascii("KL_ASSORT_0");
const AS_EXEC:   ExecutorId = ExecutorId::from_ascii("assort.exec");

const AS_KEY_A: &str = "as.alpha";
const AS_KEY_B: &str = "as.beta";
const AS_KEY_C: &str = "as.gamma";
const AS_KEY_D: &str = "as.delta";
const AS_KEY_E: &str = "as.epsilon";
const AS_KEY_F: &str = "as.zeta";

const AS_ID_A: NodeId = derive_node_id(AS_PLUGIN, AS_KEY_A);
const AS_ID_B: NodeId = derive_node_id(AS_PLUGIN, AS_KEY_B);
const AS_ID_C: NodeId = derive_node_id(AS_PLUGIN, AS_KEY_C);
const AS_ID_D: NodeId = derive_node_id(AS_PLUGIN, AS_KEY_D);
const AS_ID_E: NodeId = derive_node_id(AS_PLUGIN, AS_KEY_E);
const AS_ID_F: NodeId = derive_node_id(AS_PLUGIN, AS_KEY_F);

const AS_VEC_A: VectorAddress = VectorAddress::new(41, 1, 1, 0);
const AS_VEC_B: VectorAddress = VectorAddress::new(41, 1, 2, 0);
const AS_VEC_C: VectorAddress = VectorAddress::new(41, 1, 3, 0);
const AS_VEC_D: VectorAddress = VectorAddress::new(41, 1, 4, 0);
const AS_VEC_E: VectorAddress = VectorAddress::new(41, 1, 5, 0);
const AS_VEC_F: VectorAddress = VectorAddress::new(41, 1, 6, 0);

const AS_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    AS_PLUGIN,
    name:         "kl-graph-assortativity-harness",
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

fn node_spec(key: &'static str, node_id: NodeId) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       AS_EXEC,
        state_schema_hash: 0xA550,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(AS_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(AS_PLUGIN, vec, node_spec(key, id)).unwrap();
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

fn run_assort() -> (i32, usize, usize) {
    gos_runtime::graph_assortativity()
}

// ── 1. Empty graph ────────────────────────────────────────────────────────────

#[test]
fn empty_graph_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (ppm, edges, nodes) = run_assort();
    assert_eq!(ppm,   0, "empty: ppm must be 0");
    assert_eq!(edges, 0, "empty: edges must be 0");
    assert_eq!(nodes, 0, "empty: nodes must be 0");
}

// ── 2. Nodes but no edges ─────────────────────────────────────────────────────

#[test]
fn nodes_no_edges_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(AS_VEC_A, AS_KEY_A, AS_ID_A);
    add_node(AS_VEC_B, AS_KEY_B, AS_ID_B);

    let (ppm, edges, nodes) = run_assort();
    assert_eq!(ppm,   0, "no-edges: ppm must be 0");
    assert_eq!(edges, 0, "no-edges: edges must be 0");
    assert_eq!(nodes, 2, "no-edges: 2 registered nodes");
}

// ── 3. Single directed edge A→B — both deg=1, denominator=0 → ppm=0 ──────────

#[test]
fn single_edge_regular_undefined() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(AS_VEC_A, AS_KEY_A, AS_ID_A);
    add_node(AS_VEC_B, AS_KEY_B, AS_ID_B);
    add_edge(AS_ID_A, AS_ID_B, "as.ab.t3");

    let (ppm, edges, _nodes) = run_assort();
    // deg(A)=1, deg(B)=1 → all edges (1,1) → denom=4M*S1−T²=4*1*1−4=0 → undefined → 0
    assert_eq!(ppm,   0, "single edge (1,1): regular → ppm=0");
    assert_eq!(edges, 1, "single edge: edges=1");
}

// ── 4. Path A→B→C — perfectly disassortative ─────────────────────────────────
//
// deg(A)=1 (neighbor B only), deg(B)=2 (neighbors A and C), deg(C)=1 (neighbor B only)
// Edges: A→B (1,2), B→C (2,1)
// S1=4, T=6, Q=10, M=2
// numer = 4·2·4 − 36 = −4   denom = 2·2·10 − 36 = 4   r = −1.0

#[test]
fn path_graph_perfectly_disassortative() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(AS_VEC_A, AS_KEY_A, AS_ID_A);
    add_node(AS_VEC_B, AS_KEY_B, AS_ID_B);
    add_node(AS_VEC_C, AS_KEY_C, AS_ID_C);
    add_edge(AS_ID_A, AS_ID_B, "as.ab.t4");
    add_edge(AS_ID_B, AS_ID_C, "as.bc.t4");

    let (ppm, _edges, _nodes) = run_assort();
    assert_eq!(ppm, -1_000_000,
        "path A→B→C: hub B(deg=2) connects only to deg-1 endpoints → r=−1");
}

// ── 5. Directed triangle cycle — regular, all deg=2 → ppm=0 ──────────────────

#[test]
fn triangle_cycle_regular_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(AS_VEC_A, AS_KEY_A, AS_ID_A);
    add_node(AS_VEC_B, AS_KEY_B, AS_ID_B);
    add_node(AS_VEC_C, AS_KEY_C, AS_ID_C);
    // Directed cycle A→B→C→A; each node's undirected deg = 2.
    add_edge(AS_ID_A, AS_ID_B, "as.ab.t5");
    add_edge(AS_ID_B, AS_ID_C, "as.bc.t5");
    add_edge(AS_ID_C, AS_ID_A, "as.ca.t5");

    let (ppm, _edges, _nodes) = run_assort();
    assert_eq!(ppm, 0, "triangle cycle: all deg=2 → regular → ppm=0");
}

// ── 6. Star hub→B,C,D — perfectly disassortative ─────────────────────────────
//
// deg(hub)=3, deg(leaf)=1
// Edges: (3,1),(3,1),(3,1)  S1=9, T=12, Q=30, M=3
// numer = 4·3·9 − 144 = −36   denom = 2·3·30 − 144 = 36   r = −1.0

#[test]
fn star_graph_perfectly_disassortative() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(AS_VEC_A, AS_KEY_A, AS_ID_A); // hub
    add_node(AS_VEC_B, AS_KEY_B, AS_ID_B);
    add_node(AS_VEC_C, AS_KEY_C, AS_ID_C);
    add_node(AS_VEC_D, AS_KEY_D, AS_ID_D);
    add_edge(AS_ID_A, AS_ID_B, "as.ab.t6");
    add_edge(AS_ID_A, AS_ID_C, "as.ac.t6");
    add_edge(AS_ID_A, AS_ID_D, "as.ad.t6");

    let (ppm, _edges, _nodes) = run_assort();
    assert_eq!(ppm, -1_000_000,
        "star: hub(deg=3) connects only to leaves(deg=1) → r=−1");
}

// ── 7. Two disjoint directed triangles — both regular, all deg=2 → ppm=0 ──────

#[test]
fn two_disjoint_triangles_regular_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(AS_VEC_A, AS_KEY_A, AS_ID_A);
    add_node(AS_VEC_B, AS_KEY_B, AS_ID_B);
    add_node(AS_VEC_C, AS_KEY_C, AS_ID_C);
    add_edge(AS_ID_A, AS_ID_B, "as.ab.t7");
    add_edge(AS_ID_B, AS_ID_C, "as.bc.t7");
    add_edge(AS_ID_C, AS_ID_A, "as.ca.t7");

    add_node(AS_VEC_D, AS_KEY_D, AS_ID_D);
    add_node(AS_VEC_E, AS_KEY_E, AS_ID_E);
    add_node(AS_VEC_F, AS_KEY_F, AS_ID_F);
    add_edge(AS_ID_D, AS_ID_E, "as.de.t7");
    add_edge(AS_ID_E, AS_ID_F, "as.ef.t7");
    add_edge(AS_ID_F, AS_ID_D, "as.fd.t7");

    let (ppm, _edges, _nodes) = run_assort();
    assert_eq!(ppm, 0,
        "two disjoint triangles: all deg=2, regular → ppm=0");
}

// ── 8. Disjoint K3-cycle + K2-pair — perfectly assortative ───────────────────
//
// K3 cycle: A→B, B→C, C→A  (each node deg=2)
// K2 pair:  D→E             (each node deg=1)
// All edges connect same-degree nodes: K3 edges (2,2), K2 edge (1,1).
//
// S1 = 3·4 + 1·1 = 13
// T  = 3·4 + 1·2 = 14
// Q  = 3·8 + 1·2 = 26
// M  = 4
// numer = 4·4·13 − 196 = 208 − 196 = 12
// denom = 2·4·26  − 196 = 208 − 196 = 12
// r = 12/12 = +1.0 → ppm = +1_000_000

#[test]
fn disjoint_cliques_perfectly_assortative() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // K3 cycle (all deg=2)
    add_node(AS_VEC_A, AS_KEY_A, AS_ID_A);
    add_node(AS_VEC_B, AS_KEY_B, AS_ID_B);
    add_node(AS_VEC_C, AS_KEY_C, AS_ID_C);
    add_edge(AS_ID_A, AS_ID_B, "as.ab.t8");
    add_edge(AS_ID_B, AS_ID_C, "as.bc.t8");
    add_edge(AS_ID_C, AS_ID_A, "as.ca.t8");
    // K2 pair (all deg=1)
    add_node(AS_VEC_D, AS_KEY_D, AS_ID_D);
    add_node(AS_VEC_E, AS_KEY_E, AS_ID_E);
    add_edge(AS_ID_D, AS_ID_E, "as.de.t8");

    let (ppm, edges, nodes) = run_assort();
    assert_eq!(ppm, 1_000_000,
        "disjoint K3+K2: all edges between same-degree nodes → r=+1");
    assert_eq!(edges, 4, "4 directed edges stored");
    assert_eq!(nodes, 5, "5 nodes registered");
}

// ── 9. Edge count returned correctly ─────────────────────────────────────────

#[test]
fn edge_count_returned_correctly() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // 3-node star with hub + 4 leaves → 4 edges
    add_node(AS_VEC_A, AS_KEY_A, AS_ID_A); // hub
    add_node(AS_VEC_B, AS_KEY_B, AS_ID_B);
    add_node(AS_VEC_C, AS_KEY_C, AS_ID_C);
    add_node(AS_VEC_D, AS_KEY_D, AS_ID_D);
    add_node(AS_VEC_E, AS_KEY_E, AS_ID_E);
    add_edge(AS_ID_A, AS_ID_B, "as.ab.t9");
    add_edge(AS_ID_A, AS_ID_C, "as.ac.t9");
    add_edge(AS_ID_A, AS_ID_D, "as.ad.t9");
    add_edge(AS_ID_A, AS_ID_E, "as.ae.t9");

    let (_ppm, edges, _nodes) = run_assort();
    assert_eq!(edges, 4, "hub-star with 4 leaves → exactly 4 edges");
}

// ── 10. Node count includes isolated nodes ────────────────────────────────────

#[test]
fn node_count_includes_isolated() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Triangle A→B→C→A + two isolated nodes D, E (no edges)
    add_node(AS_VEC_A, AS_KEY_A, AS_ID_A);
    add_node(AS_VEC_B, AS_KEY_B, AS_ID_B);
    add_node(AS_VEC_C, AS_KEY_C, AS_ID_C);
    add_node(AS_VEC_D, AS_KEY_D, AS_ID_D); // isolated
    add_node(AS_VEC_E, AS_KEY_E, AS_ID_E); // isolated
    add_edge(AS_ID_A, AS_ID_B, "as.ab.t10");
    add_edge(AS_ID_B, AS_ID_C, "as.bc.t10");
    add_edge(AS_ID_C, AS_ID_A, "as.ca.t10");

    let (ppm, edges, nodes) = run_assort();
    // Triangle is regular (all deg=2) → ppm=0
    assert_eq!(ppm,   0, "triangle + 2 isolated: regular → ppm=0");
    assert_eq!(edges, 3, "3 edges in triangle");
    assert_eq!(nodes, 5, "5 nodes total, including 2 isolated");
}
