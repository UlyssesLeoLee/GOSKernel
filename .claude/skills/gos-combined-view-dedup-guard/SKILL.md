---
name: gos-combined-view-dedup-guard
description: When a k-shell display function merges two complementary node sets (e.g. center + peripheral) into one panel, gate the second section with `scalar_a != scalar_b` to prevent double-listing nodes that simultaneously satisfy both conditions (e.g. when radius == diameter every node appears in both sets).
---

# Combined Node-Set Display: Double-Listing Dedup Guard

## The rule

When `dispatch_graph_diameter` (or any future combined-view function) shows two
complementary node sets from two separate API calls, gate the second section:

```rust
// Show center nodes unconditionally (always listed first).
let mut i = 0;
while i < center_count {
    // ... display c_vecs[i], c_ecc[i], "center" ...
    i += 1;
}

// Only show peripheral if the two scalars differ.
// When radius == diameter every node is simultaneously center AND peripheral —
// showing both sections would list each node twice.
if radius != diameter {
    let mut j = 0;
    while j < periph_count {
        // ... display p_vecs[j], p_ecc[j], "peripheral" ...
        j += 1;
    }
}
```

The same guard belongs in any combined view where both sets are defined by
equality to a global scalar: `ecc == radius` vs `ecc == diameter`.

## Why it's non-obvious

Both center and peripheral are valid non-empty sets whenever the graph has ≥ 1
reachable pair. When the graph is symmetric (complete graph, directed cycle with
all-equal ecc), radius == diameter holds and the two API calls return overlapping
node sets. A naïve concatenated display would list every node twice — which looks
like a bug and inflates the node count in the footer.

The guard is zero-cost at runtime (one comparison) and the rule is:
**show the "tighter" set first (center = min ecc) and skip the "looser" set (peripheral
= max ecc) when the two scalars collide**.

## GOSKernel context

- `dispatch_graph_diameter` (V2.82): `crates/k-shell/src/lib.rs`
- Calls `graph_peripheral::<64>()` → `(p_vecs, p_ecc, periph_count, node_count, diameter)`
- Calls `graph_center::<64>()` → `(c_vecs, c_ecc, center_count, _, radius)`
- Guard: `if radius != diameter { /* show peripheral section */ }`
- Shell: "graph diameter" / "gdiameter"; L4=58 for test harness

## From this session

V2.82 combined-view implementation. Test 5 (bidirected triangle: radius=1, diameter=1)
and test 6 (directed cycle: radius=2, diameter=2) both exercise the equal-scalar case.
The guard was designed up-front when we noticed that in a complete graph all nodes
satisfy both `ecc==radius` and `ecc==diameter` simultaneously.
