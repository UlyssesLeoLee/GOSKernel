# GOS 硬化日志 — V2.25

| 项目 | 内容 |
|---|---|
| 版本 | V2.25 |
| 日期 | 2026-07-01 |

## 摘要

V2.25 新增了逐节点生命周期事件日志，以及 `node log <vec>` / `nlog <vec>` shell 命令——这是 graph-OS 对应于 `journalctl -u <service>` 的等价物。现在每一次生命周期转换（`Registered → Allocated → Running → Ready → Faulted → Ready` 等）都会被记录进一个带有单调 tick 时间戳的 16 槽逐节点环形缓冲区，为运维人员提供每个图节点自启动以来完整演化过程的审计轨迹。

---

## 1. 变更目标

（见上文摘要：为每个图节点建立生命周期转换的完整审计轨迹，对应 Linux 的 `journalctl -u <service>`。）

---

## 2. 修改清单

### 1. `NodeLogEntry` — gos-protocol (`crates/gos-protocol/src/lib.rs`)

从 gos-protocol 导出的新公开结构体：

```rust
pub struct NodeLogEntry {
    pub tick:      u64,   // monotonic runtime tick at transition time
    pub lifecycle: u8,    // NodeLifecycle discriminant (e.g. 0xFF = Faulted)
    pub _pad:      [u8; 7],
}
impl NodeLogEntry {
    pub const EMPTY: Self = Self { tick: 0, lifecycle: 0, _pad: [0u8; 7] };
}
```

### 2. 逐节点生命周期日志环形缓冲区 — gos-runtime (`crates/gos-runtime/src/lib.rs`)

- 新增常量 `MAX_NODE_LOG: usize = 16`。
- 为 `GraphRuntime` 新增三个字段：
  - `node_log: [[NodeLogEntry; MAX_NODE_LOG]; MAX_NODES]` — 每个节点槽的环形存储。
  - `node_log_head: [u8; MAX_NODES]` — 每个节点槽的下一写入指针。
  - `node_log_total: [u16; MAX_NODES]` — 累计记录的转换总数（饱和于 u16::MAX）。
- `GraphRuntime::new()` 将以上三者全部初始化为 EMPTY / 零值。
- `state_delta()`——每次生命周期变化时调用的唯一内部钩子——现在也会向该节点的日志环形缓冲区推入一条 `NodeLogEntry { tick, lifecycle }`。这在快速路径上是零开销的：只需一次数组写入加一次 `saturating_add`。
- 新增 `node_log_page()` 实现方法：返回最新在前、上限为 MAX_NODE_LOG 条目的结果。
- 新增全局 `node_log_page()` 包装函数（沿用 `node_trace_page` 的模式）。
- 在协议导入代码块中，`NodeLogEntry` 与 `NodeTraceEntry` 一并导入。

### 3. `node log` shell 命令 — k-shell (`crates/k-shell/src/lib.rs`、`crates/k-shell/src/proc.rs`)

`lib.rs` 中新增的分发函数 `dispatch_node_log()`：

- 头部：节点标识符 + 事件总数 + 显示数量。
- 表格：`tick | lifecycle label` —— 每一行代表一次转换。
- 对生命周期进行颜色编码：绿色 = Ready/Registered，红色 = Faulted，黄色 = Running，青色 = Suspended，灰色 = Discovered/Terminated，白色 = 其他。
- 辅助函数 `lifecycle_log_entry()` 将原始 u8 判别值映射为标签 + 颜色。
- 页脚提示：`node trace <vec> for signal history | ninfo <vec> for full view`。

Shell 分发 (`proc.rs`)：
- `node log <vec>` / `nlog <vec>` → `dispatch_node_log(sink, vec)`
- `help` 文本更新，新增两条条目。

### 4. 测试套件 — `host-tests/gos-node-log-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `log_unknown_vector_returns_not_found` | 未注册 vector 返回 NodeNotFound |
| 2 | `log_fresh_node_has_no_entries` | register 后，API 调用成功且环形缓冲区有效 |
| 3 | `log_contains_allocated_after_register` | register_node 时记录 Allocated 的 state_delta |
| 4 | `log_faulted_entry_after_fault_node` | fault_node() 后最新条目为 Faulted |
| 5 | `log_ready_entry_after_resume_node` | resume_node() 后最新条目为 Ready |
| 6 | `log_newest_first_ordering` | fault → resume → fault：[0]=Faulted，[1]=Ready |
| 7 | `log_total_increases_with_events` | total 随每次生命周期变化严格递增 |
| 8 | `log_ring_wraps_after_max_entries` | 环形缓冲区填满：溢出后返回值 == MAX_NODE_LOG |
| 9 | `log_faulted_discriminant_is_0xff` | Faulted 生命周期 == 0xFF，与 #[repr(u8)] 规格一致 |
|10 | `log_two_nodes_independent` | 节点 A 的 fault 不会出现在节点 B 的日志中 |

---

## 3. 测试结果

```
cd host-tests/gos-node-log-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

内核构建：
```
cargo build --release
# Finished `release` profile [optimized]
```

---

## 生产级质量依据

| 能力 | Linux/macOS 对应物 | GOS V2.25 |
|---|---|---|
| 服务生命周期日志 | `journalctl -u <service>` | `node log <vec>` / `nlog <vec>` |
| 审计轨迹 | systemd unit journal | 每节点 16 槽环形缓冲区，附带 tick 时间戳 |
| 最新在前视图 | `journalctl -u svc --reverse` | 设计上始终最新在前 |
| 故障/恢复历史 | `journalctl -u svc \| grep -E 'Start\|Stop\|Failed'` | 捕获全部转换 |
| 零开销记录 | systemd journal（独立进程） | `state_delta()` 内联环形缓冲区写入 |
| 环形缓冲区深度 | 可配置 | 16 条目 (MAX_NODE_LOG) |

生命周期日志与 `node trace <vec>`（信号分发历史）以及 `node info <vec>`（静态快照）相辅相成，构成完整的逐节点可观测性三件套：

| 命令 | 对应物 | 展示内容 |
|---|---|---|
| `node info <vec>` | `systemctl status <svc>` | 当前状态快照 |
| `node trace <vec>` | `strace -p <pid>` | 最近的信号分发 |
| `node log <vec>` | `journalctl -u <svc>` | 生命周期转换历史 |

---

## 4. 架构意义

生命周期日志记录的转换既由**图结构事件**驱动（节点注册、触发订阅者信号的边变更），也由操作员命令（`kill`、`resume`）驱动。`tick` 字段将每次转换与图运行时自身的单调时钟绑定，使可观测性根植于 GOS 的事件循环模型，而非对图 OS 而言外来的墙钟抽象。

---

*自动化硬化流程 — GOS V2.25 — 2026-07-01*
