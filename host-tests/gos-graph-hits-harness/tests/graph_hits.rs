// gos-graph-hits-harness — V2.44 HITS hub/authority API tests
//
// Verifies `gos_runtime::graph_hits` — Kleinberg HITS hub and authority scores.
//
// Algorithm:
//   new_a[v] = Σ_{u→v} h[u]   (authority = sum of in-neighbor hub scores)
//   new_h[v] = Σ_{v→w} a[w]   (hub = sum of out-neighbor authority scores)
//   Both normalised to max=SCALE=1_000_000 after each of 20 iterations.
//
// Converged values:
//   Isolated node:              hub=0, auth=0
//   Pure source A→B:            hub[A]=1M, auth[A]=0; hub[B]=0, auth[B]=1M
//   Chain A→B→C:                auth[A]=0, hub[C]=0; all others = 1M
//   Star-out A→{B,C,D}:         hub[A]=1M, auth[B/C/D]=1M
//   Star-in {A,B,C}→D:          hub[A/B/C]=1M, auth[D]=1M
//   Mutual A↔B:                 all = 1M (hub+authority)
//   3-cycle A→B→C→A:            all = 1M (hub+authority)
//   Bipartite {A,B}→{C,D}:      hub[A/B]=1M, auth[C/D]=1M
//
// OS analogy: vmstat/top bipartite — which nodes are the best signal-forwarders
// (hub) vs the most-cited destinations (authority)?
//
//  1. Empty graph                          → total=0
//  2. Single isolated node                 → hub=0, auth=0
//  3. Single edge A→B                      → hub[A]=1M, auth[B]=1M; A hub, B authority
//  4. Chain A→B→C                          → A: pure hub; C: pure authority; B: both
//  5. Star-out hub A→{B,C,D}              → hub[A]=1M; auth[B/C/D]=1M
//  6. Star-in authority {A,B,C}→D         → hub[A/B/C]=1M; auth[D]=1M
//  7. Mutual 2-cycle A↔B                   → hub=auth=1M for both (hub+authority)
//  8. 3-cycle A→B→C→A                      → all hub=auth=1M
//  9. Bipartite hub-authority {A,B}→{C,D} → hub[A/B]=1M; auth[C/D]=1M; isolated cross
// 10. Sort order                            → auth[0] ≥ auth[1] ≥ ... descending

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const HITS_PLUGIN: PluginId   = PluginId::from_ascii("KL_HTS0_00");
const HITS_EXEC:   ExecutorId = ExecutorId::from_ascii("hits.exec0");

const HITS_KEY_A: &str = "hits.alpha";
const HITS_KEY_B: &str = "hits.beta";
const HITS_KEY_C: &str = "hits.gamma";
const HITS_KEY_D: &str = "hits.delta";

const HITS_ID_A: NodeId = derive_node_id(HITS_PLUGIN, HITS_KEY_A);
const HITS_ID_B: NodeId = derive_node_id(HITS_PLUGIN, HITS_KEY_B);
const HITS_ID_C: NodeId = derive_node_id(HITS_PLUGIN, HITS_KEY_C);
const HITS_ID_D: NodeId = derive_node_id(HITS_PLUGIN, HITS_KEY_D);

const HITS_VEC_A: VectorAddress = VectorAddress::new(21, 1, 1, 0);
const HITS_VEC_B: VectorAddress = VectorAddress::new(21, 1, 2, 0);
const HITS_VEC_C: VectorAddress = VectorAddress::new(21, 1, 3, 0);
const HITS_VEC_D: VectorAddress = VectorAddress::new(21, 1, 4, 0);

const HITS_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    HITS_PLUGIN,
    name:         "kl-graph-hits-harness",
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
        executor_id:       HITS_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(HITS_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(HITS_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

fn find_hub(vecs: &[VectorAddress], hub: &[u32], total: usize, target: VectorAddress) -> Option<u32> {
    for i in 0..total { if vecs[i] == target { return Some(hub[i]); } }
    None
}

fn find_auth(vecs: &[VectorAddress], auth: &[u32], total: usize, target: VectorAddress) -> Option<u32> {
    for i in 0..total { if vecs[i] == target { return Some(auth[i]); } }
    None
}

// ── 1. Empty graph → total=0 ─────────────────────────────────────────────────

#[test]
fn empty_graph_hits_total_is_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _hub, _auth, total) = gos_runtime::graph_hits::<128>();
    assert_eq!(total, 0, "empty graph: 0 nodes");
}

