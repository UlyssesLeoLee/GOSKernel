# GOS 硬化日志 — V2.26

| 项目 | 内容 |
|---|---|
| 版本 | V2.26 |
| 日期 | 2026-07-01 |

## 摘要

V2.26 新增 `node log clear <vec>` / `nlog clear <vec>` shell 命令以及底层的 `clear_node_log()` 运行时 API，使运维人员能够在节点恢复后丢弃陈旧的生命周期历史——类似于 Linux 上的 `journalctl --vacuum-time` 或 `truncate -s0 /var/log/syslog`。

这完成了**逐节点生命周期日志管理三元组**：
- `node log <vec>` — 读取日志（V2.25）
- `node log clear <vec>` — 丢弃日志（V2.26，本版本）

---

## 1. 变更目标

（见上文摘要：补齐生命周期日志的清除能力，与 V2.25 的读取能力构成完整的读/写管理三元组。）

---

## 2. 修改清单

### 1. `clear_node_log_inner()` + `clear_node_log()` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

`GraphRuntime` 上新增的方法：

```rust
pub fn clear_node_log_inner(&mut self, vector: VectorAddress) -> Result<(), RuntimeError>
```

- 根据 `vector` 解析节点槽；若不存在则返回 `Err(RuntimeError::NodeNotFound)`。
- 将 `node_log[slot]` 清零为 `NodeLogEntry::EMPTY`，将 `node_log_head[slot]` 重置为 0，并将 `node_log_total[slot]` 重置为 0。
- 操作复杂度为 O(MAX_NODE_LOG)——环形缓冲区为 16 条目，开销可忽略不计。

新的公开 API 函数：

```rust
pub fn clear_node_log(vec: VectorAddress) -> Result<(), RuntimeError>
```

委托给 `RUNTIME.lock().clear_node_log_inner(vec)`。

### 2. `dispatch_node_log_clear()` — k-shell (`crates/k-shell/src/lib.rs`)

新增的公开函数，与 `dispatch_node_log()` 对应：

- 调用 `gos_runtime::clear_node_log(vec)`。
- 成功时：绿色确认行 `" node log cleared  <vec>"`。
- 出错时：红色 `" node not found: <vec>"`。

### 3. Shell 分发 — k-shell (`crates/k-shell/src/proc.rs`)

在现有的 `node log <vec>` 分支**之前**新增两个分发分支（以确保较长的 `"node log clear "` 前缀被优先匹配）：

```
node log clear <vec>   — clear lifecycle log
nlog clear <vec>       — alias for node log clear
```

帮助文本新增两条条目，说明其与 `--vacuum-time` 的对应关系。

---

## 测试套件 — `host-tests/gos-node-log-clear-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `clear_unknown_vector_returns_not_found` | 未知 vec → NodeNotFound |
| 2 | `clear_fresh_node_gives_zero_entries` | 清除后，(total=0, returned=0) |
| 3 | `clear_does_not_unregister_node` | 清除后 node_log_page 仍调用成功（节点存活） |
| 4 | `clear_discards_faulted_entry` | 清除操作移除 Faulted 条目 |
| 5 | `clear_discards_ready_entry` | 清除操作移除 Ready 条目 |
| 6 | `clear_is_idempotent` | 两次清除仍得到 (0, 0) |
| 7 | `clear_then_new_events_logged_correctly` | 清除后的事件被正确记录 |
| 8 | `clear_does_not_affect_sibling_node` | 清除 A 不影响 B 的日志 |
| 9 | `clear_resets_total_counter_to_zero` | 清除后 total=0；1 次事件后 total=1 |
|10 | `clear_returns_ok_for_live_node` | 对已注册节点返回 Ok(()) |

---

## 3. 测试结果

```
cd host-tests/gos-node-log-clear-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cargo build -p gos-runtime -p gos-protocol
# Finished dev profile — no errors
```

**回归验证：V2.25 `gos-node-log-harness` — 10 passed; 0 failed。**

---

## 生产级质量依据

| 能力 | Linux/macOS 对应物 | GOS V2.26 |
|---|---|---|
| 清除单服务日志 | `journalctl --vacuum-time=0 -u <svc>` | `node log clear <vec>` |
| 重启后清空 | `truncate -s0 /var/log/svc.log` | `nlog clear <vec>` |
| 幂等清除 | `journalctl --vacuum-time` 可安全重复执行 | 两次清除仍返回 Ok(()) |
| 兄弟节点隔离 | 日志按 unit 隔离 | 每个节点拥有独立的环形缓冲区 |

清除操作是一个**写 API**——它是 node-log 子系统中唯一的变更操作，且刻意设计为显式命令（`node log clear <vec>`），以防止意外数据丢失。

---

## 4. 架构意义

`node log clear` 作用于单个 vector 地址，保持在图原生的寻址模型内。日志环形缓冲区是一个逐节点的子资源，通过 GOS 运行时中通用的 vector 查找方式访问——不涉及 PID 到 unit 的映射，也不涉及文件名查找。

---

*自动化硬化流程 — GOS V2.26 — 2026-07-01*
