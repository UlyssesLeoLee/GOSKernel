---
name: gos-multi-metric-epoch-consistency
description: When capturing multiple graph metrics that must all reflect the same graph state, chain their *_inner() calls inside one RUNTIME.lock() hold rather than calling the public wrappers sequentially. Apply whenever building composite metric snapshots or summary functions that need epoch-consistency across all measurements.
---

# Multi-Metric Epoch-Consistent Capture

## The rule

To capture N metrics atomically (all from the same graph epoch), write a private
`*_inner(&self)` method that chains the constituent `*_inner()` calls on `self`:

```rust
fn graph_snapshot_inner(&self) -> MetricSnapshot {
    let (density_ppm, node_count, edge_count) = self.graph_density_inner();
    let (trans_ppm, _, _, _)                   = self.graph_transitivity_inner();
    let (geff_ppm,  _, _)                       = self.graph_global_efficiency_inner();
    // ... more *_inner() calls ...
    MetricSnapshot { valid: true, epoch: self.graph_epoch, ... }
}

pub fn graph_snapshot_save() -> u64 {
    let snap = RUNTIME.lock().graph_snapshot_inner();  // one lock hold
    let epoch = snap.epoch;
    *METRIC_SNAPSHOT.lock() = snap;
    epoch
}
```

**Do NOT** call the public wrappers sequentially — each acquires its own lock:
```rust
// WRONG: separate lock acquisitions — graph can mutate between calls
let density = gos_runtime::graph_density();      // lock+release
let trans   = gos_runtime::graph_transitivity(); // lock+release (may see new epoch!)
```

## Why it's non-obvious

Each public graph metric function (`graph_density()`, `graph_global_efficiency()`, etc.)
acquires and releases `RUNTIME.lock()` independently. If any structural mutation
(node/edge add/remove) happens between two calls, the two metrics reflect different
graph states — their values are inconsistent with each other. The epoch field would
differ too.

Chaining `*_inner()` in one lock hold eliminates this inconsistency: all metrics are
computed on the same graph, and `self.graph_epoch` is the guaranteed shared epoch.

## Which `_inner()` methods can be chained

Only chain `_inner()` methods that are already called via the **direct lock pattern**
(`RUNTIME.lock().*_inner()`), not those that use the **topology_snapshot pattern**.

**Direct lock (chainable):**
- `graph_density_inner`, `graph_transitivity_inner`, `graph_clustering_inner`
- `graph_avg_clustering_inner`, `graph_global_efficiency_inner`
- `graph_local_efficiency_inner`, `graph_small_world_inner`
- `graph_scale_free_inner`, `graph_power_law_inner`
- `graph_assortativity_inner`, `graph_reciprocity_inner`, `graph_girth_inner`
- `graph_wiener_inner`

**Topology-snapshot pattern (do NOT chain under lock — IRQ deadlock risk):**
- `graph_katz_inner`, `graph_pagerank_inner`, `graph_hits_inner`
- `graph_community_inner`, `graph_spanning_inner`, `graph_color_inner`
- `graph_mst_inner`, `graph_flow_inner`, `graph_sim_inner`

See [[gos-topology-snapshot-pattern]] for the lock-release-compute rule.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs`: `GraphRuntime::graph_snapshot_inner()` (V2.83)
- `MetricSnapshot` struct + `METRIC_SNAPSHOT` static (V2.83)
- `graph_snapshot_save()` / `graph_snapshot_compare()` public API (V2.83)
- The `self.graph_epoch` field is the authoritative epoch — always read it from `self`
  inside the chained inner, not from a separate `gos_runtime::current_epoch()` call.

## From this session

V2.83 `graph_snapshot_save()` needs all 8 metrics to reflect one graph state for the
"compare" feature to show meaningful deltas. Using separate public-function calls would
allow a mutation (e.g., a node registration) to sneak in between density and efficiency,
producing a table where "density" and "E_global" reflect different graph sizes.
