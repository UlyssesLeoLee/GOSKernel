// gos-graph-katz-harness — V2.42 Katz centrality API tests
//
// Verifies `gos_runtime::graph_katz` — incoming Katz centrality per node.
//
// KC[v] = Σ_{k=1}^{∞} α^k × (directed walks of length k ending at v), α = 1/8.
// Iterative fixed-point: x^(t+1)[v] = Σ_{u: u→v} (SCALE + x^(t)[u]) / 8
// Converged values (SCALE = 1_000_000, 20 iterations):
//   Isolated node:         KC = 0
//   Node with 1 in-edge from leaf: KC = SCALE / 8 = 125_000
//   Node with 2 in-edges from leaves: KC = 250_000
//   Node with 3 in-edges from leaves: KC = 375_000
//   Node in a 2-cycle (mutual):  KC = SCALE/7 ≈ 142_857
//   Node in a 3-cycle:           KC ≈ 142_857 (same fixed-point)
//   Self-loop only:              KC ≈ 142_857 (α/(1-α))
//
// OS analogy: `netstat -s` — which kernel service receives the most signal traffic
// accumulated over all directed path lengths?
//
//  1. Empty graph                       → total=0
//  2. Single isolated node              → kc[A]=0
//  3. Single edge A→B                   → kc[B]=125_000, kc[A]=0; B sorted first
//  4. Chain A→B→C                       → kc[C]=140_625 > kc[B]=125_000 > kc[A]=0
//  5. Fan-in {A,B,C}→D                  → kc[D]=375_000 > kc[A/B/C]=0
//  6. 3-cycle A→B→C→A                   → all kc equal (symmetry) ≈ 142_857
//  7. Mutual 2-cycle A↔B                → kc[A]==kc[B] ≈ 142_857
//  8. Fork + chain A→B, A→C, B→D       → kc[D]>kc[B]=kc[C]>kc[A]=0
//  9. In-degree proportionality         → 2 in-edges gives 2× score vs 1 in-edge
// 10. Self-loop A→A                     → kc[A] > 0 (α/(1-α) fixed-point)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const KTZ_PLUGIN: PluginId   = PluginId::from_ascii("KL_KTZ0_00");
const KTZ_EXEC:   ExecutorId = ExecutorId::from_ascii("ktz.exec");

const KTZ_KEY_A: &str = "ktz.alpha";
const KTZ_KEY_B: &str = "ktz.beta";
const KTZ_KEY_C: &str = "ktz.gamma";
const KTZ_KEY_D: &str = "ktz.delta";

const KTZ_ID_A: NodeId = derive_node_id(KTZ_PLUGIN, KTZ_KEY_A);
const KTZ_ID_B: NodeId = derive_node_id(KTZ_PLUGIN, KTZ_KEY_B);
const KTZ_ID_C: NodeId = derive_node_id(KTZ_PLUGIN, KTZ_KEY_C);
const KTZ_ID_D: NodeId = derive_node_id(KTZ_PLUGIN, KTZ_KEY_D);

const KTZ_VEC_A: VectorAddress = VectorAddress::new(19, 1, 1, 0);
const KTZ_VEC_B: VectorAddress = VectorAddress::new(19, 1, 2, 0);
const KTZ_VEC_C: VectorAddress = VectorAddress::new(19, 1, 3, 0);
const KTZ_VEC_D: VectorAddress = VectorAddress::new(19, 1, 4, 0);

const KTZ_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    KTZ_PLUGIN,
    name:         "kl-graph-katz-harness",
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
        executor_id:       KTZ_EXEC,
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
    gos_runtime::discover_plugin(KTZ_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(KTZ_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

fn find_katz(
    vecs: &[VectorAddress],
    katz: &[u32],
    total: usize,
    target: VectorAddress,
) -> Option<u32> {
    for i in 0..total {
        if vecs[i] == target { return Some(katz[i]); }
    }
    None
}

// ── 1. Empty graph → total=0 ─────────────────────────────────────────────────

#[test]
fn empty_graph_katz_total_is_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _katz, total) = gos_runtime::graph_katz::<128>();
    assert_eq!(total, 0, "empty graph: 0 nodes");
}

