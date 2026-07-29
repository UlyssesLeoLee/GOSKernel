---
name: gos-assortativity-integer-formula
description: When implementing Newman (2002) degree assortativity in GOSKernel's no_std runtime, use the i64 formula (4·M·S1−T²)/(2·M·Q−T²) to avoid all floating point; denominator=0 for regular graphs is not an error — return 0 (undefined). Apply in crates/gos-runtime/src/lib.rs graph_assortativity_inner and any future graph correlation metrics.
---

# Newman Degree Assortativity: Pure Integer Formula

## The rule

Compute degree assortativity using only i64 arithmetic. Iterate each stored directed edge once; use undirected neighbour count as the degree (same definition as graph_clustering / graph_transitivity):

```rust
// Pre-compute undirected degree per node slot (same neighbor-union pattern as clustering)
let mut deg = [0u32; MAX_NODES];
// ... fill deg[slot] for each node ...

let mut s1: i64 = 0;   // Σ j*k
let mut t:  i64 = 0;   // Σ (j+k)
let mut q:  i64 = 0;   // Σ (j²+k²)
for edge in self.edges.iter().flatten() {
    let j = deg[slot_of(u)] as i64;
    let k = deg[slot_of(v)] as i64;
    s1 += j * k;
    t  += j + k;
    q  += j * j + k * k;
}
let m_i = m as i64;
let numer = 4 * m_i * s1 - t * t;
let denom = 2 * m_i * q  - t * t;
if denom == 0 { return (0, m, n); }   // ← regular graph: undefined → 0
let r_ppm = ((numer * 1_000_000) / denom)
    .max(-1_000_000)
    .min(1_000_000) as i32;
```

## Why it's non-obvious

1. **Denominator = 0 is not a bug.** For any regular graph (all nodes same degree d), every edge contributes the same (j,k)=(d,d). Then 2·M·Q−T² = 2M·2d²−(2Md)² / M²... algebraically simplifies to 0. This is the mathematical "undefined" case (Pearson correlation of a constant), not a divide-by-zero error. Returning 0 is the correct convention.

2. **Overflow is safe within i64.** With MAX_NODES=128 and MAX_EDGES=512: 4·M·S1 ≤ 4·512·(512·128²) ≈ 17×10⁹ and T² ≤ (512·256)² ≈ 17×10⁹ — both well within i64's 9.2×10¹⁸ range. The final `numer * 1_000_000` is at most 17×10¹⁵ < i64 max. No saturating_mul needed.

3. **The return type is i32, not u32.** Assortativity is signed; all other graph metrics in GOSKernel return u32 PPM. The shell dispatch must handle the sign manually since `print_num_inline` accepts only `usize`.

## Shell display pattern for signed PPM

```rust
let abs_ppm = if r_ppm < 0 { -(r_ppm as i64) } else { r_ppm as i64 } as usize;
if r_ppm < 0 { print_str(sink, "-"); }
print_num_inline(sink, abs_ppm / 10_000);      // integer part
print_str(sink, ".");
if abs_ppm % 10_000 / 100 < 10 { print_str(sink, "0"); }
print_num_inline(sink, abs_ppm % 10_000 / 100); // fractional part
```

## Canonical hand-verifiable test cases

| Graph | Calculation | r |
|-------|-------------|---|
| Path A→B→C (deg 1,2,1) | S1=4, T=6, Q=10, M=2; numer=−4, denom=4 | −1.0 |
| Star hub→B,C,D (deg 3,1,1,1) | S1=9, T=12, Q=30, M=3; numer=−36, denom=36 | −1.0 |
| K3-cycle + K2-pair (disjoint, different degrees) | S1=13, T=14, Q=26, M=4; numer=12, denom=12 | **+1.0** |
| Any regular graph (triangle, K4, …) | denom=0 | 0 (undefined) |

**The only small hand-constructible positive assortativity case**: two disjoint groups with different per-group degrees and NO inter-group edges. Both edges within K3 contribute (2,2) and the K2 edge contributes (1,1). All edges are same-degree-to-same-degree → r=+1.

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_assortativity_inner()`
- Public API: `gos_runtime::graph_assortativity() -> (i32, usize, usize)` (ppm, edges, nodes)
- Shell: "graph assortativity" / "assortativity" / "gassort" → `dispatch_graph_assortativity`
- VectorAddress L4=41 reserved for gos-graph-assortativity-harness test nodes
- Pure read, does NOT bump epoch

## From this session

V2.65: implemented graph_assortativity_inner following the patterns of graph_transitivity_inner and graph_kcore_inner. Key discovery: attempting to construct a positively assortative graph with hubs+leaves always yields r < 0 because hub-leaf connections dominate. The only tractable positive case is disjoint cliques of different sizes (K3 + K2-pair → r=+1, confirmed by hand-calculation and test 8).
