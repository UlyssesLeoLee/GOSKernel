// gos-graph-eccentricity-harness — V2.41 eccentricity API tests
//
// Verifies `gos_runtime::graph_eccentricity` — per-node max shortest-path distance
// on the live directed graph, plus the graph radius and diameter.
//
// ecc[v] = max d(v, u)  for u reachable from v via directed edges (u ≠ v)
// Isolated nodes (no reachable neighbour)      → ecc[v] = 0
// radius   = min ecc[v] for non-isolated nodes  (0 if all isolated)
// diameter = max ecc[v]                          (0 if all isolated)
//
// OS analogy: `traceroute` worst-case hop count — which kernel node has the
// tightest guaranteed latency to all reachable peers?
//
//  1. Empty graph                   → total=0, radius=0, diameter=0
//  2. Single isolated node          → ecc[A]=0, radius=0, diameter=0
//  3. Two-node A→B                  → ecc[A]=1, ecc[B]=0; radius=1, diameter=1
//  4. Path A→B→C                    → ecc[A]=2, ecc[B]=1, ecc[C]=0; radius=1, diameter=2
//  5. Star A→{B,C,D}                → ecc[A]=1, leaves=0; radius=1, diameter=1
//  6. Directed 3-cycle A→B→C→A     → all ecc=2; radius=2, diameter=2
//  7. Diamond A→{B,C}→D            → ecc[A]=2, ecc[B/C]=1, ecc[D]=0; radius=1, diameter=2
//  8. Linear 5-chain A→B→C→D→E    → ecc[D]=1..ecc[A]=4; sorted ascending, E isolated
//  9. Disconnected {A→B} ∥ {C→D}  → ecc[A]=ecc[C]=1, sinks=0; radius=1, diameter=1
// 10. Self-loop A→A plus B→C       → ecc[A]=0, ecc[B]=1, ecc[C]=0; radius=1, diameter=1

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const ECC_PLUGIN: PluginId   = PluginId::from_ascii("KL_ECC0_00");
const ECC_EXEC:   ExecutorId = ExecutorId::from_ascii("ecc.exec");

const ECC_KEY_A: &str = "ecc.alpha";
const ECC_KEY_B: &str = "ecc.beta";
const ECC_KEY_C: &str = "ecc.gamma";
const ECC_KEY_D: &str = "ecc.delta";
const ECC_KEY_E: &str = "ecc.epsilon";

const ECC_ID_A: NodeId = derive_node_id(ECC_PLUGIN, ECC_KEY_A);
const ECC_ID_B: NodeId = derive_node_id(ECC_PLUGIN, ECC_KEY_B);
const ECC_ID_C: NodeId = derive_node_id(ECC_PLUGIN, ECC_KEY_C);
const ECC_ID_D: NodeId = derive_node_id(ECC_PLUGIN, ECC_KEY_D);
const ECC_ID_E: NodeId = derive_node_id(ECC_PLUGIN, ECC_KEY_E);

const ECC_VEC_A: VectorAddress = VectorAddress::new(18, 1, 1, 0);
const ECC_VEC_B: VectorAddress = VectorAddress::new(18, 1, 2, 0);
const ECC_VEC_C: VectorAddress = VectorAddress::new(18, 1, 3, 0);
const ECC_VEC_D: VectorAddress = VectorAddress::new(18, 1, 4, 0);
const ECC_VEC_E: VectorAddress = VectorAddress::new(18, 1, 5, 0);

const ECC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    ECC_PLUGIN,
    name:         "kl-graph-eccentricity-harness",
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
        executor_id:       ECC_EXEC,
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
    gos_runtime::discover_plugin(ECC_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(ECC_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

fn find_ecc(
    vecs: &[VectorAddress],
    ecc: &[u32],
    total: usize,
    target: VectorAddress,
) -> Option<u32> {
    for i in 0..total {
        if vecs[i] == target { return Some(ecc[i]); }
    }
    None
}

// ── 1. Empty graph → total=0, radius=0, diameter=0 ──────────────────────────

#[test]
fn empty_graph_eccentricity_total_is_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _ecc, total, radius, diameter) = gos_runtime::graph_eccentricity::<128>();
    assert_eq!(total,    0, "empty graph: 0 nodes");
    assert_eq!(radius,   0, "empty graph: radius=0");
    assert_eq!(diameter, 0, "empty graph: diameter=0");
}

