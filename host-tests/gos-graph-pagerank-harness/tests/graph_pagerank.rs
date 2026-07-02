// gos-graph-pagerank-harness — V2.43 PageRank centrality API tests
//
// Verifies `gos_runtime::graph_pagerank` — PageRank stationary distribution.
//
// Algorithm: PR[v] = (1-d)×SCALE + d × Σ_{u→v, outdeg(u)>0} PR[u]/outdeg(u)
//   d = 0.85, SCALE = 1_000_000, TELE = 150_000, 20 iterations.
//
// Dangling nodes (out_degree=0) absorb their rank (no redistribution).
//
// Derived steady-state values used in assertions:
//   Isolated node:                  PR = 150_000
//   Source node (no in-edges):      PR = 150_000
//   Single-edge receiver (A→B):     PR[B] = 150_000 + 150_000×0.85 = 277_500
//   Chain A→B→C:                    PR[C] = 150_000 + 277_500×0.85 = 385_875
//   Fan-in {A,B,C}→D (each deg=1): PR[D] = 150_000 + 3×150_000×0.85 = 532_500
//   3-cycle A→B→C→A:               all PR = 1_000_000 (authority)
//   Mutual A↔B:                     both PR = 1_000_000 (authority)
//   Fork A→B, A→C (outdeg=2):       PR[B]=PR[C] = 150_000 + 150_000×0.85/2 = 213_750
//
// OS analogy: `top` by random-walk weight — most structurally authoritative nodes.
//
//  1. Empty graph                      → total=0
//  2. Single isolated node             → PR=150_000 (teleportation floor)
//  3. Single edge A→B                  → PR[B]=277_500 > PR[A]=150_000; B sorted first
//  4. Chain A→B→C                      → PR[C]=385_875 > PR[B]=277_500 > PR[A]=150_000
//  5. Fan-in {A,B,C}→D                 → PR[D]=532_500 (relay); others=150_000
//  6. 3-cycle A→B→C→A                  → all PR=1_000_000 (authority, random-walk equilibrium)
//  7. Mutual 2-cycle A↔B               → both PR=1_000_000 (authority)
//  8. Out-degree splits rank A→B, A→C  → PR[B]=PR[C]=213_750 < 277_500 (single-edge case)
//  9. Sort order                        → pr[0] ≥ pr[1] ≥ ... (descending)
// 10. Total count                       → equals number of registered nodes

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const PR_PLUGIN: PluginId   = PluginId::from_ascii("KL_PR00_00");
const PR_EXEC:   ExecutorId = ExecutorId::from_ascii("pr.exec000");

const PR_KEY_A: &str = "pr.alpha";
const PR_KEY_B: &str = "pr.beta";
const PR_KEY_C: &str = "pr.gamma";
const PR_KEY_D: &str = "pr.delta";

const PR_ID_A: NodeId = derive_node_id(PR_PLUGIN, PR_KEY_A);
const PR_ID_B: NodeId = derive_node_id(PR_PLUGIN, PR_KEY_B);
const PR_ID_C: NodeId = derive_node_id(PR_PLUGIN, PR_KEY_C);
const PR_ID_D: NodeId = derive_node_id(PR_PLUGIN, PR_KEY_D);

const PR_VEC_A: VectorAddress = VectorAddress::new(20, 1, 1, 0);
const PR_VEC_B: VectorAddress = VectorAddress::new(20, 1, 2, 0);
const PR_VEC_C: VectorAddress = VectorAddress::new(20, 1, 3, 0);
const PR_VEC_D: VectorAddress = VectorAddress::new(20, 1, 4, 0);

const PR_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    PR_PLUGIN,
    name:         "kl-graph-pagerank-harness",
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
        executor_id:       PR_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() {
    gos_runtime::reset();
}

fn register_plugin() {
    gos_runtime::discover_plugin(PR_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(PR_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
}

fn add_edge(from: NodeId, to: NodeId, key: &'static str) {
    let spec = EdgeSpec {
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
    };
    gos_runtime::register_edge(spec).unwrap();
}

fn find_pr(
    vecs: &[VectorAddress],
    pr:   &[u32],
    total: usize,
    target: VectorAddress,
) -> Option<u32> {
    for i in 0..total {
        if vecs[i] == target { return Some(pr[i]); }
    }
    None
}

// ── 1. Empty graph → total=0 ─────────────────────────────────────────────────

#[test]
fn empty_graph_pagerank_total_is_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _pr, total) = gos_runtime::graph_pagerank::<128>();
    assert_eq!(total, 0, "empty graph: 0 nodes");
}

