# GOS 硬化日志 — V2.28

| 项目 | 内容 |
|---|---|
| 版本 | V2.28 |
| 日期 | 2026-07-01 |

## 摘要

V2.28 新增 `uname` / `ver` / `version` shell 命令，以及一个将所有编译期容量上限暴露为类型化 `RuntimeCapacity` 结构体的 `runtime_capacity()` 公开 API。这是 GOS 对应于 Linux 上 `uname -a` + `sysctl kern.*` + `getrlimit` 的等价物——让运维人员无需阅读源码即可查询正在运行的内核所支持的规模上限。

---

## 1. 变更目标

（见上文摘要：暴露所有编译期容量上限，使运维人员可在不阅读源码的前提下查询内核构建规格，对应 Linux 的 `uname -a` + `sysctl kern.*` + `getrlimit`。）

---

## 2. 修改清单

### 1. `RuntimeCapacity` 结构体 — gos-runtime (`crates/gos-runtime/src/lib.rs`)

从 gos-runtime 导出的新公开结构体：

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeCapacity {
    pub max_nodes: usize,           // MAX_NODES  = 128
    pub max_edges: usize,           // MAX_EDGES  = 512
    pub max_plugins: usize,         // MAX_PLUGINS = 32
    pub max_ready_queue: usize,     // MAX_READY_QUEUE = 256
    pub max_signal_queue: usize,    // MAX_SIGNAL_QUEUE = 512
    pub max_fault_queue: usize,     // MAX_FAULT_QUEUE = 32
    pub max_diff_ring: usize,       // MAX_DIFF_RING = 128
    pub max_node_trace: usize,      // MAX_NODE_TRACE = 16
    pub max_node_log: usize,        // MAX_NODE_LOG = 16
    pub max_subscribe_pairs: usize, // MAX_SUBSCRIBE_PAIRS = 64
    pub abi_major: u8,              // GOS_ABI_MAJOR = 2
    pub abi_minor: u8,              // GOS_ABI_MINOR = 0
    pub abi_patch: u16,             // GOS_ABI_PATCH = 0
    pub protocol_version: u16,      // CONTROL_PLANE_PROTOCOL_VERSION = 1
}
```

### 2. `runtime_capacity()` — gos-runtime 公开 API

```rust
pub fn runtime_capacity() -> RuntimeCapacity { ... }
```

- 纯常量读取——**无锁、无分配**。
- 从 gos-runtime 和 gos-protocol 中的编译期常量读取所有上限值。
- 永不 panic；始终返回一个有效的结构体。

### 3. `dispatch_uname()` — k-shell (`crates/k-shell/src/lib.rs`)

新的 shell 展示函数，打印如下内容：

```
 kernel info
  GOS v2.28 (graph-kernel)  abi: 2.0.0  protocol: 1
  capacity
    nodes:          N / 128
    edges:          N / 512
    plugins:        N / 32
    ready-queue:    256  signal-queue: 512  fault-queue: 32
    diff-ring:      128  subscribe-pairs: 64
    node-trace:     16 (ring depth per node)
    node-log:       16 (ring depth per node)
  arch: x86_64  no_std  tick: T
```

实时快照数值（当前节点/边/插件计数及 tick）与容量上限一并展示，使运维人员能够一目了然地了解资源利用情况。

### 4. `uname` / `ver` 路由 — k-shell (`crates/k-shell/src/proc.rs`)

添加在 `graph health` 分支之前：

```
uname        →  dispatch_uname(sink)
uname -a     →  dispatch_uname(sink)   [flag alias]
ver          →  dispatch_uname(sink)   [short alias]
version      →  dispatch_uname(sink)   [long alias]
```

帮助文本更新，加入 `uname` 和 `ver` / `version` 条目。

### 5. 测试套件 — `host-tests/gos-uname-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `capacity_max_nodes_matches_constant` | max_nodes == MAX_NODES (128) |
| 2 | `capacity_max_edges_matches_constant` | max_edges == MAX_EDGES (512) |
| 3 | `capacity_max_plugins_matches_constant` | max_plugins == MAX_PLUGINS (32) |
| 4 | `capacity_max_ready_queue_matches_constant` | max_ready_queue == MAX_READY_QUEUE (256) |
| 5 | `capacity_max_signal_queue_matches_constant` | max_signal_queue == MAX_SIGNAL_QUEUE (512) |
| 6 | `capacity_max_fault_queue_matches_constant` | max_fault_queue == MAX_FAULT_QUEUE (32) |
| 7 | `capacity_max_diff_ring_matches_constant` | max_diff_ring == MAX_DIFF_RING (128) |
| 8 | `capacity_max_node_trace_matches_constant` | max_node_trace == MAX_NODE_TRACE (16) |
| 9 | `capacity_max_node_log_matches_constant` | max_node_log == MAX_NODE_LOG (16) |
|10 | `capacity_abi_and_protocol_version_correct` | abi_major == 2, abi_minor == 0, protocol_version == 1 |

