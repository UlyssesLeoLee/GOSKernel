// gos-graph-reciprocity-harness — V2.66 graph reciprocity
//
// Verifies `gos_runtime::graph_reciprocity` — fraction of directed edges
// that have a corresponding reverse edge (mutual / bidirectional).
//
// For each directed edge (u→v), reciprocity counts it as mutual if the edge
// (v→u) also exists. Self-loops are excluded from both the mutual count and
// the total count.
//
// reciprocity_ppm = mutual_edges / total_edges × 1_000_000
//   mutual_edges  = number of directed edges (u,v) where (v,u) also exists
//   total_edges   = directed edge count (self-loops excluded)
//
// Return value: (reciprocity_ppm, mutual_edges, total_edges)
//   1_000_000 → fully reciprocal  (all edges bidirectional)
//   0         → no mutual edges, or no edges at all
//
// Test matrix:
//  1.  Empty graph                            → (0, 0, 0)
//  2.  Nodes but no edges                     → (0, 0, 0)
//  3.  Single one-way edge A→B                → (0, 0, 1)
//  4.  Mutual pair A→B + B→A                  → (1_000_000, 2, 2)
//  5.  Path A→B→C  (no reverse edges)         → (0, 0, 2)
//  6.  Directed triangle cycle A→B→C→A        → (0, 0, 3)
//  7.  Mixed: mutual pair + one-way edge       → (666_666, 2, 3)
//  8.  Fully bidirectional 4-cycle             → (1_000_000, 8, 8)
//  9.  Star hub + partial reverse              → (500_000, 2, 4)
// 10.  Self-loop excluded from total           → self-loop ≠ edge

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const REC_PLUGIN: PluginId   = PluginId::from_ascii("KL_RECIP_000");
const REC_EXEC:   ExecutorId = ExecutorId::from_ascii("recip.exec0");

const REC_KEY_A: &str = "rec.alpha";
const REC_KEY_B: &str = "rec.beta";
const REC_KEY_C: &str = "rec.gamma";
const REC_KEY_D: &str = "rec.delta";

const REC_ID_A: NodeId = derive_node_id(REC_PLUGIN, REC_KEY_A);
const REC_ID_B: NodeId = derive_node_id(REC_PLUGIN, REC_KEY_B);
const REC_ID_C: NodeId = derive_node_id(REC_PLUGIN, REC_KEY_C);
const REC_ID_D: NodeId = derive_node_id(REC_PLUGIN, REC_KEY_D);

const REC_VEC_A: VectorAddress = VectorAddress::new(42, 1, 1, 0);
const REC_VEC_B: VectorAddress = VectorAddress::new(42, 1, 2, 0);
const REC_VEC_C: VectorAddress = VectorAddress::new(42, 1, 3, 0);
const REC_VEC_D: VectorAddress = VectorAddress::new(42, 1, 4, 0);

const REC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    REC_PLUGIN,
    name:         "kl-graph-reciprocity-harness",
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
        executor_id:       REC_EXEC,
        state_schema_hash: 0xA660,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(REC_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(REC_PLUGIN, vec, node_spec(key, id)).unwrap();
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

fn run_recip() -> (u32, usize, usize) {
    gos_runtime::graph_reciprocity()
}

// ── 1. Empty graph ────────────────────────────────────────────────────────────

#[test]
fn empty_graph_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (ppm, mutual, total) = run_recip();
    assert_eq!(ppm,    0, "empty: ppm must be 0");
    assert_eq!(mutual, 0, "empty: mutual must be 0");
    assert_eq!(total,  0, "empty: total must be 0");
}

// ── 2. Nodes but no edges ─────────────────────────────────────────────────────

#[test]
fn nodes_no_edges_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REC_VEC_A, REC_KEY_A, REC_ID_A);
    add_node(REC_VEC_B, REC_KEY_B, REC_ID_B);

    let (ppm, mutual, total) = run_recip();
    assert_eq!(ppm,    0, "no edges: ppm=0");
    assert_eq!(mutual, 0, "no edges: mutual=0");
    assert_eq!(total,  0, "no edges: total=0");
}