// ── 2. Single isolated node → ecc=0, radius=0, diameter=0 ───────────────────

#[test]
fn isolated_node_has_zero_eccentricity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ECC_VEC_A, ECC_KEY_A, ECC_ID_A, 0xE001);
    let (vecs, ecc, total, radius, diameter) = gos_runtime::graph_eccentricity::<128>();
    assert_eq!(total, 1, "1 node");
    let a = find_ecc(&vecs, &ecc, total, ECC_VEC_A).expect("A must be in result");
    assert_eq!(a, 0, "isolated node: ecc=0 (no reachable nodes)");
    assert_eq!(radius,   0, "radius=0 (all isolated)");
    assert_eq!(diameter, 0, "diameter=0 (all isolated)");
}

// ── 3. Two-node A→B: ecc[A]=1, ecc[B]=0 ─────────────────────────────────────

#[test]
fn two_node_edge_eccentricity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ECC_VEC_A, ECC_KEY_A, ECC_ID_A, 0xE001);
    add_node(ECC_VEC_B, ECC_KEY_B, ECC_ID_B, 0xE002);
    add_edge(ECC_ID_A, ECC_ID_B, "ecc.ab.t3");
    let (vecs, ecc, total, radius, diameter) = gos_runtime::graph_eccentricity::<128>();
    assert_eq!(total, 2, "2 nodes");
    let a = find_ecc(&vecs, &ecc, total, ECC_VEC_A).unwrap();
    let b = find_ecc(&vecs, &ecc, total, ECC_VEC_B).unwrap();
    // A reaches B in 1 hop: ecc[A] = 1.
    assert_eq!(a, 1, "A→B: ecc[A]=1");
    // B is a sink: ecc[B] = 0 (isolated).
    assert_eq!(b, 0, "B: ecc=0 (sink)");
    assert_eq!(radius,   1, "radius=1");
    assert_eq!(diameter, 1, "diameter=1");
    // A (ecc=1, centre) must appear first; B (ecc=0, isolated) last.
    assert_eq!(vecs[0], ECC_VEC_A, "A (ecc=1, centre) must be first");
}

// ── 4. Path A→B→C: ecc[A]=2, ecc[B]=1, ecc[C]=0 ────────────────────────────

#[test]
fn path_abc_eccentricity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ECC_VEC_A, ECC_KEY_A, ECC_ID_A, 0xE001);
    add_node(ECC_VEC_B, ECC_KEY_B, ECC_ID_B, 0xE002);
    add_node(ECC_VEC_C, ECC_KEY_C, ECC_ID_C, 0xE003);
    add_edge(ECC_ID_A, ECC_ID_B, "ecc.ab.t4");
    add_edge(ECC_ID_B, ECC_ID_C, "ecc.bc.t4");
    let (vecs, ecc, total, radius, diameter) = gos_runtime::graph_eccentricity::<128>();
    assert_eq!(total, 3, "3 nodes");
    // A reaches B in 1 hop, C in 2 hops: max=2. ecc[A]=2.
    let a = find_ecc(&vecs, &ecc, total, ECC_VEC_A).unwrap();
    assert_eq!(a, 2, "A: ecc=2 (max hop to C)");
    // B reaches C in 1 hop: ecc[B]=1.
    let b = find_ecc(&vecs, &ecc, total, ECC_VEC_B).unwrap();
    assert_eq!(b, 1, "B: ecc=1 (max hop to C)");
    // C is a sink: ecc[C]=0.
    let c = find_ecc(&vecs, &ecc, total, ECC_VEC_C).unwrap();
    assert_eq!(c, 0, "C: ecc=0 (sink)");
    assert_eq!(radius,   1, "radius=1 (B is centre)");
    assert_eq!(diameter, 2, "diameter=2 (A is periphery)");
    // Sorted ascending: B(1), A(2), C(0→last).
    assert_eq!(vecs[0], ECC_VEC_B, "B (ecc=1, centre) must be first");
    assert_eq!(vecs[1], ECC_VEC_A, "A (ecc=2, periphery) must be second");
    assert_eq!(vecs[2], ECC_VEC_C, "C (ecc=0, isolated) must be last");
}

// ── 5. Star A→{B,C,D}: ecc[A]=1, leaves=0; radius=diameter=1 ────────────────

