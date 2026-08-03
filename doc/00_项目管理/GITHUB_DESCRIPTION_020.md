<p align="center"><strong>GOS v0.2 — 图原生操作系统内核（Graph-Native Operating System Kernel）</strong></p>

# GOS v0.2：Vector Mesh 架构对外说明

| 项目 | 内容 |
|---|---|
| 文档编号 | GOS-DOC-00-03 |
| 所属阶段 | 00・项目管理（对外说明 / GitHub 主页文案） |
| 版本 / 状态 | v2.0 / 现行 |
| 作成 / 审核 / 批准 | GOS 核心团队 |
| 基线日期 | 2026-06-30 |
| 最终更新 | 2026-07-01 |

**变更履历**

| 版本 | 日期 | 变更内容 | 作成者 |
|---|---|---|---|
| v1.0 | — | 初版（英文，"25 Crates" 架构快照） | GOS 核心团队 |
| v2.0 | 2026-07-01 | 按日系工程标准全文中文化；将 crate 数量、`gos-loader` 定位等口径与 [GOS_ARCH_v2.md](../02_基本设计/GOS_ARCH_v2.md)、[GOS_GOVERNANCE_v0_2.md](./GOS_GOVERNANCE_v0_2.md) 对齐，消除文档间矛盾 | GOS 核心团队 |

> **口径说明**：本文档是面向仓库外部读者的项目介绍（GitHub 主页文案定位），叙述服从 [GOS_ARCH_v2.md](../02_基本设计/GOS_ARCH_v2.md) 的权威口径。如两者出现分歧，以 GOS_ARCH_v2.md 为准。

---

GOS 是一个完全使用 Rust 编写的实验性图原生（graph-native）操作系统内核。它把**每个组件都表达为一个 node（节点）**，把**每次交互都表达为一条 edge（边）**，运行在一个 48-bit 规范向量空间中。v0.2 代表从单体验证性原型到模块化、生产级微内核生态的一次完整架构转型。

---

## 一、架构总览

```
┌──────────────────────────────────────────────────────────┐
│                       HYPERVISOR                          │
│   kernel_main → 最小引导 → builtin graph boot → steady-state │
├──────────────────┬──────────────────────────────────────────┤
│  gos-supervisor   │  模块隔离域、资源租约、能力型 IPC、堆授权    │
├──────────────────┼──────────────────────────────────────────┤
│  gos-runtime      │  图调度器、信号分发、node arena、控制面镜像 │
├──────────────────┼──────────────────────────────────────────┤
│  gos-protocol     │  通用 ABI：VectorAddress / Signal /        │
│                   │  PluginManifest / EdgeSpec / NodeSpec      │
├──────────────────┼──────────────────────────────────────────┤
│  gos-hal          │  虚拟地址映射、node 元数据空间               │
└──────────────────┴──────────────────────────────────────────┘
```

> **与早期版本的关键差异**：`gos-loader` 曾经是启动主链的一环（"loader 先行、运行时补图"），但当前治理口径（见 [GOS_GOVERNANCE_v0_2.md §2.1](./GOS_GOVERNANCE_v0_2.md)）已明确禁止 `kernel_main` 走 `gos_loader::load_bundle`。`gos-loader` 目前仍保留在 workspace 中（其 `elf` 子模块被 Phase B.4 的模块装载设计复用），但**不再是启动主链的一部分**。详见 [GOS_ARCH_v2.md §四](../02_基本设计/GOS_ARCH_v2.md)。

### 39 个 Crate —— 完全物理解耦

仓库当前 `crates/` 目录下共 **39 个 crate**（`Cargo.toml` workspace members 精确计数，2026-07-01）。已在架构文档中明确职责的部分如下：

| 层级 | Crate | 职责 |
|---|---|---|
| **核心协议** | `gos-protocol` | 通用 ABI：`VectorAddress`、`Signal`、`PluginManifest`、`NodeSpec`、`EdgeSpec` |
| **运行时** | `gos-runtime` | 图登记、节点激活、边路由、capability 解析、图摘要、epoch 版本 |
| **监管面** | `gos-supervisor` | 模块隔离域、实例调度、resource claim、heap grant、system cycle |
| **硬件抽象** | `gos-hal` | 向量地址映射、节点元数据空间、兼容性底层桥接 |
| **迁移中组件** | `gos-loader` | 仍在 workspace 中，但已退出 `kernel_main` 主启动路径，仅作为迁移期部件 |
| **硬件驱动** | `k-gdt` `k-idt` `k-pic` `k-pit` `k-ps2` `k-mouse` `k-serial` `k-vga` `k-cpuid` `k-pmm` `k-vmm` `k-heap` | x86_64 模块化硬件驱动（`k-pit/k-ps2/k-idt/k-pmm/k-vmm/k-heap` 现列为 legacy 迁移岛，见治理文档 §三） |
| **用户可见服务** | `k-shell` `k-ai` `k-ime` `k-cypher` `k-net` `k-cuda-host` `k-vk-host` `k-panic` `k-mouse` | 面向用户的图控制服务与扩展点 |
| **内核入口** | `gos-kernel`（目录 `crates/gos-kernel`，Cargo 包名与目录名统一，ADR-011） | 启动入口、CPU 特性初始化、supervisor 编排 |

