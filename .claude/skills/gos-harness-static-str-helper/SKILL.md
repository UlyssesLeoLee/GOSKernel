---
name: gos-harness-static-str-helper
description: When GOSKernel harness helper functions call add_edge/register_edge with a &'static str key, never use &format!() — the temporary doesn't live 'static. Pass separate static string literals as distinct parameters instead.
---

# Harness Helper Functions: &'static str vs format!()

## The rule

`add_edge` (and `register_edge` / `derive_edge_id`) take `&'static str` for the edge key.
Helper functions that wrap multiple `add_edge` calls must NOT use `format!()` to construct keys:

```rust
// WRONG — compile error E0716: temporary value dropped while borrowed
fn add_bidir(a: NodeId, b: NodeId, key: &'static str) {
    add_edge(a, b, &format!("{key}f"));  // ← creates a non-static temporary
    add_edge(b, a, &format!("{key}r"));
}

// CORRECT — pass separate static key literals as extra parameters
fn add_bidir(a: NodeId, b: NodeId, fwd: &'static str, rev: &'static str) {
    add_edge(a, b, fwd);
    add_edge(b, a, rev);
}

// Call site
add_bidir(TR_ID_A, TR_ID_B, "abf", "abr");
add_bidir(TR_ID_A, TR_ID_C, "acf", "acr");
```

## Why it's non-obvious

The error message is `E0716: temporary value dropped while borrowed`, not a type mismatch error. It's easy to assume you can borrow `format!()` output as `&str` since `String` deref-coerces to `str`. But `add_edge` requires `&'static str` (which derives_edge_id needs), and a `format!()` temporary lives only for the statement, not `'static`. The compiler message mentions "argument requires that borrow lasts for 'static" which is the clue.

## GOSKernel context

- All harness test files in `host-tests/gos-*-harness/tests/*.rs`
- `add_edge(from: NodeId, to: NodeId, key: &'static str)` — the `key` is `&'static str` because `derive_edge_id` is a `const fn`
- This affects any helper that tries to programmatically generate edge key names

## From this session

V2.94 k-truss harness: `add_bidir(a, b, &format!("{key}f"))` produced E0716 at compile time.
Fixed by giving `add_bidir` two separate `fwd: &'static str, rev: &'static str` parameters and calling with inline literals: `add_bidir(TR_ID_A, TR_ID_B, "abf", "abr")`.
