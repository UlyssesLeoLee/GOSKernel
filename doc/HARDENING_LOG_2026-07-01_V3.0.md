# GOS 自动硬化日志 — 2026-07-01（第11次，V3.0 图边列表命令）

> 类型：定期自动硬化（每2小时）  
> 目标：V3.0 可观测性 — `edges` / `edges summary` Shell 命令（netstat 类比）  
> 提交：`feat(v3.0): edges/edges-summary shell commands + gos-edge-inspect-harness`

---

## 执行摘要

本次硬化补全了 **图论操作系统运行时可观测性**的最后一块拼图：V2.8 已有 `nodes`/`nodes faulted`/`nodes summary` 枚举节点，但图论 OS 的核心数据结构是**节点 + 边**，缺少边枚举是一个明显的观测缺口。

1. **`edges` / `edges all` / `edge list`** — 全量列出所有活跃图边（类比 Linux `netstat -r` / `ip route`）
2. **`edges summary` / `edges stat` / `edge types`** — 按边类型统计分布（call/spawn/depend/signal/return/mount/sync/stream/use）
3. **`gos-edge-inspect-harness`** — 8 项测试全面验证 `edge_page` / `edge_page_for_node` / `unregister_edge` 语义

全部测试绿灯：runtime **26** + supervisor **16** + rewrite 12 + integration 6 + subscribe 10 + metrics 7 + boot **11** + node-inspect **8** + **edge-inspect 8** = **104 项**。

---

## 架构动机

图论操作系统的核心主张是**一切皆图**——系统状态以图的形式（节点 + 边）存储和查询。  
任何真实 OS（Linux `netstat`、Windows 路由表、iOS 网络诊断）都能显示当前连接/路由状态。

V2.9 的 `boot verify` 展示了启动时的边自愈摘要，V2.8 的 `nodes` 可以列举节点；
但在运行时，操作员仍无法在 Shell 中看到当前图上存在哪些边、它们的类型和方向。

**问题**：缺少边枚举让 GOS 的图论特性变成了一个不可观测的黑盒。

**方案**：`gos_runtime::edge_page<N>` 已经存在于 V2.x（用于图显示面板 `k-vk-host`），
但 Shell 层从未暴露。本次直接复用该 API，无需新增 ABI，零内核改动。

---

## 变更详情

### 1. `crates/k-shell/src/lib.rs`（+157 行）

#### 新增辅助函数（3 个）

```rust
fn edge_type_color(ty: RuntimeEdgeType) -> u8
fn route_policy_label(policy: RoutePolicy) -> &'static str
```

注：`edge_type_label` 已存在于 lib.rs:1441，直接复用，无重复定义。

#### `pub fn dispatch_edges_list(sink: &ConsoleSink)`

逐页调用 `gos_runtime::edge_page::<8>()`，对每个 `GraphEdgeSummary` 渲染：

```
  <from_vector>-[<edge_type>]-><to_vector>  <route_policy>
```

边类型颜色编码：

| 颜色 | 边类型 |
|------|--------|
| 青(11) | Signal |
| 绿(10) | Depend |
| 黄(14) | Call |
| 洋红(13) | Mount / Use |
| 白(7) | Spawn / Return / Stream / Sync |

#### `pub fn dispatch_edge_type_summary(sink: &ConsoleSink)`

同样使用 `edge_page` 分页，按 `RuntimeEdgeType` 变体累积计数，输出分布表：

```
 edge type summary
  call:    3
  depend:  5
  signal: 12
  total:  20
```

使用 `macro_rules! print_edge_count!` 宏，仅在计数 > 0 时打印，零值类型不占行。

---

### 2. `crates/k-shell/src/proc.rs`（+7 行）

在 `boot verify` 分支之后插入：

```rust
} else if cmd == "edges" || cmd == "edges all" || cmd == "edge list" {
    super::dispatch_edges_list(sink);
} else if cmd == "edges summary" || cmd == "edges stat" || cmd == "edge types" {
    super::dispatch_edge_type_summary(sink);
```

`help` 文本同步更新，追加两行说明：

```
  edges              list all live graph edges (netstat-style)
  edges summary      edge type distribution count
```

---

### 3. `host-tests/gos-edge-inspect-harness/`（新增，8 项测试）

**依赖：** gos-protocol, gos-cypher-mut, gos-runtime, gos-supervisor(host-testing)

**`.cargo/config.toml`：** target = `x86_64-pc-windows-msvc`（同其他 harness）

**`tests/edge_inspect.rs`：**

| # | 测试名 | 验证点 |
|---|--------|--------|
| 1 | `empty_runtime_edge_page_returns_zero` | 空运行时 → `(0, 0)` |
| 2 | `single_edge_returned_by_edge_page` | 注册1条边 → `(1, 1)`，from/to/type 正确 |
| 3 | `edge_page_is_sorted_ascending_by_edge_vector_key` | 反序注册后 page 仍升序 |
| 4 | `all_registered_edges_appear_in_edge_page` | 三种类型边全返回 |
| 5 | `edge_page_offset_beyond_total_returns_zero` | offset ≥ total → returned = 0 |
| 6 | `edge_page_preserves_edge_type` | `edge_type` 字段经 `edge_page` round-trip 正确 |
| 7 | `unregister_edge_removes_from_edge_page` | `unregister_edge` 后 total = 0 |
| 8 | `edge_page_for_node_filters_by_node` | 按节点过滤边，A=2, B=2, C=2 |

所有测试覆盖 `dispatch_edges_list` 和 `dispatch_edge_type_summary` 所依赖的 `edge_page` 全部不变式。

---

## 质量指标

| 指标 | 本次 | 前次（V2.9） |
|------|------|--------------|
| 测试总数 | **104** | 96 |
| Clippy 警告 | **0**（新增代码无新警告） | 0 |
| 新增测试 | **+8**（edge-inspect harness） | +3 |
| 新增 Shell 命令 | **+2**（`edges` / `edges summary`） | +1 |
| 受影响 crate | 1（k-shell，零新内核 API） | 3 |

---

## 图论 OS 特性维护

- **节点 + 边完整可见**：`nodes`（V2.8）+`edges`（V3.0）合力实现图拓扑的完整运行时可观测性，达到 Linux `ps`+`netstat` 的操作级等价
- **零内核 API 增加**：`edge_page` 早已存在（`gos_runtime` lib.rs:1885），本次仅暴露到 Shell 层——符合"先内核，后界面"的硬化原则
- **类型安全枚举**：所有 9 种 `RuntimeEdgeType` 变体均有颜色编码和标签，无 `_` 漏网分支
- **复用 push_vector**：`from_vector`/`to_vector` 均为 `VectorAddress`，直接使用已有 `LineBuf::push_vector`，无新格式化代码
- **Parity Invariant 保持**：`dispatch_edges_list` 通过 `edge_page` 只读接口访问图状态，未绕过任何 capability gate

---

## 下一步（V3.0 后续）

- [ ] `edges for <vector>` — 单节点边枚举（调用 `edge_page_for_node`，已有 harness 覆盖）
- [ ] VGA 层脏行追踪（epoch-diff hardware-level skip）
- [ ] PAL_U32 → 属性节点重构（Demo A 前置）
- [ ] `metrics export` 命令（将 telemetry 写入 FAT32 日志节点）
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
host-tests/gos-boot-harness:               11 passed, 0 failed
host-tests/gos-node-inspect-harness:         8 passed, 0 failed
host-tests/gos-edge-inspect-harness:         8 passed, 0 failed  (新增)

总计：104 项测试全绿
```

---

*自动生成于 2026-07-01 定期硬化任务（第11次）*