// ── 3. Single one-way edge A→B — no reverse → reciprocity=0 ──────────────────

#[test]
fn single_one_way_edge_zero_reciprocity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REC_VEC_A, REC_KEY_A, REC_ID_A);
    add_node(REC_VEC_B, REC_KEY_B, REC_ID_B);
    add_edge(REC_ID_A, REC_ID_B, "rec.ab.t3");

    let (ppm, mutual, total) = run_recip();
    assert_eq!(ppm,    0, "one-way A→B: no reverse → ppm=0");
    assert_eq!(mutual, 0, "one-way A→B: mutual=0");
    assert_eq!(total,  1, "one-way A→B: total=1");
}

// ── 4. Mutual pair A→B + B→A — fully reciprocal ──────────────────────────────

#[test]
fn mutual_pair_fully_reciprocal() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REC_VEC_A, REC_KEY_A, REC_ID_A);
    add_node(REC_VEC_B, REC_KEY_B, REC_ID_B);
    add_edge(REC_ID_A, REC_ID_B, "rec.ab.t4");
    add_edge(REC_ID_B, REC_ID_A, "rec.ba.t4");

    let (ppm, mutual, total) = run_recip();
    assert_eq!(ppm,    1_000_000, "A→B + B→A: both mutual → ppm=1_000_000");
    assert_eq!(mutual, 2,         "both edges are mutual");
    assert_eq!(total,  2,         "2 directed edges");
}

// ── 5. Path A→B→C — no reverse edges → reciprocity=0 ─────────────────────────

#[test]
fn path_graph_zero_reciprocity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REC_VEC_A, REC_KEY_A, REC_ID_A);
    add_node(REC_VEC_B, REC_KEY_B, REC_ID_B);
    add_node(REC_VEC_C, REC_KEY_C, REC_ID_C);
    add_edge(REC_ID_A, REC_ID_B, "rec.ab.t5");
    add_edge(REC_ID_B, REC_ID_C, "rec.bc.t5");

    let (ppm, mutual, total) = run_recip();
    assert_eq!(ppm,    0, "path A→B→C: no mutual edges → ppm=0");
    assert_eq!(mutual, 0, "no mutual edges");
    assert_eq!(total,  2, "2 directed edges");
}

// ── 6. Directed triangle A→B→C→A — no bidirectional edges → recip=0 ──────────

#[test]
fn directed_triangle_zero_reciprocity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REC_VEC_A, REC_KEY_A, REC_ID_A);
    add_node(REC_VEC_B, REC_KEY_B, REC_ID_B);
    add_node(REC_VEC_C, REC_KEY_C, REC_ID_C);
    add_edge(REC_ID_A, REC_ID_B, "rec.ab.t6");
    add_edge(REC_ID_B, REC_ID_C, "rec.bc.t6");
    add_edge(REC_ID_C, REC_ID_A, "rec.ca.t6");

    let (ppm, mutual, total) = run_recip();
    assert_eq!(ppm,    0, "directed triangle: no reverse edges → ppm=0");
    assert_eq!(mutual, 0, "no mutual edges in a directed cycle");
    assert_eq!(total,  3, "3 directed edges");
}

// ── 7. Mixed: mutual pair A↔B + one-way C→D ──────────────────────────────────
//
// Edges: A→B, B→A, C→D   (total=3)
// mutual: A→B (yes), B→A (yes), C→D (no) → mutual=2
// recip_ppm = 2 * 1_000_000 / 3 = 666_666

#[test]
fn mixed_partial_reciprocity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REC_VEC_A, REC_KEY_A, REC_ID_A);
    add_node(REC_VEC_B, REC_KEY_B, REC_ID_B);
    add_node(REC_VEC_C, REC_KEY_C, REC_ID_C);
    add_node(REC_VEC_D, REC_KEY_D, REC_ID_D);
    add_edge(REC_ID_A, REC_ID_B, "rec.ab.t7");
    add_edge(REC_ID_B, REC_ID_A, "rec.ba.t7");
    add_edge(REC_ID_C, REC_ID_D, "rec.cd.t7");

    let (ppm, mutual, total) = run_recip();
    assert_eq!(mutual, 2,       "2 mutual edges (A→B and B→A)");
    assert_eq!(total,  3,       "3 directed edges");
    assert_eq!(ppm,    666_666, "2/3 reciprocal → 666_666 ppm");
}

