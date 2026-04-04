# GOS Architecture v2 — Node Graph Runtime 完整系统设计

> **版本**: v2.0 (Phase 2 设计基准)  
> **平台**: x86_64 无标准库裸机  
> **语言**: Rust `#[no_std]` + 未来阶段引入 `alloc`  
> **核心公理**: 一切皆 Node（持有状态），一切皆 Edge（管理关系）

---

## 一、系统哲学与核心不变量

### 1.1 五条不变量（必须永远成立）

```
INV-1: 所有持久状态必须存在于某个 Node 的 NodeBlock 内，不得存在"游离"全局变量。
INV-2: 所有组件间交互必须经过 Edge，不得跨 Node 直接写另一个 Node 的状态字段。
INV-3: 每个 Node 拥有唯一向量地址 VectorAddress[L4:Domain, L3:Type, L2:Instance, Offset]。
INV-4: 每条 Edge 默认 acl_mask = u64::MAX（全连通）；仅在必要时收窄掩码限制通路。
INV-5: 系统的"启动完成"不是某函数返回，而是图达到 StableState（无未决激活边）。
```

### 1.2 Node 与 Edge 的职责边界

| 概念 | 持有 | 禁止 |
|---|---|---|
| **Node** | 状态、能力声明、NodeHeader | 直接读写其他 Node 内存 |
| **Edge** | 关系类型、ACL 掩码、权重、目标向量 | 持有业务状态 |
| **Plugin** | Node 集合 + Edge 集合 + Entry Node 声明 | 拥有全局执行流 |

---

## 二、向量地址空间（Vector Address Space）

所有 Node 和 Edge 都通过 48 位规范向量地址定位，由 `K_VADDR` 插件（Domain 1, Type 9）拥有并管理。

### 2.1 3-Nibble 规则

```
Bit layout (48-bit canonical address):
  0xFFFF_8 [L4: 8bit domain] [L3: 12bit type] [L2: 12bit instance] [Offset: 12bit]

映射示例:
  K_VGA  → VectorAddress[1, 1,  0, 0] → 0xFFFF_8_01_001_000_000
  K_META → VectorAddress[1, 10, 0, 0] → 0xFFFF_8_01_00A_000_000
```

### 2.2 域（Domain）划分

| L4 Domain | 用途 | 当前插件 |
|---|---|---|
| `0x00` | 系统保留（Bootstrap） | K_PANIC |
| `0x01` | HAL 域（硬件抽象） | K_VADDR, K_META, K_VGA, K_SERIAL... |
| `0x02` | 内存管理域 | K_PMM, K_VMM, K_HEAP (Phase 2) |
| `0x03` | 图内核域 | K_NODE, K_EDGE, K_GRAPH (Phase 3) |
| `0x04` | 调度域 | K_TASK, K_EXECUTOR, K_SCHED (Phase 3) |
| `0x05` | 插件注册域 | K_REGISTRY, K_LOADER (Phase 3) |
| `0x06` | 用户空间域 | P_* 插件群 (Phase 4+) |
| `0xFF` | AI 向量域 | K_VECTOR, K_AGENT (Phase 5+) |

### 2.3 NodeBlock 内存布局（4KB / block）

```
Offset 0x000 - 0x0FF  : NodeHeader (256B)  — 元数据、Magic、UUID、ACL
Offset 0x100 - 0x3FF  : EdgeRegistry (768B) — 12条 EdgeHeader × 64B
Offset 0x400 - 0xFFF  : NodeState (3072B)  — 具体插件数据（Writer/ChainedPics/...）
```

---

## 三、核心数据结构

### 3.1 NodeHeader (256B)

```rust
#[repr(C, align(64))]
pub struct NodeHeader {
    pub magic: u32,           // 0x474F5321 "GOS!"
    pub uuid: [u8; 16],       // 全局唯一 Node ID（未来由 K_UUID 生成）
    pub label: [u8; 16],      // 域标签 e.g. "HAL"
    pub name: [u8; 16],       // 节点名 e.g. "VGA"
    pub version: u32,         // 插件版本
    pub acl: u64,             // Node 级 ACL（0xFFFF = 全开）
    pub node_type: u16,       // NodeType 枚举（见 3.3）
    pub state_flags: u16,     // 状态标志位
    pub _res: [u8; 188],      // 预留扩展（向量嵌入、AI 标签等）
}
```

