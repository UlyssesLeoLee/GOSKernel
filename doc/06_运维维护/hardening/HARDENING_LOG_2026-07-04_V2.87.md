# Hardening Log V2.87 — Eulerian Path/Circuit Detection

**Date:** 2026-07-04  
**Branch:** feat/vk-auto-live-surface  
**Commit:** c001568  
**Host-test total:** 843 (833 prior + 10 new)

---

## Feature: `graph eulerian` / `geulerian` / `eulerian` / `euler`

### Motivation

V2.85 and V2.86 added **structural fault-tolerance** primitives (cut vertices, cut edges).
V2.87 adds a complementary **traversal completeness** primitive: **Eulerian path/circuit
detection** — answering whether the directed kernel graph admits a walk that visits every
edge exactly once.

This is a classic result from graph theory (Euler 1736, the Königsberg bridge problem) with
direct OS relevance:

| Question | OS analogy |
|---|---|
| Eulerian circuit exists? | Can a maintenance daemon visit every IPC channel exactly once and return to base? |
| Eulerian path exists? | Can a single-pass audit traverse every dependency edge without retracing? |
| Neither? | The graph has isolated subsystem clusters or degree imbalance — routing is incomplete. |

In production graph platforms (NetworkX, igraph), Eulerian detection is a core primitive used
in circuit design, DNA assembly, and network audit scheduling.

---

## Algorithm: Degree Balance + Weak Connectivity (O(V+E))

Eulerian detection for directed graphs reduces to two O(V+E) checks:

### Step 1 — Degree Census

For each live node, compute `out_degree` and `in_degree` by scanning the edge table once.
Isolated nodes (out+in = 0) are excluded from all further checks.

### Step 2 — Degree Balance Classification

| Condition | Classification |
|---|---|
| All active nodes: `out == in` | Potential circuit |
| Exactly one node: `out - in = +1` (start), exactly one: `in - out = +1` (end), rest balanced | Potential path |
| Any node: `|out - in| ≥ 2`, or > 1 start/end candidate | Neither |

### Step 3 — Weak Connectivity Check

Undirected BFS from the first active node, treating every directed edge as undirected.
All active nodes must be reachable. If any active node is not reached, the result is
"neither" regardless of degree conditions.

**Key insight:** for directed graphs, the conditions are:

```
Eulerian circuit: ∀v: in_degree(v) == out_degree(v)  AND  weakly connected
Eulerian path:    ∃! s: out(s)−in(s)=1, ∃! t: in(t)−out(t)=1, ∀v≠s,t: balanced  AND  weakly connected
```

**Vacuous case:** If no edges exist, the empty walk trivially satisfies the circuit condition
(`has_circuit = true`, `has_path = false`). This applies to empty graphs and graphs with only
isolated nodes.

**Complexity:** O(V + E) — one edge scan for degrees + one undirected BFS.  
**Memory:** all arrays are stack-allocated; no heap, no_std safe.

---

## Return Value

```rust
pub fn graph_eulerian() -> (bool, bool, VectorAddress, VectorAddress, usize)
//                          has_circuit  has_path  start_vec  end_vec  node_count
```

| Field | Meaning |
|---|---|
| `has_circuit` | Eulerian circuit exists (closed walk over all edges) |
| `has_path` | Eulerian path exists (open walk); mutually exclusive with `has_circuit` |
| `start_vec` | Path start vertex vector; `VectorAddress::new(0,0,0,0)` if circuit or neither |
| `end_vec` | Path end vertex vector; `VectorAddress::new(0,0,0,0)` if circuit or neither |
| `node_count` | Total live nodes in the graph |

---

## Implementation

### crates/gos-runtime/src/lib.rs

**New method** on `GraphRuntime` (inside `impl GraphRuntime`):
```rust
pub fn graph_eulerian_inner(&self)
    -> (bool, bool, VectorAddress, VectorAddress, usize)
```

**New public function:**
```rust
/// V2.87: Eulerian path/circuit detection for the live kernel graph.
pub fn graph_eulerian() -> (bool, bool, VectorAddress, VectorAddress, usize) {
    RUNTIME.lock().graph_eulerian_inner()
}
```

**Internal arrays (all stack-allocated):**

| Array | Type | Purpose |
|---|---|---|
| `node_slots[MAX_NODES]` | `[usize; 128]` | Live node slot indices |
| `out_deg[MAX_NODES]` | `[u16; 128]` | Out-degree per slot |
| `in_deg[MAX_NODES]` | `[u16; 128]` | In-degree per slot |
| `active_slots[MAX_NODES]` | `[usize; 128]` | Non-isolated node slots |
| `visited[MAX_NODES]` | `[bool; 128]` | BFS visited flags |
| `bfs_queue[MAX_NODES]` | `[usize; 128]` | BFS queue (array-based) |

