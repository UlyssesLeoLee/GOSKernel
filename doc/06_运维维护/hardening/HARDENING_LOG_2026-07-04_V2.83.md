# Hardening Log V2.83 — Graph Metric Snapshot Save & Compare

**Date:** 2026-07-04  
**Branch:** feat/vk-auto-live-surface  
**Commit:** 1c2d26a  
**Host-test total:** 803 (793 prior + 10 new)

---

## Feature: `graph snapshot` / `graph compare`

### Motivation

Production OS monitoring systems (Linux `sysstat`, Windows Performance Monitor, iOS MetricKit)
provide the ability to capture a system metric baseline and compare it against current state
to detect drift. GOSKernel lacked this capability — operators could see current topology
metrics but could not determine how the graph had changed since a known-good state.

V2.83 adds a **metric snapshot baseline** system analogous to `sar -o snap.bin` (save) and
`sar -s <start> -e <end>` (compare) from Linux sysstat.

---

## Implementation

### gos-runtime/src/lib.rs

**New public type:**
```rust
#[derive(Copy, Clone)]
pub struct MetricSnapshot {
    pub valid: bool,       // false until first graph_snapshot_save()
    pub epoch: u64,        // graph_epoch at time of save
    pub node_count: usize,
    pub edge_count: usize,
    pub density_ppm: u32,  // graph density × 1_000_000
    pub trans_ppm: u32,    // global transitivity × 1_000_000
    pub avgcc_ppm: u32,    // avg clustering (WS) × 1_000_000
    pub geff_ppm: u64,     // global efficiency × 1_000_000
    pub leff_ppm: u32,     // local efficiency × 1_000_000
    pub sigma_ppm: u32,    // small-world σ × 1_000_000 (0=undef)
    pub kappa_ppm: u32,    // scale-free κ × 1_000_000 (0=undef)
    pub gamma_ppm: u32,    // power-law γ̂ × 1_000_000 (0=undef)
}
```

**New static:**
```rust
static METRIC_SNAPSHOT: Mutex<MetricSnapshot> = Mutex::new(MetricSnapshot { valid: false, ... });
```

**New private method** (`impl GraphRuntime`):
- `graph_snapshot_inner(&self) -> MetricSnapshot`  
  Calls all 8 `*_inner()` methods inside one `RUNTIME.lock()` hold for epoch-consistency.

**New public functions:**
- `pub fn graph_snapshot_save() -> u64`  
  Captures current metrics; stores in `METRIC_SNAPSHOT`; returns graph_epoch at capture time.
- `pub fn graph_snapshot_compare() -> (MetricSnapshot, MetricSnapshot)`  
  Returns `(saved, current)` — current is always computed live.

### crates/k-shell/src/lib.rs

**`dispatch_graph_snapshot(sink)`** — runs `graph_snapshot_save()`, prints the saved
baseline (epoch, nodes, edges, density, CC, efficiency, σ, κ, γ̂) with confirmation footer.

**`dispatch_graph_compare(sink)`** — runs `graph_snapshot_compare()`, renders a
three-column table: `saved | current | delta` with colour-coded deltas:
- Green (+): metric grew
- Red (-): metric shrank
- Grey (±0): unchanged

Footer shows: `epoch: N → M  (epoch advanced by K)` or `(no structural mutations since snapshot)`.

If no snapshot exists yet, shows: `no baseline — run 'graph snapshot' first`.

### crates/k-shell/src/proc.rs

Added routing:
```
"graph snapshot" | "gsnapshot"  → dispatch_graph_snapshot
"graph compare"  | "gcompare"   → dispatch_graph_compare
```

Added help text entries for both commands and their aliases.

---

## Test Harness: gos-graph-snapshot-harness (L4=59)

10 integration tests covering:

