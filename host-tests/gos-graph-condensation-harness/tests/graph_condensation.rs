// gos-graph-condensation-harness — V2.35 condensation DAG API tests
//
// Verifies `gos_runtime::graph_condensation` — the condensation DAG built by
// collapsing each SCC into a single super-node and recording inter-SCC edges.
// The condensation is always a DAG regardless of cycles in the source graph.
// Analogous to `sccmap -F` (Graphviz) or cargo's inter-package dependency DAG.
//
//  1. Empty graph → 0 components, 0 condensation edges.
//  2. Single isolated node → 1 component, 0 condensation edges.
//  3. Two-node mutual cycle A↔B → 1 component, 0 condensation edges.
//  4. Linear chain A→B→C → 3 components, 2 condensation edges.
//  5. Triangle cycle A→B→C→A → 1 component, 0 condensation edges.
//  6. Triangle + outgoing edge to D → 2 components, 1 condensation edge.
//  7. Diamond DAG A→B, A→C, B→D, C→D → 4 components, 4 condensation edges.
//  8. Multiple parallel edges between SCC pair → only 1 condensation edge.
//  9. Chain with embedded cycle: A↔B, B→C, C→D → 3 components, 2 edges.
// 10. Condensation is a DAG: no self-edges, verify adjacency matrix symmetry rule.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const COND_PLUGIN: PluginId   = PluginId::from_ascii("KL_CND_H00");
const COND_EXEC:   ExecutorId = ExecutorId::from_ascii("cond.exec");

const COND_KEY_A: &str = "cond.alpha";
const COND_KEY_B: &str = "cond.beta";
const COND_KEY_C: &str = "cond.gamma";
const COND_KEY_D: &str = "cond.delta";
const COND_KEY_E: &str = "cond.epsilon";

const COND_ID_A: NodeId = derive_node_id(COND_PLUGIN, COND_KEY_A);
const COND_ID_B: NodeId = derive_node_id(COND_PLUGIN, COND_KEY_B);
const COND_ID_C: NodeId = derive_node_id(COND_PLUGIN, COND_KEY_C);
const COND_ID_D: NodeId = derive_node_id(COND_PLUGIN, COND_KEY_D);
const COND_ID_E: NodeId = derive_node_id(COND_PLUGIN, COND_KEY_E);

const COND_VEC_A: VectorAddress = VectorAddress::new(14, 1, 1, 0);
const COND_VEC_B: VectorAddress = VectorAddress::new(14, 1, 2, 0);
const COND_VEC_C: VectorAddress = VectorAddress::new(14, 1, 3, 0);
const COND_VEC_D: VectorAddress = VectorAddress::new(14, 1, 4, 0);
const COND_VEC_E: VectorAddress = VectorAddress::new(14, 1, 5, 0);

const COND_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    COND_PLUGIN,
    name:         "kl-graph-condensation-harness",
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
        executor_id: COND_EXEC,
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
    gos_runtime::discover_plugin(COND_MANIFEST).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_A, node_spec(COND_KEY_A, COND_ID_A, 0xF001)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_B, node_spec(COND_KEY_B, COND_ID_B, 0xF002)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_C, node_spec(COND_KEY_C, COND_ID_C, 0xF003)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_D, node_spec(COND_KEY_D, COND_ID_D, 0xF004)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_E, node_spec(COND_KEY_E, COND_ID_E, 0xF005)).unwrap();
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

/// Unwrap the condensation output into usable slices.
fn condensation() -> (Vec<VectorAddress>, Vec<u8>, usize, [u128; 128], usize) {
    let (nodes, labels, total, scc_count, adj, cond_edges) =
        gos_runtime::graph_condensation::<128>();
    (nodes[..total].to_vec(), labels[..total].to_vec(), scc_count, adj, cond_edges)
}

/// Find the SCC label assigned to a given vector address.
fn label_of(nodes: &[VectorAddress], labels: &[u8], vec: VectorAddress) -> u8 {
    nodes.iter().zip(labels.iter())
        .find(|(&v, _)| v == vec)
        .map(|(_, &l)| l)
        .expect("node not found in condensation output")
}

/// Check whether condensation edge from_scc → to_scc exists in adj.
fn has_cond_edge(adj: &[u128; 128], from: u8, to: u8) -> bool {
    let fi = from as usize;
    let ti = to as usize;
    fi < 128 && ti < 128 && (adj[fi] >> ti) & 1 == 1
}

// ── 1. Empty graph ────────────────────────────────────────────────────────────

