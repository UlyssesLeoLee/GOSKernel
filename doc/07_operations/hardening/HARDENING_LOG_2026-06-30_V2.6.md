# GOS 自动硬化日志 — 2026-06-30（第7次，V2.6 Metrics + Epoch-diff 渲染跳过）

> 类型：定期自动硬化（每2小时）  
> 目标：V2.6 可观测性 — `metrics` 命令 + Epoch-diff 空帧跳过 + V2.3 遥测行  
> 提交：`feat(v2.6): metrics shell command + epoch-diff idle skip + telemetry harness`

---

## 执行摘要

本次硬化围绕 **V2.3 可观测性基础设施**，在 Shell 层将已存在的遥测 API 暴露给操作员，并实现 V2.3 Demo #2 的直接前置工作：

1. **`metrics` Shell 命令** — 全屏 7 行 V2.3 遥测仪表板
2. **Epoch-diff 空帧跳过** — `draw_command_deck_panel` 仅在图拓扑实际变化时重绘
3. **`where` 命令 V2.3 遥测行** — 在 row +6（最后一行）显示 epoch/idle/depth/subs 摘要
4. **`gos-metrics-harness`** — 7 项测试验证遥测 API 一致性

全部测试绿灯：runtime 26 + supervisor 16 + rewrite 12 + integration 6 + subscribe 10 + **metrics 7** = **77 项**，workspace clippy 零警告。

---

## 变更详情

### 1. `crates/k-shell/src/lib.rs`（+101 行）

#### `GRAPH_CTX_METRICS = 4` — 新增 graph context 常量

```rust
const GRAPH_CTX_METRICS: u8 = 4;
```

用于区分 `metrics` 命令（vs `where` 命令）在 `GRAPH_MODE_INFO` 下的渲染路径。

#### `ShellState::last_rendered_epoch: u64` — 新增字段

```rust
/// graph_epoch at the last draw_command_deck_panel call; enables the
/// V2.3 epoch-diff idle skip (zero unnecessary panel repaints).
last_rendered_epoch: u64,
```

插入在 `clipboard_target: u64` 之后、`console_live: u8` 之前，维持 u64 字段的自然对齐顺序。初始化为 0（启动时 epoch 从 0 递增，第一次心跳必然触发重绘）。

#### `render_metrics()` — 新增全屏遥测仪表板

7 行命令区（`GRAPH_VIEW_FIRST_ITEM_ROW` + 0~6）显示：

| 行 | 内容 |
|---|---|
| +0 | `graph_epoch` / `render_epoch` / lag（若 lag > 0） |
| +1 | `idle_cycle_count` + 说明 |
| +2 | `causal_depth_max` + cap=2048 |
| +3 | `subscribe_pairs` / `MAX_SUBSCRIBE_PAIRS(64)` |
| +4 | tick / plugins / nodes / edges |
| +5 | `domain_switch_count` / `preempt_count` |
| +6 | `boot_fallback_allocs` |

#### `render_where()` — V2.3 遥测行（row +6）

在现有 where 输出后追加：

```
ep:<N>  idle:<N>  depth:<N>  subs:<N>
```

操作员无需切换到 metrics 界面即可感知系统静默状态。

#### 快捷面板 + 命令派发

- 快捷面板 row 7 新增 `metrics` 标记（col 46，承接现有 `where` 在 col 38）
- `graph_context_label(GRAPH_CTX_METRICS)` → `"metrics"`
- `restore_graph_nav_state` 在 `GRAPH_MODE_INFO` 下按 `graph_context` 分发到 `render_metrics` 或 `render_where`
- `dispatch_command` 新增 `cmd == "metrics"` 分支

### 2. `crates/k-shell/src/proc.rs`（+10 行）

#### Epoch-diff 空帧跳过（V2.3 Demo #2 直接前置）

```rust
if state.heartbeat_divider % 4 == 0 {
    let current_epoch = gos_runtime::graph_epoch();
    if current_epoch != state.last_rendered_epoch {
        state.last_rendered_epoch = current_epoch;
        super::draw_command_deck_panel(&sink, state, snapshot);
    }
    super::redraw_footer(&sink, state, false);
}
```

**作用**：图拓扑未变化时跳过 `draw_command_deck_panel`，仅更新动态元素（header、sigil、flux、AI 面板、operator band）。`idle_cycle_count` 上升时此优化生效。

