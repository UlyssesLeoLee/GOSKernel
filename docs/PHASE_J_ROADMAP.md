# Phase J — Kernel Native Mechanism Completion

Goal: ensure any higher-level functionality can be built on the kernel's
native architectural primitives, without bespoke add-ons.

Status: **complete** for the core primitives (J.1 - J.8).  Follow-up
items below are deferred but documented.

## Delivered primitives

| # | Title | Mechanism | Surface |
|---|---|---|---|
| **J.1** | Read-side Cypher | parser + emitter callback | `k_cypher::dispatch_cypher_query`, `QueryEmitter` |
| **J.2** | Journal persistence | ring buffer auto-fed by `emit_control_plane` | `gos_runtime::journal_*` |
| **J.3** | Request-response RPC | save/restore reply slot wrapping `route_signal` | `gos_runtime::rpc_invoke / rpc_reply / rpc_request` |
| **J.4** | Capability versioning | `CapabilitySpec.version` + `ImportSpec.{min,max}_version` checked in `validate_imports` | gos-protocol struct fields |
| **J.5** | (closed by J.8) | — | — |
| **J.6** | Sub-domain ACL | `sub_domain_allows_edge(from, to, kind)` gating Cypher mutations | gos-protocol const fn, `MUTATION_GATE_ACL_VIOLATION` |
| **J.7** | Priority scheduling | `NodeRecord.priority: u8`, priority-aware ready-queue pop | `gos_runtime::set_node_priority / node_priority`, `NODE_PRIORITY_*` constants |
| **J.8** | SHOW JOURNAL / SHOW PLUGINS | extends J.1 emitter pattern | new Cypher verbs |

## What "any functionality can be built natively" now means

Concrete example: implementing a new feature against the kernel does not
require touching the runtime internals or adding bespoke ABIs.  The
primitives compose:

* **Observable** state: any feature can `SHOW NODES`, `SHOW EDGES`,
  `SHOW JOURNAL`, `SHOW PLUGINS` to read the live graph.
* **Mutable** state: any feature can `CREATE MOUNT`, `CREATE USE`,
  `LINK`, `REBIND USE`, `DELETE EDGE` to evolve the graph through the
  supervisor gate, with sub-domain ACL enforcement (J.6) and audit
  logging (J.2) running for free.
* **Communication**: any two graph nodes can talk synchronously via
  `rpc_invoke` (J.3) — one u64 in, one u64 out, with full save/restore
  across nested calls.
* **Scheduling**: latency-critical work declares
  `set_node_priority(.., NODE_PRIORITY_HIGH)` (J.7); background workers
  set `NODE_PRIORITY_BACKGROUND`.  The ready queue honours the hint.
* **Evolution**: a capability provider can publish a new version while
  legacy consumers keep working — `CapabilitySpec.version` +
  `ImportSpec.min/max_version` (J.4) negotiate during boot validation.
* **Durability**: every state transition is recorded in the journal
  ring (J.2), serializable through `journal_snapshot_into(buf)` for
  future VFS-backed cross-reboot persistence.

## Test coverage delta

Runtime harness: 29 → 32 tests across J phases.

* J.3: `rpc_invoke_round_trip_through_target_executor`,
  `rpc_invoke_returns_no_reply_when_target_silent`
* J.7: `ready_queue_pops_highest_priority_first`

Supervisor harness: 16/16 unchanged (ACL doesn't add new tests in J.6
since the existing acceptance tests already pass through the gate
without triggering the Hardware-Mount rule).

QEMU smoke remains clean at every step.

## Deferred items (Phase K candidates)

* **J.3.B** — Pointer-payload RPC: extend `rpc_invoke` to carry a
  payload buffer + length so RPCs can transfer arbitrary-sized
  messages.  Plain extension on top of J.3's slot machinery.
* **J.2.B** — VFS-backed journal persistence: write `journal_snapshot_into`
  output through `gos_vfs` to disk; replay on boot.  Requires the
  VFS write path to be wired into Cypher.
* **K.1** — Plugin hot-reload: replace an installed module's
  `BuiltinPluginDescriptor` at runtime, preserving its NodeId.  Builds
  on J.4 (version negotiation) + J.6 (ACL gate).
* **K.2** — Schema enforcement: extend `state_schema_hash` to a
  proper schema descriptor that the supervisor verifies before binding.
* **K.3** — Deadline-aware scheduling: extend J.7 with per-node
  deadlines (microsecond budget per dispatch); supervisor reports
  overruns via the fault attribution path (P1 #3).
* **K.4** — Multi-tenant user-domain processes: ring-3 ELF loader
  using the J.4 capability boundary as the protection model.