#[test]
fn empty_graph_has_no_condensation() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (nodes, _, scc_count, _, cond_edges) = condensation();
    assert_eq!(scc_count, 0, "empty graph: 0 components");
    assert_eq!(nodes.len(), 0, "empty graph: 0 nodes");
    assert_eq!(cond_edges, 0, "empty graph: 0 condensation edges");
}

// ── 2. Single isolated node → 1 component, 0 condensation edges ───────────────

#[test]
fn single_node_one_component_zero_cond_edges() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(COND_MANIFEST).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_A, node_spec(COND_KEY_A, COND_ID_A, 0xF001)).unwrap();
    let (nodes, _, scc_count, _, cond_edges) = condensation();
    assert_eq!(scc_count, 1, "single node: 1 component");
    assert_eq!(nodes.len(), 1, "single node: 1 entry");
    assert_eq!(cond_edges, 0, "single node: 0 condensation edges");
}

// ── 3. Two-node mutual cycle A↔B → 1 component, 0 condensation edges ─────────

#[test]
fn mutual_cycle_one_component_zero_cond_edges() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(COND_MANIFEST).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_A, node_spec(COND_KEY_A, COND_ID_A, 0xF001)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_B, node_spec(COND_KEY_B, COND_ID_B, 0xF002)).unwrap();
    add_edge(COND_ID_A, COND_ID_B, "cond.ab.t3a");
    add_edge(COND_ID_B, COND_ID_A, "cond.ba.t3b");
    let (nodes, labels, scc_count, adj, cond_edges) = condensation();
    assert_eq!(scc_count, 1, "A↔B cycle: 1 component");
    assert_eq!(nodes.len(), 2, "2 nodes");
    assert_eq!(cond_edges, 0, "intra-SCC edges don't create condensation edges");
    // All nodes must share the same label (both in component 0).
    let la = label_of(&nodes, &labels, COND_VEC_A);
    let lb = label_of(&nodes, &labels, COND_VEC_B);
    assert_eq!(la, lb, "A and B must be in the same SCC");
    // The adjacency row for the single SCC must be 0 (no self-edges).
    assert_eq!(adj[la as usize], 0, "no condensation self-edges");
}

// ── 4. Linear chain A→B→C → 3 components, 2 condensation edges ──────────────

#[test]
fn linear_chain_three_components_two_cond_edges() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(COND_MANIFEST).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_A, node_spec(COND_KEY_A, COND_ID_A, 0xF001)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_B, node_spec(COND_KEY_B, COND_ID_B, 0xF002)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_C, node_spec(COND_KEY_C, COND_ID_C, 0xF003)).unwrap();
    add_edge(COND_ID_A, COND_ID_B, "cond.ab.t4");
    add_edge(COND_ID_B, COND_ID_C, "cond.bc.t4");
    let (nodes, labels, scc_count, adj, cond_edges) = condensation();
    assert_eq!(scc_count, 3, "A→B→C: 3 singleton SCCs");
    assert_eq!(nodes.len(), 3);
    assert_eq!(cond_edges, 2, "2 condensation edges (A→B and B→C in condensation)");
    // All labels distinct.
    let la = label_of(&nodes, &labels, COND_VEC_A);
    let lb = label_of(&nodes, &labels, COND_VEC_B);
    let lc = label_of(&nodes, &labels, COND_VEC_C);
    assert_ne!(la, lb);
    assert_ne!(lb, lc);
    assert_ne!(la, lc);
    // Chain structure: A→B and B→C must appear in condensation adj.
    assert!(has_cond_edge(&adj, la, lb), "condensation edge A→B missing");
    assert!(has_cond_edge(&adj, lb, lc), "condensation edge B→C missing");
    // No reverse condensation edges.
    assert!(!has_cond_edge(&adj, lb, la), "no reverse condensation edge B→A");
    assert!(!has_cond_edge(&adj, lc, lb), "no reverse condensation edge C→B");
}

// ── 5. Triangle cycle A→B→C→A → 1 component, 0 condensation edges ────────────

#[test]
fn triangle_cycle_one_component_zero_cond_edges() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(COND_MANIFEST).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_A, node_spec(COND_KEY_A, COND_ID_A, 0xF001)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_B, node_spec(COND_KEY_B, COND_ID_B, 0xF002)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_C, node_spec(COND_KEY_C, COND_ID_C, 0xF003)).unwrap();
    add_edge(COND_ID_A, COND_ID_B, "cond.ab.t5");
    add_edge(COND_ID_B, COND_ID_C, "cond.bc.t5");
    add_edge(COND_ID_C, COND_ID_A, "cond.ca.t5");
    let (nodes, _, scc_count, adj, cond_edges) = condensation();
    assert_eq!(scc_count, 1, "triangle A→B→C→A: 1 component");
    assert_eq!(nodes.len(), 3);
    assert_eq!(cond_edges, 0, "all edges are intra-SCC — no condensation edges");
    // No condensation self-edges on the single component.
    assert_eq!(adj[0], 0, "no condensation edges from SCC 0");
}

