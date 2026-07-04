# HARDENING LOG — V2.79 | 2026-07-03

## 版本 / Version
**V2.79** — Graph Topology Summary: One-Shot Aggregated Topology Report

## 变更摘要 / Change Summary

### 新增功能 / New Feature

**图论操作系统新增：图拓扑一站式报告（Graph Topology Summary）**

本次迭代新增 `graph summary` / `gsummary` k-shell 命令，在单次调用中聚合所有核心
图论指标，以分类面板形式展示，无需逐一输入多个命令。

**三个展示面板：**

```
 graph topology summary
 ─────────────────────────────────────────────────────────
  [structure]
  nodes        = N
  edges_undir  = M
  edges_dir    = E
  density      = D.DDD
  k_max        = K
  avg_k        = K.KKK

  [clustering]
  global CC    = G.GGG  (transitivity)
  avg CC       = A.AAA  (Watts-Strogatz)

  [efficiency]
  E_global     = G.GGG
  E_local      = L.LLL

  [network model]
  σ (small-world) = S.SSS
  κ (heterogen.)  = K.KKK
 ─────────────────────────────────────────────────────────
  σ>1: small-world network        ← 绿色高亮（σ > 1_000_000）
  κ≈⟨k⟩: homogeneous             ← 或 "heterogeneous" / "likely scale-free"
```

**指标来源汇总：**

| 面板 | 指标 | 来源版本 | 函数 |
|------|------|---------|------|
| structure | density | V2.59 | `graph_density()` |
| structure | k_max, avg_k | V2.78 | `graph_scale_free()` |
| structure | edges_undir | V2.77 | `graph_small_world()` |
| clustering | global CC | V2.63 | `graph_transitivity()` |
| clustering | avg CC | V2.75 | `graph_avg_clustering()` |
| efficiency | E_global | V2.74 | `graph_global_efficiency()` |
| efficiency | E_local | V2.76 | `graph_local_efficiency()` |
| network model | σ | V2.77 | `graph_small_world()` |
| network model | κ | V2.78 | `graph_scale_free()` |

### 实现详情 / Implementation Details

**crates/k-shell/src/lib.rs**
- 新增 `dispatch_graph_summary(sink)` — 纯显示函数，内部顺序调用 7 个现有运行时函数
- 每次函数调用独立持有并释放 RUNTIME 锁（顺序读取，图不变）
- 辅助函数 `print_ppm3`：ppm → W.XXX（3 位小数紧凑格式，适合表格对齐）
- 小世界 banner：σ > 1_000_000 → 绿色 "σ>1: small-world"；σ > 0 → "σ≈1: random-like"
- 无标度 banner：继承 V2.78 的三级分类（scale-free / heterogeneous / homogeneous）
- 空图特殊处理：early return 显示 "(empty graph — no nodes)"

**crates/k-shell/src/proc.rs**
- 新增命令路由（位于 scale-free 路由之前，避免前缀歧义）：
  - `"graph summary"` / `"gsummary"` / `"topology summary"` / `"topo summary"`