// ── 2. Single isolated node → hub=0, auth=0 ──────────────────────────────────

#[test]
fn isolated_node_has_zero_hub_and_auth() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HITS_VEC_A, HITS_KEY_A, HITS_ID_A, 0xB001);
    let (vecs, hub, auth, total) = gos_runtime::graph_hits::<128>();
    assert_eq!(total, 1, "1 node");
    let h = find_hub(&vecs, &hub, total, HITS_VEC_A).expect("A must appear");
    let a = find_auth(&vecs, &auth, total, HITS_VEC_A).expect("A must appear");
    assert_eq!(h, 0, "isolated node: no out-edges → hub = 0");
    assert_eq!(a, 0, "isolated node: no in-edges → auth = 0");
}

// ── 3. Single edge A→B: hub[A]=1M, auth[B]=1M; A is hub, B is authority ─────

#[test]
fn single_edge_hub_and_authority_roles() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HITS_VEC_A, HITS_KEY_A, HITS_ID_A, 0xB001);
    add_node(HITS_VEC_B, HITS_KEY_B, HITS_ID_B, 0xB002);
    add_edge(HITS_ID_A, HITS_ID_B, "hits.ab.t3");
    let (vecs, hub, auth, total) = gos_runtime::graph_hits::<128>();
    assert_eq!(total, 2, "2 nodes");
    let h_a = find_hub(&vecs, &hub, total, HITS_VEC_A).unwrap();
    let a_a = find_auth(&vecs, &auth, total, HITS_VEC_A).unwrap();
    let h_b = find_hub(&vecs, &hub, total, HITS_VEC_B).unwrap();
    let a_b = find_auth(&vecs, &auth, total, HITS_VEC_B).unwrap();
    // A has no in-edges → authority = 0; points to authority B → hub = 1M.
    assert_eq!(a_a, 0,         "A: no in-edges → auth = 0");
    assert_eq!(h_a, 1_000_000, "A: points to B (authority) → hub = 1M");
    // B has no out-edges → hub = 0; cited by hub A → authority = 1M.
    assert_eq!(h_b, 0,         "B: no out-edges → hub = 0");
    assert_eq!(a_b, 1_000_000, "B: cited by A (hub) → auth = 1M");
    // Sorted descending by authority: B first (a=1M), A second (a=0).
    assert_eq!(vecs[0], HITS_VEC_B, "B (auth=1M) must be sorted first");
}

// ── 4. Chain A→B→C: A=pure hub, C=pure authority, B=both ─────────────────────

#[test]
fn chain_hub_relay_authority_roles() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HITS_VEC_A, HITS_KEY_A, HITS_ID_A, 0xB001);
    add_node(HITS_VEC_B, HITS_KEY_B, HITS_ID_B, 0xB002);
    add_node(HITS_VEC_C, HITS_KEY_C, HITS_ID_C, 0xB003);
    add_edge(HITS_ID_A, HITS_ID_B, "hits.ab.t4");
    add_edge(HITS_ID_B, HITS_ID_C, "hits.bc.t4");
    let (vecs, hub, auth, total) = gos_runtime::graph_hits::<128>();
    assert_eq!(total, 3, "3 nodes");
    let h_a = find_hub(&vecs, &hub, total, HITS_VEC_A).unwrap();
    let a_a = find_auth(&vecs, &auth, total, HITS_VEC_A).unwrap();
    let h_b = find_hub(&vecs, &hub, total, HITS_VEC_B).unwrap();
    let a_b = find_auth(&vecs, &auth, total, HITS_VEC_B).unwrap();
    let h_c = find_hub(&vecs, &hub, total, HITS_VEC_C).unwrap();
    let a_c = find_auth(&vecs, &auth, total, HITS_VEC_C).unwrap();
    // A: pure hub (out-edge to B, no in-edges).
    assert_eq!(a_a, 0,         "A: no in-edges → auth = 0");
    assert_eq!(h_a, 1_000_000, "A: points to B → hub = 1M");
    // C: pure authority (in-edge from B, no out-edges).
    assert_eq!(h_c, 0,         "C: no out-edges → hub = 0");
    assert_eq!(a_c, 1_000_000, "C: cited by B → auth = 1M");
    // B: both hub and authority.
    assert_eq!(h_b, 1_000_000, "B: points to C (authority) → hub = 1M");
    assert_eq!(a_b, 1_000_000, "B: cited by A (hub) → auth = 1M");
    // A has auth=0 → sorted last.
    assert_eq!(vecs[2], HITS_VEC_A, "A (auth=0) must be last");
}

