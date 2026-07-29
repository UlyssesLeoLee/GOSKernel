---
name: gos-rust-format-curly-brace
description: In Rust format string literals (including assert_eq! messages and print! calls), literal curly braces must be doubled: { → {{ and } → }}. Math/graph notation like K_{1,3} or regex {n,m} inside test message strings will cause compile errors unless escaped to K_{{1,3}} or {{n,m}}. Apply when writing test assertion messages that contain mathematical graph notation.
---

# Rust Format String: Escaping Literal Curly Braces

## The rule

In any Rust format string — including the message argument of `assert_eq!`, `assert!`, `panic!`, `format!`, and `print!` — a literal `{` or `}` must be written as `{{` or `}}`. Otherwise Rust interprets `{...}` as a format argument.

```rust
// WRONG — Rust sees {1,3} as a malformed format argument → compile error
assert_eq!(chromatic, 2, "star graph K_{1,3} is bipartite → 2 colors");
// error: invalid format string: python's numeric grouping `,` is not supported

// CORRECT — escape the braces
assert_eq!(chromatic, 2, "star graph K_{{1,3}} is bipartite → 2 colors");
```

Common math/graph notation that needs escaping:
| Written in math | Rust string literal | Error type |
|----------------|---------------------|------------|
| `K_{1,3}` | `"K_{{1,3}}"` | invalid format specifier |
| `K_{n,m}` | `"K_{{n,m}}"` | invalid format specifier |
| `{0,1,...,n}` | `"{{0,1,...,n}}"` | invalid format specifier |
| `O({n²})` | `"O({{n²}})"` | invalid format specifier |
| regex `\d{3}` | `"\\d{{3}}"` | invalid format specifier |
| `clique={A,B}` | `"clique=A-B"` or `"clique={{A,B}}"` | `{A,B}` → named arg `A,B` (comma invalid) or `{A}` → E0425 (name not in scope) |
| `{A}`, `{B}` | `"{{A}}"`, `"{{B}}"` | E0425: cannot find value `A` in this scope |

## Why it's non-obvious

Mathematical notation for graphs and complexity theory frequently uses `{n,m}` subscripts and set notation `{...}`. These look like ordinary text but Rust's format machinery processes them as interpolation placeholders. The error message "python's numeric grouping `,` is not supported" is confusing — it doesn't say "escape your braces", it says "wrong format specifier", which makes the root cause easy to miss.

This only matters in format strings, not in regular `&str` literals. So `let s: &str = "K_{1,3}";` compiles fine; it's only `format!("K_{1,3}")` or `assert_eq!(x, y, "K_{1,3}")` that fail.

## GOSKernel context

- Appears in harness test files: `host-tests/gos-graph-*/tests/*.rs`
- Especially in test descriptions for bipartite (`K_{2,2}`), complete (`K_3`, `K_4`), and star (`K_{1,3}`) graph cases
- The `assert_eq!` third argument is a format string, so even a "static" description must follow format-string rules

## From this session

V2.47 `gos-graph-color-harness`: test `descending_degree_order` had:
```rust
assert_eq!(chromatic, 2, "star graph K_{1,3} is bipartite → 2 colors");
```
Compile error: `invalid format string: python's numeric grouping`,` is not supported`. Fixed by escaping to `K_{{1,3}}`.