### 3.2 EdgeHeader (64B) — 含 Omni-Link 算法

```rust
#[repr(C, align(32))]
pub struct EdgeHeader {
    pub magic: u32,           // 0x45444745 "EDGE"
    pub edge_type: EdgeType,  // 语义类型（见 3.4）
    pub type_tag: [u8; 8],    // 辅助标签 e.g. "INIT"
    pub target_vec: u64,      // 目标 Node 的向量地址
    pub weight: f32,          // 边权重（调度优先级参考）
    pub acl_mask: u64,        // Omni-Link 掩码：MAX=全通，其他=按位过滤
    pub _res: [u8; 20],       // 预留（扩展为 EdgeState、回调地址等）
}

/// 检测调用方是否可以穿越此 Edge（O(1) 位运算）
impl EdgeHeader {
    pub fn permits(&self, caller_vec: u64) -> bool {
        self.acl_mask == u64::MAX                         // 全通（Omni-Link 默认）
            || (caller_vec & self.acl_mask) == self.target_vec  // 精确过滤
    }
}
```

### 3.3 NodeType 枚举

```rust
#[repr(u16)]
pub enum NodeType {
    /// 硬件抽象节点 — 对应物理硬件或固定内存映射
    Hardware    = 0x0001,
    /// 驱动节点 — 管理设备状态、提供 I/O 能力
    Driver      = 0x0002,
    /// 系统服务节点 — 逻辑服务（调度、内存管理等）
    Service     = 0x0003,
    /// 插件入口节点 — 每个插件的 Entry Node
    PluginEntry = 0x0010,
    /// 计算节点 — 执行一段算法并将结果写入输出 Edge
    Compute     = 0x0020,
    /// 路由节点 — 根据条件将激活信号转发到不同路径
    Router      = 0x0030,
    /// 聚合节点 — 等待多条输入 Edge 全部 Ready 后才激活
    Aggregator  = 0x0040,
    /// 向量节点 — 携带嵌入向量，支持 AI 语义检索
    Vector      = 0x00FF,
}
```

### 3.4 EdgeType 枚举（语义边类型）

```rust
#[repr(u8)]
pub enum EdgeType {
    /// 调用边：激活目标 Node 并等待其完成（同步语义）
    Call    = 0x01,
    /// 生成边：激活目标 Node 但不等待（异步/spawn 语义）
    Spawn   = 0x02,
    /// 依赖边：声明 "我依赖此 Node 必须先处于 Ready 状态"
    Depend  = 0x03,
    /// 信号边：发送非阻塞信号给目标 Node（中断/事件语义）
    Signal  = 0x04,
    /// 返回边：计算完成后将结果传递给调用方
    Return  = 0x05,
    /// 挂载边：将一个 Node 的能力接口挂载为另一个的子节点
    Mount   = 0x06,
    /// 同步边：协调两个 Node 的时间点（Barrier 语义）
    Sync    = 0x07,
    /// 数据流边：持续将数据从源 Node 流向目标 Node
    Stream  = 0x08,
    /// 继承边：目标 Node 继承源 Node 的部分状态（克隆/继承语义）
    Inherit = 0x09,
}
```

---

## 四、插件规范（Plugin Contract）

每个插件必须满足以下合约：

### 4.1 插件目录结构

```
pluginGroup/
└── K_EXAMPLE/
    ├── mod.rs          ← 插件根：声明 node/edge 子模块，导出 init()
    ├── node/
    │   └── mod.rs      ← Node 模块：定义状态结构、NODE_VEC、node_ptr()、init_node_state()
    └── edge/
        ├── mod.rs      ← 聚合所有 edge 子模块
        ├── init.rs     ← 初始化 Edge：注册元数据
        └── *.rs        ← 其他能力 Edge（call, signal, stream 等）
```

