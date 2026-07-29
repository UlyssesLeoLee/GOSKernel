---
name: gos-bfs-pairwise-triple-accumulation
description: When implementing multiple distance-based topological indices together (Wiener W, Harary H, Hyper-Wiener WW) in GOSKernel, run a single BFS per source with u8 dist/INF=255, accumulate all three indices in one pass over pairs (src < v), using d*(d+1)/2 for WW (always integer) and 1_000_000/d for H (floor ppm). Apply in graph_topo_indices7_inner.
---

# BFS Pairwise Triple Accumulation (W + H + WW)

## The rule

To compute Wiener W, Harary H (ppm), and Hyper-Wiener WW simultaneously from one BFS pass:

```rust
const INF: u8 = 255;
let mut dist  = [INF; MAX_NODES];
let mut queue = [0u8; MAX_NODES];

let mut wiener:        u64 = 0;
let mut harary_ppm:    u64 = 0;
let mut hyper_wiener:  u64 = 0;

for src in 0..nc {
    for i in 0..nc { dist[i] = INF; }
    dist[src] = 0;
    let mut qhead = 0usize;
    let mut qtail = 0usize;
    queue[qtail] = src as u8; qtail += 1;
    while qhead < qtail {
        let cur   = queue[qhead] as usize; qhead += 1;
        let d_cur = dist[cur];
        let mut bits = adj[cur];          // undirected bitmask
        while bits != 0 {
            let nb = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            if dist[nb] == INF {
                dist[nb] = d_cur + 1;
                queue[qtail] = nb as u8; qtail += 1;
            }
        }
    }
    for v in (src + 1)..nc {             // only upper triangle: no double-count
        let d8 = dist[v];
        if d8 == INF { continue; }       // disconnected pair: skip
        let d = d8 as u64;
        wiener       += d;
        harary_ppm   += 1_000_000 / d;  // floor(10^6 / d)
        hyper_wiener += d * (d + 1) / 2; // always integer: d*(d+1) is even
    }
}
```

## Why it's non-obvious

**u8 dist with INF=255 works**: max BFS distance in a MAX_NODES=128 graph is 127, so u8 is sufficient. INF=255 is a safe sentinel. This saves 4× memory vs u32 and fits the BFS queue in u8 too.

**d*(d+1)/2 is always integer**: d and d+1 are consecutive integers, so their product is always even. No floor division needed — this is an exact integer formula for Hyper-Wiener per-pair contribution.

**Wiener vs Hyper-Wiener for complete graphs**: W(K_n) = WW(K_n) = n*(n-1)/2. This is because all d=1 → d*(d+1)/2 = 1*(2)/2 = 1 = d. For non-complete graphs, WW > W because d² amplifies long distances.

**Upper-triangle loop `(src+1)..nc`**: accumulate pairs (src < v) only, not all directed pairs. This avoids double-counting the undirected pair {u,v}. BFS is still run from every node (needed for asymmetric distances in directed projection), but accumulation is one-sided.

**Harary is H_ppm ≈ W_ppm for complete graphs**: When all d=1, H = n*(n-1)/2 = W, and H_ppm = W × 10^6. For irregular graphs H_ppm < W × 10^6 because floor(10^6/d) < 10^6 for d>1.

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_topo_indices7_inner()` (V3.18)
- Public export: `graph_topo_indices7() -> (wiener: u64, harary_ppm: u64, hyper_wiener: u64, edge_count: usize, node_count: usize)`
- Shell commands: "graph topo7" / "gtopo7" / "wiener index" / "harary index" / "hyper wiener"
- VectorAddress L4=94 for gos-graph-topo7-harness
- Predecessor (directed Wiener, single index): `gos-wiener-sum-distances-pattern` (V2.70)
- This pattern (undirected, triple index): graph_topo_indices7_inner (V3.18)

## From this session

V3.18: implemented W + H + WW together in one BFS pass. All 10 harness tests passed on first compile with correct values. Key cross-checks validated:
- K₄: W=H_ppm/10^6=WW=6 (complete graph invariant ✓)
- P₄: W=10=4*(16-1)/6 (path formula ✓), H_ppm=4_333_333, WW=15
- K_{1,4}: W=16, H=7_000_000, WW=22 (4 center-leaf d=1 + 6 leaf-leaf d=2 ✓)
- Two isolated nodes: W=H=WW=0 (disconnected pair excluded ✓)
