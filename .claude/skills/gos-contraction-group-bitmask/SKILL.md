---
name: gos-contraction-group-bitmask
description: When implementing graph contraction algorithms (Stoer-Wagner min-cut, Chu-Liu/Edmonds arborescence) in GOSKernel, track super-node membership via `group_mbrs[si]: u128` bitmask — initialized to `1u128 << ci` and OR'd at each merge — so the optimal partition can be recovered in O(N) from `best_b_mask` after all phases complete.
---

# Graph Contraction: u128 Group-Membership Bitmask

## The rule

In any algorithm that contracts nodes into super-nodes over multiple phases, maintain:
```rust
let mut group_mbrs = [0u128; MAX_NODES];
for i in 0..nc { group_mbrs[i] = 1u128 << i; }
```
On each merge of `last_t` into `last_s`:
```rust
group_mbrs[last_s] |= group_mbrs[last_t];
```
When you find a new optimum at a phase:
```rust
best_b_mask = group_mbrs[last_t]; // BEFORE the merge
```
Recovery: `(best_b_mask >> ci) & 1 == 1` iff original node `ci` is on side B.

## Why it's non-obvious

Without the bitmask, recovering the partition after all contractions requires either (a) storing the full partition state at every phase (expensive), or (b) re-running the algorithm to identify which phase produced the minimum (wasteful). The u128 bitmask gives O(1) membership check per node and O(N) recovery, and for N≤128 it never overflows.

The critical ordering rule: capture `best_b_mask = group_mbrs[last_t]` **before** the merge that sets `group_mbrs[last_s] |= group_mbrs[last_t]` and deactivates `last_t`. After the merge, `group_mbrs[last_t]` is still correct (we don't zero it), but if you write this after the merge it still works — the deactivation only sets `node_active[last_t] = false`, not the bitmask.

## GOSKernel context

Used in `graph_min_cut_inner<N>` (V3.02) for Stoer-Wagner partition recovery. The same principle applies in any future contraction-based algorithm (Borůvka MST, Karger-Stein min-cut, etc.).

## From this session

V3.02: without the bitmask, recovering the optimal B-side partition after V-1 phases would require storing `nc` booleans per phase = up to 128×128 = 16KB. The bitmask reduces this to 128×16 = 2KB total.
