// gos-graph-spectral-harness — V3.09 spectral analysis tests
//
// Verifies `gos_runtime::graph_spectral()`:
//   - ρ(A): spectral radius of the undirected adjacency matrix (power iteration, 60 steps).
//   - λ₂(L): algebraic connectivity (Fiedler value) of the Laplacian L = D − A.
//
// Known exact values used for comparison (all within ±5_000 ppm tolerance ~ 0.5%):
//
//  Graph          ρ(A)             λ₂(L)          notes
//  Empty          0                0               trivial
//  1 node         0                0               single isolated
//  Single edge    1.000            2.000           eigenvalues A: {1,-1}; L: {0,2}
//  Path P₃        √2 ≈ 1.414       2−√2 ≈ 0.586    eigenvalues A: {√2,0,−√2}
//  Triangle K₃    2.000            3.000           eigenvalues A: {2,−1,−1}; L: {0,3,3}
//  Complete K₄    3.000            4.000           eigenvalues A: {3,−1,−1,−1}; L: {0,4,4,4}
//  Star K_{1,4}   2.000            1.000           eigenvalues A: {2,0,0,0,−2}; L: {0,1,1,1,1,5}
//  2 disconnected 0                0               λ₂=0 for disconnected
//  Path+isolated  1.000            0               A-B connected, C isolated → λ₂=0
//  Cycle C₄       2.000            2.000           eigenvalues A: {2,0,−2,0}; L: {0,2,2,4}
//
// Tests (10):
//  1.  Empty graph                       → ρ=0, λ₂=0.
//  2.  Single isolated node              → ρ=0, λ₂=0.
//  3.  Single edge A-B                   → ρ≈1.000, λ₂≈2.000.
//  4.  Path P₃ = A-B-C                   → ρ≈1.414, λ₂≈0.586.
//  5.  Triangle K₃                       → ρ≈2.000, λ₂≈3.000.
//  6.  Complete K₄                       → ρ≈3.000, λ₂≈4.000.
//  7.  Star K_{1,4}                      → ρ≈2.000, λ₂≈1.000.
//  8.  Two disconnected isolated nodes   → ρ=0, λ₂=0.
//  9.  Edge A-B plus isolated C          → ρ≈1.000, λ₂=0 (disconnected).
// 10.  Cycle C₄                          → ρ≈2.000, λ₂≈2.000.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const SP_PLUGIN: PluginId   = PluginId::from_ascii("KL_GRAPH_SPEC_H");
const SP_EXEC:   ExecutorId = ExecutorId::from_ascii("spectral.exec");

const SP_KEY_A: &str = "spectral.a";
const SP_KEY_B: &str = "spectral.b";
const SP_KEY_C: &str = "spectral.c";
const SP_KEY_D: &str = "spectral.d";
const SP_KEY_E: &str = "spectral.e";

const SP_ID_A: NodeId = derive_node_id(SP_PLUGIN, SP_KEY_A);
const SP_ID_B: NodeId = derive_node_id(SP_PLUGIN, SP_KEY_B);
const SP_ID_C: NodeId = derive_node_id(SP_PLUGIN, SP_KEY_C);
const SP_ID_D: NodeId = derive_node_id(SP_PLUGIN, SP_KEY_D);
const SP_ID_E: NodeId = derive_node_id(SP_PLUGIN, SP_KEY_E);

// L4=85 namespace for this harness.
const SP_VEC_A: VectorAddress = VectorAddress::new(85, 1, 1, 0);
const SP_VEC_B: VectorAddress = VectorAddress::new(85, 1, 2, 0);
const SP_VEC_C: VectorAddress = VectorAddress::new(85, 1, 3, 0);
const SP_VEC_D: VectorAddress = VectorAddress::new(85, 2, 1, 0);
const SP_VEC_E: VectorAddress = VectorAddress::new(85, 2, 2, 0);

const SP_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    SP_PLUGIN,
    name:         "kl-graph-spectral-harness",
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
        executor_id:       SP_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(SP_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(SP_MANIFEST).unwrap();
    g
}

/// Tolerance in ppm for power-iteration approximations (±5_000 ppm ≈ 0.5%).
const TOLERANCE: u32 = 5_000;

