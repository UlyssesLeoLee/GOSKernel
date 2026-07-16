// gos-graph-topo34-harness — V3.45 NOC + NHHS + NFSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices34()`:
//   Returns (noc, nhhs, nfso, edge_count, node_count)
//   - noc  = NOC(G)  = Σ_v S(v)^8                  (exact u64; S-octic vertex sum)
//   - nhhs = NHHS(G) = Σ_{uv∈E} (S_u+S_v)^7        (exact u64; S-septic/hepta edge-sum)
//   - nfso = NFSO(G) = Σ_{uv∈E} (S_u²+S_v²)²       (exact u64; S-Fourth Sombor, α=4)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOC(G) = Σ_v S(v)^8
//     S-octic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34).
//     NOC = n·S^8 for S-regular.
//     Overflow: S^8 ≤ 16129^8 ≈ 5.6×10^32 > u64::MAX → u128 accumulator, clamp to u64::MAX.
//
//   NHHS(G) = Σ_{uv∈E} (S_u+S_v)^7
//     S-septic (hepta) edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34).
//     NHHS = |E|·(2S)^7 = 128|E|S^7 for S-regular.
//     Overflow per edge: (2×16129)^7 ≈ 1.51×10^30 > u64::MAX → u128 accumulator.
//
//   NFSO(G) = Σ_{uv∈E} (S_u²+S_v²)²
//     S-Fourth Sombor: generalised Sombor SO^α with α=4 on S-variant.
//     NSO(topo21)=Σ(S²+S²)^{1/2} (α=1), NCSO(topo33)=Σ(S²+S²)^{3/2} (α=3),
//     NFSO(topo34)=Σ(S²+S²)^2 (α=4) — exact integer, no isqrt needed.
//     NFSO = |E|·(2S²)² = 4|E|S⁴ for S-regular.
//     Overflow per edge: (2×16129²)^2 ≈ 2.7×10^17 > u64::MAX → u128 accumulator.
//
// S VALUES PER GRAPH:
//   K₂        : S(A)=S(B)=1
//   P₃=A-B-C  : S(A)=S(B)=S(C)=2    → S-uniform S=2
//   K₃        : S(each)=4            → S-uniform S=4
//   K_{1,4}   : S(hub)=4, S(leaf)=4  → S-uniform S=4
//   P₄=A-B-C-D: S(A)=S(D)=2, S(B)=S(C)=3 → mixed S
//   K₄        : S(each)=9            → S-uniform S=9
//   K_{2,3}   : S(all)=6             → S-uniform S=6
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph       NOC(exact)     NHHS(exact)     NFSO(exact)    edges  nodes
//  Empty                0               0               0       0      0
//  1 node               0               0               0       0      1
//  K₂                   2             128               4       1      2
//  P₃                 768          32_768             128       2      3
//  K₃             196_608       6_291_456           3_072       3      3
//  K_{1,4}        327_680       8_388_608           4_096       4      5
//  P₄              13_634         436_186             662       3      4
//  K₄         172_186_884   3_673_320_192         157_464       6      4
//  2 isolated           0               0               0       0      2
//  K_{2,3}      8_398_080     214_990_848          31_104       6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NOC: 1^8 + 1^8 = 2. ✓
//     NHHS: (1+1)^7 = 2^7 = 128. ✓
//     NFSO: (1+1)^2 = 2^2 = 4. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOC: 3×2^8 = 3×256 = 768. ✓
//     NHHS: 2×(2+2)^7 = 2×4^7 = 2×16_384 = 32_768. ✓
//     NFSO: 2×(4+4)^2 = 2×64 = 128. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOC: 3×4^8 = 3×65_536 = 196_608. ✓
//     NHHS: 3×(4+4)^7 = 3×8^7 = 3×2_097_152 = 6_291_456. ✓
//     NFSO: 3×(16+16)^2 = 3×32^2 = 3×1_024 = 3_072. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOC: 5×4^8 = 5×65_536 = 327_680. ✓
//     NHHS: 4×8^7 = 4×2_097_152 = 8_388_608. ✓
//     NFSO: 4×32^2 = 4×1_024 = 4_096. ✓
//     Note: K₃ and K_{1,4} share S=4; same per-edge NHHS and NFSO; NOC differs by n.
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOC: 2^8+3^8+3^8+2^8 = 256+6_561+6_561+256 = 13_634. ✓
//     NHHS: 5^7+6^7+5^7 = 78_125+279_936+78_125 = 436_186. ✓
//       (5^7=78_125; 6^7=279_936)
//     NFSO: (4+9)^2+(9+9)^2+(9+4)^2 = 13^2+18^2+13^2 = 169+324+169 = 662. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOC: 4×9^8 = 4×43_046_721 = 172_186_884. ✓
//     NHHS: 6×18^7 = 6×612_220_032 = 3_673_320_192. ✓
//       (18^7: 18^2=324; 18^3=5832; 18^4=104_976; 18^5=1_889_568; 18^6=34_012_224; 18^7=612_220_032)
//     NFSO: 6×(81+81)^2 = 6×162^2 = 6×26_244 = 157_464. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOC: 5×6^8 = 5×1_679_616 = 8_398_080. ✓
//     NHHS: 6×12^7 = 6×35_831_808 = 214_990_848. ✓
//       (12^7: 12^2=144; 12^3=1728; 12^4=20_736; 12^5=248_832; 12^6=2_985_984; 12^7=35_831_808)
//     NFSO: 6×(36+36)^2 = 6×72^2 = 6×5_184 = 31_104. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOC  = n·S^8 for S-regular ✓
//   NHHS = |E|·(2S)^7 = 128|E|·S^7 for S-regular ✓
//   NFSO = |E|·(2S²)^2 = 4|E|·S^4 for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 128, 4, 1, 2)
//  4.  Path P₃ = A-B-C                   → (768, 32_768, 128, 2, 3)
//  5.  Triangle K₃                       → (196_608, 6_291_456, 3_072, 3, 3)
//  6.  Star K_{1,4}                      → (327_680, 8_388_608, 4_096, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (13_634, 436_186, 662, 3, 4)
//  8.  Complete K₄                       → (172_186_884, 3_673_320_192, 157_464, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (8_398_080, 214_990_848, 31_104, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T34_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_34");
const T34_EXEC:   ExecutorId = ExecutorId::from_ascii("t34.exec");

