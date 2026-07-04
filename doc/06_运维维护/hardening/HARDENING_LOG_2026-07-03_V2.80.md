# HARDENING LOG — V2.80 | 2026-07-03

## 版本 / Version
**V2.80** — Power-Law Exponent MLE: Clauset–Newman–Shalizi 最大似然估计幂律指数

## 变更摘要 / Change Summary

### 新增功能 / New Feature

**图论操作系统新增：幂律指数最大似然估计（Power-Law Exponent MLE）**

本次迭代实现 Clauset–Newman–Shalizi (2009) 幂律指数最大似然估计器，
从无向度序列中定量估算幂律衰减指数 γ̂，为无标度网络检测（V2.78 κ 指标）
提供精确的数值量化补充。

**算法公式：**
```
γ̂ = 1 + n_fit × [Σ_{k_i ≥ 1} ln(k_i)]^{-1}

其中：
  n_fit    = 度数 k ≥ 1 的节点数（孤立节点 k=0 排除在外）
  k_min    = 1（固定；考虑所有有边连接的节点）
  sum_ln   = Σ LN_TABLE[k_i]  （整数 LN_TABLE，ln(k) × 10^6）
  gamma_ppm = 1_000_000 + n_fit × 10^12 / sum_ln_ppm
```

**Shell 输出示例（双向 K3 三角形）：**
```
 graph power-law exponent (MLE)
 ───────────────────────────────────────────────────────────
  γ̂         = 2.442695
  n_fit      = 3  (nodes with k≥1, k_min=1)
 ───────────────────────────────────────────────────────────
  γ∈[1,3]: compatible with power-law / scale-free
  nodes=3  n_fit=3
```

**分类区间：**

| γ 范围 | 含义 | 颜色 |
|--------|------|------|
| γ = 0 | MLE 未定义（所有非孤立节点 k=1） | 灰色 |
| γ ∈ [1, 3] | 与幂律/无标度分布兼容 | 绿色 |
| γ ∈ (3, 4] | 急剧衰减尾部；弱异质性 | 黄色 |
| γ > 4 | 不符合幂律度分布特征 | 白色 |

### 实现详情 / Implementation Details

**crates/gos-runtime/src/lib.rs**
- 新增 `graph_power_law_inner()` 方法（在 `graph_scale_free_inner` 之后）
  - 与 V2.78 共享度序列计算逻辑（相同的无向度去重方法）
  - 内嵌独立 `LN_TABLE[0..129]`（与 V2.77 small-world 中相同的常量，no_std 安全）
  - 整数算术：sum_ln_ppm（u64）+ n × 10^12 / sum_ppm（u64，不溢出）
  - 越界保护：k > 128 时用 k.min(128)（MAX_NODES=128，实际不会超出）
- 新增公开 API `graph_power_law() -> (gamma_ppm: u32, n_fit: usize, node_count: usize)`

**crates/k-shell/src/lib.rs**
- 新增 `dispatch_graph_power_law(sink)` — 位于 `dispatch_graph_scale_free` 之后
  - `print_ppm6` 辅助函数（与 scale-free dispatch 相同格式，6 位小数）
  - sum_ln=0（全 k=1）时显示 "undefined (all non-isolated k=1; MLE degenerate)"
  - 三级分类 banner（[1,3] / (3,4] / >4）

**crates/k-shell/src/proc.rs**
- 新增命令路由（位于 scale-free 路由之后）：
  - `"graph power law"` / `"graph power-law"` / `"gpowerlaw"` / `"power law"` / `"gpl"`