// ── 6. Triangle + outgoing edge to D → 2 components, 1 condensation edge ──────

#[test]
fn triangle_plus_exit_gives_one_cond_edge() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(COND_MANIFEST).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_A, node_spec(COND_KEY_A, COND_ID_A, 0xF001)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_B, node_spec(COND_KEY_B, COND_ID_B, 0xF002)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_C, node_spec(COND_KEY_C, COND_ID_C, 0xF003)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_D, node_spec(COND_KEY_D, COND_ID_D, 0xF004)).unwrap();
    // Triangle A→B→C→A
    add_edge(COND_ID_A, COND_ID_B, "cond.ab.t6");
    add_edge(COND_ID_B, COND_ID_C, "cond.bc.t6");
    add_edge(COND_ID_C, COND_ID_A, "cond.ca.t6");
    // Outgoing cross-SCC edge: A→D
    add_edge(COND_ID_A, COND_ID_D, "cond.ad.t6");
    let (nodes, labels, scc_count, adj, cond_edges) = condensation();
    assert_eq!(scc_count, 2, "triangle + singleton D: 2 components");
    assert_eq!(nodes.len(), 4);
    assert_eq!(cond_edges, 1, "exactly 1 condensation edge (triangle SCC → D SCC)");
    // Triangle SCC contains A, B, C; D is its own SCC.
    let la = label_of(&nodes, &labels, COND_VEC_A);
    let ld = label_of(&nodes, &labels, COND_VEC_D);
    assert_ne!(la, ld, "A and D in different SCCs");
    assert!(has_cond_edge(&adj, la, ld), "condensation edge triangle→D must exist");
    assert!(!has_cond_edge(&adj, ld, la), "no reverse condensation edge D→triangle");
}

// ── 7. Diamond DAG → 4 components, 4 condensation edges ──────────────────────

#[test]
fn diamond_dag_four_components_four_cond_edges() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(COND_MANIFEST).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_A, node_spec(COND_KEY_A, COND_ID_A, 0xF001)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_B, node_spec(COND_KEY_B, COND_ID_B, 0xF002)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_C, node_spec(COND_KEY_C, COND_ID_C, 0xF003)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_D, node_spec(COND_KEY_D, COND_ID_D, 0xF004)).unwrap();
    // A → B, A → C, B → D, C → D
    add_edge(COND_ID_A, COND_ID_B, "cond.ab.t7");
    add_edge(COND_ID_A, COND_ID_C, "cond.ac.t7");
    add_edge(COND_ID_B, COND_ID_D, "cond.bd.t7");
    add_edge(COND_ID_C, COND_ID_D, "cond.cd.t7");
    let (nodes, labels, scc_count, adj, cond_edges) = condensation();
    assert_eq!(scc_count, 4, "diamond DAG: 4 singleton SCCs");
    assert_eq!(nodes.len(), 4);
    assert_eq!(cond_edges, 4, "4 condensation edges (A→B, A→C, B→D, C→D)");
    let la = label_of(&nodes, &labels, COND_VEC_A);
    let lb = label_of(&nodes, &labels, COND_VEC_B);
    let lc = label_of(&nodes, &labels, COND_VEC_C);
    let ld = label_of(&nodes, &labels, COND_VEC_D);
    assert!(has_cond_edge(&adj, la, lb), "condensation A→B");
    assert!(has_cond_edge(&adj, la, lc), "condensation A→C");
    assert!(has_cond_edge(&adj, lb, ld), "condensation B→D");
    assert!(has_cond_edge(&adj, lc, ld), "condensation C→D");
}

// ── 8. Multiple parallel edges between SCC pair → only 1 condensation edge ───

