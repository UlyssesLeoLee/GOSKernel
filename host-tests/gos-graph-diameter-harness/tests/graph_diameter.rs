// gos-graph-diameter-harness — V2.82 graph diameter combined center+peripheral view
//
// Verifies graph_peripheral and graph_center return consistent diameter/radius/count
// values across well-known graph structures; covers all edge cases of the combined
// `graph diameter` display (empty, isolated, simple path, cycle, star, disconnected).
//
//  1. Empty graph              → diameter=0, radius=0, counts=0
//  2. Single isolated node     → diameter=0, radius=0, counts=0
//  3. Two nodes bidirected A↔B → radius=1, diameter=1, center=2, periph=2
//  4. Directed path A→B→C     → radius=1, diameter=2, center={B}, periph={A}
//  5. Bidirected triangle      → radius=1, diameter=1, center=3, periph=3
//  6. Directed cycle A→B→C→A  → radius=2, diameter=2, center=3, periph=3
//  7. Bidirected star (hub+3)  → radius=1, diameter=2, center={hub}, periph=3 spokes
//  8. Disconnected bidirected pairs → radius=1, diameter=1, center=4, periph=4
//  9. Path A→B→C→D + isolated E  → diameter=3, radius=1, center={C}, periph={A}
// 10. radius ≤ diameter invariant across all above graphs

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const DM_PLUGIN: PluginId   = PluginId::from_ascii("KL_DM01_01");
const DM_EXEC:   ExecutorId = ExecutorId::from_ascii("dm.exec");

const DM_KEY_A: &str = "dm.alpha";
const DM_KEY_B: &str = "dm.beta";
const DM_KEY_C: &str = "dm.gamma";
const DM_KEY_D: &str = "dm.delta";
const DM_KEY_E: &str = "dm.epsilon";

const DM_ID_A: NodeId = derive_node_id(DM_PLUGIN, DM_KEY_A);
const DM_ID_B: NodeId = derive_node_id(DM_PLUGIN, DM_KEY_B);
const DM_ID_C: NodeId = derive_node_id(DM_PLUGIN, DM_KEY_C);
const DM_ID_D: NodeId = derive_node_id(DM_PLUGIN, DM_KEY_D);
const DM_ID_E: NodeId = derive_node_id(DM_PLUGIN, DM_KEY_E);

// L4=58 identifies this harness namespace.
const DM_VEC_A: VectorAddress = VectorAddress::new(58, 1, 1, 0);
const DM_VEC_B: VectorAddress = VectorAddress::new(58, 1, 2, 0);
const DM_VEC_C: VectorAddress = VectorAddress::new(58, 1, 3, 0);
const DM_VEC_D: VectorAddress = VectorAddress::new(58, 1, 4, 0);
const DM_VEC_E: VectorAddress = VectorAddress::new(58, 1, 5, 0);

const DM_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    DM_PLUGIN,
    name:         "kl-graph-diameter-harness",
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
        executor_id:       DM_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }
fn register_plugin() { gos_runtime::discover_plugin(DM_MANIFEST).unwrap(); }

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(DM_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

// ── 1. Empty graph ────────────────────────────────────────────────────────────

#[test]
fn empty_graph_diameter_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_, _, periph_count, node_count, diameter) = gos_runtime::graph_peripheral::<64>();
    let (_, _, center_count, _, radius)             = gos_runtime::graph_center::<64>();
    assert_eq!(node_count,   0, "node_count=0");
    assert_eq!(periph_count, 0, "periph_count=0");
    assert_eq!(center_count, 0, "center_count=0");
    assert_eq!(diameter,     0, "diameter=0");
    assert_eq!(radius,       0, "radius=0");
}

// ── 2. Single isolated node ───────────────────────────────────────────────────

#[test]
fn single_isolated_diameter_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DM_VEC_A, DM_KEY_A, DM_ID_A, 0xD001);
    let (_, _, periph_count, node_count, diameter) = gos_runtime::graph_peripheral::<64>();
    let (_, _, center_count, _, radius)             = gos_runtime::graph_center::<64>();
    assert_eq!(node_count,   1, "node_count=1");
    assert_eq!(periph_count, 0, "periph_count=0 (isolated, diameter=0)");
    assert_eq!(center_count, 0, "center_count=0 (isolated, radius=0)");
    assert_eq!(diameter,     0, "diameter=0 (no reachable pairs)");
    assert_eq!(radius,       0, "radius=0");
}