### 4.2 plugins.toml 规范（Phase 3 引入 K_LOADER 时读取）

```toml
[[plugin]]
id       = "K_VGA"
domain   = 1
type_id  = 1
entry    = "vga::edge::init::init"     # Entry Node 对应的激活函数
depends  = []
exports  = ["print", "set_color", "clear"]

[[plugin]]
id       = "K_GRAPH"
domain   = 3
type_id  = 1
entry    = "graph::edge::bootstrap::init"
depends  = ["K_HEAP", "K_VADDR", "K_META"]
exports  = ["node_create", "edge_link", "graph_query"]
```

### 4.3 Entry Node 协议

每个插件必须有且仅有一个 Entry Node。Entry Node 通过 `PluginEntry` NodeType 标识，由 Bootstrap 层在启动时自动扫描并激活。

```rust
// 每个插件的 edge/init.rs 是其 Entry Edge（等价于 Entry Node 的激活函数）
pub fn init() {
    unsafe {
        let p = node_ptr();
        burn_node_metadata(p, "DOMAIN_LABEL", "PLUGIN_NAME");
        init_node_state();
        // 注册自身的能力 Edge 到 EdgeRegistry
        burn_edge_metadata(p, 0, "INIT", 0x0); // slot 0: 自身已初始化信号
    }
}
```

---

## 五、Node Graph Runtime（节点图运行时）— Phase 3 核心

NGR 是整个 GOS 的调度心脏，位于 `Domain 0x03`，由 `K_GRAPH`、`K_NODE`、`K_EDGE` 三个插件组成。

### 5.1 NGR 职责

```
┌─────────────────────────────────────────────────────────────┐
│                  Node Graph Runtime (NGR)                    │
├──────────────┬──────────────┬──────────────┬────────────────┤
│  Node Pool   │  Edge Router │  Scheduler   │  State Machine │
│  (K_NODE)    │  (K_EDGE)    │  (K_SCHED)   │  (K_GRAPH)     │
│              │              │              │                │
│ - 分配/释放   │ - 路由激活信号 │ - Ready队列   │ - 系统状态机    │
│   NodeBlock  │ - ACL 检查   │ - 优先级调度  │ - StableState  │
│ - 管理生命周期 │ - 跨域转发   │ - 时间片控制  │   检测          │
│ - 索引向量地址 │ - Edge 类型  │ - 抢占/协作   │ - 热更新触发    │
│              │   分发策略   │   模式切换    │                │
└──────────────┴──────────────┴──────────────┴────────────────┘
```

### 5.2 Node 生命周期状态机

```
        ┌────────────────────────────────────────────┐
        │                                            │
        ▼                                            │
  [Unregistered]                                     │
        │ K_LOADER 扫描到插件元数据                   │
        ▼                                            │
   [Registered]                                      │
        │ Bootstrap 为其分配 NodeBlock 并烧录元数据   │
        ▼                                            │
    [Allocated]                                      │
        │ Entry Edge init() 执行完毕                  │
        ▼                                            │
     [Ready]  ◄──────────────────────────────────────┤
        │ NGR 调度器激活此 Node                       │
        ▼                                            │
    [Running]                                        │
        │ 执行完成，发出 Return/Signal Edge            │
        ▼                                            │
   [Suspended] ────────────────────────────────────►─┘
        │ 某条 Depend Edge 的上游 Node 被销毁
        ▼
   [Terminated]
```

### 5.3 图稳定状态（Stable State）检测

系统不再有"main 函数返回 = 启动完成"，改为：

```
StableState 条件:
  ∀ Node n ∈ RegisteredNodes:
    n.state == Ready || n.state == Suspended || n.state == Terminated
  ∧
  ∀ Edge e ∈ PendingEdges: e.target.state == Ready
  ∧
  Scheduler.ready_queue.is_empty()

达成 StableState 后，NGR 进入 EventLoop 模式（等待中断/外部触发）。
```

---

## 六、Bootstrap 引导层（当前 Phase 1 已实现）