// ── 5. Star-out A→{B,C,D}: A is hub, B/C/D are authorities ───────────────────

#[test]
fn star_out_hub_and_leaf_authorities() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HITS_VEC_A, HITS_KEY_A, HITS_ID_A, 0xB001);
    add_node(HITS_VEC_B, HITS_KEY_B, HITS_ID_B, 0xB002);
    add_node(HITS_VEC_C, HITS_KEY_C, HITS_ID_C, 0xB003);
    add_node(HITS_VEC_D, HITS_KEY_D, HITS_ID_D, 0xB004);
    add_edge(HITS_ID_A, HITS_ID_B, "hits.ab.t5");
    add_edge(HITS_ID_A, HITS_ID_C, "hits.ac.t5");
    add_edge(HITS_ID_A, HITS_ID_D, "hits.ad.t5");
    let (vecs, hub, auth, total) = gos_runtime::graph_hits::<128>();
    assert_eq!(total, 4, "4 nodes");
    let h_a = find_hub(&vecs, &hub, total, HITS_VEC_A).unwrap();
    let a_a = find_auth(&vecs, &auth, total, HITS_VEC_A).unwrap();
    let a_b = find_auth(&vecs, &auth, total, HITS_VEC_B).unwrap();
    let a_c = find_auth(&vecs, &auth, total, HITS_VEC_C).unwrap();
    let a_d = find_auth(&vecs, &auth, total, HITS_VEC_D).unwrap();
    // A: top hub, no in-edges so auth=0.
    assert_eq!(h_a, 1_000_000, "A: points to 3 authorities → hub = 1M");
    assert_eq!(a_a, 0,         "A: no in-edges → auth = 0");
    // B, C, D: each has auth=1M (all cited by hub A).
    assert_eq!(a_b, 1_000_000, "B: cited by A → auth = 1M");
    assert_eq!(a_c, 1_000_000, "C: cited by A → auth = 1M");
    assert_eq!(a_d, 1_000_000, "D: cited by A → auth = 1M");
    // A must be last (auth=0).
    assert_eq!(vecs[3], HITS_VEC_A, "A (auth=0) must be sorted last");
}

// ── 6. Star-in {A,B,C}→D: D is authority, A/B/C are hubs ────────────────────

#[test]
fn star_in_leaf_hubs_and_top_authority() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HITS_VEC_A, HITS_KEY_A, HITS_ID_A, 0xB001);
    add_node(HITS_VEC_B, HITS_KEY_B, HITS_ID_B, 0xB002);
    add_node(HITS_VEC_C, HITS_KEY_C, HITS_ID_C, 0xB003);
    add_node(HITS_VEC_D, HITS_KEY_D, HITS_ID_D, 0xB004);
    add_edge(HITS_ID_A, HITS_ID_D, "hits.ad.t6");
    add_edge(HITS_ID_B, HITS_ID_D, "hits.bd.t6");
    add_edge(HITS_ID_C, HITS_ID_D, "hits.cd.t6");
    let (vecs, hub, auth, total) = gos_runtime::graph_hits::<128>();
    assert_eq!(total, 4, "4 nodes");
    let h_a = find_hub(&vecs, &hub, total, HITS_VEC_A).unwrap();
    let h_b = find_hub(&vecs, &hub, total, HITS_VEC_B).unwrap();
    let h_c = find_hub(&vecs, &hub, total, HITS_VEC_C).unwrap();
    let a_d = find_auth(&vecs, &auth, total, HITS_VEC_D).unwrap();
    let h_d = find_hub(&vecs, &hub, total, HITS_VEC_D).unwrap();
    let a_a = find_auth(&vecs, &auth, total, HITS_VEC_A).unwrap();
    // A, B, C: each is a hub pointing to authority D.
    assert_eq!(h_a, 1_000_000, "A: points to D (authority) → hub = 1M");
    assert_eq!(h_b, 1_000_000, "B: points to D (authority) → hub = 1M");
    assert_eq!(h_c, 1_000_000, "C: points to D (authority) → hub = 1M");
    assert_eq!(a_a, 0,         "A: no in-edges → auth = 0");
    // D: top authority, no out-edges so hub=0.
    assert_eq!(a_d, 1_000_000, "D: cited by A, B, C (hubs) → auth = 1M");
    assert_eq!(h_d, 0,         "D: no out-edges → hub = 0");
    // D (auth=1M) must be sorted first.
    assert_eq!(vecs[0], HITS_VEC_D, "D (highest auth) must be first");
}

