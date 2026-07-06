// gos-graph-bcc-harness — V3.05 biconnected components tests
//
// Verifies `gos_runtime::graph_bcc` — iterative Tarjan edge-stack BCC
// decomposition on the undirected projection of the live kernel graph.
//
// A biconnected component (BCC) is a maximal 2-vertex-connected subgraph.
// Articulation points (cut vertices) appear in 2+ BCCs and are returned
// with bcc_id=255.  Each isolated node forms its own singleton BCC.
//
// Directed edges A→B and B→A are treated as one undirected edge A-B.
// Self-loops are excluded from the BCC analysis.
//
// Key invariants tested:
//   - K_n (n≥3) is biconnected → 1 BCC, no APs.
//   - A path of n nodes has n-1 BCCs (one per bridge edge).
//   - Each internal node of a path is an AP (bcc_id=255).
//   - Star K_{1,k}: k BCCs, center is AP.
//   - Two triangles sharing one vertex (hourglass): 2 BCCs, shared vertex AP.
//   - bcc_count invariant: connected graph with no APs → 1 BCC.
//
//  1. Empty graph                              → bcc_count=0, node_count=0.
//  2. Single isolated node                     → bcc_count=1, node in BCC 0.
//  3. Two isolated nodes (no edge)             → bcc_count=2, each in own BCC.
//  4. Single edge A-B                          → bcc_count=1, both in BCC 0, no AP.
//  5. Path A-B-C (two bridges)                 → bcc_count=2, B is AP (255).
//  6. Triangle K₃ (A-B-C)                      → bcc_count=1, all BCC 0, no APs.
//  7. K₄ (complete on 4 nodes)                 → bcc_count=1, all BCC 0, no APs.
//  8. Hourglass (K₃ A-B-C + K₃ C-D-E, C shared) → bcc_count=2, C is AP (255).
//  9. Star K_{1,4} (center B, 4 leaves)        → bcc_count=4, B is AP (255).
// 10. Cross-check: APs from graph_bcc match graph_articulation output.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const BCC_PLUGIN: PluginId   = PluginId::from_ascii("KL_GRAPH_BCC__H");
const BCC_EXEC:   ExecutorId = ExecutorId::from_ascii("bcc.exec");

const BCC_KEY_A: &str = "bcc.a";
const BCC_KEY_B: &str = "bcc.b";
const BCC_KEY_C: &str = "bcc.c";
const BCC_KEY_D: &str = "bcc.d";
const BCC_KEY_E: &str = "bcc.e";

const BCC_ID_A: NodeId = derive_node_id(BCC_PLUGIN, BCC_KEY_A);
const BCC_ID_B: NodeId = derive_node_id(BCC_PLUGIN, BCC_KEY_B);
const BCC_ID_C: NodeId = derive_node_id(BCC_PLUGIN, BCC_KEY_C);
const BCC_ID_D: NodeId = derive_node_id(BCC_PLUGIN, BCC_KEY_D);
const BCC_ID_E: NodeId = derive_node_id(BCC_PLUGIN, BCC_KEY_E);

// L4=81 namespace for this harness.
const BCC_VEC_A: VectorAddress = VectorAddress::new(81, 1, 1, 0);
const BCC_VEC_B: VectorAddress = VectorAddress::new(81, 1, 2, 0);
const BCC_VEC_C: VectorAddress = VectorAddress::new(81, 1, 3, 0);
const BCC_VEC_D: VectorAddress = VectorAddress::new(81, 2, 1, 0);
const BCC_VEC_E: VectorAddress = VectorAddress::new(81, 2, 2, 0);

