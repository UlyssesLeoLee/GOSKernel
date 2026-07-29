---
name: gos-kshell-u64-decimal-print
description: When a k-shell dispatch function must print a u64 value as decimal (not hex), use a 20-byte stack buffer and divide-by-10 loop — print_num_inline only handles usize. Apply in crates/k-shell/src/lib.rs whenever displaying wiener_index, flow totals, or any u64 scalar.
---

# k-shell: Print u64 as Decimal in no_std Dispatch Functions

## The rule

`print_num_inline` in k-shell works for `usize`, not `u64`. For any `u64` display,
use a stack buffer with a right-to-left fill loop:

```rust
fn print_u64_inline(sink: &ConsoleSink, mut v: u64) {
    let mut buf = [b'0'; 20];
    let mut pos = 20usize;
    if v == 0 {
        pos -= 1;
        buf[pos] = b'0';
    } else {
        while v > 0 {
            pos -= 1;
            buf[pos] = b'0' + (v % 10) as u8;
            v /= 10;
        }
    }
    for i in pos..20 {
        print_byte(sink, buf[i]);
    }
}
```

Or inline if only used once in a dispatch function. Call `print_byte`, NOT `sink.write_byte`
(ConsoleSink has no `write_byte` method — it's a standalone `fn print_byte(sink, byte)`).

For 3-decimal-place average path length from a u64/usize ratio:
```rust
let avg_whole = (wiener / pairs as u64) as usize;
let avg_frac  = ((wiener % pairs as u64) * 1000 / pairs as u64) as usize;
print_num_inline(sink, avg_whole);
print_str(sink, ".");
if avg_frac < 10   { print_str(sink, "00"); }
else if avg_frac < 100 { print_str(sink, "0"); }
print_num_inline(sink, avg_frac);
```

For 6-decimal-place ppm values (e.g. global efficiency E(G) ∈ [0, 1_000_000]):
```rust
let whole = (ppm / 1_000_000) as usize;   // 0 or 1
let frac  = (ppm % 1_000_000) as usize;   // 0..999_999
print_num_inline(sink, whole);
print_str(sink, ".");
// Zero-pad frac to 6 digits
if frac < 10          { print_str(sink, "00000"); }
else if frac < 100    { print_str(sink, "0000"); }
else if frac < 1_000  { print_str(sink, "000"); }
else if frac < 10_000 { print_str(sink, "00"); }
else if frac < 100_000 { print_str(sink, "0"); }
print_num_inline(sink, frac);
```

The `whole` part is 0 for all in-range values; it becomes 1 only for a complete graph
(ppm=1_000_000). Always print it: `0.500000` not `.500000`.

## Why it's non-obvious

The k-shell crate is `no_std` — `format!("{}", v)` is unavailable. `print_num_inline`
exists for `usize` (which is 64-bit on Windows) and would work if you cast u64→usize, but
the cast is lossy for values > usize::MAX on 32-bit targets. More importantly, there's no
`sink.write_byte(b)` method — `print_byte` is a module-level function that takes
`(sink: &ConsoleSink, byte: u8)`. Confusing it with a method call (`sink.write_byte(b)`)
compiles with a helpful error but costs a round-trip to notice.

## GOSKernel context

- `crates/k-shell/src/lib.rs` — `dispatch_graph_wiener` (V2.70) is the first place u64
  decimal printing was needed in a dispatch function
- `print_byte(sink, b)` defined around line 534 in lib.rs
- `print_num_inline(sink, n: usize)` defined nearby — covers usize (decimal only)
- For hex u32 display see the `gos-kshell-hex-display-helper` skill

## From this session

V2.70: initial draft used `sink.write_byte(buf[i])` — compile error (no such method).
Fixed by changing to `print_byte(sink, buf[i])`. Added a 20-byte buffer u64 decimal loop
for the wiener_index field in `dispatch_graph_wiener`.
