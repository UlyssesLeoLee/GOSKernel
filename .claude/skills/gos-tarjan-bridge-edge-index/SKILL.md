---
name: gos-tarjan-bridge-edge-index
description: When implementing Tarjan bridge (cut-edge) detection in GOSKernel, track the parent by edge-index (par_ei), NOT by parent-slot — and use strict > for the bridge condition (not >=). Anti-parallel edges A→B + B→A must NOT be treated as a bridge; edge-index tracking makes the reverse a visible back-edge. Apply in crates/gos-runtime/src/lib.rs graph_bridges_inner.
---

# Tarjan Bridge Detection: Edge-Index Parent + Strict `>` Condition

## The rule

Bridge detection differs from articulation points in two critical ways:

**1. Track parent by edge-index, not parent-slot:**
```rust
// WRONG for bridges — skips ALL edges to parent slot
let parent_guard = nbr_slot != par_slot[cur_slot];

// CORRECT for bridges — skips only the specific arrival edge
let parent_guard = ei != par_ei[cur_slot];
```

**2. Bridge condition uses strict `>` (not `>=`):**
```rust
// Articulation point (non-root): low[child] >= disc[parent]
if low[cur_slot] >= disc[p] && par[p] != NO_PAR { is_ap[p] = true; }

// Bridge: low[child] > disc[parent]  (strict >)
if low[cur_slot] > disc[p] { /* emit bridge (p, cur_slot) */ }
```

**3. No root special case for bridges:**
- Articulation points require a separate `dfs_children[root] >= 2` check after each DFS tree.
- Bridges have NO root special case — the condition `low[child] > disc[parent]` applies uniformly.

## Why it's non-obvious

**The anti-parallel trap:** In a directed graph treated as undirected, two edges A→B and
B→A form a single undirected path A-B. They are NOT a bridge. But if bridge detection
uses parent-slot tracking (`par_slot`), when DFS is at B (arrived via A→B), it skips
ALL edges to slot A — including B→A. That leaves B with no back-edge to A, so
`low[B] = disc[B] > disc[A]` → false bridge emitted.

With edge-index tracking (`par_ei`), B skips only the specific edge `A→B` (by index).
The edge `B→A` (different index) is seen as a back-edge: `low[B] = min(low[B], disc[A])`,
so `low[B] <= disc[A]` → no bridge. Correct.

**The existing articulation-point skill documents the OPPOSITE:** `gos-tarjan-articulation-iterative`
notes "if both directed edges A→B and B→A exist, both are correctly skipped (both resolve
to parent slot)." This is correct for APs but WRONG for bridges — the two algorithms
diverge exactly here.

**Strict vs non-strict:** `>` vs `>=` — a single character difference with a meaningful
semantic distinction:
- `>=` (AP): the child's subtree can't reach past the parent, making the parent a single point
  of failure for vertex removal.
- `>` (bridge): the child's subtree can't even reach the parent itself, making the edge a
  single point of failure for edge removal. Equality (`low[child] == disc[parent]`) means
  the subtree CAN reach the parent (but not higher) — so the edge is not a bridge.

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_bridges_inner<N>` (V2.86)
- Public wrapper: `gos_runtime::graph_bridges::<128>()` (no RUNTIME snapshot — uses `&self`)
- Returns `([VectorAddress; N], [VectorAddress; N], usize, usize)` = (from_vecs, to_vecs, bridge_count, node_count)
- Each bridge canonicalized: `from = min(a, b)` by `as_u64()`, sorted by `(from, to)` ascending
- Shell: "graph bridges" / "gbridges" / "cut edges" / "gcute"
- VectorAddress L4=62 reserved for gos-graph-bridges-harness test nodes
- Undirected projection: both endpoint directions followed for each edge

## State arrays (bridges vs APs)

| Array | Articulation (V2.85) | Bridge (V2.86) |
|---|---|---|
| `par[slot]` | parent **slot** | — (not used for guard) |
| `par_ei[slot]` | — (not used) | arrival **edge index** |
| `par_slot[slot]` | same as par[] | parent slot (emit only) |
| `dfs_children[]` | needed (root check) | not needed |
| `is_ap[]` | bool flag array | — (emit immediately on pop) |

## From this session

V2.86: Test 6 (`test_06_antiparallel_not_a_bridge`) specifically validates that A→B + B→A
produces bridge_count=0. If edge-index tracking is replaced with slot tracking, this test
fails with bridge_count=1 (false positive). The test was designed specifically to catch this.

Test 9 (`test_09_two_triangles_one_bridge`) confirms the strict `>` condition: inside each
triangle, all edges have `low[child] == disc[parent]` (cycle back-edge reaches parent) →
not bridges. Only the inter-triangle edge C→F has `low[F] > disc[C]` → bridge.
