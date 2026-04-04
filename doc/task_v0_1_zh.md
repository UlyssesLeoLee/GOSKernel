# GOS v0.1 原子插件开发任务清单

完整任务追踪表。每个条目对应一个原子插件，开发时**每次只开发一个**。

---

## 🔴 阶段 0：引导冒烟（Boot Smoke Test）

- [x] `K_BUILD` — 配置 `.cargo/config.toml`，交叉编译目标 + QEMU runner
- [x] `K_BOOT` — 实现 `_start` 入口，读取 bootloader BootInfo
- [x] `K_VGA` — VGA 文本缓冲区驱动（0xb8000），实现 `println!`
- [x] `K_SERIAL` — UART 16550 串口驱动，`serial_println!`
- [x] `K_PANIC` — panic_handler，输出位置信息后发出 `hlt`

**里程碑验证**: `cargo run` 在 QEMU 中显示 "GOS v0.1 booting..."

---

## 🟠 阶段 1：硬件抽象层（HAL Ready）

- [x] `K_GDT` — 设置 GDT 和 TSS
- [x] `K_IDT` — 设置 IDT，注册 CPU 异常 handler（breakpoint, page_fault, double_fault）
- [x] `K_PIC` — 初始化并重映射 8259A PIC（IRQ 向量表偏移 32）
- [x] `K_PIT` — 8254 定时器，产生周期性 IRQ0；实现系统 tick 计数
- [x] `K_PS2` — PS/2 键盘驱动，扫描码 → ASCII；维护环形缓冲队列
- [x] `K_CPUID` — 读取 CPU 信息写入初始图节点

**里程碑验证**: 键盘按键有回显，定时器 tick 可观测

---

## 🟡 阶段 2：内存管理（Heap Online）

- [ ] `K_PMM` — 物理页帧分配器（位图管理 `BootInfo::memory_map`）
- [ ] `K_VMM` — 4 级页表管理，提供 `map_page()` / `translate_addr()`
- [ ] `K_HEAP` — 链表分配器，实现 `GlobalAlloc`，启用 `alloc` crate

**里程碑验证**: 可在内核 `alloc::vec![1,2,3]`，不 panic

---

## 🟢 阶段 3：图内核（Graph Core）

- [ ] `K_SPINLOCK` — 自旋锁包装器
- [ ] `K_ID_GEN` — 原子 ID 生成器（`AtomicU64`）
- [ ] `K_NODE` — `GNode` 结构体（id, name, type, props: BTreeMap）
- [ ] `K_EDGE` — `GEdge` 结构体（from, to, type, weight）
- [ ] `K_GRAPH` — 图容器（增删改查节点、边）
- [ ] `K_DFS` — 深度优先搜索遍历
- [ ] `K_BFS` — 广度优先搜索遍历
- [ ] `K_TOPO` — Kahn 拓扑排序 + 环路检测
- [ ] `K_QUERY` — 图查询引擎（按 type/name/props 过滤）
- [ ] `K_STORE` — RAM 图序列化/快照

**里程碑验证**: 单元测试，创建节点/连边/拓扑排序，结果正确

---

## 🔵 阶段 4：插件 SDK 与 Shell（Plugin Shell）

- [ ] `K_SDK` — `GosPlugin` Trait 定义
- [ ] `K_RESULT` — `PluginResult` 与 `GosError` 枚举
- [ ] `K_CONTEXT` — `ExecCtx` 执行上下文
- [ ] `K_REGISTRY` — 插件注册表（静态数组）
- [ ] `K_PIPELINE` — 拓扑管道编排器
- [ ] `K_SHELL_LEXER` — 命令词法分析
- [ ] `K_SHELL_PARSER` — 命令语法解析 → `Command` 枚举
- [ ] `K_SHELL_HIST` — 命令历史（环形缓冲，方向键翻历史）
- [ ] `K_SHELL_DISP` — 格式化输出（列对齐、分页）
- [ ] `K_SHELL` — Shell 主循环（`gos> ` 提示符）
- [ ] `P_HELP` — `gos help`
- [ ] `P_STATUS` — `gos status`
- [ ] `P_CREATE` — `gos create node type=... name=...`
- [ ] `P_LINK` — `gos link A -> B type=...`
- [ ] `P_DELETE` — `gos delete node/edge`
- [ ] `P_LIST` — `gos list [nodes|edges|type=...]`
- [ ] `P_GET` — `gos get node <name>`
- [ ] `P_SET` — `gos set <name>.<key>=<val>`
- [ ] `P_MATCH` — `gos match (n)-[:dep]->(m)`
- [ ] `P_RUN` — `gos run <node>`
- [ ] `P_SNAPSHOT` — `gos snapshot save|restore`
- [ ] `P_CLEAR` — `gos clear`

**里程碑验证**: 在 QEMU 中完整执行构建管道演示图

---

## 🟣 阶段 5：执行引擎与并发（Execution Engine）

- [ ] `K_TASK` — 协作式任务结构体
- [ ] `K_WAKER` — Waker 机制实现
- [ ] `K_EXECUTOR` — 协作式异步执行器
- [ ] `P_EXEC_TOPO` — 拓扑执行引擎插件
- [ ] `P_EXEC_ASYNC` — 异步并行执行插件
- [ ] `P_SANDBOX` — 子图沙盒执行隔离

**里程碑验证**: 含 3 个依赖节点的管道可完整执行

---

## ⚪ 阶段 6：扩展能力（Extension）

- [ ] `K_RAMDISK` — 内存文件系统
- [ ] `P_FS_MAP` — 目录结构 → 图节点映射
- [ ] `P_FS_READ` — 读取文件节点内容
- [ ] `P_FS_WRITE` — 写入文件节点内容
- [ ] `P_VECTOR` — 节点向量存储与归一化
- [ ] `P_COSINE` — 两节点向量余弦相似度
- [ ] `P_ACTIVATE` — 图激活传播
- [ ] `P_AGENT` — Agent 子图快速建模模板

**里程碑验证**: `gos match` 可按向量相似度查找节点

---

## 插件统计

| 阶段 | 插件数量 |
|------|----------|
| 阶段 0 | 5 |
| 阶段 1 | 6 |
| 阶段 2 | 3 |
| 阶段 3 | 10 |
| 阶段 4 | 22 |
| 阶段 5 | 6 |
| 阶段 6 | 8 |
| **合计** | **60** |
