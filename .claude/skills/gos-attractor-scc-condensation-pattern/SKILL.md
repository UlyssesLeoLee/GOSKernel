---
name: gos-attractor-scc-condensation-pattern
description: When implementing graph attractor classification (bottom SCC detection) in GOSKernel, self-loops and intra-SCC edges must be excluded from the condensation edge scan — otherwise self-loop nodes incorrectly appear as non-attractors. Use two separate edge-scan passes (one for scc_has_out, one for scc_adj_attract) rather than combining them, to avoid reading attractor status before all SCCs have been classified. Apply whenever implementing or reviewing graph_attractor_inner in crates/gos-runtime/src/lib.rs.
---

# Graph Attractor: SCC + Condensation Classification Pattern

## The rule

The attractor classification algorithm has three separate phases after Kosaraju SCC:

**Phase 3a — find SCCs with outgoing condensation edges:**
```rust
// SKIP both self-loops and intra-SCC edges
if from_slot == to_slot { continue; }       // self-loop guard
let sf = scc_id[from_slot];
let st = scc_id[to_slot];
if sf == UNSET || st == UNSET || sf == st { continue; }  // intra-SCC guard
scc_has_out[sf as usize] = true;
```

**Phase 3b — find SCCs directly adjacent to attractor SCCs (drains):**
```rust
// Run as a SEPARATE second scan — not combined with 3a
if from_slot == to_slot { continue; }
let sf = scc_id[from_slot];
let st = scc_id[to_slot];
if sf == UNSET || st == UNSET || sf == st { continue; }
if !scc_has_out[st as usize] {   // destination SCC is an attractor
    scc_adj_attract[sf as usize] = true;
}
```

**Role assignment:**
```rust
let node_role: u8 = if !scc_has_out[sci] {
    0 // attractor: no condensation out-edges
} else if scc_adj_attract[sci] {
    1 // drain: direct condensation edge to an attractor SCC
} else {
    2 // transient: out-edges exist but none to attractors
};
```

## Why it's non-obvious

### Self-loops must NOT count as condensation edges

A self-loop `A→A` means `from_slot == to_slot`. If this check is absent from
Phase 3a, `scc_has_out[scc_id[A]]` would be set to `true`, making A appear as a
non-attractor. But a self-loop creates no condensation edge at all — the node A
remains in a trivial singleton SCC with no edges leaving its SCC.

Correct: `{from_slot == to_slot → continue}` guards both Phase 3a AND 3b.
The same guard already exists in the Kosaraju DFS phases for the same reason.

### Two-pass scan is required, not one

If you try to combine Phase 3a and 3b into a single scan:
```rust
// WRONG: scc_has_out not yet complete when computing scc_adj_attract
for ei in 0..MAX_EDGES {
    scc_has_out[sf] = true;
    if !scc_has_out[st] { scc_adj_attract[sf] = true; }  // reads incomplete scc_has_out!
}
```
This fails because `scc_has_out[st]` may not yet be set when the edge `sf→st` is
processed. You would falsely classify a non-attractor SCC as an attractor
(if its own out-edges haven't been scanned yet), causing incorrect drain labeling.

### The `sf == st` guard is also required in the condensation scan

Even for inter-slot edges that don't self-loop, an edge within the same SCC
(e.g. A→B where both are in SCC 0) must be skipped in the condensation scan.
Only cross-SCC edges (`sf != st`) contribute to the condensation.

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_attractor_inner<N>` (V2.54)
- Public wrapper: `pub fn graph_attractor<N>()` (same lock-and-call pattern as `graph_centrality`, `graph_between`)
- Uses `&self` (not snapshot) — O(V+E) is fast enough to hold RUNTIME lock
- VectorAddress namespace for harness: L4=31
- Shell aliases: `graph attractor` / `attractor` / `gattractor` / `graph attract` / `attract`

## From this session

V2.54 `graph attractor`: Test 10 (`self_loop_and_isolated_both_attractor`) specifically
verifies that a node with only a self-loop `A→A` is correctly classified as role=0
(attractor), not role=1 (drain) or role=2 (transient). Without the `from_slot == to_slot`
guard in Phase 3a, the self-loop would have set `scc_has_out[A's SCC] = true`, making
A appear as a non-attractor. All 10 harness tests passed on first compile.
