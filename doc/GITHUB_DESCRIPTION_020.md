# GOS v0.2.0: Modular Evolution & Next-Gen Visuals

GOS v0.2.0 marks a pivotal leap from a monolithic experimental kernel to a professional, modular **Cargo Workspace** architecture. This release introduces a physically decoupled micro-kernel ecosystem designed for high-assurance, graph-native distributed computing.

## 🚀 Key Highlights

### 🛰️ Modular Workspace Migration
The entire project has been restructured into **20+ specialized crates**. This decoupling ensures true physical isolation of OS components:
- **`gos-protocol`**: The universal node graph charter defining cross-plugin ABIs.
- **`hypervisor`**: The core graph-driven kernel binary.
- **`k-*`**: Modular hardware drivers (VGA, PS/2, PIT, PMM, VMM) and kernel libraries.
- **`gos-runtime`**: The graph-native scheduler and state-delta mirror.

### 🎭 Next-Gen "VGA text mode Cinema"
Introducing a revolutionary, stylized ASCII/CP437 boot experience rendered natively in VGA text mode:
- **Graph-Native Boot Sequence**: A 5-phase transition (`DISCOVER`, `DEPEND`, `ARENA`, `SYNC`, `HANDOFF`) with real-time telemetry animation.
- **Animated Sigils**: A dynamic, swaying "G-Sigil" central core that pulses with system resonance.
- **Starfield Backdrop**: Real-time starfield generation using CP437 character variants for depth.
- **Live Telemetry Panel**: Continuous monitoring of plugin counts, node identities, and ready-queue length during the boot process.

### 🧠 AI Supervisor Integration
Laying the foundation for AI-native control:
- **AI-API Uplink**: Real-time configuration of AI supervisor tokens via the shell.
- **Control Interface**: Early support for `AI_CONTROL_API` signals for graph manipulation.

### 🕸️ Graph CLI Control Surface
The shell now exposes first-class graph navigation commands:
- **Node Browser**: `show`, direct node-vector selection, and per-node detail inspection.
- **Edge Browser**: `edge`, inbound/outbound relation listing, and stable synthetic edge vectors for edge selection.
- **Safe Node Control**: `activate` and `spawn` operate on the current selected node without exposing ambiguous raw edge routing.

### ⛓️ Cypher Node Control
GOS now ships a native `K_CYPHER` node that accepts a controlled Cypher v1 subset:
- **Cypher Query Lane**: `cypher MATCH ...` or direct `MATCH ...` entry from the shell.
- **Node / Edge Browse**: `MATCH (n) RETURN n` and `MATCH ()-[e]-() RETURN e`.
- **Executable Graph Control**: `CALL activate(n)`, `CALL spawn(n)`, and `CALL route(e)` are routed through the runtime instead of bypassing it.

### 🛡️ Robustness & IO Stability
- **Wait-free IO**: Optimized keyboard input handling via polished PS/2 and PIT interrupt logic.
- **Stable Interrupt Handling**: Resolved Phase 1 IRQ delivery issues, enabling a deadlock-free interactive shell.
- **Phase 1 Memory Foundation**: Physical Memory Manager (PMM) and initial Virtual Memory (VMM) layers are now modular and ready for Phase 2 paging development.

## 🛠️ Architecture: The Universal Node Graph
GOS v0.2.0 treats everything as a node or an edge in a 48-bit canonical vector space.
- **VectorAddress**: Canonical addressing for graph coordinates (`L4.L3.L2.Offset`).
- **Signal-native RPC**: Every interaction is an asynchronous signal (`Call`, `Data`, `Control`, `Interrupt`) flowing through the mesh.

---

*“Designs systems that remain correct as the world changes.”* — GOS v0.2.0
