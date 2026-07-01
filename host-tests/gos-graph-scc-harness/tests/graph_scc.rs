// gos-graph-scc-harness — V2.34 strongly connected components API tests
//
// Verifies `gos_runtime::graph_scc` — Kosaraju's two-pass DFS SCC decomposition
// added in V2.34.  Analogous to `scc(1)` / Graphviz `sccmap` / cargo's
// condensation step in dependency resolution.
//
//  1. Empty graph → 0 components, 0 total.
//  2. Single isolated node → 1 SCC of size 1.
//  3. Single node with self-loop → 1 SCC of size 1 (self-loop ignored).
//  4. Two-node mutual cycle A↔B → 1 SCC of size 2.
//  5. Linear chain A→B→C (DAG) → 3 SCCs of size 1 each, scc_count==total.
//  6. Triangle cycle A→B→C→A → 1 SCC of size 3.
//  7. Diamond DAG → 4 SCCs (all singletons), scc_count==total.
//  8. Mixed: triangle A→B→C→A + isolated D → 2 SCCs (sizes 3 and 1).
//  9. Two separate cycles → 2 SCCs.
// 10. When scc_count == total the graph is confirmed acyclic (DAG).

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const SCC_PLUGIN: PluginId   = PluginId::from_ascii("KL_SCC_H00");
const SCC_EXEC:   ExecutorId = ExecutorId::from_ascii("scc.exec");

const SCC_KEY_A: &str = "scc.alpha";
const SCC_KEY_B: &str = "scc.beta";
const SCC_KEY_C: &str = "scc.gamma";
const SCC_KEY_D: &str = "scc.delta";
const SCC_KEY_E: &str = "scc.epsilon";

const SCC_ID_A: NodeId = derive_node_id(SCC_PLUGIN, SCC_KEY_A);
const SCC_ID_B: NodeId = derive_node_id(SCC_PLUGIN, SCC_KEY_B);
const SCC_ID_C: NodeId = derive_node_id(SCC_PLUGIN, SCC_KEY_C);
const SCC_ID_D: NodeId = derive_node_id(SCC_PLUGIN, SCC_KEY_D);
const SCC_ID_E: NodeId = derive_node_id(SCC_PLUGIN, SCC_KEY_E);

const SCC_VEC_A: VectorAddress = VectorAddress::new(13, 1, 1, 0);
const SCC_VEC_B: VectorAddress = VectorAddress::new(13, 1, 2, 0);
const SCC_VEC_C: VectorAddress = VectorAddress::new(13, 1, 3, 0);
const SCC_VEC_D: VectorAddress = VectorAddress::new(13, 1, 4, 0);
const SCC_VEC_E: VectorAddress = VectorAddress::new(13, 1, 5, 0);

const SCC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    SCC_PLUGIN,
    name:         "kl-graph-scc-harness",
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
        local_node_key: key,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: SCC_EXEC,
        state_schema_hash: schema,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    }
}

fn reset() {
    gos_runtime::reset();
}

fn register_abcde() {
    gos_runtime::discover_plugin(SCC_MANIFEST).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_A, node_spec(SCC_KEY_A, SCC_ID_A, 0xE001)).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_B, node_spec(SCC_KEY_B, SCC_ID_B, 0xE002)).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_C, node_spec(SCC_KEY_C, SCC_ID_C, 0xE003)).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_D, node_spec(SCC_KEY_D, SCC_ID_D, 0xE004)).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_E, node_spec(SCC_KEY_E, SCC_ID_E, 0xE005)).unwrap();
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

fn scc() -> (Vec<VectorAddress>, Vec<u8>, usize) {
    let (nodes, labels, total, scc_count) = gos_runtime::graph_scc::<128>();
    (nodes[..total].to_vec(), labels[..total].to_vec(), scc_count)
}

/// Helper: for a given SCC index, collect all nodes belonging to it.
fn nodes_in_scc(nodes: &[VectorAddress], labels: &[u8], target: u8) -> Vec<VectorAddress> {
    nodes.iter().zip(labels.iter())
        .filter(|(_, &l)| l == target)
        .map(|(&v, _)| v)
        .collect()
}

// ── 1. Empty graph ────────────────────────────────────────────────────────────

#[test]
fn empty_graph_has_zero_components() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (nodes, _, scc_count) = scc();
    assert_eq!(scc_count, 0, "empty graph must have 0 SCCs");
    assert_eq!(nodes.len(), 0, "empty graph must have 0 nodes in output");
}

// ── 2. Single isolated node → 1 SCC of size 1 ────────────────────────────────

