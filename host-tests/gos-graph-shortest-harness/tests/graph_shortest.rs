// gos-graph-shortest-harness — V2.49 Dijkstra single-source shortest paths
//
// Verifies `gos_runtime::graph_shortest` — Dijkstra's algorithm over the
// **directed** GOS kernel graph from a given source VectorAddress.
//
// Algorithm summary (Dijkstra):
//   1. Treat edges as directed (use out-edges only from each settled node).
//   2. source node gets distance 0; all others start at ∞.
//   3. Greedily settle the unvisited node with minimum distance, then relax
//      its directed out-edges.
//   4. Nodes with no directed path from source remain at distance u32::MAX.
//
// Return value: (vecs, parents, distances, node_count)
//   vecs[i]      — all live nodes (source first)
//   parents[i]   — parent in shortest-path tree (self for source,
//                  ZERO_VEC for unreachable)
//   distances[i] — distance from source × 1000 as u32 (u32::MAX = unreachable)
//   node_count   — total live nodes
//
// Key invariants:
//   Empty graph            → node_count=0
//   Unknown source         → all distances=u32::MAX
//   Source is always first → vecs[0]==source, distances[0]==0
//   Unreachable node       → distances[i]==u32::MAX, parents[i]==ZERO_VEC
//   Source parent==self    → parents[0]==source
//   Directed path A→B→C   → dist(C)=dist(A→B)+dist(B→C)
//   No reverse edge B→A   → B unreachable from A if no A→B edge
//
// Test matrix:
//  1.  Empty graph                        → node_count=0
//  2.  Single node, source=itself         → dist=0, parent=self
//  3.  Unknown source (no match)          → all distances=u32::MAX
//  4.  K₂ A→B weight=1.0, source=A       → B dist=1000, parent=A
//  5.  K₂ A→B weight=2.5, source=A       → B dist=2500
//  6.  Directed path A→B→C weight 1,2    → C dist=3000 from A
//  7.  Directed: A→B, no B→A; source=B   → A unreachable from B
//  8.  Diamond A→B(1), A→C(2), B→D(1), C→D(2); source=A → D dist=2000 (via B)
//  9.  Two components: A→B, C (isolated); source=A → C unreachable
// 10.  Source parent invariant: parents[0]==vecs[0] (source is its own parent)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const SP_PLUGIN: PluginId   = PluginId::from_ascii("KL_SPT_00");
const SP_EXEC:   ExecutorId = ExecutorId::from_ascii("spt.exec0");

const SP_KEY_A: &str = "sp.alpha";
const SP_KEY_B: &str = "sp.beta";
const SP_KEY_C: &str = "sp.gamma";
const SP_KEY_D: &str = "sp.delta";

const SP_ID_A: NodeId = derive_node_id(SP_PLUGIN, SP_KEY_A);
const SP_ID_B: NodeId = derive_node_id(SP_PLUGIN, SP_KEY_B);
const SP_ID_C: NodeId = derive_node_id(SP_PLUGIN, SP_KEY_C);
const SP_ID_D: NodeId = derive_node_id(SP_PLUGIN, SP_KEY_D);

const SP_VEC_A: VectorAddress = VectorAddress::new(26, 1, 1, 0);
const SP_VEC_B: VectorAddress = VectorAddress::new(26, 1, 2, 0);
const SP_VEC_C: VectorAddress = VectorAddress::new(26, 1, 3, 0);
const SP_VEC_D: VectorAddress = VectorAddress::new(26, 1, 4, 0);
const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);

const SP_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    SP_PLUGIN,
    name:         "kl-graph-shortest-harness",
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

fn node_spec(key: &'static str, node_id: NodeId, schema: u64) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       SP_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(SP_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(SP_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
}

