# HARDENING LOG — V2.68: graph rich-club coefficient + gos-graph-rich-club-harness

**Date:** 2026-07-03  
**Version:** V2.68  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.68 新增 `graph_rich_club(k)` API，计算给定度阈值 k 下的富人俱乐部系数。
该指标衡量"高度节点"（度 > k）之间的连接密度，是网络科学中检测精英互联
（elite interconnection）的标准工具。纯整数实现，无浮点，no_std 安全。

V2.68 adds `graph_rich_club(k)` to gos_runtime, computing the rich-club coefficient
for degree threshold k. The metric measures how densely the "rich" nodes (those with
undirected degree > k) are connected to each other, relative to a clique of the same size.
High ρ(k) signals hub-to-hub elite connectivity; 1.0 means rich nodes form a perfect clique.
Directed edges are treated as undirected; self-loops are excluded.

Shell: `graph rich club <k>` / `richclub <k>` / `grichclub <k>`

---

## 背景 / Background

富人俱乐部系数 (Rich-Club Coefficient) 最早由 Zhou & Mondragón (2004) 提出，
是衡量网络中高度节点间互联密度的标准指标：

- **ρ(k) = E_{>k} / [N_{>k} × (N_{>k}−1) / 2]**
  - N_{>k}：度 > k 的节点数（"富人"集合大小）
  - E_{>k}：富人集合内的无向边数
  - 分母：N_{>k} 个节点可能的最大无向边数

- **ρ(k) = 1.0**：富人节点形成完全图（富人俱乐部中每两个节点都互连）
- **ρ(k) = 0**：富人节点之间没有边（或富人不足 2 个）
- **ρ(k) 随 k 增大**：若高度节点之间互联更紧密，则表现出"富人俱乐部效应"

在 GOS 内核图中，富人俱乐部系数揭示内核高中心度子系统的互联质量：
- k=0：所有有边连接的节点（等价于图密度的子集视角）
- k=1：度 ≥ 2 的节点（排除纯叶节点）
- k=2：度 ≥ 3 的节点（仅核心枢纽）

配合 k-core 分解（V2.64）和度同配系数（V2.65），
三者共同刻画网络的核-外围结构（core-periphery structure）。

---

## 实现细节 / Implementation

### 核心算法

```
1. 计算每个节点的无向度 deg[v]（邻居去重，有向边视为无向）
2. 筛选富人节点：deg[v] > k → rich_slots, rich_ids, n_rich
3. 若 n_rich < 2 → 早退，返回 (0, n_rich, 0)
4. 遍历所有边，保留两端都是富人的边；去重有向对 → e_rich
5. ρ_ppm = e_rich × 2_000_000 / (n_rich × (n_rich − 1))
```

纯整数运算，无浮点，no_std 安全。溢出分析：
- e_rich ≤ MAX_EDGES = 512
- n_rich ≤ MAX_NODES = 128
- 最大分子：512 × 2_000_000 = 1_024_000_000 < u64::MAX（~1.8×10¹⁹）
- 分母最小值（n_rich ≥ 2）：2 × 1 = 2（安全，无除零）

### 返回值 / Return values

```rust
pub fn graph_rich_club(k: u8) -> (u32, usize, usize)
// (rich_club_ppm, rich_node_count, edges_among_rich)
```

- `rich_club_ppm`：ρ(k) × 1_000_000，u32
- `rich_node_count`：满足 deg > k 的节点数 (N_{>k})
- `edges_among_rich`：富人集合内的无向边数 (E_{>k})

### 边界情况 / Edge cases

| 场景 | 返回值 |
|------|--------|
| 空图 | (0, 0, 0) |
| 孤立节点（k=0） | (0, 0, 0) — 无节点满足 deg > 0 |
| 富人节点数 < 2 | (0, n_rich, 0) — 无法形成边 |
| 富人节点间无边 | (0, n_rich, 0) |
| 富人节点形成完全图 | (1_000_000, n_rich, max_edges) |
| 互惠对 A↔B | 计为 1 条无向边（去重） |

### 无向度计算 / Undirected degree

