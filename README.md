# GOS Kernel

GOS is a graph-native operating system kernel for x86_64 bare metal.

The current system model is:

- `hypervisor` performs minimal bootstrap only
- builtin graph boot registers the runtime-visible node and edge graph
- `gos-supervisor` owns the long-lived system service cycle
- nodes, edges, vectors, capabilities, `mount`, and `use` are public execution concepts

The repository is no longer documented as a loader-first or procedure-first kernel. The recommended architecture is graph-native and supervisor-owned; the remaining legacy island is tracked as migration debt.

## Current Architecture

- `hypervisor`
  - enables CPU/platform prerequisites
  - initializes `gos-hal` vector and metadata spaces
  - installs builtin supervisor module descriptors
  - boots the builtin graph through `builtin_bundle::boot_builtin_graph(...)`
  - hands steady-state servicing to `gos_supervisor::service_system_cycle()`
- `gos-runtime`
  - registers plugins, nodes, and edges
  - owns activation, routing, capability lookup, and graph summaries
- `gos-supervisor`
  - owns module descriptors, domains, capability publication, instance queues, claims, and the system cycle
- native graph plugins
  - `k-shell`, `k-cypher`, `k-ai`, `k-cuda-host`, `k-net`, `k-ime`, `k-mouse`, `k-vga`

## User-Facing Graph Features

- Graph shell with contextual `show`, `back`, `node <vector>`, `edge <vector>`, `PgUp/PgDn`
- `theme.current -[use]-> theme.wabi|theme.shoji`
- `clipboard.mount` as a shared mounted clipboard node
- Cypher v1 subset for graph browsing and controlled activation/routing
- Host-backed CUDA bridge for graph-visible accelerator work
- Native network status node exporting `net/uplink`

See:

- [doc/GOS_ARCH_v2.md](./doc/GOS_ARCH_v2.md)
- [doc/GRAPH_CLI_COMMANDS_zh.md](./doc/GRAPH_CLI_COMMANDS_zh.md)
- [doc/CYPHER_NODE_zh.md](./doc/CYPHER_NODE_zh.md)
- [doc/NETWORK_NODE_zh.md](./doc/NETWORK_NODE_zh.md)

## Developer Quick Start

Prerequisites:

- Rust nightly with `rust-src` and `llvm-tools-preview`
- `cargo bootimage`
- QEMU
- PowerShell 7

Run the development VM:

```powershell
pwsh -File .\run.ps1
```

Run governance checks only:

```powershell
pwsh -File .\run.ps1 -ValidateOnly
```

Direct verification:

```powershell
pwsh -File .\tools\verify-graph-architecture.ps1
cargo check -p gos-kernel
```

## Bare-Metal Install

The project includes an installer packaging flow so the target machine does not need a Rust toolchain.

Build a portable installer package on a build machine:

```powershell
pwsh -File .\tools\build-installer.ps1 -Profile release
```

This creates:

- `dist\gos-installer\gos-installer.img`
- `dist\gos-installer\installer-manifest.json`
- `dist\gos-installer\INSTALL_BARE_METAL_zh.md`
- `dist\gos-installer-release.zip`

Write the image to a USB drive:

```powershell
pwsh -File .\tools\write-usb-image.ps1 -List
pwsh -File .\tools\write-usb-image.ps1 -ImagePath .\dist\gos-installer\gos-installer.img -DiskNumber 3
```

Detailed Chinese install instructions are in [doc/INSTALL_BARE_METAL_zh.md](./doc/INSTALL_BARE_METAL_zh.md).

## Current Development Priority

The next development phases are intentionally substrate-first:

1. clear the remaining legacy island
2. finish native module execution, isolated domains, resource arbitration, and private heaps
3. only then expand higher-level AI, CUDA, and developer experience features on top of the stabilized substrate
