---
name: gos-persistent-state-outside-runtime
description: When adding state that must survive gos_runtime::reset() (e.g. monitoring baselines, configuration, saved snapshots), store it in a separate static Mutex<T> outside of RUNTIME — not inside GraphRuntime. Apply whenever building any feature that needs cross-reset persistence in gos-runtime.
---

# Persistent State Outside RUNTIME

## The rule

State that must outlive `gos_runtime::reset()` belongs in its own top-level static,
NOT in a field of `GraphRuntime`:

```rust
// CORRECT: separate static — persists across reset()
static METRIC_SNAPSHOT: Mutex<MetricSnapshot> = Mutex::new(MetricSnapshot {
    valid: false,
    // ... zero-initialized fields ...
});

pub fn graph_snapshot_save() -> u64 {
    let snap = RUNTIME.lock().graph_snapshot_inner();
    *METRIC_SNAPSHOT.lock() = snap;          // stored outside RUNTIME
    snap.epoch
}
```

**Do NOT** put it inside `GraphRuntime`:
```rust
// WRONG: cleared by reset()
struct GraphRuntime {
    // ...
    saved_snapshot: MetricSnapshot,  // reset() zeroes this — persistence lost!
}
```

## Why it's non-obvious

`gos_runtime::reset()` replaces `*RUNTIME.lock()` with a freshly zeroed `GraphRuntime`.
Any field you add to `GraphRuntime` is obliterated on every reset. In a host-test
harness (and at kernel boot), `reset()` is called between test cases — so a snapshot
saved in test 1 would vanish before test 2 could compare against it.

A separate `static Mutex<T>` is entirely outside `RUNTIME`; `reset()` has no visibility
into it. The state persists from the moment it is first written until the program exits
(or you explicitly clear it).

## When this matters

- Monitoring baselines that predate the test/session that reads them
- User-visible configuration that should survive topology resets
- Counters or logs that track history across multiple reset cycles

## Const-initialization requirement

The type `T` in `static Mutex<T>` must be const-initializable. Prefer:
- `valid: bool` sentinel field over `Option<T>` (avoids Option discriminant issues)
- All-zero numeric fields (use `= 0` not `= Default::default()`)
- `Copy + Clone` for the outer type (allows `*lock = new_value` assignment)

## GOSKernel context

- `METRIC_SNAPSHOT: Mutex<MetricSnapshot>` — `crates/gos-runtime/src/lib.rs` (V2.83)
- `RUNTIME: Mutex<GraphRuntime>` — cleared by `reset()`; do not add long-lived state here
- `IRQ_TABLE: Mutex<IrqTable>` — another example of a separate persistent static
- Pattern first appeared with `IRQ_TABLE` (V2.x interrupt routing); reused for snapshots

## From this session

V2.83 snapshot harness test 5 verified this: save a snapshot with 1 node, then call
`reset()` (which happens implicitly between tests via the TEST_LOCK serialization and
each test calling `reset()` at the top). The saved snapshot with node_count=1 survives
into the comparison in test 8 (double-save test), confirming METRIC_SNAPSHOT outlives RUNTIME.

See [[gos-multi-metric-epoch-consistency]] for how to populate the snapshot atomically.
