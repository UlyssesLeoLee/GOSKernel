# OPS: Supervisor Snapshot View

**Shell command**: `sup` (alias: `supervisor`)  
**Added**: 2026-06-30 (automated hardening pass, firing 7)  
**Crate touched**: `k-shell`  

## What it does

Prints a structured snapshot of the GOS supervisor's internal accounting
state. This is the supervisor-level health view — distinct from the runtime
graph snapshot (`info`/`graph`) which shows node/edge/plugin counts, and
from `modules` which shows per-module lifecycle state.

The snapshot is populated by `gos_supervisor::snapshot()`, which reads
the live `SUPERVISOR` lock and returns a point-in-time `SupervisorSnapshot`.

## Output format

```
 supervisor snapshot
  modules    installed:8  running:6
  instances  live:14  ready:3  waiting:2  suspended:0
  templates  registered:8  domains:6
  resources  registered:4  claims:12  revokes:0  restarts_q:0
  memory     heap_grants:8  heap_pages:24
  ipc        caps:16  endpoints:16  queued_msgs:0
  lanes      ctrl:2  io:1  compute:3  bg:1
```

Field meanings:

| Section | Field | Meaning |
|---------|-------|---------|
| modules | installed | Modules registered with the supervisor |
| modules | running | Modules currently in `Running` lifecycle state |
| instances | live | Total `NodeInstance` entries allocated |
| instances | ready | Instances on the ready queue |
| instances | waiting | Instances blocked on a resource or IPC reply |
| instances | suspended | Instances paused (e.g. Suspend policy triggered) |
| templates | registered | `NodeTemplate` entries derived from installed modules |
| templates | domains | Isolated page-table domains created for modules |
| resources | registered | Hardware resources declared to the supervisor |
| resources | claims | Active `ResourceClaim` leases |
| resources | revokes | Claims queued for revocation |
| resources | restarts_q | Modules queued for supervisor-managed restart |
| memory | heap_grants | Heap window grants in the grant table |
| memory | heap_pages | Total heap pages currently charged across all instances |
| ipc | caps | Published capability slots |
| ipc | endpoints | Registered capability endpoints |
| ipc | queued_msgs | Messages sitting in inter-module queues |
| lanes | ctrl/io/compute/bg | Ready-queue instances per execution-lane class |

## Why this was added

Before this command, there was no operator view of the supervisor's own
internal counters. The `info` command shows the runtime graph snapshot
(plugin/node/edge/queue counts), and `modules` shows per-module health,
but neither shows:

- How many isolated domains (page tables) are live
- Whether the resource claim table or heap grant table is under pressure
- How instances are distributed across execution-lane classes
- Whether any messages are stuck in the IPC queue

This gap makes it hard to diagnose degraded-but-not-faulted conditions
(e.g. the heap grant table approaching `MAX_HEAP_GRANTS=256`, or
queued restarts accumulating faster than `service_system_cycle` can drain them).

## Implementation notes

- `gos_supervisor::snapshot()` already existed at `gos-supervisor/src/lib.rs:2661`;
  this change only adds a shell surface for it in `k-shell/src/proc.rs`.
- No new public API was added to `gos-supervisor`.
- If the supervisor has not been bootstrapped (e.g. during early boot, before
  `bootstrap()` has been called), the command prints an error rather than
  panicking — the underlying `ensure_bootstrapped()` returns
  `Err(SupervisorError::NotBootstrapped)`.

## Known limitations

- Snapshot is point-in-time under the supervisor lock; it is not a live
  streaming view. Call `sup` again to refresh.
- `queued_messages` counts messages in the supervisor's own IPC queue, not
  messages buffered inside individual plugin nodes.
