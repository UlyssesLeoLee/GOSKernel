// gos-graph-bimatch-harness -- V2.92 maximum bipartite matching tests
//
// Verifies `gos_runtime::graph_bipartite_match` — Kuhn's iterative DFS
// augmenting-path algorithm on 2-coloured bipartite graphs.
//
//  1. Empty graph              -> match_count=0, is_bipartite=true.
//  2. Single node, no edges    -> match_count=0, is_bipartite=true.
//  3. Not bipartite (triangle) -> is_bipartite=false, match_count=0.
//  4. Single edge A0-B0        -> match_count=1, pair (A0,B0).
//  5. Path A0-B0-A1 (chain)    -> match_count=1 (only one A-B edge usable).
//  6. K_{2,2} complete bipartite -> match_count=2 (perfect matching).
//  7. K_{2,3}: 2 left, 3 right  -> match_count=2 (all left nodes matched).
//  8. Augmenting path needed (swap): A0-B0-A1-B1 structure.
//  9. Disconnected bipartite components -> each component matched independently.
// 10. Cross-check: match_count <= min(|A|, |B|) and matched pairs disjoint.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const BIM_PLUGIN: PluginId   = PluginId::from_ascii("KL_BIMATCH_H_");
const BIM_EXEC:   ExecutorId = ExecutorId::from_ascii("bim.exec");

const BIM_KEY_A0: &str = "bim.a0";
const BIM_KEY_A1: &str = "bim.a1";
const BIM_KEY_A2: &str = "bim.a2";
const BIM_KEY_B0: &str = "bim.b0";
const BIM_KEY_B1: &str = "bim.b1";
const BIM_KEY_B2: &str = "bim.b2";

const BIM_ID_A0: NodeId = derive_node_id(BIM_PLUGIN, BIM_KEY_A0);
const BIM_ID_A1: NodeId = derive_node_id(BIM_PLUGIN, BIM_KEY_A1);
const BIM_ID_A2: NodeId = derive_node_id(BIM_PLUGIN, BIM_KEY_A2);
const BIM_ID_B0: NodeId = derive_node_id(BIM_PLUGIN, BIM_KEY_B0);
const BIM_ID_B1: NodeId = derive_node_id(BIM_PLUGIN, BIM_KEY_B1);
const BIM_ID_B2: NodeId = derive_node_id(BIM_PLUGIN, BIM_KEY_B2);

// L4=68 namespace for this harness.
const BIM_VEC_A0: VectorAddress = VectorAddress::new(68, 1, 1, 0);
const BIM_VEC_A1: VectorAddress = VectorAddress::new(68, 1, 2, 0);
const BIM_VEC_A2: VectorAddress = VectorAddress::new(68, 1, 3, 0);
const BIM_VEC_B0: VectorAddress = VectorAddress::new(68, 2, 1, 0);
const BIM_VEC_B1: VectorAddress = VectorAddress::new(68, 2, 2, 0);
const BIM_VEC_B2: VectorAddress = VectorAddress::new(68, 2, 3, 0);

const BIM_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    BIM_PLUGIN,
    name:         "kl-graph-bimatch-harness",
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
        executor_id:       BIM_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(BIM_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    let g = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(BIM_MANIFEST).unwrap();
    g
}

/// Returns true if the pair (left, right) appears in the output.
fn has_pair(
    left_vecs:  &[VectorAddress],
    right_vecs: &[VectorAddress],
    count:      usize,
    left:       VectorAddress,
    right:      VectorAddress,
) -> bool {
    for i in 0..count {
        if left_vecs[i] == left && right_vecs[i] == right { return true; }
    }
    false
}

// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_01_empty_graph() {
    let _g = setup();
    let (_, _, match_count, is_bipartite, node_count) =
        gos_runtime::graph_bipartite_match::<128>();
    assert!(is_bipartite,    "empty graph is vacuously bipartite");
    assert_eq!(node_count,  0, "empty graph: 0 nodes");
    assert_eq!(match_count, 0, "empty graph: 0 matched pairs");
}

#[test]
fn test_02_single_node_no_edges() {
    let _g = setup();
    add_node(BIM_VEC_A0, BIM_KEY_A0, BIM_ID_A0);
    let (_, _, match_count, is_bipartite, node_count) =
        gos_runtime::graph_bipartite_match::<128>();
    assert!(is_bipartite,   "isolated node is bipartite");
    assert_eq!(node_count,  1);
    assert_eq!(match_count, 0, "single isolated node: no matching possible");
}

