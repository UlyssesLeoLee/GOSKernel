# GOS (Graph Operating System) — 主设计文档 v0.1

**目标平台**: x86_64 裸机 / qemu-system-x86_64  
**开发语言**: Rust (`#[no_std]`)  
**核心理念**: 每个功能点 = 一个原子插件（Atomic Plugin）。内核本身是一个图，插件是节点，调用关系是边。

---

## 一、分层架构总览

```
┌─────────────────────────────────────────────────────────┐
│  Layer 9 │  Agent 插件层     (P_AGENT, P_VECTOR)         │
├──────────┼──────────────────────────────────────────────┤
│  Layer 8 │  文件系统映射层   (P_FS_*)                    │
├──────────┼──────────────────────────────────────────────┤
│  Layer 7 │  执行引擎插件层   (P_EXEC_*, P_SANDBOX)       │
├──────────┼──────────────────────────────────────────────┤
│  Layer 6 │  内建命令插件层   (P_CREATE, P_LINK, P_RUN…)  │
├──────────┼──────────────────────────────────────────────┤
│  Layer 5 │  插件 SDK 层      (K_SDK, K_REGISTRY, K_CTX)  │
├──────────┼──────────────────────────────────────────────┤
│  Layer 4 │  图内核层         (K_NODE, K_EDGE, K_GRAPH…)  │
├──────────┼──────────────────────────────────────────────┤
│  Layer 3 │  并发与调度层     (K_TASK, K_EXECUTOR)        │
├──────────┼──────────────────────────────────────────────┤
│  Layer 2 │  内存管理层       (K_PMM, K_VMM, K_HEAP)      │
├──────────┼──────────────────────────────────────────────┤
│  Layer 1 │  硬件抽象层 HAL   (K_VGA, K_PS2, K_GDT…)     │
├──────────┼──────────────────────────────────────────────┤
│  Layer 0 │  引导层           (K_BUILD, K_BOOT, K_PANIC)  │
└──────────┴──────────────────────────────────────────────┘
```

---

## 二、完整原子插件目录（Plugin Registry）

### Layer 0 — 引导层

| 插件 ID      | 源文件路径                    | 职责说明                                                   | 依赖插件 |
|--------------|-------------------------------|------------------------------------------------------------|----------|
| `K_BUILD`    | `build.rs`, `.cargo/config`   | 配置交叉编译目标 (`x86_64-unknown-none`)、bootimage 打包、QEMU runner | — |
| `K_BOOT`     | `src/main.rs`                 | 内核入口 `_start`，读取 bootloader 传入的 `BootInfo`，初始化各层 | `K_BUILD` |
| `K_PANIC`    | `src/panic.rs`                | `#[panic_handler]`，显示 panic 信息并发出 `hlt` 停机指令   | `K_VGA`, `K_SERIAL` |

---

### Layer 1 — 硬件抽象层 (HAL)

| 插件 ID      | 源文件路径                    | 职责说明                                                   | 依赖插件 |
|--------------|-------------------------------|------------------------------------------------------------|----------|
| `K_VGA`      | `src/hal/vga_buffer.rs`       | 操作 `0xb8000` VGA 文本缓冲区；支持 16 色；实现全局 `println!` 宏；管理光标位置 | — |
| `K_SERIAL`   | `src/hal/serial.rs`           | UART 16550 串口驱动；用于調試输出；实现 `serial_println!` 宏 | — |
| `K_GDT`      | `src/hal/gdt.rs`              | 设置全局描述符表（GDT）；定义 TSS（任务状态段），供双重故障处理使用 | — |
| `K_IDT`      | `src/hal/interrupts.rs`       | 设置中断描述符表（IDT）框架；注册所有中断处理入口，包括 CPU 异常 | `K_GDT` |
| `K_PIC`      | `src/hal/pic.rs`              | 初始化并重映射 8259A 可编程中断控制器（IRQ 0-15 → 中断向量 32-47）| `K_IDT` |
| `K_PIT`      | `src/hal/pit.rs`              | 8253/8254 可编程间隔定时器（产生周期性时钟中断 IRQ0）；实现 `sleep_ms` 等待 | `K_PIC` |
| `K_PS2`      | `src/hal/ps2_kbd.rs`          | PS/2 键盘驱动；从端口 `0x60` 读取扫描码；维护按键缓冲队列；支持 US/ASCII 键盘映射 | `K_PIC`, `K_IDT` |
| `K_CPUID`    | `src/hal/cpuid.rs`            | 读取 CPUID 指令，获取 CPU 厂商、功能标志（如 SSE/AVX）；存入图属性节点 | `K_BOOT` |

