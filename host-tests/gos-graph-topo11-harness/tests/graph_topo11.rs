// gos-graph-topo11-harness — V3.22 transmission-based topological indices
//
// Verifies `gos_runtime::graph_topo_indices11()`:
//   Returns (j_ppm, ti, piv, edge_count, node_count)
//   - j_ppm  = J(G) × 10^6  (floor ppm; Balaban 1982)
//             J = (m/μ) × Σ_{uv∈E} 1/√(T_u·T_v)
//             μ = max(1, m−n+2);  T_v = vertex transmittance = Σ_{w reachable} d(v,w)
//   - ti     = TI(G) = Σ_{uv∈E} |T_u − T_v|  (exact u64; Abdo & Dimitrov 2014)
//   - piv    = PI_v(G) = Σ_{uv∈E} (T_u + T_v) (exact u64; Khalifeh et al. 2008)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// DEFINITIONS:
//   T_v = Σ_{w: d(v,w) finite, w≠v} d(v,w)  (within-component BFS distance sum)
//   For any edge {u,v}: T_u ≥ 1 and T_v ≥ 1 (both in same component)
//   J contribution per edge: floor(10^6/√(T_u·T_v)) = isqrt64(10^12/(T_u·T_v))
//     via identity floor(A/√B) = floor(√(A²/B)) for positive A,B
//   μ = max(1, m−n+2): cyclomatic-number proxy; = 1 for trees; ≥ 2 for unicyclic+
//
// KEY INVARIANTS:
//   J: for K_n (n≥2): T_v=n-1 ∀v; μ=(n-1)(n-2)/2+2-n+2=?; all edges same.
//   TI = 0 iff graph is transmission-regular (all T_v equal, e.g. vertex-transitive).
//   PI_v = Σ_v deg(v)·T_v  (equivalent degree-weighted-transmission formula).
//   For trees: μ=1 so J_ppm = m × Σ_e 1/√(T_a·T_b) × 10^6.
//   J_ppm is a floor approximation; error ≤ m/μ ppm per edge contribution.
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph           J_ppm       TI   PI_v  edges  nodes
//  Empty           0           0     0     0      0
//  1 node          0           0     0     0      1
//  Edge A-B        1_000_000   0     2     1      2
//  Path P₃         1_632_992   2    10     2      3
//  Triangle K₃     2_250_000   0    12     3      3
//  Star K_{1,4}    3_023_712  12    44     4      5
//  Path P₄         1_974_744   4    28     3      4
//  Complete K₄     2_999_997   0    36     6      4
//  Two isolated    0           0     0     0      2
//  K_{2,3}         2_190_888   6    66     6      5
//
// Derivations:
//   Edge A-B: T_A=T_B=1. μ=max(1,1-2+2)=1.
//     j_raw=isqrt64(10^12/1)=1_000_000; J_ppm=1_000_000×1/1=1_000_000.
//     TI=|1-1|=0. PI_v=1+1=2.
//
//   P₃ (A-B-C): T_A=1+2=3; T_B=1+1=2; T_C=2+1=3. μ=max(1,2-3+2)=1.
//     {A,B}: isqrt64(10^12/6)=408248; {B,C}: same.
//     J_ppm=816496×2/1=1_632_992. TI=|3-2|+|2-3|=2. PI_v=5+5=10.
//
//   K₃: T_u=2 ∀u. μ=max(1,3-3+2)=2.
//     j_raw=3×isqrt64(10^12/4)=3×500_000=1_500_000.
//     J_ppm=1_500_000×3/2=2_250_000. TI=0. PI_v=3×4=12.
//
//   K_{1,4}: T(center)=4; T(leaf)=1+2+2+2=7. μ=max(1,4-5+2)=1.
//     j_raw=4×isqrt64(10^12/28)=4×188982=755928.
//     J_ppm=755928×4/1=3_023_712. TI=4×|4-7|=12. PI_v=4×11=44.
//
//   P₄ (A-B-C-D): T_A=6;T_B=4;T_C=4;T_D=6. μ=max(1,3-4+2)=1.
//     {A,B}=isqrt64(10^12/24)=204124; {B,C}=isqrt64(10^12/16)=250_000;
//     {C,D}=204124. J_ppm=658248×3/1=1_974_744. TI=2+0+2=4. PI_v=10+8+10=28.
//
//   K₄: T_u=3 ∀u. μ=max(1,6-4+2)=4.
//     j_raw=6×isqrt64(10^12/9)=6×333_333=1_999_998.
//     J_ppm=1_999_998×6/4=2_999_997. TI=0. PI_v=6×6=36.
//
//   K_{2,3}: T(left=A,B)=2+1+1+1=5; T(right=C,D,E)=1+1+2+2=6. μ=max(1,6-5+2)=3.
//     j_raw=6×isqrt64(10^12/30)=6×182_574=1_095_444.
//     J_ppm=1_095_444×6/3=2_190_888. TI=6×|5-6|=6. PI_v=6×11=66.
//
// isqrt64 values used above:
//   isqrt64(10^12/1)  = 1_000_000  (exact: √1_000_000_000_000 = 1_000_000)
//   isqrt64(10^12/4)  =   500_000  (exact: √250_000_000_000 = 500_000)
//   isqrt64(10^12/6)  =   408_248  (floor: √166_666_666_666 = 408248.29…)
//   isqrt64(10^12/9)  =   333_333  (floor: √111_111_111_111 = 333333.33…)
//   isqrt64(10^12/16) =   250_000  (exact: √62_500_000_000 = 250_000)
//   isqrt64(10^12/24) =   204_124  (floor: √41_666_666_666 = 204124.14…)
//   isqrt64(10^12/28) =   188_982  (floor: √35_714_285_714 = 188982.23…)
//   isqrt64(10^12/30) =   182_574  (floor: √33_333_333_333 = 182574.18…)
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (1_000_000, 0, 2, 1, 2)
//  4.  Path P₃ = A-B-C                    → (1_632_992, 2, 10, 2, 3)
//  5.  Triangle K₃                        → (2_250_000, 0, 12, 3, 3)
//  6.  Star K_{1,4}                       → (3_023_712, 12, 44, 4, 5)
//  7.  Path P₄ = A-B-C-D                  → (1_974_744, 4, 28, 3, 4)
//  8.  Complete K₄                        → (2_999_997, 0, 36, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check      → (2_190_888, 6, 66, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T11_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_11");
const T11_EXEC:   ExecutorId = ExecutorId::from_ascii("t11.exec");

