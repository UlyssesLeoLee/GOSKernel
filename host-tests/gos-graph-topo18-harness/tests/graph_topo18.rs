// gos-graph-topo18-harness — V3.29 Neighborhood Zagreb NM₁ + NM₂ + GA₂
//
// Verifies `gos_runtime::graph_topo_indices18()`:
//   Returns (nm1, nm2, ga2_ppm, edge_count, node_count)
//   - nm1     = NM₁(G) = Σ_v S(v)²                              (exact u64)
//   - nm2     = NM₂(G) = Σ_{uv∈E} S(u)·S(v)                    (exact u64)
//   - ga2_ppm = GA₂(G) × 10^6 = Σ_{uv∈E} floor(2√(S·S)/(S+S) × 10^6) (floor ppm)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// DEFINITION:
//   S(v) = Σ_{u∈N(v)} deg(u)  — sum of degrees of v's neighbors.
//
// ALGORITHM: O(V+E) — compute S(v) from adj+deg; then edge scan.
//   GA₂ per edge = isqrt128(4·S_u·S_v·10^12) / (S_u+S_v)
//   When S_u=S_v: GA₂ per edge = 10^6 exactly (S-uniform graph).
//
// S-UNIFORM INVARIANT: GA₂ = |E|×10^6 when all S(v) are equal.
//   Graphs that are S-uniform: P₃, K₃, K₄, K_{1,4}, K_{2,3}, K_n, K_{r,s}.
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph         NM₁   NM₂    GA₂(ppm)    edges  nodes   S-values
//  Empty           0     0           0      0      0
//  1 node          0     0           0      0      1     S(A)=0
//  Edge A-B        2     1   1_000_000      1      2     S(A)=S(B)=1
//  Path P₃        12     8   2_000_000      2      3     S(v)=2 all
//  Triangle K₃    48    48   3_000_000      3      3     S(v)=4 all
//  Star K_{1,4}   80    64   4_000_000      4      5     S(v)=4 all
//  Path P₄        26    21   2_959_590      3      4     S=(2,3,3,2)
//  Complete K₄   324   486   6_000_000      6      4     S(v)=9 all
//  Two isolated    0     0           0      0      2     S=0 all
//  K_{2,3}       180   216   6_000_000      6      5     S(v)=6 all
//
// Derivations:
//
//   Edge A-B (d_A=d_B=1):
//     S(A)=d_B=1, S(B)=d_A=1.
//     NM₁=1+1=2. NM₂=1·1=1. GA₂=2√(1·1)/(1+1)×10^6=10^6. ✓
//
//   Path P₃ = A-B-C (d_A=d_C=1, d_B=2):
//     S(A)=d_B=2, S(B)=d_A+d_C=1+1=2, S(C)=d_B=2. All S=2.
//     NM₁=3·4=12. NM₂=2·(2·2)=8. GA₂=2·10^6=2_000_000 (S-uniform). ✓
//
//   Triangle K₃ (Δ=2):
//     S(v)=2+2=4 for all. NM₁=3·16=48. NM₂=3·16=48. GA₂=3·10^6=3_000_000. ✓
//
//   Star K_{1,4} (center A:d=4, leaves B-E:d=1):
//     S(A)=4·1=4. S(leaf)=d_A=4. All S=4.
//     NM₁=5·16=80. NM₂=4·16=64. GA₂=4·10^6=4_000_000 (S-uniform). ✓
//
//   Path P₄ = A-B-C-D (d_A=d_D=1, d_B=d_C=2):
//     S(A)=d_B=2, S(B)=d_A+d_C=1+2=3, S(C)=d_B+d_D=2+1=3, S(D)=d_C=2.
//     NM₁=4+9+9+4=26. NM₂: {A,B}:2·3=6; {B,C}:3·3=9; {C,D}:3·2=6. NM₂=21.
//     GA₂: {A,B}(S=2,3): isqrt128(4·6·10^12)/(5)=isqrt128(24·10^12)/5.
//       √(24·10^12)=2√6·10^6≈4_898_979.48. isqrt128=4_898_979. 4_898_979/5=979_795.
//       {B,C}(S=3,3): isqrt128(4·9·10^12)/6=6·10^6/6=1_000_000.
//       {C,D}=979_795. GA₂=979_795+1_000_000+979_795=2_959_590. ✓
//
//   Complete K₄ (Δ=3): S(v)=3·3=9. NM₁=4·81=324. NM₂=6·81=486. GA₂=6·10^6. ✓
//
//   K_{2,3} (left={A,B}:d=3, right={C,D,E}:d=2):
//     S(A)=S(B)=2+2+2=6. S(C)=S(D)=S(E)=3+3=6. All S=6.
//     NM₁=5·36=180. NM₂=6·36=216. GA₂=6·10^6=6_000_000 (S-uniform). ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (2, 1, 1_000_000, 1, 2)
//  4.  Path P₃ = A-B-C                   → (12, 8, 2_000_000, 2, 3)
//  5.  Triangle K₃                       → (48, 48, 3_000_000, 3, 3)
//  6.  Star K_{1,4}                      → (80, 64, 4_000_000, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (26, 21, 2_959_590, 3, 4)
//  8.  Complete K₄                       → (324, 486, 6_000_000, 6, 4)
//  9.  Two isolated nodes                → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (180, 216, 6_000_000, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T18_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_18");
const T18_EXEC:   ExecutorId = ExecutorId::from_ascii("t18.exec");