// ── 2. Single isolated node → kc=0 ───────────────────────────────────────────

#[test]
fn isolated_node_has_zero_katz() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(KTZ_VEC_A, KTZ_KEY_A, KTZ_ID_A, 0xF001);
    let (vecs, katz, total) = gos_runtime::graph_katz::<128>();
    assert_eq!(total, 1, "1 node");
    let a = find_katz(&vecs, &katz, total, KTZ_VEC_A).expect("A must be in result");
    assert_eq!(a, 0, "isolated node: no walks reach it, kc=0");
}

// ── 3. Single edge A→B: kc[B]=125_000, kc[A]=0 ──────────────────────────────

#[test]
fn single_edge_katz_receiver_is_nonzero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(KTZ_VEC_A, KTZ_KEY_A, KTZ_ID_A, 0xF001);
    add_node(KTZ_VEC_B, KTZ_KEY_B, KTZ_ID_B, 0xF002);
    add_edge(KTZ_ID_A, KTZ_ID_B, "ktz.ab.t3");
    let (vecs, katz, total) = gos_runtime::graph_katz::<128>();
    assert_eq!(total, 2, "2 nodes");
    let a = find_katz(&vecs, &katz, total, KTZ_VEC_A).unwrap();
    let b = find_katz(&vecs, &katz, total, KTZ_VEC_B).unwrap();
    // B receives one walk of length 1 from A (with A having no predecessors).
    // Steady-state: x_B = (1_000_000 + x_A) / 8 = 1_000_000 / 8 = 125_000 (since x_A = 0).
    assert_eq!(a, 0, "A has no incoming edges: kc=0");
    assert_eq!(b, 125_000, "B receives from A: kc = SCALE/8 = 125_000");
    // Sorted descending: B first.
    assert_eq!(vecs[0], KTZ_VEC_B, "B (higher kc) must be sorted first");
}

// ── 4. Chain A→B→C: kc[C]>kc[B]>kc[A]=0 ─────────────────────────────────────

#[test]
fn chain_katz_descending_ordering() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(KTZ_VEC_A, KTZ_KEY_A, KTZ_ID_A, 0xF001);
    add_node(KTZ_VEC_B, KTZ_KEY_B, KTZ_ID_B, 0xF002);
    add_node(KTZ_VEC_C, KTZ_KEY_C, KTZ_ID_C, 0xF003);
    add_edge(KTZ_ID_A, KTZ_ID_B, "ktz.ab.t4");
    add_edge(KTZ_ID_B, KTZ_ID_C, "ktz.bc.t4");
    let (vecs, katz, total) = gos_runtime::graph_katz::<128>();
    assert_eq!(total, 3, "3 nodes");
    let a = find_katz(&vecs, &katz, total, KTZ_VEC_A).unwrap();
    let b = find_katz(&vecs, &katz, total, KTZ_VEC_B).unwrap();
    let c = find_katz(&vecs, &katz, total, KTZ_VEC_C).unwrap();
    // x_B converges to 1M/8 = 125_000 (A contributes 1M each iter).
    // x_C converges to (1M + 125_000) / 8 = 1_125_000 / 8 = 140_625.
    assert_eq!(a, 0,       "A has no in-edges: kc=0");
    assert_eq!(b, 125_000, "B: kc = SCALE/8 = 125_000");
    assert_eq!(c, 140_625, "C: kc = (SCALE + 125_000) / 8 = 140_625");
    // C > B > A: sorted descending → C first, A last.
    assert_eq!(vecs[0], KTZ_VEC_C, "C (highest kc) must be first");
    assert_eq!(vecs[2], KTZ_VEC_A, "A (kc=0) must be last");
}

