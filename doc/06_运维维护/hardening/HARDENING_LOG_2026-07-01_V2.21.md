# GOS 硬化日志 — V2.21 — 2026-07-01

## 概述

V2.21 新增了 `kill <vec>` / `node fault <vec>` shell 命令，以及底层的
`fault_node()` runtime API，赋予操作者从控制台手动使任意存活 graph 节点
故障化的能力 —— 这是 Linux/Unix 上 `kill -9 <pid>` 在 graph-OS 中的对应物。
这填补了一个关键的操作者控制缺口：此前节点进入 `NodeLifecycle::Faulted`
状态的唯一途径是 CPU 异常，或原生插件返回 `ExecStatus::Fault`。

---

## 修改清单

### 1. `fault_node()` API —— gos-runtime（`crates/gos-runtime/src/lib.rs`）

#### `impl GraphRuntime` 方法

```rust
pub fn fault_node(&mut self, vector: VectorAddress) -> Result<(), RuntimeError>
```

- 按 `vector` 解析节点槽位；未知的 vector 返回 `Err(NodeNotFound)`。
- 设置 `record.lifecycle = NodeLifecycle::Faulted`。
- 调用 `state_delta(node_id, Faulted)` —— 发出 `StateDelta` 控制面事件。
- 将 `vector` 加入 `fault_queue`（supervisor 的重启策略会在下一次 `pump()`
  tick 时排空该队列）。
- **不会**推进 `graph_epoch` —— 故障化是生命周期状态变更，而非结构性拓扑
  变更。这使得 `graph diff` / `graph health` 的 diff 保持整洁。

#### 模块级包装函数

```rust
pub fn fault_node(vector: VectorAddress) -> Result<(), RuntimeError>
```

标准的 `RUNTIME.lock().fault_node(vector)` 委托调用，与其他所有 gos-runtime
公开 API 保持一致。

### 2. `dispatch_node_kill()` —— k-shell（`crates/k-shell/src/lib.rs`）

在 `dispatch_node_stat` 之后新增展示函数：

- 成功时（绿色）：打印 `kill: node faulted`、该 vector、生命周期迁移文字，
  以及运行 `nodes faulted` 以确认的提示。
- 失败时（红色）：打印 `kill: node not found: <vec>`。

### 3. Shell 命令 —— k-shell（`crates/k-shell/src/proc.rs`）

三个分发到 `dispatch_node_kill` 的命令别名：

| 命令 | 说明 |
|---|---|
| `kill <vec>` | 主要形式 —— 对应 Unix 的 `kill -9` |
| `node fault <vec>` | 详细形式 —— 明确表达 graph-OS 意图 |
| `fault <vec>` | 交互式使用的简短别名 |

帮助文本更新，包含两种形式及说明。

### 4. 测试套件 —— `host-tests/gos-kill-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `fault_node_unknown_vector_returns_not_found` | 无效 vector → Err(NodeNotFound) |
| 2 | `fault_node_registered_returns_ok` | 已注册节点 → Ok(()) |
| 3 | `fault_node_sets_lifecycle_to_faulted` | kill 之后 proc_stat 显示 Faulted |
| 4 | `fault_node_does_not_remove_node_from_graph` | fault 之后节点仍在 proc_page 中 |
| 5 | `fault_node_enqueues_to_fault_queue` | drain_next_fault 返回该 vector |
| 6 | `fault_node_increases_faulted_node_count` | faulted_node_count 从 0 变为 1 |
| 7 | `fault_node_idempotent_on_already_faulted_node` | 重复 fault → 仍为 Ok，计数保持 1 |
| 8 | `fault_node_does_not_bump_graph_epoch` | fault 之后 graph_epoch 不变 |
| 9 | `fault_node_preserves_signal_count` | signal_count 不因 fault 而重置 |
| 10 | `two_fault_nodes_enqueue_two_vectors` | 两次 kill → 两次出队，之后为 None |

---

## 验证

```
cd host-tests/gos-kill-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

内核编译：
```
cargo build --release
# Finished `release` profile
```

回归验证：
```
cd host-tests/gos-proc-harness && cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
cd host-tests/gos-graph-health-harness && cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

---

## 生产质量说明

| 能力 | Linux/macOS 对应物 | GOS V2.21 |
|---|---|---|
| 强制终止进程 | `kill -9 <pid>` / `SIGKILL` | `kill <vec>` / `node fault <vec>` |
| 即时生命周期变更 | 内核将其从运行队列移除 | 生命周期 → Faulted，加入 fault_queue |
| 通知 supervisor | 内核向父进程发送 SIGCHLD | fault_queue 在下一次 pump() 时排空，触发重启策略 |
| 拓扑保留 | 进程从 PID 表中移除 | 节点仍留在 graph 中（graph_epoch 不变） |
| 可观测性 | `ps` 短暂显示僵尸进程后消失 | `nodes faulted` 显示该故障节点 |

关键设计决策：`fault_node` **不会**将节点从 graph 中移除，因为 GOS 节点是
带有附属边的 graph 顶点 —— 拓扑移除是一个独立的操作（`unregister_edge` +
未来的 `unregister_node`），必须考虑依赖的订阅者和重写规则。故障化只改变
生命周期状态；下一步（重启、降级或排空）由 supervisor 的重启策略决定。

---

## Graph-OS 特性的保留

`kill <vec>` 通过节点的 **vector 地址**（一个 graph 坐标）而非不透明的
数字 PID 来定位节点，使操作者的心智模型始终扎根于 graph 拓扑。
`node fault <vec>` 使这一意图更加明确：你是在对一个 graph 顶点执行生命周期
变更，而不是向一个扁平进程发送 Unix 信号。

---

*自动化硬化流程 — GOS V2.21 — 2026-07-01*
