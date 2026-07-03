// gos-graph-sim-harness — V2.52 random walk simulation
//
// Verifies `gos_runtime::graph_sim` — simulate N random-walk steps over the
// live kernel graph and report per-node visit counts sorted descending.
//
// Algorithm summary:
//   1. Start at a random live node (seeded from `seed` via xorshift32).
//   2. Each step: sample an outgoing edge proportional to edge weight; move.
//   3. Dead-end nodes (no outgoing edges) → teleport to a random live node.
//   4. Count visits per node; sort descending; return with step accounting.
//
// Return value: (vecs, visits, node_count, actual_steps, stuck_steps)
//   vecs[0..n]   — nodes sorted by visit count descending
//   visits[0..n] — raw visit count for each node
//   n            — total live nodes
//   actual       — steps that traversed an edge
//   stuck        — steps that teleported (dead ends)
//
// Key invariants:
//   Empty graph                 → node_count=0, actual=0, stuck=0
//   steps=0                     → all zeros
//   steps clamped to 256        → actual+stuck ≤ 256
//   Non-empty, steps>0          → sum(visits[0..n]) == 1 + actual + stuck == 1 + steps
//   actual + stuck              == steps (for steps clamped to 256)
//   Output sorted descending    → visits[0] ≥ visits[1] ≥ ... ≥ visits[n-1]
//   node_count                  → matches registered node count
//   Single node, no edges       → stuck=steps, actual=0, visits[0]=steps+1
//   Single node, self-loop      → stuck=0, actual=steps, visits[0]=steps+1
//
// Test matrix:
//  1.  Empty graph, steps=8              → node_count=0, actual=0, stuck=0
//  2.  steps=0, graph has nodes          → all zeros returned
//  3.  Single node, no edges, steps=8   → stuck=8, actual=0, visits[0]=9
//  4.  Single node, self-loop, steps=8  → stuck=0, actual=8, visits[0]=9
//  5.  steps clamped to 256             → actual+stuck ≤ 256
//  6.  Visit sum invariant (3-node DAG) → sum(visits)==1+steps
//  7.  actual+stuck == steps            → step accounting complete
//  8.  node_count matches registered    → correct count for multi-node graph
//  9.  Output sorted descending         → visits[i] >= visits[i+1]
// 10.  Two-cycle A→B→A, self-consistent → sum(visits)==1+steps, sorted

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const SIM_PLUGIN: PluginId   = PluginId::from_ascii("KL_SIM_000");
const SIM_EXEC:   ExecutorId = ExecutorId::from_ascii("sim.exec00");

const SIM_KEY_A: &str = "sim.alpha";
const SIM_KEY_B: &str = "sim.beta";
const SIM_KEY_C: &str = "sim.gamma";
const SIM_KEY_D: &str = "sim.delta";

const SIM_ID_A: NodeId = derive_node_id(SIM_PLUGIN, SIM_KEY_A);
const SIM_ID_B: NodeId = derive_node_id(SIM_PLUGIN, SIM_KEY_B);
const SIM_ID_C: NodeId = derive_node_id(SIM_PLUGIN, SIM_KEY_C);
const SIM_ID_D: NodeId = derive_node_id(SIM_PLUGIN, SIM_KEY_D);

const SIM_VEC_A: VectorAddress = VectorAddress::new(29, 1, 1, 0);
const SIM_VEC_B: VectorAddress = VectorAddress::new(29, 1, 2, 0);
const SIM_VEC_C: VectorAddress = VectorAddress::new(29, 1, 3, 0);
const SIM_VEC_D: VectorAddress = VectorAddress::new(29, 1, 4, 0);

