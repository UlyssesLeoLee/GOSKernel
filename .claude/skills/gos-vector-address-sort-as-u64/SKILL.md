---
name: gos-vector-address-sort-as-u64
description: VectorAddress has no Ord/PartialOrd impl — to sort nodes deterministically by address in GOSKernel, use `VectorAddress::as_u64()` as the sort key, which encodes l4→l3→l2→offset into a comparable u64. Apply in any runtime function that needs a stable output order by VectorAddress.
---

# VectorAddress Ordering via `as_u64()`

## The rule

VectorAddress does **not** derive `Ord` or `PartialOrd`. Use `as_u64()` as the sort key:

```rust
// Insertion-sort ascending by VectorAddress (l4→l3→l2→offset natural order).
for i in 1..count {
    let key = slots[i];
    let key_addr = self.nodes[key]
        .map(|r| r.vector.as_u64())
        .unwrap_or(0);
    let mut j = i;
    while j > 0 {
        let prev = slots[j - 1];
        let prev_addr = self.nodes[prev]
            .map(|r| r.vector.as_u64())
            .unwrap_or(0);
        if prev_addr <= key_addr { break; }
        slots[j] = slots[j - 1];
        j -= 1;
    }
    slots[j] = key;
}
```

`as_u64()` encoding: `KERNEL_BASE | (l4 << 36) | (l3 << 24) | (l2 << 12) | offset`

This gives correct lexicographic order: l4 is the most significant, offset is least significant.

## Why it's non-obvious

`VectorAddress` is `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]` — it has `Eq` but NOT
`Ord`. There is no `.cmp()` method, no tuple destructuring comparison, and no `<` operator.
The four fields (l4: u8, l3: u16, l2: u16, offset: u16) could be compared with a 4-field
tuple `(l4, l3, l2, offset)`, but since `Ord` is not derived you cannot call `.cmp()` on
VectorAddress directly.

The `as_u64()` approach works because the bit encoding places l4 in the highest bits,
so integer comparison of the u64 values exactly mirrors lexicographic (l4, l3, l2, offset)
ordering with no further effort.

Do not try `(a.l4, a.l3, a.l2, a.offset).cmp(...)` inside a no_std context — that would
require a local struct to be created. The `as_u64()` approach is both concise and
no_std safe (it only uses arithmetic).

## GOSKernel context

- VectorAddress defined in `crates/gos-protocol/src/lib.rs`
- `as_u64()` encodes: `KERNEL_BASE | (l4 << 36) | (l3 << 24) | (l2 << 12) | offset`
- Used in `graph_peripheral_inner` (V2.72) for deterministic peripheral node output
- Alternative (other functions): some functions sort by score descending (e.g. harmonic, katz)
  and accept any stable tie-breaking — those use insertion sort by score, not by address
- When address order matters for test determinism (e.g. "node at index 0 must be the lowest address"),
  always use `as_u64()` sort

## From this session

V2.72 peripheral nodes: needed deterministic output order across test runs. The `find_peripheral`
test helper uses linear search (order-independent), but the `vecs[0]` index check would be
fragile without a defined order. Using `as_u64()` sort makes output stable and test-friendly.
