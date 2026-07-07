// gos-graph-topo10-harness — V3.21 edge-partition distance topological indices
//
// Verifies `gos_runtime::graph_topo_indices10()`:
//   Returns (sz, rsz_ppm, mo, edge_count, node_count)
//   - sz      = Sz(G)  = Σ_{uv∈E} n_u(uv)·n_v(uv)                    (exact u64; Gutman & Klavžar 1995)
//   - rsz_ppm = rSz(G)×10^6 = Σ_{uv∈E} (n_u+n₀/2)·(n_v+n₀/2)×10^6  (floor ppm; Pisanski & Randić 2010)
//   - mo      = Mo(G)  = Σ_{uv∈E} |n_u − n_v|                         (exact u64; Doslić et al. 2018)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// For each undirected edge {a,b}:
//   n_u = #{w : d(w,a) < d(w,b)},  n_v = #{w : d(w,a) > d(w,b)},
//   n_0 = #{w : d(w,a) = d(w,b)}  (equidistant; arises on cycles only in connected graphs)
// Vertices in disconnected components (both d=∞) excluded.
//
// KEY INVARIANTS:
//   Tree invariant: n_0 = 0 for every tree edge → Sz = rSz = Wiener index.
//   Vertex-transitive invariant (K_n, K_{r,s} when r=s): Mo = 0 (n_u = n_v for all edges).
//   rSz stored as (4·rSz_int)×250_000 to avoid quarter-integer fractions.
//   rSz ≥ Sz always (AM-GM on (n_u+n_0/2) vs n_u, etc.); rSz = Sz iff n_0 = 0 for all edges.
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph         Sz  rSz_ppm         Mo  edges  nodes
//  Empty          0        0          0      0      0
//  1 node         0        0          0      0      1
//  Edge A-B       1  1_000_000        0      1      2
//  Path P₃        4  4_000_000        2      2      3
//  Triangle K₃    3  6_750_000        0      3      3
//  Star K_{1,4}  16 16_000_000       12      4      5
//  Path P₄       10 10_000_000        4      3      4
//  Complete K₄    6 24_000_000        0      6      4
//  2 isolated     0        0          0      0      2
//  K_{2,3}       36 36_000_000        6      6      5
//
// Derivations:
//   Edge A-B: only edge {A,B}; w=A→n_u, w=B→n_v; n_u=1,n_v=1,n_0=0
//     Sz=1; rsz_4=(2)(2)=4; rsz_ppm=1_000_000; mo=0
//
//   P₃ (A-B-C):
//     {A,B}: d_A=[0,1,2] d_B=[1,0,1]; A→n_u, B,C→n_v; n_u=1,n_v=2,n_0=0
//       Sz+=2; rsz_4+=(2)(4)=8; mo+=1
//     {B,C}: d_B=[1,0,1] d_C=[2,1,0]; A,B→n_u, C→n_v; n_u=2,n_v=1,n_0=0
//       Sz+=2; rsz_4+=(4)(2)=8; mo+=1
//     Sz=4; rsz_4=16; rsz_ppm=4_000_000; mo=2
//     Note: P₃ is a tree → Sz=rSz/10^6=Wiener(P₃)=4 ✓
//
//   K₃ (all d=1):
//     {A,B}: w=A→n_u, w=B→n_v, w=C→n_0 (d_A=d_B=1); n_u=1,n_v=1,n_0=1
//       Sz+=1; rsz_4+=(2+1)(2+1)=9; mo+=0
//     All 3 edges same: Sz=3; rsz_4=27; rsz_ppm=27×250_000=6_750_000; mo=0
//     rSz=27/4=6.75 (quarter-integer, stored as ppm)
//
//   K_{1,4} (center A, leaves B,C,D,E):
//     {A,B}: d_A=[0,1,1,1,1] d_B=[1,0,2,2,2]; A,C,D,E→n_u, B→n_v; n_u=4,n_v=1,n_0=0
//       Sz+=4; rsz_4+=(8)(2)=16; mo+=3
//     All 4 edges same: Sz=16; rsz_4=64; rsz_ppm=16_000_000; mo=12
//     Tree → Sz=rSz/10^6=Wiener(K_{1,4})=4×1+6×2=16 ✓
//
//   P₄ (A-B-C-D):
//     {A,B}: n_u=1(A), n_v=3(B,C,D), n_0=0 → Sz+=3; rsz_4+=(2)(6)=12; mo+=2
//     {B,C}: n_u=2(A,B), n_v=2(C,D), n_0=0 → Sz+=4; rsz_4+=(4)(4)=16; mo+=0
//     {C,D}: n_u=3(A,B,C), n_v=1(D), n_0=0 → Sz+=3; rsz_4+=(6)(2)=12; mo+=2
//     Sz=10; rsz_4=40; rsz_ppm=10_000_000; mo=4
//     Tree → Sz=Wiener(P₄)=1+2+3+1+2+1=10 ✓
//
//   K₄ (all d=1, 4 nodes):
//     {A,B}: w=A→n_u, w=B→n_v, w=C,D→n_0; n_u=1,n_v=1,n_0=2
//       Sz+=1; rsz_4+=(2+2)(2+2)=16; mo+=0
//     All 6 edges same: Sz=6; rsz_4=96; rsz_ppm=24_000_000; mo=0
//
//   K_{2,3} (left={A,B}, right={C,D,E}; all d(left,right)=1, d(left,left)=d(right,right)=2):
//     {A,C}: d_A=[0,2,1,1,1] d_C=[1,1,0,2,2]; A,D,E→n_u, B,C→n_v; n_u=3,n_v=2,n_0=0
//       Sz+=6; rsz_4+=(6)(4)=24; mo+=1
//     All 6 cross-edges same: Sz=36; rsz_4=144; rsz_ppm=36_000_000; mo=6
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (1, 1_000_000, 0, 1, 2)
//  4.  Path P₃ = A-B-C                    → (4, 4_000_000, 2, 2, 3)
//  5.  Triangle K₃                        → (3, 6_750_000, 0, 3, 3)
//  6.  Star K_{1,4}                       → (16, 16_000_000, 12, 4, 5)
//  7.  Path P₄ = A-B-C-D                  → (10, 10_000_000, 4, 3, 4)
//  8.  Complete K₄                        → (6, 24_000_000, 0, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check      → (36, 36_000_000, 6, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T10_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_10");
const T10_EXEC:   ExecutorId = ExecutorId::from_ascii("t10.exec");

