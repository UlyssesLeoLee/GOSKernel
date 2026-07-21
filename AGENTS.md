# AGENTS.md — GOS Kernel

GOS is an experimental `no_std`/`no_main` graph-theory operating system
kernel for bare-metal x86_64, written in Rust. Persistent state lives on
**nodes**; relationships, mounts, capability bindings, and activation paths
live on **edges**. The long-term goal is a complete graph-native OS
ecosystem competitive with commercial OSes — without compromising that
architecture for convenience.

## 1. Read this first: the prime rules

Two documents are the repo's non-negotiable constitution. Read them before
touching boot sequencing, the plugin model, the runtime/supervisor, or graph
mutation:

- [`doc/00_项目管理/RULE_GRAPH_PRIME.md`](doc/00_项目管理/RULE_GRAPH_PRIME.md) — hard invariants:
  bootstrap boundary, stable `NodeId`/`VectorAddress`/`EdgeId`, native-first
  plugins, graph-mediated cooperation, zero-new-legacy.
- [`doc/00_项目管理/GOS_GOVERNANCE_v0_2.md`](doc/00_项目管理/GOS_GOVERNANCE_v0_2.md) — rationale,
  current legacy allowlist (`k-pit`, `k-ps2`, `k-idt`, `k-pmm`, `k-vmm`,
  `k-heap` — do not extend it), and the merge checklist.

Constraints from those docs that are easy to violate by accident:

- `hypervisor::kernel_main` only does minimal bring-up, then delegates to
  `gos_supervisor::service_system_cycle()`. Don't reintroduce
  `gos_loader::load_bundle`, a direct `gos_runtime::pump`, or hand-rolled
  `post_signal` startup ordering.
- Every `NodeSpec`/`EdgeSpec` literal needs an explicit `vector_ref`.
- User-visible relationships (theme, clipboard, capability use) must be real
  graph edges (`use`/`mount`/import-export) — if it can't be explained via a
  graph summary, the design is wrong (Public Semantics Rule).
- Graph mutations go through `gos-runtime`'s atomic dispatcher
  (`RuntimeDispatcher::add_edge` / `rebind_exclusive_use` / `rebind_use`):
  one `graph_epoch` bump per mutation, never a partial write, and exactly
  one `Use` edge per source node.

> A separate, **not-yet-ratified** "edge algebra constitution" redesign
> (sometimes referenced as `ADR-001`/`ADR-004`) may exist only as a proposal
> in a sibling agent worktree (`doc/03_详细设计/ADR-001-edge-algebra-constitution.md`,
> `plan/V2_DEVELOPMENT_PLAN.md`). If you see an `ADR-00x` citation, confirm
> the doc actually exists and is ratified **on your branch** before relying
> on it or repeating the citation — don't propagate references to docs you
> haven't verified yourself.

## 2. Repo layout

- `crates/hypervisor` — boot entry (`kernel_main`); the only place allowed
  to do imperative bring-up. Everything after bring-up is steady-state.
- `crates/gos-*` — graph runtime/protocol/supervisor core: `gos-runtime`,
  `gos-protocol`, `gos-supervisor`, `gos-cypher-mut`, `gos-loader`,
  `gos-hal`, `gos-journal`, `gos-vfs`, `gos-sign`, `gos-verify`,
  `gos-cluster`, `gos-ai-bridge`.
- `crates/k-*` — kernel-tier drivers/services: gdt/idt/pic/pit/ps2/vga/
  serial/cpuid/net/mouse/heap/pmm/vmm, plus shell/chat/ime/cypher/ai/
  vk-host/cuda-host/nim/fat32/panic/core. `k-pit`, `k-ps2`, `k-idt`,
  `k-pmm`, `k-vmm`, `k-heap` are the only crates allowed to use legacy
  traits (migration debt — see governance doc).
- `tools/gos-vk-viewer` — **detached** host-side GPU viewer: its own
  `[workspace]`, builds for the host target via `wgpu`/`winit`, not the
  `no_std` kernel target (see its `Cargo.toml` header comment and
  `.cargo/config.toml`). Screen-space HUD/overlay elements (e.g. the B3b
  input-echo strip) reuse the main shader via a second pipeline with
  `view_proj = IDENTITY` and `depth_compare = Always` — no egui/glyphon
  dependency. Follow this pattern for Phase I UI work rather than adding one.
- `tools/*.py`, `tools/*.ps1` — build/launch helpers and empirical
  verification harnesses (see §4).
