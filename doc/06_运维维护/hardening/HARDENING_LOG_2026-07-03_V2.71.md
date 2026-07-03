# HARDENING LOG — V2.71: graph harmonic centrality (sum of reciprocal BFS distances)

**Date:** 2026-07-03  
**Version:** V2.71  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.71 新增 `graph_harmonic()` API，计算有向图中每个节点的**调和中心性（Harmonic Centrality）**。
调和中心性是经典接近中心性（Closeness Centrality）的改进版本，通过对距离取倒数求和的方式，
自然支持非强连通图——不可达节点对的贡献为零，无需特殊处理。纯整数 BFS，no_std 安全。

V2.71 adds `graph_harmonic()` to gos_runtime, computing **harmonic centrality** for each
live directed-graph node.  HC[v] = Σ 1_000_000/d(v,u) — the sum of scaled reciprocal BFS
distances.  Unlike closeness centrality, harmonic centrality handles disconnected graphs
naturally; unreachable pairs contribute 0.  Pure integer BFS, no_std safe.

Shell: `graph harmonic` / `gharm`

---

## 背景 / Background

### 图论定义

**调和中心性（Harmonic Centrality）** 定义为：

```
HC(v) = Σ_{u≠v, d(v,u)<∞} 1/d(v,u)
```

其中 d(v,u) 为从节点 v 到 u 的有向最短路径长度（BFS 无权图）。
实现中使用整数 ppm 缩放：每项为 `1_000_000 / d(v,u)`（截断整数除法）。

### 与接近中心性的区别

| 特性 | 接近中心性（V2.40）| 调和中心性（V2.71）|
|------|-------------------|-------------------|
| 公式 | N_reach × 1e6 / Σd | Σ (1e6/d) |
| 不可达图 | 需要单独计算归一化 | 自然处理（贡献为0） |
| 链头节点 | 链中间节点得分最高 | 链头节点得分最高 |
| 最大值 | 1_000_000（单跳）| 无固定上界（随可达邻居数增长）|

**关键差别——路径 A→B→C：**
- 接近中心性：CC[B]=1_000_000（比 CC[A]=666_666 更高），B（中间节点）胜出；
- 调和中心性：HC[A]=1_500_000（比 HC[B]=1_000_000 更高），A（源节点）胜出——
  因为 A 既能以 d=1 到达 B，又能以 d=2 到达 C，额外的 1/2 贡献使其超越 B。

### 内核意义

在 GOS 内核图中，调和中心性直接量化**服务节点的综合可达影响力**：

- **HC 高** → 该服务能在少跳数内直接/间接影响最多其他服务；
- **HC 低** → 该服务为孤立节点或路径末端（汇节点）；
- 在**断连图**中（如多插件不互联场景），各连通分量的计算完全独立，
  无需额外处理，与 Wiener 指数（V2.70）的设计一致。
- 调和中心性特别适合**插件图中的影响力分析**：哪个内核服务的故障会沿最短跳数传播？

---

## 算法 / Algorithm

```
对每个源节点 s 做 BFS（无权，有向）：
  dist[s] = 0，入队
  处理节点 v（dist = d_v）：
    对每条有向出边 v→w：
      若 dist[w] == u32::MAX（未访问）→ dist[w] = d_v+1，入队

BFS 结束后：
  对所有 t ≠ s：
    若 dist[t] != u32::MAX（可达）：
      hc[s] += 1_000_000 / dist[t]   （整数除法，dist[t] ≥ 1，无除零风险）

返回 ([VectorAddress; N], [u32; N], usize)
排列顺序：按 HC 降序（HC 最高的节点排在前）
```

**自环处理**：自环 A→A 时 `dist[A]=0` 已设置，BFS 遇到 A→A 边时跳过（已访问），
因此 HC[A] 不受自环影响（只有到其他节点 u≠A 的路径才贡献）。

**除零安全**：由于 BFS 仅将 `dist[t]` 设为 ≥1 的正整数（`dist[t]=0` 仅对 t=s），
所有参与求和的 `dist[t]` 必定 ≥1，整数除法 `1_000_000/dist[t]` 安全无溢出。

**复杂度**：O(V × (V + E))，同 Wiener 指数（V2.70）和接近中心性（V2.40）。

---

## 返回值 / Return values

```rust
pub fn graph_harmonic<const N: usize>() -> ([VectorAddress; N], [u32; N], usize)
// (vecs, hc_ppm, total)
```

| 字段       | 类型            | 含义                                      |
|------------|-----------------|-------------------------------------------|
| `vecs`     | [VectorAddress] | 节点向量地址，按 HC 降序排列              |
| `hc_ppm`   | [u32]           | HC 值（每项为 Σ 1_000_000/d，截断整数）  |
| `total`    | usize           | 活跃节点总数（填充到 min(total, N)）      |

### 边界情况 / Edge cases