const T34_KEY_A: &str = "t34.alpha";
const T34_KEY_B: &str = "t34.beta";
const T34_KEY_C: &str = "t34.gamma";
const T34_KEY_D: &str = "t34.delta";
const T34_KEY_E: &str = "t34.epsilon";

const T34_ID_A: NodeId = derive_node_id(T34_PLUGIN, T34_KEY_A);
const T34_ID_B: NodeId = derive_node_id(T34_PLUGIN, T34_KEY_B);
const T34_ID_C: NodeId = derive_node_id(T34_PLUGIN, T34_KEY_C);
const T34_ID_D: NodeId = derive_node_id(T34_PLUGIN, T34_KEY_D);
const T34_ID_E: NodeId = derive_node_id(T34_PLUGIN, T34_KEY_E);

// L4=121 namespace for this harness.
const T34_VEC_A: VectorAddress = VectorAddress::new(121, 1, 1, 0);
const T34_VEC_B: VectorAddress = VectorAddress::new(121, 1, 2, 0);
const T34_VEC_C: VectorAddress = VectorAddress::new(121, 1, 3, 0);
const T34_VEC_D: VectorAddress = VectorAddress::new(121, 2, 1, 0);
const T34_VEC_E: VectorAddress = VectorAddress::new(121, 2, 2, 0);

