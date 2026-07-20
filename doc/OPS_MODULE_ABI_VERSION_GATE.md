# Module ABI version gate (`gos-supervisor`)

Status: shipped. Branch `claude/module-abi-version-gate`.

## Gap

`gos-protocol` carries **two independent ABI axes**:

1. `GOS_ABI_VERSION` / `PluginManifest.abi_version` — the plugin-manifest
   axis. Checked at load time via `gos_protocol::abi_compatible()`, both
   in `gos-loader::validate_manifest` and `hypervisor::builtin_bundle::
   validate_manifest`, each rejecting a mismatch with their own
   `AbiVersionMismatch` error.
2. `MODULE_ABI_VERSION` / `ModuleDescriptor.abi_version` — the
   **module-vtable axis**: the shape of `ModuleAbiV1`, the struct of
   function pointers a module's `module_init`/`module_start` entry point
   receives. This is what actually crosses the supervisor↔module call
   boundary at runtime.

Axis 2 was set on every `ModuleDescriptor` (builtin and otherwise) but
**never compared** anywhere in `gos-supervisor`. A module compiled
against a future-major or newer-minor `MODULE_ABI_VERSION` than the
running host would install (`install_module` only runs the G.2
signature gate) and proceed through `validate_module` (dependency
check only) straight to `map_module`/`call_entry`, handing it a
`ModuleAbiV1` vtable shape it was never built to interpret — silently,
with no diagnostic, instead of failing fast.

## Fix

`gos_supervisor::Supervisor::validate_module` (`crates/gos-supervisor/
src/lib.rs`) now runs `gos_protocol::abi_compatible(descriptor.
abi_version, MODULE_ABI_VERSION)` for every `ModuleSource::Descriptor`
before the existing dependency check, at the same `Installed`/
`Stopped` → `Validated` transition — mirroring `gos-loader::
validate_manifest`'s axis-1 gate exactly, just on the other axis. A
mismatch returns the new `SupervisorError::AbiVersionMismatch` variant
(parallel to the existing `LoaderError::AbiVersionMismatch` and
`BuiltinBootError::AbiVersionMismatch`), and — because this runs inside
`bring_up_module`'s validate→map→instantiate→start pipeline — the
module rolls back to `Faulted` with no leaked instance/domain/claim,
the same clean-failure shape `missing_dependency_is_rejected` already
exercises for the dependency-check path.

`ModuleSource::Empty` (the pre-descriptor legacy builtin path) carries
no `abi_version` and is intentionally left unchecked, matching how
`gos-loader` only ever gates manifests that exist.

## Test coverage

New host-harness test
`incompatible_module_abi_version_is_rejected_at_bring_up`
(`host-tests/gos-supervisor-harness/tests/supervisor.rs`): installs a
descriptor with `MODULE_ABI_VERSION + (1 << 24)` (major bump, the one
`abi_compatible` rule with zero tolerance), asserts `bring_up_module`
returns `Err(SupervisorError::AbiVersionMismatch)` and the module ends
up `Faulted`. 17/17 `gos-supervisor-harness` tests pass (16 pre-existing
+ 1 new). `cargo check -p gos-kernel` clean.

## Known follow-up (not attempted here)

This closes axis 2 of the 3-axis gap a prior design note (draft
ADR-015, currently only on the unmerged `feat/v2-mutation-dispatcher`
worktree, not on `main`) identified: axis 1 (plugin manifest) was
already gated, axis 2 (module vtable) is gated as of this change, axis
3 — `gos-journal`'s `CONTROL_PLANE_PROTOCOL_VERSION` on
`ControlPlaneEnvelope` — is still read by `deserialize_envelope` but
never compared. That axis has a different shape (a wire/journal format
version, not a load-time install gate) and deserves its own scoped
change rather than being folded into this one.
