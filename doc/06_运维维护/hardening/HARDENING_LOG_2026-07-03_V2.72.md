# HARDENING LOG — V2.72: graph peripheral nodes (ecc == diameter boundary set)

**Date:** 2026-07-03  
**Version:** V2.72  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.72 新增 `graph_peripheral()` API，计算有向图中的**外围节点集（Peripheral Nodes）**。
外围节点是离心率等于直径的节点——图中"距离最远的边界"。外围节点集是中心节点集的结构对立面。
纯 BFS 实现，no_std 安全，结果按 VectorAddress 升序排列，确定性输出。

V2.72 adds `graph_peripheral()` to gos_runtime, identifying the **peripheral node set**
of the live directed graph.  A peripheral node v satisfies ecc[v] = diameter — it lies
at the structural boundary of the graph, farthest from at least one other node.  Pure
integer BFS, no_std safe, output sorted ascending by VectorAddress.

Shell: `graph peripheral` / `gperiph`

---

## 背景 / Background

### 图论定义

**外围节点（Peripheral Nodes）** 定义为：

```
Periphery(G) = { v ∈ V : ecc(v) = diam(G) }
```

其中：
- `ecc(v) = max{ d(v,u) : u ∈ V, u ≠ v, d(v,u) < ∞ }` — 节点 v 的**离心率**（可达最远距离）
- `diam(G) = max{ ecc(v) : v ∈ V }` — 图的**直径**（最大离心率）

外围节点与中心节点（`ecc(v) = rad(G)`，其中 `rad(G) = min ecc(v)`）互为对立。
当 `rad(G) == diam(G)` 时，所有节点同时是中心节点和外围节点（正则图、完全图等情形）。

### 孤立节点处理

孤立节点的离心率为 0，而直径 `diam(G) ≥ 1`（只要图中存在至少一条边），因此孤立节点
永远不会出现在外围节点集中。若所有节点均为孤立节点，则 `diam(G) = 0` 且外围节点集为空。

### OS 类比

`traceroute` 中跳数最多的节点——哪些内核服务处于可达图的极限边界？
外围节点是系统拓扑中的"最远前哨"，关键路径往往以它们为终点。

---

## 实现细节 / Implementation

### API 签名

```rust
// crates/gos-runtime/src/lib.rs

/// Returns (vecs, ecc, peripheral_count, node_count, diameter).
pub fn graph_peripheral<const N: usize>(
) -> ([VectorAddress; N], [u32; N], usize, usize, u32)
```

**返回值语义：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `vecs[0..peripheral_count]` | `[VectorAddress; N]` | 外围节点的向量地址，按 VectorAddress 升序排列 |
| `ecc[0..peripheral_count]`  | `[u32; N]`           | 各外围节点的离心率（均等于直径） |
| `peripheral_count`           | `usize`              | 外围节点数量（上限 N） |
| `node_count`                 | `usize`              | 图中总活跃节点数 |
| `diameter`                   | `u32`                | 图的直径（全孤立图为 0） |

### 算法

```
for each node s:
    BFS from s along directed edges
    ecc[s] = max { dist[t] : t reachable from s, t ≠ s }
    (isolated s: ecc[s] = 0)

diameter = max { ecc[s] : s ∈ V }

Periphery = { s : diameter > 0 AND ecc[s] == diameter }

Sort Periphery ascending by VectorAddress.as_u64()
```

**复杂度：** O(V × (V + E))，与 `graph_eccentricity` 相同。

### 边界情况

| 情景 | diameter | peripheral_count | 说明 |
|------|---------|-----------------|------|
| 空图 | 0 | 0 | 无节点 |
| 全孤立节点 | 0 | 0 | ecc=0 全部，无法满足 ecc==diameter>0 |
| 正则图（所有节点 ecc 相等） | = rad | = node_count | 全部节点既是中心又是外围 |
| 非连通图 | max across components | 仅最长分量的源节点 | BFS 仅计算有向可达距离 |

### 关键实现细节

- 排序键使用 `VectorAddress::as_u64()` 确保 l4→l3→l2→offset 的自然字典序
- `diameter > 0` 守卫：防止全孤立图将所有节点（ecc=0）标记为外围
- 与 `graph_eccentricity_inner` 共享相同的 BFS 模板，保持一致性

---

## k-shell 集成

### 新增命令

| 命令 | 别名 | 说明 |
|------|------|------|
| `graph peripheral` | `gperiph` | 显示外围节点表（VectorAddress + ecc 列） |

### 输出格式

