---
name: gos-nightly-toolchain-pin
description: GOSKernel's CI installs Rust via dtolnay/rust-toolchain@master with an explicit `toolchain:` input in the workflow YAML — this action does NOT consult rust-toolchain.toml when that input is set, so editing rust-toolchain.toml alone never fixes a CI toolchain issue. Apply whenever CI fails with a dependency trait-implementation error (E0046/E0277-style) that doesn't reproduce locally, or when bumping the pinned nightly date.
---

# CI Toolchain Pin: Workflow YAML Wins, Not rust-toolchain.toml Alone

## The rule

`rust-toolchain.toml` pins `channel = "nightly-YYYY-MM-DD"` (a **dated** nightly, never floating `"nightly"`). But CI does NOT read that file for toolchain selection — both `.github/workflows/graph-governance.yml` and `.github/workflows/installer-artifact.yml` call `dtolnay/rust-toolchain@master` with an explicit `toolchain:` input. When `dtolnay/rust-toolchain` is given an explicit `toolchain` input, **it ignores `rust-toolchain.toml` entirely**. Both must be edited together and kept in sync:
- `rust-toolchain.toml` → governs local `cargo`/`rustup` toolchain resolution
- both workflow YAMLs' `toolchain:` input → governs what CI actually installs

## Why it's non-obvious

A floating `channel = "nightly"` (or `toolchain: nightly` in the YAML) works fine for months, then silently breaks CI on some future date when upstream Rust nightly changes a trait signature that a pinned dependency hasn't caught up with — while local dev machines keep working because their locally-cached "nightly" toolchain was never re-`rustup update`d. This produces the confusing symptom "CI fails, but `cargo check` passes locally with the exact same command." The fix isn't in this repo's source code at all.

## From this session (2026-07-16)

CI's `verify` job failed with:
```
error[E0046]: not all trait items implemented, missing: `forward_overflowing`, `backward_overflowing`
error: could not compile `x86_64` (lib) due to 2 previous errors
```
Root cause: `rust-toolchain.toml` and both workflow YAMLs had a floating `nightly`. A newer nightly (fetched fresh by every CI runner) added required `core::iter::Step` methods that the pinned `x86_64` crate v0.14.13 doesn't implement. Local `rustc +nightly --version` was still dated 2026-04-02 (last `rustup update`), which compiled clean — proving the break was purely a toolchain-version drift, not a code regression.

Fixed in commit `f560716`: pinned all three files (`rust-toolchain.toml` + both workflow YAMLs) to `nightly-2026-04-02`, verified locally with `cargo +nightly check -p gos-kernel` (the already-cached build matching that date) before pushing.

## Bumping the pin later

When intentionally moving to a newer nightly: update all three files together, run `cargo +nightly-<newdate> check -p gos-kernel` locally first (install via `rustup toolchain install nightly-<newdate> --profile minimal --component rust-src`), confirm clean, then push. Never bump by deleting the date and going back to floating `"nightly"`.