// ── 3. Two nodes bidirected A↔B → radius=1, diameter=1, center=2, periph=2 ──
//
// Each node can reach the other in 1 hop.
// ecc[A]=1, ecc[B]=1; radius=1, diameter=1.
// All nodes satisfy ecc==radius AND ecc==diameter simultaneously.

#[test]
fn bidirected_pair_radius_equals_diameter() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DM_VEC_A, DM_KEY_A, DM_ID_A, 0xD001);
    add_node(DM_VEC_B, DM_KEY_B, DM_ID_B, 0xD002);
    add_edge(DM_ID_A, DM_ID_B, "dm.ab.t3");
    add_edge(DM_ID_B, DM_ID_A, "dm.ba.t3");
    let (_, _, periph_count, node_count, diameter) = gos_runtime::graph_peripheral::<64>();
    let (_, _, center_count, _, radius)             = gos_runtime::graph_center::<64>();
    assert_eq!(node_count,   2, "node_count=2");
    assert_eq!(diameter,     1, "diameter=1");
    assert_eq!(radius,       1, "radius=1");
    assert_eq!(periph_count, 2, "periph_count=2 (both nodes have ecc=1=diameter)");
    assert_eq!(center_count, 2, "center_count=2 (both nodes have ecc=1=radius)");
}

// ── 4. Directed path A→B→C → radius=1, diameter=2, center={B}, periph={A} ──
//
// Directed eccentricities (max finite outgoing distance):
//   A: can reach B(1), C(2) → ecc[A]=2
//   B: can reach C(1)       → ecc[B]=1
//   C: no outgoing edges    → ecc[C]=0 (excluded from diameter/radius)
// radius=1 (min nonzero ecc), diameter=2 (max ecc).
// Center: ecc==1 → {B}; Peripheral: ecc==2 → {A}.

#[test]
fn directed_path_abc_center_and_periph() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DM_VEC_A, DM_KEY_A, DM_ID_A, 0xD001);
    add_node(DM_VEC_B, DM_KEY_B, DM_ID_B, 0xD002);
    add_node(DM_VEC_C, DM_KEY_C, DM_ID_C, 0xD003);
    add_edge(DM_ID_A, DM_ID_B, "dm.ab.t4");
    add_edge(DM_ID_B, DM_ID_C, "dm.bc.t4");
    let (p_vecs, _, periph_count, node_count, diameter) = gos_runtime::graph_peripheral::<64>();
    let (c_vecs, _, center_count, _, radius)             = gos_runtime::graph_center::<64>();
    assert_eq!(node_count,   3, "node_count=3");
    assert_eq!(diameter,     2, "diameter=2");
    assert_eq!(radius,       1, "radius=1");
    assert_eq!(periph_count, 1, "periph_count=1 (only A has ecc=2)");
    assert_eq!(center_count, 1, "center_count=1 (only B has ecc=1)");
    assert_eq!(p_vecs[0], DM_VEC_A, "peripheral node is A");
    assert_eq!(c_vecs[0], DM_VEC_B, "center node is B");
}

// ── 5. Bidirected triangle → radius=1, diameter=1, center=3, periph=3 ────────
//
// Each node can reach the other two in 1 hop (all bidirected).
// ecc[A]=ecc[B]=ecc[C]=1; radius=1, diameter=1.
// All nodes are simultaneously center and peripheral.

