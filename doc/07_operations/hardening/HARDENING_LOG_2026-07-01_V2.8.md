# GOS 自动硬化日志 — 2026-07-01（第9次，V2.8 节点巡检命令）

> 类型：定期自动硬化（每2小时）  
> 目标：V2.8 可观测性 — `nodes` / `nodes faulted` / `nodes summary` Shell 命令（ps 类比）  
> 提交：`feat(v2.8): nodes/nodes-faulted/nodes-summary shell commands + gos-node-inspect-harness`

---

## 执行摘要

本次硬化围绕 **图操作系统的运行时可观测性**，为 Shell 层新增三条节点巡检命令，补全 OS 级别调试能力的关键缺口：

1. **`nodes` 命令** — 全量列出所有活跃图节点（类比 Linux `ps`）
2. **`nodes faulted`** — 仅显示 `NodeLifecycle::Faulted` 节点，快速故障定位
3. **`nodes summary`** — 生命周期分布统计（各状态节点数量一览）
4. **`gos-node-inspect-harness`** — 8 项测试验证 `node_page` / `node_exists_by_id` 的完整语义

全部测试绿灯：runtime 26 + supervisor 16 + rewrite 12 + integration 6 + subscribe 10 + metrics 7 + boot 8 + **node-inspect 8** = **93 项**。

---

## 架构动机

图论操作系统的核心主张是**一切皆图节点**——系统状态以图的形式存储和查询。  
任何真实 OS（Windows 任务管理器、Linux `ps`、iOS 活动监控器）都必须能枚举活跃进程/线程及其状态。

V2.7 的启动清单自愈实现了声明式依赖图，但操作员仍无法在 Shell 中直接看到
当前有哪些节点、它们处于什么状态。新增三条命令填补这一缺口：

- `nodes` 消费 `gos_runtime::node_page()` API（已有，V2.x 引入，用于图显示面板）
- 通过 **颜色编码生命周期**（绿=ready/run，黄=wait/suspend，红=fault）
  实现即时可读性
- `nodes faulted` 是 `nodes | grep fault` 的语义等价，减少故障排查跳转

---

## 变更详情

### 1. `crates/k-shell/src/lib.rs`（+103 行）

#### `pub fn dispatch_nodes_list(sink, faulted_only)`

逐页调用 `gos_runtime::node_page::<8>()`，对每个 `GraphNodeSummary` 打印：

```
  <vector>  <plugin_name>/<local_node_key>  <lifecycle>
```

生命周期颜色编码：

| 颜色 | 生命周期 |
|------|----------|
| 绿(10) | Ready / Running |
| 黄(14) | Waiting / Suspended |
| 红(12) | Faulted |
| 白(7) | boot-phase（Discovered / Loaded / Registered / Allocated）|
| 灰(8) | lifecycle label（低调排版，不抢焦点）|

当 `faulted_only = true` 时，`continue` 跳过非 Faulted 节点，无额外 API 调用。

#### `pub fn dispatch_lifecycle_summary(sink)`

同样使用 `node_page` 分页，按 `NodeLifecycle` 变体累积计数，输出分布表：

```
 node lifecycle summary
  boot-phase: 3
  ready:      12
  faulted:    1
  total:      16
```

使用 `macro_rules! print_count!` 宏消除重复（仅在计数 > 0 时打印）。  
宏展开为普通打印调用，零抽象开销。

#### 公共函数加入 `lifecycle_label()` 复用

两个新函数均复用已有的 `lifecycle_label()` / `print_num_inline()` / `LineBuf` 内部工具。

---

### 2. `crates/k-shell/src/proc.rs`（+10 行）

在 `dispatch_text_command` 的 `modules` 分支之后，`theme` 分支之前插入：

```rust
} else if cmd == "nodes" || cmd == "nodes all" {
    super::dispatch_nodes_list(sink, false);
} else if cmd == "nodes faulted" || cmd == "nodes fault" || cmd == "faults" {
    super::dispatch_nodes_list(sink, true);
} else if cmd == "nodes summary" || cmd == "nodes stat" {
    super::dispatch_lifecycle_summary(sink);
```

`help` 文本同步更新，在 `modules` 之后追加三行说明：

```
  nodes              list all live graph nodes (ps-style)
  nodes faulted      list only faulted nodes
  nodes summary      lifecycle distribution count
```

---

### 3. `host-tests/gos-node-inspect-harness/`（新增，8 项测试）

**依赖：** gos-protocol, gos-cypher-mut, gos-runtime, gos-supervisor(host-testing)

**`.cargo/config.toml`：** target = `x86_64-pc-windows-msvc`（同其他 harness）

**`tests/node_inspect.rs`：**

| # | 测试名 | 验证点 |
|---|--------|--------|
| 1 | `empty_runtime_node_page_returns_zero` | 空运行时 → `(0, 0)` |
| 2 | `single_node_returned_by_node_page` | 注册1节点 → `(1, 1)` 含正确 vector |
| 3 | `new_node_has_allocated_lifecycle` | 新注册节点生命周期 = Allocated |
| 4 | `all_registered_nodes_appear_in_node_page` | 3节点全返回，所有 vector 可找到 |
| 5 | `node_page_offset_beyond_total_returns_zero` | offset ≥ total → returned = 0 |
| 6 | `node_exists_by_id_tracks_registration` | 注册前 false，注册后 true |
| 7 | `node_page_is_sorted_ascending_by_vector_key` | 反序注册后 page 仍升序 |
| 8 | `register_node_is_idempotent` | 重复注册返回相同 NodeId，无重复节点 |

测试覆盖了 `dispatch_nodes_list` 所依赖的全部 `node_page` 语义不变式。

---

## 质量指标

| 指标 | 本次 | 前次（V2.7） |
|------|------|--------------|
| 测试总数 | **93** | 85 |
| Clippy 警告 | **0** | 0 |
| 新增测试 | **+8** | +8 |
| 新增 Shell 命令 | **+3** | — |
| 受影响 crate | 2（k-shell） | 4 |

---

## 图论 OS 特性维护

- **`node_page` 封装**：新命令完全通过已有 `gos_runtime::node_page()` API，  
  未绕开任何 capability gate 或信号路由——符合 Parity Invariant
- **无新机制**：复用排序缓存（`node_order_epoch` 懒更新）、`GraphNodeSummary` 结构、  
  `lifecycle_label()` 工具函数
- **产品级输出格式**：颜色编码 + 对齐列 + 空结果提示，与 `modules` 命令风格一致

---

## 下一步（V2.8 后续）

- [ ] VGA 层脏行追踪（epoch-diff hardware-level skip）
- [ ] PAL_U32 → 属性节点重构（Demo A 前置）
- [ ] `boot verify` 命令（调用 `verify_boot_manifest_graph()` 并显示自愈报告）
- [ ] V3 生态层规划（见 `plan/V3_DEVELOPMENT_PLAN.md`）

---

## 测试结果

```
host-tests/gos-runtime-harness:              26 passed, 0 failed
host-tests/gos-supervisor-harness:          16 passed, 0 failed
host-tests/gos-rewrite-harness:             12 passed, 0 failed
host-tests/gos-rewrite-integration-harness:  6 passed, 0 failed
host-tests/gos-subscribe-harness:           10 passed, 0 failed
host-tests/gos-metrics-harness:              7 passed, 0 failed
host-tests/gos-boot-harness:                 8 passed, 0 failed
host-tests/gos-node-inspect-harness:         8 passed, 0 failed  (新增)

总计：93 项测试全绿
```

---

*自动生成于 2026-07-01 定期硬化任务（第9次）*
