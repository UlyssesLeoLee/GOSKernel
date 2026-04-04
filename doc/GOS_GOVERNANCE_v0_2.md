# GOS Governance v0.2

This document explains how the repository is governed so that future work stays
aligned with the vector-carrying graph architecture.

## Governance Goals

- keep bootstrap minimal
- force plugin registration through manifests
- keep runtime execution graph-shaped instead of procedure-shaped
- prevent new legacy islands
- require vector metadata to remain attached to graph structure
- give AI-native orchestration a controlled place in the system

## Mandatory Design Rules

### 1. Bootstrap Boundary

- `hypervisor` may only do CPU/platform enablement, compatibility mapping, metadata init, and `load_bundle`.
- If a new feature needs direct calls from `kernel_main`, the design is wrong unless it is itself bootstrap infrastructure.

### 2. Plugin Acceptance Rules

A plugin is mergeable only if it defines:

- a `PluginManifest`
- one or more `NodeSpec`
- declared permissions
- declared imports/exports
- a stable `local_node_key` per node
- a `NodeExecutorVTable`

If it does not meet those conditions, it must stay out of the main bundle or be
explicitly added to the legacy allowlist.

### 3. Graph Connectivity Rules

- startup dependencies must be represented by `depends_on` or runtime edges
- cross-plugin service usage must be represented by capability imports/exports
- direct references to another plugin are acceptable only for bootstrap plumbing, compatibility shims, or runtime-owned orchestration
- new feature logic must not depend on handwritten call order in `main`

### 4. Vector Rules

Every node and edge is required to declare a vector policy.

- `vector_ref: Some(...)` is the default target for semantic nodes and semantic edges
- `vector_ref: None` is acceptable only for hardware adjacency, low-level migration shims, or other explicitly non-semantic runtime glue
- if `None` is used, the reviewer should ask whether the node or edge is truly non-semantic

### 5. AI Control Rules

AI nodes are allowed to:

- observe graph state
- drain control-plane events
- emit scheduling advice
- trigger node activation through runtime-approved paths
- coordinate shell or service handoff

AI nodes are not allowed to:

- bypass runtime lifecycle checks
- write directly into another node's state page
- bypass permissions with ad hoc global hooks
- replace manifest-driven graph construction with hidden imperative logic

## Repository Enforcement Layers

### Layer 1: Human-readable contract

- [RULE_GRAPH_PRIME.md](/e:/GOSKernel/doc/RULE_GRAPH_PRIME.md)
- this governance document
- `.cursorrules`

### Layer 2: Mechanical verification

- `tools/verify-graph-architecture.ps1`
- `run.ps1` invokes the verifier before launch by default
- CI runs the verifier on pushes and pull requests

### Layer 3: Runtime shape

- loader owns plugin registration
- runtime owns activation, routing, capability lookup, and state transitions
- AI supervisor owns orchestration, not kernel `main`

## Merge Checklist

Before merging a graph-affecting change:

1. Run `pwsh -File tools/verify-graph-architecture.ps1`
2. Run `cargo check -p gos-kernel`
3. Confirm the change does not add a new legacy plugin unless explicitly approved
4. Confirm every new node and edge explicitly sets `vector_ref`
5. Confirm bootstrap still only loads the bundle and pumps the runtime