const SIM_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    SIM_PLUGIN,
    name:         "kl-graph-sim-harness",
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
        executor_id:       SIM_EXEC,
        state_schema_hash: 0xD0D0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(SIM_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(SIM_PLUGIN, vec, node_spec(key, id)).unwrap();
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

// Deterministic seed for tests — epoch is 0 after reset().
const FIXED_SEED: u32 = 0xCAFE_BABE;

fn run_sim(steps: u32) -> ([VectorAddress; 128], [u32; 128], usize, u32, u32) {
    gos_runtime::graph_sim::<128>(steps, FIXED_SEED)
}

// ── 1. Empty graph → node_count=0, actual=0, stuck=0 ─────────────────────────

#[test]
fn empty_graph_returns_all_zeros() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_, visits, n, actual, stuck) = run_sim(8);
    assert_eq!(n, 0, "empty graph: node_count must be 0");
    assert_eq!(actual, 0, "empty graph: actual_steps must be 0");
    assert_eq!(stuck, 0, "empty graph: stuck_steps must be 0");
    assert_eq!(visits[0], 0, "empty graph: visits[0] must be 0");
}

// ── 2. steps=0 → all zeros ───────────────────────────────────────────────────

#[test]
fn zero_steps_returns_all_zeros() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SIM_VEC_A, SIM_KEY_A, SIM_ID_A);

    let (_, visits, n, actual, stuck) = run_sim(0);
    assert_eq!(n, 1, "steps=0: node_count must still be 1");
    assert_eq!(actual, 0, "steps=0: actual_steps must be 0");
    assert_eq!(stuck, 0, "steps=0: stuck_steps must be 0");
    let total_visits: u32 = visits.iter().take(n).sum();
    assert_eq!(total_visits, 0, "steps=0: sum(visits) must be 0");
}

// ── 3. Single node, no edges, steps=8 → stuck=8, actual=0, visits[0]=9 ───────

#[test]
fn single_node_no_edges_all_stuck() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SIM_VEC_A, SIM_KEY_A, SIM_ID_A);

    let (_, visits, n, actual, stuck) = run_sim(8);
    assert_eq!(n, 1, "single node: count=1");
    assert_eq!(actual, 0, "single node no edges: no traversals possible");
    assert_eq!(stuck, 8, "single node no edges: all 8 steps stuck");
    // Starting visit (1) + 8 teleports back to the only node = 9 total.
    assert_eq!(visits[0], 9, "single node: visits[0] must be 9 (1 start + 8 stuck)");
}

// ── 4. Single node, self-loop, steps=8 → stuck=0, actual=8, visits[0]=9 ──────

#[test]
fn single_node_self_loop_no_stuck() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SIM_VEC_A, SIM_KEY_A, SIM_ID_A);
    add_edge(SIM_ID_A, SIM_ID_A, "sim.aa.self");

    let (vecs, visits, n, actual, stuck) = run_sim(8);
    assert_eq!(n, 1, "self-loop: 1 node");
    assert_eq!(stuck, 0, "self-loop: no dead ends");
    assert_eq!(actual, 8, "self-loop: 8 traversals");
    assert_eq!(visits[0], 9, "self-loop: visits[0] = 1 start + 8 transitions = 9");
    assert_eq!(vecs[0], SIM_VEC_A, "self-loop: only node is SIM_VEC_A");
}

// ── 5. steps clamped to 256 ──────────────────────────────────────────────────

#[test]
fn steps_clamped_to_256() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SIM_VEC_A, SIM_KEY_A, SIM_ID_A);
    add_edge(SIM_ID_A, SIM_ID_A, "sim.aa.cap");

    // Pass 999 — internal clamp should limit to 256.
    let (_, visits, n, actual, stuck) = gos_runtime::graph_sim::<128>(999, FIXED_SEED);
    assert_eq!(n, 1);
    assert!(actual + stuck <= 256, "clamped: actual+stuck must not exceed 256 (got {})", actual + stuck);
    // sum(visits) == 1 + clamped_steps
    let sum: u32 = visits.iter().take(n).sum();
    assert_eq!(sum, 1 + actual + stuck, "clamped: sum invariant holds");
}

// ── 6. Visit sum invariant (3-node linear DAG A→B→C) ─────────────────────────