#[test]
fn single_node_is_one_scc() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(SCC_MANIFEST).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_A, node_spec(SCC_KEY_A, SCC_ID_A, 0xE001)).unwrap();
    let (nodes, labels, scc_count) = scc();
    assert_eq!(scc_count, 1, "single node must have 1 SCC");
    assert_eq!(nodes.len(), 1, "must have exactly 1 node in output");
    let members = nodes_in_scc(&nodes, &labels, 0);
    assert_eq!(members.len(), 1, "SCC #0 must have size 1");
    assert!(members.contains(&SCC_VEC_A), "SCC #0 must contain A");
}

// ── 3. Single node with self-loop → 1 SCC of size 1 ─────────────────────────

#[test]
fn self_loop_does_not_merge_sccs() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(SCC_MANIFEST).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_A, node_spec(SCC_KEY_A, SCC_ID_A, 0xE001)).unwrap();
    add_edge(SCC_ID_A, SCC_ID_A, "scc.aa");
    let (nodes, labels, scc_count) = scc();
    assert_eq!(scc_count, 1, "self-loop must still yield 1 SCC");
    let members = nodes_in_scc(&nodes, &labels, 0);
    assert_eq!(members.len(), 1, "self-loop SCC must have size 1");
    assert!(members.contains(&SCC_VEC_A));
}

// ── 4. Two-node mutual cycle A↔B → 1 SCC of size 2 ──────────────────────────

#[test]
fn two_node_mutual_cycle_is_one_scc() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(SCC_MANIFEST).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_A, node_spec(SCC_KEY_A, SCC_ID_A, 0xE001)).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_B, node_spec(SCC_KEY_B, SCC_ID_B, 0xE002)).unwrap();
    add_edge(SCC_ID_A, SCC_ID_B, "scc.ab1");
    add_edge(SCC_ID_B, SCC_ID_A, "scc.ba1");
    let (nodes, labels, scc_count) = scc();
    assert_eq!(scc_count, 1, "A↔B cycle must be 1 SCC");
    let members = nodes_in_scc(&nodes, &labels, 0);
    assert_eq!(members.len(), 2, "SCC must contain both A and B");
    assert!(members.contains(&SCC_VEC_A), "SCC must include A");
    assert!(members.contains(&SCC_VEC_B), "SCC must include B");
}

// ── 5. Linear chain A→B→C → 3 singleton SCCs ─────────────────────────────────

#[test]
fn linear_chain_gives_singleton_sccs() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(SCC_MANIFEST).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_A, node_spec(SCC_KEY_A, SCC_ID_A, 0xE001)).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_B, node_spec(SCC_KEY_B, SCC_ID_B, 0xE002)).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_C, node_spec(SCC_KEY_C, SCC_ID_C, 0xE003)).unwrap();
    add_edge(SCC_ID_A, SCC_ID_B, "scc.ab2");
    add_edge(SCC_ID_B, SCC_ID_C, "scc.bc2");
    let (nodes, _, scc_count) = scc();
    assert_eq!(scc_count, 3, "linear chain A→B→C must yield 3 singleton SCCs");
    assert_eq!(nodes.len(), 3, "all 3 nodes must appear");
    // scc_count == total → graph is a DAG
    assert_eq!(scc_count, nodes.len(), "scc_count == total nodes means DAG");
}

// ── 6. Triangle cycle A→B→C→A → 1 SCC of size 3 ─────────────────────────────

#[test]
fn triangle_cycle_is_one_scc_of_three() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(SCC_MANIFEST).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_A, node_spec(SCC_KEY_A, SCC_ID_A, 0xE001)).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_B, node_spec(SCC_KEY_B, SCC_ID_B, 0xE002)).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_C, node_spec(SCC_KEY_C, SCC_ID_C, 0xE003)).unwrap();
    add_edge(SCC_ID_A, SCC_ID_B, "scc.ab3");
    add_edge(SCC_ID_B, SCC_ID_C, "scc.bc3");
    add_edge(SCC_ID_C, SCC_ID_A, "scc.ca3");
    let (nodes, labels, scc_count) = scc();
    assert_eq!(scc_count, 1, "triangle A→B→C→A must be 1 SCC");
    assert_eq!(nodes.len(), 3);
    let members = nodes_in_scc(&nodes, &labels, 0);
    assert_eq!(members.len(), 3, "the single SCC must contain all 3 nodes");
    assert!(members.contains(&SCC_VEC_A));
    assert!(members.contains(&SCC_VEC_B));
    assert!(members.contains(&SCC_VEC_C));
}

// ── 7. Diamond DAG → 4 singleton SCCs, scc_count == total ────────────────────

