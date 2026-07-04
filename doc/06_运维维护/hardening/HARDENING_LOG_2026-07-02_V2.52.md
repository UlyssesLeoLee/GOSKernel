# GOS 硬化日志 — V2.52（2026-07-02）

## 版本号: V2.52
## 功能: `graph sim <N>` — 随机游走信号流量模拟

---

## 变更摘要

在活跃内核图上实现有向随机游走模拟器，按降序报告每节点访问计数。用于识别在模拟随机信号负载下哪些图节点吸引了最多流量——是内核原生等价于 `strace -e trace=signal` 的观测工具。

---

## 变更内容

### `crates/gos-runtime/src/lib.rs`

**新内部方法：`GraphRuntime::graph_sim_inner<N>`**（位于 `impl GraphRuntime` 内）

算法：
1. 用 `seed` 播种确定性 xorshift32 PRNG（0 会被映射为 `0xDEAD_BEEF`）
2. 从随机活跃节点出发：`node_slots[xorshift32() % n]`
3. 记录初始访问（`raw_visits[cur_slot] += 1`）
4. 对每个 `steps` 迭代：
   - 收集 `cur_slot` 的所有活跃出边（匹配 `edge_from == slot_id[cur_slot]`）
   - 将边权重求和（× 1000 为 u32；权重为0的边计为1）
   - 若无出边：**传送**——随机选取一个活跃节点，增加其计数，`stuck_steps++`
   - 否则：使用 `xorshift32() % total_w` 按权重比例采样边，遍历、增加目标计数、`actual_steps++`
5. 使用插入排序将每 slot 的访问计数按降序打包进输出数组

**新公开 API：`pub fn graph_sim<const N: usize>(steps: u32, seed: u32) -> (...)`**

- `steps` 在调用 `graph_sim_inner` 前被截断至256
- 返回 `(vecs, visits, node_count, actual_steps, stuck_steps)`

**关键不变量（已证明成立）：**
```
sum(visits[0..n]) == 1 + actual_steps + stuck_steps == 1 + min(steps, 256)
```
N 步中每一步都恰好增加一个访问计数器（传送目的地或遍历目的地），加上初始起始位置。

### `crates/k-shell/src/lib.rs`

**新增 `pub fn dispatch_graph_sim(sink: &ConsoleSink, steps: u32)`**

Shell 输出格式：
```
 graph sim  steps=32  seed=3735928559
 ───────────────────────────────────────────────────────────
  rank  visits  vector
     1      14  1.0.0.1        ← 洋红（排名1）
     2       9  6.1.0.0        ← 青色（排名2-3）
     3       5  2.0.0.1
     4       4  3.0.0.1        ← 白色（排名4+）
 ───────────────────────────────────────────────────────────
 4 node(s)  31 walk steps  1 teleport(s)
```
页脚显示 `N teleport(s)`（黄色）或 `no dead ends`（绿色）。

### `crates/k-shell/src/proc.rs`

新路由：
```
graph sim           → dispatch_graph_sim(sink, 16)   [默认16步]
sim                 → 同上
gsim                → 同上
graph walk          → 同上
walk                → 同上
graph sim <N>       → dispatch_graph_sim(sink, N.min(256).max(1))
sim <N>             → 同上
gsim <N>            → 同上
graph walk <N>      → 同上
walk <N>            → 同上
```
非数字 N 以红色打印错误。

---

## 测试用例（10/10 全绿）：`host-tests/gos-graph-sim-harness`

| 编号 | 测试名 | 验证点 |
|------|--------|--------|
| 1 | `empty_graph_returns_all_zeros` | 空图 → node_count=0, actual=0, stuck=0 |
| 2 | `zero_steps_returns_all_zeros` | steps=0 → 全零（提前返回路径） |
| 3 | `single_node_no_edges_all_stuck` | 死胡同节点：stuck=8, actual=0, visits[0]=9 |
| 4 | `single_node_self_loop_no_stuck` | 自环：stuck=0, actual=8, visits[0]=9 |
| 5 | `steps_clamped_to_256` | steps=999 → actual+stuck ≤ 256 |
| 6 | `visit_sum_invariant_linear_dag` | 3节点 DAG：sum(visits) == 1+steps |
| 7 | `actual_plus_stuck_equals_steps` | 步数核算：actual+stuck == min(steps,256) |
| 8 | `node_count_matches_registered` | 注册4节点 → node_count=4 |
| 9 | `output_sorted_descending` | 对所有 i：visits[i] ≥ visits[i+1] |
| 10 | `two_cycle_sum_invariant_and_sorted` | 2-环：无 stuck，总数不变量，已排序 |

L4 命名空间：**29**（保留给 graph-sim harness 测试节点）

---

## 宿主测试套件

| V2.52 之前 | V2.52 之后 |
|------------|------------|
| 483 个测试 | **493 个测试**（483 + 10 新增） |

---

## 设计说明

### PRNG 选择：xorshift32

- `no_std` 安全——无堆分配，无 OS 熵源
- 周期 2³²−1——足够支持256步
- 给定 `seed` 时确定性——便于测试
- 公开 API 使用 `graph_epoch ^ steps ^ 0xDEAD_BEEF` 混合作为种子，使重复的 shell 调用无需硬件时钟即可产生变化

### 传送语义

死胡同节点触发均匀随机传送（类比 PageRank 的阻尼系数 `d`）。确保游走者不会永久卡在孤立子图中，并保持不变量 `actual + stuck == steps` 简洁。

### 按权重比例采样

`EdgeSpec.weight`（f32）的边权重被缩放 × 1000 转为 u32 后再采样。零权重边视为权重=1，避免排除以默认权重注册的图结构。

### 不推进 epoch

`graph_sim` 是纯读操作——不触及 `graph_epoch`、diff ring 或任何可变状态。可安全地以任意频率调用。

### OS 类比

`graph sim` 是内核拓扑层面等价于：
- `strace -e trace=signal` —— 哪些子系统处于热信号路径？
- `perf record -g` → `perf report` —— 谁主导调用图？
- `netstat -s | grep segments` —— 哪些网络节点处理最多流量？

---

## 后续建议

- V2.53：`graph between` —— 全对 Dijkstra 介数中心性（有向、带权）
- PAL_U32 → attribute node 重构（Demo A 前置条件）

---

*由自动强化任务生成 · 2026-07-02*