fn add_edge(from: NodeId, to: NodeId, key: &'static str, weight: f32) {
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

fn dist_of(vecs: &[VectorAddress], dists: &[u32], total: usize, target: VectorAddress) -> Option<u32> {
    for i in 0..total { if vecs[i] == target { return Some(dists[i]); } }
    None
}

fn parent_of(vecs: &[VectorAddress], parents: &[VectorAddress], total: usize, target: VectorAddress) -> Option<VectorAddress> {
    for i in 0..total { if vecs[i] == target { return Some(parents[i]); } }
    None
}

// ── 1. Empty graph ────────────────────────────────────────────────────────────

#[test]
fn empty_graph() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _parents, _dists, total) = gos_runtime::graph_shortest::<128>(SP_VEC_A);
    assert_eq!(total, 0, "empty graph: no nodes");
}

// ── 2. Single node, source=itself → dist=0, parent=self ──────────────────────

#[test]
fn single_node_self_source() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A, 0xE001);
    let (vecs, parents, dists, total) = gos_runtime::graph_shortest::<128>(SP_VEC_A);
    assert_eq!(total, 1, "1 node");
    assert_eq!(vecs[0], SP_VEC_A, "source is first");
    assert_eq!(dists[0], 0, "source distance = 0");
    assert_eq!(parents[0], SP_VEC_A, "source parent = self");
}

// ── 3. Unknown source → all distances = u32::MAX ─────────────────────────────

#[test]
fn unknown_source_all_unreachable() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A, 0xE001);
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B, 0xE002);
    // SP_VEC_C is not registered — use it as unknown source.
    let (_vecs, _parents, dists, total) = gos_runtime::graph_shortest::<128>(SP_VEC_C);
    assert_eq!(total, 2, "2 nodes registered");
    for i in 0..total {
        assert_eq!(dists[i], u32::MAX, "all unreachable from unknown source");
    }
}

// ── 4. K₂ A→B weight=1.0, source=A → B dist=1000 ────────────────────────────

#[test]
fn k2_directed_dist_one() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A, 0xE001);
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B, 0xE002);
    add_edge(SP_ID_A, SP_ID_B, "sp.ab.t4", 1.0);
    let (vecs, parents, dists, total) = gos_runtime::graph_shortest::<128>(SP_VEC_A);
    assert_eq!(total, 2, "2 nodes");
    assert_eq!(vecs[0], SP_VEC_A, "source first");
    assert_eq!(dists[0], 0, "source dist=0");
    let db = dist_of(&vecs, &dists, total, SP_VEC_B).expect("B present");
    assert_eq!(db, 1000, "B dist from A = 1000");
    let pb = parent_of(&vecs, &parents, total, SP_VEC_B).expect("B present");
    assert_eq!(pb, SP_VEC_A, "B parent = A");
}

// ── 5. K₂ A→B weight=2.5, source=A → B dist=2500 ───────────────────────────

#[test]
fn k2_directed_dist_two_point_five() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A, 0xE001);
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B, 0xE002);
    add_edge(SP_ID_A, SP_ID_B, "sp.ab.t5", 2.5);
    let (vecs, _parents, dists, total) = gos_runtime::graph_shortest::<128>(SP_VEC_A);
    assert_eq!(total, 2);
    let db = dist_of(&vecs, &dists, total, SP_VEC_B).expect("B present");
    assert_eq!(db, 2500, "B dist = 2500");
}

// ── 6. Path A→B(1)→C(2), source=A → C dist=3000 ─────────────────────────────

#[test]
fn path_abc_cumulative_dist() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A, 0xE001);
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B, 0xE002);
    add_node(SP_VEC_C, SP_KEY_C, SP_ID_C, 0xE003);
    add_edge(SP_ID_A, SP_ID_B, "sp.ab.t6", 1.0);
    add_edge(SP_ID_B, SP_ID_C, "sp.bc.t6", 2.0);
    let (vecs, parents, dists, total) = gos_runtime::graph_shortest::<128>(SP_VEC_A);
    assert_eq!(total, 3, "3 nodes");
    let db = dist_of(&vecs, &dists, total, SP_VEC_B).expect("B");
    let dc = dist_of(&vecs, &dists, total, SP_VEC_C).expect("C");
    assert_eq!(db, 1000, "B dist from A = 1000");
    assert_eq!(dc, 3000, "C dist from A = 1+2 = 3000");
    let pc = parent_of(&vecs, &parents, total, SP_VEC_C).expect("C");
    assert_eq!(pc, SP_VEC_B, "C parent in SPT = B");
}