- `doc/`, `plan/` — design docs, specs, phase roadmaps (mostly Chinese),
  organized under `doc/00_项目管理` … `doc/06_运维维护` following a
  Japanese-style SDLC document structure (project mgmt → requirements →
  basic design → detailed design → implementation plan → test verification
  → operations). `00_项目管理/RULE_GRAPH_PRIME.md` /
  `00_项目管理/GOS_GOVERNANCE_v0_2.md` are the load-bearing ones; treat
  everything else as design history, not current law, unless cross-checked
  against code. Loose `.md` files directly under `doc/` (outside the
  numbered folders) are archival stubs pointing at the canonical numbered
  location — follow the link rather than reading the stub itself.

## 3. Build, run, check

```powershell
./run.ps1                  # governance check -> cargo run -p gos-kernel --release -> QEMU
./run.ps1 -SkipGovernance  # skip the graph-architecture gate (debugging only)
./run.ps1 -ValidateOnly    # just run the governance check, no build/launch
./run.ps1 -Clean           # cargo clean first (full rebuild)

pwsh -File ./tools/verify-graph-architecture.ps1   # the governance gate; CI runs this too
cargo check -p gos-kernel
cargo check -p k-shell
```

`run.ps1` builds `--release`: the in-guest desktop is a CPU software
rasterizer, and the `dev` profile runs it at ~5.8 FPS (visible mouse
stutter) vs ~18-19 FPS in `--release`, for only ~1s extra incremental-build
cost.

`run.ps1` kills any stale `qemu-system-x86_64.exe` holding the disk image
before relaunching (`Stop-StaleGosQemu`). Do the same before starting your
own QEMU instance for testing — a leftover process holding the image will
make the new one fail to boot.

## 4. Empirical verification (QEMU + serial)

QEMU's serial ports are bridged to the host:

| Port | I/O addr | Host side   | Carries |
| ---- | -------- | ----------- | ------- |
| COM1 | 0x3F8    | stdio       | boot log, `raw_serial_println!`, B3b `vk-input:` echo |
| COM2 | 0x2F8    | `tcp:14444` | k-chat |
| COM3 | 0x3E8    | `tcp:14445` | k-vk-host / B3b `@gos.vk` display-list + input protocol |

`tools/verify_*.py` is the established pattern for *behavioral* (not just
compile-time) verification: launch via
`Start-Process -RedirectStandardOutput/-RedirectStandardError`, poll the
COM1 log file and/or connect TCP sockets for COM2/COM3, drive input, and
assert on **observed** timing/content — not on source shape.
`tools/verify_b3b_input_roundtrip.py` is the most recent template. Use
generous time windows (multiple seconds, not hundreds of ms): QEMU's
TCP-chardev delivery latency is high enough that tight windows produce false
negatives.

Always clean up after a verification run: kill the QEMU + cargo processes
you started, and don't leave temporary `*.log`/`*.txt` artifacts around
(already gitignored, but noisy).

## 5. Toolchain pins worth knowing about

- `x86_64 = "0.14"` is pinned across ~17 crates. Its `Star::write` requires
  `kernel_data_selector == kernel_code_selector + 8` and
  `user_data_selector == user_code_selector - 8`. `k_gdt::Selectors::kernel_data_selector`
  is installed immediately after `code_selector` in `init_hal_state` (Phase
  E.2.1) specifically to satisfy this — if you reorder or insert GDT entries
  there, preserve that adjacency or `ring3::init`'s `Star::write` call will
  fail its invariant check. Before assuming an API shape, check the vendored
  source in your local cargo registry cache
  (`$CARGO_HOME/registry/src/.../x86_64-0.14.*`) or run `cargo doc -p x86_64
  --open` — don't trust docs.rs for an unspecified patch version.
- Nightly Rust + `build-std` (`core`, `alloc`, `compiler_builtins`) for the
  custom `x86_64-gos-kernel.json` target — see `.cargo/config.toml`.

## 6. Git / PR workflow

- **Never commit or push unless explicitly asked** — including from an
  autonomous/looping session. Land work as file edits; let the user (or an
  explicit instruction) trigger commit/push/PR/merge.
- Feature branches -> PRs against `main`, reviewed via CodeRabbit
  (`.coderabbit.yaml`). Don't force-push, amend published commits, or merge
  to `main` without **fresh** explicit confirmation — even if a previous
  commit/push was approved, merging to `main` is a separate decision.
- **Multiple agent sessions may run against this repo concurrently**
  (`git worktree list` — see `.claude/worktrees/*`, gitignored). Don't
  assume exclusive ownership of the working tree: check `git status`/`git
  diff` for unexpected uncommitted changes before you start, and don't
  discard or overwrite changes you didn't make without investigating first.

## 7. Keeping this file current

If you hit a hard-won lesson — a footgun, a missing invariant, an
undocumented convention — don't let it evaporate at the end of the session.
Fold it into this file (the `sync-agents-md` skill, if available, automates
gathering candidates from recent commits/review feedback). Keep entries
short and concrete; prune anything that no longer matches the code.