**host-tests/gos-graph-power-law-harness/**
- `Cargo.toml` — 标准独立 workspace
- `.cargo/config.toml` — x86_64-pc-windows-msvc + build-std
- `tests/graph_power_law.rs` — 10 个测试用例
  - L4=56 命名空间（与已有 harness 隔离）

### 测试用例设计 / Test Case Design

| # | 图结构 | 关键验证 |
|---|--------|---------|
| 1 | 空图 | (0, 0, 0) |
| 2 | 单孤立节点 | n_fit=0, gamma=0 |
| 3 | 两节点一条边 (k={1,1}) | sum_ln=0 → gamma=0 (MLE 未定义) |
| 4 | 双向三角形 K3 (k={2,2,2}) | gamma ≈ 2.442 (幂律兼容范围) |
| 5 | 完全图 K4 双向 (k={3,3,3,3}) | gamma ≈ 1.910 |
| 6 | 有向链 A→B→C→D (k={1,2,2,1}) | gamma ≈ 3.885 (超出常规幂律) |
| 7 | 星形 hub→3辐 (k={3,1,1,1}) | gamma ≈ 4.641 (不符合幂律) |
| 8 | 星形 hub→6辐 (k={6,1,1,1,1,1,1}) | gamma ≈ 4.907 |
| 9 | K4 + 孤立节点 E | n_fit=4≠5，gamma 与 K4 相同 |
| 10 | 混合度图 (k={4,2,2,1,1,0}) | gamma ≈ 2.803 ∈ [2,3] 幂律范围 |

### 整数算术推导 / Integer Arithmetic Derivation

LN_TABLE[k] = floor(ln(k) × 1_000_000)：
- LN[1] = 0
- LN[2] = 693_147
- LN[3] = 1_098_612
- LN[4] = 1_386_294
- LN[6] = 1_791_759

关键计算验证：
```
K3 三角形: sum_ln = 3 × 693_147 = 2_079_441
gamma = 1_000_000 + 3_000_000_000_000 / 2_079_441 = 1_000_000 + 1_442_695 = 2_442_695

K4 完全图: sum_ln = 4 × 1_098_612 = 4_394_448
gamma = 1_000_000 + 4_000_000_000_000 / 4_394_448 = 1_000_000 + 910_467 = 1_910_467

混合图 (k=4,2,2,1,1): sum_ln = 1_386_294 + 693_147 + 693_147 + 0 + 0 = 2_772_588
gamma = 1_000_000 + 5_000_000_000_000 / 2_772_588 = 1_000_000 + 1_803_369 = 2_803_369
```

溢出分析（安全）：
- `sum_ln_ppm` 最大值：128 × LN[128] = 128 × 4_852_030 ≈ 6.21 × 10^8 < u64::MAX
- `n_fit × 10^12` 最大值：128 × 10^12 = 1.28 × 10^14 < u64::MAX (≈1.84 × 10^19)

## 测试结果 / Test Results

```
running 10 tests
test bidirected_triangle_gamma_approx_2_44 ... ok
test complete_k4_gamma_approx_1_91 ... ok
test directed_chain_abcd_gamma_approx_3_89 ... ok
test empty_graph_returns_zero ... ok
test k4_plus_isolated_n_fit_excludes_isolated ... ok
test mixed_degree_gamma_in_powerlaw_range ... ok
test single_isolated_node_undefined ... ok
test star_3spokes_gamma_approx_4_64 ... ok
test star_6spokes_gamma_approx_4_91 ... ok
test two_nodes_k_one_undefined ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

## VectorAddress 命名空间 / VectorAddress Namespace

**L4=56** — gos-graph-power-law-harness

## 关键不变量 / Key Invariants

- **纯读取，不修改 epoch**：`graph_power_law` 是纯读取操作
- **孤立节点排除**：k=0 的节点不计入 n_fit 和 sum_ln（k_min=1 硬编码）
- **全 k=1 时 gamma=0**：sum_ln=0 时 MLE 退化，返回 0 表示"未定义"（而非无穷大）
- **LN_TABLE 独立内嵌**：与 V2.77 小世界系数中的表格相同，但各自内嵌（no_std，无全局常量）
- **u64 中间值安全**：n × 10^12 <= 128 × 10^12 < u64::MAX，无溢出风险

## 图论意义 / Graph Theory Significance

**V2.78 κ（度异质性）与 V2.80 γ̂（幂律指数）的互补关系：**

| 指标 | 问题 | 精度 |
|------|------|------|
| κ = ⟨k²⟩/⟨k⟩ (V2.78) | 分布是否异质？ | 定性（>2×、>3× 阈值） |
| γ̂ = 1 + n/Σln(k) (V2.80) | 异质性有多强？ | 定量（幂律衰减速率） |

**操作系统类比：**
κ 是"是否存在超级节点"的告警，γ̂ 是"超级节点影响力有多大"的量化。
γ ∈ [2, 3] 的网络（如万维网、引用图、社交网络）具有"无标度"特征——
极少数高度节点（hub）承载大量流量，单点故障风险高；
GOS 运维可通过 γ 值判断是否需要引入冗余路径或负载均衡。

## 与 V2.78 scale-free 检测的关系 / Relationship with V2.78

```
graph scale free  →  κ = 3.5 → "heterogeneous degree distribution"
graph power law   →  γ̂ = 4.9 → "γ>4: not consistent with power-law"
```
两者一致：高 κ 意味着存在 hub，但纯星形（spokes k=1）并不是真正的幂律分布，
γ > 4 正确反映了这一点（典型幂律 γ ∈ [2, 3]，星形图度分布是两点分布非幂律）。

## 下一步 / Next Steps

- Graph diameter view（combined center + peripheral 合并输出，纯 k-shell 函数）
- Shell `graph compare <snapshot>` — 保存并对比两个时间点的拓扑指标快照
- 将 V2.80 γ̂ 集成入 `graph summary` 面板（V2.79 aggregator）
