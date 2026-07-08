// gos-graph-topo17-harness — V3.28 Zagreb Coindices + Forgotten Coindex
//
// Verifies `gos_runtime::graph_topo_indices17()`:
//   Returns (mbar1, mbar2, fbar, edge_count, node_count)
//   - mbar1 = M̄₁(G) = Σ_{uv∉E, u≠v} (d_u + d_v)   (exact u64; Zagreb coindex 1)
//   - mbar2 = M̄₂(G) = Σ_{uv∉E, u≠v} d_u · d_v     (exact u64; Zagreb coindex 2)
//   - fbar  = F̄(G)  = Σ_{uv∉E, u≠v} (d_u² + d_v²) (exact u64; forgotten coindex)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// DEFINITIONS:
//   d_v = degree of v (undirected projection). Sums are over unordered pairs
//   {u,v} with u≠v and {u,v} ∉ E(G).
//
// CLOSED-FORM IDENTITIES (computed analytically, no complement scan):
//   Let M₁=Σ_v d_v², M₂=Σ_{uv∈E}d_u·d_v, F=Σ_v d_v³, m=|E|, n=|V|.
//     M̄₁ = 2m(n−1) − M₁
//     M̄₂ = 2m² − M₁/2 − M₂   (M₁ always even; # odd-degree nodes always even)
//     F̄  = (n−1)·M₁ − F
//
// KEY INVARIANTS:
//   M̄₁ = M̄₂ = F̄ = 0 for complete graphs K_n (no non-edges exist).
//   M̄₁ = M̄₂ = F̄ = 0 for empty graphs (no nodes → no pairs).
//   M̄₁ = M̄₂ = F̄ = 0 for the edge A-B (only one pair {A,B} ∈ E, no non-edges).
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph        M̄₁   M̄₂   F̄   edges  nodes
//  Empty           0     0    0      0      0
//  1 node          0     0    0      0      1
//  Edge A-B        0     0    0      1      2
//  Path P₃         2     1    2      2      3
//  Triangle K₃     0     0    0      3      3
//  Star K_{1,4}   12     6   12      4      5
//  Path P₄         8     5   12      3      4
//  Complete K₄     0     0    0      6      4
//  Two isolated    0     0    0      0      2
//  K_{2,3}        18    21   42      6      5
//
// Derivations:
//
//   Edge A-B (n=2, m=1, d_A=d_B=1):
//     M₁=2, M₂=1, F=2.
//     M̄₁ = 2·1·1 − 2 = 0. (no non-edges)
//     M̄₂ = 2·1 − 1 − 1 = 0.
//     F̄  = 1·2 − 2 = 0.
//
//   Path P₃ = A-B-C (n=3, m=2, d_A=d_C=1, d_B=2):
//     M₁=1+4+1=6, M₂=(1·2)+(2·1)=4, F=1+8+1=10.
//     M̄₁ = 2·2·2 − 6 = 8−6 = 2.  Non-edge {A,C}: (1+1)=2. ✓
//     M̄₂ = 2·4 − 3 − 4 = 8−7 = 1. Non-edge {A,C}: 1·1=1. ✓
//     F̄  = 2·6 − 10 = 2.           Non-edge {A,C}: (1+1)=2. ✓
//
//   Triangle K₃ (n=3, m=3, Δ=2):
//     M₁=12, M₂=12, F=24.
//     M̄₁ = 2·3·2 − 12 = 0. (complete K₃) ✓
//     M̄₂ = 2·9 − 6 − 12 = 0. ✓
//     F̄  = 2·12 − 24 = 0. ✓
//
//   Star K_{1,4} (n=5, m=4, d_ctr=4, d_leaf=1, 4 leaves):
//     M₁=16+4=20, M₂=4·4=16, F=64+4=68.
//     M̄₁ = 2·4·4 − 20 = 32−20 = 12.  6 non-edges (leaf pairs): 6·(1+1)=12. ✓
//     M̄₂ = 2·16 − 10 − 16 = 6.       6 non-edges: 6·(1·1)=6. ✓
//     F̄  = 4·20 − 68 = 80−68 = 12.   6 non-edges: 6·(1+1)=12. ✓
//
//   Path P₄ = A-B-C-D (n=4, m=3, d_A=d_D=1, d_B=d_C=2):
//     M₁=1+4+4+1=10, M₂=2+4+2=8, F=1+8+8+1=18.
//     M̄₁ = 2·3·3 − 10 = 18−10 = 8.
//       Non-edges: {A,C}:(1+2)=3, {A,D}:(1+1)=2, {B,D}:(2+1)=3. Sum=8. ✓
//     M̄₂ = 2·9 − 5 − 8 = 5.
//       Non-edges: {A,C}:1·2=2, {A,D}:1·1=1, {B,D}:2·1=2. Sum=5. ✓
//     F̄  = 3·10 − 18 = 12.
//       Non-edges: {A,C}:(1+4)=5, {A,D}:(1+1)=2, {B,D}:(4+1)=5. Sum=12. ✓
//
//   Complete K₄ (n=4, m=6, Δ=3):
//     M₁=36, M₂=54, F=108.
//     M̄₁ = 2·6·3 − 36 = 0. (complete K₄) ✓
//     M̄₂ = 2·36 − 18 − 54 = 0. ✓
//     F̄  = 3·36 − 108 = 0. ✓
//
//   K_{2,3} (n=5, left={A,B}:d=3, right={C,D,E}:d=2):
//     M₁=9+9+4+4+4=30, M₂=6·(3·2)=36, F=27+27+8+8+8=78.
//     M̄₁ = 2·6·4 − 30 = 48−30 = 18.
//       Non-edges: {A,B}:(3+3)=6; {C,D},{C,E},{D,E}:(2+2)=4×3=12. Sum=18. ✓
//     M̄₂ = 2·36 − 15 − 36 = 21.
//       Non-edges: {A,B}:3·3=9; {C,D},{C,E},{D,E}:2·2=4×3=12. Sum=21. ✓
//     F̄  = 4·30 − 78 = 120−78 = 42.
//       Non-edges: {A,B}:(9+9)=18; {C,D},{C,E},{D,E}:(4+4)=8×3=24. Sum=42. ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (0, 0, 0, 1, 2)
//  4.  Path P₃ = A-B-C                   → (2, 1, 2, 2, 3)
//  5.  Triangle K₃                       → (0, 0, 0, 3, 3)
//  6.  Star K_{1,4}                      → (12, 6, 12, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (8, 5, 12, 3, 4)
//  8.  Complete K₄                       → (0, 0, 0, 6, 4)
//  9.  Two isolated nodes                → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (18, 21, 42, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T17_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_17");
const T17_EXEC:   ExecutorId = ExecutorId::from_ascii("t17.exec");