const T11_KEY_A: &str = "t11.alpha";
const T11_KEY_B: &str = "t11.beta";
const T11_KEY_C: &str = "t11.gamma";
const T11_KEY_D: &str = "t11.delta";
const T11_KEY_E: &str = "t11.epsilon";

const T11_ID_A: NodeId = derive_node_id(T11_PLUGIN, T11_KEY_A);
const T11_ID_B: NodeId = derive_node_id(T11_PLUGIN, T11_KEY_B);
const T11_ID_C: NodeId = derive_node_id(T11_PLUGIN, T11_KEY_C);
const T11_ID_D: NodeId = derive_node_id(T11_PLUGIN, T11_KEY_D);
const T11_ID_E: NodeId = derive_node_id(T11_PLUGIN, T11_KEY_E);

// L4=98 namespace for this harness.
const T11_VEC_A: VectorAddress = VectorAddress::new(98, 1, 1, 0);
const T11_VEC_B: VectorAddress = VectorAddress::new(98, 1, 2, 0);
const T11_VEC_C: VectorAddress = VectorAddress::new(98, 1, 3, 0);
const T11_VEC_D: VectorAddress = VectorAddress::new(98, 2, 1, 0);
const T11_VEC_E: VectorAddress = VectorAddress::new(98, 2, 2, 0);

const T11_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T11_PLUGIN,
    name:         "kl-graph-topo11-harness",
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
        executor_id:       T11_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T11_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T11_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
// No nodes. All values are 0.

