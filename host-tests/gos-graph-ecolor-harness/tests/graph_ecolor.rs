// gos-graph-ecolor-harness — V3.08 edge coloring tests
//
// Verifies `gos_runtime::graph_edge_color` — greedy edge coloring
// achieving χ'(G) ≤ Δ+1 (Vizing 1964).
//
// Edge coloring assigns a colour to every undirected edge so that no two
// edges sharing an endpoint receive the same colour.  The chromatic index
// χ'(G) is either Δ(G) (class 1) or Δ(G)+1 (class 2).
//
// Algorithm: greedy — iterate edges; for each edge (a,b) assign the lowest
// colour not already used by any incident edge at a or b.  Uses per-node
// u128 colour bitmasks; trailing_ones(forbidden) = lowest free slot. O(E).
//
// Tests (10):
//  1.  Empty graph                              → ec=0, χ'=0.
//  2.  Single isolated node                     → ec=0, χ'=0.
//  3.  Single edge A→B                          → ec=1, χ'=1, color=0.
//  4.  Path A→B→C  (Δ=2)                        → ec=2, χ'=2, adjacent colors differ.
//  5.  Triangle K_3 (directed cycle A→B→C→A)   → ec=3, Δ=2, χ'=3 (odd, class 2).
//  6.  C_4 (directed cycle A→B→C→D→A)          → ec=4, Δ=2, χ'=2 (even, class 1).
//  7.  K_4 complete (one-direction edges)       → ec=6, Δ=3, χ'=3 (class 1).
//  8.  Star K_{1,4} (center→4 leaves)           → ec=4, Δ=4, χ'=4 (stars: class 1).
//  9.  Self-loop only A→A                       → ec=0, χ'=0 (self-loops excluded).
// 10.  Vizing invariant + validity on K_{3,3} → greedy χ'=4=Δ+1, no conflicts.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const EC_PLUGIN: PluginId   = PluginId::from_ascii("KL_GRAPH_ECOL_H");
const EC_EXEC:   ExecutorId = ExecutorId::from_ascii("ecolor.exec");

const EC_KEY_A: &str = "ecolor.a";
const EC_KEY_B: &str = "ecolor.b";
const EC_KEY_C: &str = "ecolor.c";
const EC_KEY_D: &str = "ecolor.d";
const EC_KEY_E: &str = "ecolor.e";
const EC_KEY_F: &str = "ecolor.f";

const EC_ID_A: NodeId = derive_node_id(EC_PLUGIN, EC_KEY_A);
const EC_ID_B: NodeId = derive_node_id(EC_PLUGIN, EC_KEY_B);
const EC_ID_C: NodeId = derive_node_id(EC_PLUGIN, EC_KEY_C);
const EC_ID_D: NodeId = derive_node_id(EC_PLUGIN, EC_KEY_D);
const EC_ID_E: NodeId = derive_node_id(EC_PLUGIN, EC_KEY_E);
const EC_ID_F: NodeId = derive_node_id(EC_PLUGIN, EC_KEY_F);

// L4=84 namespace for this harness.
const EC_VEC_A: VectorAddress = VectorAddress::new(84, 1, 1, 0);
const EC_VEC_B: VectorAddress = VectorAddress::new(84, 1, 2, 0);
const EC_VEC_C: VectorAddress = VectorAddress::new(84, 1, 3, 0);
const EC_VEC_D: VectorAddress = VectorAddress::new(84, 2, 1, 0);
const EC_VEC_E: VectorAddress = VectorAddress::new(84, 2, 2, 0);
const EC_VEC_F: VectorAddress = VectorAddress::new(84, 2, 3, 0);

const EC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    EC_PLUGIN,
    name:         "kl-graph-ecolor-harness",
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
        executor_id:       EC_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(EC_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(EC_MANIFEST).unwrap();
    g
}

