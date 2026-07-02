# GOS Hardening Log — V2.16 — 2026-07-01

## Summary

V2.16 adds epoch-addressed `graph diff <N>` shell command, letting the operator query
topology mutations since any arbitrary epoch number rather than only the stored pin epoch.
This mirrors `git log --since-commit=<sha>` semantics within the graph-OS shell.

---

## Changes

### 1. `parse_epoch_decimal` — k-shell (`crates/k-shell/src/lib.rs`)

New `pub(crate)` helper added:

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

- No `std`/`alloc` required — pure byte iteration over the input slice.
- `saturating_mul` + `saturating_add` prevent panic on overflow; very large epoch strings
  saturate to `u64::MAX` (which correctly returns 0 diff entries).
- Returns `None` on any non-digit character, producing a user-facing error message.

### 2. `graph diff <N>` shell command — k-shell (`crates/k-shell/src/proc.rs`)

New branch inserted after `graph diff reset` in `dispatch_text_command()`:

```
graph diff <N>   →   dispatch_graph_diff(sink, N)
diff <N>         →   same (short alias works too)
```

Implementation pattern:

```rust
} else if let Some(epoch_str) = cmd
    .strip_prefix("graph diff ")
    .or_else(|| cmd.strip_prefix("diff "))
    .filter(|s| *s != "pin" && *s != "reset")
{
    let trimmed = epoch_str.trim();
    if let Some(epoch) = super::parse_epoch_decimal(trimmed) {
        super::dispatch_graph_diff(sink, epoch);
    } else {
        // print error: "graph diff <epoch>: epoch must be a decimal number"
    }
```

The `filter(|s| *s != "pin" && *s != "reset")` guard is redundant (exact match branches
come first in the `else if` chain) but makes intent explicit and future-proof.

Help text updated with:
```
  graph diff <N>     show topology changes since epoch N (e.g. graph diff 42)
```

### 3. Test harness — `host-tests/gos-graph-diff-epoch-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `diff_since_zero_returns_all_mutations` | epoch 0 → all 3 node registrations visible |
| 2 | `diff_since_current_epoch_returns_nothing` | diff_since(current) → 0 entries |
| 3 | `diff_since_mid_epoch_shows_only_later_mutations` | pin in middle → only post-pin entry |
| 4 | `diff_since_epoch_boundary_is_exclusive` | epoch boundary exclusive: node at E not in diff_since(E) |
| 5 | `diff_since_max_epoch_returns_nothing` | diff_since(u64::MAX) → 0 |
| 6 | `diff_since_zero_shows_mixed_node_and_edge_events` | NodeAdded + EdgeAdded both visible |
| 7 | `diff_since_epoch_before_edge_shows_edge_added` | pin before edge → EdgeAdded visible |
| 8 | `diff_since_after_node_shows_edge_not_node` | pin after nodes → only EdgeAdded visible |
| 9 | `diff_since_fills_capped_at_page_size` | total > PAGE → filled == PAGE, total correct |
|10 | `diff_since_pin_shows_edge_removed` | pin → unregister_edge → EdgeRemoved visible |

---

## Verification

```
cd host-tests/gos-graph-diff-epoch-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cd host-tests/gos-graph-diff-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed  (regression: unchanged)

cargo build --release
# Finished `release` profile
```

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.16 |
|---|---|---|
| Point-in-time diff from a known checkpoint | `git log <commit>..HEAD` | `graph diff <epoch>` |
| All-time topology history | `git log` | `graph diff 0` |
| Diff since last operation | `git diff HEAD~1` | `graph diff <epoch_before>` |
| Diff only from a future point | N/A | `graph diff <future_epoch>` → empty |

Before V2.16, operators could only diff against the stored pin epoch (last `graph diff pin`)
or epoch 0. V2.16 allows any epoch to be addressed directly, enabling one-shot queries like
"what changed since I registered node X?" without needing to pre-pin.

---

## Graph-OS Characteristic Preserved

The epoch system is the graph-OS analogue of a monotonic logical clock: every structural
mutation (node register/unregister, edge register/unregister) advances the epoch by one.
`graph diff <N>` exposes that clock directly to the operator shell, staying true to the
graph-OS principle that the topology is a first-class, inspectable, auditable structure —
not a hidden kernel implementation detail.

---

*Automated hardening pass — GOS V2.16 — 2026-07-01*
