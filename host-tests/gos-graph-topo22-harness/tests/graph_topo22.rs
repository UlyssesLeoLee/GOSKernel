// gos-graph-topo22-harness — V3.33 NR + NF + NSC (Neighborhood S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices22()`:
//   Returns (nr_ppm, nf, nsc_ppm, edge_count, node_count)
//   - nr_ppm  = NR(G)  × 10^6 = Σ_{uv∈E} 1/√(S_u·S_v) × 10^6   (floor ppm; S-analogue of Randić R)
//   - nf      = NF(G)          = Σ_v S(v)³                        (exact u64; S-analogue of Forgotten F)
//   - nsc_ppm = NSC(G) × 10^6 = Σ_{uv∈E} 1/√(S_u+S_v) × 10^6   (floor ppm; S-analogue of SC)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NR(G)  = Σ_{uv∈E} (S_u·S_v)^{-1/2}    (S-Randić;        Randić 1975 analogue)
//   NF(G)  = Σ_v S(v)³                      (S-Forgotten;     Furtula & Gutman 2015 analogue)
//   NSC(G) = Σ_{uv∈E} (S_u+S_v)^{-1/2}    (S-Sum-Conn.;     Zhou & Trinajstić 2009 analogue)
//
// IMPLEMENTATION FORMULAS (no float, no_std safe):
//   NR  per edge = isqrt64(10^12 / (S_u · S_v))         [always finite: S≥1 at edge endpoints]
//   NF  per node = S(v)³                                 [exact; S ≤ 127² = 16129; S³ ≤ 4.2×10^12]
//   NSC per edge = isqrt64(10^12 / (S_u + S_v))
//
// KEY INVARIANTS:
//   NR = NSC for S=2 uniform graphs: S_u·S_v = S_u+S_v = 4 when both S=2.
//   For S-uniform (S=c): NR = m·isqrt64(10^12/c²) = m·floor(10^6/c) when c | 10^6.
//   NF=0 iff no edges and all nodes isolated (S=0 for all nodes).
//   NR=NSC=0 iff no edges (edge scan contributes nothing).
//
// S VALUES PER GRAPH (same as topo21/topo18 family):
//   K₂        : S(each)=1               sp=1,  ssum=2
//   P₃=A-B-C  : S(A)=S(B)=S(C)=2       sp=4,  ssum=4  (S-uniform, NR=NSC per edge)
//   K₃        : S(each)=4               sp=16, ssum=8  (S-uniform)
//   K_{1,4}   : S(hub)=4, S(leaf)=4     sp=16, ssum=8  (S-uniform; same as K₃ per edge)
//   P₄=A-B-C-D: S(A)=S(D)=2, S(B)=S(C)=3  (mixed)
//   K₄        : S(each)=9               sp=81, ssum=18 (S-uniform)
//   K_{2,3}   : S(all)=6               sp=36, ssum=12 (S-uniform)
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph        NR(ppm)    NF       NSC(ppm)    edges  nodes
//  Empty              0     0              0       0      0
//  1 node             0     0              0       0      1
//  Edge K₂    1_000_000     2        707_106       1      2
//  Path P₃    1_000_000    24      1_000_000       2      3
//  Triangle K₃  750_000   192      1_060_659       3      3
//  Star K_{1,4} 1_000_000  320     1_414_212       4      5
//  Path P₄    1_149_829    70      1_302_674       3      4
//  Complete K₄  666_666  2916      1_414_212       6      4
//  2 isolated         0     0              0       0      2
//  K_{2,3}      999_996  1080      1_732_050       6      5
//
// Derivations:
//
//   K₂ (S_A=S_B=1, sp=1, ssum=2):
//     NR:  isqrt64(10^12/1) = 10^6 = 1_000_000. ✓
//     NF:  S(A)³+S(B)³ = 1+1 = 2. ✓
//     NSC: isqrt64(10^12/2) = floor(√(5×10^11)) = floor(707_106.78) = 707_106. ✓
//
//   P₃ (S-uniform S=2, 2 edges):
//     NR  per edge: isqrt64(10^12/4) = isqrt64(2.5×10^11) = 500_000. Total: 2×500_000=1_000_000. ✓
//     NF:  3×2³ = 24. ✓
//     NSC per edge: isqrt64(10^12/4) = 500_000 (same as NR; S_u·S_v=S_u+S_v=4). Total: 1_000_000. ✓
//
//   K₃ (S=4, sp=16, ssum=8, 3 edges):
//     NR  per edge: isqrt64(10^12/16) = isqrt64(6.25×10^10) = floor(250_000) = 250_000.
//     NF:  3×4³ = 3×64 = 192. ✓
//     NSC per edge: isqrt64(10^12/8) = isqrt64(1.25×10^11) = floor(353_553.39) = 353_553.
//     Total: (3×250_000, 192, 3×353_553) = (750_000, 192, 1_060_659). ✓
//
//   K_{1,4} (S=4 for all; same sp,ssum as K₃; 4 edges):
//     NR  per edge: 250_000 (identical to K₃). Total: 4×250_000=1_000_000. ✓
//     NF:  (1 hub + 4 leaves) × 4³ = 5×64 = 320. ✓
//     NSC per edge: 353_553. Total: 4×353_553=1_414_212. ✓
//
//   P₄ (S_A=2, S_B=3, S_C=3, S_D=2):
//     Edge A-B (sp=6, ssum=5):
//       NR:  isqrt64(10^12/6)  = isqrt64(166_666_666_666) = floor(408_248.29) = 408_248.
//       NSC: isqrt64(10^12/5)  = isqrt64(200_000_000_000) = floor(447_213.59) = 447_213.
//     Edge B-C (sp=9, ssum=6):
//       NR:  isqrt64(10^12/9)  = isqrt64(111_111_111_111) = floor(333_333.33) = 333_333.
//       NSC: isqrt64(10^12/6)  = isqrt64(166_666_666_666) = 408_248.
//     Edge C-D (sp=6, ssum=5): same as A-B.
//     NF: S_A³+S_B³+S_C³+S_D³ = 8+27+27+8 = 70. ✓
//     Total: (408_248+333_333+408_248, 70, 447_213+408_248+447_213)
//          = (1_149_829, 70, 1_302_674). ✓
//
//   K₄ (S=9, sp=81, ssum=18, 6 edges):
//     NR  per edge: isqrt64(10^12/81) = isqrt64(12_345_679_012) = floor(111_111.11) = 111_111.
//     NF:  4×9³ = 4×729 = 2916. ✓
//     NSC per edge: isqrt64(10^12/18) = isqrt64(55_555_555_555) = floor(235_702.26) = 235_702.
//     Total: (6×111_111, 2916, 6×235_702) = (666_666, 2916, 1_414_212). ✓
//
//   K_{2,3} (S=6 for all, sp=36, ssum=12, 6 edges):
//     NR  per edge: isqrt64(10^12/36) = isqrt64(27_777_777_777) = floor(166_666.67) = 166_666.
//     NF:  5×6³ = 5×216 = 1080. ✓
//     NSC per edge: isqrt64(10^12/12) = isqrt64(83_333_333_333) = floor(288_675.13) = 288_675.
//     Total: (6×166_666, 1080, 6×288_675) = (999_996, 1080, 1_732_050). ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (1_000_000, 2, 707_106, 1, 2)
//  4.  Path P₃ = A-B-C                   → (1_000_000, 24, 1_000_000, 2, 3)
//  5.  Triangle K₃                       → (750_000, 192, 1_060_659, 3, 3)
//  6.  Star K_{1,4}                      → (1_000_000, 320, 1_414_212, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (1_149_829, 70, 1_302_674, 3, 4)
//  8.  Complete K₄                       → (666_666, 2916, 1_414_212, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (999_996, 1080, 1_732_050, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T22_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_22");
const T22_EXEC:   ExecutorId = ExecutorId::from_ascii("t22.exec");