const T18_KEY_A: &str = "t18.alpha";
const T18_KEY_B: &str = "t18.beta";
const T18_KEY_C: &str = "t18.gamma";
const T18_KEY_D: &str = "t18.delta";
const T18_KEY_E: &str = "t18.epsilon";

const T18_ID_A: NodeId = derive_node_id(T18_PLUGIN, T18_KEY_A);
const T18_ID_B: NodeId = derive_node_id(T18_PLUGIN, T18_KEY_B);
const T18_ID_C: NodeId = derive_node_id(T18_PLUGIN, T18_KEY_C);
const T18_ID_D: NodeId = derive_node_id(T18_PLUGIN, T18_KEY_D);
const T18_ID_E: NodeId = derive_node_id(T18_PLUGIN, T18_KEY_E);

// L4=105 namespace for this harness.
const T18_VEC_A: VectorAddress = VectorAddress::new(105, 1, 1, 0);
const T18_VEC_B: VectorAddress = VectorAddress::new(105, 1, 2, 0);
const T18_VEC_C: VectorAddress = VectorAddress::new(105, 1, 3, 0);
const T18_VEC_D: VectorAddress = VectorAddress::new(105, 2, 1, 0);
const T18_VEC_E: VectorAddress = VectorAddress::new(105, 2, 2, 0);

