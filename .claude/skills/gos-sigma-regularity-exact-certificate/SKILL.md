---
name: gos-sigma-regularity-exact-certificate
description: The Sigma index σ = Σ_{uv∈E} (da-db)² is an exact algebraic regularity certificate: σ = 0 iff every edge has da=db iff the graph is regular. Unlike GA≤|E| or SDD≥2|E| (inequality bounds), σ=0 is a direct algebraic identity — no floor error, no AM-GM, just whether the sum of non-negative terms vanishes.
---

# σ(G) = 0 ↔ Regular Graph (Exact Algebraic Certificate)

## The rule

The Sigma index σ(G) = Σ_{uv∈E} (da - db)²:

```rust
// σ contribution per undirected edge (a, b):
let diff = if da >= db { da - db } else { db - da };
sigma_acc += diff * diff;
// σ is an exact integer — no isqrt64, no ppm, no floor error
```

Regularity criterion:
```
σ = 0   →  every edge has da = db  →  graph is regular
σ > 0   →  at least one edge has da ≠ db  →  graph is irregular
```

**Use σ = 0 as the rigorous regularity test in harness assertions:**

```rust
// Regular graph K₃ (all da=db=2): σ must be exactly 0
assert_eq!(sigma, 0, "K₃: σ=0 certifies regularity (Δ=2 regular)");

// Non-regular star K_{1,4} (da=4, db=1 for all edges):
// σ = 4 × (4-1)² = 4 × 9 = 36
assert!(sigma > 0, "K_{{1,4}}: σ>0 (non-regular graph)");

// Non-regular K_{2,3} (da=3 or da=2 for endpoints):
// σ = 6 × (3-2)² = 6
assert_eq!(sigma, 6, "K_{{2,3}}: σ=6 (all 6 edges have |3-2|=1)");
```

Also use as a "free" annotation trigger in the shell display:
```rust
if sigma == 0 && edge_count > 0 {
    // annotate: "(regular: σ=0)"
}
```

## Why it's non-obvious

**σ is different from GA and SDD regularity tests:**

| Index | Test | Type |
|-------|------|------|
| GA = |E| | ga_ppm == ec × 1_000_000 | Inequality bound (AM-GM ≤) — can have floor errors |
| SDD = 2|E| | sdd_ppm == ec × 2_000_000 | Inequality bound (AM-GM ≥) — exact for regular |
| **σ = 0** | sigma == 0 | **Direct algebraic identity** — always exact, never floor-rounded |

σ = 0 is the *strongest* regularity test because it's direct: each (da-db)² ≥ 0, so the sum is 0 iff every term is 0, iff every edge has da=db, iff the graph is regular. No inequality inversion, no precision concern.

By contrast:
- GA = |E| can be off by 1 ppm due to floor in isqrt → in principle could have false positive for near-regular graphs
- SDD = 2|E| is exact for regular (floor has no effect when da=db) but relies on (da²+db²)/(da·db) = 2 exactly

σ's regularity certificate is also more discriminating than looking at the degree sequence: it captures *which edges* are irregular (those with diff ≠ 0), enabling per-edge analysis if needed.

## GOSKernel context

- Implemented in `graph_topo_indices4_inner` (V3.15, `crates/gos-runtime/src/lib.rs`)
- Shell: `graph topo4` / `gsigma` / `sigma index`
- Display annotates with "σ=0 (regular)" when sigma=0 and edge_count>0
- Overflow safety: max diff = 127; max diff² = 16_129; max sigma = 512 × 16_129 ≈ 8.3×10^6 — trivially fits u64
- Related regularity tests: [[gos-ga-regularity-invariant]] (GA upper bound), [[gos-sdd-regularity-invariant]] (SDD lower bound)

## From this session

V3.15 tests confirmed:
- K₃ (Δ=2 regular): sigma=0 ✓ (all edges have da=db=2, diff=0 always)
- K₄ (Δ=3 regular): sigma=0 ✓ (all edges have da=db=3)
- K_{1,4} (star, non-regular): sigma=36=4×9 ✓ (4 edges, each |4-1|²=9)
- K_{2,3} (bipartite, non-regular): sigma=6=6×1 ✓ (6 edges, each |3-2|²=1)
- P₃, P₄ (paths, non-regular): sigma=2 ✓ (pendant edges each contribute 1)
