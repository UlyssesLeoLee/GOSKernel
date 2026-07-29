---
name: gos-randomized-algorithm-invariant-testing
description: When testing a randomized algorithm (random walk, PRNG-driven sampling) in a GOSKernel harness, design all tests around provably-correct mathematical invariants that hold for any seed — never assert exact node visit counts or exact PRNG outputs. Apply whenever writing a harness for any gos-runtime function that uses a PRNG.
---

# Harness Testing for Randomized Algorithms: Assert Invariants, Not PRNG Outputs

## The rule

For randomized algorithms, structure every test around **seed-independent mathematical properties**:

```rust
// WRONG — breaks on any PRNG change or seed adjustment
assert_eq!(visits[0], 7, "node A should get 7 visits");

// CORRECT — holds regardless of PRNG sequence
let sum: u32 = visits.iter().take(n).sum();
assert_eq!(sum, 1 + actual + stuck,    "sum invariant");
assert_eq!(actual + stuck, steps,       "step accounting");
assert!(visits[0] >= visits[1],         "sorted descending");
assert_eq!(visits[0], steps + 1,        "self-loop: all visits on one node");
```

**Invariants that are always testable without knowing PRNG output:**
- `sum(visits) == 1 + actual_steps + stuck_steps`
- `actual_steps + stuck_steps == min(steps, clamp_limit)`
- `visits[i] >= visits[i+1]` (sorted output)
- `actual == 0` when the graph has no edges (structural constraint, not PRNG)
- `stuck == 0` when every node has at least one outgoing edge (structural)
- For self-loops: `visits[0] == steps + 1` and `stuck == 0` (deterministic regardless of PRNG)
- `node_count == number_of_registered_nodes` (pure registration count)

## Why it's non-obvious

The xorshift PRNG with a fixed seed IS deterministic, so you CAN write exact-value tests and they will pass locally. But they become brittle if: (a) the seed generation formula changes, (b) the xorshift variant changes, (c) the PRNG is called additional times for a new feature, or (d) the test environment changes graph_epoch (which feeds the seed). Invariant tests survive all of these changes.

The key insight: a random walk produces non-deterministic *per-node* distributions but *deterministic aggregate accounting*. The sum of all visits is always exactly 1 + N_steps because each step (or start) adds exactly one visit somewhere. This is provably correct regardless of which node gets the visits.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_sim_inner`: invariant `sum(visits) = 1 + actual + stuck`
- `host-tests/gos-graph-sim-harness/tests/graph_sim.rs` — 10 tests, all invariant-based
- The single-node self-loop case is the only case where you CAN assert `visits[0] == steps+1` since there's only one possible node to visit

## Structural tests that are always deterministic

Even with PRNG, certain tests have known-exact outputs because the graph structure forces them:
1. **Empty graph** → all zeros (0 nodes = 0 visits)
2. **steps=0** → all zeros (early return path, no PRNG involved)
3. **Single node, no edges** → `stuck=steps, actual=0, visits[0]=steps+1` (only one teleport destination)
4. **Single node, self-loop** → `stuck=0, actual=steps, visits[0]=steps+1` (only one edge to follow)

These structural-determinism cases are the best test anchors — they verify the PRNG path indirectly by checking the accounting, without depending on which node was chosen.

## From this session

V2.52 `graph sim` harness: initial draft included `assert_eq!(visits[0], 14)` for a 3-node DAG test. Replaced with `assert_eq!(sum, 1 + actual + stuck)` because the exact distribution between nodes depends on which node the xorshift starts at, and that changes based on graph_epoch at test time. All 10 final tests pass with invariant-only assertions.