fn assert_ppm_approx(got: u32, expected_ppm: u32, label: &str) {
    let lo = expected_ppm.saturating_sub(TOLERANCE);
    let hi = expected_ppm.saturating_add(TOLERANCE);
    assert!(
        got >= lo && got <= hi,
        "{}: got {} ppm, expected {}±{} ppm",
        label, got, expected_ppm, TOLERANCE
    );
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (rho, lam2, nc) = gos_runtime::graph_spectral();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(rho,  0, "empty: rho=0");
    assert_eq!(lam2, 0, "empty: lambda2=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A);

    let (rho, lam2, nc) = gos_runtime::graph_spectral();
    assert_eq!(nc,   1, "single node: node_count=1");
    assert_eq!(rho,  0, "single node: rho=0 (no edges)");
    assert_eq!(lam2, 0, "single node: lambda2=0");
}

// ── Test 3: Single edge A-B — ρ=1, λ₂=2 ─────────────────────────────────────

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A);
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B);
    add_edge(SP_ID_A, SP_ID_B, "ab");

    let (rho, lam2, nc) = gos_runtime::graph_spectral();
    assert_eq!(nc, 2, "single edge: node_count=2");
    // ρ(A) = 1: eigenvalues of [[0,1],[1,0]] are ±1.
    assert_ppm_approx(rho,  1_000_000, "single edge: rho=1");
    // λ₂(L) = 2: eigenvalues of [[1,-1],[-1,1]] are 0,2.
    assert_ppm_approx(lam2, 2_000_000, "single edge: lambda2=2");
}

// ── Test 4: Path P₃ = A-B-C — ρ=√2, λ₂=1 ───────────────────────────────────

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A);
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B);
    add_node(SP_VEC_C, SP_KEY_C, SP_ID_C);
    add_edge(SP_ID_A, SP_ID_B, "ab");
    add_edge(SP_ID_B, SP_ID_C, "bc");

    let (rho, lam2, nc) = gos_runtime::graph_spectral();
    assert_eq!(nc, 3, "P3: node_count=3");
    // ρ(A) = √2 ≈ 1.41421; in ppm: 1_414_214.
    assert_ppm_approx(rho,  1_414_214, "P3: rho=sqrt(2)");
    // λ₂(L) = 1; Laplacian spectrum of P₃ is {0, 1, 3}.
    assert_ppm_approx(lam2, 1_000_000, "P3: lambda2=1");
}

// ── Test 5: Triangle K₃ — ρ=2, λ₂=3 ─────────────────────────────────────────

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A);
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B);
    add_node(SP_VEC_C, SP_KEY_C, SP_ID_C);
    add_edge(SP_ID_A, SP_ID_B, "ab");
    add_edge(SP_ID_B, SP_ID_C, "bc");
    add_edge(SP_ID_C, SP_ID_A, "ca");

    let (rho, lam2, nc) = gos_runtime::graph_spectral();
    assert_eq!(nc, 3, "K3: node_count=3");
    // ρ(A) = 2: eigenvalues of K₃ adjacency are {2, -1, -1}.
    assert_ppm_approx(rho,  2_000_000, "K3: rho=2");
    // λ₂(L) = 3: Laplacian eigenvalues of K₃ are {0, 3, 3}.
    assert_ppm_approx(lam2, 3_000_000, "K3: lambda2=3");
}

// ── Test 6: Complete K₄ — ρ=3, λ₂=4 ─────────────────────────────────────────

#[test]
fn test_06_complete_k4() {
    let _g = setup();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A);
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B);
    add_node(SP_VEC_C, SP_KEY_C, SP_ID_C);
    add_node(SP_VEC_D, SP_KEY_D, SP_ID_D);
    // One-direction edges; undirected projection = K₄.
    add_edge(SP_ID_A, SP_ID_B, "ab");
    add_edge(SP_ID_A, SP_ID_C, "ac");
    add_edge(SP_ID_A, SP_ID_D, "ad");
    add_edge(SP_ID_B, SP_ID_C, "bc");
    add_edge(SP_ID_B, SP_ID_D, "bd");
    add_edge(SP_ID_C, SP_ID_D, "cd");

    let (rho, lam2, nc) = gos_runtime::graph_spectral();
    assert_eq!(nc, 4, "K4: node_count=4");
    // ρ(A) = 3: eigenvalues of K₄ adjacency are {3, -1, -1, -1}.
    assert_ppm_approx(rho,  3_000_000, "K4: rho=3");
    // λ₂(L) = 4: Laplacian eigenvalues of K₄ are {0, 4, 4, 4}.
    assert_ppm_approx(lam2, 4_000_000, "K4: lambda2=4");
}