// ── 2. Single isolated node → PR = 150_000 (teleportation floor) ─────────────

#[test]
fn isolated_node_has_teleportation_floor_pr() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PR_VEC_A, PR_KEY_A, PR_ID_A, 0xA001);
    let (vecs, pr, total) = gos_runtime::graph_pagerank::<128>();
    assert_eq!(total, 1, "1 node");
    let a = find_pr(&vecs, &pr, total, PR_VEC_A).expect("A must be in result");
    // No edges → only teleportation: PR = (1-0.85) × SCALE = 150_000.
    assert_eq!(a, 150_000, "isolated node: PR = teleportation floor 150_000");
}

// ── 3. Single edge A→B: PR[B]=277_500, PR[A]=150_000; B sorted first ─────────

#[test]
fn single_edge_receiver_has_higher_pr() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PR_VEC_A, PR_KEY_A, PR_ID_A, 0xA001);
    add_node(PR_VEC_B, PR_KEY_B, PR_ID_B, 0xA002);
    add_edge(PR_ID_A, PR_ID_B, "pr.ab.t3");
    let (vecs, pr, total) = gos_runtime::graph_pagerank::<128>();
    assert_eq!(total, 2, "2 nodes");
    let a = find_pr(&vecs, &pr, total, PR_VEC_A).unwrap();
    let b = find_pr(&vecs, &pr, total, PR_VEC_B).unwrap();
    // Steady state:
    //   PR[A] = 150_000  (no in-edges)
    //   PR[B] = 150_000 + 150_000 × 85/100 = 150_000 + 127_500 = 277_500
    assert_eq!(a, 150_000, "A: source node, PR = 150_000");
    assert_eq!(b, 277_500, "B: single in-edge from A, PR = 277_500");
    assert!(b > a, "receiver must outrank source");
    // Sorted descending: B first.
    assert_eq!(vecs[0], PR_VEC_B, "B (higher PR) must be sorted first");
}

// ── 4. Chain A→B→C: PR[C]=385_875 > PR[B]=277_500 > PR[A]=150_000 ───────────

#[test]
fn chain_pagerank_descending_with_depth() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PR_VEC_A, PR_KEY_A, PR_ID_A, 0xA001);
    add_node(PR_VEC_B, PR_KEY_B, PR_ID_B, 0xA002);
    add_node(PR_VEC_C, PR_KEY_C, PR_ID_C, 0xA003);
    add_edge(PR_ID_A, PR_ID_B, "pr.ab.t4");
    add_edge(PR_ID_B, PR_ID_C, "pr.bc.t4");
    let (vecs, pr, total) = gos_runtime::graph_pagerank::<128>();
    assert_eq!(total, 3, "3 nodes");
    let a = find_pr(&vecs, &pr, total, PR_VEC_A).unwrap();
    let b = find_pr(&vecs, &pr, total, PR_VEC_B).unwrap();
    let c = find_pr(&vecs, &pr, total, PR_VEC_C).unwrap();
    // PR[A] = 150_000
    // PR[B] = 150_000 + 150_000 × 85/100 = 277_500
    // PR[C] = 150_000 + 277_500 × 85/100 = 150_000 + 235_875 = 385_875
    assert_eq!(a, 150_000, "A: no in-edges, PR = 150_000");
    assert_eq!(b, 277_500, "B: receives from A, PR = 277_500");
    assert_eq!(c, 385_875, "C: receives from B (which has rank from A), PR = 385_875");
    // Ordering: C > B > A.
    assert!(c > b, "C must outrank B");
    assert!(b > a, "B must outrank A");
    // Sorted descending: C first, A last.
    assert_eq!(vecs[0], PR_VEC_C, "C (highest PR) must be first");
    assert_eq!(vecs[2], PR_VEC_A, "A (lowest PR) must be last");
}

// ── 5. Fan-in {A,B,C}→D: PR[D]=532_500, spokes=150_000; D first ─────────────