// Verify proper edge coloring: no two adjacent edges share a colour.
fn assert_proper_coloring(
    from_vecs:   &[VectorAddress],
    to_vecs:     &[VectorAddress],
    edge_colors: &[u8],
    ec:          usize,
) {
    for i in 0..ec {
        for j in (i + 1)..ec {
            let share = from_vecs[i] == from_vecs[j]
                || from_vecs[i] == to_vecs[j]
                || to_vecs[i]   == from_vecs[j]
                || to_vecs[i]   == to_vecs[j];
            if share {
                assert_ne!(
                    edge_colors[i], edge_colors[j],
                    "edges {} and {} share a vertex but have the same colour {}",
                    i, j, edge_colors[i]
                );
            }
        }
    }
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (_, _, _, ec, chi) = gos_runtime::graph_edge_color::<64>();
    assert_eq!(ec,  0, "empty: ec=0");
    assert_eq!(chi, 0, "empty: chi'=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);

    let (_, _, _, ec, chi) = gos_runtime::graph_edge_color::<64>();
    assert_eq!(ec,  0, "single node: ec=0 (no edges)");
    assert_eq!(chi, 0, "single node: chi'=0");
}

// ── Test 3: Single edge A→B — ec=1, χ'=1 ────────────────────────────────────

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_edge(EC_ID_A, EC_ID_B, "ab");

    let (fv, tv, ec_arr, ec, chi) = gos_runtime::graph_edge_color::<64>();
    assert_eq!(ec,  1, "single edge: ec=1");
    assert_eq!(chi, 1, "single edge: chi'=1");
    assert_eq!(ec_arr[0], 0, "single edge: assigned colour 0");
    // The two endpoints must be A and B (in some canonical order).
    assert!(
        (fv[0] == EC_VEC_A && tv[0] == EC_VEC_B)
            || (fv[0] == EC_VEC_B && tv[0] == EC_VEC_A),
        "single edge: endpoints must be A and B"
    );
}

// ── Test 4: Path A→B→C — ec=2, χ'=2 ─────────────────────────────────────────

#[test]
fn test_04_path_abc() {
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_node(EC_VEC_C, EC_KEY_C, EC_ID_C);
    add_edge(EC_ID_A, EC_ID_B, "ab");
    add_edge(EC_ID_B, EC_ID_C, "bc");

    let (fv, tv, ec_arr, ec, chi) = gos_runtime::graph_edge_color::<64>();
    assert_eq!(ec,  2, "path A-B-C: ec=2");
    assert_eq!(chi, 2, "path A-B-C: chi'=2 (B has degree 2)");
    assert_ne!(ec_arr[0], ec_arr[1], "path: two edges incident to B must differ");
    assert_proper_coloring(&fv, &tv, &ec_arr, ec);
}

// ── Test 5: Triangle K_3 (directed cycle) — ec=3, Δ=2, χ'=3 ─────────────────

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_node(EC_VEC_C, EC_KEY_C, EC_ID_C);
    add_edge(EC_ID_A, EC_ID_B, "ab");
    add_edge(EC_ID_B, EC_ID_C, "bc");
    add_edge(EC_ID_C, EC_ID_A, "ca");

    let (fv, tv, ec_arr, ec, chi) = gos_runtime::graph_edge_color::<64>();
    assert_eq!(ec,  3, "K_3: ec=3");
    assert_eq!(chi, 3, "K_3: chi'=3 (odd cycle, class 2 = Delta+1)");
    // All three colours must be distinct.
    assert_ne!(ec_arr[0], ec_arr[1], "K_3: colours must all differ");
    assert_ne!(ec_arr[1], ec_arr[2], "K_3: colours must all differ");
    assert_ne!(ec_arr[0], ec_arr[2], "K_3: colours must all differ");
    assert_proper_coloring(&fv, &tv, &ec_arr, ec);
}

// ── Test 6: C_4 directed cycle — ec=4, Δ=2, χ'=2 ────────────────────────────

#[test]
fn test_06_cycle_c4() {
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_node(EC_VEC_C, EC_KEY_C, EC_ID_C);
    add_node(EC_VEC_D, EC_KEY_D, EC_ID_D);
    add_edge(EC_ID_A, EC_ID_B, "ab");
    add_edge(EC_ID_B, EC_ID_C, "bc");
    add_edge(EC_ID_C, EC_ID_D, "cd");
    add_edge(EC_ID_D, EC_ID_A, "da");

    let (fv, tv, ec_arr, ec, chi) = gos_runtime::graph_edge_color::<64>();
    assert_eq!(ec,  4, "C_4: ec=4");
    assert_eq!(chi, 2, "C_4: chi'=2 (even cycle, class 1 = Delta)");
    assert_proper_coloring(&fv, &tv, &ec_arr, ec);
}

// ── Test 7: K_4 complete — ec=6, Δ=3, χ'=3 ──────────────────────────────────

#[test]
fn test_07_k4_complete() {
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_node(EC_VEC_C, EC_KEY_C, EC_ID_C);
    add_node(EC_VEC_D, EC_KEY_D, EC_ID_D);
    // One-direction edges → undirected projection = K_4.
    add_edge(EC_ID_A, EC_ID_B, "ab");
    add_edge(EC_ID_A, EC_ID_C, "ac");
    add_edge(EC_ID_A, EC_ID_D, "ad");
    add_edge(EC_ID_B, EC_ID_C, "bc");
    add_edge(EC_ID_B, EC_ID_D, "bd");
    add_edge(EC_ID_C, EC_ID_D, "cd");

    let (fv, tv, ec_arr, ec, chi) = gos_runtime::graph_edge_color::<64>();
    assert_eq!(ec,  6, "K_4: ec=6");
    assert_eq!(chi, 3, "K_4: chi'=3 = Delta (class 1)");
    assert_proper_coloring(&fv, &tv, &ec_arr, ec);
}

