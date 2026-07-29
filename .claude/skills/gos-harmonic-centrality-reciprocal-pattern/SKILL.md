---
name: gos-harmonic-centrality-reciprocal-pattern
description: When implementing harmonic centrality in GOSKernel, accumulate 1_000_000/dist[t] (integer reciprocal) for each reachable t≠s after BFS — NOT a sum of distances like Wiener, NOT a ratio like closeness. Division is always safe (dist[t] ≥ 1 for non-source reachable nodes). Source nodes in chains score HIGHER than intermediate nodes. Apply in crates/gos-runtime/src/lib.rs graph_harmonic_inner.
---

# Harmonic Centrality: Reciprocal-Sum BFS Pattern

## The rule

Harmonic centrality HC[v] = Σ 1_000_000/d(v,u) over all reachable u≠v.
After a plain BFS from source s, accumulate `SCALE / dist[t]` for all reachable t≠s:

```rust
pub fn graph_harmonic_inner<const N: usize>(&self) -> ([VectorAddress; N], [u32; N], usize) {
    const SCALE: u64 = 1_000_000;
    let mut hc = [0u64; MAX_NODES];

    for si in 0..node_count {
        let s = node_slots[si];
        // ... standard BFS, fills dist[*] ...

        // Accumulate reciprocal distances
        for ti in 0..node_count {
            let t = node_slots[ti];
            if t == s { continue; }                        // skip source
            if dist[t] != u32::MAX {                       // skip unreachable
                hc[s] = hc[s].saturating_add(SCALE / dist[t] as u64);  // 1e6/d
            }
        }
    }

    // Sort descending, pack output...
    (out_vecs, out_hc, copy_len)
}
```

Return type: `([VectorAddress; N], [u32; N], usize)` — same shape as graph_closeness.
Values fit in u32: max = 127 × 1_000_000 = 127_000_000 << u32::MAX (4_294_967_295).

## Why it's non-obvious

**Division is always safe — no guard needed**: BFS sets `dist[t] ≥ 1` for all `t ≠ s`
because `dist[s] = 0` is pre-set and the `t == s` guard skips it. There is no way for
`dist[t] == 0` to pass the `t != s && dist[t] != u32::MAX` filter. Do NOT add a
`if dist[t] == 0 { continue }` guard — it is redundant noise.

**Source nodes score HIGHER than chain intermediates (opposite of closeness)**:
- Closeness: CC[v] = N_reach × 1e6 / Σd — chain tail is best (shortest total dist)
- Harmonic: HC[v] = Σ (1e6/d) — chain HEAD is best (accumulates 1/1 + 1/2 + 1/3 + ...)
- Path A→B→C: HC[A]=1_500_000 > HC[B]=1_000_000 (A gains 1/2 from C that B doesn't have)

**Disconnected graphs are handled automatically**: unreachable nodes (dist[t] == u32::MAX)
are skipped by the same guard as Wiener index — no normalization factor needed.
This is the core advantage of harmonic over closeness centrality.

**Integer formula for harmonic**: each 1/d term is `1_000_000 / dist[t]` using integer
(truncated) division. Example: 1e6/3 = 333_333, not 333_334. Test values must use the
same truncated integers:
- HC[A] for 5-chain: 1e6/1 + 1e6/2 + 1e6/3 + 1e6/4 = 1_000_000 + 500_000 + 333_333 + 250_000 = 2_083_333

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_harmonic_inner<N>()` (V2.71)
- Public wrapper: `graph_harmonic<N>() -> ([VectorAddress; N], [u32; N], usize)`
- Shell: "graph harmonic" / "gharm"
- VectorAddress L4=47 reserved for gos-graph-harmonic-harness test nodes
- Complexity: O(V × (V + E)) — identical to Wiener index and closeness centrality
- Uses `&self` (not topology_snapshot) — consistent with Wiener, girth, clustering

## Key differences from related metrics

| Metric | Formula | Chain winner | Disconnected |
|--------|---------|-------------|--------------|
| Closeness (V2.40) | N_reach × 1e6 / Σd | tail node | needs N_reach normalization |
| Harmonic (V2.71) | Σ (1e6/d) | source/head node | automatic (0 contribution) |
| Wiener (V2.70) | Σ d (global) | N/A | excluded pairs |

## From this session

V2.71: implemented `graph_harmonic_inner` following this pattern. All 10 harness tests
passed on first compile. Key correctness tests:
- Test 4 (path A→B→C): HC[A]=1_500_000 > HC[B]=1_000_000 — source wins over intermediate ✓
- Test 7 (diamond A→{B,C}→D): HC[A]=2_500_000 — reaches B,C at d=1 and D at d=2 ✓
- Test 8 (5-chain): HC[A]=2_083_333 (sum of 1e6/{1,2,3,4}) — full arithmetic verified ✓
- Test 10 (self-loop + B→C): HC[A]=0 — self-loop auto-excluded, same as Wiener ✓
