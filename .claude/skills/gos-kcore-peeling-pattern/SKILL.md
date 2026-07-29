---
name: gos-kcore-peeling-pattern
description: When implementing k-core decomposition (Batagelj-Zaversnik peeling) in GOSKernel, surviving nodes need coreness = k.saturating_sub(1) set AFTER the loop exits — not left at 0. Also: star-graph hubs get coreness=1 not degree; peeling-step neighbor updates must deduplicate. Apply in crates/gos-runtime/src/lib.rs graph_kcore_inner.
---

# K-Core Peeling: Loop Exit and Two Gotchas

## The rule

The outer peeling loop increments `k` one past its last successful peel. After it exits, surviving (non-removed) nodes must have their coreness set explicitly:

```rust
let mut k: u8 = 1;
while k <= max_deg && remaining > 0 {
    // ... peel at this k ...
    k = k.saturating_add(1);   // ← k is incremented here
}
// After loop: k == max_deg + 1 (or where remaining hit 0 and we stopped)
let final_k = k.saturating_sub(1);   // ← one less than current k
for ki in 0..nc {
    let slot = node_slots[ki];
    if !removed[slot] {
        coreness[slot] = final_k;     // ← survivors' coreness = final_k, NOT k
    }
}
```

**Off-by-one trap**: if you use `k` instead of `k.saturating_sub(1)`, K₄ would show coreness=4 instead of the correct coreness=3.

## Why it's non-obvious

The loop exits when `k > max_deg`. At that point `k = max_deg + 1`. Survivors have degree ≥ max_deg in the remaining subgraph, so their coreness = max_deg = `k - 1`. Using `k` directly is always off by one.

## Two additional gotchas

**Gotcha 1 — Star graph coreness is 1, not hub degree.**

A hub connected to 3 leaves (star K₁,₃): hub degree=3, each leaf degree=1.
- k=1: no removals (all degree ≥ 1)
- k=2: leaves have effective_degree=1 < 2 → removed (coreness=1). After removal, hub's effective_degree drops to 0 < 2 → also removed (coreness=1).
- Degeneracy = 1, not 3.

Counter-intuitive: high-degree hubs in a star ARE NOT high-coreness nodes because their entire degree is contributed by low-degree leaves that get peeled away first.

**Gotcha 2 — Deduplication in the peeling update step.**

When node v is removed, decrement each distinct non-removed neighbor's effective_degree exactly once, even if multiple edges connect v to that neighbor:

```rust
let mut seen_u = [NodeId::ZERO; MAX_NODES];
let mut nb_u   = 0usize;
for edge in self.edges.iter().flatten() {
    let other = /* undirected neighbor */;
    if seen_u[..nb_u].contains(&other) { continue; }  // ← dedup
    seen_u[nb_u] = other;
    nb_u += 1;
    if let Some(ns) = self.node_slot_by_id(other) {
        if !removed[ns] && eff_deg[ns] > 0 {
            eff_deg[ns] -= 1;  // ← only decremented once per distinct neighbor
        }
    }
}
```

Without dedup, parallel edges cause double-decrement → effective_degree goes negative (wraps on u8) → incorrect coreness for the affected node.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_kcore_inner<const N: usize>()`
- Returns `([VectorAddress; N], [u8; N], usize, u8)` = (vecs, coreness, n, max_coreness)
- Uses `&self` pattern (same as graph_clustering_inner, graph_transitivity_inner) — NOT the snapshot pattern
- VectorAddress L4=40 reserved for gos-graph-kcore-harness test nodes
- Shell: "graph kcore" / "kcore" / "gkcore" / "coreness"
- `r.vector` on a NodeRecord gives its VectorAddress (do NOT call node_vector() which returns Result)

## From this session

V2.64: K₄ (complete 4-graph) test verified the off-by-one: all 4 nodes must show coreness=3. Triangle+pendant test verified mixed coreness: A/B/C=2, D=1. Star test verified coreness=1 for all nodes including hub. Two-disjoint-triangles test verified degeneracy=2. All 10 tests passed first try because these gotchas were caught during design, not during debugging.
