# Hardening Log V3.28 — Zagreb Coindices M̄₁ + M̄₂ + F̄ (2026-07-08)

## Summary

Added three **Zagreb coindex** topological graph invariants to `gos_runtime`:
- **M̄₁(G)** — first Zagreb coindex (Ashrafi, Došlić & Hamzeh 2010)
- **M̄₂(G)** — second Zagreb coindex (Ashrafi, Došlić & Hamzeh 2010)
- **F̄(G)**  — forgotten coindex (De 2016)

These are *complement-space* counterparts to the Zagreb indices (V3.11), summing over **non-edges** of the graph instead of edges. They are computed analytically via closed-form identities — **no complement scan required**, O(V+E) like all degree-based indices.

---

## Mathematical Definitions

For an undirected graph G = (V, E) with d_v = degree of v:

| Index | Definition | Formula |
|-------|-----------|---------|
| M̄₁(G) | Σ_{uv∉E, u≠v} (d_u + d_v) | = 2m(n−1) − M₁ |
| M̄₂(G) | Σ_{uv∉E, u≠v} d_u · d_v  | = 2m² − M₁/2 − M₂ |
| F̄(G)  | Σ_{uv∉E, u≠v} (d_u²+d_v²) | = (n−1)·M₁ − F |

Where M₁ = Σ_v d_v², M₂ = Σ_{uv∈E} d_u·d_v, F = Σ_v d_v³, m = |E|, n = |V|.

### Proof that M₁ is always even

M₁ = Σ d_v² ≡ #{odd-degree vertices} (mod 2). By the handshaking lemma, the number of odd-degree vertices is always even. Therefore M₁ is always even, and M₁/2 is always a non-negative integer.

### Key Invariants

- M̄₁ = M̄₂ = F̄ = 0 iff G is complete (no non-edges exist).
- M̄₁ ≥ 0, M̄₂ ≥ 0, F̄ ≥ 0 always (each term is non-negative).
- Comparing Zagreb vs Zagreb coindices reveals how much of the graph's degree pressure is in edges vs non-edges.

---

## Cross-Check Table

| Graph       | M̄₁ | M̄₂ | F̄  | edges | nodes |
|-------------|-----|-----|-----|-------|-------|
| Empty       | 0   | 0   | 0   | 0     | 0     |
| 1 node      | 0   | 0   | 0   | 0     | 1     |
| Edge A-B    | 0   | 0   | 0   | 1     | 2     |
| Path P₃     | 2   | 1   | 2   | 2     | 3     |
| Triangle K₃ | 0   | 0   | 0   | 3     | 3     |
| Star K_{1,4}| 12  | 6   | 12  | 4     | 5     |
| Path P₄     | 8   | 5   | 12  | 3     | 4     |
| Complete K₄ | 0   | 0   | 0   | 6     | 4     |
| Two isolated| 0   | 0   | 0   | 0     | 2     |
| K_{2,3}     | 18  | 21  | 42  | 6     | 5     |

---

## OS Analogy

| Index | OS Interpretation |
|-------|------------------|
| M̄₁   | "latent channel pressure" — sum of degree-sums across all missing IPC links; high value means many potential high-degree channels aren't connected |
| M̄₂   | "hub-hub complement co-load" — product of degrees across missing links; high value means high-degree nodes are NOT directly connected (hub isolation) |
| F̄    | "squared-degree complement pressure" — amplified version of M̄₁ emphasizing hubs; zero for fully connected meshes |

In a graph-OS context: M̄₁=M̄₂=F̄=0 is the ideal fully-meshed state (no missing critical links). A high ratio F̄/F indicates the graph is structurally sparse relative to full connectivity.

---

## Algorithm

O(V+E) degree scan — same complexity class as V3.11 (Zagreb indices):
1. Build compact node index and undirected adjacency bitmasks.
2. Compute degree array from bitmasks: d_v = popcount(adj[v]).
3. Accumulate M₁=Σd², M₂=Σ_{edges}d_u·d_v, F=Σd³ in two passes.
4. Apply identities:
   - M̄₁ = 2m(n−1) − M₁
   - M̄₂ = 2m² − M₁/2 − M₂
   - F̄  = (n−1)·M₁ − F

**No BFS, no complement graph enumeration needed.**

Stack: adj[128](u128=2KB) + deg[128](u64=1KB) ≈ 3KB total.

---

## Shell Interface

```
graph topo17        # full name
gtopo17             # short alias
zagreb coindex      # semantic alias
gcoindex            # short semantic
complement zagreb   # complement framing
gcozagreb           # short complement
forgotten coindex   # forgotten-coindex specific
gfbar               # F̄ specific
gm1barm2barfbar     # all three
```

---

## File Changes

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | Added `graph_topo_indices17_inner()` + public `graph_topo_indices17()` |
| `crates/k-shell/src/lib.rs` | Added `dispatch_graph_topo_indices17()` with colored display |
| `crates/k-shell/src/proc.rs` | Added dispatch branch for "graph topo17" et al. |
| `host-tests/gos-graph-topo17-harness/` | New harness: Cargo.toml, .cargo/config.toml, tests/graph_topo17.rs (10 tests) |

---

## Test Coverage

**10 new tests** in `gos-graph-topo17-harness`:
1. Empty graph → (0,0,0,0,0)
2. Single node → (0,0,0,0,1)
3. Edge A→B → (0,0,0,1,2) — no non-edges on a complete 2-node pair
4. Path P₃ → (2,1,2,2,3) — one non-edge {A,C}
5. Triangle K₃ → (0,0,0,3,3) — complete, no non-edges
6. Star K_{1,4} → (12,6,12,4,5) — 6 leaf-leaf non-edges
7. Path P₄ → (8,5,12,3,4) — 3 non-edges, mixed degrees
8. Complete K₄ → (0,0,0,6,4) — no non-edges
9. Two isolated nodes → (0,0,0,0,2) — zero-degree non-edge contributes 0
10. K_{2,3} bipartite → (18,21,42,6,5) — identity cross-check ✓

**Result: 10/10 PASS**  
**Cumulative host-test suite: 1253 tests** (was 1243 through V3.27)

---

## VectorAddress Namespace

L4=104 for `gos-graph-topo17-harness`

```
...102=graph-topo15, 103=graph-topo16, 104=graph-topo17
```

---

## Literature

- Ashrafi, A.R., Došlić, T. & Hamzeh, A. (2010). *The Zagreb coindices of graph operations.* Discrete Applied Mathematics, 158(15), 1571–1578.
- De, N. (2016). *The forgotten topological coindex.* AKCE International Journal of Graphs and Combinatorics.
- Gutman, I. & Trinajstić, N. (1972). *Graph theory and molecular orbitals.* (Zagreb indices, for comparison.)
