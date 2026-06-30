<p align="center"><strong>GOS v0.2 — Graph-Native Operating System Kernel</strong></p>

# GOS v0.2: The Vector Mesh Architecture

GOS is an experimental, graph-native operating system kernel written entirely in Rust. It treats **every component as a node** and **every interaction as an edge** in a 48-bit canonical vector space. v0.2 represents a full architectural pivot from a monolithic proof-of-concept to a production-grade, modular micro-kernel ecosystem.

---

## Architecture Overview

```
┌──────────────────────────────────────────────────────────┐
│                     HYPERVISOR                           │
│  kernel_main → bootstrap → bundle load → pump loop      │
├──────────────┬───────────────────────────────────────────┤
│ gos-supervisor │  Module isolation, resource leasing,    │
│                │  capability-based IPC, heap grants      │
├──────────────┬┴──────────────────────────────────────────┤
│ gos-loader   │  Dependency-ordered module loading,       │
│              │  permission verification, graph sync      │
├──────────────┼───────────────────────────────────────────┤
│ gos-runtime  │  Graph scheduler, signal dispatch,        │
│              │  node arena, control-plane mirror         │
├──────────────┼───────────────────────────────────────────┤
│ gos-protocol │  Universal ABI: VectorAddress, Signal,    │
│              │  NodeCell, PluginManifest, EdgeSpec        │
├──────────────┼───────────────────────────────────────────┤
│   gos-hal    │  Virtual address mapping, node metadata   │
└──────────────┴───────────────────────────────────────────┘
```

### 25 Crates — Full Physical Decoupling

| Layer | Crates | Role |
|-------|--------|------|
| **Core Protocol** | `gos-protocol` | Universal ABI, `VectorAddress`, `Signal`, `NodeCell`, `PluginManifest` |
| **Runtime** | `gos-runtime` | Graph scheduler, ring queues, node arena (64 pages), control-plane mirror |
| **Supervisor** | `gos-supervisor` | Module isolation domains, resource leasing, capability tokens, heap grants, 4-lane scheduling |
| **Loader** | `gos-loader` | Dependency-sorted bundle loading, permission ACL, legacy/native/ELF module types |
| **HAL** | `gos-hal` | Virtual address mapping, node metadata schemas |
| **Hardware** | `k-gdt` `k-idt` `k-pic` `k-pit` `k-ps2` `k-mouse` `k-serial` `k-vga` `k-cpuid` `k-pmm` `k-vmm` `k-heap` | Modular x86_64 hardware drivers |
| **Services** | `k-shell` `k-ai` `k-ime` `k-cypher` `k-net` `k-cuda-host` `k-panic` | User-facing services and extension points |
| **Kernel** | `hypervisor` | Boot entry, CPU feature init, supervisor orchestration |

---

## Key Features

### 🔀 Universal Node Graph
- **VectorAddress** — 48-bit canonical coordinates (`L4.L3.L2.Offset`) mapped onto the kernel's virtual address space
- **Signal-native RPC** — Every interaction is an asynchronous `Signal` (`Call`, `Data`, `Control`, `Interrupt`, `Spawn`, `Terminate`) flowing through the mesh
- **Stable IDs** — Deterministic `FNV-1a`-based identity derivation for nodes, edges, and plugins
- **9 Edge Types** — `Call`, `Spawn`, `Depend`, `Signal`, `Return`, `Mount`, `Sync`, `Stream`, `Use`

### 🛡️ Supervisor & Isolation
- **Module Domains** — Per-module isolated address spaces with separate image, stack, IPC, and heap windows
- **Resource Leasing** — Epoch-based claim/revocation protocol for shared resources (frame allocator, page mapper, display console, GPU, heap)
- **Capability-Based IPC** — Typed capability tokens with endpoint pub/sub messaging
- **4-Lane Scheduling** — `Control`, `IO`, `Compute`, `Background` execution lanes with per-lane ready queues
- **Heap Grants** — Page-granular memory grants with quota enforcement per module instance
- **Fault Policy** — Per-module configurable restart or manual recovery