#[test]
fn test_01_empty() {
    let _g = setup();

    let (j, ti, piv, ec, nc) = gos_runtime::graph_topo_indices11();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(j,   0, "empty: J=0");
    assert_eq!(ti,  0, "empty: TI=0");
    assert_eq!(piv, 0, "empty: PI_v=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// No edges. All indices are 0.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T11_VEC_A, T11_KEY_A, T11_ID_A);

    let (j, ti, piv, ec, nc) = gos_runtime::graph_topo_indices11();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(j,   0, "single: J=0");
    assert_eq!(ti,  0, "single: TI=0");
    assert_eq!(piv, 0, "single: PI_v=0");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// T_A = T_B = 1. μ=max(1,1-2+2)=1.
// j_raw = isqrt64(10^12/1) = 1_000_000. J_ppm = 1_000_000×1/1 = 1_000_000.
// TI = |1-1| = 0. PI_v = 1+1 = 2.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T11_VEC_A, T11_KEY_A, T11_ID_A);
    add_node(T11_VEC_B, T11_KEY_B, T11_ID_B);
    add_edge(T11_ID_A, T11_ID_B, "t11.e.ab");

    let (j, ti, piv, ec, nc) = gos_runtime::graph_topo_indices11();
    assert_eq!(nc,  2,         "edge: node_count=2");
    assert_eq!(ec,  1,         "edge: edge_count=1");
    assert_eq!(j,   1_000_000, "edge: J_ppm=1_000_000 (exact: J=1, T_A=T_B=1)");
    assert_eq!(ti,  0,         "edge: TI=0 (T_A=T_B=1, transmission-regular)");
    assert_eq!(piv, 2,         "edge: PI_v=2 (T_A+T_B=1+1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// T_A=3, T_B=2, T_C=3. μ=max(1,2-3+2)=1.
// j_raw = 2×isqrt64(10^12/6) = 2×408248 = 816496.
// J_ppm = 816496×2/1 = 1_632_992.
// TI = |3-2|+|2-3| = 2. PI_v = (3+2)+(2+3) = 10.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T11_VEC_A, T11_KEY_A, T11_ID_A);
    add_node(T11_VEC_B, T11_KEY_B, T11_ID_B);
    add_node(T11_VEC_C, T11_KEY_C, T11_ID_C);
    add_edge(T11_ID_A, T11_ID_B, "t11.e.ab");
    add_edge(T11_ID_B, T11_ID_C, "t11.e.bc");

    let (j, ti, piv, ec, nc) = gos_runtime::graph_topo_indices11();
    assert_eq!(nc,  3,         "p3: node_count=3");
    assert_eq!(ec,  2,         "p3: edge_count=2");
    assert_eq!(j,   1_632_992, "p3: J_ppm=1_632_992 (floor; exact J=4/\u{221a}6\u{2248}1.6330)");
    assert_eq!(ti,  2,         "p3: TI=2 (|3-2|+|2-3|)");
    assert_eq!(piv, 10,        "p3: PI_v=10 ((3+2)+(2+3))");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// T_u=2 ∀u. μ=max(1,3-3+2)=2.
// j_raw = 3×isqrt64(10^12/4) = 3×500_000 = 1_500_000.
// J_ppm = 1_500_000×3/2 = 2_250_000 (exact: J=9/4).
// TI=0 (transmission-regular). PI_v=3×(2+2)=12.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T11_VEC_A, T11_KEY_A, T11_ID_A);
    add_node(T11_VEC_B, T11_KEY_B, T11_ID_B);
    add_node(T11_VEC_C, T11_KEY_C, T11_ID_C);
    add_edge(T11_ID_A, T11_ID_B, "t11.e.ab");
    add_edge(T11_ID_B, T11_ID_A, "t11.e.ba");
    add_edge(T11_ID_B, T11_ID_C, "t11.e.bc");
    add_edge(T11_ID_C, T11_ID_B, "t11.e.cb");
    add_edge(T11_ID_A, T11_ID_C, "t11.e.ac");
    add_edge(T11_ID_C, T11_ID_A, "t11.e.ca");

    let (j, ti, piv, ec, nc) = gos_runtime::graph_topo_indices11();
    assert_eq!(nc,  3,         "k3: node_count=3");
    assert_eq!(ec,  3,         "k3: edge_count=3");
    assert_eq!(j,   2_250_000, "k3: J_ppm=2_250_000 (exact: J=9/4=2.25; T_u=2, μ=2)");
    assert_eq!(ti,  0,         "k3: TI=0 (vertex-transitive: all T_u=2)");
    assert_eq!(piv, 12,        "k3: PI_v=12 (3 edges × (2+2))");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Center=A: T(A)=4 (4 leaves at d=1).
// Leaf=B,C,D,E: T(leaf)=1+2+2+2=7 (center at d=1, other 3 leaves at d=2).
// μ=max(1,4-5+2)=1.
// j_raw = 4×isqrt64(10^12/28) = 4×188982 = 755928.
// J_ppm = 755928×4/1 = 3_023_712.
// TI = 4×|4-7| = 12. PI_v = 4×(4+7) = 44.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T11_VEC_A, T11_KEY_A, T11_ID_A);
    add_node(T11_VEC_B, T11_KEY_B, T11_ID_B);
    add_node(T11_VEC_C, T11_KEY_C, T11_ID_C);
    add_node(T11_VEC_D, T11_KEY_D, T11_ID_D);
    add_node(T11_VEC_E, T11_KEY_E, T11_ID_E);
    add_edge(T11_ID_A, T11_ID_B, "t11.e.ab");
    add_edge(T11_ID_A, T11_ID_C, "t11.e.ac");
    add_edge(T11_ID_A, T11_ID_D, "t11.e.ad");
    add_edge(T11_ID_A, T11_ID_E, "t11.e.ae");

    let (j, ti, piv, ec, nc) = gos_runtime::graph_topo_indices11();
    assert_eq!(nc,  5,         "star: node_count=5");
    assert_eq!(ec,  4,         "star: edge_count=4");
    assert_eq!(j,   3_023_712, "star: J_ppm=3_023_712 (floor; exact J=16/\u{221a}28\u{2248}3.0237)");
    assert_eq!(ti,  12,        "star: TI=12 (4 edges × |4-7|=3)");
    assert_eq!(piv, 44,        "star: PI_v=44 (4 edges × (4+7)=11)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// T_A=6; T_B=1+1+2=4; T_C=2+1+1=4; T_D=3+2+1=6. μ=max(1,3-4+2)=1.
// {A,B}: isqrt64(10^12/24)=204124; {B,C}: isqrt64(10^12/16)=250_000; {C,D}: 204124.
// J_ppm = (204124+250000+204124)×3/1 = 658248×3 = 1_974_744.
// TI = |6-4|+|4-4|+|4-6| = 4. PI_v = (6+4)+(4+4)+(4+6) = 28.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T11_VEC_A, T11_KEY_A, T11_ID_A);
    add_node(T11_VEC_B, T11_KEY_B, T11_ID_B);
    add_node(T11_VEC_C, T11_KEY_C, T11_ID_C);
    add_node(T11_VEC_D, T11_KEY_D, T11_ID_D);
    add_edge(T11_ID_A, T11_ID_B, "t11.e.ab");
    add_edge(T11_ID_B, T11_ID_C, "t11.e.bc");
    add_edge(T11_ID_C, T11_ID_D, "t11.e.cd");

    let (j, ti, piv, ec, nc) = gos_runtime::graph_topo_indices11();
    assert_eq!(nc,  4,         "p4: node_count=4");
    assert_eq!(ec,  3,         "p4: edge_count=3");
    assert_eq!(j,   1_974_744, "p4: J_ppm=1_974_744 (floor; J\u{2248}1.9747)");
    assert_eq!(ti,  4,         "p4: TI=4 (|6-4|+0+|4-6|)");
    assert_eq!(piv, 28,        "p4: PI_v=28 (10+8+10)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// T_u=3 ∀u (n-1=3). μ=max(1,6-4+2)=4.
// j_raw = 6×isqrt64(10^12/9) = 6×333_333 = 1_999_998.
// J_ppm = 1_999_998×6/4 = 2_999_997 (exact J=3; 1 ppm floor error from 6 edges).
// TI=0 (vertex-transitive: all T_u=3). PI_v=6×(3+3)=36.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T11_VEC_A, T11_KEY_A, T11_ID_A);
    add_node(T11_VEC_B, T11_KEY_B, T11_ID_B);
    add_node(T11_VEC_C, T11_KEY_C, T11_ID_C);
    add_node(T11_VEC_D, T11_KEY_D, T11_ID_D);
    add_edge(T11_ID_A, T11_ID_B, "t11.e.ab");
    add_edge(T11_ID_B, T11_ID_A, "t11.e.ba");
    add_edge(T11_ID_A, T11_ID_C, "t11.e.ac");
    add_edge(T11_ID_C, T11_ID_A, "t11.e.ca");
    add_edge(T11_ID_A, T11_ID_D, "t11.e.ad");
    add_edge(T11_ID_D, T11_ID_A, "t11.e.da");
    add_edge(T11_ID_B, T11_ID_C, "t11.e.bc");
    add_edge(T11_ID_C, T11_ID_B, "t11.e.cb");
    add_edge(T11_ID_B, T11_ID_D, "t11.e.bd");
    add_edge(T11_ID_D, T11_ID_B, "t11.e.db");
    add_edge(T11_ID_C, T11_ID_D, "t11.e.cd");
    add_edge(T11_ID_D, T11_ID_C, "t11.e.dc");

    let (j, ti, piv, ec, nc) = gos_runtime::graph_topo_indices11();
    assert_eq!(nc,  4,         "k4: node_count=4");
    assert_eq!(ec,  6,         "k4: edge_count=6");
    assert_eq!(j,   2_999_997, "k4: J_ppm=2_999_997 (floor of exact J=3; T_u=3, μ=4)");
    assert_eq!(ti,  0,         "k4: TI=0 (vertex-transitive: all T_u=3)");
    assert_eq!(piv, 36,        "k4: PI_v=36 (6 edges × 6)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// n=2, m=0. No edges → all indices are 0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T11_VEC_A, T11_KEY_A, T11_ID_A);
    add_node(T11_VEC_B, T11_KEY_B, T11_ID_B);

    let (j, ti, piv, ec, nc) = gos_runtime::graph_topo_indices11();
    assert_eq!(nc,  2, "isolated: node_count=2");
    assert_eq!(ec,  0, "isolated: no edges");
    assert_eq!(j,   0, "isolated: J=0");
    assert_eq!(ti,  0, "isolated: TI=0");
    assert_eq!(piv, 0, "isolated: PI_v=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}(deg=3), right={C,D,E}(deg=2). All cross d=1; same-side d=2.
// T(A)=T(B) = 2+1+1+1 = 5 (d to other left=2, d to 3 right=1).
// T(C)=T(D)=T(E) = 1+1+2+2 = 6 (d to 2 left=1, d to 2 other right=2).
// μ=max(1,6-5+2)=3.
// j_raw = 6×isqrt64(10^12/30) = 6×182574 = 1_095_444.
// J_ppm = 1_095_444×6/3 = 2_190_888.
// TI = 6×|5-6| = 6. PI_v = 6×(5+6) = 66.
// INVARIANT: TI>0 confirms K_{2,3} is NOT transmission-regular (T_left≠T_right).

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T11_VEC_A, T11_KEY_A, T11_ID_A);
    add_node(T11_VEC_B, T11_KEY_B, T11_ID_B);
    add_node(T11_VEC_C, T11_KEY_C, T11_ID_C);
    add_node(T11_VEC_D, T11_KEY_D, T11_ID_D);
    add_node(T11_VEC_E, T11_KEY_E, T11_ID_E);
    // Left A connects to all right
    add_edge(T11_ID_A, T11_ID_C, "t11.e.ac");
    add_edge(T11_ID_C, T11_ID_A, "t11.e.ca");
    add_edge(T11_ID_A, T11_ID_D, "t11.e.ad");
    add_edge(T11_ID_D, T11_ID_A, "t11.e.da");
    add_edge(T11_ID_A, T11_ID_E, "t11.e.ae");
    add_edge(T11_ID_E, T11_ID_A, "t11.e.ea");
    // Left B connects to all right
    add_edge(T11_ID_B, T11_ID_C, "t11.e.bc");
    add_edge(T11_ID_C, T11_ID_B, "t11.e.cb");
    add_edge(T11_ID_B, T11_ID_D, "t11.e.bd");
    add_edge(T11_ID_D, T11_ID_B, "t11.e.db");
    add_edge(T11_ID_B, T11_ID_E, "t11.e.be");
    add_edge(T11_ID_E, T11_ID_B, "t11.e.eb");

    let (j, ti, piv, ec, nc) = gos_runtime::graph_topo_indices11();
    assert_eq!(nc,  5,         "k23: node_count=5");
    assert_eq!(ec,  6,         "k23: edge_count=6");
    assert_eq!(j,   2_190_888, "k23: J_ppm=2_190_888 (floor; J=12/\u{221a}30\u{2248}2.1909)");
    assert_eq!(ti,  6,         "k23: TI=6 (6 edges × |5-6|=1; confirms non-transmission-regular)");
    assert_eq!(piv, 66,        "k23: PI_v=66 (6 edges × (5+6)=11)");
}