#[test]
fn parallel_inter_scc_edges_deduplicated() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(COND_MANIFEST).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_A, node_spec(COND_KEY_A, COND_ID_A, 0xF001)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_B, node_spec(COND_KEY_B, COND_ID_B, 0xF002)).unwrap();
    // Two parallel edges A→B (distinct edge keys so register_edge accepts both).
    add_edge(COND_ID_A, COND_ID_B, "cond.ab.t8a");
    add_edge(COND_ID_A, COND_ID_B, "cond.ab.t8b");
    let (nodes, labels, scc_count, adj, cond_edges) = condensation();
    assert_eq!(scc_count, 2, "A and B are separate singleton SCCs");
    assert_eq!(cond_edges, 1, "two parallel edges collapse to 1 condensation edge");
    let la = label_of(&nodes, &labels, COND_VEC_A);
    let lb = label_of(&nodes, &labels, COND_VEC_B);
    assert!(has_cond_edge(&adj, la, lb), "condensation edge A→B must exist");
    assert!(!has_cond_edge(&adj, lb, la), "no reverse condensation edge");
}

// ── 9. Chain with embedded cycle: A↔B, B→C, C→D → 3 components, 2 edges ─────

#[test]
fn chain_with_cycle_gives_three_components_two_edges() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(COND_MANIFEST).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_A, node_spec(COND_KEY_A, COND_ID_A, 0xF001)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_B, node_spec(COND_KEY_B, COND_ID_B, 0xF002)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_C, node_spec(COND_KEY_C, COND_ID_C, 0xF003)).unwrap();
    gos_runtime::register_node(COND_PLUGIN, COND_VEC_D, node_spec(COND_KEY_D, COND_ID_D, 0xF004)).unwrap();
    // Cycle A↔B, then chain B→C→D.
    add_edge(COND_ID_A, COND_ID_B, "cond.ab.t9a");
    add_edge(COND_ID_B, COND_ID_A, "cond.ba.t9b");
    add_edge(COND_ID_B, COND_ID_C, "cond.bc.t9");
    add_edge(COND_ID_C, COND_ID_D, "cond.cd.t9");
    let (nodes, labels, scc_count, adj, cond_edges) = condensation();
    // SCCs: {A,B} (cycle), {C}, {D} → 3 components.
    assert_eq!(scc_count, 3, "AB-cycle + C + D gives 3 components");
    assert_eq!(nodes.len(), 4);
    assert_eq!(cond_edges, 2, "condensation edges: AB-scc to C and C to D");
    let la = label_of(&nodes, &labels, COND_VEC_A);
    let lb = label_of(&nodes, &labels, COND_VEC_B);
    let lc = label_of(&nodes, &labels, COND_VEC_C);
    let ld = label_of(&nodes, &labels, COND_VEC_D);
    assert_eq!(la, lb, "A and B share the same SCC (cycle)");
    assert_ne!(la, lc);
    assert_ne!(lc, ld);
    assert!(has_cond_edge(&adj, la, lc), "condensation AB-scc to C");
    assert!(has_cond_edge(&adj, lc, ld), "condensation C to D");
}

// ── 10. Condensation is a DAG: no self-edges; adj is acyclic for a chain ──────

#[test]
fn condensation_is_always_a_dag() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_abcde();
    // Mixed: A→B→A cycle, C→D→E chain.
    add_edge(COND_ID_A, COND_ID_B, "cond.ab.t10");
    add_edge(COND_ID_B, COND_ID_A, "cond.ba.t10");
    add_edge(COND_ID_C, COND_ID_D, "cond.cd.t10");
    add_edge(COND_ID_D, COND_ID_E, "cond.de.t10");
    let (nodes, labels, scc_count, adj, _) = condensation();
    // 3 SCCs: {A,B}, {C}, {D}, {E} — wait, E is isolated so 4 SCCs total.
    // Let's verify: A↔B is 1 SCC; C, D, E are singletons (3); total = 4.
    assert_eq!(scc_count, 4, "AB-cycle plus C, D, E gives 4 SCCs");
    assert_eq!(nodes.len(), 5);

    // No self-edges in condensation (would violate DAG property).
    for i in 0..scc_count {
        if i < 128 {
            let self_bit = 1u128 << i;
            assert_eq!(adj[i] & self_bit, 0, "SCC {} must not have a self-edge in condensation", i);
        }
    }

    // For the chain C→D→E: verify condensation edges exist but no reverse.
    let lc = label_of(&nodes, &labels, COND_VEC_C);
    let ld = label_of(&nodes, &labels, COND_VEC_D);
    let le = label_of(&nodes, &labels, COND_VEC_E);
    assert!(has_cond_edge(&adj, lc, ld), "condensation C→D");
    assert!(has_cond_edge(&adj, ld, le), "condensation D→E");
    assert!(!has_cond_edge(&adj, ld, lc), "no reverse condensation D→C");
    assert!(!has_cond_edge(&adj, le, ld), "no reverse condensation E→D");
}
