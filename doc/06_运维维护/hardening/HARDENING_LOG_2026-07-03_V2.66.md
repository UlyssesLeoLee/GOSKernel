# HARDENING LOG — V2.66: graph reciprocity + gos-graph-reciprocity-harness

**Date:** 2026-07-03  
**Version:** V2.66  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.66 新增 `graph_reciprocity()` API，计算有向图互惠性（reciprocity）。
该指标衡量有向边中存在反向对应边的比例：若 A→B 的同时 B→A 也存在，
则两条边均为互惠边。自环不计入总数。结果以 ppm 表示（1_000_000 = 完全互惠）。

V2.66 adds `graph_reciprocity()` to gos_runtime, measuring the fraction of directed
edges that are mutual (bidirectional). For each directed edge (u→v), the edge is
"mutual" if the reverse edge (v→u) also exists. Self-loops are excluded from both
the mutual count and the total count. Result is expressed in parts-per-million:
1_000_000 = fully reciprocal (all edges bidirectional), 0 = no mutual edges.

In the GOS kernel signal graph, reciprocity reveals whether inter-module communication
is strictly one-directional (low r) or bidirectional/feedback-capable (high r).
High reciprocity indicates a reactive, feedback-rich topology; low reciprocity
indicates a pipeline or command-dispatch topology.

Shell: `graph reciprocity` / `reciprocity` / `grecip`

---

## 背景 / Background

互惠性（reciprocity）是有向图的基础结构指标：

- **r = 1.0（1_000_000 ppm）**：所有边双向——完全双向通信网络
- **r = 0（0 ppm）**：无任何双向边——纯单向信号流（如 DAG / 流水线）
- **0 < r < 1**：部分双向——混合架构，部分模块间有反馈

在 GOS 内核图中，互惠性揭示内核信号拓扑的通信模式：
- 若 r → 0，内核以单向命令派发为主（适合批处理/流水线）
- 若 r > 0.5，存在大量双向响应通道（适合响应式/反应式系统）

互惠性与无向聚类系数、同配系数共同构成 GOS 图结构的三维画像：
聚类性（局部密度）、同配性（度混合模式）、互惠性（方向对称性）。

---

## 实现细节 / Implementation

### 核心算法（O(M²)）

```
对每条有向边 (u,v)（排除自环 u==v）：
  检查是否存在反向边 (v,u)
  mutual += 1 (若存在)

reciprocity_ppm = mutual * 1_000_000 / total_edges
```

最坏情况 O(M²)，MAX_EDGES=512 时最多 512×512=262_144 次比较——对内核规模完全可行。

### 返回值 / Return values

```rust
pub fn graph_reciprocity() -> (u32, usize, usize)
// (reciprocity_ppm, mutual_edges, total_edges)
```

- `reciprocity_ppm`：互惠边占总边数的比例，ppm 表示（0..=1_000_000）
- `mutual_edges`：存在反向边的有向边数量（双向对中每条边各计一次）
- `total_edges`：有向边总数（自环不计）

### 边界情况 / Edge cases

| 场景 | 返回值 |
|------|--------|
| 无边 | (0, 0, 0) |
| 无节点 | (0, 0, 0) |
| 有节点无边 | (0, 0, 0) |
| 自环（A→A） | 不计入 total；返回 (0, 0, 0) |
| 单向边 A→B | (0, 0, 1) |
| 互惠对 A→B + B→A | (1_000_000, 2, 2) |

---

## VectorAddress 命名空间扩展

VectorAddress L4=42 分配给 gos-graph-reciprocity-harness（测试隔离用）

新 L4 命名空间一览：
```
29=node-attr, 32=pal-boot, 33=pal-render, 34=node-attr-list, 35=graph-density,
36=node-attr-list-u8, 37=graph-clustering, 38=pal-full, 39=graph-transitivity,
40=graph-kcore, 41=graph-assortativity, 42=graph-reciprocity
```

---

## 修改文件 / Changed files

| 文件 | 修改内容 |
|------|----------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_reciprocity_inner()` 方法 + 公共 `graph_reciprocity()` 函数 |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_reciprocity()` shell 显示函数 |
| `crates/k-shell/src/proc.rs` | 新增路由：`graph reciprocity` / `reciprocity` / `grecip` + help 文本 |
| `host-tests/gos-graph-reciprocity-harness/Cargo.toml` | 新建测试 harness |
| `host-tests/gos-graph-reciprocity-harness/.cargo/config.toml` | host 目标覆盖 |
| `host-tests/gos-graph-reciprocity-harness/tests/graph_reciprocity.rs` | 10 个测试 |

---

## 测试矩阵 / Test matrix (10 tests, all passing)

| # | 场景 | 期望结果 |
|---|------|----------|
| 1 | 空图 | (0, 0, 0) |
| 2 | 有节点无边 | ppm=0, mutual=0, total=0 |
| 3 | 单向边 A→B | (0, 0, 1) |
| 4 | 互惠对 A→B + B→A | (1_000_000, 2, 2) |
| 5 | 路径 A→B→C（无反向）| (0, 0, 2) |
| 6 | 有向三角形 A→B→C→A | (0, 0, 3) |
| 7 | 混合：互惠对 + 单向边 | (666_666, 2, 3) |
| 8 | 完全双向四环 | (1_000_000, 8, 8) |
| 9 | 星形 hub→B/C/D + B→hub | (500_000, 2, 4) |
| 10 | 自环排除验证 | total=1, mutual=0, ppm=0 |

---

## 核心指标全貌 / Complete metric set

| 类别 | 指标 | 版本 |
|------|------|------|
| 连通性 | SCC 数 | V2.34 |
| 中心性 | Degree / PageRank | V2.38 / V2.43 |
| 可达性 | Eccentricity / diam / rad | V2.41 |
| 流量 | 最大流 (Edmonds-Karp) | V2.50 |
| 全局结构 | 图密度 | V2.59 |
| 局部结构 | 聚类系数 / 传递性 | V2.61 / V2.63 |
| 核-外围 | k-core 分解 / 退化度 | V2.64 |
| 混合模式 | 度同配系数 (Newman) | V2.65 |
| 方向对称 | 互惠性 (reciprocity) | V2.66 |
