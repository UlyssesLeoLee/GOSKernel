---
name: gos-runtime-api
description: GOSKernel gos_runtime public API reference for common mistakes. There is NO free node_count() function — use gos_runtime::snapshot().node_count instead. Apply whenever calling gos_runtime functions, especially from k-shell dispatch functions or any code merged from main that references gos_runtime.
---

# gos_runtime API: Common Mistakes

## The rule

`gos_runtime` does **not** expose a free `node_count()` function. To get the total live node count, call `snapshot()` and read the field:

```rust
// WRONG — does not exist, compile error: cannot find function `node_count`
let total = gos_runtime::node_count();

// CORRECT
let total = gos_runtime::snapshot().node_count;
```

## Why it's non-obvious

The name `node_count()` is intuitive and looks like it should exist as a convenience function (similar to `graph_degree()`, `graph_bipartite()`, etc.). But `gos_runtime` exposes aggregate state only through `snapshot() -> GraphSnapshot`, which holds `node_count`, `edge_count`, `epoch`, etc. as fields. There is no separate free function for individual fields.

## GOSKernel context

- `gos_runtime::snapshot()` is defined in `crates/gos-runtime/src/lib.rs` (around line 3808)
- `GraphSnapshot` struct has: `node_count: usize`, `edge_count: usize`, `epoch: u64`, and others
- Shell dispatch functions in `crates/k-shell/src/lib.rs` call this (e.g. `dispatch_graph_toposort`)

Confirmed free functions that **do** exist (as of V2.39):
- `gos_runtime::snapshot()` → `GraphSnapshot`
- `gos_runtime::graph_toposort::<N>()` → `([VectorAddress; N], usize, bool)`
- `gos_runtime::graph_degree::<N>()` → `([VectorAddress; N], [u32; N], [u32; N], usize)`
- `gos_runtime::graph_centrality::<N>()` → `([VectorAddress; N], [u32; N], usize)`
- `gos_runtime::post_signal(vec, signal)` → `Result<(), _>`
- `gos_runtime::register_node_routes(vec, routes)` → `Result<(), _>`

## From this session

CI "verify" failed in `k-shell` with "cannot find function `node_count` in crate `gos_runtime`". The V2.33 hardening commit added `let total = gos_runtime::node_count();` which never compiled. Fixed to `gos_runtime::snapshot().node_count`.
