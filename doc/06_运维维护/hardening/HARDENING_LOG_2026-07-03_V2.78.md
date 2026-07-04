# HARDENING LOG — V2.78 | 2026-07-03

## 版本 / Version
**V2.78** — Graph Scale-Free Detection: Degree Heterogeneity Index κ = ⟨k²⟩/⟨k⟩

## 变更摘要 / Change Summary

### 新增功能 / New Feature

**图论操作系统新增：图无标度检测（Graph Scale-Free Detection）**

本次迭代实现度异质性指数 κ，作为无标度网络（scale-free network）的整数近似检测方法：

```
κ = ⟨k²⟩ / ⟨k⟩
```

其中 k_v 为节点 v 的**无向度数**（去重无向邻居数量，含双向有向边，不含自环）。

**三类拓扑分类：**

| 条件 | 分类 | 典型网络 |
|------|------|---------|
| κ_ppm > 3 × ⟨k⟩_ppm | likely scale-free（幂律度分布特征）| Barabási–Albert 网络 |
| κ_ppm > 2 × ⟨k⟩_ppm | heterogeneous（异质度分布）| 含 6+ 辐节点的星形图 |
| 否则 | homogeneous（均匀，正则/随机图特征）| k-正则图、E-R 随机图 |

**理论依据：**
- 对 k-正则图：⟨k²⟩ = k², ⟨k⟩ = k → κ = k = ⟨k⟩（均匀）
- 对 E-R 随机图：κ ≈ ⟨k⟩ + 1（接近均匀）
- 对无标度（幂律）网络：κ → ∞（随 n 发散）；实际大型 hub-spoke 图 κ >> ⟨k⟩

**最小满足无标度阈值的星形图：**
n 辐节点星形图满足 κ > 2⟨k⟩ 的条件为：
```
(n+1)² / (4n) > 2  →  n > 5.83  →  n ≥ 6 辐
```
6 辐星形图（7 节点）是在 MAX_NODES=128 约束下最小的可验证异质性案例。

### 实现详情 / Implementation Details

**crates/gos-runtime/src/lib.rs**
- 新增 `graph_scale_free_inner()` → `(u32, u32, u32, usize, usize)`
  - 返回 `(kappa_ppm, max_degree, avg_degree_ppm, node_count, m_undir)`
  - `kappa_ppm`：κ × 1_000_000（无边时为 0）
  - `max_degree`：最大无向度数 k_max
  - `avg_degree_ppm`：⟨k⟩ × 1_000_000（= sum_k × 1_000_000 / n）
  - `node_count`：存活节点总数
  - `m_undir`：去重无向边数
- 新增 `pub fn graph_scale_free()` 公开包装函数

**算法（纯整数，no_std 安全，无浮点）：**

1. **无向度数计算**：对每个存活节点 v，枚举所有边，收集去重无向邻居集合，获得 k_v
2. **聚合统计**：sum_k = Σk_v，sum_k2 = Σk_v²，max_k = max(k_v)
3. **无向边去重**（与 V2.77 一致）：有向边对 (u,v)+(v,u) 计为 1 条无向边
4. **整数除法**：
   - `kappa_ppm = sum_k2 × 1_000_000 / sum_k`（sum_k=0 时返回 0）
   - `avg_degree_ppm = sum_k × 1_000_000 / n`（n=0 时返回 0）
5. **溢出分析**（MAX_NODES=128，max_k ≤ 127）：
   - max(sum_k2) = 128 × 127² = 2_064_512
   - max(kappa_ppm) = 2_064_512 × 10⁶ / 1 ≈ 2.06 × 10¹²（超 u32，但实际 sum_k ≥ sum_k2 / 127 → kappa ≤ 127）
   - 实际 kappa_ppm ≤ 127 × 1_000_000 = 127_000_000 < u32::MAX ✓

