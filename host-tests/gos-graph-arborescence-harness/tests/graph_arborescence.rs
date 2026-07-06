// gos-graph-arborescence-harness -- V3.00 minimum spanning arborescence tests
//
// Verifies `gos_runtime::graph_arborescence` — minimum spanning arborescence
// (directed MST) from a root via Chu-Liu / Edmonds 1967.
//
// An arborescence from root r is a directed spanning tree where every non-root
// node v has exactly one directed path from r to v.
//
// Key invariants tested:
//   Connectivity: is_connected=false iff no arborescence exists from root.
//   Weight:       total_w = Σ(arborescence edge weights × 1000).
//   Tree:         nc-1 edges in a spanning arborescence.
//   Optimality:   Chu-Liu/Edmonds selects the minimum-weight incoming edge per
//                 super-node, contracting cycles to find the global optimum.
//
//  1. Empty graph                       → nc=0,  is_connected=true.
//  2. Single node A, root=A             → nc=1,  self-parent,  total=0.
//  3. Single directed edge A→B, root=A  → B parent=A,  total=1000.
//  4. Chain A→B→C→D, root=A            → B←A, C←B, D←C, total=3000.
//  5. Directed 3-cycle A→B→C→A, root=A → A reachable to all; total=2000.
//  6. Cycle contraction test            → cheaper path via contracted cycle.
//  7. Disconnected: D unreachable       → is_connected=false.
//  8. Complete directed triangle K3     → two cheapest incoming edges chosen.
//  9. Star out-tree A→{B,C,D,E}        → trivial arborescence, total=4000.
// 10. Cross-check: arborescence has exactly nc-1 non-root nodes with parent≠self.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const ARB_PLUGIN: PluginId   = PluginId::from_ascii("KL_ARB_HARNESS__");
const ARB_EXEC:   ExecutorId = ExecutorId::from_ascii("arb.exec");

const ARB_KEY_A: &str = "arb.a";
const ARB_KEY_B: &str = "arb.b";
const ARB_KEY_C: &str = "arb.c";
const ARB_KEY_D: &str = "arb.d";
const ARB_KEY_E: &str = "arb.e";

const ARB_ID_A: NodeId = derive_node_id(ARB_PLUGIN, ARB_KEY_A);
const ARB_ID_B: NodeId = derive_node_id(ARB_PLUGIN, ARB_KEY_B);
const ARB_ID_C: NodeId = derive_node_id(ARB_PLUGIN, ARB_KEY_C);
const ARB_ID_D: NodeId = derive_node_id(ARB_PLUGIN, ARB_KEY_D);
const ARB_ID_E: NodeId = derive_node_id(ARB_PLUGIN, ARB_KEY_E);

// L4=76 namespace for this harness.
const ARB_VEC_A: VectorAddress = VectorAddress::new(76, 1, 1, 0);
const ARB_VEC_B: VectorAddress = VectorAddress::new(76, 1, 2, 0);
const ARB_VEC_C: VectorAddress = VectorAddress::new(76, 1, 3, 0);
const ARB_VEC_D: VectorAddress = VectorAddress::new(76, 1, 4, 0);
const ARB_VEC_E: VectorAddress = VectorAddress::new(76, 2, 1, 0);

const ARB_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    ARB_PLUGIN,
    name:         "kl-graph-arborescence-harness",
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
        executor_id:       ARB_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(ARB_PLUGIN, vec, node_spec(key, id)).unwrap();
}

fn add_edge_w(from: NodeId, to: NodeId, key: &'static str, weight: f32) {
    gos_runtime::register_edge(EdgeSpec {
        edge_id:              derive_edge_id(from, to, key),
        from_node:            from,
        to_node:              to,
        edge_type:            RuntimeEdgeType::Signal,
        weight,
        acl_mask:             u64::MAX,
        route_policy:         RoutePolicy::Direct,
        capability_namespace: None,
        capability_binding:   None,
        vector_ref:           None,
    }).unwrap();
}

fn add_edge(from: NodeId, to: NodeId, key: &'static str) {
    add_edge_w(from, to, key, 1.0);
}

fn setup() -> std::sync::MutexGuard<'static, ()> {
    let g = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(ARB_MANIFEST).unwrap();
    g
}

// ── Test 1: empty graph ────────────────────────────────────────────────────────

#[test]
fn test_01_empty_graph() {
    let _g = setup();

    let (_, _, _, nc, total_w, is_conn) =
        gos_runtime::graph_arborescence::<128>(ARB_VEC_A);

    assert_eq!(nc, 0, "empty: nc=0");
    assert_eq!(total_w, 0, "empty: total_w=0");
    assert!(is_conn, "empty: vacuously connected");
}

