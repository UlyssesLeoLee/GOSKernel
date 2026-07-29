---
name: gos-greedy-coverage-bitmask-pattern
description: When implementing greedy set-cover-style graph algorithms (dominating set, set cover, facility location) in no_std GOSKernel, encode each node's coverage neighbourhood as a u128 bitmask over compact indices and greedily pick by count_ones() — isolated nodes are forced into D automatically with no special case.
---

# Greedy Coverage Bitmask Pattern

## The rule

For greedy set-cover-style problems on ≤128 nodes:

```rust
// 1. Build compact index mapping
let mut slot_to_ci = [usize::MAX; MAX_NODES];
for ci in 0..node_count { slot_to_ci[node_slots[ci]] = ci; }

// 2. Build coverage bitmask: covered[ci] = {ci} ∪ undirected-neighbours
let mut covered = [0u128; 128];
for ci in 0..node_count { covered[ci] |= 1u128 << ci; } // self
for ei in 0..MAX_EDGES {
    let edge = match self.edges[ei] { Some(e) => e, None => continue };
    if edge.spec.from_node == edge.spec.to_node { continue; } // self-loop
    let fci = slot_to_ci[fs]; let tci = slot_to_ci[ts];
    covered[fci] |= 1u128 << tci;
    covered[tci] |= 1u128 << fci; // undirected: symmetric
}

// 3. Greedy loop: always pick max count_ones() coverage
let all_mask: u128 = if node_count >= 128 { u128::MAX } else { (1u128 << node_count) - 1 };
let mut undominated: u128 = all_mask;
let mut in_set = [false; 128];
while undominated != 0 {
    let mut best_ci = 0; let mut best = 0u32;
    for ci in 0..node_count {
        if in_set[ci] { continue; }
        let c = (covered[ci] & undominated).count_ones();
        if c > best { best = c; best_ci = ci; }
    }
    in_set[best_ci] = true;
    undominated &= !covered[best_ci];
}
```

## Why it's non-obvious

**Isolated nodes require no special case.** An isolated node v has `covered[v] = {v}` only. When v remains in `undominated`, the greedy loop must eventually pick it because:
- v is not in any other node's `covered` bitmask (no neighbours)
- Only v itself can remove v from `undominated`
- The loop terminates only when `undominated == 0`

So isolated nodes are automatically forced into the output set without any explicit check. Forgetting this and adding an `if isolated { skip }` guard would produce an incorrect (non-dominating) set.

**Guard for n ≥ 128**: `1u128 << node_count` overflows when `node_count == 128`. Use:
```rust
let all_mask: u128 = if node_count >= 128 { u128::MAX } else { (1u128 << node_count) - 1 };
```
The same guard appears in `graph_clique` and `graph_independent_set`.

**best_count starts at 0, not 1**: Because `covered[ci] & undominated` might be 0 for nodes whose entire neighbourhood is already dominated. Those nodes are never selected (coverage=0 doesn't beat best=0's initial `best_ci=0` — so initialise `best_ci=0` as fallback, which is fine since any remaining undominated node will have coverage ≥ 1 for itself).

## Approximation guarantee

The greedy algorithm achieves ≤ H(Δ)+1 ≈ ln(Δ)+1 approximation ratio where Δ = max degree. This is asymptotically optimal (no poly-time algorithm can do better unless P=NP). For sparse OS graphs (Δ << n), the approximation is tight in practice.

## Cross-validation invariant

For any graph: **γ(G) ≤ τ(G)** (dominating set ≤ vertex cover). Because every vertex cover is a dominating set (every non-isolated node has an edge whose endpoint is in the cover). Use this in test 10 to cross-check results.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_dominating_set_inner` (V2.98)
- Same compact-index + u128 bitmask technique as `graph_clique_inner` (V2.95) and `graph_independent_set_inner` (V2.96)
- Shell: "graph domset" / "gdomset" / "dominating set"
- VectorAddress L4=74 for gos-graph-domset-harness

## From this session

V2.98 implemented `graph_dominating_set`. The pattern generalises directly to any set-cover-style problem on ≤128 nodes: build per-node coverage bitmasks, track uncovered set as u128, greedily pick by `count_ones()`. All 10 harness tests passed including: isolated forced (tests 2,3,9), K_n/star optimal (tests 5,7,8), γ≤τ cross-check (test 10).