```
 graph peripheral nodes
 ───────────────────────────────────────────────────────────
  vector              ecc
  48.1.1.0              4   ← 红色高亮（外围边界）
  48.1.3.0              4
 ───────────────────────────────────────────────────────────
  diameter=4  peripheral=2  nodes=5
```

- 外围节点：**红色**（color 12），与 `graph eccentricity` 中外围行一致
- 空图或全孤立：显示说明性灰色文字，不打印空表
- 页脚：直径、外围节点数、总节点数

---

## 测试覆盖 / Test Coverage

新建测试集：`host-tests/gos-graph-peripheral-harness/tests/graph_peripheral.rs`

VectorAddress 命名空间：**L4 = 48**

| # | 测试名 | 测试场景 | 验证要点 |
|---|--------|---------|---------|
| 1 | `empty_graph_no_peripheral_nodes` | 空图 | peripheral=0, node=0, diam=0 |
| 2 | `isolated_node_not_peripheral` | 单孤立节点 | diam=0, peripheral=0, node=1 |
| 3 | `two_node_source_is_peripheral` | A→B | A (ecc=1=diam) 是外围，B (ecc=0) 不是 |
| 4 | `path_abc_source_is_only_peripheral` | A→B→C | 仅 A (ecc=2=diam) 是外围 |
| 5 | `directed_cycle_all_nodes_peripheral` | A→B→C→A | 全部 3 节点外围（radius==diam==2） |
| 6 | `star_out_center_is_only_peripheral` | A→{B,C,D} | 仅 A (ecc=1=diam) 是外围 |
| 7 | `linear_five_chain_source_is_only_peripheral` | A→B→C→D→E | 仅 A (ecc=4=diam) 是外围 |
| 8 | `disconnected_only_long_component_source_is_peripheral` | {A→B} ∥ {C→D→E} | 仅 C (ecc=2=diam) 是外围 |
| 9 | `complete_bidirectional_k3_all_peripheral` | K3 双向 | 全部 3 节点外围（diam=1） |
| 10 | `cycle_with_isolated_node_excludes_isolated_from_peripheral` | A→B→C→A + 孤立 D | {A,B,C} 外围，D 排除 |

**测试结果：** 10/10 通过

---

## VectorAddress L4 命名空间

本版本新增 L4=48：

```
29=node-attr, 32=pal-boot, 33=pal-render, 34=node-attr-list, 35=graph-density,
36=node-attr-list-u8, 37=graph-clustering, 38=pal-full, 39=graph-transitivity,
40=graph-kcore, 41=graph-assortativity, 42=graph-reciprocity, 43=graph-modularity,
44=graph-rich-club, 45=graph-girth, 46=graph-wiener, 47=graph-harmonic,
48=graph-peripheral  ← NEW
```

---

## 核心指标集更新

| 类别 | 指标 | 版本 |
|------|------|------|
| 连通性 | SCC 数 | V2.34 |
| 中心性 | 度/PageRank | V2.38/V2.43 |
| 可达性 | 离心率/直径/半径 | V2.41 |
| 流量 | 最大流 (Edmonds-Karp) | V2.50 |
| 全局结构 | 图密度 | V2.59 |
| 局部结构 | 聚类系数/传递性 | V2.61/V2.63 |
| 核心-外围 | k-core 分解/退化度 | V2.64 |
| 混合模式 | 度同配系数 (Newman) | V2.65 |
| 方向对称性 | 互惠性 | V2.66 |
| 社区质量 | 模块度 (Newman-Girvan Q) | V2.67 |
| 精英连接性 | 富人俱乐部系数 ρ(k) | V2.68 |
| 圈结构 | 围长（最短有向圈） | V2.69 |
| 路径总代价 | Wiener 指数 | V2.70 |
| 调和可达性 | 调和中心性 HC[v]=Σ 1/d(v,u) | V2.71 |
| **边界识别** | **外围节点集（ecc==diam）** | **V2.72** |

---

## 文件变更 / Changed Files

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `crates/gos-runtime/src/lib.rs` | 新增方法 + API | `graph_peripheral_inner<N>` + `graph_peripheral<N>` |
| `crates/k-shell/src/lib.rs` | 新增函数 | `dispatch_graph_peripheral` |
| `crates/k-shell/src/proc.rs` | 路由 + 帮助文本 | `graph peripheral` / `gperiph` 命令 |
| `host-tests/gos-graph-peripheral-harness/` | 新建测试集 | 10 个测试，L4=48，全部通过 |
| `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.72.md` | 新增归档 | 本文档 |

---

## 主机测试结果

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**累计测试数：683 + 10 = 693 个主机测试**
