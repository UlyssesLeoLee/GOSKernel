# Hardening Log — GOS V2.35
**Date**: 2026-07-02  
**Commit**: feat(v2.35): graph condensation / condense  
**Branch**: main  
**Author**: Automated hardening pass (Claude Sonnet 4.6)

---

## Summary

Implemented `graph condensation` — the condensation DAG of the live node graph.  
This completes the **graph algorithm quartet** (cycles / toposort / scc / condensation).

---

## What Changed

### `crates/gos-runtime/src/lib.rs`

Added `graph_condensation_inner<const N: usize>` method on `RuntimeState`:

- Runs the same Kosaraju two-pass DFS as `graph_scc_inner` to assign SCC IDs per slot.
- Phase 3 (new): scans all live edges; for each edge crossing SCC boundaries (`scc_id[from] != scc_id[to]`), sets `adj[from_scc] |= 1u128 << to_scc`. Duplicate cross-SCC edges are deduplicated in O(1) via the bitmask.
- Phase 4: packs nodes and labels in SCC order (identical to `graph_scc_inner` output).
- Return: `([VectorAddress; N], [u8; N], usize, usize, [u128; 128], usize)`  
  = `(nodes, labels, total, scc_count, adj, cond_edges)`.
- Complexity: O(V + E), no_std safe, all fixed-stack arrays (no heap).

Added public wrapper `graph_condensation<const N: usize>()` — thin lock+call.

**Stack budget**: ~7.5 KB (two DFS stacks + scc_id array + condensation adj matrix + output arrays). Within kernel stack limits (≥ 16 KB per thread on x86_64-gos).

### `crates/k-shell/src/lib.rs`

Added `dispatch_graph_condensation(sink: &ConsoleSink)`:

- Header: `GRAPH CONDENSATION` (cyan on black).
- Summary line: `N components / M condensation edges / K nodes`.
- Per-SCC block (same layout as `graph scc`):
  - `C#i` label (showing "cycle" diamond for multi-node SCCs).
  - Member vector addresses (4 per row).
  - Node keys + plugin names.
  - Outgoing condensation edges: `→ C#j, C#k, …` (yellow arrow, cyan targets).
- Footer hint: `condensation is always a DAG | use 'graph scc' to see cycle details`.

### `crates/k-shell/src/proc.rs`

Added routing (after `graph scc`):
```
"graph condensation" | "condensation" | "condense" | "graph condense"
    → dispatch_graph_condensation(sink)
```

### `host-tests/gos-graph-condensation-harness/`

New harness crate — 10 integration tests, all green:

| # | Scenario | Expected |
|---|----------|----------|
| 1 | Empty graph | 0 components, 0 condensation edges |
| 2 | Single isolated node | 1 component, 0 condensation edges |
| 3 | Two-node mutual cycle A↔B | 1 component, 0 condensation edges |
| 4 | Linear chain A→B→C | 3 components, 2 condensation edges |
| 5 | Triangle cycle A→B→C→A | 1 component, 0 condensation edges |
| 6 | Triangle + outgoing edge to D | 2 components, 1 condensation edge |
| 7 | Diamond DAG (A→B, A→C, B→D, C→D) | 4 components, 4 condensation edges |
| 8 | Multiple parallel edges between same SCC pair | 1 condensation edge (dedup) |
| 9 | Chain with embedded cycle: A↔B, B→C, C→D | 3 components, 2 condensation edges |
| 10 | DAG invariant: no self-edges in condensation adj | Verified for mixed graph |

---

## Graph Algorithm Quartet — Complete

| Command | Algorithm | Since | POSIX analogue |
|---------|-----------|-------|----------------|
| `graph cycles` / `cycles` | DFS 3-color | V2.32 | `tsort` / cycle checkers |
| `graph toposort` / `tsort` | Kahn BFS | V2.33 | `tsort(1)` |
| `graph scc` / `scc` | Kosaraju 2-pass | V2.34 | `scc(1)` / `sccmap` |
| `graph condensation` / `condense` | Kosaraju + adj scan | V2.35 | `sccmap -F` / cargo inter-pkg |

---

## Test Results

```
gos-graph-condensation-harness:
  10 passed; 0 failed  ✓

gos-graph-scc-harness (regression):
  10 passed; 0 failed  ✓
```

**Host-test total: 353 tests** (all green)

---

## Invariants Maintained

- All dispatch functions are pure reads — no epoch bump, no write ops.
- `graph_condensation_inner` uses the same `TEST_LOCK + reset()` isolation pattern.
- Harness has its own `[workspace]` + `.cargo/config.toml` (`x86_64-pc-windows-msvc`).
- Condensation adjacency is bit-packed (one `u128` per SCC row), deduplicating parallel inter-SCC edges automatically.
- Self-loops in source graph: skip `from_slot == to_slot` in phase 3 (unchanged from SCC).
- Same-SCC edges: skip `fs == ts` in phase 3 — only cross-SCC edges contribute.

---

## Next Steps (V2.36+)

- `journal ring <N>` — runtime-configurable JournalRing capacity
- `node checkpoint <vec>` — snapshot node state to diff ring
- `graph path --all <to>` — multi-source path enumeration
- PAL_U32 → attribute node refactor (Demo A prerequisite)
