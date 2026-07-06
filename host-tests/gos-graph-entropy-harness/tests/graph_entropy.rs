// gos-graph-entropy-harness — V3.10 graph entropy tests
//
// Verifies `gos_runtime::graph_entropy()`:
//   - entropy_ppm    = H × 10^6  where H = -Σ p(d) ln p(d)  (nats)
//   - normalized_ppm = H/ln(n) × 10^6  ∈ [0, 1_000_000]
//   - node_count     = number of alive nodes
//
// All arithmetic is exact integer — no floating-point tolerance needed.
//
// Exact values verified analytically (LN_TABLE[k] = round(ln(k) × 10^6)):
//
//  Graph         entropy_ppm  normalized_ppm  notes
//  Empty         0            0               no nodes
//  1 node        0            0               single degree-0 node; ln(1)=0
//  Edge A-B      0            0               both degree-1 → H=0 (regular)
//  Path P₃       636_514      579_380         2×deg1 + 1×deg2; H=0.6365 nat
//  Triangle K₃   0            0               all degree-2 → H=0 (regular)
//  Star K_{1,4}  500_401      310_916         1×deg4 + 4×deg1; H=0.5004 nat
//  Path P₄       693_147      500_000         2×deg1 + 2×deg2; H=ln(2); H'=1/2
//  Complete K₄   0            0               all degree-3 → H=0 (regular)
//  2 isolated    0            0               both degree-0 → H=0 (regular)
//  K_{2,3}       673_011      418_165         2×deg3 + 3×deg2; H=0.6730 nat
//
// Tests (10):
//  1.  Empty graph                       → (0, 0, 0).
//  2.  Single isolated node              → (0, 0, 1).
//  3.  Single edge A-B                   → (0, 0, 2).
//  4.  Path P₃ = A-B-C                   → (636_514, 579_380, 3).
//  5.  Triangle K₃                       → (0, 0, 3).
//  6.  Star K_{1,4}                      → (500_401, 310_916, 5).
//  7.  Path P₄ = A-B-C-D                 → (693_147, 500_000, 4).
//  8.  Complete K₄                       → (0, 0, 4).
//  9.  Two isolated nodes                → (0, 0, 2).
// 10.  K_{2,3} bipartite                 → (673_011, 418_165, 5).

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const EN_PLUGIN: PluginId   = PluginId::from_ascii("KL_GRAPH_ENT_H");
const EN_EXEC:   ExecutorId = ExecutorId::from_ascii("entropy.exec");

const EN_KEY_A: &str = "entropy.a";
const EN_KEY_B: &str = "entropy.b";
const EN_KEY_C: &str = "entropy.c";
const EN_KEY_D: &str = "entropy.d";
const EN_KEY_E: &str = "entropy.e";

const EN_ID_A: NodeId = derive_node_id(EN_PLUGIN, EN_KEY_A);
const EN_ID_B: NodeId = derive_node_id(EN_PLUGIN, EN_KEY_B);
const EN_ID_C: NodeId = derive_node_id(EN_PLUGIN, EN_KEY_C);
const EN_ID_D: NodeId = derive_node_id(EN_PLUGIN, EN_KEY_D);
const EN_ID_E: NodeId = derive_node_id(EN_PLUGIN, EN_KEY_E);

// L4=86 namespace for this harness.
const EN_VEC_A: VectorAddress = VectorAddress::new(86, 1, 1, 0);
const EN_VEC_B: VectorAddress = VectorAddress::new(86, 1, 2, 0);
const EN_VEC_C: VectorAddress = VectorAddress::new(86, 1, 3, 0);
const EN_VEC_D: VectorAddress = VectorAddress::new(86, 2, 1, 0);
const EN_VEC_E: VectorAddress = VectorAddress::new(86, 2, 2, 0);

