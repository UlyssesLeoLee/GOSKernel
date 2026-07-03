# HARDENING LOG — V2.61: graph clustering coefficient (Watts-Strogatz)

**Date:** 2026-07-03  
**Version:** V2.61  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.61 为运行图添加全局聚类系数（Watts-Strogatz 变体）。聚类系数衡量图的"小世界"
特性——某节点的邻居之间互相连接的比例，是图论 OS 拓扑健康视图的核心指标之一。

V2.61 adds the global clustering coefficient — the Watts-Strogatz variant that
measures what fraction of "open triplets" (pairs of neighbors sharing a common node)
are actually "closed" (the two neighbors also share an edge). This is a foundational
small-world network metric, complementing V2.59's graph density (sparsity) with a
locality/cliquishness indicator.

与 V2.59 图密度的对比：
- **图密度** (V2.59): 全局视角 — 实际边 / 最大可能边
- **聚类系数** (V2.61): 局部视角 — 共同邻居之间的连接比例

两者组合可以区分：
- 稀疏低聚类（树状/路径图）
- 稀疏高聚类（小世界图，有局部三角形）
- 密集高聚类（完全图）

---

## 变更范围 / Change Scope

### 1. `crates/gos-runtime/src/lib.rs`

**新内部方法** `GraphRuntime::graph_clustering_inner() -> (u32, usize)`:

```rust
// For each node v with >= 2 undirected neighbors:
//   count triangle_pairs = edge pairs among v's neighbors (undirected)
//   count pair_triplets  = C(k, 2) = k*(k-1)/2
// clustering_ppm = total_triangle_pairs * 1_000_000 / total_pair_triplets
pub fn graph_clustering_inner(&self) -> (u32, usize) {
    let n = self.nodes.iter().filter(|s| s.is_some()).count();
    let mut total_triangles: u64 = 0;
    let mut total_triplets: u64 = 0;
    for slot in 0..MAX_NODES {
        // ... collect undirected neighbors ...
        // ... count edge-pairs among neighbors ...
    }
    if total_triplets == 0 { return (0, n); }
    let ppm = ((total_triangles * 1_000_000) / total_triplets).min(1_000_000) as u32;
    (ppm, n)
}
```

**新公开函数** `pub fn graph_clustering() -> (u32, usize)` — (clustering_ppm, node_count)

### 2. `crates/k-shell/src/lib.rs`

**新函数** `dispatch_graph_clustering(sink)` — 输出格式：

```
 graph clustering
  clustering: 66.67%  (666667 ppm)
  nodes=3
```

### 3. `crates/k-shell/src/proc.rs`

- **新路由** `cmd == "graph clustering"` / `"clustering"` / `"gcluster"`
- **帮助文本** 新增 `graph clustering` / `gcluster` 条目

### 4. `host-tests/gos-graph-clustering-harness/` (新建)

- `Cargo.toml` — `[workspace]` 隔离
- `.cargo/config.toml` — target = x86_64-pc-windows-msvc, build-std
- `tests/graph_clustering.rs` — 10 个测试全绿

---

## 算法设计 / Algorithm Design

```
graph_clustering_inner():
  for each node v in graph:
    collect undirected_neighbors(v) = {u : edge(v,u) OR edge(u,v) exists, u ≠ v}
    k = |undirected_neighbors(v)|
    if k < 2: skip

    pair_triplets += k*(k-1)/2  // C(k,2) unordered pairs of neighbors
    for each unordered pair (B, C) in undirected_neighbors(v):
      if edge(B,C) OR edge(C,B): triangle_pairs += 1

  if pair_triplets == 0: return (0, n)
  clustering_ppm = triangle_pairs * 1_000_000 / pair_triplets
```

**关键设计决策 / Key Design Decisions:**

| 决策 | 选择 | 理由 |
|------|------|------|
| 邻居集合 | 无向（in+out 并集） | 有向版本排除了方向相反的3-环，低估聚类度 |
| 聚类类型 | 全局（加权平均） | 比局部 CC 均值更稳定，避免单节点极值影响 |
| 输出格式 | ppm (u32) | 与现有 closeness/density 等指标一致，避免 soft-float |
| 分母保护 | total_triplets == 0 返回 (0, n) | 空图/无三角图不报错 |

