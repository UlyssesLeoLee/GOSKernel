---
name: gos-konig-vertex-cover-pattern
description: When building minimum vertex cover from a bipartite max matching in no_std Rust, use alternating-path BFS (unmatched A→B, matched B→A) from unmatched A-nodes to find Z, then cover = (A \ Z_A) ∪ Z_B — skipping match_a[a]==b to avoid following the matched edge from A.
---

# König's Min Vertex Cover Construction Pattern

## The rule

After running Kuhn's max bipartite matching (arrays `match_a[]`, `match_b[]`), find the minimum vertex cover via König's construction:

```rust
// BFS from all unmatched A-nodes
let mut in_z = [false; MAX_NODES];
let mut q = [0usize; MAX_NODES]; let mut qh = 0; let mut qt = 0;

for ki in 0..node_count {
    let s = node_slots[ki];
    if slot_color[s] == 0 && match_a[s] == NIL {  // unmatched A-node
        in_z[s] = true; q[qt] = s; qt += 1;
    }
}
while qh < qt {
    let cur = q[qh]; qh += 1;
    let cur_id = self.nodes[cur].map(|r| r.spec.node_id).unwrap_or(...);
    if slot_color[cur] == 0 {
        // A-node: follow UNMATCHED edges to B (skip b == match_a[cur])
        for ei in 0..MAX_EDGES {
            // ... find b_slot (B-side neighbor) ...
            if match_a[cur] == b_slot { continue; }  // ← KEY: skip matched edge
            if !in_z[b_slot] { in_z[b_slot] = true; q[qt] = b_slot; qt += 1; }
        }
    } else {
        // B-node: follow MATCHED edge back to A
        let a = match_b[cur];
        if a != NIL && !in_z[a] { in_z[a] = true; q[qt] = a; qt += 1; }
    }
}
// Cover = (A not in Z) + (B in Z)
for ki in 0..node_count {
    let s = node_slots[ki];
    let in_cover = if slot_color[s] == 0 { !in_z[s] } else { in_z[s] };
    if in_cover { /* add s to cover */ }
}
```

## Why it's non-obvious

Three tricky details that are easy to get wrong:

1. **Skip `match_a[cur] == b_slot`** — From an A-node in Z, you want to follow UNMATCHED edges to B. The matched edge `a→match_a[a]` must be excluded. If you forget this skip, you'll traverse matched edges from A to B (contradicting the alternating-path rule) and over-extend Z, causing the cover to be too small (missing covered edges).

2. **Isolated/unmatched A-nodes start in Z** — A node with no edges (color=0, match_a=NIL) goes into Z_A, so it's excluded from the cover. This is correct: it has no incident edges to cover.

3. **Cover formula is asymmetric**: Cover = (A \ Z_A) ∪ Z_B — NOT (A ∩ Z_A) or (B \ Z_B). This is counter-intuitive: you take the A-nodes NOT reachable from unmatched A-nodes, combined with the B-nodes that ARE reachable.

**Proof sketch:** For any edge (a, b):
- Case b ∉ Z_B: b was never reachable → if a were also not in cover (a ∈ Z_A), then the alternating path to a would have extended to b (contradiction). So a ∈ cover.
- Case b ∈ Z_B: b ∈ cover directly.
Thus every edge is covered.

## Key invariants

- `|cover| = |max matching|` — König's theorem: τ(G) = ν(G) for bipartite G.
- `α(G) + τ(G) = n` — Gallai: independence number + cover size = node count.
- Cross-check: `cover_size == match_count` AND `cover_size + is_size == node_count`.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_vertex_cover_inner` (V2.97)
- Uses same Kuhn matching arrays as `graph_bipartite_match_inner` (V2.92)
- `match_a[s] == NIL` sentinel for unmatched nodes (NIL = usize::MAX)
- Self-loops excluded: `if b == cur { continue; }` in the BFS edge scan

## From this session

V2.97 implemented `graph_vertex_cover`. The key bug to avoid: not skipping `match_a[cur] == b_slot` would follow the matching edge as an "unmatched" edge, over-extending Z and producing a cover smaller than τ(G) that misses some edges. All 10 harness tests passed including the König cross-check (test 10: τ = ν = 3 for K_{3,3}) and Gallai cross-check (test 9: α+τ=n=4 for P4).
