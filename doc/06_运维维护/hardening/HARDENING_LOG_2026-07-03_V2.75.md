# GOSKernel Hardening Log — V2.75
**Date:** 2026-07-03  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated Hardening Pass

---

## 变更摘要 (Change Summary)

**V2.75 — 图平均聚类系数 (Graph Average Clustering Coefficient)**

新增 `graph avg clustering` 指令：计算图的真实 Watts-Strogatz 平均聚类系数 avg_CC——每节点局部聚类系数的无权平均值。

本指标与 V2.61 (`graph clustering`) 和 V2.63 (`graph transitivity`) 的关键区别：后两者实际上计算相同的**全局过渡性比率** (total_triangles/total_triplets)，对高度数节点有更高权重。本版本实现的 avg_CC 是**无权平均**，对每个节点赋予相同权重，是标准 WS 聚类系数定义。

---

## 数学定义 (Mathematical Definition)

对于图 G = (V, E)，n = |V|：

$$\text{avg\_CC} = \frac{1}{n} \sum_{v \in V} C(v)$$

$$C(v) = \frac{\text{triangles}(v)}{\binom{k_v}{2}} = \frac{\text{edges among neighbours of } v}{k_v(k_v-1)/2}$$

其中：
- $k_v$：节点 v 的无向度（去重，排除自环）
- $\text{triangles}(v)$：v 的邻居之间的无向边数
- 度 < 2 的节点贡献 $C(v) = 0$

**与相关指标的对比：**

| 指标 | 公式 | 特点 |
|------|------|------|
| graph clustering (V2.61) | total_triangles / total_triplets | 全局比率，等同于 transitivity |
| graph transitivity (V2.63) | 同上 | 标记不同但结果相同 |
| **avg clustering (V2.75)** | (1/n) × Σ C(v) | 无权均值，WS 标准定义 |

**边界情况：**

| 情形 | 结果 |
|------|------|
| 空图 (n=0) | avg_CC=0 |
| 单节点 | avg_CC=0 |
| 无边图 | avg_CC=0 |
| 路径图 | avg_CC=0 (中间节点无三角形) |
| 完全无向图 K_n | avg_CC=1.0 |
| 三角形图 | avg_CC=1.0 |
| 三角形 + 孤立节点 | 0 < avg_CC < 1 |

**OS 类比：** 类似于进程调度图中的局部密度均值——avg_CC 高说明节点邻域整体高度互连，调度热路径有多路冗余。

---

## 新增 API (New API)

### gos-runtime

```rust
/// V2.75: 返回 (avg_ppm, nodes_computed, node_count)
/// avg_ppm       = avg_CC × 1_000_000  (0..=1_000_000)
/// nodes_computed = 无向度 ≥ 2 的节点数（参与求和）
/// node_count    = 存活节点总数（求均值的分母）
pub fn graph_avg_clustering() -> (u32, usize, usize)
```

精度说明：ppm 精度为 1e-6，整数运算，无浮点误差。

### k-shell

```
graph avg clustering  — 计算并显示平均聚类系数 avg_CC
gavgcc                — 最短别名
```

输出格式：绿色高亮 `avg CC = X.XXXXXX`（6位小数），页脚显示 `nodes_computed=X nodes=X`。

---

## 算法 (Algorithm)

与 V2.61 (`graph_clustering_inner`) 相同的邻居枚举框架，但计算改为**每节点独立求比**再求均值：

1. 枚举每个存活节点 v
2. 收集无向邻居集合（去重，排除自环），得到 k_v
3. 若 k_v < 2：跳过（贡献 0，nodes_computed 不计入）
4. 枚举所有邻居对 (b, c)：若存在无向边 b-c，triangles_v++
5. cc_ppm_v = triangles_v × 1_000_000 / C(k_v, 2)
6. sum_cc_ppm += cc_ppm_v，nodes_computed++
7. avg_ppm = sum_cc_ppm / n（注意：分母是全部存活节点 n，不是 nodes_computed）

**关键点：**
- 分母 n 包括度 < 2 的节点（它们贡献 0，拉低均值）
- 这正是 WS 标准定义，与 NetworkX 的 `average_clustering()` 一致

