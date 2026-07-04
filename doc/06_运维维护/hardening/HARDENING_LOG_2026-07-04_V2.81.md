# HARDENING LOG — V2.81 | 2026-07-04

## 版本 / Version
**V2.81** — γ̂ 集成入 Graph Summary 面板：拓扑摘要含幂律指数显示

## 变更摘要 / Change Summary

### 新增功能 / New Feature

**图论操作系统：将幂律指数 γ̂（V2.80）集成入 `graph summary` 一站式拓扑摘要面板**

本次迭代将 V2.80 的 `graph_power_law()` 输出集成入 V2.79 的 `dispatch_graph_summary`，
实现单命令呈现所有关键拓扑度量的完整视图：结构 + 聚类 + 效率 + 网络模型（含 γ̂）。

**更新后的 `graph summary` 输出结构：**
```
 graph topology summary
 ───────────────────────────────────────────────────────────
  [structure]
  nodes        = N
  edges_undir  = M
  edges_dir    = E
  density      = W.XXX
  k_max        = K
  avg_k        = W.XXX

  [clustering]
  global CC    = W.XXX  (transitivity)
  avg CC       = W.XXX  (Watts-Strogatz)

  [efficiency]
  E_global     = W.XXX
  E_local      = W.XXX

  [network model]
  σ (small-world) = W.XXX          ← V2.77
  κ (heterogen.)  = W.XXX          ← V2.78
  γ̂ (power-law)  = W.XXX          ← V2.81 新增
 ───────────────────────────────────────────────────────────
  σ>1: small-world network          ← σ 分类
  κ≫⟨k⟩: likely scale-free         ← κ 分类
  γ̂∈[1,3]: compatible with power-law / scale-free tail  ← γ̂ 分类 (新增)
```

**γ̂ 分类区间（与 V2.80 独立命令一致）：**

| γ̂ 值 | 分类 Banner | 颜色 |
|-------|------------|------|
| 0 | `γ̂: MLE undefined (all non-isolated k=1 or no edges)` | 灰色 |
| ∈ [1, 3] | `γ̂∈[1,3]: compatible with power-law / scale-free tail` | 绿色 |
| ∈ (3, 4] | `γ̂∈(3,4]: steep tail; weakly heterogeneous` | 黄色 |
| > 4 | `γ̂>4: not consistent with power-law degree distribution` | 白色 |

### 实现详情 / Implementation Details

**crates/k-shell/src/lib.rs — `dispatch_graph_summary`**

1. 在顶部 metric 收集区增加：
   ```rust
   let (gamma_ppm, _n_fit, _) = gos_runtime::graph_power_law();
   ```
2. 在 `[network model]` 节区 κ 行之后增加 γ̂ 行（沿用 `print_ppm3` 3位小数格式）：
   ```rust
   print_str(sink, "\n  γ̂ (power-law)  = ");
   if gamma_ppm == 0 { ... "undef" ... } else { print_ppm3(gamma_ppm) }
   ```
3. 在 classification banner κ 分类之后增加 γ̂ 分类（同 `dispatch_graph_power_law` 逻辑）

**注意：** 摘要面板用 3 位小数（W.XXX），独立 `graph power law` 命令用 6 位（W.XXXXXX）。

### 新增测试 / New Tests

**host-tests/gos-graph-summary2-harness/ (L4=57)**

10 个测试，专注于 γ̂ 与其他摘要指标的跨度量不变量：

| # | 图结构 | 关键验证 |
|---|--------|---------|
| 1 | 空图 | gamma=0, n_fit=0, n=0 |
| 2 | 单孤立节点 | gamma=0, n_fit=0, n=1 |
| 3 | 两节点一条边 A→B (k=1,1) | gamma=0 (sum_ln=0, MLE 退化) |
| 4 | 双向三角形 (k=2,2,2) | gamma_ppm=2_442_695 ∈ [1,3] |
| 5 | K4 完全图 双向 (k=3,3,3,3) | gamma_ppm=1_910_239 ∈ [1,3] |
| 6 | K4 + 孤立节点 | n_fit=4 不变，gamma 不变 |
| 7 | 有向链 A→B→C→D (k=1,2,2,1) | gamma_ppm=3_885_390 ∈ (3,4] |
| 8 | 有向星形 hub+3辐 (k=3,1,1,1) | gamma_ppm=4_640_957 >4 |
| 9 | K4 regular graph | gamma∈[1,3] 与 kappa=avg_k 共现 |
| 10 | n_fit 结构不变量 | n_fit = node_count - isolated_count |

**精确值验证（整数算术）：**
```
双向三角形: sum_ln = 3×LN[2] = 3×693_147 = 2_079_441
  gamma = 1_000_000 + 3×10^12 / 2_079_441 = 2_442_695  ✓

K4 完全图: sum_ln = 4×LN[3] = 4×1_098_612 = 4_394_448
  gamma = 1_000_000 + 4×10^12 / 4_394_448 = 1_910_239  ✓ (运行时整数截断)

有向链: sum_ln = 2×LN[2] = 2×693_147 = 1_386_294
  gamma = 1_000_000 + 4×10^12 / 1_386_294 = 3_885_390  ✓

有向星形: sum_ln = LN[3] = 1_098_612
  gamma = 1_000_000 + 4×10^12 / 1_098_612 = 4_640_957  ✓ (运行时整数截断)
```

## 测试结果 / Test Results

```
gos-graph-summary-harness (V2.79 回归测试):
running 10 tests
..........
test result: ok. 10 passed; 0 failed

gos-graph-summary2-harness (V2.81 新增):
running 10 tests
..........
test result: ok. 10 passed; 0 failed
```

## VectorAddress 命名空间 / VectorAddress Namespace

**L4=57** — gos-graph-summary2-harness

（L4=56 为 V2.80 gos-graph-power-law-harness）

## 关键不变量 / Key Invariants

- **γ̂ 在 summary 中沿用 print_ppm3**（3位小数，与摘要其他指标格式一致）
- **独立命令 `graph power law` 沿用 print_ppm6**（6位小数，高精度显示）
- **gamma=0 时显示 "undef"**（与 κ=0 显示 "undef" 对称处理）
- **n_fit 不暴露于摘要**（摘要只显示值，不显示拟合样本数）
- **分类 banner 逻辑与 dispatch_graph_power_law 完全一致**（单一真相来源）
- **纯读取**：graph_power_law 是 pure read，不 bump epoch

## 累积测试套件 / Cumulative Test Suite

| 新增 Harness | 测试数 |
|-------------|--------|
| gos-graph-summary2-harness (V2.81) | 10 |
| 累积总数 | **783 tests** (773 + 10) |

## 下一步 / Next Steps

- Graph diameter view（combined center + peripheral 合并输出，单命令 `graph diameter`）
- Shell `graph compare <snapshot>` — 保存并对比两个时间点的拓扑指标快照
- 考虑将 `graph_clustering`（V2.61）重命名为 `graph_transitivity`，消除"Watts-Strogatz"命名误导