const T10_KEY_A: &str = "t10.alpha";
const T10_KEY_B: &str = "t10.beta";
const T10_KEY_C: &str = "t10.gamma";
const T10_KEY_D: &str = "t10.delta";
const T10_KEY_E: &str = "t10.epsilon";

const T10_ID_A: NodeId = derive_node_id(T10_PLUGIN, T10_KEY_A);
const T10_ID_B: NodeId = derive_node_id(T10_PLUGIN, T10_KEY_B);
const T10_ID_C: NodeId = derive_node_id(T10_PLUGIN, T10_KEY_C);
const T10_ID_D: NodeId = derive_node_id(T10_PLUGIN, T10_KEY_D);
const T10_ID_E: NodeId = derive_node_id(T10_PLUGIN, T10_KEY_E);

// L4=97 namespace for this harness.
const T10_VEC_A: VectorAddress = VectorAddress::new(97, 1, 1, 0);
const T10_VEC_B: VectorAddress = VectorAddress::new(97, 1, 2, 0);
const T10_VEC_C: VectorAddress = VectorAddress::new(97, 1, 3, 0);
const T10_VEC_D: VectorAddress = VectorAddress::new(97, 2, 1, 0);
const T10_VEC_E: VectorAddress = VectorAddress::new(97, 2, 2, 0);

const T10_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T10_PLUGIN,
    name:         "kl-graph-topo10-harness",
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
        executor_id:       T10_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T10_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T10_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
// No nodes. All values are 0.