// ── 7. Directed A→B only; source=B → A unreachable ──────────────────────────

#[test]
fn directed_no_reverse_edge_unreachable() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A, 0xE001);
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B, 0xE002);
    add_edge(SP_ID_A, SP_ID_B, "sp.ab.t7", 1.0); // A→B only; no B→A
    let (vecs, parents, dists, total) = gos_runtime::graph_shortest::<128>(SP_VEC_B);
    assert_eq!(total, 2, "2 nodes");
    assert_eq!(vecs[0], SP_VEC_B, "source B is first");
    assert_eq!(dists[0], 0, "source dist=0");
    let da = dist_of(&vecs, &dists, total, SP_VEC_A).expect("A present");
    assert_eq!(da, u32::MAX, "A is unreachable from B (no B→A edge)");
    let pa = parent_of(&vecs, &parents, total, SP_VEC_A).expect("A present");
    assert_eq!(pa, ZERO_VEC, "unreachable node has ZERO_VEC parent");
}

// ── 8. Diamond: A→B(1), A→C(2), B→D(1), C→D(2); source=A → D dist=2000 ──────

#[test]
fn diamond_shortest_via_b() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A, 0xE001);
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B, 0xE002);
    add_node(SP_VEC_C, SP_KEY_C, SP_ID_C, 0xE003);
    add_node(SP_VEC_D, SP_KEY_D, SP_ID_D, 0xE004);
    add_edge(SP_ID_A, SP_ID_B, "sp.ab.t8", 1.0);
    add_edge(SP_ID_A, SP_ID_C, "sp.ac.t8", 2.0);
    add_edge(SP_ID_B, SP_ID_D, "sp.bd.t8", 1.0);
    add_edge(SP_ID_C, SP_ID_D, "sp.cd.t8", 2.0);
    let (vecs, parents, dists, total) = gos_runtime::graph_shortest::<128>(SP_VEC_A);
    assert_eq!(total, 4, "4 nodes");
    let dd = dist_of(&vecs, &dists, total, SP_VEC_D).expect("D");
    // Shortest: A→B(1)→D(1) = 2.  Via A→C(2)→D(2) = 4.  Must pick 2.
    assert_eq!(dd, 2000, "D shortest from A = 1+1 = 2000 (via B)");
    let pd = parent_of(&vecs, &parents, total, SP_VEC_D).expect("D");
    assert_eq!(pd, SP_VEC_B, "D parent in SPT = B");
}

// ── 9. Two components: A→B, C isolated; source=A → C unreachable ─────────────

#[test]
fn two_components_unreachable() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A, 0xE001);
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B, 0xE002);
    add_node(SP_VEC_C, SP_KEY_C, SP_ID_C, 0xE003);
    add_edge(SP_ID_A, SP_ID_B, "sp.ab.t9", 1.0);
    // C has no edges — unreachable from A.
    let (vecs, _parents, dists, total) = gos_runtime::graph_shortest::<128>(SP_VEC_A);
    assert_eq!(total, 3, "3 nodes");
    let dc = dist_of(&vecs, &dists, total, SP_VEC_C).expect("C present");
    assert_eq!(dc, u32::MAX, "isolated C is unreachable from A");
}

// ── 10. Source parent invariant: parents[0] == vecs[0] ───────────────────────

#[test]
fn source_parent_is_self() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A, 0xE001);
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B, 0xE002);
    add_node(SP_VEC_C, SP_KEY_C, SP_ID_C, 0xE003);
    add_edge(SP_ID_A, SP_ID_B, "sp.ab.t10", 1.0);
    add_edge(SP_ID_A, SP_ID_C, "sp.ac.t10", 2.0);
    let (vecs, parents, dists, total) = gos_runtime::graph_shortest::<128>(SP_VEC_A);
    assert_eq!(total, 3, "3 nodes");
    assert_eq!(vecs[0], SP_VEC_A, "source A is first");
    assert_eq!(dists[0], 0, "source dist=0");
    assert_eq!(parents[0], SP_VEC_A, "source parent == self (invariant)");
}
