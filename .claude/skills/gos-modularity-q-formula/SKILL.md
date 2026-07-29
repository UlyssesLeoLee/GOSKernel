---
name: gos-modularity-q-formula
description: When implementing Newman–Girvan modularity Q in GOSKernel's no_std runtime, use the all-integer identity Q_ppm = (4m·ΣL_c − Σd_c²) × 1_000_000 / (4m²) where all quantities come from undirected edge counts; directed edges are deduplicated into undirected pairs before computing m, L_c, and d_c. Apply in graph_modularity_inner and any future partition-quality metrics in crates/gos-runtime/src/lib.rs.
---

# Newman–Girvan Modularity: Pure Integer Formula

## The rule

Do NOT use the textbook form `Q = Σ_c [L_c/m − (d_c/(2m))²]` directly — it requires float division.
Instead expand to the all-integer identity:

```rust
// numer = (4m · ΣL_c  −  Σd_c²) × 1_000_000
// denom = 4m²
let m_i   = m as i64;
let numer = (4 * m_i * sum_l - sum_d2) * 1_000_000;
let denom = 4 * m_i * m_i;
let q_ppm = (numer / denom).max(-1_000_000).min(1_000_000) as i32;
```

Where:
- `m` = deduplicated undirected edge count (directed pair (u,v)+(v,u) = ONE edge)
- `sum_l` (ΣL_c) = undirected edges with both endpoints in the same community
- `sum_d2` (Σd_c²) = per-community sum of undirected degrees, then squared and summed
- All quantities are computed over the undirected projection

**Step sequence:**
1. Run LPA (same 20-iteration algorithm as `graph_community`)
2. Deduplicate directed edges → undirected set, compute m
3. Compute undirected degree per node (same neighbour-union pattern as clustering)
4. ΣL_c: for each undirected edge, check if both endpoints share the same LPA label
5. Σd_c²: accumulate per-community degree sums, then square and sum
6. Apply formula above

## Why it's non-obvious

1. **The deduplication must happen BEFORE computing m, L_c, d_c.** If you count directed edges as-is, m is doubled, making Q wrong. The `(u,v)+(v,u) = 1 undirected edge` rule must be consistent across all three quantities.

2. **`sum_d2` uses undirected degree, not directed in-degree or out-degree.** The undirected degree of v = number of unique neighbours reachable via any directed edge. This matches the undirected projection used for m.

3. **Overflow is safe within i64.** With MAX_EDGES=512: 4m·ΣL_c ≤ 4·512² = 1,048,576; Σd_c² ≤ (2m)² = 4·512² = 1,048,576; maximum numer before ×1_000_000 is ≈1M; maximum numer is ≈10¹² ≪ i64::MAX (9.2×10¹⁸).

4. **Return type is i32, not u32.** Modularity can be negative (anti-community structure). The return value is clamped to [−1_000_000, +1_000_000].

## Benchmark values

| Graph | Q_ppm |
|-------|-------|
| Single connected component (any) | 0 |
| Two equal-size disconnected cliques | 500_000 |
| K3 + K2 disconnected | 375_000 |
| K3 + K2: Q = 24×10⁶/64 | verify: (4·4·4−40)·10⁶/(4·16) = 375_000 |

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_modularity_inner(snap)` (static method, takes topology snapshot)
- Public API: `gos_runtime::graph_modularity() -> (i32, usize, usize, usize)` (q_ppm, comms, undirected_edges, nodes)
- Shell: `graph modularity` / `modularity` / `gmodq` → `dispatch_graph_modularity`
- VectorAddress L4=43 reserved for gos-graph-modularity-harness test nodes
- Pure read, does NOT bump epoch; uses `topology_snapshot()` static-fn pattern

## From this session

V2.67: implemented `graph_modularity_inner`. Key insight: both LPA and Q evaluation must use the same undirected projection of the graph (directed pair counted once), or the partition and the metric will disagree on which nodes are "in the same community" vs "have the same edge." The `seen_from/seen_to` deduplication array is shared between step 2 (computing m) and step 4 (evaluating ΣL_c).