// ── 7. Mutual 2-cycle A↔B: both hub=auth=1M ──────────────────────────────────

#[test]
fn mutual_cycle_both_hub_and_authority() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HITS_VEC_A, HITS_KEY_A, HITS_ID_A, 0xB001);
    add_node(HITS_VEC_B, HITS_KEY_B, HITS_ID_B, 0xB002);
    add_edge(HITS_ID_A, HITS_ID_B, "hits.ab.t7");
    add_edge(HITS_ID_B, HITS_ID_A, "hits.ba.t7");
    let (vecs, hub, auth, total) = gos_runtime::graph_hits::<128>();
    assert_eq!(total, 2, "2 nodes");
    let h_a = find_hub(&vecs, &hub, total, HITS_VEC_A).unwrap();
    let a_a = find_auth(&vecs, &auth, total, HITS_VEC_A).unwrap();
    let h_b = find_hub(&vecs, &hub, total, HITS_VEC_B).unwrap();
    let a_b = find_auth(&vecs, &auth, total, HITS_VEC_B).unwrap();
    // Mutual cycle: symmetric → both score 1M for both hub and authority.
    assert_eq!(h_a, 1_000_000, "A: points to B (which is also authority) → hub = 1M");
    assert_eq!(a_a, 1_000_000, "A: cited by B (which is also hub) → auth = 1M");
    assert_eq!(h_b, 1_000_000, "B: hub = 1M (symmetric)");
    assert_eq!(a_b, 1_000_000, "B: auth = 1M (symmetric)");
    let _ = vecs;
}

// ── 8. 3-cycle A→B→C→A: all hub=auth=1M ─────────────────────────────────────

#[test]
fn three_cycle_all_hub_and_authority() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HITS_VEC_A, HITS_KEY_A, HITS_ID_A, 0xB001);
    add_node(HITS_VEC_B, HITS_KEY_B, HITS_ID_B, 0xB002);
    add_node(HITS_VEC_C, HITS_KEY_C, HITS_ID_C, 0xB003);
    add_edge(HITS_ID_A, HITS_ID_B, "hits.ab.t8");
    add_edge(HITS_ID_B, HITS_ID_C, "hits.bc.t8");
    add_edge(HITS_ID_C, HITS_ID_A, "hits.ca.t8");
    let (vecs, hub, auth, total) = gos_runtime::graph_hits::<128>();
    assert_eq!(total, 3, "3 nodes");
    let h_a = find_hub(&vecs, &hub, total, HITS_VEC_A).unwrap();
    let a_a = find_auth(&vecs, &auth, total, HITS_VEC_A).unwrap();
    let h_b = find_hub(&vecs, &hub, total, HITS_VEC_B).unwrap();
    let a_b = find_auth(&vecs, &auth, total, HITS_VEC_B).unwrap();
    let h_c = find_hub(&vecs, &hub, total, HITS_VEC_C).unwrap();
    let a_c = find_auth(&vecs, &auth, total, HITS_VEC_C).unwrap();
    // 3-cycle symmetry: all nodes equal, all hub=auth=1M.
    assert_eq!(h_a, 1_000_000, "A: hub = 1M (3-cycle symmetry)");
    assert_eq!(a_a, 1_000_000, "A: auth = 1M (3-cycle symmetry)");
    assert_eq!(h_b, 1_000_000, "B: hub = 1M");
    assert_eq!(a_b, 1_000_000, "B: auth = 1M");
    assert_eq!(h_c, 1_000_000, "C: hub = 1M");
    assert_eq!(a_c, 1_000_000, "C: auth = 1M");
    // All auth equal — any ordering is valid.
    for i in 0..3 {
        assert_eq!(auth[i], 1_000_000, "all 3-cycle entries auth = 1M");
        assert_eq!(hub[i],  1_000_000, "all 3-cycle entries hub = 1M");
    }
    let _ = vecs;
}