// ── Test 2: single node ────────────────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(ARB_VEC_A, ARB_KEY_A, ARB_ID_A);

    let (vecs, parents, _, nc, total_w, is_conn) =
        gos_runtime::graph_arborescence::<128>(ARB_VEC_A);

    assert_eq!(nc, 1, "single: nc=1");
    assert_eq!(total_w, 0, "single: total_w=0");
    assert!(is_conn, "single: connected");
    assert_eq!(vecs[0], ARB_VEC_A, "single: vec[0]=A");
    assert_eq!(parents[0], ARB_VEC_A, "single: parent[0]=self");
}

// ── Test 3: single directed edge A→B, root=A ──────────────────────────────────

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(ARB_VEC_A, ARB_KEY_A, ARB_ID_A);
    add_node(ARB_VEC_B, ARB_KEY_B, ARB_ID_B);
    add_edge(ARB_ID_A, ARB_ID_B, "ab");

    let (vecs, parents, weights, nc, total_w, is_conn) =
        gos_runtime::graph_arborescence::<128>(ARB_VEC_A);

    assert_eq!(nc, 2, "edge: nc=2");
    assert!(is_conn, "edge: connected");
    assert_eq!(total_w, 1000, "edge: total_w=1.000");

    assert_eq!(vecs[0], ARB_VEC_A, "edge: root first");
    assert_eq!(parents[0], ARB_VEC_A, "edge: root self-parent");

    let bi = (0..nc).find(|&i| vecs[i] == ARB_VEC_B).expect("B in output");
    assert_eq!(parents[bi], ARB_VEC_A, "edge: B parent=A");
    assert_eq!(weights[bi], 1000, "edge: B weight=1.000");
}

// ── Test 4: chain A→B→C→D, root=A ────────────────────────────────────────────

#[test]
fn test_04_chain() {
    let _g = setup();
    add_node(ARB_VEC_A, ARB_KEY_A, ARB_ID_A);
    add_node(ARB_VEC_B, ARB_KEY_B, ARB_ID_B);
    add_node(ARB_VEC_C, ARB_KEY_C, ARB_ID_C);
    add_node(ARB_VEC_D, ARB_KEY_D, ARB_ID_D);
    add_edge(ARB_ID_A, ARB_ID_B, "ab");
    add_edge(ARB_ID_B, ARB_ID_C, "bc");
    add_edge(ARB_ID_C, ARB_ID_D, "cd");

    let (vecs, parents, _, nc, total_w, is_conn) =
        gos_runtime::graph_arborescence::<128>(ARB_VEC_A);

    assert_eq!(nc, 4, "chain: nc=4");
    assert!(is_conn, "chain: connected");
    assert_eq!(total_w, 3000, "chain: total_w=3.000");

    let find = |v: VectorAddress| (0..nc).find(|&i| vecs[i] == v).unwrap();
    assert_eq!(parents[find(ARB_VEC_A)], ARB_VEC_A, "chain: A self-parent");
    assert_eq!(parents[find(ARB_VEC_B)], ARB_VEC_A, "chain: B\u{2190}A");
    assert_eq!(parents[find(ARB_VEC_C)], ARB_VEC_B, "chain: C\u{2190}B");
    assert_eq!(parents[find(ARB_VEC_D)], ARB_VEC_C, "chain: D\u{2190}C");
}

// ── Test 5: directed 3-cycle A→B→C→A, root=A ─────────────────────────────────

#[test]
fn test_05_directed_3cycle() {
    let _g = setup();
    add_node(ARB_VEC_A, ARB_KEY_A, ARB_ID_A);
    add_node(ARB_VEC_B, ARB_KEY_B, ARB_ID_B);
    add_node(ARB_VEC_C, ARB_KEY_C, ARB_ID_C);
    add_edge(ARB_ID_A, ARB_ID_B, "ab");
    add_edge(ARB_ID_B, ARB_ID_C, "bc");
    add_edge(ARB_ID_C, ARB_ID_A, "ca"); // back-edge

    let (vecs, parents, _, nc, total_w, is_conn) =
        gos_runtime::graph_arborescence::<128>(ARB_VEC_A);

    assert_eq!(nc, 3, "3cycle: nc=3");
    assert!(is_conn, "3cycle: connected from A");
    // Optimal: A(root)→B(1)→C(1); C→A is unused; total=2
    assert_eq!(total_w, 2000, "3cycle: total_w=2.000");

    let find = |v: VectorAddress| (0..nc).find(|&i| vecs[i] == v).unwrap();
    assert_eq!(parents[find(ARB_VEC_A)], ARB_VEC_A, "3cycle: A self-parent");
    assert_eq!(parents[find(ARB_VEC_B)], ARB_VEC_A, "3cycle: B\u{2190}A");
    assert_eq!(parents[find(ARB_VEC_C)], ARB_VEC_B, "3cycle: C\u{2190}B");
}