---

### Layer 2 — 内存管理层

| 插件 ID      | 源文件路径                    | 职责说明                                                   | 依赖插件 |
|--------------|-------------------------------|------------------------------------------------------------|----------|
| `K_PMM`      | `src/mem/frame_alloc.rs`      | 物理页帧分配器；基于位图（Bitmap）管理物理内存；提供 `alloc_frame()` / `free_frame()` | `K_BOOT` BootInfo |
| `K_VMM`      | `src/mem/paging.rs`           | 4 级页表管理（PML4/PDPT/PD/PT）；页映射/取消映射；地址转换 | `K_PMM` |
| `K_HEAP`     | `src/mem/heap.rs`             | 实现 `GlobalAlloc` 接口，提供内核堆；支持 `alloc` crate（用于 `Vec`, `String`, `Box`） | `K_VMM`, `K_PMM` |

---

### Layer 3 — 并发与调度层

| 插件 ID      | 源文件路径                    | 职责说明                                                   | 依赖插件 |
|--------------|-------------------------------|------------------------------------------------------------|----------|
| `K_SPINLOCK` | `src/sync/spinlock.rs`        | 自旋锁 (`Mutex<T>`)；封装 `core::sync::atomic`；用于多处理器/中断安全访问 | — |
| `K_TASK`     | `src/task/task.rs`            | 定义协作式任务结构体（`Task`），包含 `Future` 和唯一 ID    | `K_HEAP` |
| `K_WAKER`    | `src/task/waker.rs`           | 实现 Rust Waker 机制，允许异步任务被唤醒重新调度           | `K_TASK` |
| `K_EXECUTOR` | `src/task/executor.rs`        | 协作式异步执行器；基于 BTreeMap 管理任务队列；驱动 `Future` 轮询 | `K_TASK`, `K_WAKER`, `K_HEAP` |

---

### Layer 4 — 图内核层 (GOS Core)

> **这是整个系统的核心**。图数据结构存在内存中，所有操作均通过原子插件实现。

| 插件 ID        | 源文件路径                       | 职责说明                                                   | 依赖插件 |
|----------------|----------------------------------|------------------------------------------------------------|----------|
| `K_NODE`       | `src/graph/node.rs`              | 定义 `GNode` 结构体：`id: u64`，`name: String`，`node_type: NodeType`（Enum: Plugin/Data/Process/File/Dir/Agent），`props: BTreeMap<String, Prop>` | `K_HEAP` |
| `K_EDGE`       | `src/graph/edge.rs`              | 定义 `GEdge` 结构体：`from: u64`，`to: u64`，`edge_type: EdgeType`（Enum: Dependency/DataFlow/Control/Ownership/Call），`weight: f32` | `K_HEAP` |
| `K_GRAPH`      | `src/graph/graph.rs`             | 图容器：使用 `BTreeMap<u64, GNode>` + `Vec<GEdge>`；提供 `add_node()`, `add_edge()`, `remove_node()`, `get_neighbors()` 基础 API | `K_NODE`, `K_EDGE`, `K_HEAP` |
| `K_TOPO`       | `src/graph/topo.rs`              | 基于 Kahn 算法的拓扑排序；检测环路（Cycle Detection）；输出有序执行序列 | `K_GRAPH` |
| `K_DFS`        | `src/graph/dfs.rs`               | 深度优先搜索；返回 DFS 遍历路径；支持前序/后序；用于依赖分析 | `K_GRAPH` |
| `K_BFS`        | `src/graph/bfs.rs`               | 广度优先搜索；按层级展开图；用于 `gos match` 的范围查询    | `K_GRAPH` |
| `K_QUERY`      | `src/graph/query.rs`             | 图查询引擎：支持按 `node_type`/`name`/`props` 过滤节点；支持简单路径模式匹配（仿 Cypher 语法子集）| `K_GRAPH`, `K_BFS`, `K_DFS` |
| `K_STORE`      | `src/graph/store.rs`             | RAM 图序列化：将图状态编码为紧凑字节数组；支持快照（Snapshot）与恢复；v0.1 存于静态内存缓冲区 | `K_GRAPH` |
| `K_ID_GEN`     | `src/graph/id_gen.rs`            | 单调递增 ID 生成器；线程安全（基于原子操作）；为节点和边分配唯一 `u64` ID | `K_SPINLOCK` |

---

### Layer 5 — 插件 SDK 层

