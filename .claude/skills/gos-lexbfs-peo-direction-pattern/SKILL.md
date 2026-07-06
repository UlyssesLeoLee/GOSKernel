---
name: gos-lexbfs-peo-direction-pattern
description: In LexBFS-based chordal graph recognition, N+(v) is the set of EARLIER-numbered neighbours (pos_of < pos), not later; and w = argmax pos_of (most recently numbered), not argmin. Swapping the direction silently passes non-chordal graphs as chordal.
---

# LexBFS PEO Verification Direction

## The rule

When implementing **LexBFS chordal graph recognition** (Rose, Tarjan & Lueker 1976), the PEO
verification loop must use **earlier-numbered** neighbours, not later:

```rust
// CORRECT
let u_pos = pos_of[uci];
if u_pos != NIL && u_pos < pos {  // ← earlier (pos_of < pos)
    nplus |= 1u128 << uci;
}

// Find w = most-recently-numbered in N+(v)
if w_ci == NIL || u_pos > w_pos {  // ← argmax (largest pos_of)
    w_ci = uci;
    w_pos = u_pos;
}
```

Do **NOT** use `u_pos > pos` (later neighbours) or `u_pos < w_pos` (argmin). Both are wrong.

The full check: for each node v at PEO position `pos`:
- **N+(v)** = {u : adj(v,u) and pos_of[u] < pos}  (numbered before v)
- Let **w** = argmax_{u ∈ N+(v)} pos_of[u]  (the MOST RECENTLY numbered)
- Assert **N+(v) \ {w} ⊆ adj[w]**  (all other earlier neighbours of v are also adjacent to w)

## Why it's non-obvious

The LexBFS algorithm numbers nodes from pos=0 (first) to pos=n-1 (last). In the *original*
Rose-Tarjan-Lueker paper, nodes are numbered from n down to 1 (backwards), and the PEO condition
is defined as N+(v) = {u : σ(u) > σ(v)} = nodes with HIGHER number = numbered EARLIER. Because
my forward numbering maps pos=0 to σ=n (first), the translation is:

    N+(v) original = {u : σ(u) > σ(v)} = {u : n−pos_of[u] > n−pos_of[v]} = {u : pos_of[u] < pos}

Naively, "later in the ordering" (pos_of[u] > pos) feels like the right definition (v's
"successors"), but it's the wrong direction. Using later-numbered neighbours makes the check
trivially pass for the first-numbered node (empty N+), but fails to detect chordless cycles that
span positions in the wrong direction.

Similarly: w should be the node in N+(v) that was numbered MOST RECENTLY (largest pos_of), not
first (smallest pos_of). The Fulkerson-Gross argument is: w is the node in N+(v) that appears
latest in the PEO; if N+(v) is a clique, then w must be adjacent to all earlier members.

**Empirical failure mode:** With the wrong direction (using `pos_of > pos`), C4+chord (a chordal
graph) returns `is_chordal=false` — the test `C4+chord: chordal` assertion fires. With the wrong
argmin for w, different graphs fail. Both bugs look like "implementation error", not "direction
error", which makes them confusing to debug.

## GOSKernel context

- Implemented in `crates/gos-runtime/src/lib.rs`, method `graph_chordal_inner<const N>`
- PEO verification section: the `'peo_check` loop
- The LexBFS label update (setting bits) is independent of the PEO check direction and is correct:
  `label[nci] |= 1u128 << pos` (adjacent to node numbered at pos gets bit `pos` set)

## From this session

V3.04: Initial implementation used `u_pos > pos` (later) and `u_pos < w_pos` (argmin). Test
`test_06_c4_with_chord` failed with `"C4+chord: chordal"` assertion. Manual trace showed PEO
[A,B,C,D] for C4+chord correctly passes the check when using earlier-neighbours + argmax-w.
Fixed by swapping both conditions. All 10 tests then green.
