# GOS Graph Prime Rules v0.2

This file defines the non-negotiable rules for the Graph-Oriented OS codebase.
Any new subsystem, plugin, runtime change, or tooling change must satisfy these
rules before it is considered valid.

## 1. Prime Directive

Everything in GOS is expressed as graph structure plus graph execution.

- Persistent state lives in a `Node`.
- Relationships, activation paths, capability bindings, and control transfer live in an `Edge`.
- Startup is graph registration plus graph activation, never a handwritten procedural chain.

## 2. Hard Invariants

### 2.1 Runtime Authority

- `hypervisor` bootstrap may only initialize minimal platform prerequisites and call the loader.
- Business or feature plugins must not be started directly from `kernel_main`.
- Execution begins from manifest-declared entry nodes routed by the runtime.

### 2.2 Stable Identity

- Every plugin must own a stable `PluginId`.
- Every node must own a stable `NodeId` derived from `plugin_id + local_node_key`.
- `VectorAddress` is runtime location, not logical identity.
- Hot reload, restart, and migration logic must preserve `NodeId` when `local_node_key` is unchanged.

### 2.3 Native-First Plugin Model

- All new plugins must be manifest-native and executor-driven.
- New code must not introduce new `NodeCell` / `PluginEntry` based plugins.
- Legacy `NodeCell` support exists only for explicitly allowed migration islands.

### 2.4 Graph-Mediated Interaction

- Nodes may not directly mutate another node's state memory.
- Cross-plugin cooperation must happen through `EdgeSpec`, capability resolution, or runtime signal routing.
- Direct startup orchestration through ad hoc calls is forbidden.

### 2.5 Vector-Carrying Graph

- Every `NodeSpec` must explicitly declare `vector_ref`.
- Every `EdgeSpec` must explicitly declare `vector_ref`.
- Semantic plugins should carry a real `VectorRef`; hardware shims may use `None` only as a documented exception.
- Vector metadata is part of graph semantics, not an optional afterthought.

### 2.6 Permissioned Control

- Any node that can influence scheduling, activation, or external sync must declare explicit permissions.
- AI and control-plane nodes may advise or orchestrate, but must still route through runtime permission and lifecycle checks.

## 3. Allowed Legacy Island

The following plugins are currently allowed to remain legacy while migration continues:

- `K_PANIC`
- `K_SERIAL`
- `K_GDT`
- `K_CPUID`
- `K_PIC`
- `K_PIT`
- `K_PS2`
- `K_IDT`
- `K_PMM`
- `K_VMM`
- `K_HEAP`

Any plugin outside this list that uses `NodeCell`, `PluginEntry`, or `try_mount_cell` is a policy violation.

## 4. Required Development Artifacts

Every new plugin or major graph feature must provide all of the following:

- `PluginManifest`
- `NodeSpec` set
- `EdgeSpec` set or explicit graph-generation rule
- permission declaration
- import/export declaration
- stable `local_node_key`
- `NodeExecutorVTable`
- vector policy for nodes and edges

## 5. Review Checklist

- Is the feature represented as nodes and edges instead of hidden control flow?
- Does bootstrap remain minimal and loader-driven?
- Are all new plugins manifest-native?
- Are node identities stable and derived, not handwritten random bytes?
- Does every node and edge declare `vector_ref`?
- Are scheduling or AI capabilities protected by explicit permissions?
- Does the change avoid growing the legacy island?

## 6. Enforcement

The project ships with `tools/verify-graph-architecture.ps1`.
Changes that violate the graph contract should fail this verifier and should not
be merged.