| 插件 ID         | 源文件路径                       | 职责说明                                                   | 依赖插件 |
|-----------------|----------------------------------|------------------------------------------------------------|----------|
| `K_SDK`         | `src/sdk/plugin_trait.rs`        | 定义核心 Trait：`pub trait GosPlugin { fn id(&self) -> &str; fn execute(&self, ctx: &mut ExecCtx) -> PluginResult; }` | — |
| `K_CONTEXT`     | `src/sdk/context.rs`             | 执行上下文 `ExecCtx`：持有当前图的可变引用、命令参数、输出缓冲区；在插件调用链中传递状态 | `K_GRAPH`, `K_HEAP` |
| `K_RESULT`      | `src/sdk/result.rs`              | 定义 `PluginResult`（`Ok(Output)` / `Err(GosError)`）和错误类型枚举 | — |
| `K_REGISTRY`    | `src/sdk/registry.rs`            | 插件注册表：使用静态数组维护所有已注册插件的引用；提供 `find_plugin(id: &str)` 查找 | `K_SDK` |
| `K_PIPELINE`    | `src/sdk/pipeline.rs`            | 管道编排器：根据图拓扑顺序，依次调用多个插件；传递上下文；处理中间失败 | `K_TOPO`, `K_REGISTRY`, `K_CONTEXT` |

---

### Layer 6 — 内建命令插件层

> 每个 CLI 命令对应一个独立的插件，通过 `K_REGISTRY` 注册。

| 插件 ID       | 命令格式                                          | 职责说明                                            | 依赖插件 |
|---------------|---------------------------------------------------|-----------------------------------------------------|----------|
| `P_HELP`      | `gos help [command]`                              | 列出所有可用命令和用法                              | `K_SDK`, `K_VGA` |
| `P_STATUS`    | `gos status`                                      | 统计输出：节点总数、边总数、各类型节点占比          | `K_GRAPH`, `K_VGA` |
| `P_CREATE`    | `gos create node type=plugin name=foo`            | 创建新节点，分配 ID，写入 `props`，插入图           | `K_NODE`, `K_ID_GEN`, `K_GRAPH` |
| `P_LINK`      | `gos link foo -> bar type=dependency`             | 创建有向边，验证两端节点存在，写入图                | `K_EDGE`, `K_GRAPH` |
| `P_DELETE`    | `gos delete node foo` / `gos delete edge foo bar` | 从图中移除节点或边；自动清理悬空边                  | `K_GRAPH` |
| `P_LIST`      | `gos list [nodes\|edges\|type=plugin]`            | 按条件列出节点或边，分页显示                        | `K_QUERY`, `K_VGA` |
| `P_MATCH`     | `gos match (n)-[:dep]->(m)`                       | 简化 Cypher 模式查询；输出匹配的节点对              | `K_QUERY` |
| `P_GET`       | `gos get node foo`                                | 显示单个节点的全部属性                              | `K_GRAPH`, `K_VGA` |
| `P_SET`       | `gos set foo.version=2`                           | 修改节点的属性值                                    | `K_GRAPH` |
| `P_RUN`       | `gos run foo`                                     | 执行目标节点；解析依赖图；按拓扑顺序调用插件管道    | `K_TOPO`, `K_PIPELINE` |
| `P_SNAPSHOT`  | `gos snapshot save\|restore`                      | 将当前图状态序列化存入 RAM 缓冲；或恢复快照         | `K_STORE` |
| `P_CLEAR`     | `gos clear`                                       | 重置图为空状态                                      | `K_GRAPH` |

---

### Layer 7 — 执行引擎插件层

| 插件 ID          | 源文件路径                          | 职责说明                                                   | 依赖插件 |
|------------------|-------------------------------------|------------------------------------------------------------|----------|
| `P_EXEC_TOPO`    | `src/plugins/exec/topo_exec.rs`     | 拓扑执行引擎：对目标节点的完整依赖子图进行拓扑排序后顺序执行 | `K_TOPO`, `K_PIPELINE` |
| `P_EXEC_ASYNC`   | `src/plugins/exec/async_exec.rs`    | 将无依赖的节点包装为 `Task`，通过 `K_EXECUTOR` 并发执行   | `K_EXECUTOR`, `K_TASK` |
| `P_SANDBOX`      | `src/plugins/exec/sandbox.rs`       | 为图的子图创建隔离执行上下文；子图运行失败不影响主图状态   | `K_CONTEXT`, `K_GRAPH` |

---

### Layer 8 — 文件系统映射插件层

