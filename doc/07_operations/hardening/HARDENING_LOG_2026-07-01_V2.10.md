# GOS 自动硬化日志 — 2026-07-01（第11次，V2.10 metrics export 命令）

> 类型：定期自动硬化（每2小时）  
> 目标：V2.10 可观测性 — `metrics export` Shell 命令（机器可读遥测导出）  
> 提交：`feat(v2.10): metrics export shell command + 3 telemetry API harness tests`

---

## 执行摘要

本次硬化围绕 **机器可读遥测导出**，为 Shell 层新增 `metrics export` 命令。已有的 `metrics` 命令以 TUI 图形面板形式展示运行时指标；`metrics export` 是其串口友好的对等命令，输出 `key=value\n` 格式的平铺文本，可被主机侧脚本直接解析而无需处理 ANSI 转义序列。

1. **`metrics export` / `metrics dump`** — 输出14个遥测指标，`key=value` 格式
2. **`pub fn dispatch_metrics_export(sink: &ConsoleSink)`** — 新 k-shell 导出函数
3. **help 文本同步** — 在 `boot verify` 条目之后新增说明行
4. **gos-metrics-harness +3 测试**（tests 8-10）— 覆盖三个此前未测试的遥测 API

全部测试绿灯：runtime 26 + supervisor 16 + rewrite 12 + integration 6 + subscribe 10 + metrics **10** + boot 11 + node-inspect 8 = **99 项**。

---

## 架构动机

V2.6 的 `metrics` 命令将遥测数据渲染为 TUI 图形面板（`render_metrics`），需要占据整个命令区域。在以下场景中，这种展示方式不足：

- **主机侧自动化采集**：通过串口捕获输出时，TUI 转义码会干扰解析
- **串口日志记录**：纯文本格式可直接 `grep` 和 `awk`
- **CI/哨兵脚本**：与 boot 报告、节点状态等同风格的 `key=value` 输出更易于集成

**方案**：完全复用已有 API（`gos_runtime::*` + `gos_supervisor::*`），仅以 `print_str` + `print_num_inline` 输出，不引入任何新的数据层或状态。此模式与 `dispatch_boot_verify`、`dispatch_lifecycle_summary` 完全一致。

---

## 变更详情

### 1. `crates/k-shell/src/lib.rs`（+39 行）

在 `dispatch_boot_verify` 之后、`module_lifecycle_label` 之前插入：

```rust
pub fn dispatch_metrics_export(sink: &ConsoleSink) {
    let g_epoch = gos_runtime::graph_epoch();
    let r_epoch = gos_supervisor::render_epoch();
    let snap    = gos_runtime::snapshot();
    // ...14 kv! lines...
}
```

导出的14个键值：

| 键 | 来源 API |
|----|---------|
| `graph_epoch` | `gos_runtime::graph_epoch()` |
| `render_epoch` | `gos_supervisor::render_epoch()` |
| `idle_cycles` | `gos_supervisor::idle_cycle_count()` |
| `causal_depth_max` | `gos_supervisor::causal_depth_max()` |
| `subscribe_pairs` | `gos_runtime::subscribe_pair_count()` |
| `tick` | `snapshot.tick` |
| `plugins` | `snapshot.plugin_count` |
| `nodes` | `snapshot.node_count` |
| `edges` | `snapshot.edge_count` |
| `domain_switches` | `gos_runtime::domain_switch_count()` |
| `preemptions` | `gos_runtime::preempt_count()` |
| `boot_fallback_allocs` | `gos_runtime::boot_fallback_alloc_count()` |
| `boot_rules_checked` | `gos_runtime::boot_manifest_rules_checked()` |
| `boot_edges_healed` | `gos_runtime::boot_manifest_edges_healed()` |

**示例输出**（运行时稳定态）：

```
 telemetry export
  graph_epoch=42
  render_epoch=42
  idle_cycles=1337
  causal_depth_max=3
  subscribe_pairs=4
  tick=200
  plugins=12
  nodes=47
  edges=95
  domain_switches=0
  preemptions=0
  boot_fallback_allocs=0
  boot_rules_checked=27
  boot_edges_healed=0
```

---

### 2. `crates/k-shell/src/proc.rs`（+4 行）

在 `boot` 分支之后新增：

```rust
} else if cmd == "metrics export" || cmd == "metrics dump" {
    super::dispatch_metrics_export(sink);
```

help 文本新增：

```
  metrics export     machine-parseable key=value telemetry dump
```

---

### 3. `host-tests/gos-metrics-harness/tests/metrics.rs`（+36 行，3 项新测试）

| # | 测试名 | 验证点 |
|---|--------|--------|
| 8 | `domain_switch_count_starts_at_zero_after_reset` | reset 后 `domain_switch_count()` == 0 |
| 9 | `preempt_count_starts_at_zero_after_reset` | reset 后 `preempt_count()` == 0 |
| 10 | `boot_fallback_alloc_count_starts_at_zero_after_reset` | reset 后 `boot_fallback_alloc_count()` == 0 |

这三个 API 在 `dispatch_metrics_export` 中被导出，但此前在 metrics harness 中没有覆盖。

---

## 质量指标

| 指标 | 本次 | 前次（V2.9） |
|------|------|--------------|
| 测试总数 | **99** | 96 |
| Clippy 警告（新增） | **0** | 0 |
| 新增测试 | **+3**（metrics harness 8-10） | +3 |
| 新增 Shell 命令 | **+1**（`metrics export`） | +1 |
| 受影响 crate | 2（k-shell、metrics harness） | 3 |

> 注：`cargo check` 显示4个关于 `pub fn dispatch_*` 与 `pub(crate) ConsoleSink` 的可见性警告——这些是**预存在警告**，对应 `dispatch_nodes_list`、`dispatch_lifecycle_summary`、`dispatch_boot_verify` 和新增的 `dispatch_metrics_export`，本次未引入新警告。

---

## 图论 OS 特性维护

- **无新数据层**：`dispatch_metrics_export` 是纯读取路径——对运行时图无任何写操作，不触发 epoch bump，符合 ADR-001 的 "read must be pure" 约束
- **统一遥测门**：所有14个指标均通过 `gos_runtime` / `gos_supervisor` 的公共 API 访问，与 TUI `metrics` 面板和 `boot verify` 共用同一数据源，三者保持一致
- **可观测性链路扩展**：serial log → runtime atomics → TUI panel（`metrics`）→ **text export（`metrics export`）**，四层冗余覆盖

---

## 下一步（V2.10 后续）

- [ ] VGA 层脏行追踪（epoch-diff hardware-level skip）
- [ ] PAL_U32 → 属性节点重构（Demo A 前置）
- [ ] `journal` 命令（查询 gos-journal 控制平面事件队列）
- [ ] V3 生态层规划（见 `plan/V3_DEVELOPMENT_PLAN.md`）

---

## 测试结果

```
host-tests/gos-runtime-harness:              26 passed, 0 failed
host-tests/gos-supervisor-harness:          16 passed, 0 failed
host-tests/gos-rewrite-harness:             12 passed, 0 failed
host-tests/gos-rewrite-integration-harness:  6 passed, 0 failed
host-tests/gos-subscribe-harness:           10 passed, 0 failed
host-tests/gos-metrics-harness:            10 passed, 0 failed  (+3 新增)
host-tests/gos-boot-harness:               11 passed, 0 failed
host-tests/gos-node-inspect-harness:         8 passed, 0 failed

总计：99 项测试全绿
```

---

*自动生成于 2026-07-01 定期硬化任务（第11次）*