const T34_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T34_PLUGIN,
    name:         "kl-graph-topo34-harness",
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
        executor_id:       T34_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T34_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T34_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (noc, nhhs, nfso, ec, nc) = gos_runtime::graph_topo_indices34();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(noc,  0, "empty: NOC=0");
    assert_eq!(nhhs, 0, "empty: NHHS=0");
    assert_eq!(nfso, 0, "empty: NFSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NOC: 0^8=0; NHHS: no edges; NFSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T34_VEC_A, T34_KEY_A, T34_ID_A);

    let (noc, nhhs, nfso, ec, nc) = gos_runtime::graph_topo_indices34();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(noc,  0, "single: NOC=0 (S=0; 0^8=0)");
    assert_eq!(nhhs, 0, "single: NHHS=0 (no edges)");
    assert_eq!(nfso, 0, "single: NFSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NOC: 1^8+1^8 = 2.
// NHHS: (1+1)^7 = 2^7 = 128.
// NFSO: (1^2+1^2)^2 = 2^2 = 4.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T34_VEC_A, T34_KEY_A, T34_ID_A);
    add_node(T34_VEC_B, T34_KEY_B, T34_ID_B);
    add_edge(T34_ID_A, T34_ID_B, "t34.e.ab");

    let (noc, nhhs, nfso, ec, nc) = gos_runtime::graph_topo_indices34();
    assert_eq!(nc,   2,   "k2: node_count=2");
    assert_eq!(ec,   1,   "k2: edge_count=1");
    assert_eq!(noc,  2,   "k2: NOC=2 (1\u{2078}+1\u{2078}=2; S-uniform S=1)");
    assert_eq!(nhhs, 128, "k2: NHHS=128 ((1+1)\u{2077}=2\u{2077}=128; S-uniform S=1)");
    assert_eq!(nfso, 4,   "k2: NFSO=4 ((1\u{00b2}+1\u{00b2})\u{00b2}=2\u{00b2}=4; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NOC: 3×2^8 = 3×256 = 768.
// NHHS: 2×(2+2)^7 = 2×4^7 = 2×16_384 = 32_768.
// NFSO: 2×(4+4)^2 = 2×64 = 128.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T34_VEC_A, T34_KEY_A, T34_ID_A);
    add_node(T34_VEC_B, T34_KEY_B, T34_ID_B);
    add_node(T34_VEC_C, T34_KEY_C, T34_ID_C);
    add_edge(T34_ID_A, T34_ID_B, "t34.e.ab");
    add_edge(T34_ID_B, T34_ID_C, "t34.e.bc");

    let (noc, nhhs, nfso, ec, nc) = gos_runtime::graph_topo_indices34();
    assert_eq!(nc,   3,      "p3: node_count=3");
    assert_eq!(ec,   2,      "p3: edge_count=2");
    assert_eq!(noc,  768,    "p3: NOC=768 (3\u{00d7}256; 2\u{2078}=256; S-uniform S=2)");
    assert_eq!(nhhs, 32_768, "p3: NHHS=32_768 (2\u{00d7}16_384; (2+2)\u{2077}=4\u{2077}=16_384; S-uniform S=2)");
    assert_eq!(nfso, 128,    "p3: NFSO=128 (2\u{00d7}64; (4+4)\u{00b2}=8\u{00b2}=64; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NOC: 3×4^8 = 3×65_536 = 196_608.
// NHHS: 3×(4+4)^7 = 3×8^7 = 3×2_097_152 = 6_291_456.
// NFSO: 3×(16+16)^2 = 3×32^2 = 3×1_024 = 3_072.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T34_VEC_A, T34_KEY_A, T34_ID_A);
    add_node(T34_VEC_B, T34_KEY_B, T34_ID_B);
    add_node(T34_VEC_C, T34_KEY_C, T34_ID_C);
    add_edge(T34_ID_A, T34_ID_B, "t34.e.ab");
    add_edge(T34_ID_B, T34_ID_A, "t34.e.ba");
    add_edge(T34_ID_B, T34_ID_C, "t34.e.bc");
    add_edge(T34_ID_C, T34_ID_B, "t34.e.cb");
    add_edge(T34_ID_A, T34_ID_C, "t34.e.ac");
    add_edge(T34_ID_C, T34_ID_A, "t34.e.ca");

    let (noc, nhhs, nfso, ec, nc) = gos_runtime::graph_topo_indices34();
    assert_eq!(nc,   3,          "k3: node_count=3");
    assert_eq!(ec,   3,          "k3: edge_count=3");
    assert_eq!(noc,  196_608,    "k3: NOC=196_608 (3\u{00d7}65_536; 4\u{2078}=65_536; S-uniform S=4)");
    assert_eq!(nhhs, 6_291_456,  "k3: NHHS=6_291_456 (3\u{00d7}2_097_152; (4+4)\u{2077}=8\u{2077}=2_097_152; S-uniform S=4)");
    assert_eq!(nfso, 3_072,      "k3: NFSO=3_072 (3\u{00d7}1_024; (16+16)\u{00b2}=32\u{00b2}=1_024; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-edge NHHS (2_097_152) and NFSO (1_024) as K₃; NOC and totals differ.
// NOC: 5×4^8 = 5×65_536 = 327_680.
// NHHS: 4×8^7 = 4×2_097_152 = 8_388_608.
// NFSO: 4×32^2 = 4×1_024 = 4_096.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T34_VEC_A, T34_KEY_A, T34_ID_A);
    add_node(T34_VEC_B, T34_KEY_B, T34_ID_B);
    add_node(T34_VEC_C, T34_KEY_C, T34_ID_C);
    add_node(T34_VEC_D, T34_KEY_D, T34_ID_D);
    add_node(T34_VEC_E, T34_KEY_E, T34_ID_E);
    add_edge(T34_ID_A, T34_ID_B, "t34.e.ab");
    add_edge(T34_ID_A, T34_ID_C, "t34.e.ac");
    add_edge(T34_ID_A, T34_ID_D, "t34.e.ad");
    add_edge(T34_ID_A, T34_ID_E, "t34.e.ae");

    let (noc, nhhs, nfso, ec, nc) = gos_runtime::graph_topo_indices34();
    assert_eq!(nc,   5,          "star: node_count=5");
    assert_eq!(ec,   4,          "star: edge_count=4");
    assert_eq!(noc,  327_680,    "star: NOC=327_680 (5\u{00d7}65_536; same S as K\u{2083})");
    assert_eq!(nhhs, 8_388_608,  "star: NHHS=8_388_608 (4\u{00d7}2_097_152; same per-edge as K\u{2083})");
    assert_eq!(nfso, 4_096,      "star: NFSO=4_096 (4\u{00d7}1_024; same per-edge as K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NOC: 2^8+3^8+3^8+2^8 = 256+6_561+6_561+256 = 13_634.
// NHHS: (2+3)^7+(3+3)^7+(3+2)^7 = 5^7+6^7+5^7 = 78_125+279_936+78_125 = 436_186.
// NFSO: (4+9)^2+(9+9)^2+(9+4)^2 = 13^2+18^2+13^2 = 169+324+169 = 662.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T34_VEC_A, T34_KEY_A, T34_ID_A);
    add_node(T34_VEC_B, T34_KEY_B, T34_ID_B);
    add_node(T34_VEC_C, T34_KEY_C, T34_ID_C);
    add_node(T34_VEC_D, T34_KEY_D, T34_ID_D);
    add_edge(T34_ID_A, T34_ID_B, "t34.e.ab");
    add_edge(T34_ID_B, T34_ID_C, "t34.e.bc");
    add_edge(T34_ID_C, T34_ID_D, "t34.e.cd");

    let (noc, nhhs, nfso, ec, nc) = gos_runtime::graph_topo_indices34();
    assert_eq!(nc,   4,       "p4: node_count=4");
    assert_eq!(ec,   3,       "p4: edge_count=3");
    assert_eq!(noc,  13_634,  "p4: NOC=13_634 (256+6561+6561+256; 2\u{2078}+3\u{2078}+3\u{2078}+2\u{2078})");
    assert_eq!(nhhs, 436_186, "p4: NHHS=436_186 (78_125+279_936+78_125; 5\u{2077}+6\u{2077}+5\u{2077})");
    assert_eq!(nfso, 662,     "p4: NFSO=662 (169+324+169; 13\u{00b2}+18\u{00b2}+13\u{00b2})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NOC: 4×9^8 = 4×43_046_721 = 172_186_884.
// NHHS: 6×18^7 = 6×612_220_032 = 3_673_320_192.
// NFSO: 6×(81+81)^2 = 6×162^2 = 6×26_244 = 157_464.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T34_VEC_A, T34_KEY_A, T34_ID_A);
    add_node(T34_VEC_B, T34_KEY_B, T34_ID_B);
    add_node(T34_VEC_C, T34_KEY_C, T34_ID_C);
    add_node(T34_VEC_D, T34_KEY_D, T34_ID_D);
    add_edge(T34_ID_A, T34_ID_B, "t34.e.ab");
    add_edge(T34_ID_B, T34_ID_A, "t34.e.ba");
    add_edge(T34_ID_A, T34_ID_C, "t34.e.ac");
    add_edge(T34_ID_C, T34_ID_A, "t34.e.ca");
    add_edge(T34_ID_A, T34_ID_D, "t34.e.ad");
    add_edge(T34_ID_D, T34_ID_A, "t34.e.da");
    add_edge(T34_ID_B, T34_ID_C, "t34.e.bc");
    add_edge(T34_ID_C, T34_ID_B, "t34.e.cb");
    add_edge(T34_ID_B, T34_ID_D, "t34.e.bd");
    add_edge(T34_ID_D, T34_ID_B, "t34.e.db");
    add_edge(T34_ID_C, T34_ID_D, "t34.e.cd");
    add_edge(T34_ID_D, T34_ID_C, "t34.e.dc");

    let (noc, nhhs, nfso, ec, nc) = gos_runtime::graph_topo_indices34();
    assert_eq!(nc,   4,               "k4: node_count=4");
    assert_eq!(ec,   6,               "k4: edge_count=6");
    assert_eq!(noc,  172_186_884,     "k4: NOC=172_186_884 (4\u{00d7}43_046_721; 9\u{2078}=43_046_721; S-uniform S=9)");
    assert_eq!(nhhs, 3_673_320_192,   "k4: NHHS=3_673_320_192 (6\u{00d7}612_220_032; 18\u{2077}=612_220_032; S-uniform S=9)");
    assert_eq!(nfso, 157_464,         "k4: NFSO=157_464 (6\u{00d7}26_244; 162\u{00b2}=26_244; S-uniform S=9)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NOC=0; NHHS=0; NFSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T34_VEC_A, T34_KEY_A, T34_ID_A);
    add_node(T34_VEC_B, T34_KEY_B, T34_ID_B);

    let (noc, nhhs, nfso, ec, nc) = gos_runtime::graph_topo_indices34();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(noc,  0, "isolated: NOC=0 (S=0; 0^8=0)");
    assert_eq!(nhhs, 0, "isolated: NHHS=0 (no edges)");
    assert_eq!(nfso, 0, "isolated: NFSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NOC: 5×6^8 = 5×1_679_616 = 8_398_080.
// NHHS: 6×12^7 = 6×35_831_808 = 214_990_848.
// NFSO: 6×(36+36)^2 = 6×72^2 = 6×5_184 = 31_104.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T34_VEC_A, T34_KEY_A, T34_ID_A);
    add_node(T34_VEC_B, T34_KEY_B, T34_ID_B);
    add_node(T34_VEC_C, T34_KEY_C, T34_ID_C);
    add_node(T34_VEC_D, T34_KEY_D, T34_ID_D);
    add_node(T34_VEC_E, T34_KEY_E, T34_ID_E);
    add_edge(T34_ID_A, T34_ID_C, "t34.e.ac");
    add_edge(T34_ID_C, T34_ID_A, "t34.e.ca");
    add_edge(T34_ID_A, T34_ID_D, "t34.e.ad");
    add_edge(T34_ID_D, T34_ID_A, "t34.e.da");
    add_edge(T34_ID_A, T34_ID_E, "t34.e.ae");
    add_edge(T34_ID_E, T34_ID_A, "t34.e.ea");
    add_edge(T34_ID_B, T34_ID_C, "t34.e.bc");
    add_edge(T34_ID_C, T34_ID_B, "t34.e.cb");
    add_edge(T34_ID_B, T34_ID_D, "t34.e.bd");
    add_edge(T34_ID_D, T34_ID_B, "t34.e.db");
    add_edge(T34_ID_B, T34_ID_E, "t34.e.be");
    add_edge(T34_ID_E, T34_ID_B, "t34.e.eb");

    let (noc, nhhs, nfso, ec, nc) = gos_runtime::graph_topo_indices34();
    assert_eq!(nc,   5,           "k23: node_count=5");
    assert_eq!(ec,   6,           "k23: edge_count=6");
    assert_eq!(noc,  8_398_080,   "k23: NOC=8_398_080 (5\u{00d7}1_679_616; 6\u{2078}=1_679_616; S-uniform S=6)");
    assert_eq!(nhhs, 214_990_848, "k23: NHHS=214_990_848 (6\u{00d7}35_831_808; 12\u{2077}=35_831_808; S-uniform S=6)");
    assert_eq!(nfso, 31_104,      "k23: NFSO=31_104 (6\u{00d7}5_184; 72\u{00b2}=5_184; S-uniform S=6)");
}
