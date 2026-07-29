---
name: gos-edge-cross-reference-scan
description: When implementing a graph metric that needs to check pairs of edges (e.g. "does the reverse of edge (u,v) also exist?"), pre-collect all (from, to) pairs into fixed-size arrays, then do a nested index loop — do NOT try to nest two self.edges.iter() calls. Apply in crates/gos-runtime/src/lib.rs for any cross-edge reference computation.
---

# Edge Cross-Reference Scan: Pre-Collect then Index

## The rule

For any metric that checks cross-edge relationships (like reciprocity, symmetry checking, or mutual-edge counting), pre-collect all (from, to) pairs into flat arrays first, then do a nested index scan:

```rust
// Pre-collect phase
let mut from_ids = [NodeId::ZERO; MAX_EDGES];
let mut to_ids   = [NodeId::ZERO; MAX_EDGES];
let mut m = 0usize;
for edge in self.edges.iter().flatten() {
    let u = edge.spec.from_node;
    let v = edge.spec.to_node;
    if u == v { continue; }  // always exclude self-loops
    if m < MAX_EDGES {
        from_ids[m] = u;
        to_ids[m]   = v;
        m += 1;
    }
}

// Cross-reference scan phase — nested index loops, not nested iterators
let mut mutual = 0usize;
for i in 0..m {
    let u = from_ids[i];
    let v = to_ids[i];
    for j in 0..m {
        if from_ids[j] == v && to_ids[j] == u {
            mutual += 1;
            break;
        }
    }
}
```

**Complexity:** O(M²). At MAX_EDGES=512 this is at most 262_144 comparisons per call — acceptable for kernel analytics.

## Why it's non-obvious

You cannot write `for edge_a in self.edges.iter() { for edge_b in self.edges.iter() {...} }` while inside an `impl GraphRuntime` method because you'd be borrowing `self.edges` twice. The flat array snapshot decouples the data from the borrow checker, enabling the nested loop.

Also: self-loops (u==v) must be excluded in the pre-collect phase, not in the cross-reference scan. If you include self-loops and check for their "reverse", every self-loop satisfies `from_ids[j]==v && to_ids[j]==u` trivially — inflating the mutual count.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — used in `graph_reciprocity_inner()` (V2.66)
- Array sizes: `[NodeId::ZERO; MAX_EDGES]` = 512 entries × 8 bytes = 4 KB stack per array (two arrays = 8 KB)
- Always guard `if m < MAX_EDGES` before writing to prevent buffer overrun on dense graphs
- This is a per-call (`&self`) method, NOT a `GraphTopologySnapshot` function (which is for long-running O(V×E) analytics where IRQ deadlock is a concern)

## From this session

V2.66: implementing graph reciprocity required checking whether each edge (u,v) has a corresponding reverse (v,u). The natural nested-iterator approach is blocked by Rust borrow rules. The pre-collect pattern (used here for the first time in GOSKernel graph metrics) cleanly solves this. All 10 reciprocity tests pass.

Contrast with the neighbor-union pattern (gos-clustering-undirected-neighbors): that pattern uses a per-node `seen[]` dedup array while iterating edges once per node. Cross-reference scan is different — it compares edges against each other, not edges against a per-node set.