// ── 8. Fully bidirectional 4-cycle: A↔B↔C↔D↔A — all 8 edges mutual ──────────
//
// Edges (8): A→B, B→A, B→C, C→B, C→D, D→C, D→A, A→D
// Every edge has a reverse → recip_ppm = 1_000_000

#[test]
fn bidirectional_four_cycle_fully_reciprocal() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REC_VEC_A, REC_KEY_A, REC_ID_A);
    add_node(REC_VEC_B, REC_KEY_B, REC_ID_B);
    add_node(REC_VEC_C, REC_KEY_C, REC_ID_C);
    add_node(REC_VEC_D, REC_KEY_D, REC_ID_D);
    add_edge(REC_ID_A, REC_ID_B, "rec.ab.t8");
    add_edge(REC_ID_B, REC_ID_A, "rec.ba.t8");
    add_edge(REC_ID_B, REC_ID_C, "rec.bc.t8");
    add_edge(REC_ID_C, REC_ID_B, "rec.cb.t8");
    add_edge(REC_ID_C, REC_ID_D, "rec.cd.t8");
    add_edge(REC_ID_D, REC_ID_C, "rec.dc.t8");
    add_edge(REC_ID_D, REC_ID_A, "rec.da.t8");
    add_edge(REC_ID_A, REC_ID_D, "rec.ad.t8");

    let (ppm, mutual, total) = run_recip();
    assert_eq!(ppm,    1_000_000, "bidirectional 4-cycle: all edges mutual → 1_000_000");
    assert_eq!(mutual, 8,         "8 mutual edges");
    assert_eq!(total,  8,         "8 directed edges");
}

// ── 9. Star hub→B,C,D + one reverse B→hub — 2 mutual out of 4 → 500_000 ppm ──
//
// Edges: A→B, A→C, A→D, B→A   (total=4)
// mutual: A→B (B→A exists)=1, A→C (no)=0, A→D (no)=0, B→A (A→B exists)=1
// recip_ppm = 2 * 1_000_000 / 4 = 500_000

#[test]
fn star_partial_reverse_half_reciprocal() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REC_VEC_A, REC_KEY_A, REC_ID_A); // hub
    add_node(REC_VEC_B, REC_KEY_B, REC_ID_B);
    add_node(REC_VEC_C, REC_KEY_C, REC_ID_C);
    add_node(REC_VEC_D, REC_KEY_D, REC_ID_D);
    add_edge(REC_ID_A, REC_ID_B, "rec.ab.t9");
    add_edge(REC_ID_A, REC_ID_C, "rec.ac.t9");
    add_edge(REC_ID_A, REC_ID_D, "rec.ad.t9");
    add_edge(REC_ID_B, REC_ID_A, "rec.ba.t9"); // only B replies

    let (ppm, mutual, total) = run_recip();
    assert_eq!(mutual, 2,       "A→B and B→A are mutual; others are not");
    assert_eq!(total,  4,       "4 directed edges");
    assert_eq!(ppm,    500_000, "2/4 mutual → 500_000 ppm");
}

// ── 10. Self-loop excluded from totals ───────────────────────────────────────
//
// A→A is a self-loop and must NOT be counted in total_edges.
// Only A→B counts (no reverse), so total=1, mutual=0.

#[test]
fn self_loop_excluded_from_total() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REC_VEC_A, REC_KEY_A, REC_ID_A);
    add_node(REC_VEC_B, REC_KEY_B, REC_ID_B);
    add_edge(REC_ID_A, REC_ID_A, "rec.aa.t10"); // self-loop — excluded
    add_edge(REC_ID_A, REC_ID_B, "rec.ab.t10"); // one-way — counted

    let (ppm, mutual, total) = run_recip();
    assert_eq!(total,  1, "self-loop excluded → only 1 real directed edge");
    assert_eq!(mutual, 0, "A→B has no reverse → no mutual edges");
    assert_eq!(ppm,    0, "0/1 mutual → ppm=0");
}
