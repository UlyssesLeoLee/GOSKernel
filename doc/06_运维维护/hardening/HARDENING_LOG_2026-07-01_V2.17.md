# GOS Hardening Log — V2.17 — 2026-07-01

## Summary

V2.17 adds an L4-domain topology inspection command (`graph topo` / `graph topo <L4>`)
and two new gos-runtime APIs (`node_count_for_l4`, `node_page_l4`), bringing graph
namespace introspection up to the level of Linux's `ip route show` / `lshw -short`.

---

## Changes

### 1. Two new APIs — gos-runtime (`crates/gos-runtime/src/lib.rs`)

#### `RuntimeState::node_count_for_l4(l4: u8) -> usize`
Private impl method — single-pass count of nodes whose `vector.l4 == l4`.

#### `RuntimeState::node_page_l4<const N>(l4: u8, offset: usize, out: &mut [GraphNodeSummary; N]) -> (usize, usize)`
Private impl method — returns a sorted page of `GraphNodeSummary` entries filtered
to a specific l4 domain.  Uses `refresh_node_order()` so results are sorted by vector
address (same invariant as `node_page`).  Skips `offset` matching entries before
filling the page.  Returns `(total_in_domain, filled)`.

#### Public module-level exports (after `proc_stat_for_vector`)
```rust
pub fn node_count_for_l4(l4: u8) -> usize { RUNTIME.lock().node_count_for_l4(l4) }
pub fn node_page_l4<const N: usize>(l4: u8, offset: usize, out: ...) -> (usize, usize) { ... }
```

### 2. `graph topo` shell command — k-shell (`crates/k-shell/src/lib.rs`, `crates/k-shell/src/proc.rs`)

#### `dispatch_graph_topo(sink, l4_filter: Option<u8>)`

**Overview mode** (`graph topo` / `topo`):
- Pages through all live nodes via `node_page`, buckets by `l4` using a local
  `[u8; 64]` / `[usize; 64]` pair (no heap, no 256-entry table).
- Insertion-sorts the domain list by l4 value for deterministic output.
- Prints one line per non-empty domain: `[l4=N]  K node(s)`.
- Footer: domain count + grand total + hint to use `graph topo <l4>`.

**Domain detail mode** (`graph topo <L4>` / `topo <L4>`):
- Calls `node_page_l4` with the given l4 value, pages until exhausted.
- Prints each node: vector (padded to 16 chars), lifecycle label, plugin/key.
- Footer: total node count in that domain.

Shell dispatch additions in `proc.rs`:
- Exact match `"graph topo"` and `"topo"` → `dispatch_graph_topo(sink, None)`.
- Prefix `"graph topo <L4>"` / `"topo <L4>"` → reuses `parse_epoch_decimal()`,
  validates 0–255, then calls `dispatch_graph_topo(sink, Some(l4))`.
- Help text updated with two new entries.

### 3. Test harness — `host-tests/gos-graph-topo-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `empty_graph_count_for_l4_returns_zero` | Empty runtime → count_for_l4(5) == 0 |
| 2 | `single_node_counted_in_correct_l4_domain` | Node at l4=5 → count_for_l4(5) == 1 |
| 3 | `node_not_counted_in_wrong_l4_domain` | Node at l4=5 → count_for_l4(6) == 0 |
| 4 | `two_nodes_same_l4_counted_correctly` | 2 nodes at l4=3 → count_for_l4(3) == 2 |
| 5 | `two_nodes_different_l4_each_counted_separately` | l4=5 and l4=6 → each count == 1 |
| 6 | `node_page_l4_returns_only_matching_domain` | l4=5 filter excludes l4=6 node |
| 7 | `node_page_l4_empty_domain_returns_zero` | Filter l4=99 → (0, 0) |
| 8 | `node_page_l4_total_matches_count_api` | page total == node_count_for_l4 |
| 9 | `node_page_l4_offset_skips_correctly` | 3 nodes, offset=1 → filled==2 |
|10 | `node_page_l4_caps_filled_at_page_size` | 5 nodes, PAGE=2 → total=5, filled=2 |

---

## Verification

```
cd host-tests/gos-graph-topo-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

Kernel build:
```
cargo build --release
# Finished `release` profile [optimized]
```

Total host-test suite: **173 tests** (163 V2.16 + 10 new)

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.17 |
|---|---|---|
| Network topology view | `ip route show` / `ip link show` | `graph topo` / `graph topo <l4>` |
| Hardware device tree | `lshw -short` | `graph topo` with domain breakdown |
| Per-subsystem node listing | `ip link show type veth` | `graph topo 6` (shell domain nodes) |
| Domain-scoped node count | `ip -s link` counters | `node_count_for_l4(l4)` |

The `graph topo` command makes the GOS vector address namespace directly observable
from the operator surface, completing the observability trio:
- **what runs** → `proc` (nodes × signals × edges)
- **how they connect** → `edges` (edge topology)
- **where they live** → `graph topo` (domain namespace distribution)

---

## Graph-OS Characteristic Preserved

`graph topo` is unique to GOS — no POSIX equivalent exists because conventional OSes
have no concept of a hierarchical vector address namespace.  The l4 domain byte is
GOS's top-level namespace partition (similar to a BGP AS number), and `graph topo`
makes this partition boundary the primary lens for understanding system structure.

---

*Automated hardening pass — GOS V2.17 — 2026-07-01*