#[test]
fn star_center_eccentricity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ECC_VEC_A, ECC_KEY_A, ECC_ID_A, 0xE001);
    add_node(ECC_VEC_B, ECC_KEY_B, ECC_ID_B, 0xE002);
    add_node(ECC_VEC_C, ECC_KEY_C, ECC_ID_C, 0xE003);
    add_node(ECC_VEC_D, ECC_KEY_D, ECC_ID_D, 0xE004);
    add_edge(ECC_ID_A, ECC_ID_B, "ecc.ab.t5");
    add_edge(ECC_ID_A, ECC_ID_C, "ecc.ac.t5");
    add_edge(ECC_ID_A, ECC_ID_D, "ecc.ad.t5");
    let (vecs, ecc, total, radius, diameter) = gos_runtime::graph_eccentricity::<128>();
    assert_eq!(total, 4, "4 nodes");
    // A reaches B, C, D all in 1 hop: max=1. ecc[A]=1.
    let a = find_ecc(&vecs, &ecc, total, ECC_VEC_A).unwrap();
    assert_eq!(a, 1, "star centre A: ecc=1");
    let b = find_ecc(&vecs, &ecc, total, ECC_VEC_B).unwrap();
    let c = find_ecc(&vecs, &ecc, total, ECC_VEC_C).unwrap();
    let d = find_ecc(&vecs, &ecc, total, ECC_VEC_D).unwrap();
    assert_eq!(b, 0, "leaf B: ecc=0 (sink)");
    assert_eq!(c, 0, "leaf C: ecc=0 (sink)");
    assert_eq!(d, 0, "leaf D: ecc=0 (sink)");
    assert_eq!(radius,   1, "radius=1");
    assert_eq!(diameter, 1, "diameter=1");
    // A (ecc=1) must appear first; leaves (ecc=0) are last.
    assert_eq!(vecs[0], ECC_VEC_A, "A (ecc=1, centre) must be first");
}

// ── 6. Directed 3-cycle A→B→C→A: all ecc=2, radius=diameter=2 ───────────────

#[test]
fn directed_cycle_all_nodes_same_eccentricity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ECC_VEC_A, ECC_KEY_A, ECC_ID_A, 0xE001);
    add_node(ECC_VEC_B, ECC_KEY_B, ECC_ID_B, 0xE002);
    add_node(ECC_VEC_C, ECC_KEY_C, ECC_ID_C, 0xE003);
    add_edge(ECC_ID_A, ECC_ID_B, "ecc.ab.t6");
    add_edge(ECC_ID_B, ECC_ID_C, "ecc.bc.t6");
    add_edge(ECC_ID_C, ECC_ID_A, "ecc.ca.t6");
    let (vecs, ecc, total, radius, diameter) = gos_runtime::graph_eccentricity::<128>();
    assert_eq!(total, 3, "3 nodes in cycle");
    // BFS from A: d(A,B)=1, d(A,C)=2. max=2. ecc[A]=2.
    // By rotational symmetry: ecc[B]=ecc[C]=2.
    let a = find_ecc(&vecs, &ecc, total, ECC_VEC_A).unwrap();
    let b = find_ecc(&vecs, &ecc, total, ECC_VEC_B).unwrap();
    let c = find_ecc(&vecs, &ecc, total, ECC_VEC_C).unwrap();
    assert_eq!(a, 2, "A: ecc=2 (cycle symmetry)");
    assert_eq!(b, 2, "B: ecc=2 (cycle symmetry)");
    assert_eq!(c, 2, "C: ecc=2 (cycle symmetry)");
    // radius == diameter == 2: all nodes are simultaneously centre and periphery.
    assert_eq!(radius,   2, "radius=2");
    assert_eq!(diameter, 2, "diameter=2");
    // All three entries must have ecc=2.
    for i in 0..3 {
        assert_eq!(ecc[i], 2, "all entries must have ecc=2");
    }
}

// ── 7. Diamond A→{B,C}→D: ecc[A]=2, ecc[B/C]=1, ecc[D]=0 ──────────────────