**host-tests/gos-graph-summary-harness/**
- `Cargo.toml` — 标准独立 workspace
- `.cargo/config.toml` — x86_64-pc-windows-msvc + build-std
- `tests/graph_summary.rs` — 10 个跨指标一致性测试用例
  - L4=55 命名空间（与已有 harness 隔离）

### 跨指标一致性约束 / Cross-Metric Consistency

harness 验证的关键跨指标关系：

| 约束 | 条件 | 已验证 |
|------|------|-------|
| 完全图 K_n：density=1 ↔ all CC=1 ↔ E_global=1 | bidirected K3 / K4 | ✓ |
| 正则图：kappa_ppm == avg_degree_ppm | K4 | ✓ |
| 星形图：kappa > avg_k | hub+3辐 | ✓ |
| 有向图（非双向）：E_global < 1.0 | 有向三角形 | ✓ |
| 孤立节点：E_global 下降（分母增大，分子不变）| K4+孤立 | ✓ |
| 纯有向链：E_local=0（无邻居之间的边）| A→B→C→D | ✓ |
| K3 双向：σ > 1（小世界结构）| bidirected K3 | ✓ |
| graph_density() 返回顺序：(ppm, node_count, edge_count)| 所有测试 | ✓ |

**重要发现（graph_density 返回顺序）：**
`graph_density()` → `(density_ppm, node_count, edge_count)`（第 2 项是 node_count，第 3 项是 edge_count）。
外部调用者应注意此顺序，避免与 `(ppm, edge_count, node_count)` 混淆。

### 调试说明 / Debug Notes

**ppm 精度选择：3 位小数（W.XXX）而非 6 位（W.XXXXXX）**

summary 视图选用 3 位小数是因为：
- 各指标在 [0, 1] 范围内，3 位已足够区分 0.001 粒度的差异
- 6 位数字（如 1_000_000）对齐后视觉凌乱
- 单独指标命令（graph small world、graph scale free 等）仍保留 6 位精度供精确读取

## 测试结果 / Test Results

```
running 10 tests
test empty_graph_all_zero ... ok
test single_node_all_zero_except_n ... ok
test two_nodes_one_edge_consistency ... ok
test bidirected_triangle_maximal_metrics ... ok
test directed_triangle_partial_efficiency ... ok
test path_efficiency_relationship ... ok
test k4_regular_kappa_equals_avg_k ... ok
test star_kappa_exceeds_avg_k ... ok
test k4_plus_isolated_penalizes_efficiency ... ok
test sigma_and_cc_relationship ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured
```

## VectorAddress 命名空间 / VectorAddress Namespace

**L4=55** — gos-graph-summary-harness

## 关键不变量 / Key Invariants

- **纯读取，不修改 epoch**：dispatch_graph_summary 调用的全部函数均为纯读取
- **顺序加锁**：每个 `gos_runtime::graph_*()` 调用均独立获取/释放 RUNTIME 锁，
  不存在死锁风险（k-shell 处于非中断上下文）
- **fbtest.rs render_frame 不调用此函数**：render_frame 禁止锁定 RUNTIME，
  summary 仅由用户命令触发，不在渲染路径中
- **空图保护**：n=0 时 early return，不调用任何度量函数，避免除零

## 图论意义 / Graph Theory Significance

`graph summary` 命令将 GOS 的图论能力汇聚为一个统一视图：

- **操作系统类比**：类似 `top` / `htop` 之于进程统计，`graph summary` 是图拓扑
  的"仪表板"——运维人员一条命令即可判断当前服务图是 "高效稳定"（σ>1, E_global高）
  还是 "脆弱中枢依赖"（κ>>⟨k⟩, E_local低）
- **产品成熟度信号**：V2.79 标志着 GOS 从 "单一指标系统" 成长为 "多维拓扑分析平台"，
  具备类比工业图数据库的综合诊断能力
- **下游价值**：summary 报告可作为 AI 辅助决策的上下文输入（AI_NATIVE_OS_PLAN §L1
  "context assembly = subgraph selection"）

## 依赖关系 / Dependencies

V2.79 是纯聚合层，零新算法，全部依赖已有运行时函数：

| 依赖版本 | 函数 | 用途 |
|---------|------|------|
| V2.59 | `graph_density()` | 密度 + 节点/边数 |
| V2.63 | `graph_transitivity()` | 全局聚类系数（转导率）|
| V2.74 | `graph_global_efficiency()` | 全局效率 E_global |
| V2.75 | `graph_avg_clustering()` | 平均聚类系数（WS 定义）|
| V2.76 | `graph_local_efficiency()` | 局部效率 E_local |
| V2.77 | `graph_small_world()` | 小世界系数 σ + m_undir |
| V2.78 | `graph_scale_free()` | 度异质性指数 κ + k_max + ⟨k⟩ |

## 下一步 / Next Steps

- Graph diameter view（combined center + peripheral 合并输出）
- Power-law exponent estimation via MLE（最大似然幂律指数估计）
- Shell `graph compare <g1> <g2>`（保存并对比两个图状态的拓扑指标）
