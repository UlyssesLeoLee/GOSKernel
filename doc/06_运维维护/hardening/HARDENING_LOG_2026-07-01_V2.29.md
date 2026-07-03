# GOS 硬化日志 — V2.29

| 项目 | 内容 |
|---|---|
| 版本 | V2.29 |
| 日期 | 2026-07-01 |

## 摘要

V2.29 新增了逐节点信号计数重置功能，以及 `node stat clear` / `nstat clear` shell 命令，将 graph-OS 的计数器管理提升到与 Linux 上 `perf stat reset` 和 `echo 0 > /proc/<pid>/clear_refs` 相当的生产级标准。

这补齐了七命令的逐节点可观测性能力面，并为所有逐节点计数器实现了读/写对称：`stat` 暴露的每一个计数器现在都有对应的重置操作。

---

## 1. 变更目标

（见上文摘要：为 `stat` 暴露的逐节点计数器补齐重置能力，实现观测面的读/写对称，对应 Linux 的 `perf stat reset`。）

---

## 2. 修改清单

### 1. `reset_node_stat_inner()` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

`Runtime` 上新增的方法：

```rust
pub fn reset_node_stat_inner(&mut self, vector: VectorAddress) -> Result<(), RuntimeError> {
    let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
    let record = self.nodes[slot].as_mut().ok_or(RuntimeError::NodeNotFound)?;
    record.signal_count = 0;
    Ok(())
}
```

通过一次 `u32` 存储，将目标节点的 `NodeRecord::signal_count` 清零。不涉及 `node_trace`、`node_trace_count`、`node_log` 或任何其他逐节点状态。

### 2. `reset_node_stat()` — 公开 API（gos-runtime）

```rust
pub fn reset_node_stat(vec: VectorAddress) -> Result<(), RuntimeError> {
    RUNTIME.lock().reset_node_stat_inner(vec)
}
```

一层薄的加锁包装。若 vector 未注册，返回 `Err(RuntimeError::NodeNotFound)`。

### 3. `dispatch_node_stat_clear()` — k-shell (`crates/k-shell/src/lib.rs`)

新的公开分发函数。调用 `reset_node_stat()` 并输出一行颜色编码的状态信息：

- **绿色**：`node stat cleared  <vec>` + `signal_count -> 0  (trace ring and log unaffected)`
- **红色**：`node not found: <vec>`

### 4. Shell 路由 — k-shell (`crates/k-shell/src/proc.rs`)

`dispatch_text_command` 现在会匹配（插入在 `stat ` / `node stat ` 之前，以避免与 `node stat clear` 产生前缀冲突）：

```
node stat clear <vector>   →  dispatch_node_stat_clear(sink, vec)
nstat clear <vector>       →  dispatch_node_stat_clear(sink, vec)
```

帮助文本在 `stat` 小节下新增两条条目。

### 5. 测试套件 — `host-tests/gos-node-stat-clear-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `reset_stat_unknown_vector_returns_not_found` | 未知 vector → NodeNotFound |
| 2 | `reset_stat_fresh_node_returns_ok` | 全新节点（count == 0）→ Ok(()) |
| 3 | `reset_stat_zeroes_signal_count_after_one_dispatch` | 1 次分发 → 重置 → count == 0 |
| 4 | `reset_stat_zeroes_signal_count_after_many_dispatches` | 7 次分发 → 重置 → count == 0 |
| 5 | `reset_stat_is_idempotent` | 两次重置仍保持为 0 |
| 6 | `reset_stat_new_dispatches_increment_from_zero` | 5 次分发，重置，再 3 次 → count == 3 |
| 7 | `reset_stat_does_not_affect_sibling_node` | 重置 A → B 的计数不变 |
| 8 | `reset_stat_preserves_trace_ring` | 重置 stat → 追踪环形缓冲区条目保持完整 |
| 9 | `reset_stat_reflects_in_proc_page` | 重置后 proc_page 显示为 0 |
|10 | `reset_stat_returns_ok_for_live_node` | 对存活节点返回 Ok(()) |

---

## 3. 测试结果

```
cd host-tests/gos-node-stat-clear-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cargo build --release
# Finished `release` profile [optimized]
```

---

## 生产级质量依据

| 能力 | Linux/macOS 对应物 | GOS V2.29 |
|---|---|---|
| 计数器重置 | `perf stat reset` | `node stat clear <vec>` |
| 定向重置 | `echo 0 > /proc/<pid>/clear_refs` | `reset_node_stat()` API |
| 计数器隔离 | 单一计数器，无副作用 | 仅 `signal_count` 被清零 |
| 对称性 | 每个展示操作都有对应的清除操作 | stat/clear 配对完整 |
| 测量窗口 | `perf stat` 全新运行 | 清除后再分发 N 个信号 |
| 别名易用性 | 高频操作的简短别名 | `nstat clear <vec>` |

`reset_node_stat` 路径只获取一次 Mutex 锁并执行一次 `u32` 存储——与信号分发本身相比，开销几乎为零。

---

## 4. 架构意义

`node stat clear` 只作用于**计数器抽象**（signal_count）——图拓扑（边）、结构变更日志（diff ring）以及信号追踪环形缓冲区均保持不变。这保留了 GOS 的一项性质：可观测性工具永远不会破坏因果历史，只会重置特定的测量窗口。

图模型保持一致：为某个节点清除计数器不会级联影响其邻居节点，也不会改变任何边关系。

---

## 逐节点可观测性能力面 — 截至 V2.29 已完成

| 命令 | 对应物 | 描述 |
|---|---|---|
| `node info <vec>` | `systemctl status` | 当前状态快照 |
| `node trace <vec>` | `strace -p` | 信号分发历史 |
| `node trace clear <vec>` | `perf trace reset` | 丢弃信号追踪环形缓冲区 |
| `node log <vec>` | `journalctl -u` | 生命周期转换历史 |
| `node log clear <vec>` | `journalctl --vacuum-time` | 丢弃生命周期日志 |
| `stat <vec>` | `/proc/<pid>/status` | 包含 signal_count 的完整统计 |
| `node stat clear <vec>` | `perf stat reset` | 将 signal_count 重置为 0 **(V2.29)** |

七命令的逐节点可观测性能力面现已**全部完成**。

---

## 5. 累计 host 测试数（V2.29）

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
| gos-uname-harness | 10 | V2.28 |
| **gos-node-stat-clear-harness** | **10** | **V2.29** |
| **合计** | **293** | |

---

*自动化硬化流程 — GOS V2.29 — 2026-07-01*