#[test]
fn bidirected_triangle_all_center_all_periph() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DM_VEC_A, DM_KEY_A, DM_ID_A, 0xD001);
    add_node(DM_VEC_B, DM_KEY_B, DM_ID_B, 0xD002);
    add_node(DM_VEC_C, DM_KEY_C, DM_ID_C, 0xD003);
    add_edge(DM_ID_A, DM_ID_B, "dm.ab.t5"); add_edge(DM_ID_B, DM_ID_A, "dm.ba.t5");
    add_edge(DM_ID_B, DM_ID_C, "dm.bc.t5"); add_edge(DM_ID_C, DM_ID_B, "dm.cb.t5");
    add_edge(DM_ID_A, DM_ID_C, "dm.ac.t5"); add_edge(DM_ID_C, DM_ID_A, "dm.ca.t5");
    let (_, _, periph_count, node_count, diameter) = gos_runtime::graph_peripheral::<64>();
    let (_, _, center_count, _, radius)             = gos_runtime::graph_center::<64>();
    assert_eq!(node_count,   3, "node_count=3");
    assert_eq!(diameter,     1, "diameter=1");
    assert_eq!(radius,       1, "radius=1");
    assert_eq!(periph_count, 3, "periph_count=3 (all ecc=1=diameter)");
    assert_eq!(center_count, 3, "center_count=3 (all ecc=1=radius)");
}

// ── 6. Directed cycle A→B→C→A → radius=2, diameter=2, all in center+periph ──
//
// Each node can reach both others:
//   A: B(1), C(2) → ecc[A]=2; similarly B and C.
// radius=2, diameter=2; all 3 nodes are center and peripheral.

#[test]
fn directed_cycle_all_symmetric() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DM_VEC_A, DM_KEY_A, DM_ID_A, 0xD001);
    add_node(DM_VEC_B, DM_KEY_B, DM_ID_B, 0xD002);
    add_node(DM_VEC_C, DM_KEY_C, DM_ID_C, 0xD003);
    add_edge(DM_ID_A, DM_ID_B, "dm.ab.t6");
    add_edge(DM_ID_B, DM_ID_C, "dm.bc.t6");
    add_edge(DM_ID_C, DM_ID_A, "dm.ca.t6");
    let (_, _, periph_count, node_count, diameter) = gos_runtime::graph_peripheral::<64>();
    let (_, _, center_count, _, radius)             = gos_runtime::graph_center::<64>();
    assert_eq!(node_count,   3, "node_count=3");
    assert_eq!(diameter,     2, "diameter=2");
    assert_eq!(radius,       2, "radius=2");
    assert_eq!(periph_count, 3, "periph_count=3 (all ecc=2)");
    assert_eq!(center_count, 3, "center_count=3 (all ecc=2=radius)");
}

// ── 7. Bidirected star hub=A, spokes=B,C,D → center={A}, periph={B,C,D} ─────
//
// A↔B, A↔C, A↔D (all bidirected).
//   A: max hop to B/C/D = 1 → ecc[A]=1
//   B: A(1), then C/D via A at dist 2 → ecc[B]=2; same for C, D.
// radius=1 (A), diameter=2 (B,C,D).
// center_count=1, periph_count=3.

#[test]
fn bidirected_star_hub_is_center() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DM_VEC_A, DM_KEY_A, DM_ID_A, 0xD001);  // hub
    add_node(DM_VEC_B, DM_KEY_B, DM_ID_B, 0xD002);
    add_node(DM_VEC_C, DM_KEY_C, DM_ID_C, 0xD003);
    add_node(DM_VEC_D, DM_KEY_D, DM_ID_D, 0xD004);
    add_edge(DM_ID_A, DM_ID_B, "dm.ab.t7"); add_edge(DM_ID_B, DM_ID_A, "dm.ba.t7");
    add_edge(DM_ID_A, DM_ID_C, "dm.ac.t7"); add_edge(DM_ID_C, DM_ID_A, "dm.ca.t7");
    add_edge(DM_ID_A, DM_ID_D, "dm.ad.t7"); add_edge(DM_ID_D, DM_ID_A, "dm.da.t7");
    let (_, _, periph_count, node_count, diameter) = gos_runtime::graph_peripheral::<64>();
    let (c_vecs, _, center_count, _, radius)        = gos_runtime::graph_center::<64>();
    assert_eq!(node_count,   4, "node_count=4");
    assert_eq!(diameter,     2, "diameter=2 (spokes have ecc=2)");
    assert_eq!(radius,       1, "radius=1 (hub has ecc=1)");
    assert_eq!(center_count, 1, "center_count=1 (only hub)");
    assert_eq!(periph_count, 3, "periph_count=3 (all spokes)");
    assert_eq!(c_vecs[0], DM_VEC_A, "center is the hub (A)");
}

