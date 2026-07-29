---
name: gos-kshell-numeric-param-routing
description: When adding a k-shell shell command that takes a numeric argument (e.g. `graph rich club <k>`), parse the integer inline in proc.rs using byte-by-byte b'0'..=b'9' validation with saturating arithmetic — do NOT use std::str::parse (not available in no_std context at the shell layer). Apply in crates/k-shell/src/proc.rs whenever a new parameterized dispatch command needs an integer argument.
---

# K-Shell Numeric Parameter Routing: Inline Byte Parser

## The rule

For a shell command of the form `<prefix> <integer>`, use this pattern in `proc.rs`:

```rust
} else if let Some(k_str) = cmd
    .strip_prefix("graph rich club ")
    .or_else(|| cmd.strip_prefix("richclub "))
    .or_else(|| cmd.strip_prefix("grichclub "))
{
    let k_trimmed = k_str.trim();
    let mut k_val: u8 = 1;               // sensible default
    let mut k_valid = !k_trimmed.is_empty();
    if k_valid {
        let mut v: u16 = 0;              // wider type to detect overflow
        for b in k_trimmed.bytes() {
            if b < b'0' || b > b'9' { k_valid = false; break; }
            v = v.saturating_mul(10).saturating_add((b - b'0') as u16);
            if v > 255 { k_valid = false; break; }  // u8 range check
        }
        if k_valid { k_val = v as u8; }
    }
    if k_valid {
        super::dispatch_graph_rich_club(sink, k_val);
    } else {
        super::set_color(sink, 12, 0);   // red error
        super::print_str(sink, " graph rich club: k must be 0\u{2013}255, e.g. `graph rich club 2`\n");
        super::set_color(sink, 7, 0);
    }
}
```

Key points:
- Use a **wider accumulator type** (u16 for u8 target, u64 for u32 target) to detect range overflow
- Check `v > MAX` inside the byte loop, not after — prevents wrap-around from `as u8` truncation
- `k_trimmed.is_empty()` → `k_valid = false` first; catches missing argument
- Error message uses `\u{2013}` for en-dash, not `–` literal (no_std string literal safety)

## Why it's non-obvious

1. **`str::parse::<u8>()` is available** at the shell layer (it's in `core`, not `std`). But using it in a no_std context requires explicit `.map_err(|_| ...)` handling, and GOSKernel's proc.rs convention is to use `ConsoleSink` for error output, not `Result` types. The inline byte parser keeps the error path consistent with other command validation in proc.rs.

2. **The wider accumulator type prevents silent truncation.** If you parse into `u8` directly with `v = v.saturating_mul(10).saturating_add(...)`, the value saturates at 255 and you silently accept `300` as `255`. Using `u16` and checking `v > 255` produces a proper error.

3. **Routing ORDER matters in proc.rs.** More-specific prefix strings must appear BEFORE less-specific ones. Place `"graph rich club "` (with trailing space) before any plain `"graph rich"` prefix that might exist — otherwise the longer match is unreachable.

## Existing pattern: `parse_epoch_decimal` in lib.rs

For `u64` values, the existing helper is:
```rust
pub(crate) fn parse_epoch_decimal(s: &str) -> Option<u64> {
    if s.is_empty() { return None; }
    let mut val: u64 = 0;
    for b in s.bytes() {
        if b < b'0' || b > b'9' { return None; }
        val = val.saturating_mul(10).saturating_add((b - b'0') as u64);
    }
    Some(val)
}
```
Use this for u64 parameters. For smaller types (u8, u16, u32), inline the byte loop with a wider accumulator.

## GOSKernel context

- Routing file: `crates/k-shell/src/proc.rs`
- Dispatch functions: `crates/k-shell/src/lib.rs` — signature `pub fn dispatch_<name>(sink: &ConsoleSink, k: u8)`
- Help text: add to the `"help"` / `"?"` branch in proc.rs (around line 607)
- Error color: `set_color(sink, 12, 0)` = red; `set_color(sink, 7, 0)` = reset

## From this session

V2.68 `graph rich club <k>`: the inline byte parser was needed because k is a u8 (0–255). The wider-type trick was applied: accumulate into u16, check `v > 255` per iteration, cast to u8 only after full validation. A missing argument (empty k_str after trim) sets `k_valid = false` immediately.