// ── Test 6: cycle contraction — Chu-Liu/Edmonds beats naive greedy ─────────────
//
// Nodes: A(root), B, C
// Edges: A→B(w=5), A→C(w=3), B→C(w=1), C→B(w=1)
//
// Naive greedy from A: B min-in=A→B(5); C min-in=A→C(3) → total=8.
// Edmonds cycle contraction:
//   Round 0: B min-in=C→B(1); C min-in=B→C(1) → cycle {B,C}
//   Adjust: A→B eff=5-1=4; A→C eff=3-1=2
//   Round 1: super{B,C} min-in=A→C(eff=2, orig=3) → break-point=C
//   Result: A=root, C←A(3), B←C(1); total=4

#[test]
fn test_06_cycle_contraction() {
    let _g = setup();
    add_node(ARB_VEC_A, ARB_KEY_A, ARB_ID_A);
    add_node(ARB_VEC_B, ARB_KEY_B, ARB_ID_B);
    add_node(ARB_VEC_C, ARB_KEY_C, ARB_ID_C);
    add_edge_w(ARB_ID_A, ARB_ID_B, "ab", 5.0);
    add_edge_w(ARB_ID_A, ARB_ID_C, "ac", 3.0);
    add_edge_w(ARB_ID_B, ARB_ID_C, "bc", 1.0);
    add_edge_w(ARB_ID_C, ARB_ID_B, "cb", 1.0);

    let (vecs, parents, weights, nc, total_w, is_conn) =
        gos_runtime::graph_arborescence::<128>(ARB_VEC_A);

    assert_eq!(nc, 3, "contraction: nc=3");
    assert!(is_conn, "contraction: connected");
    // Edmonds optimal: 3+1=4 (not naive 5+1=6 or 5+3=8)
    assert_eq!(total_w, 4000, "contraction: total_w=4.000 (Edmonds beats greedy)");

    let find = |v: VectorAddress| (0..nc).find(|&i| vecs[i] == v).unwrap();
    assert_eq!(parents[find(ARB_VEC_A)], ARB_VEC_A, "contraction: A self-parent");
    assert_eq!(parents[find(ARB_VEC_C)], ARB_VEC_A, "contraction: C\u{2190}A(3)");
    assert_eq!(weights[find(ARB_VEC_C)], 3000, "contraction: C weight=3.000");
    assert_eq!(parents[find(ARB_VEC_B)], ARB_VEC_C, "contraction: B\u{2190}C(1)");
    assert_eq!(weights[find(ARB_VEC_B)], 1000, "contraction: B weight=1.000");
}

// ── Test 7: disconnected — D has no incoming edges ────────────────────────────

#[test]
fn test_07_disconnected() {
    let _g = setup();
    add_node(ARB_VEC_A, ARB_KEY_A, ARB_ID_A);
    add_node(ARB_VEC_B, ARB_KEY_B, ARB_ID_B);
    add_node(ARB_VEC_C, ARB_KEY_C, ARB_ID_C);
    add_node(ARB_VEC_D, ARB_KEY_D, ARB_ID_D);
    add_edge(ARB_ID_A, ARB_ID_B, "ab");
    add_edge(ARB_ID_A, ARB_ID_C, "ac");
    // D has no incoming edges — unreachable from A

    let (_, _, _, nc, total_w, is_conn) =
        gos_runtime::graph_arborescence::<128>(ARB_VEC_A);

    assert_eq!(nc, 4, "disconn: nc=4 (all nodes counted)");
    assert!(!is_conn, "disconn: is_connected=false (D unreachable)");
    assert_eq!(total_w, 0, "disconn: total_w=0 when not connected");
}

// ── Test 8: complete directed triangle K3 (all 6 edges), root=A ───────────────

#[test]
fn test_08_complete_triangle() {
    let _g = setup();
    add_node(ARB_VEC_A, ARB_KEY_A, ARB_ID_A);
    add_node(ARB_VEC_B, ARB_KEY_B, ARB_ID_B);
    add_node(ARB_VEC_C, ARB_KEY_C, ARB_ID_C);
    add_edge(ARB_ID_A, ARB_ID_B, "ab");
    add_edge(ARB_ID_A, ARB_ID_C, "ac");
    add_edge(ARB_ID_B, ARB_ID_A, "ba");
    add_edge(ARB_ID_B, ARB_ID_C, "bc");
    add_edge(ARB_ID_C, ARB_ID_A, "ca");
    add_edge(ARB_ID_C, ARB_ID_B, "cb");

    let (vecs, parents, _, nc, total_w, is_conn) =
        gos_runtime::graph_arborescence::<128>(ARB_VEC_A);

    assert_eq!(nc, 3, "K3: nc=3");
    assert!(is_conn, "K3: connected");
    // Optimal arborescence: 2 edges of weight 1 each → total=2000
    assert_eq!(total_w, 2000, "K3: total_w=2.000 (2 edges)");

    let ia = (0..nc).find(|&i| vecs[i] == ARB_VEC_A).expect("A in output");
    assert_eq!(parents[ia], ARB_VEC_A, "K3: A self-parent");

    // nc-1 = 2 non-root nodes each have a non-self parent
    let child_count = (0..nc).filter(|&i| vecs[i] != parents[i]).count();
    assert_eq!(child_count, 2, "K3: 2 non-root edges");
}

