# Ops Note — Journal Control-Plane Protocol Version Gate

Status: shipped (hardening fix, no new capability)
Touches: `crates/gos-journal`, `host-tests/gos-runtime-harness`

## Gap

`gos-protocol` carries three independent ABI/version axes (see ADR-015,
drafted on the unmerged `feat/v2-mutation-dispatcher` worktree):

1. `GOS_ABI_VERSION` (plugin manifest / `KernelAbi`) — gated since Phase D.5
   via `gos_protocol::abi_compatible` in `gos-loader::validate_manifest`.
2. `MODULE_ABI_VERSION` (module vtable / `ModuleDescriptor`) — addressed by
   the unmerged `claude/module-abi-version-gate` branch's
   `gos_supervisor::validate_module` check.
3. `CONTROL_PLANE_PROTOCOL_VERSION` (the wire tag every `ControlPlaneEnvelope`
   carries) — **set at emit time** (`gos_runtime::emit_control_plane` stamps
   `CONTROL_PLANE_PROTOCOL_VERSION` onto every envelope) but, until this
   change, **never compared** anywhere a journal record was read back.

`gos_journal::JournalHeader::parse` already rejects a journal *container*
whose own `JOURNAL_VERSION` doesn't match (the 8-byte blob header). That is
a different, coarser axis — one version per blob/file. The per-record
`ControlPlaneEnvelope.version` field is finer-grained: it travels with each
individual 40-byte record and is the one tied to `CONTROL_PLANE_PROTOCOL_VERSION`,
the actual wire-format axis #3 from ADR-015. `deserialize_envelope` read this
field into the decoded struct but silently accepted any value — a future
protocol-wire bump (new envelope fields, reinterpreted `arg0`/`arg1`
semantics, etc.) would have old readers misinterpret new records instead of
rejecting them.

## Fix

`crates/gos-journal/src/lib.rs`:

- New `JournalError::UnsupportedProtocolVersion(u16)` variant.
- `deserialize_envelope` now checks the record's `version` field against
  `gos_protocol::CONTROL_PLANE_PROTOCOL_VERSION` and returns
  `Err(UnsupportedProtocolVersion(got))` on mismatch, before attempting to
  decode `kind`/`subject`/`arg0`/`arg1`. `replay` inherits the rejection
  automatically since it calls `deserialize_envelope` per record and
  propagates `?`.

This mirrors the existing strict-equality style already used for
`JOURNAL_VERSION` in `JournalHeader::parse` (axis #3 is a flat `u16`, not a
packed major/minor/patch like `GOS_ABI_VERSION`, so equality — not
`abi_compatible`-style major/minor comparison — is the right check here).

## Tests

`host-tests/gos-runtime-harness/tests/runtime.rs`:
`journal_rejects_envelope_with_stale_protocol_version` — encodes a valid
envelope, confirms it decodes, then tampers the record's version field and
asserts `deserialize_envelope` rejects it; repeats the tamper through a full
`JournalRing` → `flush_into` → `replay` path to confirm the rejection
surfaces end-to-end, not just at the single-record decode call. 25/25
`gos-runtime-harness` tests pass after this change (was 24); `gos-protocol-harness`
(8/8) and `cargo check -p gos-kernel` from the workspace root remain green.

## Known limitation / follow-up

This gate is strict equality against a single current version
(`CONTROL_PLANE_PROTOCOL_VERSION = 1`). There is no compatibility window —
the first time this constant bumps, every journal reader must bump in
lockstep, or old on-disk journals (once F.5 wires real persistence) become
unreadable. ADR-015's "minor-bump checklist" follow-up should cover whether
axis #3 ever needs a major/minor split like axis #1, or whether strict
equality is permanently fine because journals are an internal/ephemeral
control-plane artifact, not a cross-version compatibility surface like
plugin manifests are.