#[test]
fn fan_in_hub_accumulates_rank() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PR_VEC_A, PR_KEY_A, PR_ID_A, 0xA001);
    add_node(PR_VEC_B, PR_KEY_B, PR_ID_B, 0xA002);
    add_node(PR_VEC_C, PR_KEY_C, PR_ID_C, 0xA003);
    add_node(PR_VEC_D, PR_KEY_D, PR_ID_D, 0xA004);
    add_edge(PR_ID_A, PR_ID_D, "pr.ad.t5");
    add_edge(PR_ID_B, PR_ID_D, "pr.bd.t5");
    add_edge(PR_ID_C, PR_ID_D, "pr.cd.t5");
    let (vecs, pr, total) = gos_runtime::graph_pagerank::<128>();
    assert_eq!(total, 4, "4 nodes");
    let a = find_pr(&vecs, &pr, total, PR_VEC_A).unwrap();
    let b = find_pr(&vecs, &pr, total, PR_VEC_B).unwrap();
    let c = find_pr(&vecs, &pr, total, PR_VEC_C).unwrap();
    let d = find_pr(&vecs, &pr, total, PR_VEC_D).unwrap();
    // Spokes: PR = 150_000 (no in-edges).
    // D: PR = 150_000 + 3 × 150_000 × 85/100 = 150_000 + 382_500 = 532_500.
    assert_eq!(a, 150_000, "A: spoke, PR = 150_000");
    assert_eq!(b, 150_000, "B: spoke, PR = 150_000");
    assert_eq!(c, 150_000, "C: spoke, PR = 150_000");
    assert_eq!(d, 532_500, "D: hub, PR = 532_500");
    assert!(d > a, "hub must outrank spokes");
    // D sorted first.
    assert_eq!(vecs[0], PR_VEC_D, "D (hub) must be sorted first");
}

// ── 6. 3-cycle A→B→C→A: all PR=1_000_000 (authority) ────────────────────────

#[test]
fn directed_cycle_all_at_authority_level() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PR_VEC_A, PR_KEY_A, PR_ID_A, 0xA001);
    add_node(PR_VEC_B, PR_KEY_B, PR_ID_B, 0xA002);
    add_node(PR_VEC_C, PR_KEY_C, PR_ID_C, 0xA003);
    add_edge(PR_ID_A, PR_ID_B, "pr.ab.t6");
    add_edge(PR_ID_B, PR_ID_C, "pr.bc.t6");
    add_edge(PR_ID_C, PR_ID_A, "pr.ca.t6");
    let (vecs, pr, total) = gos_runtime::graph_pagerank::<128>();
    assert_eq!(total, 3, "3 nodes in cycle");
    let a = find_pr(&vecs, &pr, total, PR_VEC_A).unwrap();
    let b = find_pr(&vecs, &pr, total, PR_VEC_B).unwrap();
    let c = find_pr(&vecs, &pr, total, PR_VEC_C).unwrap();
    // Symmetry → all equal. Fixed-point: x = 150_000 + 0.85x → x = 1_000_000.
    assert_eq!(a, b, "3-cycle symmetry: A and B must be equal");
    assert_eq!(b, c, "3-cycle symmetry: B and C must be equal");
    assert_eq!(a, 1_000_000, "3-cycle steady state: PR = 1_000_000 (authority)");
    // All entries in the output must equal 1_000_000.
    for i in 0..3 {
        assert_eq!(pr[i], 1_000_000, "cycle entry {} must have PR = 1_000_000", i);
    }
    let _ = vecs;
}

// ── 7. Mutual 2-cycle A↔B: both PR=1_000_000 (authority) ─────────────────────

#[test]
fn mutual_cycle_both_authority() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PR_VEC_A, PR_KEY_A, PR_ID_A, 0xA001);
    add_node(PR_VEC_B, PR_KEY_B, PR_ID_B, 0xA002);
    add_edge(PR_ID_A, PR_ID_B, "pr.ab.t7");
    add_edge(PR_ID_B, PR_ID_A, "pr.ba.t7");
    let (vecs, pr, total) = gos_runtime::graph_pagerank::<128>();
    assert_eq!(total, 2, "2 nodes");
    let a = find_pr(&vecs, &pr, total, PR_VEC_A).unwrap();
    let b = find_pr(&vecs, &pr, total, PR_VEC_B).unwrap();
    // Mutual cycle: same fixed-point as 3-cycle → x = 1_000_000.
    assert_eq!(a, b, "mutual cycle: both nodes must have equal PR");
    assert_eq!(a, 1_000_000, "mutual cycle: PR = 1_000_000 (authority)");
    let _ = vecs;
}