// ── 5. Fan-in {A,B,C}→D: kc[D]=375_000, leaves=0 ────────────────────────────

#[test]
fn fan_in_hub_has_highest_katz() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(KTZ_VEC_A, KTZ_KEY_A, KTZ_ID_A, 0xF001);
    add_node(KTZ_VEC_B, KTZ_KEY_B, KTZ_ID_B, 0xF002);
    add_node(KTZ_VEC_C, KTZ_KEY_C, KTZ_ID_C, 0xF003);
    add_node(KTZ_VEC_D, KTZ_KEY_D, KTZ_ID_D, 0xF004);
    add_edge(KTZ_ID_A, KTZ_ID_D, "ktz.ad.t5");
    add_edge(KTZ_ID_B, KTZ_ID_D, "ktz.bd.t5");
    add_edge(KTZ_ID_C, KTZ_ID_D, "ktz.cd.t5");
    let (vecs, katz, total) = gos_runtime::graph_katz::<128>();
    assert_eq!(total, 4, "4 nodes");
    let a = find_katz(&vecs, &katz, total, KTZ_VEC_A).unwrap();
    let b = find_katz(&vecs, &katz, total, KTZ_VEC_B).unwrap();
    let c = find_katz(&vecs, &katz, total, KTZ_VEC_C).unwrap();
    let d = find_katz(&vecs, &katz, total, KTZ_VEC_D).unwrap();
    // D receives 3 direct edges from isolated sources: x_D = 3 × 1M/8 = 375_000.
    assert_eq!(d, 375_000, "D: kc = 3 × SCALE/8 = 375_000");
    assert_eq!(a, 0, "A: leaf, kc=0");
    assert_eq!(b, 0, "B: leaf, kc=0");
    assert_eq!(c, 0, "C: leaf, kc=0");
    // D must be sorted first.
    assert_eq!(vecs[0], KTZ_VEC_D, "D (highest kc) must be first");
}

// ── 6. 3-cycle A→B→C→A: all equal kc (symmetry) ──────────────────────────────

#[test]
fn directed_cycle_all_nodes_same_katz() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(KTZ_VEC_A, KTZ_KEY_A, KTZ_ID_A, 0xF001);
    add_node(KTZ_VEC_B, KTZ_KEY_B, KTZ_ID_B, 0xF002);
    add_node(KTZ_VEC_C, KTZ_KEY_C, KTZ_ID_C, 0xF003);
    add_edge(KTZ_ID_A, KTZ_ID_B, "ktz.ab.t6");
    add_edge(KTZ_ID_B, KTZ_ID_C, "ktz.bc.t6");
    add_edge(KTZ_ID_C, KTZ_ID_A, "ktz.ca.t6");
    let (vecs, katz, total) = gos_runtime::graph_katz::<128>();
    assert_eq!(total, 3, "3 nodes in cycle");
    let a = find_katz(&vecs, &katz, total, KTZ_VEC_A).unwrap();
    let b = find_katz(&vecs, &katz, total, KTZ_VEC_B).unwrap();
    let c = find_katz(&vecs, &katz, total, KTZ_VEC_C).unwrap();
    // By 3-cycle symmetry all nodes converge to the same fixed-point ≈ 142_857.
    assert_eq!(a, b, "3-cycle symmetry: A and B must have equal kc");
    assert_eq!(b, c, "3-cycle symmetry: B and C must have equal kc");
    // Each score must be positive (walks circulate).
    assert!(a > 0, "3-cycle node A must have kc > 0");
    // Convergence: x* = 1M / 7 = 142_857 (integer truncation).
    assert!(a >= 142_857 && a <= 142_858, "kc ≈ SCALE/7 = 142_857±1; got {}", a);
    // All three entries must have equal kc.
    for i in 0..3 {
        assert_eq!(katz[i], a, "all cycle entries must have equal kc");
    }
}

// ── 7. Mutual 2-cycle A↔B: kc[A]==kc[B] ─────────────────────────────────────