#[test]
fn diamond_eccentricity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ECC_VEC_A, ECC_KEY_A, ECC_ID_A, 0xE001);
    add_node(ECC_VEC_B, ECC_KEY_B, ECC_ID_B, 0xE002);
    add_node(ECC_VEC_C, ECC_KEY_C, ECC_ID_C, 0xE003);
    add_node(ECC_VEC_D, ECC_KEY_D, ECC_ID_D, 0xE004);
    add_edge(ECC_ID_A, ECC_ID_B, "ecc.ab.t7");
    add_edge(ECC_ID_A, ECC_ID_C, "ecc.ac.t7");
    add_edge(ECC_ID_B, ECC_ID_D, "ecc.bd.t7");
    add_edge(ECC_ID_C, ECC_ID_D, "ecc.cd.t7");
    let (vecs, ecc, total, radius, diameter) = gos_runtime::graph_eccentricity::<128>();
    assert_eq!(total, 4, "4 nodes");
    // A: d(A,B)=1, d(A,C)=1, d(A,D)=2. max=2. ecc[A]=2.
    let a = find_ecc(&vecs, &ecc, total, ECC_VEC_A).unwrap();
    assert_eq!(a, 2, "A: ecc=2 (reaches D in 2 hops)");
    // B: d(B,D)=1. max=1. ecc[B]=1.
    let b = find_ecc(&vecs, &ecc, total, ECC_VEC_B).unwrap();
    assert_eq!(b, 1, "B: ecc=1 (reaches D in 1 hop)");
    // C: d(C,D)=1. max=1. ecc[C]=1.
    let c = find_ecc(&vecs, &ecc, total, ECC_VEC_C).unwrap();
    assert_eq!(c, 1, "C: ecc=1 (reaches D in 1 hop)");
    // D is a sink: ecc[D]=0.
    let d = find_ecc(&vecs, &ecc, total, ECC_VEC_D).unwrap();
    assert_eq!(d, 0, "D: ecc=0 (sink)");
    assert_eq!(radius,   1, "radius=1 (B and C are centre)");
    assert_eq!(diameter, 2, "diameter=2 (A is periphery)");
    // B or C must be first (both ecc=1, centre).
    assert!(
        vecs[0] == ECC_VEC_B || vecs[0] == ECC_VEC_C,
        "first entry must be B or C (both ecc=1, centre)"
    );
    // D (ecc=0, isolated) must be last.
    assert_eq!(vecs[3], ECC_VEC_D, "D (ecc=0, isolated) must be last");
}

// ── 8. Linear 5-chain A→B→C→D→E: ascending ecc sort ────────────────────────

#[test]
fn linear_five_node_chain_eccentricity_ordering() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ECC_VEC_A, ECC_KEY_A, ECC_ID_A, 0xE001);
    add_node(ECC_VEC_B, ECC_KEY_B, ECC_ID_B, 0xE002);
    add_node(ECC_VEC_C, ECC_KEY_C, ECC_ID_C, 0xE003);
    add_node(ECC_VEC_D, ECC_KEY_D, ECC_ID_D, 0xE004);
    add_node(ECC_VEC_E, ECC_KEY_E, ECC_ID_E, 0xE005);
    add_edge(ECC_ID_A, ECC_ID_B, "ecc.ab.t8");
    add_edge(ECC_ID_B, ECC_ID_C, "ecc.bc.t8");
    add_edge(ECC_ID_C, ECC_ID_D, "ecc.cd.t8");
    add_edge(ECC_ID_D, ECC_ID_E, "ecc.de.t8");
    let (vecs, ecc, total, radius, diameter) = gos_runtime::graph_eccentricity::<128>();
    assert_eq!(total, 5, "5 nodes");
    // ecc[D]=1 (reaches E), ecc[C]=2, ecc[B]=3, ecc[A]=4, ecc[E]=0 (sink).
    let a = find_ecc(&vecs, &ecc, total, ECC_VEC_A).unwrap();
    let b = find_ecc(&vecs, &ecc, total, ECC_VEC_B).unwrap();
    let c = find_ecc(&vecs, &ecc, total, ECC_VEC_C).unwrap();
    let d = find_ecc(&vecs, &ecc, total, ECC_VEC_D).unwrap();
    let e = find_ecc(&vecs, &ecc, total, ECC_VEC_E).unwrap();
    assert_eq!(d, 1, "D: ecc=1 (reaches E in 1 hop)");
    assert_eq!(c, 2, "C: ecc=2 (reaches E in 2 hops)");
    assert_eq!(b, 3, "B: ecc=3 (reaches E in 3 hops)");
    assert_eq!(a, 4, "A: ecc=4 (reaches E in 4 hops)");
    assert_eq!(e, 0, "E: ecc=0 (sink)");
    assert_eq!(radius,   1, "radius=1 (D is centre)");
    assert_eq!(diameter, 4, "diameter=4 (A is periphery)");
    // Sorted ascending with E (isolated) last: D, C, B, A, E.
    assert_eq!(vecs[0], ECC_VEC_D, "D (ecc=1, centre) must be first");
    assert_eq!(vecs[4], ECC_VEC_E, "E (ecc=0, isolated) must be last");
    // Non-isolated entries must be in ascending order.
    for i in 0..3 {
        assert!(
            ecc[i] <= ecc[i + 1],
            "output[{}]={} must be <= output[{}]={} (ascending among non-isolated)",
            i, ecc[i], i + 1, ecc[i + 1]
        );
    }
}