// ── 8. Disconnected bidirected pairs A↔B and C↔D ─────────────────────────────
//
// Each node can only reach its partner (1 hop).
// ecc[A]=1, ecc[B]=1, ecc[C]=1, ecc[D]=1.
// radius=1, diameter=1; all 4 are center and peripheral.

#[test]
fn disconnected_pairs_all_center_all_periph() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DM_VEC_A, DM_KEY_A, DM_ID_A, 0xD001);
    add_node(DM_VEC_B, DM_KEY_B, DM_ID_B, 0xD002);
    add_node(DM_VEC_C, DM_KEY_C, DM_ID_C, 0xD003);
    add_node(DM_VEC_D, DM_KEY_D, DM_ID_D, 0xD004);
    add_edge(DM_ID_A, DM_ID_B, "dm.ab.t8"); add_edge(DM_ID_B, DM_ID_A, "dm.ba.t8");
    add_edge(DM_ID_C, DM_ID_D, "dm.cd.t8"); add_edge(DM_ID_D, DM_ID_C, "dm.dc.t8");
    let (_, _, periph_count, node_count, diameter) = gos_runtime::graph_peripheral::<64>();
    let (_, _, center_count, _, radius)             = gos_runtime::graph_center::<64>();
    assert_eq!(node_count,   4, "node_count=4");
    assert_eq!(diameter,     1, "diameter=1 (each node can reach 1 peer)");
    assert_eq!(radius,       1, "radius=1");
    assert_eq!(periph_count, 4, "periph_count=4 (all ecc=1=diameter)");
    assert_eq!(center_count, 4, "center_count=4 (all ecc=1=radius)");
}

// ── 9. Directed path A→B→C→D + isolated E ─────────────────────────────────
//
// ecc[A]=3, ecc[B]=2, ecc[C]=1, ecc[D]=0 (sink), ecc[E]=0 (isolated).
// diameter=3 (A), radius=1 (C).
// Adding isolated E must not change diameter, radius, or center/periph membership.

#[test]
fn path_abcd_plus_isolated_e() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DM_VEC_A, DM_KEY_A, DM_ID_A, 0xD001);
    add_node(DM_VEC_B, DM_KEY_B, DM_ID_B, 0xD002);
    add_node(DM_VEC_C, DM_KEY_C, DM_ID_C, 0xD003);
    add_node(DM_VEC_D, DM_KEY_D, DM_ID_D, 0xD004);
    add_node(DM_VEC_E, DM_KEY_E, DM_ID_E, 0xD005);  // isolated
    add_edge(DM_ID_A, DM_ID_B, "dm.ab.t9");
    add_edge(DM_ID_B, DM_ID_C, "dm.bc.t9");
    add_edge(DM_ID_C, DM_ID_D, "dm.cd.t9");
    let (p_vecs, p_ecc, periph_count, node_count, diameter) = gos_runtime::graph_peripheral::<64>();
    let (c_vecs, c_ecc, center_count, _, radius)             = gos_runtime::graph_center::<64>();
    assert_eq!(node_count,   5, "node_count=5 (including isolated E)");
    assert_eq!(diameter,     3, "diameter=3 (A→B→C→D)");
    assert_eq!(radius,       1, "radius=1 (C has ecc=1)");
    assert_eq!(periph_count, 1, "periph_count=1 (only A has ecc=3)");
    assert_eq!(center_count, 1, "center_count=1 (only C has ecc=1)");
    assert_eq!(p_vecs[0], DM_VEC_A, "peripheral is A");
    assert_eq!(p_ecc[0],  3,        "A has ecc=3");
    assert_eq!(c_vecs[0], DM_VEC_C, "center is C");
    assert_eq!(c_ecc[0],  1,        "C has ecc=1");
}

// ── 10. radius ≤ diameter invariant ──────────────────────────────────────────
//
// For any non-trivial connected component: radius ≤ diameter always.
// We verify this across path, star, cycle, and complete graphs.