const EN_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    EN_PLUGIN,
    name:         "kl-graph-entropy-harness",
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
        executor_id:       EN_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(EN_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    let g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    gos_runtime::reset();
    gos_runtime::discover_plugin(EN_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
// No nodes → H=0, H'=0, nc=0.

#[test]
fn test_01_empty() {
    let _g = setup();

    let (ent, norm, nc) = gos_runtime::graph_entropy();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ent,  0, "empty: entropy=0");
    assert_eq!(norm, 0, "empty: normalized=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// One node, degree 0. Only one degree class → p(0)=1 → H=0.
// ln(1)=0 so normalized is also 0.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(EN_VEC_A, EN_KEY_A, EN_ID_A);

    let (ent, norm, nc) = gos_runtime::graph_entropy();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ent,  0, "single: entropy=0 (one degree class)");
    assert_eq!(norm, 0, "single: normalized=0 (ln(1)=0)");
}

// ── Test 3: Single edge A-B ───────────────────────────────────────────────────
// Both nodes have degree 1 → one degree class → H=0.
// nc_ln = LN_TABLE[2] = 693_147; count[1]=2; term = 693_147 - 693_147 = 0.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(EN_VEC_A, EN_KEY_A, EN_ID_A);
    add_node(EN_VEC_B, EN_KEY_B, EN_ID_B);
    add_edge(EN_ID_A, EN_ID_B, "ab");

    let (ent, norm, nc) = gos_runtime::graph_entropy();
    assert_eq!(nc,   2, "edge: node_count=2");
    assert_eq!(ent,  0, "edge: entropy=0 (regular, both deg-1)");
    assert_eq!(norm, 0, "edge: normalized=0");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// Degrees: A=1, B=2, C=1. count[1]=2, count[2]=1. nc=3.
// entropy_scaled = 2×(ln3-ln2) + 1×(ln3-ln1)
//               = 2×405_465 + 1×1_098_612 = 1_909_542
// entropy_ppm   = 1_909_542 / 3 = 636_514  (exact, no remainder)
// normalized    = 636_514 × 10^6 / 1_098_612 = 579_380

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(EN_VEC_A, EN_KEY_A, EN_ID_A);
    add_node(EN_VEC_B, EN_KEY_B, EN_ID_B);
    add_node(EN_VEC_C, EN_KEY_C, EN_ID_C);
    add_edge(EN_ID_A, EN_ID_B, "ab");
    add_edge(EN_ID_B, EN_ID_C, "bc");

    let (ent, norm, nc) = gos_runtime::graph_entropy();
    assert_eq!(nc,   3, "P3: node_count=3");
    assert_eq!(ent,  636_514, "P3: entropy_ppm");
    assert_eq!(norm, 579_380, "P3: normalized_ppm");
}

// ── Test 5: Triangle K₃ ───────────────────────────────────────────────────────
// All three nodes have degree 2 → one degree class → H=0.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(EN_VEC_A, EN_KEY_A, EN_ID_A);
    add_node(EN_VEC_B, EN_KEY_B, EN_ID_B);
    add_node(EN_VEC_C, EN_KEY_C, EN_ID_C);
    add_edge(EN_ID_A, EN_ID_B, "ab");
    add_edge(EN_ID_B, EN_ID_C, "bc");
    add_edge(EN_ID_C, EN_ID_A, "ca");

    let (ent, norm, nc) = gos_runtime::graph_entropy();
    assert_eq!(nc,   3, "K3: node_count=3");
    assert_eq!(ent,  0, "K3: entropy=0 (regular, all deg-2)");
    assert_eq!(norm, 0, "K3: normalized=0");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Centre A: degree 4. Leaves B,C,D,E: degree 1. count[4]=1, count[1]=4. nc=5.
// entropy_scaled = 4×(ln5-ln4) + 1×(ln5-ln1)
//               = 4×223_143 + 1×1_609_437
//               = 892_572 + 1_609_437 = 2_502_009
// entropy_ppm   = 2_502_009 / 5 = 500_401  (rem=4)
// normalized    = 500_401 × 10^6 / 1_609_437 = 310_916

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(EN_VEC_A, EN_KEY_A, EN_ID_A); // centre
    add_node(EN_VEC_B, EN_KEY_B, EN_ID_B);
    add_node(EN_VEC_C, EN_KEY_C, EN_ID_C);
    add_node(EN_VEC_D, EN_KEY_D, EN_ID_D);
    add_node(EN_VEC_E, EN_KEY_E, EN_ID_E);
    add_edge(EN_ID_A, EN_ID_B, "ab");
    add_edge(EN_ID_A, EN_ID_C, "ac");
    add_edge(EN_ID_A, EN_ID_D, "ad");
    add_edge(EN_ID_A, EN_ID_E, "ae");

    let (ent, norm, nc) = gos_runtime::graph_entropy();
    assert_eq!(nc,   5, "K_{{1,4}}: node_count=5");
    assert_eq!(ent,  500_401, "K_{{1,4}}: entropy_ppm");
    assert_eq!(norm, 310_916, "K_{{1,4}}: normalized_ppm");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// Degrees: A=1, B=2, C=2, D=1. count[1]=2, count[2]=2. nc=4.
