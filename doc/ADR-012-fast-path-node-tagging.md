# ADR-012：Fast-path 标签节点——给已经在跑的 bulk-read 模式一个名字和门禁

> 状态：**提案待选向** · 提案日期：2026-06-12 · 配套：[V2 计划风险表 line 136](../plan/V2_DEVELOPMENT_PLAN.md)（"真热路径走 fast-path 标签节点"）、[ADR-002](./ADR-002-rewrite-engine.md)（"性能需 fast-path 标签节点兜底"）、[V3 计划 line 27](../plan/V3_DEVELOPMENT_PLAN.md)（"性能逃生舱走 fast-path 标签节点，ADR-012 待起草"）、[ADR-006 选项 A](./ADR-006-capability-graph-migration.md)（影子验证/等价性证明方法论，本 ADR 复用）、[ADR-014 §3.1/§四](./ADR-014-process-as-subgraph-compat-strategy.md)（wasm 解释器把本 ADR 列为解释开销的长期答案）
>
> 口径：三份文档三次承诺"fast-path 标签节点"是 GOS"重可表达性 > 极限性能"取舍的逃生舱，但都没定义"标签"是什么、贴在哪、解锁什么。同时，`k-vk-host::render_live_graph`（B3b）已经在跑一个具体机制——bulk `node_page`/`edge_page` 读，绕过逐节点 `on_event` 派发——却没有名字、没有声明式开关、没有治理脚本可见性。本 ADR 不是"设计新机制"，是"命名已经在跑的机制，给它一个 governance-visible 的入口和等价性义务"——与 ADR-007/009/010 同型的"文档落后于现实"，只是这次现实跑在了文档前面。

## 一、问题陈述

### 1.1 三处承诺，零定义

- V2 计划风险表（line 136）："性能上限低于命令式（中风险）| GOS 重可表达性 > 极限性能。真热路径（rasterizer/DMA）走 fast-path 标签节点。"
- ADR-002 §（图 IS 场景选项 A 的代价）："renderer 重写量大；性能需 fast-path 标签节点兜底。"
- V3 计划 line 27（诚实清单）："性能逃生舱走 fast-path 标签节点（ADR-012 待起草，任务 #43）。"

三处都假定读者知道"fast-path 标签节点"指什么——但 `crates/` 里没有任何 `FastPath`/`fast_path` 字符串（V2.6c 起草前已确认零命中）。这是一个被反复引用、从未定义的占位符。

### 1.2 `render_live_graph` 已经是这个模式的事实实现

`k-vk-host::render_live_graph`（`crates/k-vk-host/src/lib.rs:249`）：

```rust
/// Walk the live runtime graph and emit it as one `@gos.vk` frame.
///
/// Safe to call from `on_event`: `route_signal` releases the runtime lock
/// before invoking the executor, so the `node_page`/`edge_page` locks taken
/// here cannot deadlock. ...
fn render_live_graph() {
    ...
    let (total, _) = interrupts::without_interrupts(|| gos_runtime::node_page::<PAGE>(0, &mut probe));
    ...
}
```

这正是 V2 line 136 所说的"rasterizer 热路径"：渲染一帧时，不对每个 node 触发一次 `NodeExecutorVTable::on_event` 派发，而是**分页批量读** `node_page`/`edge_page`，一次性序列化成 `@gos.vk` display list。`route_signal` 释放锁后才调 executor，所以这个 bulk read 不会死锁——这是已经验证、已经在跑的并发设计。

**缺的是什么**：(1) 这个"绕过逐节点派发、改用分页快照"的权限今天是**隐式的**——任何 executor 理论上都能调用 `gos_runtime::node_page`/`edge_page`（它们是 crate 公开 API），没有声明式标记说"这个节点的 fast path 是被审查过的"；(2) 治理脚本无法区分"一个节点的 `on_event` 是纯逐边响应"还是"偶尔绕过去读全图快照"——后者的正确性依赖一个 V2 line 136/ADR-002 都没说清楚的不变式：**快照读到的状态必须是逐边路径会产生的状态的投影，不能是另一个事实来源**（"the table is the graph"，[ADR-006 §一](./ADR-006-capability-graph-migration.md) 同型表述）；(3) [ADR-014 §3.1](./ADR-014-process-as-subgraph-compat-strategy.md) 已经预订了第二个消费者——wasm 解释器的 `capability_check` 热循环——但没有本 ADR 就无法回答"解释器可不可以缓存 Grant 边视图,而不是每次 `fd_read` 都 BFS"。

## 二、选项

### 选项 A —— `RoutePolicy` 新增变体（edge-scoped）

`RoutePolicy`（`gos-protocol/src/lib.rs:759-764`：`Direct=0x00`/`Weighted=0x01`/`Broadcast=0x02`/`FailFast=0x03`）是 `EdgeSpec.route_policy`/`EdgeRecord.route_policy` 字段的类型，`route_edge`（`gos-runtime/src/lib.rs:1107`）已有 `let _ = edge.route_policy;` 的现成接线点（"reserved for future use"）。新增 `RoutePolicy::FastPath = 0x04`：标记在某条边上的边，其投递可以被一次 bulk 快照替代,而不必逐条 `post_signal`。

- **优点**：纯加法（新枚举变体，不改变现有判别值 0x00-0x03）；接线点已存在（`route_edge` 的 match 只需新增一支）。
- **代价**：语义是"边级"的，但 V2/ADR-002/V3 三处原文都说"**标签节点**"——一个节点有多条边时，"哪条边代表这个节点整体进入了 fast path"是个新的歧义；`render_live_graph` 的 bulk read 读的是*全图快照*，不是"沿某条特定边"，edge-scoped 的标记在语义上不太贴切。