| 插件 ID       | 源文件路径                         | 职责说明                                                   | 依赖插件 |
|---------------|------------------------------------|------------------------------------------------------------|----------|
| `P_FS_MAP`    | `src/plugins/fs/fs_map.rs`         | 将 RamDisk 目录结构映射为 GOS 图节点（Dir/File 类型）      | `K_GRAPH`, `K_NODE` |
| `P_FS_READ`   | `src/plugins/fs/fs_read.rs`        | 读取文件节点内容，输出到 ExecCtx 的输出缓冲区              | `K_GRAPH`, `K_CONTEXT` |
| `P_FS_WRITE`  | `src/plugins/fs/fs_write.rs`       | 将 ExecCtx 输出缓冲区内容写回 RamDisk 文件                 | `K_GRAPH`, `K_CONTEXT` |
| `K_RAMDISK`   | `src/fs/ramdisk.rs`                | 基于静态字节数组的内存文件系统；支持简单文件/目录查询      | `K_HEAP` |

---

### Layer 9 — 向量与 Agent 插件层（v0.1 简化版）

| 插件 ID        | 源文件路径                          | 职责说明                                                   | 依赖插件 |
|----------------|-------------------------------------|------------------------------------------------------------|----------|
| `P_VECTOR`     | `src/plugins/ai/vector.rs`          | 将 `f32` 向量数组存入节点 `props["vector"]`；支持 L2 范数归一化 | `K_NODE`, `K_HEAP` |
| `P_COSINE`     | `src/plugins/ai/cosine.rs`          | 计算两节点向量的余弦相似度；输出分数                    | `P_VECTOR` |
| `P_ACTIVATE`   | `src/plugins/ai/activate.rs`        | 激活传播：将激活信号沿边传递，衰减因子为 `weight`；用于图神经网络原语 | `K_BFS`, `P_VECTOR` |
| `P_AGENT`      | `src/plugins/ai/agent.rs`           | Agent 模板：创建 `perception → planning → action` 三节点子图；自动连边 | `P_CREATE`, `P_LINK` |

---

### 内核 Shell 系统 (贯穿各层)

| 插件 ID          | 源文件路径                          | 职责说明                                                   | 依赖插件 |
|------------------|-------------------------------------|------------------------------------------------------------|----------|
| `K_SHELL`        | `src/shell/mod.rs`                  | Shell 主循环：`gos> ` 提示符，读取按键，维护当前命令行缓冲区 | `K_PS2`, `K_VGA` |
| `K_SHELL_LEXER`  | `src/shell/lexer.rs`                | 词法分析：将命令字符串切分为 Token（命令名、参数、箭头等）  | — |
| `K_SHELL_PARSER` | `src/shell/parser.rs`               | 语法解析：将 Token 序列解析为结构化 `Command` 枚举          | `K_SHELL_LEXER` |
| `K_SHELL_HIST`   | `src/shell/history.rs`              | 命令历史缓冲区（环形数组）；支持上下方向键翻历史             | `K_HEAP` |
| `K_SHELL_DISP`   | `src/shell/display.rs`              | Shell 显示专用器：带列对齐的输出格式化；分页（Page Up/Down） | `K_VGA` |

---

## 三、完整项目目录结构

