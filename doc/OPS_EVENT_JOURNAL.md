# 控制面事件日志 (`journal` / `events`) — Operator Guide

> 状态：已落地
> 覆盖：`gos-runtime` 控制面持久化环形缓冲 + `k-shell` `journal`/`events` 命令
> 关联：`perf+feat: pump() lock merge, module health visibility, ai event counters`
>       （`c10f877`，本次补上其遗留的"事件历史不可追溯"缺口）

## 一、问题：状态快照能看，历史看不到

`modules` 命令（`gos-supervisor::module_status_summaries`）能看到模块*当前*
是什么状态、重启过几次，但看不到"刚才发生了什么"——比如某个模块在过去
几秒内连续 fault 了三次，还是仅仅一次，`modules` 只剩最后一帧。

控制面本身确实有事件：`gos_runtime::GraphRuntime::emit_control_plane` 在
`PluginDiscovered` / `NodeUpsert` / `EdgeUpsert` / `StateDelta` / `Fault` /
`Metric` 等时机把 `ControlPlaneEnvelope` 推进一个 256 容量的队列
（`MAX_CONTROL_PLANE_MESSAGES`），但这个队列只有一个消费者
（`k-ai::drain_control_plane_into`，每帧把消息累加进计数器后就丢弃），
一旦被消费，原始事件就不存在了——只剩聚合计数（`plugin_events`/
`fault_events`/...），定位不到具体哪个模块、哪个 tick 发生了什么。

`gos-journal` crate（Phase F.4）早就定义了 `JournalRing<N>` —— 一个
`no_std`/无堆分配的有界环形缓冲，专门设计来缓存 `ControlPlaneEnvelope`——
但在这次改动之前，全仓库零生产调用点（只在自己的单元测试里被构造过），
是一个"脚手架搭好了但没接线"的典型缺口。

## 二、本次改动：把 `JournalRing` 接到 `emit_control_plane`

`crates/gos-runtime/src/lib.rs`：

- `GraphRuntime` 新增字段 `journal: gos_journal::JournalRing<JOURNAL_RING_CAPACITY>`
  （`JOURNAL_RING_CAPACITY = 64`），与现有 `control_plane`
  （消费即丢弃的 dispatch 队列）并存、互不影响。
- `emit_control_plane` 现在在推进 dispatch 队列的同一时刻，把同一个
  envelope 也 `append` 进 `journal`；满了就 `reset()` 后重新开始追加
  （"清零重灌"式 wrap，不是真正的环形覆盖最旧一条——这点在代码注释和下面
  的"已知限制"里都写明了）。
- 新增只读读取入口（不消费、可重复调用）：
  - `GraphRuntime::journal_recent(&self, out: &mut [ControlPlaneEnvelope]) -> usize`
  - 模块级包装 `gos_runtime::journal_recent(out)`
  - 配套新增 `gos_runtime::emit_control_plane(...)` 模块级包装，
    与既有的 `drain_control_plane()` 对称。

`crates/gos-journal/src/lib.rs`：

- `JournalRing::get(&self, index) -> Option<ControlPlaneEnvelope>` ——
  按下标解码单条已缓冲的 envelope，免得纯内存读取场景也要走
  `flush_into` + `replay` 这套为"落盘"设计的往返序列化。

`crates/k-shell`：

- 新增 `journal` / `events` 命令（`proc.rs`），打印最近缓冲的事件，
  每条显示 kind（`fault`/`node-upsert`/...）、`subject`（按现有 ascii-tag
  惯例从 `[u8;16]` 裁到第一个 `\0`）、`arg0`/`arg1`。
- `help` 文本登记新命令。

```
gos> journal
 control-plane journal (oldest buffered first)
  fault  subject: K_AI  arg0: 1  arg1: 2
  metric  subject:   arg0: 7  arg1: 0
```

## 三、设计取舍

- **不落盘**：`gos-vfs::FileSystem` 还没有写路径（F.5 未落地，
  `gos-journal` 模块注释里写得很清楚——"ready to hand to a
  `gos_vfs::FileSystem` write path once F.5 lands"），所以这次只做纯内存、
  跨 fault/重启会丢的"最近事件"视图，类似 `dmesg`（重启清空）而不是持久
  系统日志。落盘是独立的、量级更大的 F.5 集成任务，不在这次范围内。
- **双写而非改造单一队列**：没有改 `control_plane` 队列本身的消费语义
  （仍然是 `k-ai` 单消费者、pop 即丢），而是在 `emit_control_plane` 这个
  唯一的 producer 入口上加一份只读副本。这样不影响 `k-ai` 现有的计数器
  逻辑，也不需要引入第二个队列消费者去抢同一份数据。
- **满了清零，不是真环形**：`JournalRing::append` 返回 `Err` 表示满，
  没有"覆盖最旧一条"的 API。本次选择满了就 `reset()` 再追加，行为简单、
  容易验证，代价是 wrap 瞬间会丢掉一整批历史而不是逐条淘汰最旧的。

## 四、已知限制 / 后续可做

- wrap 策略是"清零重灌"不是真正的 ring-buffer 覆盖，高频事件场景下可能
  在 wrap 瞬间丢失一批刚发生的历史。如果需要真正的"最近 N 条，逐条淘汰"，
  需要在 `gos-journal` 里加一个支持覆盖写的变体，而不是复用现在的
  `JournalRing::append`/`is_full`/`reset` 三件套。
- 仍然是纯内存、重启丢失。落盘持久化（`doc/OPS_MODULE_HEALTH.md` 已经
  提到这个待办）需要先有 `gos-vfs` 的写路径（F.5），属于单独的大任务。
- `subject` 目前按 ascii-tag 惯例裁剪显示；不是所有 `ControlPlaneMessageKind`
  的 `subject` 都保证是可打印 ascii（取决于调用方传的是 `ModuleId`/
  `PluginId` 还是别的 16 字节负载），非 ascii 场景下会显示为空或乱码，
  没有 hex fallback。

## 五、覆盖的测试

`host-tests/gos-runtime-harness/tests/runtime.rs`：

- `journal_recent_reads_back_emitted_envelopes_without_draining` ——
  验证 `emit_control_plane` 同时写两份、`journal_recent` 读到的内容和
  顺序正确，并且读 journal 不影响 `drain_control_plane` 仍能拿到全部
  原始消息。
- `journal_wraps_by_resetting_once_full` —— 验证填满 `JOURNAL_RING_CAPACITY`
  条后再 `emit` 一条，旧的整批历史按设计被清空，只剩这一条新的。

`cargo check -p gos-runtime -p gos-supervisor -p k-shell -p gos-kernel`
全部通过；`gos-runtime-harness`（26/26）与 `gos-supervisor-harness`
（16/16）host-test 套件全部通过，无回归。