### 选项 B —— `PermissionKind` 新增变体（node-scoped，我倾向的方向）

`PermissionKind`（`gos-protocol/src/lib.rs:770-784`：`PortIo`..`ScheduleHint`，`0x01`-`0x09`，`#[repr(u8)]`）是 `NodeSpec.permissions: &'static [PermissionSpec]` 数组的元素类型——这是节点*声明自己被允许做什么*的既有机制（`PortIo`/`IrqBind`/`PhysMap` = 硬件权限，`GraphRead`/`GraphWrite` = 图变更权限，`CapabilityExport`/`Consume` = 能力声明，`ScheduleHint` = 调度提示）。新增 `PermissionKind::FastPathSnapshot = 0x0A`：节点在 `PluginManifest.permissions` 里声明此项，即声明"本节点的 executor 被允许调用 `node_page`/`edge_page` 做全图/子图快照读，绕过逐边 `on_event` 投递"。

- **优点**：纯加法（`#[repr(u8)]` 新变体 `0x0A`，不改变 `0x01`-`0x09` 判别值，不改变任何 `#[repr(C)]` 结构体布局——`NodeSpec`/`PluginManifest` 的 ABI 形状不变，[ADR-015](../plan/V3_DEVELOPMENT_PLAN.md) 的版本政策不受影响）；语义贴合三处原文的"标签**节点**"；声明式、`PluginManifest.permissions` 本就是治理脚本已经在扫描的数组（与 `DEPENDS_ON`/`EXPORTS`/`IMPORTS` 同一审计面），零新增基建——治理脚本只需新增一条"声明了 `FastPathSnapshot` 的节点，必须有对应的等价性 harness"规则；`render_live_graph`（`k-vk-host`）补声明此 permission 即"完成 retrofit"，不改一行运行时代码。
- **代价**：仅仅是"声明"——`gos_runtime::node_page`/`edge_page` 本身今天不检查调用者是否声明了该 permission（这是"声明式标记"和"运行时强制"的差距，与 [ADR-006 选项 A](./ADR-006-capability-graph-migration.md)"不进入热路径、只做等价性证明"同型——本 ADR 的标记同样是**治理时**的声明,不是 trap 时的强制)。若未来需要运行时强制（例如 ring3 隔离下，外来 executor 不应被允许 bulk 读全图），那是 ADR-006 选项 B 同型的"提到热路径"问题,独立 ADR。

### 选项 C —— 不新增类型，仅文档记录

只在 `render_live_graph` 的文档注释和 V2/V3 计划里写明"这就是 fast-path 标签节点的参考实现",不新增 `RoutePolicy`/`PermissionKind` 变体，未来需要时再设计。

- **代价**：[ADR-014 §3.1](./ADR-014-process-as-subgraph-compat-strategy.md) 已经把"ADR-012 fast-path"列为 wasm 解释器性能问题的长期答案——选 C 等于让 ADR-014 的这条引用继续悬空，三份文档对"fast-path 标签节点"的承诺仍然零定义,问题只是从"V2.6 待办"变成"V3 待办"。

## 三、建议与门禁

倾向 **B**：`PermissionKind::FastPathSnapshot = 0x0A`（命名可在选向时调整），声明式、零 ABI 影响、语义对齐"标签节点"、复用 `PluginManifest.permissions` 既有治理审计面。`render_live_graph`（`k-vk-host`）是第一个该补声明的节点——补上后，"fast-path 标签节点"从三份文档里的占位符变成一个**有具体实例**的概念。

**等价性义务**（mirrors [ADR-006 选项 A](./ADR-006-capability-graph-migration.md)）：任何声明 `FastPathSnapshot` 的节点，必须有 host-harness 证明"`node_page`/`edge_page` 快照读到的结果"与"假设该节点改走逐边 `on_event`/`Subscribe` 投递会收到的结果"在给定 epoch 下等价——快照是逐边路径的**投影**，不是第二个事实来源。这条义务本身不要求快照路径有逐边等价实现（`render_live_graph` 没有,也不需要有）,只要求**可以构造一个 harness 断言两者在合成场景下一致**（mirrors V2.4c 的 `capability_specs.rs` 手法,任务 #32）。

**与 [ADR-014](./ADR-014-process-as-subgraph-compat-strategy.md) 的连接**：wasm 解释器（ADR-014 选项 A）的 `ExecutorId`（如 `native.wasm`）在其 `PluginManifest.permissions` 声明 `FastPathSnapshot`，即可在 `capability_check` 热循环中使用一份 `node_page`/`edge_page` 派生的本地 Grant 边缓存,而不必每次 `fd_read`/`fd_write` 都对活图做 `reachable_via_grant` BFS——等价性 harness 断言"缓存视图 == 缓存构建时刻活图的 `reachable_via_grant` 结果"。这把 ADR-014 §3.1 的"长期答案"从一句引用变成一个可实现的具体机制。

**门禁**：`PermissionKind::FastPathSnapshot` 新增变体本身是纯加法、低风险，可在选向后随时落地（建议与 `render_live_graph` 的 retrofit 一起做,作为一个 harness-provable 的小步骤,mirrors V2.5d/e 的形态）。门禁范围**不含**"运行时强制该 permission"（选项 B 代价段落已述,属 ADR-006-B 同型的独立问题）以及"wasm 解释器使用此缓存"（依赖 [ADR-014](./ADR-014-process-as-subgraph-compat-strategy.md) 自身的 A/B/C 选向,本 ADR 只确保该选项存在时有地基）。