const T17_KEY_A: &str = "t17.alpha";
const T17_KEY_B: &str = "t17.beta";
const T17_KEY_C: &str = "t17.gamma";
const T17_KEY_D: &str = "t17.delta";
const T17_KEY_E: &str = "t17.epsilon";

const T17_ID_A: NodeId = derive_node_id(T17_PLUGIN, T17_KEY_A);
const T17_ID_B: NodeId = derive_node_id(T17_PLUGIN, T17_KEY_B);
const T17_ID_C: NodeId = derive_node_id(T17_PLUGIN, T17_KEY_C);
const T17_ID_D: NodeId = derive_node_id(T17_PLUGIN, T17_KEY_D);
const T17_ID_E: NodeId = derive_node_id(T17_PLUGIN, T17_KEY_E);

// L4=104 namespace for this harness.
const T17_VEC_A: VectorAddress = VectorAddress::new(104, 1, 1, 0);
const T17_VEC_B: VectorAddress = VectorAddress::new(104, 1, 2, 0);
const T17_VEC_C: VectorAddress = VectorAddress::new(104, 1, 3, 0);
const T17_VEC_D: VectorAddress = VectorAddress::new(104, 2, 1, 0);
const T17_VEC_E: VectorAddress = VectorAddress::new(104, 2, 2, 0);

