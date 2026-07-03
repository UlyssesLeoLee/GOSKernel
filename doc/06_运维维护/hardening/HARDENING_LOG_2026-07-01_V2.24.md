# GOS 硬化日志 — V2.24

| 项目 | 内容 |
|---|---|
| 版本 | V2.24 |
| 日期 | 2026-07-01 |

## 摘要

V2.24 新增了逐节点信号追踪环形缓冲区，以及 `node trace <vec>` / `ntrace <vec>` shell 命令，将 graph-OS 的信号可观测性提升到与 Linux 上 `strace -p <pid>` 相当的生产级标准。现在每一次信号分发都会被记录进一个 16 条目的逐节点环形缓冲区；shell 命令以「最新在前」的顺序渲染该环形缓冲区，列出 kind、cmd、序列号和发送方 vector。

---

## 1. 变更目标

（见上文摘要：为 graph-OS 补齐信号可观测性能力，达到与 `strace -p <pid>` 相当的水准。）

---

## 2. 修改清单

### 1. `NodeTraceEntry` — gos-protocol (`crates/gos-protocol/src/lib.rs`)

从 gos-protocol 导出的新公开结构体：

```rust
pub struct NodeTraceEntry {
    /// Sender's raw vector address (0 for kernel-initiated signals).
    pub from:   u64,
    /// signal_count value just before this dispatch (monotonic sequence number).
    pub serial: u32,
    /// Signal kind discriminant — matches KernelSignalKind u8 values.
    /// 0 = EMPTY sentinel (no signal recorded in this ring slot yet).
    pub kind:   u8,
    /// Control: cmd byte.  Interrupt: irq byte.  Data: data byte.  Others: 0.
    pub cmd:    u8,
}
```

`NodeTraceEntry::EMPTY` 作为环形数组的 const 初始值提供。

### 2. 逐节点追踪环形缓冲区 — gos-runtime (`crates/gos-runtime/src/lib.rs`)

新增常量：
```rust
pub const MAX_NODE_TRACE: usize = 16;
```

为 `GraphRuntime` 新增的字段：
- `node_trace: [[NodeTraceEntry; MAX_NODE_TRACE]; MAX_NODES]` — 每个节点槽一个的追踪环形缓冲区。
- `node_trace_head: [u8; MAX_NODES]` — 每个槽的下一写入位置。

内存开销：128 × 16 × 16 字节 = **32 KB** 静态分配（对裸机内核而言可接受）。

`prepare_signal_dispatch()` 修改为接受 `signal: Signal` 参数：
- 新增 `signal_trace_fields(signal)` 辅助函数，用于提取 `(kind, from, cmd)`。
- 在递增 `signal_count` **之前**，先将追踪条目记录到 `node_trace[slot][head]`，使 `serial` 等于该信号的索引（从 0 开始）。
- 以回绕方式推进 `node_trace_head[slot]`。

新的公开方法 `GraphRuntime::node_trace_page()`：
- 从 head 开始反向读取环形缓冲区（最新在前）。
- 返回 `(total_signals, entries_written)`。

新的公开 API 包装函数：
```rust
pub fn node_trace_page(
    vec: VectorAddress,
    out: &mut [NodeTraceEntry; MAX_NODE_TRACE],
) -> Result<(u32, usize), RuntimeError>
```

### 3. `node trace` / `ntrace` shell 命令 — k-shell

`lib.rs` 中新增的分发函数 `dispatch_node_trace(sink, vec)`：
- 头部：节点 vector + 总分发次数 + 环形缓冲区填充数。
- 列标题：`seq | kind | cmd | from`
- 对信号种类进行颜色编码：绿色=call，品红=spawn，蓝色=irq，白色=data，黄色=control，红色=term。
- 打印发送方 vector（从 `from` 字段解码得出），对于系统发起的信号则打印 `kernel`。
- 页脚：指向 `node info` 和 `proc` 的提示。

Shell 分发 (`proc.rs`)：
- `node trace <vec>`、`ntrace <vec>` → `dispatch_node_trace(sink, vec)`
- `help` 文本更新，加入 `node trace` 和 `ntrace` 条目。

### 4. 测试套件 — `host-tests/gos-node-trace-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `trace_unknown_vector_returns_not_found` | 未知 vec 返回 NodeNotFound |
| 2 | `trace_fresh_node_returns_zero_entries` | 全新节点：(0, 0) |
| 3 | `trace_one_dispatch_returns_one_entry` | 一次信号 → (1, 1) |
| 4 | `trace_entry_kind_matches_control` | Control 信号 → kind == 0x05 |
| 5 | `trace_entry_cmd_matches_control_cmd` | Control.cmd 传播到 entry.cmd |
| 6 | `trace_data_signal_kind_and_cmd` | Data 信号 → kind == 0x04，cmd == byte |
| 7 | `trace_first_entry_serial_is_zero` | 首次分发 serial == 0 |
| 8 | `trace_second_entry_serial_is_one` | 第二次分发 serial == 1；验证最新在前 |
| 9 | `trace_ring_wraps_after_max_entries` | MAX+4 个信号 → returned == MAX_NODE_TRACE |
|10 | `trace_newest_first_ordering` | ring[0].cmd == 最后发送的 cmd |

---

## 3. 测试结果

```
cd host-tests/gos-node-trace-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cargo build --release (workspace root)
# Finished `release` profile
```

---

## 生产级质量依据

| 能力 | Linux/macOS 对应物 | GOS V2.24 |
|---|---|---|
| 实时信号追踪 | `strace -p <pid>` | `node trace <vec>` / `ntrace <vec>` |
| 信号历史 | `/proc/<pid>/syscall`（最近一次系统调用） | 16 条目的环形追踪缓冲区 |
| 分发序号 | strace 序号 | `serial` 字段（基于 signal_count） |
| 信号种类 | 系统调用名称 | `kind` 判别值 + 标签列 |
| 发送方身份 | 调用进程 PID | `from` vector 地址 |
| 子信号负载 | 系统调用参数 | `cmd` 字节（Control/Interrupt/Data） |

追踪环形缓冲区是一种始终开启、零拷贝的被动日志：每次分发只需向预分配的静态数组写入一个结构体——除 `prepare_signal_dispatch` 中已有的 `RUNTIME.lock()` 之外，不引入堆分配，也不引入额外的锁争用。

---

## 4. 架构意义

`node trace` 展示了 GOS 原生的**信号驱动执行模型**：与展示 OS 调用的 Unix strace 不同，GOS 的追踪展示的是节点间的信号消息，保留了图拓扑抽象。`from` vector 字段使图通信的来源可见，这在扁平进程 OS 中没有直接对应物。

---

## V2.24 之后的累计 host 测试套件

- 22 个套件下共 **243 个测试**（全部通过）
- 新增：`gos-node-trace-harness` — 10 个测试

---

*自动化硬化流程 — GOS V2.24 — 2026-07-01*
