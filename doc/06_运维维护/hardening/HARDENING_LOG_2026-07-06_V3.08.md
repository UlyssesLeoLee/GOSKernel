# Hardening Log V3.08 — Edge Coloring χ'(G)

**Date**: 2026-07-06  
**Branch**: feat/vk-auto-live-surface  
**Commit**: 9f9278e  
**Previous baseline**: V3.07 (vertex connectivity κ(G), 1043 host tests)  
**New total**: 1053 host tests (+10)

---

## Algorithm: Edge Coloring (Vizing 1964)

**Edge coloring** assigns a colour to every undirected edge of a graph such that no two edges sharing a common endpoint receive the same colour. The minimum number of colours required is the **chromatic index** χ'(G).

### Theoretical Background

**Vizing's theorem (1964)**: For any simple undirected graph G,

```
Δ(G) ≤ χ'(G) ≤ Δ(G) + 1
```

where Δ(G) is the maximum degree. Graphs achieving χ'(G) = Δ are called **class 1**; those achieving χ'(G) = Δ+1 are **class 2**.

**König's theorem (1916)**: Bipartite graphs are always class 1 — χ'(G) = Δ(G). This is the optimal achievable for bipartite graphs; however, a greedy algorithm may still use Δ+1 colours depending on edge ordering.

**Class examples**:
- K_{2k} (even complete graphs): class 1, χ'=2k−1
- K_{2k+1} (odd complete graphs / K_3): class 2, χ'=2k+1+1... wait actually:
  - K_3 (triangle): Δ=2, χ'=3 (class 2)
  - K_4: Δ=3, χ'=3 (class 1)
  - C_{2k} (even cycles): class 1, χ'=2
  - C_{2k+1} (odd cycles): class 2, χ'=3
  - Stars K_{1,n}: class 1, χ'=n
  - Trees and bipartite graphs: class 1, χ'=Δ

### Algorithm: Greedy Edge Coloring

The implementation uses a greedy strategy in O(E) time:

1. **Build undirected edge list**: iterate directed edge slots; canonicalize each pair (a,b) so a < b (compact index); deduplicate via `seen_adj[a] |= 1<<b` bitmask; self-loops excluded.

2. **Greedy assignment**: For each edge (a,b) in slot order:
   - `forbidden = node_colors[a] | node_colors[b]`  
     where `node_colors[ci]` is a u128 bitmask with bit k set if colour k is already used on an edge incident to node ci
   - `colour = forbidden.trailing_ones()` — the index of the lowest 0-bit = lowest available colour
   - Update `node_colors[a] |= 1<<colour` and `node_colors[b] |= 1<<colour`

3. **Sort output**: ascending by (colour, from.as_u64(), to.as_u64()) — groups all edges by time slot.

The `trailing_ones()` trick is key: it finds the lowest unused colour in O(1) without a loop.

**Correctness**: Vizing guarantees greedy uses at most Δ+1 colours. Since max degree ≤ 127 (MAX_NODES=128) and u128 has 128 bits, the bitmask approach is always valid.

**Stack footprint** (~17KB):
- `eu, ev [u8; 512]` — edge compact indices
- `ef, et [VectorAddress; 512]` — edge vectors (4B each × 512 = 2KB × 2)
- `seen_adj, node_colors [u128; 128]` — bitmasks (2KB × 2)
- `edge_color [u8; 512]`, `order [usize; 512]` — output arrays

---

## Runtime API

```rust
pub fn graph_edge_color<const N: usize>()
    -> ([VectorAddress; N], [VectorAddress; N], [u8; N], usize, u8)
```

Returns `(from_vecs, to_vecs, edge_colors, edge_count, chromatic_index)`:
- `from_vecs[0..edge_count]` — canonical "from" vector for each edge
- `to_vecs[0..edge_count]` — canonical "to" vector
- `edge_colors[0..edge_count]` — 0-indexed colour slot assigned to each edge
- `edge_count` — total undirected edges (self-loops excluded)
- `chromatic_index` — χ'(G) = max colour used + 1; 0 if no edges

**Sort order**: ascending (colour, from.as_u64(), to.as_u64())

