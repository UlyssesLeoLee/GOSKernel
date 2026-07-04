# GOS 硬化日志 — V2.51（2026-07-02）

## 版本号: V2.51
## 功能: `node checkpoint` — 节点状态观测性快照

---

## 变更摘要

实现 `node checkpoint <vec>` —— 一个图原生的观测性原语，将活跃节点的当前状态以 `GraphDiffKind::NodeCheckpoint` 条目的形式快照到结构 diff ring 中。类比 `perf record --event=mark` 或 `gdb checkpoint`：在调用瞬间捕获节点的向量地址、key、signal_count、生命周期状态和 edge_out_count，既不修改节点也不推进图 epoch。

---

## 变更内容

### `crates/gos-protocol/src/lib.rs`

- 为 `GraphDiffKind` 新增 `NodeCheckpoint = 4` 变体（`#[repr(u8)]`）
- 更新 `is_node()`，将 `NodeCheckpoint` 纳入其中（使 diff 展示层将其渲染为节点样式条目——显示向量+标签，而非边对）
- `is_addition()` 对 `NodeCheckpoint` 返回 `false`（它既非新增也非删除——只是一个观测性标记）

### `crates/gos-runtime/src/lib.rs`

- 新增 `GraphRuntime::node_checkpoint_inner(vector)`：
  - 通过 `proc_stat_for_vector` 按 `VectorAddress` 解析节点
  - 调用 `push_diff(GraphDiffKind::NodeCheckpoint, vector, ZERO, key_bytes)`
  - **不**推进图 epoch——只推进 `diff_ring_head` 和 `diff_total`
  - 返回 `Ok(NodeProcSummary)` 或 `Err(RuntimeError::NodeNotFound)`
- 新增公开函数 `node_checkpoint(vec) -> Result<NodeProcSummary, RuntimeError>`

### `crates/k-shell/src/lib.rs`

- 新增 `dispatch_node_checkpoint(sink, vec)`：
  - `Err` 时：打印红色"未找到节点"
  - `Ok` 时：打印捕获的 key、生命周期（按颜色编码）、signal_count、edge_out_count，并提示可通过 `graph diff` 查看该条目
- 更新 `dispatch_graph_diff` 对 `GraphDiffKind` 的匹配：
  - `NodeCheckpoint` 以黄色（fg=14）渲染为 `[ckpt  ]`，前缀 `·`
  - 保留四种结构类型的对齐标签填充

### `crates/k-shell/src/proc.rs`

- 在 `node stat clear` 分支之前接入 `node checkpoint <vec>` / `ncp <vec>` / `checkpoint <vec>`

### `host-tests/gos-node-checkpoint-harness/`（新建）

- Cargo.toml、.cargo/config.toml（host target override）、tests/node_checkpoint.rs
- 10 个测试，0.01 秒内全绿

---

## Shell 接口

| 命令 | 别名 | 动作 |
|------|------|------|
| `node checkpoint <vec>` | `ncp <vec>`、`checkpoint <vec>` | 快照节点状态 → diff ring |

**快照后的展示：**
```
 node checkpoint  28.1.1.0
  key:          cp.alpha
  lifecycle:     running
  signal_count:  0
  edge_out:      1
  → recorded in diff ring as [ckpt]  (graph diff to view)
```

**`graph diff` 输出（NodeCheckpoint 条目）：**
```
 · [ckpt  ] 28.1.1.0  cp.alpha
```

---

## 测试用例（10 项）

| 编号 | 用例 | 预期 |
|------|------|------|
| 1 | 空图，未知向量 | `Err(NodeNotFound)` |
| 2 | 有节点的图，未知向量 | `Err(NodeNotFound)` |
| 3 | 已知节点 → Ok | 返回 signal_count=0 |
| 4 | diff ring 填充量增加 1 | fill(之后) = fill(之前) + 1 |
| 5 | 图 epoch 不变 | 前后相同 |
| 6 | diff 条目类型 | `GraphDiffKind::NodeCheckpoint` |
| 7 | diff 条目 from_vector | 等于被快照节点的向量 |
| 8 | diff 条目 label | 等于节点的 local_node_key |
| 9 | 两次快照 | fill 增长 2 |
| 10 | 带边的节点 | summary 中 edge_out_count 正确 |

**全部通过 10/10**

---

## 不变量确认

- [x] 现有所有 diff ring 调用方不受影响——`push_diff` 路径未改变
- [x] `graph_epoch` 不推进——checkpoint 是纯观测性标记
- [x] 同一提交中同步更新了 `dispatch_graph_diff` 中 `GraphDiffKind` 的穷尽匹配（无潜在的非穷尽匹配告警）
- [x] `is_node()` 对 `NodeCheckpoint` 返回 `true`——diff 渲染器显示向量+标签（与节点事件一致，而非边事件）
- [x] 宿主测试套件：483 个（此前 473 + 新增 10）—— 全绿

---

## 后续建议（V2.52+ 候选）

- `graph sim <N>` —— 模拟 N 步随机游走，输出信号流量轨迹
- `graph between` —— 通过全对 Dijkstra 计算的有向带权介数中心性
- PAL_U32 → attribute node 重构（Demo A 前置条件）

---

*由自动强化任务生成 · 2026-07-02*