// ── 8. Out-degree splits rank: A→B, A→C each get less than A→B alone ─────────

#[test]
fn out_degree_splits_rank_contribution() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PR_VEC_A, PR_KEY_A, PR_ID_A, 0xA001);
    add_node(PR_VEC_B, PR_KEY_B, PR_ID_B, 0xA002);
    add_node(PR_VEC_C, PR_KEY_C, PR_ID_C, 0xA003);
    add_edge(PR_ID_A, PR_ID_B, "pr.ab.t8");
    add_edge(PR_ID_A, PR_ID_C, "pr.ac.t8");
    let (vecs, pr, total) = gos_runtime::graph_pagerank::<128>();
    assert_eq!(total, 3, "3 nodes");
    let a = find_pr(&vecs, &pr, total, PR_VEC_A).unwrap();
    let b = find_pr(&vecs, &pr, total, PR_VEC_B).unwrap();
    let c = find_pr(&vecs, &pr, total, PR_VEC_C).unwrap();
    // A has out_deg=2: each target gets half A's contribution.
    // PR[A] = 150_000; contrib per target = 150_000 × 85 / (2 × 100) = 63_750.
    // PR[B] = PR[C] = 150_000 + 63_750 = 213_750.
    // Compare: single-edge case A→B gives PR[B]=277_500 (test 3) > 213_750.
    assert_eq!(a, 150_000, "A: source, PR = 150_000");
    assert_eq!(b, 213_750, "B: receives half of A's contribution, PR = 213_750");
    assert_eq!(c, 213_750, "C: receives half of A's contribution, PR = 213_750");
    assert!(b < 277_500, "split contribution must be less than single-target case");
    assert_eq!(b, c, "B and C receive equal contributions from A");
    let _ = vecs;
}

// ── 9. Sort order: pr[0] ≥ pr[1] ≥ ... ≥ pr[total-1] ───────────────────────

#[test]
fn output_is_sorted_descending() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Mix of authorities, relays, and sinks.
    add_node(PR_VEC_A, PR_KEY_A, PR_ID_A, 0xA001); // source → low PR
    add_node(PR_VEC_B, PR_KEY_B, PR_ID_B, 0xA002); // mid-chain
    add_node(PR_VEC_C, PR_KEY_C, PR_ID_C, 0xA003); // tail of chain → higher PR
    add_node(PR_VEC_D, PR_KEY_D, PR_ID_D, 0xA004); // receives from A,B,C → highest
    add_edge(PR_ID_A, PR_ID_B, "pr.ab.t9");
    add_edge(PR_ID_B, PR_ID_C, "pr.bc.t9");
    add_edge(PR_ID_A, PR_ID_D, "pr.ad.t9");
    add_edge(PR_ID_B, PR_ID_D, "pr.bd.t9");
    add_edge(PR_ID_C, PR_ID_D, "pr.cd.t9");
    let (_vecs, pr, total) = gos_runtime::graph_pagerank::<128>();
    assert_eq!(total, 4, "4 nodes");
    for i in 1..total {
        assert!(
            pr[i - 1] >= pr[i],
            "sort violation at index {}: pr[{}]={} < pr[{}]={}",
            i, i - 1, pr[i - 1], i, pr[i]
        );
    }
}

// ── 10. Total count equals number of registered nodes ────────────────────────

#[test]
fn total_count_matches_registered_nodes() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Register 3 nodes, verify total=3.
    add_node(PR_VEC_A, PR_KEY_A, PR_ID_A, 0xA001);
    add_node(PR_VEC_B, PR_KEY_B, PR_ID_B, 0xA002);
    add_node(PR_VEC_C, PR_KEY_C, PR_ID_C, 0xA003);
    add_edge(PR_ID_A, PR_ID_B, "pr.ab.t10");
    add_edge(PR_ID_B, PR_ID_C, "pr.bc.t10");
    let (_vecs, _pr, total) = gos_runtime::graph_pagerank::<128>();
    assert_eq!(total, 3, "total must equal number of registered nodes");
}