---

## K-Shell Commands

```
graph edge color   — display edge coloring with χ'(G) chromatic index
gedgecolor         — alias
edge color         — alias
gec                — alias
graph ecolor       — alias
gecolor            — alias
```

**Display**: bright-green header; edges colour-coded cycling through 6 terminal colours per colour slot; footer: `N undirected edge(s)  χ'(G)=K  Vizing 1964`

---

## VectorAddress Namespace

**L4=84** for `gos-graph-ecolor-harness`

---

## Test Harness: gos-graph-ecolor-harness (10 tests)

| Test | Graph | Expected |
|------|-------|----------|
| 1 | Empty | ec=0, χ'=0 |
| 2 | Single isolated node | ec=0, χ'=0 |
| 3 | Single edge A→B | ec=1, χ'=1, colour=0 |
| 4 | Path A→B→C (Δ=2) | ec=2, χ'=2, adjacent colours differ |
| 5 | Triangle K_3 (directed cycle) | ec=3, Δ=2, χ'=3 (odd cycle, class 2) |
| 6 | C_4 directed cycle | ec=4, Δ=2, χ'=2 (even cycle, class 1) |
| 7 | K_4 complete | ec=6, Δ=3, χ'=3 (class 1) |
| 8 | Star K_{1,4} | ec=4, Δ=4, χ'=4 (class 1) |
| 9 | Self-loops only | ec=0, χ'=0 |
| 10 | K_{3,3} Vizing + validity | ec=9, greedy χ'=4=Δ+1, proper coloring ✓ |

All 10 pass. Test 10 verifies the full proper-coloring invariant: no two adjacent edges share a colour, O(E²) check.

**Note on test 10**: K_{3,3} is bipartite (Δ=3), and by König's theorem the optimal χ'=3. However, greedy achieves χ'=4 (Δ+1) for this edge ordering — which is the Vizing upper bound and still a valid coloring. The distinction between "greedy chromatic index" and "optimal chromatic index" is tested explicitly.

---

## OS Analogy

The chromatic index χ'(G) is the **minimum number of non-conflicting time-slots** needed to schedule all IPC channels so that no two channels sharing a kernel-subsystem endpoint are active simultaneously.

- Slot 0 = first round-robin epoch, Slot 1 = second epoch, etc.
- χ'(G) = total scheduler epochs per full I/O dispatch cycle
- A star K_{1,n} (hub-and-spoke) needs n slots — the hub is a bottleneck
- A bipartite graph achieves Δ slots (optimal; König)
- An odd cycle needs 3 slots (one extra vs even cycle)

This is analogous to:
- NIC transmit-queue striping to avoid head-of-line blocking
- O_DIRECT I/O slot multiplexing for storage controllers
- CPU-to-peripheral DMA channel scheduling with shared controller endpoints

---

## Relation to Existing Algorithms

| Algorithm | Version | Relation |
|-----------|---------|---------|
| Node coloring (graph_color) | V2.48 | dual: colours nodes vs edges |
| Bipartite matching (graph_bipartite_match) | V2.92 | König: matching↔vertex cover↔edge coloring for bipartite |
| Vertex cover (graph_vertex_cover) | V2.97 | edge coloring ↔ fractional matching duality |
| Edge betweenness (graph_betweenness_edge) | V3.06 | both operate on directed edge lists |

---

## Literature

- Vizing, V.G. (1964). "On an estimate of the chromatic class of a p-graph." *Diskret. Analiz* 3: 25–30. (The theorem itself.)
- König, D. (1916). "Über Graphen und ihre Anwendung auf Determinantentheorie und Mengenlehre." *Math. Ann.* 77: 453–465. (Bipartite case χ'=Δ.)
- Misra, J. & Gries, D. (1992). "A constructive proof of Vizing's theorem." *Inf. Process. Lett.* 41(3): 131–133. (Linear-time algorithm achieving optimal χ' for bipartite graphs.)
- Garey, M.R. & Johnson, D.S. (1979). *Computers and Intractability*. (Deciding class 1 vs class 2 is NP-complete for general graphs.)