const T18_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T18_PLUGIN,
    name:         "kl-graph-topo18-harness",
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
        executor_id:       T18_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T18_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T18_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nm1, nm2, ga2, ec, nc) = gos_runtime::graph_topo_indices18();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(nm1, 0, "empty: NM₁=0");
    assert_eq!(nm2, 0, "empty: NM₂=0");
    assert_eq!(ga2, 0, "empty: GA₂=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T18_VEC_A, T18_KEY_A, T18_ID_A);

    let (nm1, nm2, ga2, ec, nc) = gos_runtime::graph_topo_indices18();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(nm1, 0, "single: NM₁=0 (S(A)=0, isolated)");
    assert_eq!(nm2, 0, "single: NM₂=0");
    assert_eq!(ga2, 0, "single: GA₂=0");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// S(A)=deg(B)=1. S(B)=deg(A)=1.
// NM₁=1+1=2. NM₂=1·1=1. GA₂=2√(1·1)/(1+1)×10^6=1_000_000.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T18_VEC_A, T18_KEY_A, T18_ID_A);
    add_node(T18_VEC_B, T18_KEY_B, T18_ID_B);
    add_edge(T18_ID_A, T18_ID_B, "t18.e.ab");

    let (nm1, nm2, ga2, ec, nc) = gos_runtime::graph_topo_indices18();
    assert_eq!(nc,  2,         "edge: node_count=2");
    assert_eq!(ec,  1,         "edge: edge_count=1");
    assert_eq!(nm1, 2,         "edge: NM₁=2 (S(A)=S(B)=1, Σ 1²+1²=2)");
    assert_eq!(nm2, 1,         "edge: NM₂=1 (1·1=1)");
    assert_eq!(ga2, 1_000_000, "edge: GA₂=1_000_000 (S-uniform, S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=1+1=2, S(C)=deg(B)=2.
// All S=2 → S-uniform. NM₁=12. NM₂=8. GA₂=2_000_000.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T18_VEC_A, T18_KEY_A, T18_ID_A);
    add_node(T18_VEC_B, T18_KEY_B, T18_ID_B);
    add_node(T18_VEC_C, T18_KEY_C, T18_ID_C);
    add_edge(T18_ID_A, T18_ID_B, "t18.e.ab");
    add_edge(T18_ID_B, T18_ID_C, "t18.e.bc");

    let (nm1, nm2, ga2, ec, nc) = gos_runtime::graph_topo_indices18();
    assert_eq!(nc,  3,         "p3: node_count=3");
    assert_eq!(ec,  2,         "p3: edge_count=2");
    assert_eq!(nm1, 12,        "p3: NM₁=12 (3×2²=12, all S=2)");
    assert_eq!(nm2, 8,         "p3: NM₂=8 (2×2·2=8)");
    assert_eq!(ga2, 2_000_000, "p3: GA₂=2_000_000 (S-uniform, S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S(v)=2+2=4 for all v. S-uniform.
// NM₁=3·16=48. NM₂=3·16=48. GA₂=3_000_000.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T18_VEC_A, T18_KEY_A, T18_ID_A);
    add_node(T18_VEC_B, T18_KEY_B, T18_ID_B);
    add_node(T18_VEC_C, T18_KEY_C, T18_ID_C);
    add_edge(T18_ID_A, T18_ID_B, "t18.e.ab");
    add_edge(T18_ID_B, T18_ID_A, "t18.e.ba");
    add_edge(T18_ID_B, T18_ID_C, "t18.e.bc");
    add_edge(T18_ID_C, T18_ID_B, "t18.e.cb");
    add_edge(T18_ID_A, T18_ID_C, "t18.e.ac");
    add_edge(T18_ID_C, T18_ID_A, "t18.e.ca");

    let (nm1, nm2, ga2, ec, nc) = gos_runtime::graph_topo_indices18();
    assert_eq!(nc,  3,         "k3: node_count=3");
    assert_eq!(ec,  3,         "k3: edge_count=3");
    assert_eq!(nm1, 48,        "k3: NM₁=48 (3×4²=48, all S=4)");
    assert_eq!(nm2, 48,        "k3: NM₂=48 (3×4·4=48)");
    assert_eq!(ga2, 3_000_000, "k3: GA₂=3_000_000 (S-uniform, S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S(center A)=4·1=4. S(leaf)=deg(A)=4. All S=4 → S-uniform.
// NM₁=5·16=80. NM₂=4·16=64. GA₂=4_000_000.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T18_VEC_A, T18_KEY_A, T18_ID_A);
    add_node(T18_VEC_B, T18_KEY_B, T18_ID_B);
    add_node(T18_VEC_C, T18_KEY_C, T18_ID_C);
    add_node(T18_VEC_D, T18_KEY_D, T18_ID_D);
    add_node(T18_VEC_E, T18_KEY_E, T18_ID_E);
    add_edge(T18_ID_A, T18_ID_B, "t18.e.ab");
    add_edge(T18_ID_A, T18_ID_C, "t18.e.ac");
    add_edge(T18_ID_A, T18_ID_D, "t18.e.ad");
    add_edge(T18_ID_A, T18_ID_E, "t18.e.ae");

    let (nm1, nm2, ga2, ec, nc) = gos_runtime::graph_topo_indices18();
    assert_eq!(nc,   5,        "star: node_count=5");
    assert_eq!(ec,   4,        "star: edge_count=4");
    assert_eq!(nm1,  80,       "star: NM₁=80 (5×4²=80, all S=4)");
    assert_eq!(nm2,  64,       "star: NM₂=64 (4×4·4=64)");
    assert_eq!(ga2, 4_000_000, "star: GA₂=4_000_000 (S-uniform, S=4)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d_A=d_D=1, d_B=d_C=2.
// S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=1+2=3, S(C)=deg(B)+deg(D)=2+1=3, S(D)=deg(C)=2.
// NM₁=4+9+9+4=26. NM₂: {A,B}:6; {B,C}:9; {C,D}:6. NM₂=21.
// GA₂: {A,B}(2,3):979_795; {B,C}(3,3):1_000_000; {C,D}(3,2):979_795. Sum=2_959_590.
//   Derivation: isqrt128(4·2·3·10^12)/5 = isqrt128(24·10^12)/5.
//   √(24·10^12)=2√6·10^6≈4_898_979.485. isqrt=4_898_979. 4_898_979/5=979_795 (r=4). ✓

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T18_VEC_A, T18_KEY_A, T18_ID_A);
    add_node(T18_VEC_B, T18_KEY_B, T18_ID_B);
    add_node(T18_VEC_C, T18_KEY_C, T18_ID_C);
    add_node(T18_VEC_D, T18_KEY_D, T18_ID_D);
    add_edge(T18_ID_A, T18_ID_B, "t18.e.ab");
    add_edge(T18_ID_B, T18_ID_C, "t18.e.bc");
    add_edge(T18_ID_C, T18_ID_D, "t18.e.cd");

    let (nm1, nm2, ga2, ec, nc) = gos_runtime::graph_topo_indices18();
    assert_eq!(nc,   4,        "p4: node_count=4");
    assert_eq!(ec,   3,        "p4: edge_count=3");
    assert_eq!(nm1,  26,       "p4: NM₁=26 (4+9+9+4)");
    assert_eq!(nm2,  21,       "p4: NM₂=21 ({{A,B}}:6+{{B,C}}:9+{{C,D}}:6)");
    assert_eq!(ga2, 2_959_590, "p4: GA₂=2_959_590 (979_795+1_000_000+979_795)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S(v)=3+3+3=9 for all v. S-uniform.
// NM₁=4·81=324. NM₂=6·81=486. GA₂=6_000_000.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T18_VEC_A, T18_KEY_A, T18_ID_A);
    add_node(T18_VEC_B, T18_KEY_B, T18_ID_B);
    add_node(T18_VEC_C, T18_KEY_C, T18_ID_C);
    add_node(T18_VEC_D, T18_KEY_D, T18_ID_D);
    add_edge(T18_ID_A, T18_ID_B, "t18.e.ab");
    add_edge(T18_ID_B, T18_ID_A, "t18.e.ba");
    add_edge(T18_ID_A, T18_ID_C, "t18.e.ac");
    add_edge(T18_ID_C, T18_ID_A, "t18.e.ca");
    add_edge(T18_ID_A, T18_ID_D, "t18.e.ad");
    add_edge(T18_ID_D, T18_ID_A, "t18.e.da");
    add_edge(T18_ID_B, T18_ID_C, "t18.e.bc");
    add_edge(T18_ID_C, T18_ID_B, "t18.e.cb");
    add_edge(T18_ID_B, T18_ID_D, "t18.e.bd");
    add_edge(T18_ID_D, T18_ID_B, "t18.e.db");
    add_edge(T18_ID_C, T18_ID_D, "t18.e.cd");
    add_edge(T18_ID_D, T18_ID_C, "t18.e.dc");

    let (nm1, nm2, ga2, ec, nc) = gos_runtime::graph_topo_indices18();
    assert_eq!(nc,   4,        "k4: node_count=4");
    assert_eq!(ec,   6,        "k4: edge_count=6");
    assert_eq!(nm1, 324,       "k4: NM₁=324 (4×9²=324, all S=9)");
    assert_eq!(nm2, 486,       "k4: NM₂=486 (6×9·9=486)");
    assert_eq!(ga2, 6_000_000, "k4: GA₂=6_000_000 (S-uniform, S=9)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S(A)=S(B)=0 (no neighbors). NM₁=0, NM₂=0, GA₂=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T18_VEC_A, T18_KEY_A, T18_ID_A);
    add_node(T18_VEC_B, T18_KEY_B, T18_ID_B);

    let (nm1, nm2, ga2, ec, nc) = gos_runtime::graph_topo_indices18();
    assert_eq!(nc,  2, "isolated: node_count=2");
    assert_eq!(ec,  0, "isolated: no edges");
    assert_eq!(nm1, 0, "isolated: NM₁=0 (S=0 everywhere)");
    assert_eq!(nm2, 0, "isolated: NM₂=0");
    assert_eq!(ga2, 0, "isolated: GA₂=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}:d=3. Right={C,D,E}:d=2. n=5, m=6.
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=2+2+2=6.
// S(C)=S(D)=S(E)=deg(A)+deg(B)=3+3=6.
// All S=6 → S-uniform. NM₁=5·36=180. NM₂=6·36=216. GA₂=6_000_000. ✓

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T18_VEC_A, T18_KEY_A, T18_ID_A);
    add_node(T18_VEC_B, T18_KEY_B, T18_ID_B);
    add_node(T18_VEC_C, T18_KEY_C, T18_ID_C);
    add_node(T18_VEC_D, T18_KEY_D, T18_ID_D);
    add_node(T18_VEC_E, T18_KEY_E, T18_ID_E);
    // Left A connects to all right
    add_edge(T18_ID_A, T18_ID_C, "t18.e.ac");
    add_edge(T18_ID_C, T18_ID_A, "t18.e.ca");
    add_edge(T18_ID_A, T18_ID_D, "t18.e.ad");
    add_edge(T18_ID_D, T18_ID_A, "t18.e.da");
    add_edge(T18_ID_A, T18_ID_E, "t18.e.ae");
    add_edge(T18_ID_E, T18_ID_A, "t18.e.ea");
    // Left B connects to all right
    add_edge(T18_ID_B, T18_ID_C, "t18.e.bc");
    add_edge(T18_ID_C, T18_ID_B, "t18.e.cb");
    add_edge(T18_ID_B, T18_ID_D, "t18.e.bd");
    add_edge(T18_ID_D, T18_ID_B, "t18.e.db");
    add_edge(T18_ID_B, T18_ID_E, "t18.e.be");
    add_edge(T18_ID_E, T18_ID_B, "t18.e.eb");

    let (nm1, nm2, ga2, ec, nc) = gos_runtime::graph_topo_indices18();
    assert_eq!(nc,   5,        "k23: node_count=5");
    assert_eq!(ec,   6,        "k23: edge_count=6");
    assert_eq!(nm1, 180,       "k23: NM₁=180 (5×6²=180, all S=6)");
    assert_eq!(nm2, 216,       "k23: NM₂=216 (6×6·6=216)");
    assert_eq!(ga2, 6_000_000, "k23: GA₂=6_000_000 (S-uniform, S=6)");
}
