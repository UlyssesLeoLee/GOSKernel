---
name: gos-rust-unicode-escapes
description: In Rust string literals, \xNN byte escapes are only valid for 0x00–0x7F (ASCII range). Multi-byte UTF-8 characters (CJK, arrows, emoji, any codepoint > 0x7F) must use \u{NNNN} Unicode escapes or raw character literals — never \xE2\x86... style byte sequences. Apply whenever writing or reviewing Rust strings containing non-ASCII characters, especially in no_std kernel crates where display strings embed Unicode arrows or symbols.
---

# Rust Unicode Escapes in no_std Strings

## The rule

In Rust, `\xNN` hex escapes are **only valid for values 0x00–0x7F**. Any codepoint above 0x7F must use `\u{NNNN}` (Unicode scalar value escape) or a raw UTF-8 character literal.

```rust
// WRONG — \xE2\x86\x92 are the UTF-8 bytes for →, but Rust rejects \xNN > 0x7F
print_str(sink, " \xE2\x86\x92 ");  // compile error: must be 0x00–0x7F

// CORRECT
print_str(sink, " \u{2192} ");  // → (U+2192 RIGHTWARDS ARROW)
print_str(sink, " → ");         // or just embed the literal character
```

Common arrows and their escapes:
| Char | Codepoint | `\u{...}` |
|------|-----------|-----------|
| →    | U+2192    | `\u{2192}` |
| ↩    | U+21A9    | `\u{21A9}` |
| ↓    | U+2193    | `\u{2193}` |
| ←    | U+2190    | `\u{2190}` |
| ↑    | U+2191    | `\u{2191}` |

## Why it's non-obvious

C programmers (and copy-paste from hex dumps) often write multi-byte UTF-8 sequences as `\xNN\xNN\xNN`. Python allows this. Rust does not — each `\xNN` in a Rust string is a single byte escape that must represent a valid Unicode scalar, which means ≤ 0x7F. The error message ("must be a character in the range [\x00-\x7f]") appears once per byte, so a 3-byte sequence like `→` generates 3 compile errors.

## GOSKernel context

Appears in `crates/k-shell/src/lib.rs` display functions: `dispatch_graph_toposort`, `dispatch_graph_scc`, and any other command that renders Unicode arrows in the terminal output. The V2.33 hardening commit introduced `\xE2\x86...` sequences that caused 9 compile errors (3 per arrow × 3 arrows).

## From this session

CI "verify" failed with 10 errors in `k-shell`. Root cause: the main-branch merge (V2.33 toposort) added three Unicode arrows using raw UTF-8 bytes as Rust `\xNN` escapes. Fixed by replacing with `\u{2192}`, `\u{21A9}`, `\u{2193}`.
