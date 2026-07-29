---
name: gos-edgespec-api
description: GOSKernel EdgeSpec construction API — correct field names, derive_edge_id signature, register_edge arity, and RuntimeEdgeType variants. Apply whenever writing host-test harness code that creates graph edges.
---

# EdgeSpec Construction API

## The rule

When constructing an `EdgeSpec` in a GOSKernel host-test harness (or anywhere in gos_runtime):

```rust
// CORRECT
use gos_protocol::{derive_edge_id, EdgeSpec, RoutePolicy, RuntimeEdgeType};

let spec = EdgeSpec {
    edge_id:              derive_edge_id(from_node_id, to_node_id, "edge.key"),
    from_node:            from_node_id,
    to_node:              to_node_id,
    edge_type:            RuntimeEdgeType::Signal,  // NOT ::Data
    weight:               1.0,
    acl_mask:             u64::MAX,
    route_policy:         RoutePolicy::Direct,
    capability_namespace: None,
    capability_binding:   None,
    vector_ref:           None,
};
gos_runtime::register_edge(spec).unwrap();  // 1 arg, no PluginId
```

Common wrong patterns that fail to compile:
```rust
// WRONG: derive_edge_id takes (NodeId, NodeId, &str), NOT (PluginId, u32)
derive_edge_id(MY_PLUGIN, 10u32)

// WRONG: variant doesn't exist
edge_type: RuntimeEdgeType::Data

// WRONG: field doesn't exist
permissions: &[]

// WRONG: register_edge takes 1 arg
gos_runtime::register_edge(MY_PLUGIN, spec)
```

## Why it's non-obvious

Several intuitive-looking names are wrong:
- `RuntimeEdgeType::Data` sounds natural but the variant is `::Signal`
- `register_edge(plugin_id, spec)` mirrors `register_node(plugin_id, vec, spec)` but takes only 1 arg
- `derive_edge_id` looks like it should take a PluginId (like `derive_node_id` does) but takes two NodeIds + a string key
- `EdgeSpec` has `weight`, `acl_mask`, `capability_namespace`, `capability_binding`, `vector_ref` — no `permissions`

## GOSKernel context

- `EdgeSpec` defined in `crates/gos-protocol/src/lib.rs`
- `register_edge` defined in `crates/gos-runtime/src/lib.rs`
- All existing harnesses that add edges: see `gos-graph-clustering-harness`, `gos-graph-bipartite-harness`

## From this session

V2.63 `gos-graph-transitivity-harness` initially failed with 4 compile errors:
- `derive_edge_id(TR_PLUGIN, minor as u32)` — wrong args
- `RuntimeEdgeType::Data` — variant not found
- `permissions: &[]` — no such field in EdgeSpec
- `gos_runtime::register_edge(TR_PLUGIN, spec)` — too many args

All fixed by looking at the working gos-graph-clustering-harness pattern.