#[test]
fn visit_sum_invariant_linear_dag() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SIM_VEC_A, SIM_KEY_A, SIM_ID_A);
    add_node(SIM_VEC_B, SIM_KEY_B, SIM_ID_B);
    add_node(SIM_VEC_C, SIM_KEY_C, SIM_ID_C);
    add_edge(SIM_ID_A, SIM_ID_B, "sim.ab.t6");
    add_edge(SIM_ID_B, SIM_ID_C, "sim.bc.t6");
    // C is a dead end → teleports happen.

    let steps = 32u32;
    let (_, visits, n, actual, stuck) = run_sim(steps);
    assert_eq!(n, 3, "3 nodes registered");
    let sum: u32 = visits.iter().take(n).sum();
    assert_eq!(sum, 1 + actual + stuck, "sum(visits) == 1 + actual + stuck");
    assert_eq!(actual + stuck, steps.min(256), "total steps == min(steps, 256)");
}

// ── 7. actual + stuck == steps ────────────────────────────────────────────────

#[test]
fn actual_plus_stuck_equals_steps() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SIM_VEC_A, SIM_KEY_A, SIM_ID_A);
    add_node(SIM_VEC_B, SIM_KEY_B, SIM_ID_B);
    // A→B only; B has no outgoing edges.
    add_edge(SIM_ID_A, SIM_ID_B, "sim.ab.t7");

    let steps = 20u32;
    let (_, _, n, actual, stuck) = run_sim(steps);
    assert_eq!(n, 2);
    assert_eq!(actual + stuck, steps.min(256),
        "actual({}) + stuck({}) must equal steps({})", actual, stuck, steps.min(256));
}

// ── 8. node_count matches registered nodes ────────────────────────────────────

#[test]
fn node_count_matches_registered() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SIM_VEC_A, SIM_KEY_A, SIM_ID_A);
    add_node(SIM_VEC_B, SIM_KEY_B, SIM_ID_B);
    add_node(SIM_VEC_C, SIM_KEY_C, SIM_ID_C);
    add_node(SIM_VEC_D, SIM_KEY_D, SIM_ID_D);

    let (_, _, n, _, _) = run_sim(16);
    assert_eq!(n, 4, "node_count must match 4 registered nodes");
}

// ── 9. Output sorted descending ───────────────────────────────────────────────

#[test]
fn output_sorted_descending() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SIM_VEC_A, SIM_KEY_A, SIM_ID_A);
    add_node(SIM_VEC_B, SIM_KEY_B, SIM_ID_B);
    add_node(SIM_VEC_C, SIM_KEY_C, SIM_ID_C);
    // Star: A→B, A→C (A is a potential restart target; B,C are sinks).
    add_edge(SIM_ID_A, SIM_ID_B, "sim.ab.t9");
    add_edge(SIM_ID_A, SIM_ID_C, "sim.ac.t9");

    let (_, visits, n, _, _) = run_sim(64);
    assert_eq!(n, 3);
    for i in 0..n.saturating_sub(1) {
        assert!(
            visits[i] >= visits[i + 1],
            "sorted: visits[{}]={} < visits[{}]={} — not sorted descending",
            i, visits[i], i + 1, visits[i + 1]
        );
    }
}

// ── 10. Two-cycle A→B→A — sum invariant and sorted ───────────────────────────

#[test]
fn two_cycle_sum_invariant_and_sorted() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SIM_VEC_A, SIM_KEY_A, SIM_ID_A);
    add_node(SIM_VEC_B, SIM_KEY_B, SIM_ID_B);
    add_edge(SIM_ID_A, SIM_ID_B, "sim.ab.t10");
    add_edge(SIM_ID_B, SIM_ID_A, "sim.ba.t10");

    let steps = 32u32;
    let (_, visits, n, actual, stuck) = run_sim(steps);
    assert_eq!(n, 2);
    assert_eq!(stuck, 0, "two-cycle has no dead ends");
    assert_eq!(actual, steps.min(256), "two-cycle: all steps are traversals");
    let sum: u32 = visits.iter().take(n).sum();
    assert_eq!(sum, 1 + actual + stuck,
        "two-cycle: sum(visits)={} must equal 1+{}+{}={}", sum, actual, stuck, 1+actual+stuck);
    assert!(
        visits[0] >= visits[1],
        "two-cycle: output must be sorted descending: visits[0]={} visits[1]={}", visits[0], visits[1]
    );
}
