---
name: gos-nodespec-register-api
description: GOSKernel NodeSpec construction and register_node / discover_plugin API — correct field names, 3-argument register_node signature, and discover_plugin (not register_plugin). Apply whenever writing host-test harness code that registers graph nodes.
---

# NodeSpec Construction and Node Registration API

## The rule

When writing a GOSKernel host-test harness that registers nodes, use this exact pattern:

```rust
use gos_protocol::{
    derive_node_id, EntryPolicy, ExecutorId, GOS_ABI_VERSION, NodeId,
    NodeSpec, PluginId, PluginManifest, RuntimeNodeType, VectorAddress,
};

// 1. Declare plugin and executor IDs (ASCII, 10 chars max)
const MY_PLUGIN: PluginId   = PluginId::from_ascii("KL_FOO0_01");
const MY_EXEC:   ExecutorId = ExecutorId::from_ascii("foo.exec");

// 2. Manifest uses policy_hash field (NOT absent)
const MY_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    MY_PLUGIN,
    name:         "kl-foo-harness",
    version:      1,
    depends_on:   &[],
    permissions:  &[],
    exports:      &[],
    imports:      &[],
    nodes:        &[],
    edges:        &[],
    signature:    None,
    policy_hash:  [0u8; 16],   // ← REQUIRED field, easy to forget
};

// 3. NodeSpec uses these exact field names
fn node_spec(key: &'static str, id: NodeId) -> NodeSpec {
    NodeSpec {
        node_id:           id,
        local_node_key:    key,      // ← NOT key:, NOT name:
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,  // ← NOT entry:
        executor_id:       MY_EXEC,   // ← NOT executor:
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

// 4. discover_plugin (NOT register_plugin)
gos_runtime::discover_plugin(MY_MANIFEST).unwrap();

// 5. register_node takes 3 args: (plugin_id, vector, spec)
//    NOT 1 arg like register_edge
gos_runtime::register_node(MY_PLUGIN, MY_VEC_A, node_spec("a.key", MY_ID_A)).unwrap();
```

## Why it's non-obvious

Several field names differ from intuitive guesses:
- `local_node_key` (not `key`, not `name`)
- `entry_policy` (not `entry`)
- `executor_id` (not `executor`)
- `PluginManifest` has a `policy_hash: [0u8; 16]` field that is easy to omit; the struct is `#[non_exhaustive]`-like in practice
- `discover_plugin` (not `register_plugin`) — the function name matches the "plugin discovery" lifecycle
- `register_node(plugin_id, vec, spec)` takes 3 arguments, unlike `register_edge(spec)` which takes 1

## GOSKernel context

- `NodeSpec` and `PluginManifest` defined in `crates/gos-protocol/src/lib.rs`
- `register_node(plugin_id, vector, spec)` defined in `crates/gos-runtime/src/lib.rs` (around line 7353)
- `discover_plugin(manifest)` defined in `crates/gos-runtime/src/lib.rs`
- See also: `gos-edgespec-api` skill for the corresponding EdgeSpec pattern
- All 44+ host-test harnesses follow this exact pattern — when in doubt, copy from
  `host-tests/gos-graph-snapshot-harness/tests/graph_snapshot.rs`

## From this session

V2.84 `gos-graph-link-predict-harness` initially failed with 33 compile errors because the
first draft of the test file used `gos_runtime::register_node(spec)` (1 arg) with a
`NodeSpec` that had wrong field names (`executor:`, `entry:`, `key:`, `node_type:`) and
`register_plugin` instead of `discover_plugin`. All 33 errors resolved by copying the
node-spec helper pattern from `gos-graph-snapshot-harness`.