#[test]
fn test_03_not_bipartite_triangle() {
    // A0 -> B0 -> A1 -> A0: odd cycle (3-node triangle).
    // The algorithm detects the odd cycle during BFS 2-colouring.
    let _g = setup();
    add_node(BIM_VEC_A0, BIM_KEY_A0, BIM_ID_A0);
    add_node(BIM_VEC_B0, BIM_KEY_B0, BIM_ID_B0);
    add_node(BIM_VEC_A1, BIM_KEY_A1, BIM_ID_A1);
    add_edge(BIM_ID_A0, BIM_ID_B0, "a0b0");
    add_edge(BIM_ID_B0, BIM_ID_A1, "b0a1");
    add_edge(BIM_ID_A1, BIM_ID_A0, "a1a0"); // closes odd cycle
    let (_, _, match_count, is_bipartite, node_count) =
        gos_runtime::graph_bipartite_match::<128>();
    assert!(!is_bipartite,  "triangle is not bipartite");
    assert_eq!(node_count,  3);
    assert_eq!(match_count, 0, "not bipartite: match_count forced to 0");
}

#[test]
fn test_04_single_edge() {
    // A0 -- B0: simplest bipartite graph with one matching.
    let _g = setup();
    add_node(BIM_VEC_A0, BIM_KEY_A0, BIM_ID_A0);
    add_node(BIM_VEC_B0, BIM_KEY_B0, BIM_ID_B0);
    add_edge(BIM_ID_A0, BIM_ID_B0, "a0b0");
    let (left_vecs, right_vecs, match_count, is_bipartite, node_count) =
        gos_runtime::graph_bipartite_match::<128>();
    assert!(is_bipartite,   "A0-B0 is bipartite");
    assert_eq!(node_count,  2);
    assert_eq!(match_count, 1, "one edge -> matching size 1");
    assert!(
        has_pair(&left_vecs, &right_vecs, match_count, BIM_VEC_A0, BIM_VEC_B0)
        || has_pair(&left_vecs, &right_vecs, match_count, BIM_VEC_B0, BIM_VEC_A0),
        "matched pair must be (A0, B0)"
    );
}

#[test]
fn test_05_path_chain() {
    // A0 -- B0 -- A1: chain, only one A-B edge per match possible.
    // A0 and A1 are both side-A; B0 is side-B (or vice versa depending on BFS seed).
    // Either way, maximum matching = 1 because B0 can match at most one partner.
    let _g = setup();
    add_node(BIM_VEC_A0, BIM_KEY_A0, BIM_ID_A0);
    add_node(BIM_VEC_B0, BIM_KEY_B0, BIM_ID_B0);
    add_node(BIM_VEC_A1, BIM_KEY_A1, BIM_ID_A1);
    add_edge(BIM_ID_A0, BIM_ID_B0, "a0b0");
    add_edge(BIM_ID_B0, BIM_ID_A1, "b0a1");
    let (_, _, match_count, is_bipartite, node_count) =
        gos_runtime::graph_bipartite_match::<128>();
    assert!(is_bipartite,   "path chain is bipartite");
    assert_eq!(node_count,  3);
    assert_eq!(match_count, 1, "path A0-B0-A1: max matching = 1 (B0 has only one partner)");
}

#[test]
fn test_06_k22_complete_bipartite() {
    // K_{2,2}: A0,A1 -- B0,B1 (all 4 cross-edges present).
    // Perfect matching: match_count = 2.
    let _g = setup();
    add_node(BIM_VEC_A0, BIM_KEY_A0, BIM_ID_A0);
    add_node(BIM_VEC_A1, BIM_KEY_A1, BIM_ID_A1);
    add_node(BIM_VEC_B0, BIM_KEY_B0, BIM_ID_B0);
    add_node(BIM_VEC_B1, BIM_KEY_B1, BIM_ID_B1);
    add_edge(BIM_ID_A0, BIM_ID_B0, "a0b0");
    add_edge(BIM_ID_A0, BIM_ID_B1, "a0b1");
    add_edge(BIM_ID_A1, BIM_ID_B0, "a1b0");
    add_edge(BIM_ID_A1, BIM_ID_B1, "a1b1");
    let (left_vecs, right_vecs, match_count, is_bipartite, node_count) =
        gos_runtime::graph_bipartite_match::<128>();
    assert!(is_bipartite,   "K_22 is bipartite");
    assert_eq!(node_count,  4);
    assert_eq!(match_count, 2, "K_22 perfect matching: size 2");
    // Matched pairs must be vertex-disjoint.
    let l0 = left_vecs[0];
    let l1 = left_vecs[1];
    let r0 = right_vecs[0];
    let r1 = right_vecs[1];
    assert_ne!(l0, l1, "left nodes must be distinct");
    assert_ne!(r0, r1, "right nodes must be distinct");
}