// entropy_scaled = 4 × (ln4 - ln2) = 4 × 693_147 = 2_772_588
// entropy_ppm   = 2_772_588 / 4 = 693_147  (exact = ln2 × 10^6)
// normalized    = 693_147 × 10^6 / 1_386_294 = 500_000  (exactly 1/2; ln4 = 2×ln2)

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(EN_VEC_A, EN_KEY_A, EN_ID_A);
    add_node(EN_VEC_B, EN_KEY_B, EN_ID_B);
    add_node(EN_VEC_C, EN_KEY_C, EN_ID_C);
    add_node(EN_VEC_D, EN_KEY_D, EN_ID_D);
    add_edge(EN_ID_A, EN_ID_B, "ab");
    add_edge(EN_ID_B, EN_ID_C, "bc");
    add_edge(EN_ID_C, EN_ID_D, "cd");

    let (ent, norm, nc) = gos_runtime::graph_entropy();
    assert_eq!(nc,   4, "P4: node_count=4");
    assert_eq!(ent,  693_147, "P4: entropy_ppm = ln(2)×10^6 (exact)");
    assert_eq!(norm, 500_000, "P4: normalized_ppm = 1/2 (exact)");
}

// ── Test 8: Complete K₄ ───────────────────────────────────────────────────────
// All four nodes have degree 3 → one degree class → H=0.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(EN_VEC_A, EN_KEY_A, EN_ID_A);
    add_node(EN_VEC_B, EN_KEY_B, EN_ID_B);
    add_node(EN_VEC_C, EN_KEY_C, EN_ID_C);
    add_node(EN_VEC_D, EN_KEY_D, EN_ID_D);
    add_edge(EN_ID_A, EN_ID_B, "ab");
    add_edge(EN_ID_A, EN_ID_C, "ac");
    add_edge(EN_ID_A, EN_ID_D, "ad");
    add_edge(EN_ID_B, EN_ID_C, "bc");
    add_edge(EN_ID_B, EN_ID_D, "bd");
    add_edge(EN_ID_C, EN_ID_D, "cd");

    let (ent, norm, nc) = gos_runtime::graph_entropy();
    assert_eq!(nc,   4, "K4: node_count=4");
    assert_eq!(ent,  0, "K4: entropy=0 (regular, all deg-3)");
    assert_eq!(norm, 0, "K4: normalized=0");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// Both degree 0 → one degree class → H=0.
// nc_ln = LN_TABLE[2] = 693_147; count[0]=2; term = 693_147 - 693_147 = 0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(EN_VEC_A, EN_KEY_A, EN_ID_A);
    add_node(EN_VEC_B, EN_KEY_B, EN_ID_B);
    // No edges — two isolated nodes.

    let (ent, norm, nc) = gos_runtime::graph_entropy();
    assert_eq!(nc,   2, "2-isolated: node_count=2");
    assert_eq!(ent,  0, "2-isolated: entropy=0 (both deg-0)");
    assert_eq!(norm, 0, "2-isolated: normalized=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Nodes A,B on left (degree 3); C,D,E on right (degree 2). nc=5.
// Edges: A-C, A-D, A-E, B-C, B-D, B-E.
// count[3]=2, count[2]=3.
// entropy_scaled = 3×(ln5-ln3) + 2×(ln5-ln2)
//               = 3×510_825 + 2×916_290
//               = 1_532_475 + 1_832_580 = 3_365_055
// entropy_ppm   = 3_365_055 / 5 = 673_011
// normalized    = 673_011 × 10^6 / 1_609_437 = 418_165

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(EN_VEC_A, EN_KEY_A, EN_ID_A); // left A (deg 3)
    add_node(EN_VEC_B, EN_KEY_B, EN_ID_B); // left B (deg 3)
    add_node(EN_VEC_C, EN_KEY_C, EN_ID_C); // right C (deg 2)
    add_node(EN_VEC_D, EN_KEY_D, EN_ID_D); // right D (deg 2)
    add_node(EN_VEC_E, EN_KEY_E, EN_ID_E); // right E (deg 2)
    // K_{2,3}: every left node connects to every right node.
    add_edge(EN_ID_A, EN_ID_C, "ac");
    add_edge(EN_ID_A, EN_ID_D, "ad");
    add_edge(EN_ID_A, EN_ID_E, "ae");
    add_edge(EN_ID_B, EN_ID_C, "bc");
    add_edge(EN_ID_B, EN_ID_D, "bd");
    add_edge(EN_ID_B, EN_ID_E, "be");

    let (ent, norm, nc) = gos_runtime::graph_entropy();
    assert_eq!(nc,   5,       "K_{{2,3}}: node_count=5");
    assert_eq!(ent,  673_011, "K_{{2,3}}: entropy_ppm");
    assert_eq!(norm, 418_165, "K_{{2,3}}: normalized_ppm");
}