const T17_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T17_PLUGIN,
    name:         "kl-graph-topo17-harness",
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
        executor_id:       T17_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T17_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T17_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (mb1, mb2, fb, ec, nc) = gos_runtime::graph_topo_indices17();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(mb1, 0, "empty: M̄₁=0");
    assert_eq!(mb2, 0, "empty: M̄₂=0");
    assert_eq!(fb,  0, "empty: F̄=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T17_VEC_A, T17_KEY_A, T17_ID_A);

    let (mb1, mb2, fb, ec, nc) = gos_runtime::graph_topo_indices17();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(mb1, 0, "single: M̄₁=0 (only 1 node, no pairs)");
    assert_eq!(mb2, 0, "single: M̄₂=0");
    assert_eq!(fb,  0, "single: F̄=0");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// n=2, m=1. The only pair {A,B} is an edge — no non-edges.
// M̄₁ = 2·1·1 − 2 = 0. M̄₂ = 2·1 − 1 − 1 = 0. F̄ = 1·2 − 2 = 0.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T17_VEC_A, T17_KEY_A, T17_ID_A);
    add_node(T17_VEC_B, T17_KEY_B, T17_ID_B);
    add_edge(T17_ID_A, T17_ID_B, "t17.e.ab");

    let (mb1, mb2, fb, ec, nc) = gos_runtime::graph_topo_indices17();
    assert_eq!(nc,  2, "edge: node_count=2");
    assert_eq!(ec,  1, "edge: edge_count=1");
    assert_eq!(mb1, 0, "edge: M̄₁=0 (only pair A-B is an edge)");
    assert_eq!(mb2, 0, "edge: M̄₂=0");
    assert_eq!(fb,  0, "edge: F̄=0");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// n=3, m=2, d_A=d_C=1, d_B=2.
// M₁=6, M₂=4, F=10.
// M̄₁ = 8−6=2. Non-edge {A,C}: (1+1)=2. ✓
// M̄₂ = 8−3−4=1. Non-edge {A,C}: 1·1=1. ✓
// F̄  = 12−10=2. Non-edge {A,C}: (1+1)=2. ✓

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T17_VEC_A, T17_KEY_A, T17_ID_A);
    add_node(T17_VEC_B, T17_KEY_B, T17_ID_B);
    add_node(T17_VEC_C, T17_KEY_C, T17_ID_C);
    add_edge(T17_ID_A, T17_ID_B, "t17.e.ab");
    add_edge(T17_ID_B, T17_ID_C, "t17.e.bc");

    let (mb1, mb2, fb, ec, nc) = gos_runtime::graph_topo_indices17();
    assert_eq!(nc,  3, "p3: node_count=3");
    assert_eq!(ec,  2, "p3: edge_count=2");
    assert_eq!(mb1, 2, "p3: M̄₁=2 (non-edge {{A,C}}: 1+1=2)");
    assert_eq!(mb2, 1, "p3: M̄₂=1 (non-edge {{A,C}}: 1·1=1)");
    assert_eq!(fb,  2, "p3: F̄=2 (non-edge {{A,C}}: 1²+1²=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// Complete — no non-edges. All coindices = 0.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T17_VEC_A, T17_KEY_A, T17_ID_A);
    add_node(T17_VEC_B, T17_KEY_B, T17_ID_B);
    add_node(T17_VEC_C, T17_KEY_C, T17_ID_C);
    add_edge(T17_ID_A, T17_ID_B, "t17.e.ab");
    add_edge(T17_ID_B, T17_ID_A, "t17.e.ba");
    add_edge(T17_ID_B, T17_ID_C, "t17.e.bc");
    add_edge(T17_ID_C, T17_ID_B, "t17.e.cb");
    add_edge(T17_ID_A, T17_ID_C, "t17.e.ac");
    add_edge(T17_ID_C, T17_ID_A, "t17.e.ca");

    let (mb1, mb2, fb, ec, nc) = gos_runtime::graph_topo_indices17();
    assert_eq!(nc,  3, "k3: node_count=3");
    assert_eq!(ec,  3, "k3: edge_count=3");
    assert_eq!(mb1, 0, "k3: M̄₁=0 (complete K₃: no non-edges)");
    assert_eq!(mb2, 0, "k3: M̄₂=0 (complete K₃: no non-edges)");
    assert_eq!(fb,  0, "k3: F̄=0 (complete K₃: no non-edges)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// n=5, m=4. d_ctr=4, d_leaf=1.
// M₁=20, M₂=16, F=68.
// M̄₁ = 2·4·4 − 20 = 12. (6 leaf-leaf non-edges × 2)
// M̄₂ = 2·16 − 10 − 16 = 6. (6 leaf-leaf non-edges × 1)
// F̄  = 4·20 − 68 = 12. (6 leaf-leaf non-edges × 2)

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T17_VEC_A, T17_KEY_A, T17_ID_A);
    add_node(T17_VEC_B, T17_KEY_B, T17_ID_B);
    add_node(T17_VEC_C, T17_KEY_C, T17_ID_C);
    add_node(T17_VEC_D, T17_KEY_D, T17_ID_D);
    add_node(T17_VEC_E, T17_KEY_E, T17_ID_E);
    add_edge(T17_ID_A, T17_ID_B, "t17.e.ab");
    add_edge(T17_ID_A, T17_ID_C, "t17.e.ac");
    add_edge(T17_ID_A, T17_ID_D, "t17.e.ad");
    add_edge(T17_ID_A, T17_ID_E, "t17.e.ae");

    let (mb1, mb2, fb, ec, nc) = gos_runtime::graph_topo_indices17();
    assert_eq!(nc,   5, "star: node_count=5");
    assert_eq!(ec,   4, "star: edge_count=4");
    assert_eq!(mb1, 12, "star: M̄₁=12 (6 leaf-pairs x (1+1)=2)");
    assert_eq!(mb2,  6, "star: M̄₂=6 (6 leaf-pairs x 1*1=1)");
    assert_eq!(fb,  12, "star: F̄=12 (6 leaf-pairs x (1+1)=2)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// n=4, m=3, d_A=d_D=1, d_B=d_C=2.
// M₁=10, M₂=8, F=18.
// M̄₁ = 2·3·3 − 10 = 8.  {A,C}:3, {A,D}:2, {B,D}:3. Sum=8. ✓
// M̄₂ = 2·9 − 5 − 8 = 5. {A,C}:2, {A,D}:1, {B,D}:2. Sum=5. ✓
// F̄  = 3·10 − 18 = 12.  {A,C}:5, {A,D}:2, {B,D}:5. Sum=12. ✓

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T17_VEC_A, T17_KEY_A, T17_ID_A);
    add_node(T17_VEC_B, T17_KEY_B, T17_ID_B);
    add_node(T17_VEC_C, T17_KEY_C, T17_ID_C);
    add_node(T17_VEC_D, T17_KEY_D, T17_ID_D);
    add_edge(T17_ID_A, T17_ID_B, "t17.e.ab");
    add_edge(T17_ID_B, T17_ID_C, "t17.e.bc");
    add_edge(T17_ID_C, T17_ID_D, "t17.e.cd");

    let (mb1, mb2, fb, ec, nc) = gos_runtime::graph_topo_indices17();
    assert_eq!(nc,   4, "p4: node_count=4");
    assert_eq!(ec,   3, "p4: edge_count=3");
    assert_eq!(mb1,  8, "p4: M̄₁=8 (AC:3+AD:2+BD:3)");
    assert_eq!(mb2,  5, "p4: M̄₂=5 (AC:2+AD:1+BD:2)");
    assert_eq!(fb,  12, "p4: F̄=12 (AC:5+AD:2+BD:5)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// Complete — no non-edges. All coindices = 0.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T17_VEC_A, T17_KEY_A, T17_ID_A);
    add_node(T17_VEC_B, T17_KEY_B, T17_ID_B);
    add_node(T17_VEC_C, T17_KEY_C, T17_ID_C);
    add_node(T17_VEC_D, T17_KEY_D, T17_ID_D);
    add_edge(T17_ID_A, T17_ID_B, "t17.e.ab");
    add_edge(T17_ID_B, T17_ID_A, "t17.e.ba");
    add_edge(T17_ID_A, T17_ID_C, "t17.e.ac");
    add_edge(T17_ID_C, T17_ID_A, "t17.e.ca");
    add_edge(T17_ID_A, T17_ID_D, "t17.e.ad");
    add_edge(T17_ID_D, T17_ID_A, "t17.e.da");
    add_edge(T17_ID_B, T17_ID_C, "t17.e.bc");
    add_edge(T17_ID_C, T17_ID_B, "t17.e.cb");
    add_edge(T17_ID_B, T17_ID_D, "t17.e.bd");
    add_edge(T17_ID_D, T17_ID_B, "t17.e.db");
    add_edge(T17_ID_C, T17_ID_D, "t17.e.cd");
    add_edge(T17_ID_D, T17_ID_C, "t17.e.dc");

    let (mb1, mb2, fb, ec, nc) = gos_runtime::graph_topo_indices17();
    assert_eq!(nc,  4, "k4: node_count=4");
    assert_eq!(ec,  6, "k4: edge_count=6");
    assert_eq!(mb1, 0, "k4: M̄₁=0 (complete K₄: no non-edges)");
    assert_eq!(mb2, 0, "k4: M̄₂=0 (complete K₄: no non-edges)");
    assert_eq!(fb,  0, "k4: F̄=0 (complete K₄: no non-edges)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// n=2, m=0. One non-edge {A,B}: d_A=d_B=0.
// M̄₁ = 2·0·1 − 0 = 0. M̄₂ = 0. F̄ = 0.
// (All degrees are 0 so the sums are vacuously 0.)

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T17_VEC_A, T17_KEY_A, T17_ID_A);
    add_node(T17_VEC_B, T17_KEY_B, T17_ID_B);

    let (mb1, mb2, fb, ec, nc) = gos_runtime::graph_topo_indices17();
    assert_eq!(nc,  2, "isolated: node_count=2");
    assert_eq!(ec,  0, "isolated: no edges");
    assert_eq!(mb1, 0, "isolated: M̄₁=0 (d=0 everywhere)");
    assert_eq!(mb2, 0, "isolated: M̄₂=0");
    assert_eq!(fb,  0, "isolated: F̄=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}:d=3. Right={C,D,E}:d=2. n=5, m=6.
// M₁=30, M₂=36, F=78.
// M̄₁ = 2·6·4 − 30 = 18.  {A,B}:6; {C,D},{C,E},{D,E}:4×3=12. Sum=18. ✓
// M̄₂ = 2·36 − 15 − 36 = 21. {A,B}:9; leaf-pairs:4×3=12. Sum=21. ✓
// F̄  = 4·30 − 78 = 42.   {A,B}:18; leaf-pairs:8×3=24. Sum=42. ✓

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T17_VEC_A, T17_KEY_A, T17_ID_A);
    add_node(T17_VEC_B, T17_KEY_B, T17_ID_B);
    add_node(T17_VEC_C, T17_KEY_C, T17_ID_C);
    add_node(T17_VEC_D, T17_KEY_D, T17_ID_D);
    add_node(T17_VEC_E, T17_KEY_E, T17_ID_E);
    // Left A connects to all right
    add_edge(T17_ID_A, T17_ID_C, "t17.e.ac");
    add_edge(T17_ID_C, T17_ID_A, "t17.e.ca");
    add_edge(T17_ID_A, T17_ID_D, "t17.e.ad");
    add_edge(T17_ID_D, T17_ID_A, "t17.e.da");
    add_edge(T17_ID_A, T17_ID_E, "t17.e.ae");
    add_edge(T17_ID_E, T17_ID_A, "t17.e.ea");
    // Left B connects to all right
    add_edge(T17_ID_B, T17_ID_C, "t17.e.bc");
    add_edge(T17_ID_C, T17_ID_B, "t17.e.cb");
    add_edge(T17_ID_B, T17_ID_D, "t17.e.bd");
    add_edge(T17_ID_D, T17_ID_B, "t17.e.db");
    add_edge(T17_ID_B, T17_ID_E, "t17.e.be");
    add_edge(T17_ID_E, T17_ID_B, "t17.e.eb");

    let (mb1, mb2, fb, ec, nc) = gos_runtime::graph_topo_indices17();
    assert_eq!(nc,   5, "k23: node_count=5");
    assert_eq!(ec,   6, "k23: edge_count=6");
    assert_eq!(mb1, 18, "k23: M̄₁=18 (left-pair:6 + 3 right-pairs x 4=12)");
    assert_eq!(mb2, 21, "k23: M̄₂=21 (left-pair:9 + 3 right-pairs x 4=12)");
    assert_eq!(fb,  42, "k23: F̄=42 (left-pair:18 + 3 right-pairs x 8=24)");
}
