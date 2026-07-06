# Hardening Log V3.07 — Vertex Connectivity κ(G)

**Date**: 2026-07-06  
**Branch**: feat/vk-auto-live-surface  
**Commit**: 3fd566f  
**Previous baseline**: V3.06 (edge betweenness centrality, 1033 host tests)  
**New total**: 1043 host tests (+10)

---

## Algorithm: Vertex Connectivity κ(G)

The **vertex connectivity** κ(G) of a graph G is the minimum number of vertices whose removal disconnects G (or makes it trivial). It is one of the most fundamental robustness metrics in graph theory.

### Theoretical Background

**Whitney's theorem (1932)**:

```
κ(G) ≤ κ'(G) ≤ δ(G)
```

where κ'(G) is edge connectivity and δ(G) is minimum degree. This provides a natural cross-validation: κ can never exceed the min degree.

**Menger's theorem (1927)**: κ(G) equals the maximum number of internally vertex-disjoint paths between any two non-adjacent vertices (where there exist non-adjacent vertices).

**Even's algorithm (1975)**: Fix s = argmin(degree). Then:
- κ(G) = 0 if G is disconnected
- κ(G) = n−1 if G = Kₙ (complete graph)
- κ(G) = min over all t non-adjacent to s of max-vertex-disjoint-paths(s, t)

The key insight: fixing the minimum-degree vertex s is sufficient. Any vertex separator must include all neighbors of s (of which there are δ(G)), so we only need to check non-neighbors.

### Node-Split Network Transform

To compute vertex-disjoint paths via max-flow, each internal vertex ci (≠ s, ≠ t) is split into two virtual nodes:
- ci_in  = 2·ci
- ci_out = 2·ci + 1
- Internal edge ci_in → ci_out with capacity 1 (enforces vertex disjointness)

Cross edges (from ci_out to cj_in for each original edge ci–cj) carry capacity INF (=127, since max flow ≤ δ ≤ 126).

Source s and sink t: only one virtual node each (s_out = 2·s+1, t_in = 2·t), with no internal capacity constraint.

### Implementation Details

**Constants** (no_std, stack-only):
- ME = 2560 (edge slots), MV = 256 (virtual node slots)  
- MAX_NODES=128 → at most 256 virtual nodes, ≤2·128·128=32768 cross edges, but bounded by real edges
- Actual worst case: 2·MAX_EDGES cross edges + 2·MAX_NODES internal edges ≤ 2·512 + 256 = 1280 < 2560 ✓

**Stack usage**: ef[2560] + et[2560] + ec[2560] (u8 arrays) + BFS arrays ≈ 8 KB, well within 16 KB limit.

**ei^1 backward edge trick**: Forward edges at even indices (ne starts at 0, always incremented by 2), so `ef[ei^1]` is always the backward edge of `ef[ei]`. No HashMap needed.

**Edmonds-Karp BFS**: Unit-capacity internal edges mean each augmenting path adds exactly 1 to flow. Total augmentation rounds ≤ κ ≤ δ ≤ 127.

### New API

```rust
// gos-runtime/src/lib.rs
pub fn graph_vertex_connectivity<const N: usize>(
) -> ([VectorAddress; N], usize, u32, u32)
// Returns: (sorted node addresses, node_count, kappa, min_degree)
```

### K-Shell Integration

- **Command aliases**: `graph kappa`, `gkappa`, `vertex connectivity`, `vertex conn`, `gvertconn`, `graph vertex conn`, `graph vconn`
- **Display**: bright-cyan header (color 11), bright-green node list (color 10)
- **Footer**: `κ(G)=N  δ(G)=M  Whitney: κ≤δ  Menger 1927`

---

## New Harness: gos-graph-vconn-harness

**Location**: `host-tests/gos-graph-vconn-harness/`  
**L4 namespace**: 83  
**Plugin**: `KL_GRAPH_VCON_H`  
**Executor**: `vconn.exec`

### Test Cases (10 total)

| # | Graph | n | Expected κ | Notes |
|---|-------|---|------------|-------|
| 01 | Empty | 0 | 0 | No nodes → disconnected |
| 02 | Single node | 1 | 0 | Single node → trivial |
| 03 | K₂ | 2 | 1 | One edge, removing either vertex disconnects |
| 04 | Path A–B–C | 3 | 1 | B is cut vertex |
| 05 | C₄ (4-cycle) | 4 | 2 | Must remove 2 vertices to disconnect |
| 06 | K₄ | 4 | 3 | Complete graph: κ = n−1 |
| 07 | Star K₁,₄ | 5 | 1 | Removing centre disconnects all leaves |
| 08 | Hourglass | 5 | 1 | Two triangles sharing vertex A; A is cut vertex |
| 09 | Disconnected | 4 | 0 | Two isolated edges: already disconnected |
| 10 | K₃,₃ | 6 | 3 | Complete bipartite; Whitney-tight: κ = δ = 3 |

All 10 tests pass. Exit code 0.

---

## Files Modified

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | +`graph_vertex_connectivity_inner`, +`vertex_conn_maxflow`, +`graph_vertex_connectivity` |
| `crates/k-shell/src/lib.rs` | +`dispatch_graph_vertex_connectivity` |
| `crates/k-shell/src/proc.rs` | +routing for `graph kappa` / `gkappa` / `graph vconn` etc. |
| `host-tests/gos-graph-vconn-harness/` | new harness (Cargo.toml, .cargo/config.toml, tests/graph_vconn.rs) |

---

## OS Analogy

Vertex connectivity maps to **fault tolerance** in OS design:

- A kernel with κ=0 is already partitioned — no inter-component communication possible.
- κ=1 means a single critical component (like a single scheduler or memory allocator) whose failure isolates the system.
- Higher κ graphs represent redundant, resilient architectures — analogous to multi-path I/O, RAID, or replicated state machines.

Computing κ(G) on the GOSKernel's runtime graph gives an instant robustness score for the current topology.
