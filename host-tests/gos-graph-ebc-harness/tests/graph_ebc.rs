// gos-graph-ebc-harness — V3.06 edge betweenness centrality tests
//
// Verifies `gos_runtime::graph_betweenness_edge` — Brandes (2001) edge
// betweenness centrality on the directed kernel graph.
//
// Edge betweenness score[e] = number of ordered (s, t) pairs whose unique
// shortest path traverses edge e.  Edges with equal betweenness are tied.
//
// Key invariants tested:
//   - Empty / edge-less graph → 0 edges in output.
//   - Single edge A→B → score=1 (only pair (A,B) uses it).
//   - Path A→B→C → both edges score=2 (pairs (A,B), (A,C) via A→B; pairs
//       (B,C), (A,C) via B→C — but in directed path only (A,B) uses A→B
//       and (A,B+C), so both score=2).
//   - Path A→B→C→D → middle edge B→C has score=4 (highest), ends score=3.
//   - Diamond A→{B,C}→D → all 4 edges score=1 by symmetry (two equal
//       shortest paths from A to D split evenly via Brandes).
//   - Directed triangle A→B, B→C, A→C → all 3 edges score=1.
//   - Out-star A→{B,C,D} → all 3 edges score=1.
//   - Disconnected graph A→B ∥ C→D → both edges score=1.
//   - Output sorted descending by score; max-score edge is first.
//
//  1. Empty graph                             → edge_count=0.
//  2. Single node, no edges                   → edge_count=0.
//  3. Single directed edge A→B               → 1 edge, score=1.
//  4. Path A→B→C                             → 2 edges, both score=2.
//  5. Path A→B→C→D                           → 3 edges; B→C score=4, others=3.
//  6. Diamond A→B, A→C, B→D, C→D            → 4 edges, all score=1.
//  7. Directed triangle A→B, B→C, A→C        → 3 edges, all score=1.
//  8. Out-star A→B, A→C, A→D                 → 3 edges, all score=1.
//  9. Disconnected A→B and C→D               → 2 edges, both score=1.
// 10. Max-score edge in path A→B→C→D is B→C (score=4, first in output).

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const EBC_PLUGIN: PluginId   = PluginId::from_ascii("KL_GRAPH_EBC__H");
const EBC_EXEC:   ExecutorId = ExecutorId::from_ascii("ebc.exec");

const EBC_KEY_A: &str = "ebc.a";
const EBC_KEY_B: &str = "ebc.b";
const EBC_KEY_C: &str = "ebc.c";
const EBC_KEY_D: &str = "ebc.d";

const EBC_ID_A: NodeId = derive_node_id(EBC_PLUGIN, EBC_KEY_A);
const EBC_ID_B: NodeId = derive_node_id(EBC_PLUGIN, EBC_KEY_B);
const EBC_ID_C: NodeId = derive_node_id(EBC_PLUGIN, EBC_KEY_C);
const EBC_ID_D: NodeId = derive_node_id(EBC_PLUGIN, EBC_KEY_D);

// L4=82 namespace for this harness.
const EBC_VEC_A: VectorAddress = VectorAddress::new(82, 1, 1, 0);
const EBC_VEC_B: VectorAddress = VectorAddress::new(82, 1, 2, 0);
const EBC_VEC_C: VectorAddress = VectorAddress::new(82, 1, 3, 0);
const EBC_VEC_D: VectorAddress = VectorAddress::new(82, 2, 1, 0);

const EBC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    EBC_PLUGIN,
    name:         "kl-graph-ebc-harness",
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
        executor_id:       EBC_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(EBC_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(EBC_MANIFEST).unwrap();
    g
}

// Find score for directed edge from→to in the output arrays.
fn score_of(
    from_vecs: &[VectorAddress; 512],
    to_vecs:   &[VectorAddress; 512],
    scores:    &[u32; 512],
    ec:        usize,
    from:      VectorAddress,
    to:        VectorAddress,
) -> Option<u32> {
    for i in 0..ec {
        if from_vecs[i] == from && to_vecs[i] == to {
            return Some(scores[i]);
        }
    }
    None
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (_, _, _, ec) = gos_runtime::graph_betweenness_edge::<512>();
    assert_eq!(ec, 0, "empty graph: 0 edges");
}