// ── 9. Disconnected {A→B} ∥ {C→D}: ecc[A]=ecc[C]=1, sinks=0 ────────────────

#[test]
fn disconnected_pairs_eccentricity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ECC_VEC_A, ECC_KEY_A, ECC_ID_A, 0xE001);
    add_node(ECC_VEC_B, ECC_KEY_B, ECC_ID_B, 0xE002);
    add_node(ECC_VEC_C, ECC_KEY_C, ECC_ID_C, 0xE003);
    add_node(ECC_VEC_D, ECC_KEY_D, ECC_ID_D, 0xE004);
    add_edge(ECC_ID_A, ECC_ID_B, "ecc.ab.t9");
    add_edge(ECC_ID_C, ECC_ID_D, "ecc.cd.t9");
    let (vecs, ecc, total, radius, diameter) = gos_runtime::graph_eccentricity::<128>();
    assert_eq!(total, 4, "4 nodes in 2 disconnected components");
    let a = find_ecc(&vecs, &ecc, total, ECC_VEC_A).unwrap();
    let b = find_ecc(&vecs, &ecc, total, ECC_VEC_B).unwrap();
    let c = find_ecc(&vecs, &ecc, total, ECC_VEC_C).unwrap();
    let d = find_ecc(&vecs, &ecc, total, ECC_VEC_D).unwrap();
    assert_eq!(a, 1, "A→B: ecc[A]=1");
    assert_eq!(b, 0, "B: ecc=0 (sink)");
    assert_eq!(c, 1, "C→D: ecc[C]=1");
    assert_eq!(d, 0, "D: ecc=0 (sink)");
    assert_eq!(radius,   1, "radius=1");
    assert_eq!(diameter, 1, "diameter=1");
}

// ── 10. Self-loop A→A plus B→C: self-loop contributes no eccentricity ────────

#[test]
fn self_loop_does_not_contribute_to_eccentricity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ECC_VEC_A, ECC_KEY_A, ECC_ID_A, 0xE001);
    add_node(ECC_VEC_B, ECC_KEY_B, ECC_ID_B, 0xE002);
    add_node(ECC_VEC_C, ECC_KEY_C, ECC_ID_C, 0xE003);
    add_edge(ECC_ID_A, ECC_ID_A, "ecc.aa.t10"); // self-loop
    add_edge(ECC_ID_B, ECC_ID_C, "ecc.bc.t10");
    let (vecs, ecc, total, radius, diameter) = gos_runtime::graph_eccentricity::<128>();
    assert_eq!(total, 3, "3 nodes");
    // A has only a self-loop: BFS finds no other nodes. ecc[A]=0.
    let a = find_ecc(&vecs, &ecc, total, ECC_VEC_A).unwrap();
    assert_eq!(a, 0, "A: ecc=0 (self-loop, no other reachable node)");
    // B reaches C in 1 hop: ecc[B]=1.
    let b = find_ecc(&vecs, &ecc, total, ECC_VEC_B).unwrap();
    assert_eq!(b, 1, "B: ecc=1 (reaches C in 1 hop)");
    // C is a sink: ecc[C]=0.
    let c = find_ecc(&vecs, &ecc, total, ECC_VEC_C).unwrap();
    assert_eq!(c, 0, "C: ecc=0 (sink)");
    assert_eq!(radius,   1, "radius=1 (B is the only non-isolated node)");
    assert_eq!(diameter, 1, "diameter=1");
    // B (ecc=1) must appear first; A and C (ecc=0) are isolated/last.
    assert_eq!(vecs[0], ECC_VEC_B, "B (ecc=1, centre) must be first");
}