**crates/k-shell/src/lib.rs**
- 新增 `dispatch_graph_scale_free(sink)` 格式化输出
  - κ 值（6 位小数 ppm 显示）
  - ⟨k⟩ 平均度数（6 位小数）
  - k_max 最大度数
  - 拓扑分类 banner（绿=likely scale-free，黄=heterogeneous，灰=homogeneous）
  - 节点数和无向边数页脚

**crates/k-shell/src/proc.rs**
- 新增命令路由：`"graph scale free"` / `"graph scale-free"` / `"gscalefree"` / `"scale free"`

**host-tests/gos-graph-scale-free-harness/**
- `Cargo.toml` — 标准独立 workspace
- `.cargo/config.toml` — x86_64-pc-windows-msvc + build-std
- `tests/graph_scale_free.rs` — 10 个测试用例

### 典型值 / Key Values

| 图结构 | κ_ppm | ⟨k⟩_ppm | 分类 |
|--------|-------|---------|------|
| 空图 | 0 | 0 | — |
| 单节点 | 0 | 0 | — |
| 两节点一边 | 1_000_000 | 1_000_000 | homogeneous（κ=⟨k⟩=1）|
| 双向三角形 K3 | 2_000_000 | 2_000_000 | homogeneous（2-正则）|
| 完全图 K4 | 3_000_000 | 3_000_000 | homogeneous（3-正则）|
| 星形（hub+3辐）| 2_000_000 | 1_500_000 | heterogeneous（κ>⟨k⟩ 但<2⟨k⟩）|
| 星形（hub+6辐）| 3_500_000 | 1_714_285 | heterogeneous（κ>2⟨k⟩）|

**正则图不变量：** 对 k-正则图，κ = ⟨k⟩ → kappa_ppm = avg_degree_ppm（恒等）

## 测试结果 / Test Results

```
running 10 tests
test empty_graph_all_zero ... ok
test single_node_no_edges ... ok
test two_isolated_nodes ... ok
test two_nodes_one_edge_regular ... ok
test bidirected_triangle_regular ... ok
test complete_k4_regular ... ok
test star_graph_heterogeneous ... ok
test directed_path_abcd ... ok
test k4_plus_isolated_heterogeneous ... ok
test large_star_heterogeneous_signature ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured
```

## VectorAddress 命名空间 / VectorAddress Namespace

**L4=54** — gos-graph-scale-free-harness

## 关键不变量 / Key Invariants

- **无向度数去重**：同一对节点的双向有向边只计为 1 条无向边，度数只加 1
- **sum_k=0 保护**：无边图 → kappa_ppm=0，不做除法
- **n=0 保护**：空图 → 直接返回全零
- **正则图等式**：k-正则图 kappa_ppm == avg_degree_ppm（测试 6/7 验证）
- **星形图阈值**：n≥6 辐时满足 kappa > 2×avg_k（测试 10 验证 6 辐，数学证明 n>5.83）
- **u64 中间运算**：sum_k2 × 1_000_000 最大约 2×10¹²，使用 u64 安全（u64::MAX ≈ 1.8×10¹⁹）

## 图论意义 / Graph Theory Significance

κ = ⟨k²⟩/⟨k⟩ 是刻画网络度分布"胖尾"程度的核心参数：

- **GOS 操作系统类比**：内核服务图中高 κ 意味着存在高度连接的"超级节点"（如
  调度器、内存管理器），它们的失效会显著影响全图连通性 → 设计层面需为高 κ 节点
  设置冗余和容错
- **与其他指标协同**：κ >> ⟨k⟩ 且密度低 → 幂律拓扑（可扩展但脆弱）；
  κ ≈ ⟨k⟩ 且密度适中 → 可维护随机拓扑
- **无浮点实现**：整数 ppm 编码与 V2.70（Wiener）、V2.75（avg_CC）一致，
  完全 no_std / bare-metal 安全

## 下一步 / Next Steps

- Graph topology summary（一站式 gsummary 聚合报告）→ V2.79
- Graph diameter view（combined center + peripheral 合并输出）