```
Bootstrap 启动序列:

Phase -1: K_VADDR::init()   ← 分配 HAL_MATRIX（向量空间物理后备）
          K_META::init()    ← 建立 NodeHeader/EdgeHeader Schema

Phase 0:  K_PANIC::init()   ← 全局 Panic Handler 上线

Phase 1:  K_SERIAL::init()  ← 调试输出
          K_VGA::init()     ← VGA 文本缓冲

Phase 2:  K_GDT::init()     ← 全局描述符表
          K_IDT::init()     ← 中断描述符表
          K_PIC::init()     ← 中断控制器
          K_PIT::init()     ← 定时器
          K_PS2::init()     ← 键盘
          K_CPUID::init()   ← CPU 能力

Phase 3+: K_PMM → K_VMM → K_HEAP → K_GRAPH → K_NODE → K_EDGE → K_SCHED
          → K_REGISTRY → K_LOADER → P_* 用户插件
```

---

## 七、Phase 路线图

### Phase 1 ✅ — HAL 向量化（已完成）
- [x] K_VADDR: 向量地址空间 + HAL_MATRIX
- [x] K_META: NodeHeader/EdgeHeader + acl_mask Omni-Link 算法
- [x] 全部 HAL 插件（K_VGA/SERIAL/GDT/IDT/PIC/PIT/PS2/CPUID/PANIC）Node/Edge 化

### Phase 2 — 内存管理
- [ ] `K_PMM` (Domain 2, Type 1): 物理内存帧分配器（Buddy System）
- [ ] `K_VMM` (Domain 2, Type 2): 页表管理（4级页表 x86_64）
- [ ] `K_HEAP` (Domain 2, Type 3): 内核堆分配器（使能 `alloc` crate）

### Phase 3 — 节点图运行时（NGR）
- [ ] `K_NODE` (Domain 3, Type 1): Node Pool 管理 + 生命周期状态机
- [ ] `K_EDGE` (Domain 3, Type 2): Edge Router + ACL校验 + 类型分发
- [ ] `K_GRAPH` (Domain 3, Type 3): 全局图结构 + StableState 检测
- [ ] `K_SCHED` (Domain 4, Type 1): 调度器（Ready Queue + 优先级 + 时间片）
- [ ] `K_REGISTRY` (Domain 5, Type 1): 插件注册表
- [ ] `K_LOADER` (Domain 5, Type 2): 插件扫描 + Entry Node 激活

### Phase 4 — Shell 与用户空间
- [ ] `K_SHELL` (Domain 6, Type 1): 基于 Cypher 的命令行接口
  - `MATCH (n:HAL) RETURN n.name` — 枚举 HAL 节点
  - `CREATE (a:Node {name:"MyApp"})-[:CALL]->(b:Node {name:"VGA"})` — 动态创建
- [ ] `P_APP` 插件模型: 用户程序 = 插件，入口 = Entry Node

### Phase 5 — AI 原生调度
- [ ] `K_VECTOR` (Domain 0xFF, Type 1): Node/Edge 向量嵌入（为语义检索提供特征）
- [ ] `K_AGENT` (Domain 0xFF, Type 2): AI 调度代理，读取图状态，输出调度决策
  - 决策输入: 图邻接矩阵 + Node 向量 + Edge 权重 + 历史激活序列
  - 决策输出: 下一轮激活的 Node 集合 + Edge 优先级调整

---

## 八、Edge 路由算法

```
Edge 激活流程 (NGR Edge Router):

1. Source Node 执行完毕，生成 EdgeActivation { edge_slot, payload }
2. Edge Router 从 Source Node 的 EdgeRegistry[slot] 读取 EdgeHeader
3. ACL 检查: EdgeHeader.permits(source_node.vec_addr) → 拒绝则 drop
4. 按 EdgeType 分发:
   Call   → 将 target 加入 Ready Queue，Source 进入 Suspended 状态
   Spawn  → 将 target 加入 Ready Queue，Source 继续 Running
   Signal → 写入 target 的 SignalBuffer（无阻塞），Source 继续
   Depend → 检查 target 状态，若非 Ready 则 Source 进入 Waiting
   Return → Source 完成，唤醒等待它的 Caller Node
   Mount  → 将 target 注册为 source 的子 Node（树状结构）
   Sync   → 双方互等，形成 Barrier

5. Scheduler 从 Ready Queue 取下一个 Node 执行
```

