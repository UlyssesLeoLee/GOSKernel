---
name: gos-kshell-hex-display-helper
description: k-shell has print_num_inline (decimal) but no hex printer. Any new dispatch function that shows u32/u64 values in hex must define and call print_hex32_inline (8 hex digits) locally in lib.rs — it does not exist until added. Apply whenever writing a k-shell dispatch function that displays addresses, palette colors, flags, or any hex scalar.
---

# k-shell: define print_hex32_inline for hex u32 display

## The rule

When a `dispatch_*` function in `crates/k-shell/src/lib.rs` needs to print a `u32` value in hex, define `print_hex32_inline`:

```rust
fn print_hex32_inline(sink: &ConsoleSink, value: u32) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = value.to_be_bytes();
    for b in bytes {
        print_byte(sink, HEX[(b >> 4) as usize]);
        print_byte(sink, HEX[(b & 0xF) as usize]);
    }
}
```

Place it near `print_num_inline` (around line 7835 in lib.rs). Then call it:

```rust
print_str(sink, "  =  0x");
print_hex32_inline(sink, val);
print_str(sink, "\n");
```

## Why it's non-obvious

The shell crate is `no_std` and has no `format!`, so there's no `format!("{:#010x}", val)`. The `print_num_inline` helper exists for decimal but hex was never needed until V2.55's u32 attribute display. It's easy to assume a hex helper already exists alongside the decimal one — it doesn't (or didn't) until added.

## GOSKernel context

`crates/k-shell/src/lib.rs` — the display helpers section near `print_num_inline`. The `no_std` constraint rules out `core::fmt` write! macros in this context (no sink adapter for fmt::Write).

## From this session

V2.55: `dispatch_node_attr_set` and `dispatch_node_attr_get` both call `print_hex32_inline` to display palette color values like `0x00db1c21`. The function was missing; added it after confirming `print_num_inline` existed but had no hex counterpart.
