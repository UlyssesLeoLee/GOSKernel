---
name: gos-workspace-member-tracked-check
description: Before committing any change to the root Cargo.toml [workspace] members list, verify every member path is actually tracked in git (git ls-files crates/<member> must be non-empty) — an untracked member directory passes ALL local checks but breaks CI at manifest load on a clean clone. Apply whenever a commit includes the root Cargo.toml, especially in hardening sessions using broad git add.
---

# Workspace Members Must Be Git-Tracked, Not Just Present

## The rule

When a commit touches the root `Cargo.toml` members list, check that each member referenced is tracked:

```bash
git ls-files crates/<member>/ | head -1   # must print at least one file
```

If empty, either `git add crates/<member>/` in the same commit or drop the member line.

## Why it's non-obvious

Every local gate passes with an untracked member: `cargo check -p gos-kernel`, `cargo check -p <member>`, governance, all 48 harnesses — because the directory exists on disk locally. Only a clean clone (CI) fails, with `error: failed to load manifest for workspace member ... crates/<member>` before any compilation starts. So this class of break is invisible to the gos-kshell-kernel-check-before-push gate and to every other pre-push check that runs against the working tree.

The typical cause: the user has a WIP crate locally (directory untracked, member line added to Cargo.toml), and an automated hardening session commits Cargo.toml wholesale — sweeping in the member line without the directory.

## GOSKernel context

- Root `Cargo.toml` members list is at the repo root; WIP crates appear as `?? crates/<name>/` in `git status`.
- Hardening sessions commit `Cargo.toml` when adding harnesses/crates — the sweep risk is every such commit.

## From this session

Commit b20bc1f (v3.26 Leap Zagreb) swept `"crates/k-rope"` into the members list while `crates/k-rope/` stayed untracked. CI verify failed at manifest load. Fixed in 9deea9b by committing the crate (a complete, compiling Phase R0+R1 XPBD rope solver — verified with cargo check + governance before push; removing the member line instead would have re-broken on the next sweep).