// ── 9. Bipartite {A,B}→{C,D}: hub[A/B]=1M, auth[C/D]=1M; A/B auth=0 ─────────

#[test]
fn bipartite_hubs_and_authorities_separated() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HITS_VEC_A, HITS_KEY_A, HITS_ID_A, 0xB001); // hub
    add_node(HITS_VEC_B, HITS_KEY_B, HITS_ID_B, 0xB002); // hub
    add_node(HITS_VEC_C, HITS_KEY_C, HITS_ID_C, 0xB003); // authority
    add_node(HITS_VEC_D, HITS_KEY_D, HITS_ID_D, 0xB004); // authority
    add_edge(HITS_ID_A, HITS_ID_C, "hits.ac.t9");
    add_edge(HITS_ID_A, HITS_ID_D, "hits.ad.t9");
    add_edge(HITS_ID_B, HITS_ID_C, "hits.bc.t9");
    add_edge(HITS_ID_B, HITS_ID_D, "hits.bd.t9");
    let (vecs, hub, auth, total) = gos_runtime::graph_hits::<128>();
    assert_eq!(total, 4, "4 nodes");
    let h_a = find_hub(&vecs, &hub, total, HITS_VEC_A).unwrap();
    let a_a = find_auth(&vecs, &auth, total, HITS_VEC_A).unwrap();
    let h_b = find_hub(&vecs, &hub, total, HITS_VEC_B).unwrap();
    let a_b = find_auth(&vecs, &auth, total, HITS_VEC_B).unwrap();
    let h_c = find_hub(&vecs, &hub, total, HITS_VEC_C).unwrap();
    let a_c = find_auth(&vecs, &auth, total, HITS_VEC_C).unwrap();
    let h_d = find_hub(&vecs, &hub, total, HITS_VEC_D).unwrap();
    let a_d = find_auth(&vecs, &auth, total, HITS_VEC_D).unwrap();
    // Hub group: A and B each point to both C and D (authorities).
    assert_eq!(h_a, 1_000_000, "A: top hub → hub = 1M");
    assert_eq!(h_b, 1_000_000, "B: top hub → hub = 1M");
    assert_eq!(a_a, 0,         "A: no in-edges → auth = 0");
    assert_eq!(a_b, 0,         "B: no in-edges → auth = 0");
    // Authority group: C and D each cited by both A and B (hubs).
    assert_eq!(a_c, 1_000_000, "C: top authority → auth = 1M");
    assert_eq!(a_d, 1_000_000, "D: top authority → auth = 1M");
    assert_eq!(h_c, 0,         "C: no out-edges → hub = 0");
    assert_eq!(h_d, 0,         "D: no out-edges → hub = 0");
    // C and D (auth=1M) are sorted before A and B (auth=0).
    assert!(auth[0] == 1_000_000 && auth[1] == 1_000_000, "first two must be authority nodes");
    assert!(auth[2] == 0 && auth[3] == 0, "last two must have auth=0");
    let _ = vecs;
}

// ── 10. Sort order: auth[0] ≥ auth[1] ≥ ... ─────────────────────────────────

#[test]
fn output_sorted_descending_by_authority() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Mixed: A→D (hub→auth), B→D (hub→auth), C isolated, A→B (relay).
    add_node(HITS_VEC_A, HITS_KEY_A, HITS_ID_A, 0xB001);
    add_node(HITS_VEC_B, HITS_KEY_B, HITS_ID_B, 0xB002);
    add_node(HITS_VEC_C, HITS_KEY_C, HITS_ID_C, 0xB003);
    add_node(HITS_VEC_D, HITS_KEY_D, HITS_ID_D, 0xB004);
    add_edge(HITS_ID_A, HITS_ID_D, "hits.ad.t10");
    add_edge(HITS_ID_B, HITS_ID_D, "hits.bd.t10");
    add_edge(HITS_ID_A, HITS_ID_B, "hits.ab.t10");
    let (_vecs, _hub, auth, total) = gos_runtime::graph_hits::<128>();
    assert_eq!(total, 4, "4 nodes");
    for i in 1..total {
        assert!(
            auth[i - 1] >= auth[i],
            "sort violation: auth[{}]={} < auth[{}]={}",
            i - 1, auth[i - 1], i, auth[i]
        );
    }
}