> **文档缺口提示**：`gos-ai-bridge`、`gos-cluster`、`gos-cypher-mut`、`gos-journal`、`gos-log`、`gos-rewrite`、`gos-sign`、`gos-verify`、`gos-vfs`、`k-core`、`k-chat`、`k-fat32`、`k-nim` 等 V2 阶段新增 crate 尚未在 [GOS_ARCH_v2.md](../02_基本设计/GOS_ARCH_v2.md) 中获得逐一职责说明。建议列入 04_实施计划 的文档同步待办（见 [task_v0_1_zh.md](../04_实施计划/task_v0_1_zh.md)），后续版本补齐后本表将同步更新。

---

## 二、关键特性

### 🔀 通用节点图
- **VectorAddress** — 48-bit 规范坐标（`L4.L3.L2.Offset`），映射到内核虚拟地址空间
- **Signal 原生 RPC** — 每次交互都是流经网状图的异步 `Signal`（`Call` / `Data` / `Control` / `Interrupt` / `Spawn` / `Terminate`）
- **稳定身份** — node / edge / plugin 均基于确定性 `FNV-1a` 派生身份
- **语义边代数** — 当前 runtime 一等支持 9 个命名点（`Depend` `Call` `Spawn` `Signal` `Return` `Mount` `Sync` `Stream` `Use`），其正交 primitive 分解见 [ADR-001](../03_详细设计/ADR-001-edge-algebra-constitution.md)（当前状态：提案待批准）

### 🛡️ 监管与隔离
- **模块域** — 每模块独立地址空间，含独立 image / stack / IPC / heap 窗口（详见 [PHASE_B4_DOMAIN_ISOLATION.md](../03_详细设计/PHASE_B4_DOMAIN_ISOLATION.md)）
- **资源租约** — 基于 epoch 的 claim / revoke 协议，覆盖帧分配器、页映射器、显示控制台、GPU、堆
- **能力型 IPC** — 带类型的 capability token + endpoint pub/sub 消息
- **4 车道调度** — `Control` / `IO` / `Compute` / `Background` 执行车道，各自独立 ready queue
- **堆授权** — 按模块实例的页粒度内存授权与配额强制
- **故障策略** — 按模块可配置自动重启或人工恢复（见 Phase B.5 degraded mode）

### 🖥️ 交互式图终端（`k-shell`）
- **图检查器** — 分页浏览 node / edge / overview，支持面包屑导航栈
- **命令历史** — 16 槽环形缓冲区，上下方向键遍历并保留草稿
- **图化剪贴板** — `clipboard.mount` 非排他挂载节点，`Ctrl+C/X/V` 通过 `Mount` 边复用
- **AI 监管面板** — 实时 AI 响应流、API key 配置（`^A`）、`ask` 命令
- **Cypher 查询引擎** — `MATCH (n) RETURN n` 风格图查询，路由至 `k-cypher`（受控子集，详见 [CYPHER_NODE_zh.md](../03_详细设计/CYPHER_NODE_zh.md)）
- **CUDA/GPU 提交** — `cuda submit <job>` 提交计算任务至 `k-cuda-host`
- **网络探测** — `net probe` / `net reset` 诊断网络子系统（详见 [NETWORK_NODE_zh.md](../03_详细设计/NETWORK_NODE_zh.md)）
- **IME 支持** — ASCII / 中文拼音输入模式切换
- **鼠标指针** — VGA 文本缓冲区软件光标叠加

完整命令列表见 [GRAPH_CLI_COMMANDS_zh.md](../03_详细设计/GRAPH_CLI_COMMANDS_zh.md)。

### ⚡ 中断架构
- **统一陷阱归一化** — 所有异常与 IRQ 流经单一 `gos_trap_normalizer`，捕获 `TrapFrame` 并打 TSC 时间戳
- **裸汇编 trampoline** — 通过 `global_asm!` 实现零开销中断入口
- **无死锁 IO** — IRQ 信号在锁作用域外投递到 runtime 队列，由主 pump loop 分发

### 🧠 内存子系统
- **物理内存管理器**（`k-pmm`）— 基于位图的页分配器，由 bootloader 内存映射初始化
- **虚拟内存管理器**（`k-vmm`）— 页表操作，为 supervisor domain 创建隔离地址空间
- **内核堆**（`k-heap`）— 基于 PMM/VMM 的链表分配器

---

## 三、构建与运行

```bash
# 前置条件：Rust nightly、bootimage、QEMU
cargo install bootimage

# 在 QEMU 中构建并运行
cd crates/gos-kernel
cargo bootimage
qemu-system-x86_64 \
  -drive format=raw,file=target/x86_64-gos-kernel/debug/bootimage-gos-kernel.bin \
  -serial stdio -no-reboot \
  -monitor telnet:127.0.0.1:55555,server,nowait
```

裸机安装路径见 [INSTALL_BARE_METAL_zh.md](../06_运维维护/INSTALL_BARE_METAL_zh.md)。

## 四、图拓扑示例（启动态）

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

> 本图为示意性启动态快照，权威、持续更新的对象模型与边语义以 [GOS_ARCH_v2.md](../02_基本设计/GOS_ARCH_v2.md) 为准。

## 五、许可证

Apache-2.0

---

> *设计能在世界变化中保持正确的系统。*
