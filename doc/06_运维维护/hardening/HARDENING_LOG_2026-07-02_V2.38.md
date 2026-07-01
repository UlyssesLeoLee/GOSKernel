# GOS Hardening Log — V2.38 — 2026-07-02

## Summary

V2.38 adds in/out degree census via `graph degree` — answering "how connected
is each node and which are the traffic hubs?"  For every live node the command
counts directed out-degree (edges leaving the node) and in-degree (edges
entering the node), then presents the result sorted by descending total degree
so the most-connected hubs appear first.

Nodes are automatically annotated with a role label:
- **hub** — total-degree ≥ 3 AND ≥ ceiling(max_total/2): the most-connected nodes.
- **source** — no incoming edges (out > 0, in == 0): signal originators.
- **sink** — no outgoing edges (out == 0, in > 0): terminal consumers.
- **isolated** — no edges at all (out == 0, in == 0): disconnected nodes.

OS analogies: `ip -s link show` (per-interface TX/RX packet counts),
`netstat -s` (per-socket statistics), or `ss -s` broken down by address.

---

## Changes

### 1. `graph_degree_inner<N>` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

New method on `GraphState` (inserted after `graph_bipartite_inner`):

```rust
pub fn graph_degree_inner<const N: usize>(
    &self,
) -> ([VectorAddress; N], [u16; N], [u16; N], usize)
```

**Algorithm**: O(V × E) census — scan all live edges once, resolving both
`from_node` and `to_node` slots to accumulate slot-indexed degree counters.
Then insertion-sort the live-node slot list by descending total degree.
O(V × E) is acceptable for V ≤ 128, E ≤ 512.

**Self-loops** count once toward both in-degree and out-degree (the edge
`from_node == to_node` increments `slot_out[slot]` and `slot_in[slot]`
independently), matching standard directed-graph degree conventions.

**Saturation**: degrees use `u16` with `saturating_add` to avoid overflow
even if a node accumulates more than 65535 edges.

**Return layout** (consistent with prior graph_* convention):
- `vecs[0..total]`        — live node vectors, descending total-degree order.
- `out_degrees[0..total]` — directed out-degree per node.
- `in_degrees[0..total]`  — directed in-degree per node.
- `total`                 — number of live nodes packed.

### 2. `pub fn graph_degree<const N>()` — gos-runtime public API

```rust
pub fn graph_degree<const N: usize>() -> ([VectorAddress; N], [u16; N], [u16; N], usize) {
    RUNTIME.lock().graph_degree_inner()
}
```

One-liner wrapper consistent with `graph_scc`, `graph_condensation`,
`graph_reachable`, `graph_bipartite`.

### 3. `dispatch_graph_degree` — k-shell (`crates/k-shell/src/lib.rs`)

New shell-level dispatch function (inserted before `dispatch_uname`).

Output format (colour-coded: green=out, red=in, yellow=hub, cyan=sink):
```
 graph degree
 ───────────────────────────────────────────────────────────
  vector           out    in   total  role
  6.1.0.0            3     2      5  hub
  1.1.0.0            2     1      3  hub
  2.1.0.0            0     2      2  sink
  7.1.0.0            1     0      1  source
  5.1.0.0            0     0      0  isolated
 ───────────────────────────────────────────────────────────
  5 node(s)  max-total-degree: 5  hubs: 2
```

Pure read — no epoch bump, no write ops.

### 4. Command routing — k-shell (`crates/k-shell/src/proc.rs`)

Aliases wired after the `graph bipartite` branch:

```
graph degree  |  degree  |  graph hub  |  hub
```

Help text updated with description and aliases.

### 5. `gos-graph-degree-harness` — new host-test crate

`host-tests/gos-graph-degree-harness/` — 10 integration tests covering:

| # | Scenario | Expected |
|---|----------|----------|
| 1 | Empty graph | total=0, no panics |
| 2 | Single isolated node | out=0, in=0 |
| 3 | Single edge A→B | A: out=1 in=0; B: out=0 in=1 |
| 4 | Path A→B→C | B has highest total-degree (2); appears first |
| 5 | Self-loop A→A | A: out=1, in=1 (counts both sides) |
| 6 | Fan-out hub H→{A,B,C} | H: out=3 in=0; appears first |
| 7 | Fan-in  hub {A,B,C}→H | H: out=0 in=3; appears first |
| 8 | Bidirectional A⇄B | each node: out=1, in=1 |
| 9 | Sort order verified | output strictly non-increasing by total-degree |
| 10 | Diamond A→B,A→C,B→D,C→D | all four nodes total=2; exact out/in per node verified |

All 10 tests: **PASS**.

---

## Test Results

```
running 10 tests
test bidirectional_edge_symmetric_degrees ... ok
test diamond_topology_degree_census ... ok
test empty_graph_degree_total_is_zero ... ok
test fan_in_hub_highest_degree ... ok
test fan_out_hub_highest_degree ... ok
test self_loop_counts_both_in_and_out ... ok
test output_sorted_descending_total_degree ... ok
test path_middle_node_highest_degree ... ok
test isolated_node_has_zero_degree ... ok
test single_edge_degrees ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

---

## Shell Command Surface (V2.38 additions)

| Command | Aliases | Description |
|---------|---------|-------------|
| `graph degree` | `degree`, `graph hub`, `hub` | In/out degree census sorted by descending total degree; hub/source/sink/isolated role annotation |

---

## Invariants Preserved

- `dispatch_graph_degree` is a pure read — no epoch bump, no write ops.
- Uses existing `TEST_LOCK: Mutex<()>` + `reset()` isolation pattern.
- Harness `.cargo/config.toml` sets `target = "x86_64-pc-windows-msvc"` +
  `build-std = ["std", "panic_abort"]`.
- Version number: V2.38 (sequential after V2.37 graph-bipartite).
- Node degree arrays use `u16` with `saturating_add` for overflow safety.

---

## Next Steps

- `node checkpoint <vec>` — snapshot node state to diff ring (observability)
- `journal ring <N>` — runtime-configurable JournalRing capacity
- `graph centrality` — betweenness / closeness centrality computation
- PAL_U32 → attribute node refactor (Demo A prerequisite)