// ── Test 2: Single isolated node (no edges) ───────────────────────────────────

#[test]
fn test_02_single_node_no_edges() {
    let _g = setup();
    add_node(EBC_VEC_A, EBC_KEY_A, EBC_ID_A);

    let (_, _, _, ec) = gos_runtime::graph_betweenness_edge::<512>();
    assert_eq!(ec, 0, "isolated node: 0 edges");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(EBC_VEC_A, EBC_KEY_A, EBC_ID_A);
    add_node(EBC_VEC_B, EBC_KEY_B, EBC_ID_B);
    add_edge(EBC_ID_A, EBC_ID_B, "ab");

    let (from_vecs, to_vecs, scores, ec) =
        gos_runtime::graph_betweenness_edge::<512>();
    assert_eq!(ec, 1, "single edge: 1 directed edge");

    let s = score_of(&from_vecs, &to_vecs, &scores, ec, EBC_VEC_A, EBC_VEC_B)
        .expect("A→B missing");
    assert_eq!(s, 1, "A→B score=1 (only pair (A,B))");
}

// ── Test 4: Path A→B→C ───────────────────────────────────────────────────────

#[test]
fn test_04_path_abc() {
    let _g = setup();
    add_node(EBC_VEC_A, EBC_KEY_A, EBC_ID_A);
    add_node(EBC_VEC_B, EBC_KEY_B, EBC_ID_B);
    add_node(EBC_VEC_C, EBC_KEY_C, EBC_ID_C);
    add_edge(EBC_ID_A, EBC_ID_B, "ab");
    add_edge(EBC_ID_B, EBC_ID_C, "bc");

    let (from_vecs, to_vecs, scores, ec) =
        gos_runtime::graph_betweenness_edge::<512>();
    assert_eq!(ec, 2, "path A→B→C: 2 edges");

    let s_ab = score_of(&from_vecs, &to_vecs, &scores, ec, EBC_VEC_A, EBC_VEC_B)
        .expect("A→B missing");
    let s_bc = score_of(&from_vecs, &to_vecs, &scores, ec, EBC_VEC_B, EBC_VEC_C)
        .expect("B→C missing");

    // A→B carries pairs (A,B) and (A,C) = 2 paths.
    // B→C carries pairs (B,C) and (A,C) = 2 paths.
    assert_eq!(s_ab, 2, "A→B score=2");
    assert_eq!(s_bc, 2, "B→C score=2");
}

// ── Test 5: Path A→B→C→D — middle edge has highest betweenness ───────────────

#[test]
fn test_05_path_abcd() {
    let _g = setup();
    add_node(EBC_VEC_A, EBC_KEY_A, EBC_ID_A);
    add_node(EBC_VEC_B, EBC_KEY_B, EBC_ID_B);
    add_node(EBC_VEC_C, EBC_KEY_C, EBC_ID_C);
    add_node(EBC_VEC_D, EBC_KEY_D, EBC_ID_D);
    add_edge(EBC_ID_A, EBC_ID_B, "ab");
    add_edge(EBC_ID_B, EBC_ID_C, "bc");
    add_edge(EBC_ID_C, EBC_ID_D, "cd");

    let (from_vecs, to_vecs, scores, ec) =
        gos_runtime::graph_betweenness_edge::<512>();
    assert_eq!(ec, 3, "path A→B→C→D: 3 edges");

    let s_ab = score_of(&from_vecs, &to_vecs, &scores, ec, EBC_VEC_A, EBC_VEC_B)
        .expect("A→B missing");
    let s_bc = score_of(&from_vecs, &to_vecs, &scores, ec, EBC_VEC_B, EBC_VEC_C)
        .expect("B→C missing");
    let s_cd = score_of(&from_vecs, &to_vecs, &scores, ec, EBC_VEC_C, EBC_VEC_D)
        .expect("C→D missing");

    // A→B carries: (A,B), (A,C), (A,D) = 3.
    // B→C carries: (B,C), (B,D), (A,C), (A,D) = 4.
    // C→D carries: (C,D), (B,D), (A,D) = 3.
    assert_eq!(s_ab, 3, "A→B score=3");
    assert_eq!(s_bc, 4, "B→C score=4 (highest — middle edge)");
    assert_eq!(s_cd, 3, "C→D score=3");
    assert!(s_bc > s_ab, "middle edge outranks endpoint edges");
}