---

## 九、Cypher 查询接口（Phase 4 实现，Phase 3 预留）

GOS 将 Cypher 作为系统 CLI 的查询/操作语言，取代传统命令行。

```cypher
// 查询所有 HAL 节点
MATCH (n:Node {domain: 1}) RETURN n.name, n.vec_addr, n.state

// 查找 VGA 节点可达的所有节点（2 跳内）
MATCH (vga:Node {name:"VGA"})-[:CALL|SIGNAL*1..2]->(target)
RETURN target.name, target.state

// 动态创建一条限制边（只允许 Domain 1 通过）
MATCH (src:Node {name:"PIC"}), (dst:Node {name:"IDT"})
CREATE (src)-[:SIGNAL {acl_mask: 0xFFFF000000000000}]->(dst)

// 激活一个 Node
MATCH (n:Node {name:"MyApp"}) SET n.state = "Ready"

// 查找处于 Running 状态的所有节点
MATCH (n:Node) WHERE n.state = "Running" RETURN n
```

---

## 十、AI 调度接口规范（Phase 5 预留）

### 10.1 图状态快照（AI 输入）

```rust
pub struct GraphSnapshot {
    /// 所有 Node 的向量表示（embedding）
    pub node_embeddings: &'static [[f32; 128]], // 每 Node 128维
    /// 邻接矩阵（稀疏 COO 格式）
    pub edge_src: &'static [u32],
    pub edge_dst: &'static [u32],
    pub edge_type: &'static [u8],
    pub edge_weight: &'static [f32],
    /// 当前 Ready Queue 长度
    pub ready_queue_len: usize,
    /// 当前系统时间（tick 计数）
    pub tick: u64,
}
```

### 10.2 AI 调度决策（AI 输出）

```rust
pub struct SchedulerDecision {
    /// 下一轮激活的 Node 列表（按优先级排序）
    pub activate_nodes: &'static [u64],        // Vec<VectorAddress as u64>
    /// 建议调整权重的 Edge 集合
    pub reweight_edges: &'static [(u64, f32)], // (edge_target_vec, new_weight)
    /// 建议暂停的 Node（负载均衡）
    pub suspend_nodes: &'static [u64],
}
```

---

## 十一、当前目录结构（Phase 1 完成态）

```
src/
├── main.rs                          ← K_BOOT (Entry Plugin)
└── pluginGroup/
    ├── mod.rs                       ← Plugin Group 注册表 + init_hal()
    ├── K_VADDR/ [Domain 1, Type 9]  ← 向量地址空间所有者
    │   ├── node/mod.rs              ← VectorAddress, HAL_MATRIX, NodeBlock
    │   └── edge/{ init.rs }
    ├── K_META/  [Domain 1, Type 10] ← 元数据 Schema 权威
    │   ├── node/mod.rs              ← NodeHeader, EdgeHeader, acl_mask
    │   └── edge/{ init.rs, burn.rs }
    ├── K_PANIC/ [Domain 1, Type 0]
    ├── K_VGA/   [Domain 1, Type 1]
    ├── K_SERIAL/[Domain 1, Type 2]
    ├── K_GDT/   [Domain 1, Type 3]
    ├── K_IDT/   [Domain 1, Type 4]
    ├── K_PIC/   [Domain 1, Type 5]
    ├── K_PIT/   [Domain 1, Type 6]
    ├── K_PS2/   [Domain 1, Type 7]
    └── K_CPUID/ [Domain 1, Type 8]
doc/
├── RULE_GRAPH_PRIME.md              ← 不变量准则（强制性）
└── GOS_ARCH_v2.md                   ← 本文档
```

---

*GOS: A system that remains correct as the world changes, and grows smarter as the graph grows denser.*
