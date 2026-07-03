# GOS 硬化日志 — V2.22 — 2026-07-01

## 概述

V2.22 新增了 `resume_node()` API 以及 `resume <vec>` / `node resume <vec>`
shell 命令 —— 作为 V2.21 中 `kill <vec>` 的互补功能。二者共同构成了一套
完整的节点生命周期控制对：fault 用于使节点下线，resume 用于将其恢复为
Ready 状态而不将其从 graph 中移除。

这对应了 Linux 系统服务上可用的 `systemctl stop` / `systemctl start`
（或 `kill -9` / `systemctl restart`）生命周期控制，填补了 GOS 操作工具集
中的一个重要缺口。

---

## 修改清单

### 1. `resume_node()` —— gos-runtime（`crates/gos-runtime/src/lib.rs`）

`GraphRuntime` 上的新方法：

```rust
pub fn resume_node(&mut self, vector: VectorAddress) -> Result<(), RuntimeError>
```

- 按 `vector` 查找节点槽位；不存在时返回 `Err(NodeNotFound)`。
- 设置 `record.lifecycle = NodeLifecycle::Ready`。
- 发出 `StateDelta` 控制面事件以传播状态变更。
- **不会**推进 `graph_epoch`（生命周期变更不属于结构性变更）。
- **不会**触碰 fault queue（与 `fault_node` 不同，后者会加入队列以供
  supervisor 处理重启 —— resume 是一次直接的状态迁移）。

新增的公开自由函数包装：

```rust
pub fn resume_node(vector: VectorAddress) -> Result<(), RuntimeError> {
    RUNTIME.lock().resume_node(vector)
}
```

### 2. `dispatch_node_resume()` —— k-shell（`crates/k-shell/src/lib.rs`）

新增公开 shell 分发函数，与 `dispatch_node_kill()` 对称：

- 成功（`Ok`）时：绿色的 " resume: node ready" 标题，附带 vector 及新的
  生命周期状态。
- 失败（`Err`）时：红色的 "resume: node not found: <vec>" 错误信息。
- 页脚提示："use `proc` to verify new lifecycle state"。

### 3. Shell 命令路由 —— k-shell（`crates/k-shell/src/proc.rs`）

在 `dispatch_text_command` 中新增两个命令别名：

| 命令 | 动作 |
|---|---|
| `resume <vec>` | 调用 `dispatch_node_resume(sink, vec)` |
| `node resume <vec>` | 别名 —— 相同动作 |

帮助文本更新：
```
  resume <vector>      resume a faulted/suspended node (like systemctl restart)
  node resume <vector>   alias for resume
```

### 4. 测试套件 —— `host-tests/gos-resume-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `resume_node_unknown_vector_returns_not_found` | 未知 vector → `NodeNotFound` |
| 2 | `resume_node_on_faulted_node_returns_ok` | 故障节点 → Ok |
| 3 | `resume_node_sets_lifecycle_to_ready` | 生命周期变为 `Ready` |
| 4 | `resume_node_clears_faulted_count` | `faulted_node_count()` 降为 0 |
| 5 | `resume_node_does_not_bump_graph_epoch` | `graph_epoch` 不变 |
| 6 | `resume_node_preserves_signal_count` | resume 之后 `signal_count` 不变 |
| 7 | `resume_node_does_not_enqueue_fault_queue` | fault queue 保持为空 |
| 8 | `fault_then_resume_cycle_leaves_node_ready` | 完整的 fault→resume 往返 |
| 9 | `resume_node_idempotent_on_ready_node` | 对 Ready 节点执行 resume → 仍为 Ok + Ready |
|10 | `resume_one_of_two_faulted_nodes_leaves_one_faulted` | 双节点场景下的选择性 resume |

---

## 验证

```
cd host-tests/gos-resume-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

内核编译：
```
cargo build --release
# Finished `release` profile
```

---

## 生产质量说明

| 能力 | Linux/macOS 对应物 | GOS V2.22 |
|---|---|---|
| 使服务重新上线 | `systemctl start <unit>` | `resume <vec>` |
| 清除故障状态 | `systemctl reset-failed <unit>` | `resume <vec>`（设为 Ready） |
| 与 kill 配对 | `kill -9` + `systemctl restart` | `kill <vec>` + `resume <vec>` |
| 非破坏性 | 不将节点从 graph 中移除 | 保留所有节点元数据 |
| signal 计数保留 | `/proc/<pid>/stat` 在重启时被重置 | `signal_count` 保持不变 |
| epoch 稳定性 | 无 graph 拓扑变更 | `graph_epoch` 不被推进 |

`resume_node()` 函数是一次纯粹的生命周期状态翻转 —— 一次字段写入加一次
控制面事件发出 —— 零分配，无队列副作用。

---

## Graph-OS 特性的保留

`resume` 操作的对象是 **vector 地址**（graph 的天然键空间），而非不透明的
PID，使生命周期控制始终扎根于 graph 拓扑。`kill <vec>` / `resume <vec>`
这一互补对，将故障注入与恢复表达为一等 graph 操作 —— 这在扁平 PID 操作系统
中没有直接对应物。

---

## Shell 命令一览（累计至 V2.22）

| 命令 | 新增于 | 描述 |
|---|---|---|
| `kill <vec>` / `node fault <vec>` | V2.21 | 强制使节点故障化 |
| `resume <vec>` / `node resume <vec>` | **V2.22** | **恢复一个故障节点** |
| `plugins` / `lsmod` | V2.20 | 插件清单 |
| `graph health` | V2.18 | 整体健康报告 |
| `proc` / `ps` | V2.14 | ps 风格的节点表 |
| `stat <vec>` | V2.15 | 单节点深度统计 |
| `graph diff <N>` | V2.16 | 自 epoch N 以来的 diff |
| `graph topo` | V2.17 | L4 域拓扑视图 |

---

*自动化硬化流程 — GOS V2.22 — 2026-07-01*
