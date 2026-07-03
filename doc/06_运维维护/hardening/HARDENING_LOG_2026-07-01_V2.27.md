# GOS 硬化日志 — V2.27

| 项目 | 内容 |
|---|---|
| 版本 | V2.27 |
| 日期 | 2026-07-01 |

## 摘要

V2.27 新增 `node trace clear <vec>` / `ntrace clear <vec>`——一个用于丢弃逐节点信号分发追踪环形缓冲区的 shell 命令与 API，与 `node log clear`（V2.26）对称。这补齐了可观测性四件套的清除操作：生命周期日志和信号追踪环形缓冲区现在都可以独立丢弃，且不影响累计的 proc 统计数据。

---

## 1. 变更目标

（见上文摘要：为信号追踪环形缓冲区补齐清除能力，与生命周期日志的清除操作 `node log clear`（V2.26）对称，完成可观测性四件套的清除侧。）

---

## 2. 修改清单

### 1. `node_trace_count` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

为 `GraphRuntime` 新增了逐节点计数器数组 `node_trace_count: [u32; MAX_NODES]`：

- **用途**：追踪自上次清除以来，已写入追踪环形缓冲区的信号分发次数，独立于（被 `proc` 使用的累计型）`signal_count`。
- **初始化**：在 `GraphRuntime::new()` 中初始化为 `[0u32; MAX_NODES]`。
- **递增**：`prepare_signal_dispatch()` 在每次追踪写入时，与现有的 `signal_count` 一起，通过 `saturating_add(1)` 递增 `node_trace_count[slot]`。
- **为何要分开**：`signal_count` 是绝不能被重置的单调 proc 指标；`node_trace_count` 是环形缓冲区层面的总数，会在清除时重置，从而为追踪环形缓冲区提供与 `node_log_total` 之于日志环形缓冲区相同的 `(total=0, returned=0)` 语义。

`node_trace_page()` 更新为使用 `self.node_trace_count[slot]` 而非 `record.signal_count` 作为 `total_traced` 的返回值。

### 2. `clear_node_trace_inner()` — gos-runtime

`GraphRuntime` 上新增的方法：

```rust
pub fn clear_node_trace_inner(&mut self, vector: VectorAddress) -> Result<(), RuntimeError> {
    let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
    self.node_trace[slot] = [NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    self.node_trace_head[slot] = 0;
    self.node_trace_count[slot] = 0;
    Ok(())
}
```

- 清零追踪环形缓冲区条目，重置头指针，并重置 `node_trace_count`。
- `NodeRecord` 内部的 `signal_count` 刻意保持不变——`proc` 统计数据依然有效。
- 对未注册的 vector 返回 `NodeNotFound`。

### 3. `clear_node_trace()` — gos-runtime 公开 API

```rust
pub fn clear_node_trace(vec: VectorAddress) -> Result<(), RuntimeError> {
    RUNTIME.lock().clear_node_trace_inner(vec)
}
```

### 4. `dispatch_node_trace_clear()` — k-shell (`crates/k-shell/src/lib.rs`)

新的 shell 分发函数：

- 成功时：以绿/灰色打印 `" node trace cleared  <vec>"`。
- 出错时：以红色打印 `" node not found: <vec>"`。
- 与 `dispatch_node_log_clear()`（V2.26）对应。

### 5. `node trace clear` / `ntrace clear` 路由 — k-shell (`crates/k-shell/src/proc.rs`)

Shell 路由被添加在现有的 `node trace <vec>` 分支**之前**，使 `"node trace clear X"` 先于 `"node trace X"` 被匹配：

```
node trace clear <vector>   →  dispatch_node_trace_clear(sink, vec)
ntrace clear <vector>       →  dispatch_node_trace_clear(sink, vec)   [alias]
```

帮助文本更新，加入这两个新命令。

### 6. 测试套件 — `host-tests/gos-node-trace-clear-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `clear_unknown_vector_returns_not_found` | 未注册 vector → NodeNotFound |
| 2 | `clear_fresh_node_gives_zero_entries` | 对全新节点清除 → (0, 0) |
| 3 | `clear_does_not_unregister_node` | 清除后节点仍可访问 |
| 4 | `clear_discards_single_dispatch_entry` | 1 次分发后清除 → (0, 0) |
| 5 | `clear_discards_multiple_dispatch_entries` | 5 次分发后清除 → (0, 0) |
| 6 | `clear_is_idempotent` | 两次清除仍返回 (0, 0) |
| 7 | `clear_then_new_dispatches_traced_correctly` | 清除后环形缓冲区全新；新的 kind/cmd 正确 |
| 8 | `clear_does_not_affect_sibling_node` | 清除 A 不影响 B 的追踪数据 |
| 9 | `clear_resets_total_counter_to_zero` | 清除后 total=0，下一次分发后为 1 |
|10 | `clear_returns_ok_for_live_node` | 对已注册节点返回 Ok(()) |

---

## 3. 测试结果

```
cd host-tests/gos-node-trace-clear-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cd host-tests/gos-node-trace-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed   (backward compat — total now uses node_trace_count)

cargo build --release
# Finished `release` profile [optimized]
```

---

## 生产级质量依据

| 能力 | Linux/macOS 对应物 | GOS V2.27 |
|---|---|---|
| 清除追踪缓冲区 | `perf trace reset` / `truncate -s0 strace.log` | `node trace clear <vec>` |
| 选择性清除（单进程） | 重启 `strace -p <pid>` | `ntrace clear <vec>` |
| 保留 proc 统计 | `signal_count` 不受影响 | ✓ 独立的 `node_trace_count` |
| 幂等 | 清除空缓冲区是安全的 | ✓ 两次清除安全 |
| 未知目标报错 | 无操作或报错 | ✓ `NodeNotFound` |

---

## 4. 架构意义

`node trace clear` 作用于图**vector 地址**（而非扁平 PID），强化了 GOS 以拓扑为根基的身份模型：每一次可观测性操作都通过节点在图中的位置来命名该节点。

---

## 可观测性四件套 — 已完成

| 命令 | 对应物 | 版本 |
|---|---|---|
| `node info <vec>` | `systemctl status` | V2.23 |
| `node trace <vec>` | `strace -p <pid>` | V2.24 |
| `node trace clear <vec>` | `perf trace reset` | **V2.27** |
| `node log <vec>` | `journalctl -u <svc>` | V2.25 |
| `node log clear <vec>` | `journalctl --vacuum-time` | V2.26 |

两个可观测性环形缓冲区现在都具备对称的读取 + 清除操作。

---

*自动化硬化流程 — GOS V2.27 — 2026-07-01*