```
GOSKernel/
├── .cargo/
│   └── config.toml          # 交叉编译目标 + QEMU runner 配置
├── Cargo.toml                # 依赖声明（no_std, bootloader, x86_64 等）
├── build.rs                  # bootimage 打包脚本
├── doc/
│   ├── design_v0_1_master_zh.md   ← 本文件（主设计文档）
│   ├── implementation_plan_v0_1_zh.md
│   ├── task_v0_1_zh.md
│   └── plugin_layer_deps.md        # 插件依赖有向图可视化
├── src/
│   ├── main.rs               # K_BOOT 入口
│   ├── panic.rs              # K_PANIC
│   ├── hal/
│   │   ├── vga_buffer.rs     # K_VGA
│   │   ├── serial.rs         # K_SERIAL
│   │   ├── gdt.rs            # K_GDT
│   │   ├── interrupts.rs     # K_IDT + K_PIC
│   │   ├── pit.rs            # K_PIT
│   │   ├── ps2_kbd.rs        # K_PS2
│   │   └── cpuid.rs          # K_CPUID
│   ├── mem/
│   │   ├── frame_alloc.rs    # K_PMM
│   │   ├── paging.rs         # K_VMM
│   │   └── heap.rs           # K_HEAP
│   ├── sync/
│   │   └── spinlock.rs       # K_SPINLOCK
│   ├── task/
│   │   ├── task.rs           # K_TASK
│   │   ├── waker.rs          # K_WAKER
│   │   └── executor.rs       # K_EXECUTOR
│   ├── graph/
│   │   ├── node.rs           # K_NODE
│   │   ├── edge.rs           # K_EDGE
│   │   ├── graph.rs          # K_GRAPH
│   │   ├── topo.rs           # K_TOPO
│   │   ├── dfs.rs            # K_DFS
│   │   ├── bfs.rs            # K_BFS
│   │   ├── query.rs          # K_QUERY
│   │   ├── store.rs          # K_STORE
│   │   └── id_gen.rs         # K_ID_GEN
│   ├── sdk/
│   │   ├── plugin_trait.rs   # K_SDK
│   │   ├── context.rs        # K_CONTEXT
│   │   ├── result.rs         # K_RESULT
│   │   ├── registry.rs       # K_REGISTRY
│   │   └── pipeline.rs       # K_PIPELINE
│   ├── shell/
│   │   ├── mod.rs            # K_SHELL (主循环)
│   │   ├── lexer.rs          # K_SHELL_LEXER
│   │   ├── parser.rs         # K_SHELL_PARSER
│   │   ├── history.rs        # K_SHELL_HIST
│   │   └── display.rs        # K_SHELL_DISP
│   ├── plugins/
│   │   ├── cmd/
│   │   │   ├── help.rs       # P_HELP
│   │   │   ├── status.rs     # P_STATUS
│   │   │   ├── create.rs     # P_CREATE
│   │   │   ├── link.rs       # P_LINK
│   │   │   ├── delete.rs     # P_DELETE
│   │   │   ├── list.rs       # P_LIST
│   │   │   ├── match.rs      # P_MATCH
│   │   │   ├── get.rs        # P_GET
│   │   │   ├── set.rs        # P_SET
│   │   │   ├── run.rs        # P_RUN
│   │   │   ├── snapshot.rs   # P_SNAPSHOT
│   │   │   └── clear.rs      # P_CLEAR
│   │   ├── exec/
│   │   │   ├── topo_exec.rs  # P_EXEC_TOPO
│   │   │   ├── async_exec.rs # P_EXEC_ASYNC
│   │   │   └── sandbox.rs    # P_SANDBOX
│   │   ├── fs/
│   │   │   ├── fs_map.rs     # P_FS_MAP
│   │   │   ├── fs_read.rs    # P_FS_READ
│   │   │   └── fs_write.rs   # P_FS_WRITE
│   │   └── ai/
│   │       ├── vector.rs     # P_VECTOR
│   │       ├── cosine.rs     # P_COSINE
│   │       ├── activate.rs   # P_ACTIVATE
│   │       └── agent.rs      # P_AGENT
│   └── fs/
│       └── ramdisk.rs        # K_RAMDISK
```

---

## 四、插件依赖图（文字描述版）

```
K_BUILD
  └── K_BOOT
        ├── K_PANIC ─ (K_VGA, K_SERIAL)
        ├── K_VGA
        ├── K_SERIAL
        ├── K_GDT
        │     └── K_IDT
        │           └── K_PIC
        │                 ├── K_PIT
        │                 └── K_PS2
        ├── K_PMM
        │     └── K_VMM
        │           └── K_HEAP
        │                 ├── K_SPINLOCK─ K_ID_GEN
        │                 ├── K_NODE
        │                 ├── K_EDGE
        │                 └── K_GRAPH ─── K_TOPO ── K_PIPELINE
        │                                 K_DFS
        │                                 K_BFS
        │                                 K_QUERY
        │                                 K_STORE
        │
        ├── K_TASK ─ K_WAKER ─ K_EXECUTOR
        │
        ├── K_SDK ─ K_RESULT ─ K_CONTEXT ─ K_REGISTRY ─ K_PIPELINE
        │
        └── K_SHELL ─ K_SHELL_LEXER ─ K_SHELL_PARSER ─ K_SHELL_HIST
                                                         K_SHELL_DISP
```

---

## 五、开发阶段与里程碑

### 🔴 阶段 0：引导冒烟（Boot Smoke Test）
**目标**: QEMU 能启动，打印 "GOS v0.1 booting..."。
**涉及插件**: `K_BUILD`, `K_BOOT`, `K_VGA`, `K_SERIAL`, `K_PANIC`

