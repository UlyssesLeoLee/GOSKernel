---
name: gos-step-trait-compat-window
description: This project's pinned nightly-2026-04-02 sits in a narrow, exact three-way compatibility window of unstable `core::iter::Step`'s evolving API shape. Any new dependency that itself implements `Step` (not just this repo's own `x86_64 = "0.14.13"`) can land on either side of that window and fail to compile with a *different* error than the already-documented E0046 case. Apply whenever pulling in a new crate that touches `x86_64`/paging/address-range types under this pinned toolchain, or when bumping the nightly pin.
---

# `core::iter::Step`'s unstable API has (at least) three incompatible shapes -- know which one this nightly has

## The rule

[`gos-nightly-toolchain-pin`](../gos-nightly-toolchain-pin/SKILL.md) already documents *one* failure mode: a nightly *later* than 2026-04-02 adds `forward_overflowing`/`backward_overflowing` to `Step`, breaking this repo's own `x86_64 = "0.14.13"` with **E0046** (missing trait items). That's real, but it's only one edge of a three-phase window this exact pinned nightly sits inside:

| Phase | `Step::steps_between` shape | `forward_overflowing`/`backward_overflowing` | Example `x86_64` crate version | Result on `nightly-2026-04-02` |
|---|---|---|---|---|
| 1 (oldest) | `-> Option<usize>` | doesn't exist | v0.14.10 | **E0053**: incompatible type, trait now wants `-> (usize, Option<usize>)` |
| 2 (this nightly's shape) | `-> (usize, Option<usize>)` | doesn't exist yet | v0.14.13 (this repo's pin) | ✅ compiles |
| 3 (newest) | `-> (usize, Option<usize>)` | exists, must be implemented | v0.15.5 | **E0407**: `method ... is not a member of trait` if the crate tries to `impl` these methods and this nightly's `Step` doesn't declare them yet |

Any crate that implements `Step` for its own address/page type (this project's `x86_64` dependency does this for `VirtAddr`/`PhysAddr`/`Page`) is exposed to whichever phase it was written against. **The error signature tells you which side of the window you're on**: E0053/"incompatible type for trait" means the dependency is *older* than this nightly's `Step` shape; E0407/"not a member of trait" means the dependency is *newer*.

## Why it's non-obvious

Both failure directions look like generic dependency-version noise until you actually read which specific error code fired. E0046 (missing items -- dependency too old for this compiler), E0053 (wrong signature -- also dependency too old, but a different vintage of "too old"), and E0407 (extra items -- dependency too new) are three different diagnoses that all trace back to the same one root cause (unstable `Step` API drift), and only one of the three was previously documented in this project's skills.

## From this session (2026-08-04, ADR-018 spike)

Building `bootloader` 0.11.x's own internal BIOS/UEFI stage binaries (a transitive dependency on a *different* `x86_64` crate version than this repo's own pin) hit both remaining phases depending on which `bootloader` point release was tried:
- `bootloader = "=0.11.7"` → pulls `x86_64 v0.14.10` → **E0053** (phase 1, too old).
- `bootloader = "0.11.17"` (latest) → pulls `x86_64 v0.15.5` → **E0407** (phase 3, too new).

Neither is a code bug in this repository -- both are upstream `bootloader`/`x86_64` crate releases landing outside this nightly's exact compatible slice. Full writeup, including the isolation methodology needed to reach this conclusion (running outside `E:\GOSKernel`'s directory tree to escape `.cargo/config.toml`'s inherited bare-metal target, and explicitly selecting `+nightly-2026-04-02` since directories outside the repo default to `stable`): [`doc/ADR-018-bootloader-uefi-migration.md` §四](../../doc/ADR-018-bootloader-uefi-migration.md).

## How to apply

Before assuming a new `x86_64`-crate-family dependency (or bumping one) "just needs a version bump" to fix a `Step`-related compile error:
1. Read the exact error code, not just "doesn't compile."
2. E0046 → dependency too old for this nightly (needs the *new* methods it's missing).
3. E0053 → dependency too old for this nightly (its `steps_between` signature predates the tuple return).
4. E0407 → dependency too new for this nightly (it's implementing methods this nightly's `Step` doesn't have yet).
5. A `cargo tree -p x86_64` (or equivalent) dependency-resolution check is much cheaper than a full build for binary-searching which point release of a transitive dependency lands back inside the window -- prefer that over repeated full builds when hunting for a compatible pin.
