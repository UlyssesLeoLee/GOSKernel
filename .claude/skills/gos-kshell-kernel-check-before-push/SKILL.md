---
name: gos-kshell-kernel-check-before-push
description: k-shell dispatch code (lib.rs display functions, proc.rs routing/help text) is ONLY compiled by cargo check -p gos-kernel — host-test harnesses never touch it. Always run cargo check -p gos-kernel before committing any k-shell change, even when all 48 harness tests pass. Apply at the end of every hardening session that adds a dispatch_* function or help text.
---

# k-shell Changes Require `cargo check -p gos-kernel` Before Push

## The rule

Before committing any change to `crates/k-shell/src/lib.rs` or `crates/k-shell/src/proc.rs`, run both CI gates locally:

```powershell
./tools/verify-graph-architecture.ps1   # governance
cargo check -p gos-kernel               # compiles k-shell for bare-metal
```

A green host-test suite (48/48 harnesses) is NOT evidence that k-shell compiles.

## Why it's non-obvious

The host-test harnesses exercise `gos-runtime`, `gos-protocol`, etc. on the MSVC host target — they never build `k-shell`, which is only pulled in by the bare-metal `gos-kernel` crate. A hardening session can add a runtime function, write 10 passing harness tests, wire up a `dispatch_*` display function, push, and only then discover the dispatch layer never compiled.

This exact failure shipped twice in a row:
1. V2.65/V2.66 — `\xe2\x80\x94` byte escapes >0x7F in k-shell string literals (E: out of range hex escape).
2. V2.90–V2.92 — `dispatch_graph_domtree` / `_fas` / `_bimatch` called `LineBuf::len()`, which didn't exist (E0599); `LineBuf` only had a private `len` field and `as_slice()`.

Both passed all harness tests locally and failed only on CI.

## GOSKernel context

- CI `verify` job = `./tools/verify-graph-architecture.ps1` + `cargo check -p gos-kernel` (no `-D warnings` — warnings don't fail).
- `cargo check -p k-shell` alone does NOT work from the workspace root (gos-supervisor feature-gate errors); use `-p gos-kernel`.
- `LineBuf` (k-shell lib.rs ~374) now has `len()` (added 7e981f6) alongside `as_slice()`, `push_str`, `push_dec`, `push_vector`.

## From this session

CI verify failed twice on PR #2 (fixed in fc881f7 and 7e981f6). Root cause both times: hardening sessions validated only via host harnesses and never compiled the dispatch layer they added.
