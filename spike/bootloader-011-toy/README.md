# ADR-018 spike — bootloader 0.11+ toy kernel

Not part of the main workspace (`Cargo.toml` here declares its own
`[workspace]`, same isolation pattern `host-tests/*` uses). Proves the
UefiBoot + QEMU/OVMF loop closes before any of this touches
`crates/gos-kernel`, per [ADR-018](../../doc/ADR-018-bootloader-uefi-migration.md)'s
own gate — **proven end-to-end**, see ADR-018 §六. Delete this directory
once a real migration PR for `gos-kernel` exists — it has no reason to
persist alongside the real thing.

**Known limitation of running this in place**: nested under
`E:\GOSKernel`, this inherits the repo's own `.cargo/config.toml`
(`[build] target` / `[unstable] build-std`), which fights the local
overrides in `.cargo/config.toml` / `kernel/.cargo/config.toml` here in
ways documented in ADR-018 §四/§五/§六 (`[unstable] build-std` merges
across the config hierarchy rather than overriding — a real Cargo
behavior, not a bug in this setup). The proof in ADR-018 §六 was run
from a location entirely outside `E:\GOSKernel` for exactly this
reason. If re-running this in place hits `E0152 duplicate lang item`,
that's the same known issue — copy this directory outside the repo
tree first.

Build: from `kernel/`, `cargo +nightly-2026-04-02 build`; then from
here, `cargo +nightly-2026-04-02 run --bin runner -- build <kernel-bin-path>`.
Add `uefi` as the mode (`-- uefi <path>`) to also boot it, with
`OVMF_CODE` set to a writable copy of `edk2-x86_64-code.fd` (see
`src/main.rs`'s doc comment — QEMU needs read-write pflash access even
with `readonly=on`, and a stock `Program Files` install will deny it).
