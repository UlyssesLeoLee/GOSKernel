---
name: gos-xorshift-seed-zero
description: xorshift32 with seed=0 produces 0 forever — a silent infinite-output-zero bug. Always guard against seed=0 at the call site by substituting a non-zero sentinel before the first shift. Apply whenever xorshift32/64 is used as a no_std PRNG in gos-runtime or any bare-metal kernel crate.
---

# xorshift PRNG: Seed=0 Is Silently Broken

## The rule

Map `seed == 0` to a non-zero sentinel before the first xorshift operation:

```rust
// WRONG — state=0 produces 0 forever, the PRNG is stuck
let mut rng: u32 = seed;

// CORRECT — 0 is remapped; sentinel value choice is arbitrary but non-zero
let mut rng: u32 = if seed == 0 { 0xDEAD_BEEF } else { seed };
```

The xorshift recurrence `x ^= x << 13; x ^= x >> 17; x ^= x << 5` maps 0→0 at every step.

## Why it's non-obvious

The xorshift PRNG has period 2³²−1, covering all non-zero u32 values. State=0 is the one excluded state. If a caller passes 0 (common default), the "random" output is 0 on every call — no compile error, no panic, just silently wrong walk behavior that looks like "all steps go to the same node".

In GOSKernel the seed is often `graph_epoch as u32 ^ steps ^ 0xDEAD_BEEF`. If graph_epoch=0 (empty graph, first boot, or after `reset()` in tests) and steps=0 and the constant is XORed away, seed collapses to 0.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_sim_inner` uses xorshift32
- The guard lives at the top of `graph_sim_inner`: `let mut rng: u32 = if seed == 0 { 0xDEAD_BEEF } else { seed };`
- Harness tests call `gos_runtime::graph_sim::<128>(steps, FIXED_SEED)` with a non-zero constant to avoid the issue in tests

## From this session

V2.52 `graph sim`: implemented xorshift32 for edge sampling and starting-node selection. Discovered the seed=0 trap during code review — the `macro_rules! next_rng!` block would return 0 on every invocation if `rng` started at 0, causing all walk steps to pick edge index 0 and restart slot 0 deterministically. Guarded at init with the sentinel check.
