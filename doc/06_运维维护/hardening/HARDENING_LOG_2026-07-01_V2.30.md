# GOS Hardening Log — V2.30 — 2026-07-01

## Summary

V2.30 adds a live proc watch panel — `watch` / `graph watch` commands flip the VECTOR
DECK panel into a continuously-refreshing proc table driven by the existing heartbeat tick,
analogous to `watch -n1 proc` or `htop` on Linux.  Any keypress exits watch mode and
restores the normal VECTOR DECK view.

---

## Changes

### 1. `WATCH_PROC_MODE` static — k-shell (`crates/k-shell/src/lib.rs`)

```rust
pub(crate) static WATCH_PROC_MODE: AtomicU8 = AtomicU8::new(0);
```

- `0` = normal VECTOR DECK view
- `1` = live proc watch mode

### 2. `dispatch_watch_proc()` and `dispatch_watch_stop()` — k-shell (`crates/k-shell/src/lib.rs`)

```rust
pub fn dispatch_watch_proc(sink: &ConsoleSink) { ... }
pub fn dispatch_watch_stop(sink: &ConsoleSink) { ... }
```

- `dispatch_watch_proc`: sets `WATCH_PROC_MODE = 1`, prints confirmation message.
- `dispatch_watch_stop`: sets `WATCH_PROC_MODE = 0`, prints "watch stopped".

### 3. `draw_watch_proc_panel()` — k-shell (`crates/k-shell/src/lib.rs`)

New function renders the VECTOR DECK box in watch mode.  Fixed layout (fits 47 × 10 chars):

```
╔═══════════[ PROC WATCH ]══════════════╗
║ tick 12345   nodes 8   any key stops  ║
║ vector           sig  out  lifecycle  ║
║ 6.1.0.0          145   3  running     ║
║ 6.1.1.0            0   1  running     ║
║ 6.1.2.0            0   1  running     ║
║ 6.1.3.0            0   1  running     ║
║ 6.1.4.0            0   1  running     ║
║ ... 2 more                            ║
╚═══════════════════════════════════════╝
```

- Shows top 6 nodes by vector address, all from `proc_page::<6>()`.
- Color-codes lifecycle: green = Running, red = Faulted, yellow = Suspended, grey = other.
- Displays cumulative `signal_count` and `edge_out_count` per node.
- Shows `snapshot().tick` as a live heartbeat counter.
- Renders `... N more` when total > 6.

### 4. `draw_command_deck_panel()` — k-shell (`crates/k-shell/src/lib.rs`)

Early-return delegate: if `WATCH_PROC_MODE != 0`, calls `draw_watch_proc_panel` and returns.
Normal graph stats panel is rendered otherwise (no change to existing logic).

### 5. Heartbeat always repaints in watch mode — k-shell (`crates/k-shell/src/proc.rs`)

```rust
let watch_active = super::WATCH_PROC_MODE.load(...) != 0;
if watch_active || current_epoch != state.last_rendered_epoch {
    ...
    super::draw_command_deck_panel(...);
}
```

V2.3 epoch-diff idle skip is preserved for normal mode.  In watch mode the panel
repaints every 4th heartbeat tick so the tick counter and signal counts update live.

### 6. Any keypress exits watch mode — k-shell (`crates/k-shell/src/proc.rs`)

```rust
if source == DataSource::Keyboard
    && super::WATCH_PROC_MODE.load(...) != 0
{
    super::WATCH_PROC_MODE.store(0, ...);
    state.last_rendered_epoch = u64::MAX;  // force deck repaint
    ...
    return ExecStatus::Done;
}
```

Any keyboard byte while in watch mode clears `WATCH_PROC_MODE`, forces an immediate
epoch cache invalidation so the normal deck repaints on the next heartbeat, and
prints "watch stopped" to the scroll area.

### 7. Shell commands registered — k-shell (`crates/k-shell/src/proc.rs`)

| Command | Action |
|---|---|
| `watch` | Enter live proc watch mode |
| `graph watch` | Alias for `watch` |
| `watch proc` | Alias for `watch` |
| `watch nodes` | Alias for `watch` |
| `watch stop` | Exit watch mode explicitly |
| `watch exit` | Alias for `watch stop` |

`help` text updated with all six variants.

### 8. Test harness — `host-tests/gos-watch-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `proc_page_is_idempotent` | Two consecutive proc_page calls return identical totals |
| 2 | `proc_page_empty_on_empty_runtime` | Empty runtime → watch shows "(no nodes)" |
| 3 | `proc_page_reflects_registration_immediately` | Registration visible on next proc_page call |
| 4 | `proc_count_consistent_with_proc_page_total` | proc_count() and proc_page total agree |
| 5 | `proc_page_reflects_signal_count_after_dispatch` | Live signal_count after one dispatch |
| 6 | `repeated_proc_page_reads_stable_after_dispatch` | Read-only: no mutation on repeated reads |
| 7 | `proc_page_shows_faulted_after_fault_node` | fault_node() reflected in lifecycle |
| 8 | `proc_page_shows_running_after_resume` | resume_node() clears Faulted state |
| 9 | `snapshot_node_count_matches_proc_count` | snapshot().node_count == proc_count() |
| 10 | `snapshot_tick_advances_after_pump` | snapshot().tick is live (advances on pump) |

---

## Verification

```
cd host-tests/gos-watch-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

Kernel build:
```
cargo build --release
# Finished `release` profile
```

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.30 |
|---|---|---|
| Live process monitor | `watch -n1 ps` / `htop` | `watch` / `graph watch` shell command |
| Auto-refresh | Timer-driven redraw | Heartbeat-driven repaint (no threads) |
| Any-key exit | Ctrl+C / q | Any keypress exits watch mode |
| Tick counter | wall clock | `snapshot().tick` (graph OS heartbeat) |
| Node state | STAT column | `lifecycle` column (Running/Faulted/Suspended) |
| Signal activity | utime/stime | `signal_count` (cumulative per-node) |
| Topology fan-out | open file count | `edge_out_count` (outbound edge count) |
| Watch mode flag | process state | `WATCH_PROC_MODE: AtomicU8` (zero-overhead) |

The watch panel reuses the existing fixed-position VECTOR DECK box — no additional
terminal rows consumed, no scroll-region conflicts.  The implementation adds exactly one
`AtomicU8::load` per heartbeat tick in non-watch mode (negligible overhead).

---

## Graph-OS Characteristic Preserved

The watch panel exposes **graph-topology metrics** (edge out-degree per node) alongside
signal throughput (signal_count), keeping the live monitor rooted in GOS's graph model
rather than a flat process table.  The VECTOR DECK panel's PROC WATCH mode mirrors
how `htop` overlays onto the terminal while remaining structurally graph-native.

---

*Automated hardening pass — GOS V2.30 — 2026-07-01*