| # | Test | Assertion |
|---|------|-----------|
| 1 | Before any save | `cur.valid=true`, `node_count=0` on empty graph |
| 2 | Save empty graph | `saved.valid=true`, all metrics 0 |
| 3 | Save non-empty | `node_count=2`, `edge_count=1`, `density_ppm>0` |
| 4 | Compare unchanged | `saved.epoch == cur.epoch`, metrics identical |
| 5 | Node count delta | After adding node: `cur.node_count = saved+1` |
| 6 | Density delta | After adding edge: `saved.density=0`, `cur.density>0` |
| 7 | Triangle transitivity | `trans_ppm = 1_000_000` for complete bidirected K3 |
| 8 | Double save overwrites | Second save yields `node_count=2`, first (=1) discarded |
| 9 | geff=0 isolated, >0 connected | Bidirected pair: `geff_ppm > 0` after connection |
| 10 | current.valid invariant | `cur.valid=true` across empty/isolated/connected graphs |

**Result:** 10/10 pass, exit 0.

---

## Design Decisions

1. **Single RUNTIME lock hold** for `graph_snapshot_inner()` — ensures all 8 metrics
   reflect the same graph epoch, preventing inconsistencies from interleaved mutations.

2. **Separate `METRIC_SNAPSHOT` static** rather than embedding in `GraphRuntime` — keeps
   the snapshot persistent across `reset()` calls (useful for monitoring across test cycles).

3. **`MetricSnapshot.valid`** flag instead of `Option<MetricSnapshot>` — easier to use in
   no_std context (avoids discriminant overhead; struct is always `Copy`).

4. **u64 for `geff_ppm`** — matches `graph_global_efficiency()` return type, avoids
   precision loss for dense graphs where efficiency is close to 1_000_000.

---

## VectorAddress L4 Namespace (updated)

```
L4=59  gos-graph-snapshot-harness (V2.83, new)
L4=58  gos-graph-diameter-harness (V2.82)
L4=57  gos-graph-summary2-harness (V2.81)
L4=56  gos-graph-power-law-harness (V2.80)
```

---

## Metric Coverage Matrix (post-V2.83)

| Category             | Metric                          | Version |
|----------------------|---------------------------------|---------|
| Monitoring baseline  | Snapshot save & compare (delta) | V2.83   |
| Combined view        | Center + peripheral panel       | V2.82   |
| Power-law fit        | Exponent MLE γ̂                  | V2.80   |
| Topology dashboard   | One-shot summary                | V2.79   |
| Scale-free detection | Degree heterogeneity κ          | V2.78   |
| Small-world detection| σ = (CC/CC_rand)/(L/L_rand)     | V2.77   |
| Local fault-tolerance| E_loc = (1/n)ΣE(G_v)           | V2.76   |
| Avg clustering       | WS per-node (1/n)ΣCC(v)        | V2.75   |
| Global efficiency    | E(G) = Σ1/d(i,j)/(n(n-1))      | V2.74   |
| Center nodes         | ecc == radius                   | V2.73   |
| Peripheral nodes     | ecc == diameter                 | V2.72   |
| Harmonic centrality  | HC[v] = Σ1/d(v,u)              | V2.71   |
| Wiener index         | W(G) = Σ pairwise distances     | V2.70   |
| Girth                | Shortest directed cycle length  | V2.69   |
| Rich-club            | ρ(k) = density among hubs       | V2.68   |
| Modularity           | Newman-Girvan Q                 | V2.67   |
| Reciprocity          | Mutual edge fraction            | V2.66   |
| Assortativity        | Newman degree mixing r          | V2.65   |
| k-core decomposition | Batagelj-Zaversnik peeling      | V2.64   |

---

## Next Steps

- `graph snapshot list` — future: named snapshots (requires dynamic storage)
- `graph watch compare` — overlay deltas in the live watch panel
- Correctness note: `dispatch_graph_summary` uses inverted variable names for
  `edge_count`/`node_count` from `graph_density()` but correct positional values —
  consider cleanup in a future refactor pass.
