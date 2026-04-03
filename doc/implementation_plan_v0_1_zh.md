# GOS (Graph-Oriented System) Bare-Metal Kernel v0.1 实施方案

本方案旨在将 GOS 构建为一个运行在 x86_64 裸机环境（QEMU）下的原生操作系统内核。系统以图为内核控制流，并在内核级别提供 CLI 交互。

## 用户审核必读

> [!CAUTION]
> **开发环境变更**: 从托管应用（Hosted App）全面转向 **裸机开发 (Bare-Metal)**。这意味着我们将抛弃标准库 (`std`)，直接操作硬件。
> **引导程序 (Bootloader)**: 使用 Rust `bootloader` crate 进行 64 位转换和内核引导。
> **关键挑战**: 实现稳定的人机交互（键盘中断、VGA 显示）与图内核分配器（Memory Allocator）。

## 核心架构 (Kernel Level)

### 1. 内核基础 (The Foundation)
- **指令集**: x86_64
- **无标准库**: `#[no_std]` & `#[no_main]`
- **显示**: VGA 文本缓冲区 (`0xb8000`) 驱动，实现 `println!` 宏。
- **中断 (IDT)**: 键盘中断处理，支持实时 CLI 输入。

### 2. 内存管理
- **页表**: 由 bootloader 初始化，内核进行后续映射。
- **分配器**: 实现 `linked_list_allocator` 以支持 `alloc` (用于图节点和边的动态管理)。

### 3. 原子插件系统 (In-Kernel)
插件在内核层被定义为静态注册的函数描述符。
- **Plugin SDK**: 定义内核插件的 Trait。
- **Graph Engine**: 内核态的数据结构，维护节点间的 Link。

## 原子插件任务列表 (v0.1 Kernel Edition)

- [ ] **Plugin: K_VGA** - 内核级文本输出与光标管理。
- [ ] **Plugin: K_KBD** - PS/2 键盘驱动，实现缓冲区输入。
- [ ] **Plugin: K_MEM** - 基础堆分配器，为图数据提供空间。
- [ ] **Plugin: K_CORE_GRAPH** - 内核态图搜索与遍历逻辑。
- [ ] **Plugin: K_SHELL** - 内核级命令行解析器 (支持 `gos create/link/run`)。

## 拟议变更 (文件级别)

### [MODIFY] [Cargo.toml](file:///e:/GOSKernel/Cargo.toml)
添加 `bootloader` 和 `x86_64` 依赖。

### [NEW] [src/main.rs](file:///e:/GOSKernel/src/main.rs)
内核入口点 `_start`，初始化引导。

### [NEW] [src/vga_buffer.rs](file:///e:/GOSKernel/src/vga_buffer.rs)
实现 VGA 驱动，提供彩色文本输出（增强美感）。

### [NEW] [src/interrupts.rs](file:///e:/GOSKernel/src/interrupts.rs)
设置 IDT 和键盘处理器。

### [NEW] [src/g_kernel/](file:///e:/GOSKernel/src/g_kernel/)
内核态图引擎。

## 验证计划

### QEMU 环境
- 命令: `cargo run` (配合 `bootimage` 或自定义运行器)。
- 验证现象:
    1. QEMU 启动后显示 GOS Logo。
    2. 进入 `gos>` 交互界面。
    3. 支持键盘输入，可执行 `gos create node name=test_node`。

## 开放问题

- **持久化**: 裸机环境下暂无文件系统。是否使用 **RamDisk** 作为初始图存储？
    - *建议*: v0.1 使用 Hardcoded 初始图 + RAM 持久化。
- **网络/外部交互**: v0.1 是否需要 Serial 通讯？
    - *建议*: 实现 Serial 输出，方便调试，但 CLI 面向 VGA。