// ── Test 6: Diamond A→B, A→C, B→D, C→D — symmetric, all edges score=1 ───────

#[test]
fn test_06_diamond() {
    let _g = setup();
    add_node(EBC_VEC_A, EBC_KEY_A, EBC_ID_A);
    add_node(EBC_VEC_B, EBC_KEY_B, EBC_ID_B);
    add_node(EBC_VEC_C, EBC_KEY_C, EBC_ID_C);
    add_node(EBC_VEC_D, EBC_KEY_D, EBC_ID_D);
    add_edge(EBC_ID_A, EBC_ID_B, "ab");
    add_edge(EBC_ID_A, EBC_ID_C, "ac");
    add_edge(EBC_ID_B, EBC_ID_D, "bd");
    add_edge(EBC_ID_C, EBC_ID_D, "cd");

    let (from_vecs, to_vecs, scores, ec) =
        gos_runtime::graph_betweenness_edge::<512>();
    assert_eq!(ec, 4, "diamond: 4 edges");

    // Two equal-length paths A→B→D and A→C→D split evenly (Brandes fraction).
    // After integer truncation of 1.5, each edge scores 1.
    for (f, t) in [
        (EBC_VEC_A, EBC_VEC_B),
        (EBC_VEC_A, EBC_VEC_C),
        (EBC_VEC_B, EBC_VEC_D),
        (EBC_VEC_C, EBC_VEC_D),
    ] {
        let s = score_of(&from_vecs, &to_vecs, &scores, ec, f, t)
            .expect("diamond edge missing");
        assert_eq!(s, 1, "diamond: each edge score=1 (symmetric split)");
    }
}

// ── Test 7: Directed triangle A→B, B→C, A→C ─────────────────────────────────

#[test]
fn test_07_directed_triangle() {
    let _g = setup();
    add_node(EBC_VEC_A, EBC_KEY_A, EBC_ID_A);
    add_node(EBC_VEC_B, EBC_KEY_B, EBC_ID_B);
    add_node(EBC_VEC_C, EBC_KEY_C, EBC_ID_C);
    add_edge(EBC_ID_A, EBC_ID_B, "ab");
    add_edge(EBC_ID_B, EBC_ID_C, "bc");
    add_edge(EBC_ID_A, EBC_ID_C, "ac");

    let (from_vecs, to_vecs, scores, ec) =
        gos_runtime::graph_betweenness_edge::<512>();
    assert_eq!(ec, 3, "directed triangle: 3 edges");

    // A→C is shorter than A→B→C (dist 1 vs 2), so B→C only carries (B,C).
    // A→B carries (A,B), A→C carries (A,C), B→C carries (B,C) → all score=1.
    for (f, t) in [
        (EBC_VEC_A, EBC_VEC_B),
        (EBC_VEC_B, EBC_VEC_C),
        (EBC_VEC_A, EBC_VEC_C),
    ] {
        let s = score_of(&from_vecs, &to_vecs, &scores, ec, f, t)
            .expect("triangle edge missing");
        assert_eq!(s, 1, "directed triangle: each edge score=1");
    }
}

// ── Test 8: Out-star A→{B,C,D} ───────────────────────────────────────────────

