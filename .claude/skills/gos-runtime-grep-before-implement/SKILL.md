---
name: gos-runtime-grep-before-implement
description: Before adding a new graph algorithm to GOSKernel, ALWAYS grep gos-runtime/src/lib.rs for existing `pub fn graph_` functions — memory records are stale and the runtime has 50+ functions including many not tracked in the hardening log.
---

# Always Grep gos-runtime Before Assuming an Algorithm Gap

## The rule

Before writing a new `graph_*` function or harness, run:

```powershell
grep -n "^pub fn graph_" crates/gos-runtime/src/lib.rs
```

If the function already exists, pick a different algorithm. The hardening memory file
only tracks additions since V2.71; many earlier functions (V2.37 bipartite, V2.40 closeness,
graph_katz, graph_hits, graph_mst, graph_color, graph_spanning, graph_sim, graph_between,
graph_flow, graph_attractor, etc.) are NOT in the memory log.

## Why it's non-obvious

The memory file `gos_hardening_current_state.md` starts tracking at V2.71. It does NOT
record functions added before that milestone. The runtime currently has 50+ public graph
functions spanning V2.31–V2.89. A gap "not mentioned in memory" does not mean "not implemented."

Also: `cargo check` will catch duplicates with `error[E0428]: the name X is defined multiple times`
or `error[E0592]: duplicate definitions with name X`, but only AFTER you've already written the
duplicate implementation — wasted effort.

Additionally, check k-shell for existing dispatch functions before adding routing:
```powershell
grep -n "pub fn dispatch_graph_" crates/k-shell/src/lib.rs
```
And check proc.rs for existing routing:
```powershell
grep -n "cmd == " crates/k-shell/src/proc.rs | grep "graph"
```

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — all public graph API functions
- `crates/k-shell/src/lib.rs` — all dispatch functions
- `crates/k-shell/src/proc.rs` — all routing (cmd == "...") entries
- `C:\Users\leo19\.claude\projects\E--GOSKernel\memory\gos_hardening_current_state.md` — stale past V2.70

## From this session (2026-07-04)

Attempted to add `graph_bipartite` (already V2.37) and `graph_closeness` (already V2.40).
Both caused `error[E0428]` duplicate name conflicts. Discovered ~50 untracked functions.
Final implementation: `graph_dag_layers` (V2.89) — confirmed unique by grepping first.