**复杂度：** O(V × E)，与 V2.61 相同。

---

## VectorAddress 命名空间

L4=51 用于本版本 host-test harness：`VectorAddress::new(51, 1, x, 0)`

---

## 新增文件 (New Files)

| 文件 | 说明 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | +`graph_avg_clustering_inner()` + `pub fn graph_avg_clustering()` |
| `crates/k-shell/src/lib.rs` | +`dispatch_graph_avg_clustering()` |
| `crates/k-shell/src/proc.rs` | 路由 "graph avg clustering"/"gavgcc" + 帮助文本 |
| `host-tests/gos-graph-avg-clustering-harness/Cargo.toml` | 新 harness |
| `host-tests/gos-graph-avg-clustering-harness/.cargo/config.toml` | x86_64-pc-windows-msvc 目标覆盖 |
| `host-tests/gos-graph-avg-clustering-harness/tests/graph_avg_clustering.rs` | 10 个测试用例 |

---

## 测试用例 (Test Cases — 10/10 PASS)

| # | 图结构 | 期望 ppm | 期望 nodes_computed |
|---|--------|---------|---------------------|
| 1 | 空图 | 0 | 0 |
| 2 | 单孤立节点 | 0 | 0 |
| 3 | 2 节点无边 | 0 | 0 |
| 4 | 路径 A→B→C | 0 | 1 (B 有 k=2 但无三角形) |
| 5 | 三角形 A→B,B→C,A→C | 1_000_000 | 3 |
| 6 | 星形 A→{B,C,D} | 0 | 1 (A 有 k=3，邻居间无边) |
| 7 | 三角形 + 尾 (+ D→A) | 583_333 | 3 |
| 8 | 三角形 + 孤立 D | 750_000 | 3 |
| 9 | 双三角共边 (+ B→D,C→D) | 833_333 | 4 |
| 10 | 完全 K4 (6条单向边) | 1_000_000 | 4 |

---

## 不变量确认 (Invariant Checks)

- [x] render_frame in fbtest.rs 不锁 RUNTIME（本 PR 不修改 fbtest.rs）
- [x] VectorAddress 无 ::ZERO 常量（均使用 `VectorAddress::new(0,0,0,0)`）
- [x] graph_avg_clustering 为纯读操作，不 bump epoch
- [x] 无 non-ASCII hex escape（shell 字符串均使用 `\u{xxxx}` Unicode 转义）
- [x] L4=51 命名空间未被占用
- [x] 10/10 harness 测试通过

---

## 累计指标 (Cumulative Metrics)

- **Host-test total: 723 tests** (V2.75 增加 10 个，V2.74 前为 713)
- **核心图论指标集：**

| 类别 | 指标 | 版本 |
|------|------|------|
| 局部结构(无权均值) | 平均聚类系数 avg_CC (WS 标准) | **V2.75 ★** |
| 全局效率 | 全局图效率 E(G) = Σ 1/d(i,j)/(n*(n-1)) | V2.74 |
| 中心识别 | 中心节点 (ecc==radius) | V2.73 |
| 边界识别 | 外围节点 (ecc==diameter) | V2.72 |
| 调和可达性 | 调和中心性 HC[v]=Σ 1/d(v,u) | V2.71 |
| 路径总代价 | Wiener 指数 | V2.70 |
| 环结构 | 围 (最短有向环长) | V2.69 |
| 精英连接 | Rich-club 系数 ρ(k) | V2.68 |
| 社区质量 | 模块度 (Newman-Girvan Q) | V2.67 |
| 方向对称性 | 互反性 | V2.66 |
| 混合模式 | 度同配系数 (Newman) | V2.65 |
| 核心-外围 | k-core 分解 | V2.64 |
| 局部结构(全局比) | 聚类系数/传递性 (均等同于全局比率) | V2.61/V2.63 |
| 全局结构 | 图密度 | V2.59 |

---

*自动化硬化任务 · 每 2 小时执行一次 · 保持图论操作系统的产品级水准*