#[test]
fn test_08_out_star() {
    let _g = setup();
    add_node(EBC_VEC_A, EBC_KEY_A, EBC_ID_A);
    add_node(EBC_VEC_B, EBC_KEY_B, EBC_ID_B);
    add_node(EBC_VEC_C, EBC_KEY_C, EBC_ID_C);
    add_node(EBC_VEC_D, EBC_KEY_D, EBC_ID_D);
    add_edge(EBC_ID_A, EBC_ID_B, "ab");
    add_edge(EBC_ID_A, EBC_ID_C, "ac");
    add_edge(EBC_ID_A, EBC_ID_D, "ad");

    let (from_vecs, to_vecs, scores, ec) =
        gos_runtime::graph_betweenness_edge::<512>();
    assert_eq!(ec, 3, "out-star: 3 edges");

    // Each spoke carries exactly one (A, leaf) path-pair.
    for (f, t) in [
        (EBC_VEC_A, EBC_VEC_B),
        (EBC_VEC_A, EBC_VEC_C),
        (EBC_VEC_A, EBC_VEC_D),
    ] {
        let s = score_of(&from_vecs, &to_vecs, &scores, ec, f, t)
            .expect("star edge missing");
        assert_eq!(s, 1, "out-star: each spoke score=1");
    }
}

// ── Test 9: Two disconnected edges A→B ∥ C→D ─────────────────────────────────

#[test]
fn test_09_disconnected() {
    let _g = setup();
    add_node(EBC_VEC_A, EBC_KEY_A, EBC_ID_A);
    add_node(EBC_VEC_B, EBC_KEY_B, EBC_ID_B);
    add_node(EBC_VEC_C, EBC_KEY_C, EBC_ID_C);
    add_node(EBC_VEC_D, EBC_KEY_D, EBC_ID_D);
    add_edge(EBC_ID_A, EBC_ID_B, "ab");
    add_edge(EBC_ID_C, EBC_ID_D, "cd");

    let (from_vecs, to_vecs, scores, ec) =
        gos_runtime::graph_betweenness_edge::<512>();
    assert_eq!(ec, 2, "disconnected: 2 edges");

    let s_ab = score_of(&from_vecs, &to_vecs, &scores, ec, EBC_VEC_A, EBC_VEC_B)
        .expect("A→B missing");
    let s_cd = score_of(&from_vecs, &to_vecs, &scores, ec, EBC_VEC_C, EBC_VEC_D)
        .expect("C→D missing");

    assert_eq!(s_ab, 1, "A→B score=1 (isolated component)");
    assert_eq!(s_cd, 1, "C→D score=1 (isolated component)");
}

// ── Test 10: Max-score edge is first in output (path A→B→C→D) ────────────────

#[test]
fn test_10_max_score_first() {
    let _g = setup();
    add_node(EBC_VEC_A, EBC_KEY_A, EBC_ID_A);
    add_node(EBC_VEC_B, EBC_KEY_B, EBC_ID_B);
    add_node(EBC_VEC_C, EBC_KEY_C, EBC_ID_C);
    add_node(EBC_VEC_D, EBC_KEY_D, EBC_ID_D);
    add_edge(EBC_ID_A, EBC_ID_B, "ab");
    add_edge(EBC_ID_B, EBC_ID_C, "bc");
    add_edge(EBC_ID_C, EBC_ID_D, "cd");

    let (from_vecs, to_vecs, scores, ec) =
        gos_runtime::graph_betweenness_edge::<512>();
    assert_eq!(ec, 3, "path A→B→C→D: 3 edges");

    // Output is sorted descending by score; B→C (score=4) must be first.
    assert_eq!(scores[0], 4,        "first output score=4 (B→C)");
    assert_eq!(from_vecs[0], EBC_VEC_B, "first edge from=B");
    assert_eq!(to_vecs[0],   EBC_VEC_C, "first edge to=C");

    // All scores are in non-increasing order.
    for i in 1..ec {
        assert!(
            scores[i] <= scores[i - 1],
            "output must be sorted descending: scores[{}]={} > scores[{}]={}",
            i, scores[i], i - 1, scores[i - 1]
        );
    }
}