const T22_KEY_A: &str = "t22.alpha";
const T22_KEY_B: &str = "t22.beta";
const T22_KEY_C: &str = "t22.gamma";
const T22_KEY_D: &str = "t22.delta";
const T22_KEY_E: &str = "t22.epsilon";

const T22_ID_A: NodeId = derive_node_id(T22_PLUGIN, T22_KEY_A);
const T22_ID_B: NodeId = derive_node_id(T22_PLUGIN, T22_KEY_B);
const T22_ID_C: NodeId = derive_node_id(T22_PLUGIN, T22_KEY_C);
const T22_ID_D: NodeId = derive_node_id(T22_PLUGIN, T22_KEY_D);
const T22_ID_E: NodeId = derive_node_id(T22_PLUGIN, T22_KEY_E);

// L4=109 namespace for this harness.
const T22_VEC_A: VectorAddress = VectorAddress::new(109, 1, 1, 0);
const T22_VEC_B: VectorAddress = VectorAddress::new(109, 1, 2, 0);
const T22_VEC_C: VectorAddress = VectorAddress::new(109, 1, 3, 0);
const T22_VEC_D: VectorAddress = VectorAddress::new(109, 2, 1, 0);
const T22_VEC_E: VectorAddress = VectorAddress::new(109, 2, 2, 0);