#[test]
fn mutual_edge_both_nodes_equal_katz() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(KTZ_VEC_A, KTZ_KEY_A, KTZ_ID_A, 0xF001);
    add_node(KTZ_VEC_B, KTZ_KEY_B, KTZ_ID_B, 0xF002);
    add_edge(KTZ_ID_A, KTZ_ID_B, "ktz.ab.t7");
    add_edge(KTZ_ID_B, KTZ_ID_A, "ktz.ba.t7");
    let (vecs, katz, total) = gos_runtime::graph_katz::<128>();
    assert_eq!(total, 2, "2 nodes");
    let a = find_katz(&vecs, &katz, total, KTZ_VEC_A).unwrap();
    let b = find_katz(&vecs, &katz, total, KTZ_VEC_B).unwrap();
    // Mutual cycle: x* = SCALE / 7 = 142_857 for both (same fixed-point as 3-cycle).
    assert_eq!(a, b, "mutual cycle: both nodes must have identical kc");
    assert!(a > 0, "mutual cycle node must have kc > 0");
    assert!(a >= 142_857 && a <= 142_858, "kc ≈ SCALE/7 = 142_857±1; got {}", a);
    // Equal kc: either ordering is valid.
    let _ = vecs;
}

// ── 8. Fork + chain A→B, A→C, B→D: kc ordering D > B = C > A ────────────────

#[test]
fn fork_and_chain_katz_ordering() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(KTZ_VEC_A, KTZ_KEY_A, KTZ_ID_A, 0xF001);
    add_node(KTZ_VEC_B, KTZ_KEY_B, KTZ_ID_B, 0xF002);
    add_node(KTZ_VEC_C, KTZ_KEY_C, KTZ_ID_C, 0xF003);
    add_node(KTZ_VEC_D, KTZ_KEY_D, KTZ_ID_D, 0xF004);
    add_edge(KTZ_ID_A, KTZ_ID_B, "ktz.ab.t8");
    add_edge(KTZ_ID_A, KTZ_ID_C, "ktz.ac.t8");
    add_edge(KTZ_ID_B, KTZ_ID_D, "ktz.bd.t8");
    let (vecs, katz, total) = gos_runtime::graph_katz::<128>();
    assert_eq!(total, 4, "4 nodes");
    let a = find_katz(&vecs, &katz, total, KTZ_VEC_A).unwrap();
    let b = find_katz(&vecs, &katz, total, KTZ_VEC_B).unwrap();
    let c = find_katz(&vecs, &katz, total, KTZ_VEC_C).unwrap();
    let d = find_katz(&vecs, &katz, total, KTZ_VEC_D).unwrap();
    // x_B = 1M/8 = 125_000; x_C = 1M/8 = 125_000 (both receive only from A).
    // x_D = (1M + x_B) / 8 = (1M + 125_000) / 8 = 140_625 > 125_000.
    assert_eq!(a, 0,       "A: no in-edges, kc=0");
    assert_eq!(b, 125_000, "B: receives from A, kc=125_000");
    assert_eq!(c, 125_000, "C: receives from A, kc=125_000");
    assert_eq!(d, 140_625, "D: receives from B (which itself receives from A), kc=140_625");
    // Ordering: D > B = C > A.
    assert!(d > b, "D must score higher than B");
    assert_eq!(b, c, "B and C must score equally");
    assert!(b > a, "B must score higher than A");
    // D must be sorted first.
    assert_eq!(vecs[0], KTZ_VEC_D, "D (highest kc) must be first");
    // A must be last (kc=0).
    assert_eq!(vecs[3], KTZ_VEC_A, "A (kc=0) must be last");
}

// ── 9. In-degree proportionality: 2 in-edges = 2× score of 1 in-edge ─────────

