---
name: gos-stoer-wagner-edgelist-pattern
description: When implementing Stoer-Wagner global min-cut in GOSKernel, use an edge-list representation (uf/ut[MAX_EDGES] as u8, uw[MAX_EDGES] as u16) instead of an N×N adjacency matrix — N×N at u16 = 32KB stack overflow for N=128. After each merge of last_t into last_s, redirect endpoints, kill self-loops (na==nb), then dedup parallel edges with a nested O(E²) scan using saturating_add.
---

# Stoer-Wagner: Edge-List Instead of Adjacency Matrix

## The rule

Implement Stoer-Wagner with an edge list `(uf[MAX_EDGES], ut[MAX_EDGES], uw[MAX_EDGES], ue_live[MAX_EDGES])` using `u8` for compact-index endpoints and `u16` for weights. Never use a full `[[u16; N]; N]` adjacency matrix — at N=128 this is 32KB and will overflow the kernel stack.

After merging `last_t` into `last_s`:
1. Scan all live edges; where endpoint == `last_t`, redirect to `last_s`.
2. If both endpoints become equal after redirect: `ue_live[ei] = false` (self-loop).
3. Normalize `(na, nb)` so `na < nb` (undirected invariant preserved).
4. Dedup O(E²): for each live `i`, if any `j > i` has same `(uf[j], ut[j])`, add `uw[j]` to `uw[i]` via `saturating_add` and kill `j`.

## Why it's non-obvious

Textbook Stoer-Wagner uses a dense adjacency matrix. N=128 × u16 = 32KB, which exceeds the GOSKernel kernel-stack budget. The edge-list approach requires explicit deduplication after each merge (since contracting two nodes can create parallel edges to the same endpoint), but yields ≈8KB total stack usage and O(V²×E) complexity with MAX_EDGES=512.

## GOSKernel context

Applies in `crates/gos-runtime/src/lib.rs` `graph_min_cut_inner<N>`. The `u8` type for endpoints is safe since compact indices `ci < MAX_NODES = 128 < 256`. The `u16` weight is safe since max accumulated weight after k contractions ≤ k ≤ N-1 = 127 for unweighted graphs, but saturating_add handles any weighted accumulation.

## From this session

V3.02 implementation — using `[[u16; 128]; 128]` would be 32KB; the edge-list approach brought stack to ~8KB. The dedup step is the non-obvious required addition: without it, two distinct (last_s, u) edges remain after merge rather than one combined edge, producing incorrect cut-of-phase values.
