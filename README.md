# GOS Kernel

GOS is a graph-oriented operating system kernel prototype built around plugins, nodes, edges, and a native Node Graph Runtime.

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

## Bare-Metal Install

The project now includes an installer packaging flow so a target machine does not need a Rust toolchain.

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

## Graph CLI

The shell now includes a built-in graph control layer with contextual `show`, explicit `node <vector>` / `edge <vector>` selection, and `PgUp` / `PgDn` paging for overview and relation lists.

See [doc/GRAPH_CLI_COMMANDS_zh.md](./doc/GRAPH_CLI_COMMANDS_zh.md) for the command reference.

## Cypher Node

The system now includes a native `K_CYPHER` node. It accepts a controlled Cypher v1 subset from the shell and can browse nodes, browse edges, activate nodes, spawn nodes, and route edges through the runtime.

See [doc/CYPHER_NODE_zh.md](./doc/CYPHER_NODE_zh.md) for the supported syntax.

## Network Node

The system now includes a native `K_NET` uplink node. It can probe the QEMU PCI NIC, enable the device, read BAR layout, bring up the E1000 register surface, and report MAC and carrier state through the shell.

Use `net`, `net probe`, and `net reset` from the shell. Current capability is hardware bring-up and status reporting; guest DHCP/IP/TCP is still pending.

See [doc/NETWORK_NODE_zh.md](./doc/NETWORK_NODE_zh.md) for the current network scope and commands.

## CI Installer Artifact

GitHub Actions can produce a prebuilt installer artifact:

- workflow: `.github/workflows/installer-artifact.yml`
- output: `gos-installer-release`

That artifact can be downloaded on another machine and written to USB without setting up Rust.