#[test]
fn test_01_empty() {
    let _g = setup();

    let (sz, rsz, mo, ec, nc) = gos_runtime::graph_topo_indices10();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(sz,  0, "empty: Sz=0");
    assert_eq!(rsz, 0, "empty: rSz=0");
    assert_eq!(mo,  0, "empty: Mo=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// No edges. All indices are 0.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T10_VEC_A, T10_KEY_A, T10_ID_A);

    let (sz, rsz, mo, ec, nc) = gos_runtime::graph_topo_indices10();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(sz,  0, "single: Sz=0");
    assert_eq!(rsz, 0, "single: rSz=0");
    assert_eq!(mo,  0, "single: Mo=0");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected A-B: n=2.
// Edge {A,B}: w=A→d_A=0<d_B=1→n_u; w=B→d_A=1>d_B=0→n_v; n_u=1,n_v=1,n_0=0
// Sz=1; rsz_4=(2)(2)=4; rsz_ppm=1_000_000; Mo=0.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T10_VEC_A, T10_KEY_A, T10_ID_A);
    add_node(T10_VEC_B, T10_KEY_B, T10_ID_B);
    add_edge(T10_ID_A, T10_ID_B, "t10.e.ab");

    let (sz, rsz, mo, ec, nc) = gos_runtime::graph_topo_indices10();
    assert_eq!(nc,  2, "edge: node_count=2");
    assert_eq!(ec,  1, "edge: edge_count=1");
    assert_eq!(sz,  1, "edge: Sz=1");
    assert_eq!(rsz, 1_000_000, "edge: rSz_ppm=1_000_000 (=Sz, tree)");
    assert_eq!(mo,  0, "edge: Mo=0 (symmetric)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// n=3, m=2. Tree so Sz=rSz=Wiener=4.
// Edge {A,B}: n_u=1(A), n_v=2(B,C), n_0=0 → Sz+=2; rsz_4+=8; mo+=1
// Edge {B,C}: n_u=2(A,B), n_v=1(C), n_0=0 → Sz+=2; rsz_4+=8; mo+=1
// Sz=4; rsz_ppm=16×250_000=4_000_000; mo=2.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T10_VEC_A, T10_KEY_A, T10_ID_A);
    add_node(T10_VEC_B, T10_KEY_B, T10_ID_B);
    add_node(T10_VEC_C, T10_KEY_C, T10_ID_C);
    add_edge(T10_ID_A, T10_ID_B, "t10.e.ab");
    add_edge(T10_ID_B, T10_ID_C, "t10.e.bc");

    let (sz, rsz, mo, ec, nc) = gos_runtime::graph_topo_indices10();
    assert_eq!(nc,  3, "p3: node_count=3");
    assert_eq!(ec,  2, "p3: edge_count=2");
    assert_eq!(sz,  4, "p3: Sz=4");
    assert_eq!(rsz, 4_000_000, "p3: rSz_ppm=4_000_000 (=Sz, tree)");
    assert_eq!(mo,  2, "p3: Mo=2");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// n=3, m=3. All d=1. Each edge has n_0=1 (opposite vertex equidistant).
// Edge {A,B}: n_u=1(A), n_v=1(B), n_0=1(C) → Sz+=1; rsz_4+=(2+1)(2+1)=9; mo+=0
// All 3 edges: Sz=3; rsz_4=27; rsz_ppm=27×250_000=6_750_000; Mo=0.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T10_VEC_A, T10_KEY_A, T10_ID_A);
    add_node(T10_VEC_B, T10_KEY_B, T10_ID_B);
    add_node(T10_VEC_C, T10_KEY_C, T10_ID_C);
    add_edge(T10_ID_A, T10_ID_B, "t10.e.ab");
    add_edge(T10_ID_B, T10_ID_A, "t10.e.ba");
    add_edge(T10_ID_B, T10_ID_C, "t10.e.bc");
    add_edge(T10_ID_C, T10_ID_B, "t10.e.cb");
    add_edge(T10_ID_A, T10_ID_C, "t10.e.ac");
    add_edge(T10_ID_C, T10_ID_A, "t10.e.ca");

    let (sz, rsz, mo, ec, nc) = gos_runtime::graph_topo_indices10();
    assert_eq!(nc,  3, "k3: node_count=3");
    assert_eq!(ec,  3, "k3: edge_count=3");
    assert_eq!(sz,  3, "k3: Sz=3");
    assert_eq!(rsz, 6_750_000, "k3: rSz_ppm=6_750_000 (rSz=27/4; n_0=1 per edge)");
    assert_eq!(mo,  0, "k3: Mo=0 (vertex-transitive)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// n=5, m=4. Center=A(deg=4), leaves=B,C,D,E(deg=1). Tree so Sz=rSz=Wiener=16.
// Edge {A,B}: d_A=[0,1,1,1,1] d_B=[1,0,2,2,2]
//   w=A,C,D,E: d_A<d_B → n_u; w=B: d_B=0<d_A=1 → n_v; n_0=0
//   n_u=4, n_v=1 → Sz+=4; rsz_4+=(8)(2)=16; mo+=3
// All 4 edges same: Sz=16; rsz_ppm=64×250_000=16_000_000; Mo=12.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T10_VEC_A, T10_KEY_A, T10_ID_A);
    add_node(T10_VEC_B, T10_KEY_B, T10_ID_B);
    add_node(T10_VEC_C, T10_KEY_C, T10_ID_C);
    add_node(T10_VEC_D, T10_KEY_D, T10_ID_D);
    add_node(T10_VEC_E, T10_KEY_E, T10_ID_E);
    add_edge(T10_ID_A, T10_ID_B, "t10.e.ab");
    add_edge(T10_ID_A, T10_ID_C, "t10.e.ac");
    add_edge(T10_ID_A, T10_ID_D, "t10.e.ad");
    add_edge(T10_ID_A, T10_ID_E, "t10.e.ae");

    let (sz, rsz, mo, ec, nc) = gos_runtime::graph_topo_indices10();
    assert_eq!(nc,  5, "star: node_count=5");
    assert_eq!(ec,  4, "star: edge_count=4");
    assert_eq!(sz,  16, "star: Sz=16");
    assert_eq!(rsz, 16_000_000, "star: rSz_ppm=16_000_000 (=Sz, tree)");
    assert_eq!(mo,  12, "star: Mo=12 (4 edges × |4-1|=3)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// n=4, m=3. Tree so Sz=rSz=Wiener=10.
// {A,B}: n_u=1(A), n_v=3(B,C,D), n_0=0 → Sz+=3; rsz_4+=(2)(6)=12; mo+=2
// {B,C}: n_u=2(A,B), n_v=2(C,D), n_0=0 → Sz+=4; rsz_4+=(4)(4)=16; mo+=0
// {C,D}: n_u=3(A,B,C), n_v=1(D), n_0=0 → Sz+=3; rsz_4+=(6)(2)=12; mo+=2
// Sz=10; rsz_ppm=40×250_000=10_000_000; Mo=4.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T10_VEC_A, T10_KEY_A, T10_ID_A);
    add_node(T10_VEC_B, T10_KEY_B, T10_ID_B);
    add_node(T10_VEC_C, T10_KEY_C, T10_ID_C);
    add_node(T10_VEC_D, T10_KEY_D, T10_ID_D);
    add_edge(T10_ID_A, T10_ID_B, "t10.e.ab");
    add_edge(T10_ID_B, T10_ID_C, "t10.e.bc");
    add_edge(T10_ID_C, T10_ID_D, "t10.e.cd");

    let (sz, rsz, mo, ec, nc) = gos_runtime::graph_topo_indices10();
    assert_eq!(nc,  4, "p4: node_count=4");
    assert_eq!(ec,  3, "p4: edge_count=3");
    assert_eq!(sz,  10, "p4: Sz=10");
    assert_eq!(rsz, 10_000_000, "p4: rSz_ppm=10_000_000 (=Sz, tree)");
    assert_eq!(mo,  4, "p4: Mo=4 (|n_u-n_v|=2+0+2=4)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// n=4, m=6. All d=1. Each edge: n_u=1, n_v=1, n_0=2 (other 2 vertices equidistant).
// Sz per edge = 1×1 = 1; rsz_4 per edge = (2+2)(2+2) = 16; Mo per edge = 0.
// Sz=6; rsz_ppm=96×250_000=24_000_000; Mo=0.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T10_VEC_A, T10_KEY_A, T10_ID_A);
    add_node(T10_VEC_B, T10_KEY_B, T10_ID_B);
    add_node(T10_VEC_C, T10_KEY_C, T10_ID_C);
    add_node(T10_VEC_D, T10_KEY_D, T10_ID_D);
    add_edge(T10_ID_A, T10_ID_B, "t10.e.ab");
    add_edge(T10_ID_B, T10_ID_A, "t10.e.ba");
    add_edge(T10_ID_A, T10_ID_C, "t10.e.ac");
    add_edge(T10_ID_C, T10_ID_A, "t10.e.ca");
    add_edge(T10_ID_A, T10_ID_D, "t10.e.ad");
    add_edge(T10_ID_D, T10_ID_A, "t10.e.da");
    add_edge(T10_ID_B, T10_ID_C, "t10.e.bc");
    add_edge(T10_ID_C, T10_ID_B, "t10.e.cb");
    add_edge(T10_ID_B, T10_ID_D, "t10.e.bd");
    add_edge(T10_ID_D, T10_ID_B, "t10.e.db");
    add_edge(T10_ID_C, T10_ID_D, "t10.e.cd");
    add_edge(T10_ID_D, T10_ID_C, "t10.e.dc");

    let (sz, rsz, mo, ec, nc) = gos_runtime::graph_topo_indices10();
    assert_eq!(nc,  4,  "k4: node_count=4");
    assert_eq!(ec,  6,  "k4: edge_count=6");
    assert_eq!(sz,  6,  "k4: Sz=6");
    assert_eq!(rsz, 24_000_000, "k4: rSz_ppm=24_000_000 (n_0=2 per edge: rSz=24)");
    assert_eq!(mo,  0,  "k4: Mo=0 (vertex-transitive: n_u=n_v=1 for all edges)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// n=2, m=0. No edges → all indices are 0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T10_VEC_A, T10_KEY_A, T10_ID_A);
    add_node(T10_VEC_B, T10_KEY_B, T10_ID_B);

    let (sz, rsz, mo, ec, nc) = gos_runtime::graph_topo_indices10();
    assert_eq!(nc,  2, "isolated: node_count=2");
    assert_eq!(ec,  0, "isolated: no edges");
    assert_eq!(sz,  0, "isolated: Sz=0");
    assert_eq!(rsz, 0, "isolated: rSz=0");
    assert_eq!(mo,  0, "isolated: Mo=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// n=5, m=6. Left={A,B}(deg=3), right={C,D,E}(deg=2).
// All cross d=1; left-left d=2; right-right d=2.
// Edge {A,C}: d_A=[0,2,1,1,1] d_C=[1,1,0,2,2]
//   A(0<1)→n_u, B(2>1)→n_v, C(1>0)→n_v, D(1<2)→n_u, E(1<2)→n_u
//   n_u=3(A,D,E), n_v=2(B,C), n_0=0 → Sz+=6; rsz_4+=(6)(4)=24; mo+=1
// All 6 cross-edges same: Sz=36; rsz_ppm=144×250_000=36_000_000; Mo=6.
// INVARIANT: Mo=6 > 0 confirms K_{2,3} is NOT vertex-transitive (r≠s).

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T10_VEC_A, T10_KEY_A, T10_ID_A);
    add_node(T10_VEC_B, T10_KEY_B, T10_ID_B);
    add_node(T10_VEC_C, T10_KEY_C, T10_ID_C);
    add_node(T10_VEC_D, T10_KEY_D, T10_ID_D);
    add_node(T10_VEC_E, T10_KEY_E, T10_ID_E);
    // Left A connects to all right
    add_edge(T10_ID_A, T10_ID_C, "t10.e.ac");
    add_edge(T10_ID_C, T10_ID_A, "t10.e.ca");
    add_edge(T10_ID_A, T10_ID_D, "t10.e.ad");
    add_edge(T10_ID_D, T10_ID_A, "t10.e.da");
    add_edge(T10_ID_A, T10_ID_E, "t10.e.ae");
    add_edge(T10_ID_E, T10_ID_A, "t10.e.ea");
    // Left B connects to all right
    add_edge(T10_ID_B, T10_ID_C, "t10.e.bc");
    add_edge(T10_ID_C, T10_ID_B, "t10.e.cb");
    add_edge(T10_ID_B, T10_ID_D, "t10.e.bd");
    add_edge(T10_ID_D, T10_ID_B, "t10.e.db");
    add_edge(T10_ID_B, T10_ID_E, "t10.e.be");
    add_edge(T10_ID_E, T10_ID_B, "t10.e.eb");

    let (sz, rsz, mo, ec, nc) = gos_runtime::graph_topo_indices10();
    assert_eq!(nc,  5,  "k23: node_count=5");
    assert_eq!(ec,  6,  "k23: edge_count=6");
    assert_eq!(sz,  36, "k23: Sz=36");
    assert_eq!(rsz, 36_000_000, "k23: rSz_ppm=36_000_000 (=Sz, n_0=0 for all edges)");
    assert_eq!(mo,  6,  "k23: Mo=6 (6 edges × |3-2|=1; confirms non-vertex-transitive)");
}