**对比 V2.3 设计文档**：`LAST_RENDER_EPOCH` / `IDLE_CYCLE_COUNT` 是 Supervisor 层跟踪"服务循环空闲"的计数器；Shell 层的 `last_rendered_epoch` 是"面板重绘"的独立追踪，共同构成 "0 帧空闲渲染" 基础设施（Demo #2 前置）。

### 3. `host-tests/gos-metrics-harness/`（新建，7 项测试）

| 测试 | 验证点 |
|---|---|
| `graph_epoch_starts_at_zero_and_increments_on_register_node` | reset 后 epoch=0；register_node 递增 |
| `graph_epoch_stable_after_read_only_operations` | 纯读操作不改变 epoch |
| `idle_cycle_count_increments_when_graph_stable_across_service_cycles` | 2次稳定服务循环 → idle_count += ≥2 |
| `causal_depth_max_is_zero_for_empty_service_cycles` | depth < 2048 上限（不触发 CausalOverflow） |
| `render_epoch_matches_graph_epoch_after_service_cycle` | service_system_cycle 后 render_epoch == graph_epoch |
| `subscribe_pair_count_tracks_register_idempotent_and_unregister` | 注册 +1；幂等重注册不变；注销 -1 |
| `causal_depth_max_is_cumulative_peak` | causal_depth_max 单调不减 |

---

## V2.3 退出判据核查（更新）

| 判据 | 状态 |
|---|---|
| `SubscribeTriggered` 协议事件（`0x0C`） | ✅ V2.3 core |
| Subscribe 表注册（幂等，MAX 64 对） | ✅ V2.3 core |
| `fire_subscribers` 三条变更路径覆盖 | ✅ V2.3 core |
| Epoch-diff 空帧计数基础设施 | ✅ V2.3 core |
| Subscribe 测试套件（10 项） | ✅ V2.3 + V2.5 |
| `causal_depth_max` 峰值计量 | ✅ V2.5 |
| `unregister_subscribe` 资源回收 | ✅ V2.5 |
| `subscribe_pair_count` 配额观测 | ✅ V2.5 |
| **Shell `metrics` 命令（遥测仪表板）** | ✅ **V2.6 本次** |
| **Epoch-diff Shell 面板跳过** | ✅ **V2.6 本次** |
| **V2.3 telemetry row in `where`** | ✅ **V2.6 本次** |
| Boot manifest 静态图 | 🔲 → V2.6 后续 |
| Demo A: theme 0-line 传播 | 🔲 → V2.3 后续（需 V2.4 MutationDispatcher） |

---

## V2.x 路线图（更新）

| 阶段 | 状态 |
|---|---|
| V2.0 边代数地基 | ✅ |
| V2.1 Cypher = ISA + MutationAudit | ✅ |
| V2.2 Rewrite Engine + Supervisor 接入 | ✅ |
| V2.3 Subscribe 核心原语 | ✅ |
| V2.4 能力即可达性 | 🔲 |
| **V2.5 causal_depth_max / unregister / pair_count** | ✅ |
| **V2.6 metrics 命令 + epoch-diff 跳过** | 🔶 **本次** |

### V2.6 下一步（可立即启动）

1. **Boot manifest 静态图** — 从 `hypervisor/src/builtin_bundle.rs` 的启动序列提取声明式节点/边期望，用 `EdgeAbsent` 规则在启动帧自动补全缺失边
2. **Epoch-diff 渲染跳过 for VGA layer** — 在 `k-vga` 的 `update_framebuffer` 中比对 epoch 实现真正的硬件级零空帧渲染
3. **Demo A 前置重构** — 将 `PAL_U32` 常量提升为图中的属性节点（V2.1 MutationDispatcher 实现后）

---

## 测试结果

```
host-tests/gos-runtime-harness:              26 passed, 0 failed
host-tests/gos-supervisor-harness:          16 passed, 0 failed
host-tests/gos-rewrite-harness:             12 passed, 0 failed
host-tests/gos-rewrite-integration-harness:  6 passed, 0 failed
host-tests/gos-subscribe-harness:           10 passed, 0 failed
host-tests/gos-metrics-harness:              7 passed, 0 failed  (新增)

cargo clippy --workspace:  0 warnings
cargo check --workspace:   Finished (0 errors)

总计：77 项测试全绿
```

---

*自动生成于 2026-06-30 定期硬化任务（第7次）*
