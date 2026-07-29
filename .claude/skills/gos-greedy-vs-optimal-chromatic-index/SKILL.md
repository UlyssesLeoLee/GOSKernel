---
name: gos-greedy-vs-optimal-chromatic-index
description: When testing greedy edge coloring in GOSKernel, do NOT assert that bipartite graphs achieve χ'=Δ (König's theorem) — greedy only guarantees Vizing's bound χ'≤Δ+1; assert the actual greedy result or just check proper coloring + Vizing invariant.
---

# Greedy χ' vs Optimal χ': Don't Assert König for Greedy Tests

## The rule

**Do NOT write**: `assert_eq!(chromatic_index, max_degree, "bipartite: χ'=Δ")`

**Do write**:
```rust
// Vizing invariant: χ'(G) ∈ {Δ, Δ+1}
assert!(chi >= max_deg,     "chi' must be at least Delta");
assert!(chi <= max_deg + 1, "Vizing: chi' <= Delta+1");
// OR assert the exact greedy result if you know the edge ordering:
assert_eq!(chi, 4, "K_{3,3} greedy result for this ordering");
// AND always verify proper coloring:
assert_proper_coloring(&from_vecs, &to_vecs, &edge_colors, ec);
```

## Why it's non-obvious

**König's theorem (1916)** says bipartite graphs are "class 1": the *optimal* chromatic index equals Δ.
**Vizing's theorem (1964)** says any graph has χ'∈{Δ, Δ+1}.

A **greedy** algorithm processes edges in slot order and always assigns the lowest available colour. For bipartite K_{3,3} (Δ=3), greedy gives χ'=4 (Δ+1) with the natural edge ordering:
- Edge AD: colour 0 → A={0}, D={0}
- Edge AE: colour 1 → A={0,1}, E={1}
- Edge AF: colour 2 → A={0,1,2}, F={2}
- Edge BD: colour 1 → B={1}, D={0,1}
- Edge BE: colour 0 → B={0,1}, E={0,1}
- Edge BF: **forbidden={0,1}∪{2}={0,1,2}, colour 3** → B={0,1,3}, F={2,3}
- Edge CD: colour 2 → C={2}, D={0,1,2}
- Edge CE: colour 3 → C={2,3}, E={0,1,3}
- Edge CF: **forbidden={2,3}∪{2,3}={2,3}, colour 0** → C={0,2,3}, F={0,2,3}
→ χ'=4, not the König-optimal 3.

König's theorem does NOT say greedy achieves the optimum — it says the optimum exists. Achieving it requires a smarter algorithm (e.g., Misra-Gries 1992 which runs in O(E·V)).

## GOSKernel context

- `gos_runtime::graph_edge_color` (V3.08) — greedy, O(E), achieves Vizing bound not König
- If you need König-optimal coloring for bipartite graphs, a different algorithm is required
- Test 10 in `gos-graph-ecolor-harness` was initially wrong; fixed to assert greedy result (χ'=4)

## From this session

V3.08 test 10 panicked: `assertion failed: K_{3,3}: chi'=3=Delta; left=4, right=3`.
Root cause: conflated "König theorem (optimal)" with "greedy algorithm (approximation)".
Fix: changed assertion from `== 3` to `== 4` (the actual greedy result) and documented why.
The proper-coloring validity check (`assert_proper_coloring`) is still strong — it catches any actual error.