**ppm 解读 / ppm Interpretation:**

| 值 | 含义 |
|---|------|
| `0` | 无三角形（树、DAG、路径图） |
| `333_333` | ≈33.3%，中等聚类 |
| `600_000` | 60%，较高聚类（小世界倾向） |
| `1_000_000` | 100%，完全三角化（每个开放三元组都闭合） |

---

## 图论 OS 意义 / Graph-Theory OS Significance

在 GOSKernel 中，聚类系数反映：
1. **进程间协作密度**：高聚类意味着节点之间形成"小组"（cluster），符合微内核的模块化设计目标
2. **网络健壮性**：有局部三角形的图在随机故障下更健壮（类比 Linux 内核的冗余路径）
3. **图重写触发条件**：当聚类系数低于阈值时，可触发图重写规则插入桥接边（待实现）

与 V2.59 密度配合使用的诊断矩阵：

| 密度  | 聚类系数 | 图类型         | OS 含义            |
|-------|----------|----------------|--------------------|
| 低    | 低       | 树/路径        | 严格层次结构       |
| 低    | 高       | 小世界         | 模块化集群架构 ✓   |
| 高    | 高       | 完全图/富连接  | 高耦合（可能问题） |

---

## 测试矩阵 / Test Matrix

| # | 测试名 | 验证点 | 结果 |
|---|--------|--------|------|
| 1 | `empty_graph_clustering_is_zero` | 空图 ppm=0, n=0 | ✅ |
| 2 | `single_node_no_triplets` | 单节点无三元组 ppm=0 | ✅ |
| 3 | `two_nodes_one_edge_no_triplets` | A→B：无节点有>=2邻居 ppm=0 | ✅ |
| 4 | `path_abc_no_triangles` | A→B→C：有三元组无三角形 ppm=0 | ✅ |
| 5 | `triangle_full_clustering` | A→B, A→C, B→C：完全三角化 ppm=1_000_000 | ✅ |
| 6 | `directed_3_cycle_full_clustering` | A→B→C→A：有向3-环=完全聚类 ppm=1_000_000 | ✅ |
| 7 | `star_no_inner_edges_zero_clustering` | 星图（A→B,C,D）无内边 ppm=0 | ✅ |
| 8 | `partial_clustering_three_fifths` | 3/5三元组有三角 ppm=600_000 | ✅ |
| 9 | `reset_clears_clustering` | reset后 ppm=0, n=0 | ✅ |
| 10 | `graph_clustering_does_not_bump_epoch` | 不推进 graph_epoch | ✅ |

**全部通过 10/10**

---

## 不变量 / Invariants (never break)

- `graph_clustering` 是纯读操作，不修改任何图状态，不推进 epoch
- 当 total_triplets == 0 时返回 `(0, n)`（未定义，非错误）
- ppm 上限 1_000_000，下限 0
- 无向邻居集合去重：同一节点不会重复计入
- VectorAddress L4=37 保留给 gos-graph-clustering-harness 测试节点
- 栈内 `neighbors` 缓冲区大小 = MAX_NODES = 128（2048 字节）

---

## 指标体系完整度 / Metric System Completeness

V2.61 后，GOSKernel 已覆盖图论 OS 的核心拓扑诊断指标：

| 类别     | 指标                  | 版本  |
|----------|-----------------------|-------|
| 连通性   | SCC 数量              | V2.34 |
| 中心度   | 度中心度/PageRank      | V2.38/V2.43 |
| 可达性   | 离心率/直径/半径       | V2.41 |
| 流量     | 最大流                 | V2.50 |
| 全局结构 | **图密度** (V2.59)    | V2.59 |
| 局部结构 | **聚类系数** (V2.61)  | V2.61 |

---

## 累计测试数 / Cumulative Test Count

- 本次新增：10 tests (gos-graph-clustering-harness)
- 累计：**583 host tests** (573 + 10)
