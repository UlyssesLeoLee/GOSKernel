---
name: gos-eulerian-active-node-pattern
description: When implementing Eulerian path/circuit detection in GOSKernel, collect "active nodes" (out+in > 0) first and exclude isolated nodes from BOTH the degree-balance check AND the undirected BFS — isolated nodes trivially satisfy in==out=0 but mustn't be counted as connected. Also: no-edge graph returns has_circuit=true (vacuous), not false.
---

# Eulerian Detection: Active-Node Separation + Vacuous Circuit

## The rule

Eulerian detection has three distinct passes, each operating on **active nodes only**:

```rust
// Step 1 — degree census (all edges)
let mut out_deg = [0u16; MAX_NODES];
let mut in_deg  = [0u16; MAX_NODES];
for ei in 0..MAX_EDGES { /* scan edges, accumulate per-slot */ }

// Step 2 — separate active (non-isolated) nodes
let mut active_slots = [0usize; MAX_NODES];
let mut active_count = 0usize;
for ki in 0..node_count {
    let s = node_slots[ki];
    if out_deg[s] > 0 || in_deg[s] > 0 {       // ← active = has ≥1 edge
        active_slots[active_count] = s;
        active_count += 1;
    }
}

// Step 3 — vacuous case
if active_count == 0 { return (true, false, zero, zero, node_count); }

// Step 4 — degree-balance check on active_slots only
// Step 5 — BFS connectivity check seeded from active_slots[0], visiting only via edges
```

**Degree balance dispatch (i32 diff):**
```rust
let diff = (out_deg[s] as i32) - (in_deg[s] as i32);
match diff {
    0  => {}        // balanced
    1  => { if start_slot == MAX_NODES { start_slot = s; } else { path_possible = false; } imbalanced += 1; }
    -1 => { if end_slot   == MAX_NODES { end_slot   = s; } else { path_possible = false; } imbalanced += 1; }
    _  => { path_possible = false; imbalanced += 1; }
}
```

The sentinel guard (`start_slot == MAX_NODES` check before assigning) is what catches the
"two start candidates → neither" case (test 10).

**Connectivity BFS — undirected, active-only:**
```rust
// Follow both from_node and to_node directions (undirected projection)
let nbr_id = if edge.spec.from_node == cur_id { edge.spec.to_node }
             else if edge.spec.to_node == cur_id { edge.spec.from_node }
             else { continue };
// BFS visits all reachable slots; verify all active_slots[0..active_count] are visited
```

## Why it's non-obvious

**1. Isolated nodes must be excluded from both checks.**

An isolated node has `out=0, in=0`. If you run the degree check over all live nodes,
isolated nodes trivially satisfy `in == out` (both zero) — they appear "balanced" for a
circuit. But if the rest of the graph is disconnected from them, that should block the
circuit. The fix is to exclude isolated nodes from the active set entirely: the connectivity
BFS is seeded from `active_slots[0]` (first node with an edge), and only active nodes are
checked for reachability. Isolated nodes simply don't participate.

**2. No-edge graph → has_circuit=true (not false).**

If `active_count == 0` (no edges at all, possibly with isolated nodes), the correct result
is `has_circuit=true, has_path=false`. This matches standard graph theory (vacuous universal
quantifier: ∀v: in==out holds over zero active nodes) and NetworkX's `is_eulerian(empty)`.
It is surprising because it feels like "there's nothing to traverse" should mean "false".
The key insight: an Eulerian circuit is a closed walk over all edges; the empty walk trivially
satisfies this when no edges exist.

**3. Two start/two end candidates → neither, not path.**

If two nodes have `out-in=+1`, the path condition fails. The sentinel-slot guard (check if
`start_slot == MAX_NODES` before assigning, otherwise set `path_possible = false`) catches
this in a single linear pass without needing a counter.

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_eulerian_inner()` (V2.87)
- Public wrapper: `graph_eulerian() -> (bool, bool, VectorAddress, VectorAddress, usize)`
- Shell: `graph eulerian` / `geulerian` / `eulerian` / `euler`
- VectorAddress L4=63 for `gos-graph-eulerian-harness`
- Returns `(has_circuit, has_path, start_vec, end_vec, node_count)`
- `start_vec` / `end_vec` are `VectorAddress::new(0,0,0,0)` when `has_circuit=true` or neither
- Mutually exclusive: `has_circuit=true` implies `has_path=false`

## From this session

V2.87 first-attempt: all 10 tests passed on first cargo test run. The three non-obvious
invariants (active-node separation, vacuous circuit, two-candidate guard) were baked in
from design rather than discovered through failures, informed by the earlier
`gos-tarjan-articulation-iterative` and `gos-tarjan-bridge-edge-index` pattern experience.
Test 7 (disconnected A→B, C→D) locks in the connectivity requirement; test 10 (hub→A +
hub→B + C→hub) locks in the two-start-candidate guard.