const T22_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T22_PLUGIN,
    name:         "kl-graph-topo22-harness",
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
        executor_id:       T22_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T22_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T22_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nr, nf, nsc, ec, nc) = gos_runtime::graph_topo_indices22();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(nr,  0, "empty: NR=0");
    assert_eq!(nf,  0, "empty: NF=0");
    assert_eq!(nsc, 0, "empty: NSC=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T22_VEC_A, T22_KEY_A, T22_ID_A);

    let (nr, nf, nsc, ec, nc) = gos_runtime::graph_topo_indices22();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(nr,  0, "single: NR=0 (no edges)");
    assert_eq!(nf,  0, "single: NF=0 (S=0 for isolated)");
    assert_eq!(nsc, 0, "single: NSC=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. sp=1, ssum=2.
// NR:  isqrt64(10^12/1) = 10^6 = 1_000_000.
// NF:  S(A)³+S(B)³ = 1+1 = 2.
// NSC: isqrt64(10^12/2) = isqrt64(5×10^11) = floor(707_106.78) = 707_106.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T22_VEC_A, T22_KEY_A, T22_ID_A);
    add_node(T22_VEC_B, T22_KEY_B, T22_ID_B);
    add_edge(T22_ID_A, T22_ID_B, "t22.e.ab");

    let (nr, nf, nsc, ec, nc) = gos_runtime::graph_topo_indices22();
    assert_eq!(nc,  2,         "k2: node_count=2");
    assert_eq!(ec,  1,         "k2: edge_count=1");
    assert_eq!(nr,  1_000_000, "k2: NR=1_000_000 (isqrt64(10^12/1)=10^6)");
    assert_eq!(nf,  2,         "k2: NF=2 (S(A)\u{00b3}+S(B)\u{00b3}=1+1)");
    assert_eq!(nsc, 707_106,   "k2: NSC=707_106 (isqrt64(5\u{00d7}10^11)=\u{221a}2\u{00d7}10^6 floor)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S(A)=S(B)=S(C)=2 (S-uniform S=2).
// sp=4, ssum=4 for both edges → NR=NSC per edge (S_u·S_v=S_u+S_v=4).
// NR  per edge: isqrt64(10^12/4) = 500_000. Total: 2×500_000=1_000_000.
// NF:  3×2³ = 24.
// NSC per edge: isqrt64(10^12/4) = 500_000. Total: 1_000_000.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T22_VEC_A, T22_KEY_A, T22_ID_A);
    add_node(T22_VEC_B, T22_KEY_B, T22_ID_B);
    add_node(T22_VEC_C, T22_KEY_C, T22_ID_C);
    add_edge(T22_ID_A, T22_ID_B, "t22.e.ab");
    add_edge(T22_ID_B, T22_ID_C, "t22.e.bc");

    let (nr, nf, nsc, ec, nc) = gos_runtime::graph_topo_indices22();
    assert_eq!(nc,  3,         "p3: node_count=3");
    assert_eq!(ec,  2,         "p3: edge_count=2");
    assert_eq!(nr,  1_000_000, "p3: NR=1_000_000 (2\u{00d7}500_000; S-uniform S=2, NR=NSC)");
    assert_eq!(nf,  24,        "p3: NF=24 (3\u{00d7}2\u{00b3}; S-uniform S=2)");
    assert_eq!(nsc, 1_000_000, "p3: NSC=1_000_000 (2\u{00d7}500_000; NR=NSC for S=2 uniform)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. sp=16, ssum=8.
// NR  per edge: isqrt64(10^12/16) = floor(250_000) = 250_000. Total: 3×250_000=750_000.
// NF:  3×4³ = 3×64 = 192.
// NSC per edge: isqrt64(10^12/8) = isqrt64(1.25×10^11) = floor(353_553.39) = 353_553.
// Total: 3×353_553=1_060_659.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T22_VEC_A, T22_KEY_A, T22_ID_A);
    add_node(T22_VEC_B, T22_KEY_B, T22_ID_B);
    add_node(T22_VEC_C, T22_KEY_C, T22_ID_C);
    add_edge(T22_ID_A, T22_ID_B, "t22.e.ab");
    add_edge(T22_ID_B, T22_ID_A, "t22.e.ba");
    add_edge(T22_ID_B, T22_ID_C, "t22.e.bc");
    add_edge(T22_ID_C, T22_ID_B, "t22.e.cb");
    add_edge(T22_ID_A, T22_ID_C, "t22.e.ac");
    add_edge(T22_ID_C, T22_ID_A, "t22.e.ca");

    let (nr, nf, nsc, ec, nc) = gos_runtime::graph_topo_indices22();
    assert_eq!(nc,  3,         "k3: node_count=3");
    assert_eq!(ec,  3,         "k3: edge_count=3");
    assert_eq!(nr,  750_000,   "k3: NR=750_000 (3\u{00d7}250_000; S-uniform S=4)");
    assert_eq!(nf,  192,       "k3: NF=192 (3\u{00d7}4\u{00b3}=3\u{00d7}64)");
    assert_eq!(nsc, 1_060_659, "k3: NSC=1_060_659 (3\u{00d7}353_553; isqrt64(10^12/8))");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4×1=4, S(leaf)=deg(hub)=4. S-uniform S=4.
// Same per-edge sp=16, ssum=8 as K₃.
// NR per edge: 250_000. Total: 4×250_000=1_000_000.
// NF: (1 hub + 4 leaves) × 4³ = 5×64 = 320.
// NSC per edge: 353_553. Total: 4×353_553=1_414_212.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T22_VEC_A, T22_KEY_A, T22_ID_A);
    add_node(T22_VEC_B, T22_KEY_B, T22_ID_B);
    add_node(T22_VEC_C, T22_KEY_C, T22_ID_C);
    add_node(T22_VEC_D, T22_KEY_D, T22_ID_D);
    add_node(T22_VEC_E, T22_KEY_E, T22_ID_E);
    add_edge(T22_ID_A, T22_ID_B, "t22.e.ab");
    add_edge(T22_ID_A, T22_ID_C, "t22.e.ac");
    add_edge(T22_ID_A, T22_ID_D, "t22.e.ad");
    add_edge(T22_ID_A, T22_ID_E, "t22.e.ae");

    let (nr, nf, nsc, ec, nc) = gos_runtime::graph_topo_indices22();
    assert_eq!(nc,  5,         "star: node_count=5");
    assert_eq!(ec,  4,         "star: edge_count=4");
    assert_eq!(nr,  1_000_000, "star: NR=1_000_000 (4\u{00d7}250_000; same S=4 as K\u{2083})");
    assert_eq!(nf,  320,       "star: NF=320 (5\u{00d7}4\u{00b3}=5\u{00d7}64)");
    assert_eq!(nsc, 1_414_212, "star: NSC=1_414_212 (4\u{00d7}353_553; same S=4 as K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2.
// S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=3, S(C)=deg(B)+deg(D)=3, S(D)=deg(C)=2.
// Edge A-B (sp=6, ssum=5): NR=isqrt64(10^12/6)=408_248; NSC=isqrt64(10^12/5)=447_213.
// Edge B-C (sp=9, ssum=6): NR=isqrt64(10^12/9)=333_333; NSC=isqrt64(10^12/6)=408_248.
// Edge C-D (sp=6, ssum=5): same as A-B by symmetry.
// NF: 2³+3³+3³+2³ = 8+27+27+8 = 70.
// Totals: (408_248+333_333+408_248, 70, 447_213+408_248+447_213)
//       = (1_149_829, 70, 1_302_674).

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T22_VEC_A, T22_KEY_A, T22_ID_A);
    add_node(T22_VEC_B, T22_KEY_B, T22_ID_B);
    add_node(T22_VEC_C, T22_KEY_C, T22_ID_C);
    add_node(T22_VEC_D, T22_KEY_D, T22_ID_D);
    add_edge(T22_ID_A, T22_ID_B, "t22.e.ab");
    add_edge(T22_ID_B, T22_ID_C, "t22.e.bc");
    add_edge(T22_ID_C, T22_ID_D, "t22.e.cd");

    let (nr, nf, nsc, ec, nc) = gos_runtime::graph_topo_indices22();
    assert_eq!(nc,  4,         "p4: node_count=4");
    assert_eq!(ec,  3,         "p4: edge_count=3");
    assert_eq!(nr,  1_149_829, "p4: NR=1_149_829 (408_248+333_333+408_248)");
    assert_eq!(nf,  70,        "p4: NF=70 (8+27+27+8)");
    assert_eq!(nsc, 1_302_674, "p4: NSC=1_302_674 (447_213+408_248+447_213)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=3×3=9. S-uniform S=9. sp=81, ssum=18.
// NR  per edge: isqrt64(10^12/81) = floor(111_111.11) = 111_111. Total: 6×111_111=666_666.
// NF:  4×9³ = 4×729 = 2916.
// NSC per edge: isqrt64(10^12/18) = floor(235_702.26) = 235_702. Total: 6×235_702=1_414_212.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T22_VEC_A, T22_KEY_A, T22_ID_A);
    add_node(T22_VEC_B, T22_KEY_B, T22_ID_B);
    add_node(T22_VEC_C, T22_KEY_C, T22_ID_C);
    add_node(T22_VEC_D, T22_KEY_D, T22_ID_D);
    add_edge(T22_ID_A, T22_ID_B, "t22.e.ab");
    add_edge(T22_ID_B, T22_ID_A, "t22.e.ba");
    add_edge(T22_ID_A, T22_ID_C, "t22.e.ac");
    add_edge(T22_ID_C, T22_ID_A, "t22.e.ca");
    add_edge(T22_ID_A, T22_ID_D, "t22.e.ad");
    add_edge(T22_ID_D, T22_ID_A, "t22.e.da");
    add_edge(T22_ID_B, T22_ID_C, "t22.e.bc");
    add_edge(T22_ID_C, T22_ID_B, "t22.e.cb");
    add_edge(T22_ID_B, T22_ID_D, "t22.e.bd");
    add_edge(T22_ID_D, T22_ID_B, "t22.e.db");
    add_edge(T22_ID_C, T22_ID_D, "t22.e.cd");
    add_edge(T22_ID_D, T22_ID_C, "t22.e.dc");

    let (nr, nf, nsc, ec, nc) = gos_runtime::graph_topo_indices22();
    assert_eq!(nc,  4,         "k4: node_count=4");
    assert_eq!(ec,  6,         "k4: edge_count=6");
    assert_eq!(nr,  666_666,   "k4: NR=666_666 (6\u{00d7}111_111; S-uniform S=9)");
    assert_eq!(nf,  2916,      "k4: NF=2916 (4\u{00d7}9\u{00b3}=4\u{00d7}729)");
    assert_eq!(nsc, 1_414_212, "k4: NSC=1_414_212 (6\u{00d7}235_702; isqrt64(10^12/18))");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T22_VEC_A, T22_KEY_A, T22_ID_A);
    add_node(T22_VEC_B, T22_KEY_B, T22_ID_B);

    let (nr, nf, nsc, ec, nc) = gos_runtime::graph_topo_indices22();
    assert_eq!(nc,  2, "isolated: node_count=2");
    assert_eq!(ec,  0, "isolated: no edges");
    assert_eq!(nr,  0, "isolated: NR=0 (no edges)");
    assert_eq!(nf,  0, "isolated: NF=0 (S=0 for all isolated nodes)");
    assert_eq!(nsc, 0, "isolated: NSC=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}:d=3. Right={C,D,E}:d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. sp=36, ssum=12.
// NR  per edge: isqrt64(10^12/36) = floor(166_666.67) = 166_666. Total: 6×166_666=999_996.
// NF:  5×6³ = 5×216 = 1080.
// NSC per edge: isqrt64(10^12/12) = floor(288_675.13) = 288_675. Total: 6×288_675=1_732_050.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T22_VEC_A, T22_KEY_A, T22_ID_A);
    add_node(T22_VEC_B, T22_KEY_B, T22_ID_B);
    add_node(T22_VEC_C, T22_KEY_C, T22_ID_C);
    add_node(T22_VEC_D, T22_KEY_D, T22_ID_D);
    add_node(T22_VEC_E, T22_KEY_E, T22_ID_E);
    add_edge(T22_ID_A, T22_ID_C, "t22.e.ac");
    add_edge(T22_ID_C, T22_ID_A, "t22.e.ca");
    add_edge(T22_ID_A, T22_ID_D, "t22.e.ad");
    add_edge(T22_ID_D, T22_ID_A, "t22.e.da");
    add_edge(T22_ID_A, T22_ID_E, "t22.e.ae");
    add_edge(T22_ID_E, T22_ID_A, "t22.e.ea");
    add_edge(T22_ID_B, T22_ID_C, "t22.e.bc");
    add_edge(T22_ID_C, T22_ID_B, "t22.e.cb");
    add_edge(T22_ID_B, T22_ID_D, "t22.e.bd");
    add_edge(T22_ID_D, T22_ID_B, "t22.e.db");
    add_edge(T22_ID_B, T22_ID_E, "t22.e.be");
    add_edge(T22_ID_E, T22_ID_B, "t22.e.eb");

    let (nr, nf, nsc, ec, nc) = gos_runtime::graph_topo_indices22();
    assert_eq!(nc,  5,         "k23: node_count=5");
    assert_eq!(ec,  6,         "k23: edge_count=6");
    assert_eq!(nr,  999_996,   "k23: NR=999_996 (6\u{00d7}166_666; floor loss vs 6\u{00d7}166_666.\u{0305})");
    assert_eq!(nf,  1080,      "k23: NF=1080 (5\u{00d7}6\u{00b3}=5\u{00d7}216; S-uniform S=6)");
    assert_eq!(nsc, 1_732_050, "k23: NSC=1_732_050 (6\u{00d7}288_675; isqrt64(10^12/12))");
}
