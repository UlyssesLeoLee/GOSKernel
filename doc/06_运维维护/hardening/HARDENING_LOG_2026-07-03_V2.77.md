# HARDENING LOG — V2.77 | 2026-07-03

## 版本 / Version
**V2.77** — Graph Small-World Coefficient σ (Humphries–Gurney 2008)

## 变更摘要 / Change Summary

### 新增功能 / New Feature

**图论操作系统新增：图小世界系数 σ (Graph Small-World Coefficient)**

本次迭代实现 Humphries & Gurney (2008) 定义的小世界系数：

```
σ = (CC / CC_rand) / (L / L_rand)
```

其中：

| 符号 | 含义 | 来源 |
|------|------|------|
| CC | 平均聚类系数（每节点 Watts-Strogatz 定义） | V2.75 `graph_avg_clustering` |
| CC_rand | E-R 随机图基线 ≈ 2m / (n·(n−1)) | 无向边密度 |
| L | 平均有向路径长度 = Wiener / reachable_pairs | V2.70 `graph_wiener` |
| L_rand | E-R 基线 ≈ ln(n) / ln(⟨k⟩) | ⟨k⟩ = 2m/n（整数截断） |

**解读**：
- σ > 1：小世界结构（高局部聚类 + 短全局路径）
- σ ≈ 1：Erdős–Rényi 随机图特性
- σ = 0：连通性不足，无法计算系数

### 实现详情 / Implementation Details

**crates/gos-runtime/src/lib.rs**
- 新增 `graph_small_world_inner()` → `(u32, u32, u64, u64, usize, usize)`
  - 返回 `(sigma_ppm, cc_ppm, l_ppm, l_rand_ppm, node_count, m_undir)`
  - `sigma_ppm`：σ × 1_000_000（0 表示无法计算）
  - `cc_ppm`：CC × 1_000_000（始终返回，辅助调试）
  - `l_ppm`：L × 1_000_000（无路径时为 0）
  - `l_rand_ppm`：L_rand × 1_000_000（⟨k⟩ < 2 时为 0）
  - `node_count`：存活节点总数
  - `m_undir`：去重无向边数
- 新增 `pub fn graph_small_world()` 公开包装函数

**算法（纯整数，no_std 安全，无浮点）：**

1. **有向边去重** → 无向边集合，计算 m_undir
2. **CC** = 调用 `graph_avg_clustering_inner()`（V2.75 已有）
3. **L** = 调用 `graph_wiener_inner()`（V2.70 已有），L = Wiener / pairs
4. **CC_rand** = 2m / (n·(n−1))，直接整数除法
5. **⟨k⟩** = 2m / n（整数截断）
6. **L_rand** = LN_TABLE[n] / LN_TABLE[⟨k⟩]，使用编译期 ln 表（1..=128）
7. **σ** = (cc_ppm × l_rand_ppm × 1_000_000) / (cc_rand_ppm × l_ppm)，u128 防溢出

**ln 表设计**：编译期常量数组 `LN_TABLE: [u32; 129]`，存储 `ln(x) × 1_000_000`，
覆盖 x ∈ 1..=128，对应 MAX_NODES=128 的所有可能节点数和平均度数值。

**crates/k-shell/src/lib.rs**
- 新增 `dispatch_graph_small_world(sink)` 格式化输出
  - 6 decimal display（ppm → W.XXXXXX）
  - σ 值彩色高亮（绿=≥1 小世界，黄=<1，灰=未定义）
  - 显示 CC, L, L_rand 辅助值

**crates/k-shell/src/proc.rs**
- 新增命令路由：`"graph small world"` / `"graph small-world"` / `"gsmallworld"` / `"small world"`
- 更新 help 文本

**host-tests/gos-graph-small-world-harness/**
- `Cargo.toml` — 标准独立 workspace
- `.cargo/config.toml` — x86_64-pc-windows-msvc + build-std
- `tests/graph_small_world.rs` — 10 个测试用例

### 典型值 / Key Values

| 图结构 | σ |
|--------|---|
| 空图 / 单节点 / 孤立对 | 0（无法计算）|
| 有向单边 / 星形 / 路径（⟨k⟩ < 2） | 0（L_rand 未定义）|
| 有向三角形（3节点，3边）| ≈ 1.584963（ln(3)/ln(2)）|
| 双向三角形（3节点，6有向边） | ≈ 1.584963（相同 n,m_undir）|
| 双向 K4（4节点，12有向边） | ≈ 1.261860（ln(4)/ln(3)）|
| K4 + 孤立节点（5节点）| ≈ 3.095904（CC 稀释，L_rand 增大）|

## 测试结果 / Test Results

```
running 10 tests
test bidirected_k4_plus_isolated_sigma ... ok
test bidirected_k4_sigma ... ok
test bidirected_triangle_same_sigma_as_directed ... ok
test directed_triangle_sigma ... ok
test empty_graph_zero_sigma ... ok
test two_isolated_nodes_zero_sigma ... ok
test single_node_zero_sigma ... ok
test star_graph_zero_sigma ... ok
test path_three_nodes_zero_sigma ... ok
test two_nodes_one_edge_zero_sigma ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured
```

## VectorAddress 命名空间 / VectorAddress Namespace

**L4=53** — gos-graph-small-world-harness

## 关键不变量 / Key Invariants

- **⟨k⟩ 整数截断**：`avg_k = 2m/n`，必须 ≥ 2 且 < 129，否则 σ=0（LN_TABLE 边界）
- **u128 乘法防溢出**：σ 计算中间结果使用 u128，避免 u64 溢出
- **cc_ppm 始终返回**：即使 σ=0，cc_ppm 仍有效（方便调试）
- **l_ppm=0 时不除**：无有向路径时直接返回 σ=0
- **n < 2 时早返**：σ 逻辑需要 n ≥ 2
- **m_undir 不含自环**：边去重时过滤 `u == v`
- **LN_TABLE[1] = 0**：ln(1) = 0，⟨k⟩=1 时 L_rand 无穷大，σ=0 保护

## 依赖关系 / Dependencies

V2.77 复用了以下已有能力（无代码重复）：

| 依赖 | 版本 | 用途 |
|------|------|------|
| `graph_avg_clustering_inner` | V2.75 | 计算 CC |
| `graph_wiener_inner` | V2.70 | 计算 W(G) 和可达对数 → L |

## 图论意义 / Graph Theory Significance

小世界系数 σ 是 Watts-Strogatz 小世界网络模型的核心量化指标：

- **GOS 操作系统类比**：服务节点图中 σ > 1 意味着服务既有高局部冗余（容错）
  又有短信号传播路径（低延迟）——是 "可靠+高效" 内核拓扑的数学标志
- **判别力**：σ 同时捕获聚类（CC）和路径长度（L）两个维度，单纯看密度或
  直径均无法等价替代
- **随机基线修正**：CC_rand 和 L_rand 排除了图大小和密度的影响，使不同规模
  图之间的小世界程度可横向比较

与已有指标关系：
- 综合 `graph_avg_clustering` (V2.75) 和 `graph_wiener` (V2.70)
- 与 `graph_local_efficiency` (V2.76) 互补：E_loc 用最短路径效率衡量容错，σ 用 E-R 基线归一化衡量小世界性

## 下一步 / Next Steps

- Graph network summary view（σ + E_global + E_loc + CC 一站式报告）
- Graph scale-free detection（幂律度分布检验）
- Graph diameter view（combined center + peripheral 合并输出）
