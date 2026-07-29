---
name: gos-fbtest-render-lockfree-invariant
description: fbtest.rs render_frame() must NEVER lock RUNTIME. All graph-derived data (palette colors, node counts, positions) must be loaded in init() using without_interrupts() and cached in the Desktop struct. Apply whenever adding runtime data reads to crates/hypervisor/src/fbtest.rs, especially in draw_popup or render_frame.
---

# fbtest render_frame Lock-Free Invariant

## The rule

`render_frame()` in `crates/hypervisor/src/fbtest.rs` must never call `gos_runtime::*` functions that lock `RUNTIME`. Any data derived from the runtime graph — palette colors, node/edge counts, layout coordinates — must be:

1. Read in `init()` using `without_interrupts(|| gos_runtime::...)`
2. Stored in the `Desktop` struct
3. Accessed in `render_frame()` / `draw_popup()` via `d.*` fields

## Why it's non-obvious

The comment "render_frame never locks RUNTIME (only reads k-mouse motion atomics)" exists but is easy to miss. Violating this can cause deadlocks: if an interrupt fires while render_frame holds the RUNTIME mutex, and that interrupt handler also tries to lock RUNTIME, the system deadlocks. This is a bare-metal no-preemption concern, not caught by the borrow checker.

## GOSKernel context

- `crates/hypervisor/src/fbtest.rs` — Desktop struct, init(), render_frame()
- Pattern established in V2.57: `Desktop.pal_u32: [u32; 4]` is populated in `init()` from node_attr_get; render path uses `d.pal_u32[ci]`
- `without_interrupts(|| gos_runtime::...)` is the correct pattern for RUNTIME access from kernel context
- `k-mouse` and `k-ps2` atomics are the ONLY allowed runtime reads inside render_frame

## From this session

V2.57 added `Desktop.pal_u32` to cache palette colors from theme node attrs. The init() reads both THEME_WABI_NODE_VEC and THEME_SHOJI_NODE_VEC via `without_interrupts(|| gos_runtime::node_attr_get(...))` and populates `d.pal_u32[0..1]`. render_frame then uses `d.pal_u32[ci]` in swatch drawing and rope rendering, never accessing RUNTIME directly.
