---
name: gos-vector-address-no-zero-const
description: VectorAddress has NO ::ZERO constant — use VectorAddress::new(0,0,0,0) instead. Unlike EdgeVector (which has EdgeVector::ZERO), VectorAddress omits this. Apply whenever initializing a [VectorAddress; N] array or writing a fallback/sentinel value in gos-runtime or any harness.
---

# VectorAddress Has No ZERO Constant

## The rule

Do NOT write `VectorAddress::ZERO` — it does not exist. Use `VectorAddress::new(0, 0, 0, 0)` everywhere a zero/sentinel VectorAddress is needed.

For array initialization: `[VectorAddress::new(0, 0, 0, 0); N]`

## Why it's non-obvious

`EdgeVector` (the parallel type for edge addresses) DOES have `EdgeVector::ZERO`. It's natural to assume `VectorAddress` has the same, but it was never added. The error (`no associated function or constant named 'ZERO' found for struct VectorAddress`) only appears at compile time — there's no static analysis warning before that.

## GOSKernel context

- `crates/gos-protocol/src/lib.rs` — both types defined here; only EdgeVector has ZERO
- Any harness using `out_vec: &mut [VectorAddress; N]` needs `[VectorAddress::new(0,0,0,0); N]`
- `crates/gos-runtime/src/lib.rs` — `node_attr_list_inner` fallback uses `VectorAddress::new(0,0,0,0)`
- `crates/k-shell/src/lib.rs` — `dispatch_node_attr_list` buffer init

## From this session

Writing `gos-node-attr-list-harness` and `dispatch_node_attr_list` both initially used `VectorAddress::ZERO`, causing compile failures. Fixed by using `VectorAddress::new(0, 0, 0, 0)` throughout.