// ── Test 9: star out-tree A→{B,C,D,E}, root=A ────────────────────────────────

#[test]
fn test_09_star_out_tree() {
    let _g = setup();
    add_node(ARB_VEC_A, ARB_KEY_A, ARB_ID_A);
    add_node(ARB_VEC_B, ARB_KEY_B, ARB_ID_B);
    add_node(ARB_VEC_C, ARB_KEY_C, ARB_ID_C);
    add_node(ARB_VEC_D, ARB_KEY_D, ARB_ID_D);
    add_node(ARB_VEC_E, ARB_KEY_E, ARB_ID_E);
    add_edge(ARB_ID_A, ARB_ID_B, "ab");
    add_edge(ARB_ID_A, ARB_ID_C, "ac");
    add_edge(ARB_ID_A, ARB_ID_D, "ad");
    add_edge(ARB_ID_A, ARB_ID_E, "ae");

    let (vecs, parents, _, nc, total_w, is_conn) =
        gos_runtime::graph_arborescence::<128>(ARB_VEC_A);

    assert_eq!(nc, 5, "star: nc=5");
    assert!(is_conn, "star: connected");
    assert_eq!(total_w, 4000, "star: total_w=4.000 (4 unit edges)");

    for i in 0..nc {
        if vecs[i] == ARB_VEC_A {
            assert_eq!(parents[i], ARB_VEC_A, "star: root self-parent");
        } else {
            assert_eq!(parents[i], ARB_VEC_A, "star: every leaf parent=A");
        }
    }
}

// ── Test 10: cross-check — arborescence has exactly nc-1 tree edges ───────────
//
// Diamond + tail: A→B(1), A→C(3), B→D(1), C→D(2), D→E(1)
// Optimal: A→B(1), B→D(1), D→E(1), A→C(3) — total=6
// (D picks B→D over C→D since 1 < 2; C picks A→C since only path from root)

#[test]
fn test_10_edge_count_and_weight_invariant() {
    let _g = setup();
    add_node(ARB_VEC_A, ARB_KEY_A, ARB_ID_A);
    add_node(ARB_VEC_B, ARB_KEY_B, ARB_ID_B);
    add_node(ARB_VEC_C, ARB_KEY_C, ARB_ID_C);
    add_node(ARB_VEC_D, ARB_KEY_D, ARB_ID_D);
    add_node(ARB_VEC_E, ARB_KEY_E, ARB_ID_E);
    add_edge_w(ARB_ID_A, ARB_ID_B, "ab", 1.0);
    add_edge_w(ARB_ID_A, ARB_ID_C, "ac", 3.0);
    add_edge_w(ARB_ID_B, ARB_ID_D, "bd", 1.0);
    add_edge_w(ARB_ID_C, ARB_ID_D, "cd", 2.0);
    add_edge_w(ARB_ID_D, ARB_ID_E, "de", 1.0);

    let (vecs, parents, _, nc, total_w, is_conn) =
        gos_runtime::graph_arborescence::<128>(ARB_VEC_A);

    assert_eq!(nc, 5, "diamond: nc=5");
    assert!(is_conn, "diamond: connected");

    // Exactly one root (self-parent).
    let root_count = (0..nc).filter(|&i| vecs[i] == parents[i]).count();
    assert_eq!(root_count, 1, "diamond: exactly one root");

    // nc-1 non-root edges in the arborescence.
    let child_count = (0..nc).filter(|&i| vecs[i] != parents[i]).count();
    assert_eq!(child_count, nc - 1, "diamond: exactly nc-1 non-root edges");

    // Optimal total: A→B(1) + B→D(1) + D→E(1) + A→C(3) = 6
    assert_eq!(total_w, 6000, "diamond: total_w=6.000 (min-weight MSA)");

    // D's parent should be B (weight 1), not C (weight 2)
    let find = |v: VectorAddress| (0..nc).find(|&i| vecs[i] == v).unwrap();
    assert_eq!(parents[find(ARB_VEC_D)], ARB_VEC_B, "diamond: D\u{2190}B(1) not C(2)");
}
