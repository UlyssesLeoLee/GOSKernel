---
name: gos-adj-canonical-pair-scan
description: When scanning undirected edges for per-edge aggregation (M2, Randić, irregularity, etc.) over already-built symmetric adj[] bitmasks, use `for a in 0..nc, for b in adj[a] where b > a` — no secondary seen_adj needed. Distinguish from the seen_adj pattern used when building the adj array itself from directed edges.
---

# Canonical-Pair Scan Over Symmetric adj[] Bitmasks

## The rule

Once you have symmetric undirected adjacency bitmasks (where `adj[a]` bit `b` is set iff `adj[b]` bit `a` is also set), enumerate each undirected edge exactly once with the `b > a` guard — no secondary dedup array needed:

```rust
// Phase 1 — build undirected adj (done once; uses seen_adj internally or dedup check)
let mut adj = [0u128; MAX_NODES];
for ei in 0..MAX_EDGES {
    // ... get f_ci, t_ci ...
    if f_ci == t_ci { continue; }
    if (adj[f_ci] >> t_ci) & 1 == 0 {
        adj[f_ci] |= 1u128 << t_ci;
        adj[t_ci] |= 1u128 << f_ci;  // symmetric!
    }
}

// Phase 2 — scan each undirected edge (a, b) exactly once
let mut edge_count = 0usize;
for a in 0..nc {
    let mut bits = adj[a];
    while bits != 0 {
        let b = bits.trailing_zeros() as usize;
        bits &= bits - 1;
        if b <= a { continue; }  // ← this one guard is sufficient
        // process undirected edge (a, b)
        edge_count += 1;
    }
}
```

## Why it's non-obvious

The `b <= a` guard works because:
- `adj[a]` contains ALL undirected neighbours of `a`, including those with `ci < a`
- For any undirected pair `{a, b}` with `a < b`: we encounter it in iteration `a` when `b ∈ adj[a]` (passes guard), and in iteration `b` when `a ∈ adj[b]` (skipped by guard)
- Result: each pair counted exactly once

This is simpler than allocating a secondary `seen_adj[MAX_NODES]` array for dedup during the edge scan phase. The key precondition is that adj[] is already **fully symmetric** (phase 1 sets both directions simultaneously).

Do NOT confuse with `gos-undirected-dedup-seen-adj` (which is for building the projection from directed edges when you only get one direction at a time).

## GOSKernel context

- First used explicitly in `graph_zagreb_inner` (V3.11, `crates/gos-runtime/src/lib.rs`)
- Also implicitly used in `graph_entropy_inner` (V3.10) when scanning adj for degrees (same symmetry property, though that function doesn't scan edges per-pair — it uses count_ones())
- The `seen_adj` dedup (from `gos-undirected-dedup-seen-adj`) is for building adj from directed edge stores; this `b > a` pattern is for consuming already-built adj

## From this session

V3.11 `graph_zagreb_inner`: needed to aggregate M2, Randić, irregularity per undirected edge. Rather than maintaining a `seen_adj` during the scan, used the `b > a` guard over the already-symmetric `adj[]` built in phase 2. Cleaner, saves 2KB stack.