#[test]
fn test_07_k23_more_right_than_left() {
    // K_{2,3}: A0,A1 on left; B0,B1,B2 on right; all 6 edges present.
    // Maximum matching = 2 (limited by the smaller side).
    let _g = setup();
    add_node(BIM_VEC_A0, BIM_KEY_A0, BIM_ID_A0);
    add_node(BIM_VEC_A1, BIM_KEY_A1, BIM_ID_A1);
    add_node(BIM_VEC_B0, BIM_KEY_B0, BIM_ID_B0);
    add_node(BIM_VEC_B1, BIM_KEY_B1, BIM_ID_B1);
    add_node(BIM_VEC_B2, BIM_KEY_B2, BIM_ID_B2);
    add_edge(BIM_ID_A0, BIM_ID_B0, "a0b0");
    add_edge(BIM_ID_A0, BIM_ID_B1, "a0b1");
    add_edge(BIM_ID_A0, BIM_ID_B2, "a0b2");
    add_edge(BIM_ID_A1, BIM_ID_B0, "a1b0");
    add_edge(BIM_ID_A1, BIM_ID_B1, "a1b1");
    add_edge(BIM_ID_A1, BIM_ID_B2, "a1b2");
    let (left_vecs, right_vecs, match_count, is_bipartite, node_count) =
        gos_runtime::graph_bipartite_match::<128>();
    assert!(is_bipartite,   "K_23 is bipartite");
    assert_eq!(node_count,  5);
    assert_eq!(match_count, 2, "K_23: max matching bounded by |left|=2");
    assert_ne!(left_vecs[0], left_vecs[1],   "left nodes must be distinct");
    assert_ne!(right_vecs[0], right_vecs[1], "right nodes must be distinct");
}

#[test]
fn test_08_augmenting_path_swap() {
    // Classic augmenting-path scenario:
    //   A0 -- B0 (only B-neighbor of A0)
    //   A1 -- B0, A1 -- B1
    // First pass: A0 matches B0.
    // Second pass: A1 must augment — use A0→B0→A1→B1 augmenting path.
    // Result: A0-B0 swapped so A1-B0, A0 takes ... wait, let me think again.
    //
    // Actually: Kuhn processes A0 first → matches A0-B0.
    // Then A1: tries B0 (visited, already matched to A0).
    //   DFS pushes A0, A0 tries B1 → free! augmenting path: A1→B0→A0→B1.
    //   After augment: A1-B0, A0-B1. match_count = 2.
    let _g = setup();
    add_node(BIM_VEC_A0, BIM_KEY_A0, BIM_ID_A0);
    add_node(BIM_VEC_A1, BIM_KEY_A1, BIM_ID_A1);
    add_node(BIM_VEC_B0, BIM_KEY_B0, BIM_ID_B0);
    add_node(BIM_VEC_B1, BIM_KEY_B1, BIM_ID_B1);
    // A0 can only reach B0; A1 can reach B0 and B1.
    add_edge(BIM_ID_A0, BIM_ID_B0, "a0b0");
    add_edge(BIM_ID_A1, BIM_ID_B0, "a1b0");
    add_edge(BIM_ID_A1, BIM_ID_B1, "a1b1");
    let (left_vecs, right_vecs, match_count, is_bipartite, node_count) =
        gos_runtime::graph_bipartite_match::<128>();
    assert!(is_bipartite,   "augmenting-path graph is bipartite");
    assert_eq!(node_count,  4);
    assert_eq!(match_count, 2, "augmenting path allows both A0 and A1 to be matched");
    // After augmentation both A0 and A1 must be matched, right nodes distinct.
    assert_ne!(right_vecs[0], right_vecs[1], "matched right nodes must be distinct");
    let _ = (left_vecs, right_vecs);
}