与 graph_modularity / graph_assortativity 使用相同的邻居去重逻辑：
对每个节点 v，收集所有通过有向边 (u→v 或 v→u) 连接的唯一邻居集合，
邻居集合大小即为该节点的无向度 deg[v]。

---

## Shell 命令 / Shell command

```
graph rich club <k>   → 计算并显示 ρ(k)
richclub <k>          → 别名
grichclub <k>         → 别名
```

示例输出（k=1, K4 图）：
```
 graph rich club
  rich club: 100.00%  (1000000 ppm)
  k=1  rich_nodes=4  edges_among_rich=6
```

示例输出（k=0, 星形图 A→B,C,D）：
```
 graph rich club
  rich club: 50.00%  (500000 ppm)
  k=0  rich_nodes=4  edges_among_rich=3
```

参数 k 为十进制整数，范围 0–255。若 k 无效则打印红色错误提示。

---

## VectorAddress 命名空间扩展

VectorAddress L4=44 分配给 gos-graph-rich-club-harness（测试隔离用）

完整 L4 命名空间：
```
29=node-attr, 32=pal-boot, 33=pal-render, 34=node-attr-list, 35=graph-density,
36=node-attr-list-u8, 37=graph-clustering, 38=pal-full, 39=graph-transitivity,
40=graph-kcore, 41=graph-assortativity, 42=graph-reciprocity, 43=graph-modularity,
44=graph-rich-club
```

---

## 修改文件 / Changed files

| 文件 | 修改内容 |
|------|----------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_rich_club_inner()` 静态方法 + 公共 `graph_rich_club()` 函数 |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_rich_club()` shell 显示函数 |
| `crates/k-shell/src/proc.rs` | 新增路由：`graph rich club <k>` / `richclub <k>` / `grichclub <k>` + help 文本 |
| `host-tests/gos-graph-rich-club-harness/Cargo.toml` | 新建测试 harness |
| `host-tests/gos-graph-rich-club-harness/.cargo/config.toml` | host 目标覆盖 |
| `host-tests/gos-graph-rich-club-harness/tests/graph_rich_club.rs` | 10 个测试 |

---

## 测试矩阵 / Test matrix (10 tests, all passing)

| # | 场景 | k | ρ_ppm | n_rich | e_rich |
|---|------|---|-------|--------|--------|
| 1 | 空图 | 0 | 0 | 0 | 0 |
| 2 | 3 个孤立节点（k=0）| 0 | 0 | 0 | 0 |
| 3 | 星形 A→B,C,D（k=0）| 0 | 500_000 | 4 | 3 |
| 4 | 星形 A→B,C,D（k=1）| 1 | 0 | 1 | 0 |
| 5 | K4 完全图（k=2）| 2 | 1_000_000 | 4 | 6 |
| 6 | K4 完全图（k=0）| 0 | 1_000_000 | 4 | 6 |
| 7 | 路径 A→B→C→D（k=0）| 0 | 500_000 | 4 | 3 |
| 8 | 路径 A→B→C→D（k=1）| 1 | 1_000_000 | 2 | 1 |
| 9 | 互惠对 A↔B + C→D（k=0）| 0 | 333_333 | 4 | 2 |
| 10 | K3 + 孤立 D（k=1）| 1 | 1_000_000 | 3 | 3 |

**关键测试用例数学验证：**
- 测试 3（星形 k=0）：ρ = 3×2M/(4×3) = 6M/12 = **500_000** ✓
- 测试 5（K4 k=2）：ρ = 6×2M/(4×3) = 12M/12 = **1_000_000** ✓
- 测试 7（路径 k=0）：ρ = 3×2M/(4×3) = 6M/12 = **500_000** ✓
- 测试 8（路径 k=1）：ρ = 1×2M/(2×1) = 2M/2 = **1_000_000** ✓（B-C 内聚成团）
- 测试 9（互惠+对 k=0）：ρ = 2×2M/(4×3) = 4M/12 = **333_333** ✓
- 测试 10（K3+孤立 k=1）：ρ = 3×2M/(3×2) = 6M/6 = **1_000_000** ✓

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
| 社区质量 | 模块度 (Newman–Girvan Q) | V2.67 |
| 精英互联 | 富人俱乐部系数 (rich-club ρ) | V2.68 |
