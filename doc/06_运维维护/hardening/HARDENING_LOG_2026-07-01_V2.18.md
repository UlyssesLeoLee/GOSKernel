# GOS 硬化日志 — V2.18 — 2026-07-01

## 概述

V2.18 新增了一个整体系统健康检查命令（`graph health` / `health`），以及
两个新的 gos-runtime API（`faulted_node_count`、`diff_ring_fill`），将
操作者可观测性提升到了 Linux 的 `systemctl status` + `dmesg --level=err,warn`
的水平。

---

## 修改清单

### 1. 两个新 API —— gos-runtime（`crates/gos-runtime/src/lib.rs`）

#### `GraphRuntime::faulted_node_count() -> usize`
私有 impl 方法 —— 单遍扫描统计 `lifecycle == NodeLifecycle::Faulted` 的节点数量。
直接遍历 `nodes` 数组（无需刷新顺序；故障计数与顺序无关）。

#### `GraphRuntime::diff_ring_fill() -> usize`
私有 impl 方法 —— 返回 `self.diff_total.min(MAX_DIFF_RING as u64) as usize`。
在无需遍历整个环形缓冲区的情况下，给出 128 槽结构性 diff 环形缓冲区的
当前占用量。

#### 公开的模块级导出（位于 `node_page_l4` 之后）
```rust
pub fn faulted_node_count() -> usize { RUNTIME.lock().faulted_node_count() }
pub fn diff_ring_fill() -> usize { RUNTIME.lock().diff_ring_fill() }
```

### 2. `graph health` shell 命令 —— k-shell（`crates/k-shell/src/lib.rs`、`crates/k-shell/src/proc.rs`）

#### `dispatch_graph_health(sink)`

收集十项运行时指标，渲染出带颜色编码的健康横幅以及详情表：

| 分区 | 指标 |
|---------|-------|
| **nodes（节点）** | 总数、faulted（若 > 0 则高亮红色）、边数、订阅对数 |
| **mutations（变更）** | graph epoch、累计推送的结构性 diff 总数、diff ring fill（N/128） |
| **runtime（运行时）** | 调度器抢占次数、l4 域切换次数 |
| **boot（启动）** | 已检查的 manifest 规则数、已修复的边数（若 > 0 则高亮黄色） |

**健康分级：**
- `DEGRADED`（白字红底）：faulted 节点超过总数的 25%（或当 total < 4 时
  出现任何故障）
- `WARNING`（黑字黄底）：存在任意 faulted 节点，或 diff ring ≥ 120/128
  （接近满）
- `OK`（黑字绿底）：无故障，ring 压力正常

非 OK 状态下，表格下方打印的提示行：
- DEGRADED：`run 'nodes faulted' to inspect faulted nodes`
- WARNING：`run 'nodes faulted' for fault details`

`proc.rs` 中新增的 shell 分发逻辑：
- 精确匹配 `"graph health"` 和 `"health"` → `dispatch_graph_health(sink)`。
- 帮助文本新增一条条目。

### 3. 测试套件 —— `host-tests/gos-graph-health-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `empty_faulted_node_count_is_zero` | 空 runtime → faulted_count == 0 |
| 2 | `registered_node_is_not_faulted` | 新注册节点 → 不处于 Faulted 状态 |
| 3 | `faulted_count_does_not_exceed_total` | faulted ≤ proc_count 结构性不变式 |
| 4 | `empty_diff_ring_fill_is_zero` | 空 runtime → diff_ring_fill == 0 |
| 5 | `register_node_increases_diff_ring_fill` | register_node 会推送 diff 记录 |
| 6 | `diff_ring_fill_equals_min_total_cap` | fill == min(diff_total, 128) |
| 7 | `diff_ring_fill_never_exceeds_max` | fill 始终 ≤ MAX_DIFF_RING（128） |
| 8 | `multiple_registrations_increase_diff_fill` | 每次注册都使 fill 增加 |
| 9 | `health_node_counts_consistent` | healthy + faulted == total |
|10 | `diff_ring_fill_monotonic_with_mutations` | fill 单调不减 |

---

## 验证

```
cd host-tests/gos-graph-health-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

内核编译：
```
cargo build --release
# Finished `release` profile [optimized]
```

host-test 套件总计：**183 个测试**（173 个 V2.17 + 10 个新增）

---

## 生产质量说明

| 能力 | Linux/macOS 对应物 | GOS V2.18 |
|---|---|---|
| 系统健康总览 | `systemctl status` | `graph health` 横幅（OK/WARNING/DEGRADED） |
| 故障分诊 | `systemctl --failed` | faulted 计数 + `nodes faulted` 提示 |
| 环形缓冲区压力 | 内核环形缓冲区已满告警 | diff ring fill N/128 |
| 启动完整性 | `systemd-analyze verify` | manifest 规则数 + 已修复边数 |
| 运行时吞吐量 | `vmstat` 的抢占/上下文切换列 | preempt_count + domain_switch_count |

`graph health` 命令是 GOS 中第一个将多条遥测轴综合为单一可操作健康裁决的
命令 —— 操作者无需分别检视 `nodes`、`metrics export` 和 `boot verify`；
`graph health` 一次调用即可完成这三者。

---

## Graph-OS 特性的保留

该健康模型是 graph 原生的：故障检测由生命周期状态驱动（而非基于信号），
diff ring 压力反映的是 graph 拓扑变更速率（而非内存压力），域切换次数是
一个 graph 分区指标，没有 POSIX 对应物。`diff ring fill` 指标是 GOS 独有的——
它追踪的是 graph 自身的结构性变更速度，而不是任何特定节点的活动情况。

---

*自动化硬化流程 — GOS V2.18 — 2026-07-01*