#[test]
fn indegree_proportionality() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Setup: A→C and B→C (fan-in 2 → C), D→C2 (separate node, fan-in 1).
    // Re-use KTZ_ID_A, B, C (fan-in 2) plus KTZ_ID_D (isolated source to a 2nd target).
    // We measure C (2 sources) vs a hypothetical single-source node.
    // Simpler: just measure kc[C] with 2 sources vs kc[B] with 1 source (B has 1 in-edge from A).
    add_node(KTZ_VEC_A, KTZ_KEY_A, KTZ_ID_A, 0xF001); // source 1
    add_node(KTZ_VEC_B, KTZ_KEY_B, KTZ_ID_B, 0xF002); // source 2
    add_node(KTZ_VEC_C, KTZ_KEY_C, KTZ_ID_C, 0xF003); // receives from A and B
    add_node(KTZ_VEC_D, KTZ_KEY_D, KTZ_ID_D, 0xF004); // receives from A only
    add_edge(KTZ_ID_A, KTZ_ID_C, "ktz.ac.t9");
    add_edge(KTZ_ID_B, KTZ_ID_C, "ktz.bc.t9");
    add_edge(KTZ_ID_A, KTZ_ID_D, "ktz.ad.t9");
    let (vecs, katz, total) = gos_runtime::graph_katz::<128>();
    assert_eq!(total, 4, "4 nodes");
    let c = find_katz(&vecs, &katz, total, KTZ_VEC_C).unwrap();
    let d = find_katz(&vecs, &katz, total, KTZ_VEC_D).unwrap();
    // x_C = 2 × 1M/8 = 250_000 (two isolated sources).
    // x_D = 1 × 1M/8 = 125_000 (one source).
    assert_eq!(d, 125_000, "D: 1 in-edge from isolated source, kc=125_000");
    assert_eq!(c, 250_000, "C: 2 in-edges from isolated sources, kc=250_000");
    // In-degree proportionality: kc[C] == 2 × kc[D].
    assert_eq!(c, 2 * d, "2 in-edges must give exactly 2× the kc of 1 in-edge");
    assert_eq!(vecs[0], KTZ_VEC_C, "C (higher kc) must be sorted first");
    // A and B have no in-edges: kc=0.
    let a = find_katz(&vecs, &katz, total, KTZ_VEC_A).unwrap();
    let b = find_katz(&vecs, &katz, total, KTZ_VEC_B).unwrap();
    assert_eq!(a, 0, "A: source node, kc=0");
    assert_eq!(b, 0, "B: source node, kc=0");
}

// ── 10. Self-loop A→A: kc[A] = α/(1-α) × SCALE ≈ 142_857 ───────────────────

#[test]
fn self_loop_contributes_to_katz() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(KTZ_VEC_A, KTZ_KEY_A, KTZ_ID_A, 0xF001);
    add_node(KTZ_VEC_B, KTZ_KEY_B, KTZ_ID_B, 0xF002); // isolated, for comparison
    add_edge(KTZ_ID_A, KTZ_ID_A, "ktz.aa.t10"); // self-loop
    let (vecs, katz, total) = gos_runtime::graph_katz::<128>();
    assert_eq!(total, 2, "2 nodes");
    let a = find_katz(&vecs, &katz, total, KTZ_VEC_A).unwrap();
    let b = find_katz(&vecs, &katz, total, KTZ_VEC_B).unwrap();
    // A has a self-loop: walks A→A, A→A→A, ... of all lengths contribute.
    // Fixed-point: x_A = (1M + x_A) / 8 → 7 × x_A = 1M → x_A = 142_857 (truncated).
    assert!(a > 0, "self-loop must make kc > 0");
    assert!(a >= 142_857 && a <= 142_858, "self-loop kc ≈ SCALE/7 = 142_857±1; got {}", a);
    assert_eq!(b, 0, "isolated B must have kc=0");
    // A (kc > 0) must appear before B (kc=0).
    assert_eq!(vecs[0], KTZ_VEC_A, "A (kc > 0) must be sorted first");
}