### 🟠 阶段 1：硬件就绪（HAL Ready）
**目标**: 键盘输入可用，定时器工作，CPU 异常有显示。
**涉及插件**: `K_GDT`, `K_IDT`, `K_PIC`, `K_PIT`, `K_PS2`

### 🟡 阶段 2：内存分配可用（Heap Online）
**目标**: 可以在内核中使用 `alloc::vec::Vec` 和 `alloc::string::String`。
**涉及插件**: `K_PMM`, `K_VMM`, `K_HEAP`

### 🟢 阶段 3：图内核上线（Graph Core）
**目标**: 可以在内存中创建、连接、遍历节点和边。
**涉及插件**: `K_NODE`, `K_EDGE`, `K_GRAPH`, `K_TOPO`, `K_DFS`, `K_BFS`, `K_QUERY`, `K_ID_GEN`, `K_STORE`

### 🔵 阶段 4：插件系统与 Shell（Plugin Shell）
**目标**: 出现 `gos> ` 提示符，可运行内建命令插件。
**涉及插件**: `K_SDK`, `K_CONTEXT`, `K_RESULT`, `K_REGISTRY`, `K_PIPELINE`, `K_SHELL*`, 以及全部 `P_*` 命令插件

### 🟣 阶段 5：执行引擎与并发（Execution Engine）
**目标**: `gos run` 能调度依赖图执行；异步任务可并发运行。
**涉及插件**: `K_TASK`, `K_WAKER`, `K_EXECUTOR`, `P_EXEC_TOPO`, `P_EXEC_ASYNC`, `P_SANDBOX`

### ⚪ 阶段 6：扩展能力（Extension）
**目标**: 文件系统映射、向量相似度、Agent 建图。
**涉及插件**: `K_RAMDISK`, `P_FS_*`, `P_VECTOR`, `P_COSINE`, `P_ACTIVATE`, `P_AGENT`

---

## 六、插件接口标准（Plugin SDK 规范）

```rust
// K_SDK: 所有插件必须实现此 Trait
pub trait GosPlugin: Send + Sync {
    /// 返回唯一插件 ID（如 "P_CREATE", "K_VGA"）
    fn id(&self) -> &'static str;

    /// 插件人类可读描述
    fn description(&self) -> &'static str;

    /// 执行插件逻辑，通过 ExecCtx 获取输入/输出/图引用
    fn execute(&self, ctx: &mut ExecCtx) -> PluginResult;
}

// K_CONTEXT: 执行上下文
pub struct ExecCtx<'a> {
    pub graph: &'a mut Graph,
    pub args: &'a [&'a str],     // 命令参数
    pub output: &'a mut VgaWriter, // 输出目标
    pub env: BTreeMap<&'static str, Prop>, // 环境变量
}

// K_RESULT: 结果类型
pub type PluginResult = Result<(), GosError>;

pub enum GosError {
    NodeNotFound(u64),
    CycleDetected,
    InvalidArgs(&'static str),
    StorageFull,
    PermissionDenied,
    Unknown,
}
```

---

## 七、`Cargo.toml` 依赖规划

```toml
[package]
name = "gos-kernel"
version = "0.1.0"
edition = "2021"

[dependencies]
bootloader = "0.9"          # 内核引导
x86_64 = "0.14"            # x86_64 硬件抽象（页表、端口 I/O 等）
uart_16550 = "0.2"          # K_SERIAL
pic8259 = "0.10"            # K_PIC
pc-keyboard = "0.5"         # K_PS2 扫描码解码
linked_list_allocator = "0.9" # K_HEAP
spinning_top = "0.2"        # K_SPINLOCK
conquer-once = "0.3"        # 一次性初始化原语（OnceCell for no_std）

[dependencies.futures-util]  # K_EXECUTOR
version = "0.3"
default-features = false
features = ["alloc"]

[[package.metadata.bootimage]]
test-args = ["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
             "-serial", "stdio", "-display", "none"]
```

---

## 八、v0.2 演进方向（本文档不实现）

| 功能         | 描述 |
|---|---|
| VGA 图形模式 | 从文本模式切换到 VESA 帧缓冲，实现像素级图形显示 |
| WASM 插件    | 将插件编译为 .wasm 字节码，通过内建 WASM 解释器动态加载 |
| 网络栈       | 实现 e1000 NIC 驱动 + TCP/IP 基础栈，实现图节点跨机传输 |
| Agent + RAG  | 将 LLM 的 embedding 与 GOS 向量能力对接 |
| Graph Kernel | 将图数据结构下沉至更底层，替代进程调度表 |