// ── Test 7: Star K_{1,4} — ρ=2, λ₂=1 ────────────────────────────────────────

#[test]
fn test_07_star_k14() {
    let _g = setup();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A); // centre
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B);
    add_node(SP_VEC_C, SP_KEY_C, SP_ID_C);
    add_node(SP_VEC_D, SP_KEY_D, SP_ID_D);
    add_node(SP_VEC_E, SP_KEY_E, SP_ID_E);
    add_edge(SP_ID_A, SP_ID_B, "ab");
    add_edge(SP_ID_A, SP_ID_C, "ac");
    add_edge(SP_ID_A, SP_ID_D, "ad");
    add_edge(SP_ID_A, SP_ID_E, "ae");

    let (rho, lam2, nc) = gos_runtime::graph_spectral();
    assert_eq!(nc, 5, "K_{{1,4}}: node_count=5");
    // ρ(A) = √4 = 2: eigenvalues of K_{1,4} adjacency are {2, 0, 0, 0, -2}.
    assert_ppm_approx(rho,  2_000_000, "K_{{1,4}}: rho=2");
    // λ₂(L) = 1: Laplacian eigenvalues of K_{1,4} are {0, 1, 1, 1, 1, 5}.
    assert_ppm_approx(lam2, 1_000_000, "K_{{1,4}}: lambda2=1");
}

// ── Test 8: Two disconnected isolated nodes — ρ=0, λ₂=0 ─────────────────────

#[test]
fn test_08_two_isolated() {
    let _g = setup();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A);
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B);
    // No edges — two isolated nodes.

    let (rho, lam2, nc) = gos_runtime::graph_spectral();
    assert_eq!(nc,   2, "2 isolated: node_count=2");
    assert_eq!(rho,  0, "2 isolated: rho=0 (no edges)");
    assert_eq!(lam2, 0, "2 isolated: lambda2=0 (disconnected)");
}

// ── Test 9: Edge A-B plus isolated node C — ρ=1, λ₂=0 ───────────────────────

#[test]
fn test_09_edge_plus_isolated() {
    let _g = setup();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A);
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B);
    add_node(SP_VEC_C, SP_KEY_C, SP_ID_C); // isolated
    add_edge(SP_ID_A, SP_ID_B, "ab");

    let (rho, lam2, nc) = gos_runtime::graph_spectral();
    assert_eq!(nc, 3, "edge+isolated: node_count=3");
    // ρ(A) = 1: eigenvalues of A are {1, 0, -1} (single edge + isolated).
    assert_ppm_approx(rho, 1_000_000, "edge+isolated: rho=1");
    // λ₂(L) = 0: graph is disconnected (C is isolated).
    assert_eq!(lam2, 0, "edge+isolated: lambda2=0 (disconnected)");
}

// ── Test 10: Cycle C₄ — ρ=2, λ₂=2 ───────────────────────────────────────────

#[test]
fn test_10_cycle_c4() {
    let _g = setup();
    add_node(SP_VEC_A, SP_KEY_A, SP_ID_A);
    add_node(SP_VEC_B, SP_KEY_B, SP_ID_B);
    add_node(SP_VEC_C, SP_KEY_C, SP_ID_C);
    add_node(SP_VEC_D, SP_KEY_D, SP_ID_D);
    add_edge(SP_ID_A, SP_ID_B, "ab");
    add_edge(SP_ID_B, SP_ID_C, "bc");
    add_edge(SP_ID_C, SP_ID_D, "cd");
    add_edge(SP_ID_D, SP_ID_A, "da");

    let (rho, lam2, nc) = gos_runtime::graph_spectral();
    assert_eq!(nc, 4, "C4: node_count=4");
    // ρ(A) = 2: eigenvalues of C₄ adjacency are {2, 0, -2, 0}.
    assert_ppm_approx(rho,  2_000_000, "C4: rho=2");
    // λ₂(L) = 2: Laplacian eigenvalues of C₄ are {0, 2, 2, 4}.
    assert_ppm_approx(lam2, 2_000_000, "C4: lambda2=2");
}