#[test]
fn test_09_disconnected_bipartite_components() {
    // Component-1: A0 -- B0 (matching size 1).
    // Component-2: A1 -- B1, A2 -- B1 (B1 shared, matching size 1 from this component).
    // Total: match_count = 2.
    let _g = setup();
    add_node(BIM_VEC_A0, BIM_KEY_A0, BIM_ID_A0);
    add_node(BIM_VEC_B0, BIM_KEY_B0, BIM_ID_B0);
    add_node(BIM_VEC_A1, BIM_KEY_A1, BIM_ID_A1);
    add_node(BIM_VEC_A2, BIM_KEY_A2, BIM_ID_A2);
    add_node(BIM_VEC_B1, BIM_KEY_B1, BIM_ID_B1);
    add_edge(BIM_ID_A0, BIM_ID_B0, "a0b0");   // component 1
    add_edge(BIM_ID_A1, BIM_ID_B1, "a1b1");   // component 2
    add_edge(BIM_ID_A2, BIM_ID_B1, "a2b1");   // component 2 (B1 shared)
    let (_, _, match_count, is_bipartite, node_count) =
        gos_runtime::graph_bipartite_match::<128>();
    assert!(is_bipartite,   "disconnected bipartite components");
    assert_eq!(node_count,  5);
    assert_eq!(match_count, 2,
        "comp-1: A0-B0 (1 pair); comp-2: one of A1/A2 matches B1 (1 pair)");
}

#[test]
fn test_10_invariants_size_and_disjoint() {
    // Invariant checks on a K_{3,3} graph:
    //   (a) match_count <= min(|A|, |B|) = 3.
    //   (b) Left nodes in output are pairwise distinct.
    //   (c) Right nodes in output are pairwise distinct.
    //   (d) graph_bipartite() and graph_bipartite_match() agree on is_bipartite.
    let _g = setup();
    add_node(BIM_VEC_A0, BIM_KEY_A0, BIM_ID_A0);
    add_node(BIM_VEC_A1, BIM_KEY_A1, BIM_ID_A1);
    add_node(BIM_VEC_A2, BIM_KEY_A2, BIM_ID_A2);
    add_node(BIM_VEC_B0, BIM_KEY_B0, BIM_ID_B0);
    add_node(BIM_VEC_B1, BIM_KEY_B1, BIM_ID_B1);
    add_node(BIM_VEC_B2, BIM_KEY_B2, BIM_ID_B2);
    // All 9 edges of K_{3,3}.
    add_edge(BIM_ID_A0, BIM_ID_B0, "a0b0");
    add_edge(BIM_ID_A0, BIM_ID_B1, "a0b1");
    add_edge(BIM_ID_A0, BIM_ID_B2, "a0b2");
    add_edge(BIM_ID_A1, BIM_ID_B0, "a1b0");
    add_edge(BIM_ID_A1, BIM_ID_B1, "a1b1");
    add_edge(BIM_ID_A1, BIM_ID_B2, "a1b2");
    add_edge(BIM_ID_A2, BIM_ID_B0, "a2b0");
    add_edge(BIM_ID_A2, BIM_ID_B1, "a2b1");
    add_edge(BIM_ID_A2, BIM_ID_B2, "a2b2");

    let (left_vecs, right_vecs, match_count, is_bipartite_match, node_count) =
        gos_runtime::graph_bipartite_match::<128>();
    let (_, _, _, is_bipartite_check) = gos_runtime::graph_bipartite::<128>();

    assert_eq!(node_count, 6);
    // (d) Both functions must agree on bipartite status.
    assert_eq!(
        is_bipartite_match, is_bipartite_check,
        "graph_bipartite and graph_bipartite_match must agree on is_bipartite"
    );
    assert!(is_bipartite_match, "K_33 is bipartite");
    // (a) perfect matching for K_{3,3}: match_count = 3.
    assert_eq!(match_count, 3, "K_33 perfect matching: size 3");
    // (b) & (c) all output nodes are distinct within their side.
    for i in 0..match_count {
        for j in (i + 1)..match_count {
            assert_ne!(left_vecs[i],  left_vecs[j],  "left nodes[{i}] vs [{j}] must differ");
            assert_ne!(right_vecs[i], right_vecs[j], "right nodes[{i}] vs [{j}] must differ");
        }
    }
}
