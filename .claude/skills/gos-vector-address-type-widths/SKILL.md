---
name: gos-vector-address-type-widths
description: VectorAddress::new() has mixed-width parameters — only l4 is u8; l3, l2, and offset are u16. Passing a u8 literal or variable for l3/l2/offset causes E0308 mismatched types. Always cast with `as u16`. Apply whenever constructing VectorAddress::new() in test harnesses or any Rust code.
---

# VectorAddress::new() parameter type widths

## The rule

Always cast sub-byte components to u16 when building a VectorAddress:

```rust
// Wrong — minor is u8, l2 expects u16
VectorAddress::new(29, 1, minor, 0)

// Correct
VectorAddress::new(29, 1, minor as u16, 0)
```

The signature is: `pub const fn new(l4: u8, l3: u16, l2: u16, offset: u16) -> Self`

Only `l4` is `u8`. All three of `l3`, `l2`, `offset` are `u16`.

## Why it's non-obvious

The four components look symmetric — all small integers, usually written as small literals like `29, 1, 1, 0`. When writing test fixtures with a loop variable (`minor: u8`), it's natural to pass it directly without a cast. The compile error only appears at the call site, not on the fixture function signature.

## GOSKernel context

Applies everywhere: `crates/gos-protocol/src/lib.rs` defines the type. All host-test harnesses that use `VectorAddress::new()` with variables (not just literals) are susceptible.

## From this session

V2.55 node-attr harness: `fn na_vec(minor: u8) -> VectorAddress { VectorAddress::new(29, 1, minor, 0) }` failed with `E0308: expected u16, found u8`. Fixed to `minor as u16`.
