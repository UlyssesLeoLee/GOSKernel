# ADR-018 spike — bootloader 0.11+ toy kernel

Not part of the main workspace (`Cargo.toml` here declares its own
`[workspace]`, same isolation pattern `host-tests/*` uses). Exists only
to prove the BiosBoot + UefiBoot + QEMU(+OVMF) loop closes before any
of this touches `crates/gos-kernel`, per [ADR-018](../../doc/ADR-018-bootloader-uefi-migration.md)'s
own gate. Delete this directory once the spike's findings are folded
into ADR-018 and a real migration PR for `gos-kernel` exists — it has
no reason to persist alongside the real thing.

Run: `cargo run --bin runner -- bios` or `cargo run --bin runner -- uefi`
(see `src/main.rs`).