// ── Test 8: Star K_{1,4} — ec=4, Δ=4, χ'=4 ──────────────────────────────────

#[test]
fn test_08_star_k14() {
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A); // centre
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_node(EC_VEC_C, EC_KEY_C, EC_ID_C);
    add_node(EC_VEC_D, EC_KEY_D, EC_ID_D);
    add_node(EC_VEC_E, EC_KEY_E, EC_ID_E);
    add_edge(EC_ID_A, EC_ID_B, "ab");
    add_edge(EC_ID_A, EC_ID_C, "ac");
    add_edge(EC_ID_A, EC_ID_D, "ad");
    add_edge(EC_ID_A, EC_ID_E, "ae");

    let (fv, tv, ec_arr, ec, chi) = gos_runtime::graph_edge_color::<64>();
    assert_eq!(ec,  4, "K_{{1,4}}: ec=4");
    assert_eq!(chi, 4, "K_{{1,4}}: chi'=4=Delta (all edges share centre, need 4 slots)");
    // All 4 edge colours must be distinct.
    let mut seen = [false; 4];
    for i in 0..4 {
        assert!((ec_arr[i] as usize) < 4, "colour must be in [0,3]");
        seen[ec_arr[i] as usize] = true;
    }
    assert!(seen.iter().all(|&s| s), "K_{{1,4}}: all 4 colours used");
    assert_proper_coloring(&fv, &tv, &ec_arr, ec);
}

// ── Test 9: Self-loop only A→A — ec=0 ────────────────────────────────────────

#[test]
fn test_09_self_loop_excluded() {
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_edge(EC_ID_A, EC_ID_A, "aa"); // self-loop — must be excluded
    add_edge(EC_ID_B, EC_ID_B, "bb"); // self-loop — must be excluded

    let (_, _, _, ec, chi) = gos_runtime::graph_edge_color::<64>();
    assert_eq!(ec,  0, "self-loops: ec=0");
    assert_eq!(chi, 0, "self-loops: chi'=0");
}

// ── Test 10: K_{3,3} — Vizing invariant + full validity check ────────────────

#[test]
fn test_10_k33_vizing_validity() {
    let _g = setup();
    // K_{3,3}: side-A = {A,B,C}, side-B = {D,E,F}.
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_node(EC_VEC_C, EC_KEY_C, EC_ID_C);
    add_node(EC_VEC_D, EC_KEY_D, EC_ID_D);
    add_node(EC_VEC_E, EC_KEY_E, EC_ID_E);
    add_node(EC_VEC_F, EC_KEY_F, EC_ID_F);
    // All A_i → B_j directed edges (one direction; undirected = K_{3,3}).
    add_edge(EC_ID_A, EC_ID_D, "ad"); add_edge(EC_ID_A, EC_ID_E, "ae");
    add_edge(EC_ID_A, EC_ID_F, "af"); add_edge(EC_ID_B, EC_ID_D, "bd");
    add_edge(EC_ID_B, EC_ID_E, "be"); add_edge(EC_ID_B, EC_ID_F, "bf");
    add_edge(EC_ID_C, EC_ID_D, "cd"); add_edge(EC_ID_C, EC_ID_E, "ce");
    add_edge(EC_ID_C, EC_ID_F, "cf");

    let (fv, tv, ec_arr, ec, chi) = gos_runtime::graph_edge_color::<64>();
    assert_eq!(ec, 9, "K_{{3,3}}: ec=9");

    // K_{3,3} has Δ=3. Vizing: χ'∈{Δ, Δ+1} = {3,4}.
    // Greedy with this edge ordering achieves χ'=4 (Δ+1 = Vizing upper bound).
    assert_eq!(chi, 4, "K_{{3,3}}: greedy chi'=4 (Vizing bound Delta+1)");

    // All colours must be in range [0, chi).
    for i in 0..ec {
        assert!((ec_arr[i] as usize) < chi as usize,
            "colour {} out of range [0,{})", ec_arr[i], chi);
    }

    // Full proper-coloring validation: no two adjacent edges share a colour.
    assert_proper_coloring(&fv, &tv, &ec_arr, ec);
}