---

## 3. 测试结果

```
cd host-tests/gos-uname-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cargo build --release
# Finished `release` profile [optimized]
```

---

## 生产级质量依据

| 能力 | Linux/macOS 对应物 | GOS V2.28 |
|---|---|---|
| 内核版本 | `uname -a` | `uname` shell 命令 |
| 容量上限 | `getrlimit` / `sysctl kern.*` | `RuntimeCapacity` 结构体 |
| ABI 版本 | `/proc/version` | `abi_major.abi_minor.abi_patch` |
| 协议版本 | `/proc/net/protocols` | `protocol_version` 字段 |
| 实时利用率 | `free` / `vmstat` | 节点/边/插件的当前值 vs. 上限 |
| 零开销查询 | 读取 `/proc` | 纯常量——无锁、无分配 |

`runtime_capacity()` 是一次编译期的纯读取——不获取 Mutex 锁，不进行堆分配。它在中断上下文中调用也是安全的。

`RuntimeCapacity` 结构体带有 `#[derive(Debug, PartialEq, Eq)]`，使测试套件能够在一次断言中比较整个结构体，用于在 ABI 升级过程中对容量不变式进行回归测试。

---

## 4. 架构意义

`uname` 展示的是 GOS 的图容量上限（max_nodes、max_edges、max_subscribe_pairs），而非诸如内存大小或 CPU 数量等传统 OS 概念——使运维人员的心智模型始终锚定在定义 GOS 资源模型的图拓扑层。

---

## 5. 累计 host 测试数（V2.28）

| 套件 | 测试数 | 版本 |
|---|---|---|
| gos-runtime-harness | 26 | V2.2 |
| gos-supervisor-harness | 16 | V2.2 |
| gos-rewrite-harness | 12 | V2.3 |
| gos-rewrite-integration-harness | 6 | V2.3 |
| gos-subscribe-harness | 10 | V2.5 |
| gos-metrics-harness | 10 | V2.6 |
| gos-boot-harness | 11 | V2.9 |
| gos-node-inspect-harness | 8 | V2.8 |
| gos-journal-harness | 14 | V2.11 |
| gos-edge-inspect-harness | 10 | V2.12 |
| gos-graph-diff-harness | 10 | V2.13 |
| gos-proc-harness | 10 | V2.14 |
| gos-stat-harness | 10 | V2.15 |
| gos-graph-diff-epoch-harness | 10 | V2.16 |
| gos-graph-topo-harness | 10 | V2.17 |
| gos-graph-health-harness | 10 | V2.18 |
| gos-theme-node-harness | 10 | V2.19 |
| gos-plugin-list-harness | 10 | V2.20 |
| gos-kill-harness | 10 | V2.21 |
| gos-resume-harness | 10 | V2.22 |
| gos-node-info-harness | 10 | V2.23 |
| gos-node-trace-harness | 10 | V2.24 |
| gos-node-log-harness | 10 | V2.25 |
| gos-node-log-clear-harness | 10 | V2.26 |
| gos-node-trace-clear-harness | 10 | V2.27 |
| **gos-uname-harness** | **10** | **V2.28** |
| **合计** | **283** | |

---

*自动化硬化流程 — GOS V2.28 — 2026-07-01*
