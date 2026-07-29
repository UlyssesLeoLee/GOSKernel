---
name: gos-node-prop-zero-value-sentinel
description: In gos-runtime's property tables (node_props_u8, node_props_u32), the free-slot sentinel is NodeId::ZERO (the NodeId), not the value 0. Storing value=0 is perfectly valid and returns Some(0) from the getter — it is NOT the same as "no attribute set" (None). Apply when writing or testing node_attr_set/get or any future register_node_prop_* function.
---

# node_props_u32: NodeId::ZERO is the sentinel, not value 0

## The rule

The property tables use `NodeId::ZERO` as the "free slot" marker — **not** value `0u32`:

```rust
// Free slot: (NodeId::ZERO, any_value)
// Occupied:  (some_node_id, val)   ← val may legally be 0

// node_attr_set(vec, 0u32) → Ok(())
// node_attr_get(vec)       → Some(0u32)   ← NOT None
```

A node with attribute value `0u32` (e.g. palette color black `0x00_00_00_00`) is distinct from a node that has never had an attribute set (returns `None`). Test 7 in gos-node-attr-harness explicitly verifies this.

## Why it's non-obvious

The value 0 is a common "empty" sentinel in many systems, so it's tempting to assume `node_attr_get` returns `None` when the stored value is 0. In GOSKernel's property tables the slot occupancy is tracked by the *key* (`NodeId::ZERO` = free), not by the *value*. This distinction matters for palette colors — `0x00000000` (transparent black) is a valid color, not "no color assigned".

## GOSKernel context

- `crates/gos-runtime/src/lib.rs`: `node_props_u8` and `node_props_u32` both follow this pattern
- `node_prop_u8` / `node_prop_u32` scan with: `if id == node_id && id != NodeId::ZERO`
- Reset (`GraphRuntime::new()`) zeroes the array: `[(NodeId::ZERO, 0u32); ...]` — all slots free

## From this session

V2.55 test 7 (`attr_set_zero_roundtrips`) confirms `node_attr_set(vec, 0u32)` + `node_attr_get(vec)` → `Some(0u32)`. The comment in the test clarifies: "zero value must round-trip exactly (distinct from 'no attr set')".