const BCC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    BCC_PLUGIN,
    name:         "kl-graph-bcc-harness",
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
        executor_id:       BCC_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(BCC_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(BCC_MANIFEST).unwrap();
    g
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn bcc_id_of(
    vecs: &[VectorAddress; 128],
    ids:  &[u8; 128],
    nc:   usize,
    target: VectorAddress,
) -> Option<u8> {
    for i in 0..nc {
        if vecs[i] == target { return Some(ids[i]); }
    }
    None
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (_, _, nc, bcc_count) = gos_runtime::graph_bcc::<128>();
    assert_eq!(nc, 0,        "empty: node_count=0");
    assert_eq!(bcc_count, 0, "empty: bcc_count=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_isolated() {
    let _g = setup();
    add_node(BCC_VEC_A, BCC_KEY_A, BCC_ID_A);

    let (vecs, ids, nc, bcc_count) = gos_runtime::graph_bcc::<128>();
    assert_eq!(nc, 1,        "single node: nc=1");
    assert_eq!(bcc_count, 1, "single node: 1 singleton BCC");
    assert_eq!(bcc_id_of(&vecs, &ids, nc, BCC_VEC_A), Some(0),
               "A is in BCC 0");
}

// ── Test 3: Two isolated nodes ────────────────────────────────────────────────

#[test]
fn test_03_two_isolated() {
    let _g = setup();
    add_node(BCC_VEC_A, BCC_KEY_A, BCC_ID_A);
    add_node(BCC_VEC_B, BCC_KEY_B, BCC_ID_B);

    let (vecs, ids, nc, bcc_count) = gos_runtime::graph_bcc::<128>();
    assert_eq!(nc, 2,        "two isolated: nc=2");
    assert_eq!(bcc_count, 2, "two isolated: each in own singleton BCC");

    let id_a = bcc_id_of(&vecs, &ids, nc, BCC_VEC_A).expect("A missing");
    let id_b = bcc_id_of(&vecs, &ids, nc, BCC_VEC_B).expect("B missing");
    assert_ne!(id_a, 255, "A not AP");
    assert_ne!(id_b, 255, "B not AP");
    assert_ne!(id_a, id_b, "A and B in different BCCs");
}

// ── Test 4: Single edge A-B ───────────────────────────────────────────────────

#[test]
fn test_04_single_edge() {
    let _g = setup();
    add_node(BCC_VEC_A, BCC_KEY_A, BCC_ID_A);
    add_node(BCC_VEC_B, BCC_KEY_B, BCC_ID_B);
    add_edge(BCC_ID_A, BCC_ID_B, "ab");

    let (vecs, ids, nc, bcc_count) = gos_runtime::graph_bcc::<128>();
    assert_eq!(nc, 2,        "single edge: nc=2");
    assert_eq!(bcc_count, 1, "single edge: 1 BCC");

    let id_a = bcc_id_of(&vecs, &ids, nc, BCC_VEC_A).expect("A missing");
    let id_b = bcc_id_of(&vecs, &ids, nc, BCC_VEC_B).expect("B missing");
    assert_eq!(id_a, 0, "A in BCC 0");
    assert_eq!(id_b, 0, "B in BCC 0");
}

// ── Test 5: Path A-B-C ───────────────────────────────────────────────────────

#[test]
fn test_05_path_abc() {
    let _g = setup();
    add_node(BCC_VEC_A, BCC_KEY_A, BCC_ID_A);
    add_node(BCC_VEC_B, BCC_KEY_B, BCC_ID_B);
    add_node(BCC_VEC_C, BCC_KEY_C, BCC_ID_C);
    add_edge(BCC_ID_A, BCC_ID_B, "ab");
    add_edge(BCC_ID_B, BCC_ID_C, "bc");

    let (vecs, ids, nc, bcc_count) = gos_runtime::graph_bcc::<128>();
    assert_eq!(nc, 3,        "path A-B-C: nc=3");
    assert_eq!(bcc_count, 2, "path A-B-C: 2 bridge-BCCs");

    let id_a = bcc_id_of(&vecs, &ids, nc, BCC_VEC_A).expect("A missing");
    let id_b = bcc_id_of(&vecs, &ids, nc, BCC_VEC_B).expect("B missing");
    let id_c = bcc_id_of(&vecs, &ids, nc, BCC_VEC_C).expect("C missing");

    assert_ne!(id_a, 255, "A is not AP");
    assert_eq!(id_b, 255, "B is AP");
    assert_ne!(id_c, 255, "C is not AP");
    assert_ne!(id_a, id_c, "A and C in different BCCs");
}

// ── Test 6: Triangle K₃ ──────────────────────────────────────────────────────

#[test]
fn test_06_triangle_k3() {
    let _g = setup();
    add_node(BCC_VEC_A, BCC_KEY_A, BCC_ID_A);
    add_node(BCC_VEC_B, BCC_KEY_B, BCC_ID_B);
    add_node(BCC_VEC_C, BCC_KEY_C, BCC_ID_C);
    add_edge(BCC_ID_A, BCC_ID_B, "ab");
    add_edge(BCC_ID_B, BCC_ID_C, "bc");
    add_edge(BCC_ID_C, BCC_ID_A, "ca");

    let (_, ids, nc, bcc_count) = gos_runtime::graph_bcc::<128>();
    assert_eq!(nc, 3,        "K3: nc=3");
    assert_eq!(bcc_count, 1, "K3: biconnected → 1 BCC");
    for i in 0..nc {
        assert_ne!(ids[i], 255, "K3 has no APs");
        assert_eq!(ids[i], 0,   "all nodes in BCC 0");
    }
}

// ── Test 7: K₄ ───────────────────────────────────────────────────────────────

#[test]
fn test_07_k4() {
    let _g = setup();
    add_node(BCC_VEC_A, BCC_KEY_A, BCC_ID_A);
    add_node(BCC_VEC_B, BCC_KEY_B, BCC_ID_B);
    add_node(BCC_VEC_C, BCC_KEY_C, BCC_ID_C);
    add_node(BCC_VEC_D, BCC_KEY_D, BCC_ID_D);
    add_edge(BCC_ID_A, BCC_ID_B, "ab");
    add_edge(BCC_ID_B, BCC_ID_C, "bc");
    add_edge(BCC_ID_C, BCC_ID_A, "ca");
    add_edge(BCC_ID_A, BCC_ID_D, "ad");
    add_edge(BCC_ID_B, BCC_ID_D, "bd");
    add_edge(BCC_ID_C, BCC_ID_D, "cd");

    let (_, ids, nc, bcc_count) = gos_runtime::graph_bcc::<128>();
    assert_eq!(nc, 4,        "K4: nc=4");
    assert_eq!(bcc_count, 1, "K4: 3-connected → 1 BCC");
    for i in 0..nc {
        assert_ne!(ids[i], 255, "K4 has no APs");
        assert_eq!(ids[i], 0);
    }
}

// ── Test 8: Hourglass (two triangles sharing one vertex) ─────────────────────

#[test]
fn test_08_hourglass() {
    let _g = setup();
    add_node(BCC_VEC_A, BCC_KEY_A, BCC_ID_A);
    add_node(BCC_VEC_B, BCC_KEY_B, BCC_ID_B);
    add_node(BCC_VEC_C, BCC_KEY_C, BCC_ID_C); // shared AP
    add_node(BCC_VEC_D, BCC_KEY_D, BCC_ID_D);
    add_node(BCC_VEC_E, BCC_KEY_E, BCC_ID_E);
    // Triangle 1: A-B-C
    add_edge(BCC_ID_A, BCC_ID_B, "ab");
    add_edge(BCC_ID_B, BCC_ID_C, "bc");
    add_edge(BCC_ID_C, BCC_ID_A, "ca");
    // Triangle 2: C-D-E
    add_edge(BCC_ID_C, BCC_ID_D, "cd");
    add_edge(BCC_ID_D, BCC_ID_E, "de");
    add_edge(BCC_ID_E, BCC_ID_C, "ec");

    let (vecs, ids, nc, bcc_count) = gos_runtime::graph_bcc::<128>();
    assert_eq!(nc, 5,        "hourglass: nc=5");
    assert_eq!(bcc_count, 2, "hourglass: 2 BCCs (one per triangle)");

    let id_a = bcc_id_of(&vecs, &ids, nc, BCC_VEC_A).expect("A missing");
    let id_b = bcc_id_of(&vecs, &ids, nc, BCC_VEC_B).expect("B missing");
    let id_c = bcc_id_of(&vecs, &ids, nc, BCC_VEC_C).expect("C missing");
    let id_d = bcc_id_of(&vecs, &ids, nc, BCC_VEC_D).expect("D missing");
    let id_e = bcc_id_of(&vecs, &ids, nc, BCC_VEC_E).expect("E missing");

    assert_eq!(id_c, 255, "C is the AP");
    assert_ne!(id_a, 255, "A not AP");
    assert_ne!(id_b, 255, "B not AP");
    assert_ne!(id_d, 255, "D not AP");
    assert_ne!(id_e, 255, "E not AP");
    assert_eq!(id_a, id_b,  "A and B share BCC (triangle 1)");
    assert_eq!(id_d, id_e,  "D and E share BCC (triangle 2)");
    assert_ne!(id_a, id_d,  "two triangles in distinct BCCs");
}

// ── Test 9: Star K_{1,4} ─────────────────────────────────────────────────────

#[test]
fn test_09_star_k14() {
    let _g = setup();
    add_node(BCC_VEC_A, BCC_KEY_A, BCC_ID_A); // leaf
    add_node(BCC_VEC_B, BCC_KEY_B, BCC_ID_B); // center
    add_node(BCC_VEC_C, BCC_KEY_C, BCC_ID_C); // leaf
    add_node(BCC_VEC_D, BCC_KEY_D, BCC_ID_D); // leaf
    add_node(BCC_VEC_E, BCC_KEY_E, BCC_ID_E); // leaf
    add_edge(BCC_ID_A, BCC_ID_B, "ab");
    add_edge(BCC_ID_B, BCC_ID_C, "bc");
    add_edge(BCC_ID_B, BCC_ID_D, "bd");
    add_edge(BCC_ID_B, BCC_ID_E, "be");

    let (vecs, ids, nc, bcc_count) = gos_runtime::graph_bcc::<128>();
    assert_eq!(nc, 5,        "star K_{{1,4}}: nc=5");
    assert_eq!(bcc_count, 4, "star K_{{1,4}}: 4 bridge-BCCs (one per spoke)");

    assert_eq!(bcc_id_of(&vecs, &ids, nc, BCC_VEC_B), Some(255),
               "center B is AP");

    for leaf in [BCC_VEC_A, BCC_VEC_C, BCC_VEC_D, BCC_VEC_E] {
        let id = bcc_id_of(&vecs, &ids, nc, leaf).expect("leaf missing");
        assert_ne!(id, 255, "leaf not AP");
    }

    // All four leaves in distinct BCCs.
    let id_a = bcc_id_of(&vecs, &ids, nc, BCC_VEC_A).unwrap();
    let id_c = bcc_id_of(&vecs, &ids, nc, BCC_VEC_C).unwrap();
    let id_d = bcc_id_of(&vecs, &ids, nc, BCC_VEC_D).unwrap();
    let id_e = bcc_id_of(&vecs, &ids, nc, BCC_VEC_E).unwrap();
    let leaf_ids = [id_a, id_c, id_d, id_e];
    for i in 0..4 {
        for j in (i + 1)..4 {
            assert_ne!(leaf_ids[i], leaf_ids[j], "each spoke is a separate BCC");
        }
    }
}

// ── Test 10: Cross-check BCC APs vs graph_articulation ───────────────────────
// Path A-B-C-D: two APs (B and C), three bridge-BCCs.

#[test]
fn test_10_cross_check_articulation() {
    let _g = setup();
    add_node(BCC_VEC_A, BCC_KEY_A, BCC_ID_A);
    add_node(BCC_VEC_B, BCC_KEY_B, BCC_ID_B);
    add_node(BCC_VEC_C, BCC_KEY_C, BCC_ID_C);
    add_node(BCC_VEC_D, BCC_KEY_D, BCC_ID_D);
    add_edge(BCC_ID_A, BCC_ID_B, "ab");
    add_edge(BCC_ID_B, BCC_ID_C, "bc");
    add_edge(BCC_ID_C, BCC_ID_D, "cd");

    let (bcc_vecs, bcc_ids, nc_bcc, bcc_count) =
        gos_runtime::graph_bcc::<128>();
    let (art_vecs, art_count, _nc_art) =
        gos_runtime::graph_articulation::<128>();

    assert_eq!(bcc_count, 3, "path A-B-C-D: 3 BCCs");

    // Collect APs from graph_bcc (bcc_id=255).
    let mut bcc_aps: Vec<VectorAddress> = (0..nc_bcc)
        .filter(|&i| bcc_ids[i] == 255)
        .map(|i| bcc_vecs[i])
        .collect();
    bcc_aps.sort_by_key(|v| v.as_u64());

    // Collect APs from graph_articulation.
    let mut art_aps: Vec<VectorAddress> =
        (0..art_count).map(|i| art_vecs[i]).collect();
    art_aps.sort_by_key(|v| v.as_u64());

    assert_eq!(bcc_aps, art_aps,
        "graph_bcc APs (255) must match graph_articulation output");

    assert!(bcc_aps.contains(&BCC_VEC_B), "B is AP");
    assert!(bcc_aps.contains(&BCC_VEC_C), "C is AP");
    assert!(!bcc_aps.contains(&BCC_VEC_A), "A not AP");
    assert!(!bcc_aps.contains(&BCC_VEC_D), "D not AP");
}