#[test]
fn diamond_dag_gives_four_singleton_sccs() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(SCC_MANIFEST).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_A, node_spec(SCC_KEY_A, SCC_ID_A, 0xE001)).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_B, node_spec(SCC_KEY_B, SCC_ID_B, 0xE002)).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_C, node_spec(SCC_KEY_C, SCC_ID_C, 0xE003)).unwrap();
    gos_runtime::register_node(SCC_PLUGIN, SCC_VEC_D, node_spec(SCC_KEY_D, SCC_ID_D, 0xE004)).unwrap();
    // A → B, A → C, B → D, C → D
    add_edge(SCC_ID_A, SCC_ID_B, "scc.ab4");
    add_edge(SCC_ID_A, SCC_ID_C, "scc.ac4");
    add_edge(SCC_ID_B, SCC_ID_D, "scc.bd4");
    add_edge(SCC_ID_C, SCC_ID_D, "scc.cd4");
    let (nodes, _, scc_count) = scc();
    assert_eq!(scc_count, 4, "diamond DAG must yield 4 singleton SCCs");
    assert_eq!(nodes.len(), 4, "all 4 nodes must appear");
    // scc_count == total confirms DAG
    assert_eq!(scc_count, nodes.len());
}

// ── 8. Triangle + isolated node → 2 SCCs (sizes 3 and 1) ─────────────────────

#[test]
fn triangle_plus_isolated_gives_two_sccs() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_abcde();
    // Cycle: A→B→C→A; D and E are isolated
    add_edge(SCC_ID_A, SCC_ID_B, "scc.ab5");
    add_edge(SCC_ID_B, SCC_ID_C, "scc.bc5");
    add_edge(SCC_ID_C, SCC_ID_A, "scc.ca5");
    let (nodes, labels, scc_count) = scc();
    assert_eq!(nodes.len(), 5, "all 5 nodes must appear");
    // One SCC has size 3 (A,B,C), and D and E are singletons → 3 total SCCs
    assert_eq!(scc_count, 3, "triangle + 2 isolated → 3 SCCs");
    // Find the SCC that contains A — it must also contain B and C
    let label_a = labels[nodes.iter().position(|&v| v == SCC_VEC_A).expect("A missing")] ;
    let members_a = nodes_in_scc(&nodes, &labels, label_a);
    assert_eq!(members_a.len(), 3, "A's SCC must have 3 members (A, B, C)");
    assert!(members_a.contains(&SCC_VEC_B));
    assert!(members_a.contains(&SCC_VEC_C));
}

// ── 9. Two separate cycles → 2 SCCs ──────────────────────────────────────────

#[test]
fn two_separate_cycles_give_two_sccs() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_abcde();
    // Cycle 1: A↔B  (2 nodes)
    add_edge(SCC_ID_A, SCC_ID_B, "scc.ab6");
    add_edge(SCC_ID_B, SCC_ID_A, "scc.ba6");
    // Cycle 2: C→D→E→C  (3 nodes)
    add_edge(SCC_ID_C, SCC_ID_D, "scc.cd6");
    add_edge(SCC_ID_D, SCC_ID_E, "scc.de6");
    add_edge(SCC_ID_E, SCC_ID_C, "scc.ec6");
    let (nodes, labels, scc_count) = scc();
    assert_eq!(nodes.len(), 5, "all 5 nodes must appear");
    assert_eq!(scc_count, 2, "two separate cycles must yield exactly 2 SCCs");
    // Check the sizes: one SCC has 2 members, the other has 3
    let label_a = labels[nodes.iter().position(|&v| v == SCC_VEC_A).expect("A missing")];
    let label_c = labels[nodes.iter().position(|&v| v == SCC_VEC_C).expect("C missing")];
    assert_ne!(label_a, label_c, "A and C must be in different SCCs");
    let members_ab = nodes_in_scc(&nodes, &labels, label_a);
    let members_cde = nodes_in_scc(&nodes, &labels, label_c);
    assert_eq!(members_ab.len(), 2, "A↔B SCC must have 2 members");
    assert_eq!(members_cde.len(), 3, "C→D→E→C SCC must have 3 members");
}

// ── 10. scc_count == total confirms DAG ──────────────────────────────────────

#[test]
fn scc_count_equals_total_confirms_dag() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_abcde();
    // Pure DAG: A→B, A→C, B→D, C→D, D→E
    add_edge(SCC_ID_A, SCC_ID_B, "scc.ab7");
    add_edge(SCC_ID_A, SCC_ID_C, "scc.ac7");
    add_edge(SCC_ID_B, SCC_ID_D, "scc.bd7");
    add_edge(SCC_ID_C, SCC_ID_D, "scc.cd7");
    add_edge(SCC_ID_D, SCC_ID_E, "scc.de7");
    let (nodes, _, scc_count) = scc();
    assert_eq!(nodes.len(), 5, "all 5 nodes must appear");
    assert_eq!(scc_count, 5, "a DAG with 5 nodes must have exactly 5 singleton SCCs");
    // The canonical DAG check: scc_count == total
    assert_eq!(scc_count, nodes.len(), "scc_count == total ↔ graph is a DAG");
}
