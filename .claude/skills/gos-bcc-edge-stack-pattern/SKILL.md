---
name: gos-bcc-edge-stack-pattern
description: When implementing iterative Tarjan BCC in GOSKernel, use slot-based parent tracking (not edge-index), push back-edges only toward ancestors (disc[nbr] < disc[cur]), and detect APs via bcc_mult when a node gets a second distinct BCC id.
---

# Iterative Tarjan Edge-Stack BCC — Three Non-Obvious Design Choices

## The rule

Three design decisions must all be correct for BCC to produce valid output:

**1. Slot-based parent tracking** (not edge-index like `graph_bridges`):
```rust
// par[ci] = parent compact-index (NIL = root)
if nbr_ci == par[cur_ci] { continue; }  // skip tree-parent edge
```
Do NOT track parent by edge-index. Slot-based skipping treats both A→B and B→A as "the same
parent slot," which is correct for undirected BCC (both anti-parallel arcs are one edge).
Edge-index tracking would push both directions independently, inflating edge-stack membership.

**2. Back-edge push guard — ancestor direction only**:
```rust
if disc[nbr_ci] < disc[cur_ci] && esp < MAX_EDGES {
    edge_stk[esp] = (cur_ci as u8, nbr_ci as u8);
    esp += 1;
}
```
Only push when `disc[nbr] < disc[cur]` (nbr is a proper ancestor). This ensures each
undirected back-edge is pushed exactly once, from the deeper node toward its ancestor.
Without this guard, the BCC edge stack gets duplicate entries for each back-edge.

**3. AP detection via bcc_mult flag**:
```rust
// On assigning BCC bid to endpoint ea:
if bcc_primary[ea] == 255 {
    bcc_primary[ea] = bid;
} else if bcc_primary[ea] != bid {
    bcc_mult[ea] = true;  // appears in 2+ BCCs → articulation point
}
// Output:
bcc_id = if bcc_mult[ci] { 255 } else { bcc_primary[ci] };
```
A node is an articulation point iff it appears as an endpoint in two or more distinct BCCs.
The `bcc_mult` flag records this without needing a set or count array.

## Why it's non-obvious

- `graph_bridges` uses **edge-index** parent tracking because bridge detection needs to
  distinguish which specific edge led to the parent (not just "which parent node"). BCC needs
  vertex-level parent tracking. The two algorithms look similar but require different parent
  semantics.
- Without the ancestor-direction guard on back-edges, undirected back-edges A↔B would be pushed
  twice (once as A→B, once as B→A), causing incorrect BCC membership and inflated BCC counts.
- bcc_id=255 is the sentinel for articulation points in the public API. Nodes flagged bcc_mult=true
  correspond exactly to the nodes returned by `graph_articulation` (V2.85) — verified by
  cross-check in test 10.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_bcc_inner<N>` and `pub fn graph_bcc<N>`
- `crates/k-shell/src/lib.rs` — `dispatch_graph_bcc`, 6-colour cycling, bcc_id=255 → bright-red AP
- `host-tests/gos-graph-bcc-harness/tests/graph_bcc.rs` — 10 tests covering empty, path, K3,
  K4, hourglass, star, and cross-check vs `graph_articulation`
- Contrast with `graph_bridges` (edge-index parent) and `graph_articulation` (slot-based parent —
  same style as BCC)

## From this session

V3.05 (2026-07-06). Initial implementation had slot-based parent but no back-edge guard and
no bcc_mult AP detection. Manual trace of K3, path A-B-C, and hourglass uncovered the guard
requirement and the double-push bug. The bcc_mult approach was chosen over a `bcc_count_per_node`
array to stay within the no_std fixed-array budget.
