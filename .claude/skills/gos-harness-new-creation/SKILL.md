---
name: gos-harness-new-creation
description: When creating a new GOSKernel host-test harness in host-tests/, always create three required artifacts alongside tests/: Cargo.toml with [workspace], and .cargo/config.toml overriding to x86_64-pc-windows-msvc with build-std. Without the .cargo override the harness targets the bare-metal kernel and fails with "can't find crate for std".
---

# Creating a New GOSKernel Host-Test Harness

## The rule

Every new harness under `host-tests/<name>/` needs exactly **three** setup files before `cargo test` will work:

```
host-tests/gos-graph-foo-harness/
├── Cargo.toml            ← [workspace] + standard dependencies
├── .cargo/
│   └── config.toml       ← REQUIRED host-target override
└── tests/
    └── graph_foo.rs      ← test file
```

**Cargo.toml** (copy this template exactly):
```toml
[package]
name = "gos-graph-foo-harness"
version = "0.1.0"
edition = "2021"

[workspace]          # ← REQUIRED: must be its own workspace

[dependencies]
gos-protocol = { path = "../../crates/gos-protocol" }
gos-cypher-mut = { path = "../../crates/gos-cypher-mut" }
gos-runtime = { path = "../../crates/gos-runtime" }
gos-supervisor = { path = "../../crates/gos-supervisor", default-features = false, features = ["host-testing"] }
```

**.cargo/config.toml** (copy this exactly — both sections are required):
```toml
[build]
target = "x86_64-pc-windows-msvc"

[unstable]
build-std = ["std", "panic_abort"]
build-std-features = []
```

Then run from the **harness directory**:
```powershell
cd host-tests/gos-graph-foo-harness
cargo test
```

## Why it's non-obvious

The root `.cargo/config.toml` at `E:\GOSKernel\.cargo\config.toml` sets `[build] target = "x86_64-gos-kernel.json"` globally. Cargo walks up the directory tree from the manifest to find config files, so every harness inherits the kernel target unless it has its own `.cargo/config.toml` that overrides it. The `[workspace]` in the harness Cargo.toml creates an independent Cargo workspace, but does **not** prevent config inheritance — only a local `.cargo/config.toml` override does.

Without the override, `cargo test` fails with:
```
error[E0463]: can't find crate for `std`
  = note: the `x86_64-gos-kernel` target may not support the standard library
```

The `[unstable] build-std = ["std", "panic_abort"]` section is also required — omitting it causes a different error about build-std features.

## GOSKernel context

- Root override: `E:\GOSKernel\.cargo\config.toml` sets kernel target globally
- All 44+ existing harnesses in `host-tests/gos-*-harness/` have this `.cargo/config.toml`
- Harnesses compile for host (`x86_64-pc-windows-msvc`) and link against `gos-runtime` via the `host-testing` feature of `gos-supervisor`
- Run harnesses with `Push-Location`/`Pop-Location` pattern (see `gos-harness-push-location` skill)

## From this session

V2.49 creation: `gos-graph-mst-harness` and `gos-graph-shortest-harness` initially failed because the `.cargo/config.toml` directory was missing. The MST harness error:
```
error[E0463]: can't find crate for `std`
  = note: the `x86_64-gos-kernel` target may not support the standard library
```
Fixed by creating `.cargo/config.toml` with the host-target override in both new harnesses.

V3.18 creation: `gos-graph-topo7-harness` showed a **variant** error when the Cargo.lock was copied from a sibling harness but `.cargo/config.toml` was missing. Instead of "can't find crate for std", it produced **323 compilation errors in gos-supervisor** (missing `Result`, `Ok`, trait `HeapAllocator`, etc.). Root cause: gos-supervisor compiled for the kernel target (`x86_64-gos-kernel`) even though the Cargo.lock resolved host-compatible versions — because the target selection happens at compilation time, not in the lock file. Fix: add `.cargo/config.toml`. The variant error occurs because the Cargo.lock was present, so Cargo resolved packages but still compiled for the wrong target.
