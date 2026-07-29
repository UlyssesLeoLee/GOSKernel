---
name: gos-bk-clique-iterative-pattern
description: When implementing iterative Bron-Kerbosch maximum clique in GOSKernel, each BkFrame stores (r, p, x, to_try, came_from_v: u8) where 0xFF is the root sentinel; update the parent's p and x AFTER popping a child frame (not before pushing), using came_from_v to know which vertex to remove/add.
---

# Iterative Bron-Kerbosch Maximum Clique Pattern

## The rule

The iterative BK with Tomita pivot uses a fixed-size stack of `BkFrame` structs. Each frame stores:
- `r: u128` — partial clique bitmask
- `p: u128` — working candidates (decrements as vertices are processed)
- `x: u128` — excluded (increments as vertices are processed)
- `to_try: u128` — P \ N(pivot) for this level, vertices yet to branch on
- `came_from_v: u8` — node-index that created this frame; **0xFF = root (no parent)**

Critical ordering rules:
1. Remove `v` from `to_try` **before** pushing the child frame.
2. Update parent's `p` (remove v) and `x` (add v) **when popping the child** (not when pushing it).
3. When popping: check `came_from_v != 0xFF` before using it as a shift index.
4. When `p == 0`: always pop (report clique iff `x == 0`), then update parent.
5. When `to_try == 0` (but `p != 0`): pop and update parent — BK correctness guarantees those P vertices are covered by the pivot.

```rust
const BK_MAX: usize = 128;

#[derive(Copy, Clone)]
struct BkFrame { r: u128, p: u128, x: u128, to_try: u128, came_from_v: u8 }

// Pop logic (same for both p==0 and to_try==0 cases):
depth -= 1;
if depth > 0 && stk[fi].came_from_v != 0xFF {
    let v   = stk[fi].came_from_v as usize;
    let pfi = depth - 1;
    stk[pfi].p &= !(1u128 << v);
    stk[pfi].x |=   1u128 << v;
}
```

## Why it's non-obvious

The critical confusion is **when** to update parent's p/x:
- In recursive BK: update happens AFTER the recursive call returns.
- In iterative BK: the equivalent is update WHEN POPPING the child frame back to the parent.
- If you try to update parent's p/x when PUSHING the child, you corrupt the child's new_p/new_x computation (which needs the pre-update parent values).

Also: `came_from_v = 0xFF` serves as the root sentinel. Since node indices are 0..127 (MAX_NODES=128), 0xFF=255 is always out of range and safe as sentinel. Without the sentinel check, the root frame's pop would incorrectly update a nonexistent "parent" frame.

The `to_try == 0` with `p != 0` case: it's tempting to report non-maximal cliques or skip the parent update. Neither is correct — all needed branches were already explored (BK completeness via pivot), and the parent MUST be updated with the vertex v that triggered this frame.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_clique_inner<const N: usize>` (V2.95)
- `host-tests/gos-graph-clique-harness/tests/graph_clique.rs` — 10 tests

## From this session

V2.95: `graph_clique_inner` implemented with the above pattern. Traced through K3 (triangle) to verify:
- Root frame: p=0b111, x=0, to_try={0} (after Tomita pivot selects node 0)
- Branches on v=0, then v=1, then v=2 → finds {A,B,C} as maximum clique ✓
- Pop chain correctly updates grandparent frames via came_from_v ✓

Stack memory: BkFrame ≈ 65 bytes; 128 frames ≈ 8 KB — within kernel stack limits.