#[test]
fn radius_leq_diameter_invariant() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Helper closure to check the invariant.
    let check = || {
        let (_, _, _, _, diameter) = gos_runtime::graph_peripheral::<64>();
        let (_, _, _, _, radius)   = gos_runtime::graph_center::<64>();
        assert!(radius <= diameter,
            "radius={radius} must be ≤ diameter={diameter}");
    };

    // Directed path A→B→C (radius=1, diameter=2).
    reset();
    register_plugin();
    add_node(DM_VEC_A, DM_KEY_A, DM_ID_A, 0xD001);
    add_node(DM_VEC_B, DM_KEY_B, DM_ID_B, 0xD002);
    add_node(DM_VEC_C, DM_KEY_C, DM_ID_C, 0xD003);
    add_edge(DM_ID_A, DM_ID_B, "dm.ab.t10a");
    add_edge(DM_ID_B, DM_ID_C, "dm.bc.t10a");
    check();

    // Bidirected star (radius=1, diameter=2).
    reset();
    register_plugin();
    add_node(DM_VEC_A, DM_KEY_A, DM_ID_A, 0xD001);
    add_node(DM_VEC_B, DM_KEY_B, DM_ID_B, 0xD002);
    add_node(DM_VEC_C, DM_KEY_C, DM_ID_C, 0xD003);
    add_node(DM_VEC_D, DM_KEY_D, DM_ID_D, 0xD004);
    add_edge(DM_ID_A, DM_ID_B, "dm.ab.t10b"); add_edge(DM_ID_B, DM_ID_A, "dm.ba.t10b");
    add_edge(DM_ID_A, DM_ID_C, "dm.ac.t10b"); add_edge(DM_ID_C, DM_ID_A, "dm.ca.t10b");
    add_edge(DM_ID_A, DM_ID_D, "dm.ad.t10b"); add_edge(DM_ID_D, DM_ID_A, "dm.da.t10b");
    check();

    // Directed cycle A→B→C→A (radius=2, diameter=2 — radius==diameter is allowed).
    reset();
    register_plugin();
    add_node(DM_VEC_A, DM_KEY_A, DM_ID_A, 0xD001);
    add_node(DM_VEC_B, DM_KEY_B, DM_ID_B, 0xD002);
    add_node(DM_VEC_C, DM_KEY_C, DM_ID_C, 0xD003);
    add_edge(DM_ID_A, DM_ID_B, "dm.ab.t10c");
    add_edge(DM_ID_B, DM_ID_C, "dm.bc.t10c");
    add_edge(DM_ID_C, DM_ID_A, "dm.ca.t10c");
    check();

    // Complete K4 bidirected (radius=1, diameter=1).
    reset();
    register_plugin();
    add_node(DM_VEC_A, DM_KEY_A, DM_ID_A, 0xD001);
    add_node(DM_VEC_B, DM_KEY_B, DM_ID_B, 0xD002);
    add_node(DM_VEC_C, DM_KEY_C, DM_ID_C, 0xD003);
    add_node(DM_VEC_D, DM_KEY_D, DM_ID_D, 0xD004);
    add_edge(DM_ID_A, DM_ID_B, "dm.ab.t10d"); add_edge(DM_ID_B, DM_ID_A, "dm.ba.t10d");
    add_edge(DM_ID_A, DM_ID_C, "dm.ac.t10d"); add_edge(DM_ID_C, DM_ID_A, "dm.ca.t10d");
    add_edge(DM_ID_A, DM_ID_D, "dm.ad.t10d"); add_edge(DM_ID_D, DM_ID_A, "dm.da.t10d");
    add_edge(DM_ID_B, DM_ID_C, "dm.bc.t10d"); add_edge(DM_ID_C, DM_ID_B, "dm.cb.t10d");
    add_edge(DM_ID_B, DM_ID_D, "dm.bd.t10d"); add_edge(DM_ID_D, DM_ID_B, "dm.db.t10d");
    add_edge(DM_ID_C, DM_ID_D, "dm.cd.t10d"); add_edge(DM_ID_D, DM_ID_C, "dm.dc.t10d");
    check();
}