| 场景                   | HC 值                |
|------------------------|----------------------|
| 空图                   | total=0              |
| 单节点（无边）         | HC=0                 |
| 孤立节点               | HC=0                 |
| 自环 A→A + 无其他边    | HC[A]=0              |
| A→B（单边）            | HC[A]=1_000_000, HC[B]=0 |
| 路径 A→B→C             | HC[A]=1_500_000, HC[B]=1_000_000, HC[C]=0 |
| 3-环 A→B→C→A           | 所有节点 HC=1_500_000 |
| 星形 A→{B,C,D}         | HC[A]=3_000_000，叶节点=0 |
| 钻石 A→{B,C}→D         | HC[A]=2_500_000, HC[B]=HC[C]=1_000_000, HC[D]=0 |
| 5-链 A→B→C→D→E         | HC[A]=2_083_333, HC[B]=1_833_333, HC[C]=1_500_000, HC[D]=1_000_000, HC[E]=0 |

---

## Shell 命令 / Shell command

```
graph harmonic  → 计算并显示所有节点的调和中心性（降序）
gharm           → 别名
```

示例输出（路径 A→B→C）：
```
 graph harmonic centrality
  47.1.1.0  HC=1500000
  47.1.2.0  HC=1000000
  47.1.3.0  HC=0
  nodes=3
```

示例输出（空图）：
```
 graph harmonic centrality
  harmonic: undefined (empty graph)
```

---

## VectorAddress 命名空间扩展

VectorAddress L4=47 分配给 gos-graph-harmonic-harness（测试隔离用）

完整 L4 命名空间：
```
29=node-attr, 32=pal-boot, 33=pal-render, 34=node-attr-list, 35=graph-density,
36=node-attr-list-u8, 37=graph-clustering, 38=pal-full, 39=graph-transitivity,
40=graph-kcore, 41=graph-assortativity, 42=graph-reciprocity, 43=graph-modularity,
44=graph-rich-club, 45=graph-girth, 46=graph-wiener, 47=graph-harmonic
```

---

## 修改文件 / Changed files

| 文件 | 修改内容 |
|------|----------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_harmonic_inner<N>()` 方法 + 公共 `graph_harmonic<N>()` 函数 |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_harmonic()` shell 显示函数 |
| `crates/k-shell/src/proc.rs` | 新增路由 `graph harmonic` / `gharm` + help 文本 |
| `host-tests/gos-graph-harmonic-harness/Cargo.toml` | 新建测试 harness |
| `host-tests/gos-graph-harmonic-harness/.cargo/config.toml` | host 目标覆盖 (x86_64-pc-windows-msvc) |
| `host-tests/gos-graph-harmonic-harness/tests/graph_harmonic.rs` | 10 个测试 |

---

## 测试矩阵 / Test matrix (10 tests, all passing)

| # | 场景 | HC[A] | HC[B] | HC[C] | HC[D] | HC[E] |
|---|------|-------|-------|-------|-------|-------|
| 1 | 空图 | — | — | — | — | — |
| 2 | 单节点无边 | 0 | — | — | — | — |
| 3 | A→B | 1_000_000 | 0 | — | — | — |
| 4 | 路径 A→B→C | 1_500_000 | 1_000_000 | 0 | — | — |
| 5 | 星形 A→{B,C,D} | 3_000_000 | 0 | 0 | 0 | — |
| 6 | 3-环 A→B→C→A | 1_500_000 | 1_500_000 | 1_500_000 | — | — |
| 7 | 钻石 A→{B,C}→D | 2_500_000 | 1_000_000 | 1_000_000 | 0 | — |
| 8 | 5-链 A→B→C→D→E | 2_083_333 | 1_833_333 | 1_500_000 | 1_000_000 | 0 |
| 9 | 断连 {A→B}∥{C→D} | 1_000_000 | 0 | 1_000_000 | 0 | — |
| 10 | 自环 A→A + B→C | 0 | 1_000_000 | 0 | — | — |

**关键数学验证（测试 8 — 5-链）：**
- HC[A] = 1e6/1 + 1e6/2 + 1e6/3 + 1e6/4 = 1_000_000 + 500_000 + 333_333 + 250_000 = 2_083_333 ✓
- HC[B] = 1e6/1 + 1e6/2 + 1e6/3 = 1_000_000 + 500_000 + 333_333 = 1_833_333 ✓
- HC[C] = 1e6/1 + 1e6/2 = 1_500_000 ✓

---

## 核心指标全貌 / Complete metric set (through V2.71)

| 类别 | 指标 | 版本 |
|------|------|------|
| 连通性 | SCC 数 | V2.34 |
| 中心性 | Degree / PageRank | V2.38 / V2.43 |
| 可达性 | Eccentricity / diam / rad | V2.41 |
| 接近性 | 接近中心性（Closeness） | V2.40 |
| 流量 | 最大流 (Edmonds-Karp) | V2.50 |
| 全局结构 | 图密度 | V2.59 |
| 局部结构 | 聚类系数 / 传递性 | V2.61 / V2.63 |
| 核-外围 | k-core 分解 / 退化度 | V2.64 |
| 混合模式 | 度同配系数 (Newman) | V2.65 |
| 方向对称 | 互惠性 (reciprocity) | V2.66 |
| 社区质量 | 模块度 (Newman–Girvan Q) | V2.67 |
| 精英互联 | 富人俱乐部系数 (rich-club ρ) | V2.68 |
| 环结构 | 围长 (shortest directed cycle) | V2.69 |
| 路径代价 | Wiener 指数 (sum of pairwise distances) | V2.70 |
| 调和可达性 | 调和中心性（Harmonic Centrality） | **V2.71** |