**Key invariants:**
- `active_count == 0` → vacuous circuit (early return before BFS).
- Degree diff uses `i32` arithmetic; `match diff` dispatches 0 / 1 / -1 / other cleanly.
- BFS uses undirected projection: both `from_node == cur_id` and `to_node == cur_id` edges followed.
- Self-loop guard: `nbr_slot == cur_slot` skipped in BFS.
- `circuit_degree_ok` and `path_degree_ok` are mutually exclusive: if `imbalanced == 0` then circuit; if `imbalanced == 2` with valid start/end then path.

### crates/k-shell/src/lib.rs

**New function** `dispatch_graph_eulerian(sink: &ConsoleSink)`:
- Header: ` graph eulerian` (cyan)
- If `node_count == 0`: prints `(no nodes registered)`.
- If `has_circuit`: prints green ✓ `Eulerian circuit exists` + note that any node is start/end.
- If `has_path`: prints yellow ✓ `Eulerian path exists (not a circuit)` + `start <vec>  end <vec>`.
- Otherwise: prints red ✗ `no Eulerian path or circuit` + diagnostic note.
- Footer: `nodes: N`

**Unicode used:**
- `\u{2713}` (✓) — success checkmark
- `\u{2717}` (✗) — failure crossmark
- `\u{2500}` (─) — horizontal rule

### crates/k-shell/src/proc.rs

**New routing** (inserted after `graph bridges` / `gcute` dispatch):
```
graph eulerian  →  dispatch_graph_eulerian
geulerian       →  alias
eulerian        →  alias
euler           →  alias
```

---

## Test Harness: `host-tests/gos-graph-eulerian-harness`

**VectorAddress L4=63** identifies this harness namespace.

| Test | Graph topology | Expected |
|---|---|---|
| 1 | Empty graph | has_circuit=true (vacuous), has_path=false |
| 2 | Single isolated node A (no edges) | has_circuit=true (vacuous), has_path=false |
| 3 | Triangle A→B→C→A | has_circuit=true (all balanced) |
| 4 | Single edge A→B | has_path=true, start=A, end=B |
| 5 | Path A→B→C | has_path=true, start=A, end=C |
| 6 | Anti-parallel A→B + B→A | has_circuit=true (both in=out=1) |
| 7 | Two disconnected edges A→B, C→D | neither (not weakly connected) |
| 8 | Square A→B→C→D→A | has_circuit=true (all balanced) |
| 9 | Lollipop: triangle A→B→C→A + tail C→D | has_path=true, start=C, end=D |
| 10 | Hub→A, Hub→B, C→Hub (two start/two end candidates) | neither (two start candidates) |

**Result:** 10/10 pass.

---

## VectorAddress L4 Namespace Update

| L4 | Harness |
|---|---|
| 61 | gos-graph-articulation-harness (V2.85) |
| 62 | gos-graph-bridges-harness (V2.86) |
| **63** | **gos-graph-eulerian-harness (V2.87)** |

---

## Key Graph Theory Facts

**Euler's theorem (1736):** An undirected connected graph has an Eulerian circuit if and only
if every vertex has even degree. This is the oldest result in graph theory — the resolution of
the Königsberg bridge problem.

**Directed Eulerian conditions (Hierholzer 1873):**
- **Circuit:** strongly connected (equivalently, weakly connected + all nodes balanced) AND
  ∀v: `in_degree(v) == out_degree(v)`.
- **Path:** weakly connected AND exactly one vertex with `out − in = 1` (source), exactly one
  with `in − out = 1` (sink), all others balanced.

**Relationship to other V2.x metrics:**

| Metric | Relationship |
|---|---|
| Bridges (V2.86) | A graph with any bridge cannot have an Eulerian circuit (removing a bridge disconnects) |
| Transitivity / clustering (V2.63/V2.75) | High clustering ≠ Eulerian; degree balance is the key |
| SCC (V2.34) | Eulerian circuit ↔ weakly connected + balanced; SCC-count > 1 blocks circuit but not always path |
| Degree centrality (V2.38) | Eulerian ↔ ∀v: `in_deg(v) == out_deg(v)` — pure degree condition |
| Girth (V2.69) | A DAG (girth=∞) can have Eulerian paths but never Eulerian circuits |

**Vacuous Eulerian:** The empty graph (no edges) trivially satisfies the circuit condition
because the universal quantifier `∀v: in == out` holds vacuously over zero active nodes.
This is standard in graph theory and matches NetworkX behaviour (`nx.is_eulerian(empty) → True`).

---

## Literature Reference

- L. Euler, "Solutio problematis ad geometriam situs pertinentis," Commentarii Academiae
  Scientiarum Imperialis Petropolitanae 8, 1736. The Königsberg bridge problem — first proof
  that an Eulerian circuit requires all even degrees.
- C. Hierholzer & C. Wiener, "Über die Möglichkeit, einen Linienzug ohne Wiederholung und
  ohne Unterbrechung zu umfahren," Mathematische Annalen 6, 1873. Proves the directed
  Eulerian conditions and provides a constructive O(E) circuit-finding algorithm.
