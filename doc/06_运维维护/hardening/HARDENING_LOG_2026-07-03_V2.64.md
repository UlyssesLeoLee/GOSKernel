# HARDENING LOG — V2.64: graph k-core decomposition — coreness/degeneracy + gos-graph-kcore-harness

**Date:** 2026-07-03  
**Version:** V2.64  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.64 新增 `graph_kcore()` API，实现图的 k-核分解（Batagelj-Zaversnik 剥离算法）。
为每个活跃节点计算核度（coreness）：节点所属 k-核的最大 k 值。
图的退化度（degeneracy）= 最大核度，是刻画图层次化核-外围结构的基础指标。

V2.64 adds `graph_kcore()` to gos_runtime using the Batagelj-Zaversnik iterative
peeling algorithm. Each live node receives a coreness value: the largest k such that
the node is in the k-core (maximal subgraph where every node has undirected degree ≥ k).
The graph degeneracy = max coreness. This reveals the hierarchical core-periphery
structure of the kernel graph — core subsystems dominate signal traffic;
peripheral nodes are lightly connected.

Shell: `graph kcore` / `kcore` / `gkcore` / `coreness` — shows per-node
coreness with color-coded role (core/inner/periphery) and degeneracy footer.

---

## 背景 / Background

k-核分解是网络分析中的基础工具：
- **核（core）**：核度 = 最大核度的节点，是信号/算力最密集的子图
- **内层（inner shell）**：中间核度节点，衔接核心与外围
- **外围（periphery）**：核度=0 的孤立节点或悬挂节点

在内核图中：核心子系统（高度互联）会自然涌现为高核度节点；
外围子系统（仅通过少量边连接）有低核度。退化度是图密度的精炼指标。

---

## 算法 / Algorithm

**Batagelj-Zaversnik iterative peeling:**

```
1. 计算每个节点的无向有效度（去重邻居，去除自环）
2. for k = 1, 2, ..., max_degree:
     repeat:
       for each non-removed node v:
         if eff_deg[v] < k:
           coreness[v] = k - 1
           remove v
           for each non-removed neighbor u: eff_deg[u] -= 1
     until no more removals
3. 存活节点（未被移除）: coreness = k - 1
```

算法正确性示例：
- 三角形 A-B-C（每节点度=2）: k=1 无移除 → k=2 无移除 → k=3 全部度<3被移除(coreness=2) ✓
- 三角形+悬挂节点 D-A: k=2时 D度<2被移除(coreness=1)，A度由3降2仍≥2 → k=3时 A,B,C被移除(coreness=2) ✓
- K₄（每节点度=3）: k=1,2,3无移除 → 退出循环(k=4)，存活节点 coreness=3 ✓

---

## 变更范围 / Change Scope

### 1. `crates/gos-runtime/src/lib.rs`

新增 `graph_kcore_inner<const N: usize>` 方法（在 `graph_transitivity_inner` 之后）：

```rust
pub fn graph_kcore_inner<const N: usize>(&self) -> ([VectorAddress; N], [u8; N], usize, u8)
// Returns: (vecs, coreness, n, max_coreness)
//   vecs[0..n]     — nodes sorted by coreness descending
//   coreness[0..n] — coreness per node (0 = isolated)
//   n              — live node count
//   max_coreness   — graph degeneracy
```

公共 API：

```rust
pub fn graph_kcore<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, u8) {
    RUNTIME.lock().graph_kcore_inner::<N>()
}
```

### 2. `crates/k-shell/src/lib.rs`

新增 `dispatch_graph_kcore`:
- 调用 `gos_runtime::graph_kcore::<128>()`
- 彩色表格：绿色=core, 青色=inner, 灰色=periphery
- 页脚显示节点总数和退化度

### 3. `crates/k-shell/src/proc.rs`

新增 shell 路由（紧随 "graph transitivity" 之后）：

```
"graph kcore" | "kcore" | "gkcore" | "graph core" | "core decomp" | "coreness"
→ dispatch_graph_kcore
```

### 4. `host-tests/gos-graph-kcore-harness/` (新建)

- `Cargo.toml` — `[workspace]` 隔离
- `.cargo/config.toml` — target = x86_64-pc-windows-msvc
- `tests/graph_kcore.rs` — 10 个测试全绿

---

## 测试矩阵 / Test Matrix (gos-graph-kcore-harness)

VectorAddress 命名空间：L4=40

| # | 测试名 | 验证点 | 结果 |
|---|--------|--------|------|
| 1 | `empty_graph_returns_zero` | 空图 n=0, max_coreness=0 | ✅ |
| 2 | `single_isolated_node_coreness_zero` | 单孤立节点 coreness=0 | ✅ |
| 3 | `path_graph_coreness_one` | 路径 A→B→C 所有节点 coreness=1, degeneracy=1 | ✅ |
| 4 | `triangle_coreness_two` | 三角形 A-B-C 所有节点 coreness=2, degeneracy=2 | ✅ |
| 5 | `triangle_plus_pendant_mixed_coreness` | A/B/C coreness=2, 悬挂节点 D coreness=1 | ✅ |
| 6 | `k4_complete_graph_coreness_three` | K₄ 所有节点 coreness=3, degeneracy=3 | ✅ |
| 7 | `output_sorted_descending` | 输出按核度降序排列 | ✅ |
| 8 | `n_matches_registered_node_count` | n 与已注册节点数吻合 | ✅ |
| 9 | `star_graph_all_coreness_one` | 星形图（轴+3叶）所有节点 coreness=1 | ✅ |
| 10 | `two_disjoint_triangles_coreness_two` | 两个独立三角形，全6节点 coreness=2, degeneracy=2 | ✅ |

**全部通过 10/10**

---

## VectorAddress L4 命名空间 / Namespace

```
29=node-attr,    32=pal-boot,       33=pal-render,     34=node-attr-list,
35=graph-density, 36=node-attr-list-u8, 37=graph-clustering, 38=pal-full,
39=graph-transitivity, 40=graph-kcore
```

---

## 核心指标全景 / Core Metric Panorama

| 类别 | 指标 | 版本 |
|------|------|------|
| 连通性 | SCC 数量 | V2.34 |
| 中心性 | 度中心/PageRank | V2.38/V2.43 |
| 可达性 | 离心率/直径/半径 | V2.41 |
| 流量 | 最大流 (Edmonds-Karp) | V2.50 |
| 全局结构 | 图密度 | V2.59 |
| 局部结构 | 聚类系数 / 传递性 | V2.61/V2.63 |
| **核-外围结构** | **k-核分解 / 退化度** | **V2.64** |

---

## 不变量 / Invariants

- `graph_kcore_inner` 是纯读操作，不修改图状态，不推进 epoch
- 无向度计算：去重邻居（同一对节点多条边只计一次），去除自环
- 核度范围：[0, min(max_degree, 255)]，u8 足够（MAX_NODES=128, max_degree≤127）
- VectorAddress 命名空间：L4=40 保留给 gos-graph-kcore-harness 测试节点

---

## 累计测试数 / Cumulative Test Count

- 本次新增：10 tests (gos-graph-kcore-harness)
- 累计：**613 host tests** (603 + 10)