### 🎭 VGA Text-Mode Cinema
- **5-Phase Boot Sequence** — `DISCOVER → DEPEND → ARENA → SYNC → HANDOFF` with animated telemetry panels
- **Animated G-Sigil** — Dynamic sigil core with shake, wobble, and spark effects rendered in CP437
- **Starfield Backdrop** — 28-point starfield with phase-cycled character variants
- **Wabi/Shoji Theme System** — Two curated VGA DAC palettes switchable at runtime via graph edges
- **Live Telemetry** — Real-time plugin count, node/edge metrics, ready-queue and signal-queue monitoring

### 🖥️ Interactive Shell (`k-shell` — 4,400+ lines)
- **Graph Inspector** — Navigate nodes, edges, and graph overview with paginated browsing and breadcrumb navigation stack
- **Command History** — 16-slot ring buffer with up/down arrow traversal and draft preservation
- **Clipboard System** — Graph-backed clipboard node with copy/cut/paste via `Mount` edges
- **AI Supervisor Panel** — Real-time AI response streaming, API key configuration (`^A`), and `ask` command
- **Cypher Query Engine** — `MATCH (n) RETURN n` style graph queries routed to `k-cypher`
- **CUDA/GPU Submission** — `gpu submit <command>` for compute job dispatch to `k-cuda-host`  
- **Network Probe** — `net probe` / `net reset` for network subsystem diagnostics
- **IME Support** — Input method engine with ASCII/Chinese Pinyin mode switching
- **Mouse Pointer** — Software cursor overlay in VGA text buffer

### ⚡ Interrupt Architecture
- **Unified Trap Normalizer** — All exceptions and IRQs flow through a single `gos_trap_normalizer` with `TrapFrame` capture and TSC timestamping
- **Naked ASM Trampolines** — Zero-overhead interrupt entry via `global_asm!` common save/restore
- **Deadlock-Free IO** — IRQ signals posted to runtime queue outside lock scope, dispatched in main pump loop

### 🧠 Memory Subsystem
- **Physical Memory Manager** (`k-pmm`) — Bitmap-based page allocator initialized from bootloader memory map
- **Virtual Memory Manager** (`k-vmm`) — Page table manipulation, isolated address space creation for supervisor domains
- **Kernel Heap** (`k-heap`) — Linked-list allocator backed by PMM/VMM

---

## Building & Running

```bash
# Prerequisites: Rust nightly, bootimage, QEMU
cargo install bootimage

# Build and run in QEMU
cd crates/hypervisor
cargo bootimage
qemu-system-x86_64 \
  -drive format=raw,file=target/x86_64-gos-kernel/debug/bootimage-gos-kernel.bin \
  -serial stdio -no-reboot \
  -monitor telnet:127.0.0.1:55555,server,nowait
```

## Graph Topology (Boot State)

```
 K_SERIAL [1.2.0.0]  ←depend→  K_VGA [1.1.0.0]
 K_GDT    [1.3.0.0]  ←depend→  K_IDT [1.4.0.0]
 K_PIC    [1.5.0.0]  ←depend→  K_PIT [1.6.0.0]
 K_PS2    [1.7.0.0]  →signal→  K_SHELL [6.1.0.0]
 K_PIT    [1.6.0.0]  →signal→  K_SHELL [6.1.0.0]
 K_SHELL  [6.1.0.0]  →mount→   K_VGA   [1.1.0.0]
 K_SHELL  [6.1.0.0]  →mount→   K_AI    [7.1.0.0]
 K_SHELL  [6.1.0.0]  →mount→   CLIPBOARD [6.1.4.0]
 THEME    [6.1.3.0]  →use→     WABI [6.1.1.0] | SHOJI [6.1.2.0]
```

## License

Apache-2.0

---

> *Designs systems that remain correct as the world changes.*
