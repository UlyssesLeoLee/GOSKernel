# HARDENING LOG — V2.70: graph Wiener index (sum of pairwise BFS distances)

**Date:** 2026-07-03  
**Version:** V2.70  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.70 新增 `graph_wiener()` API，计算有向图的 **Wiener 指数**——图中所有可达有序节点对
之间的有向最短路径长度之和。Wiener 指数是衡量图的整体"紧密程度"和"传输效率"的经典
图论参数，与平均路径长度密切相关。纯 BFS 实现，无浮点核心，no_std 安全。

V2.70 adds `graph_wiener()` to gos_runtime, computing the **Wiener index** of the directed
graph — the sum of all pairwise directed shortest-path distances.  The Wiener index is a
classic graph-theory parameter measuring the overall "compactness" and routing efficiency of
the kernel process graph.  Pure BFS implementation, integer arithmetic, no_std safe.

Shell: `graph wiener` / `gwiener`

---

## 背景 / Background

### 图论定义

**Wiener 指数（Wiener Index）** 定义为：

```
W(G) = Σ_{u≠v, d(u,v)<∞} d(u,v)
```

其中 d(u,v) 为从节点 u 到节点 v 的有向最短路径长度（BFS 无权图）。
不可达对（无有向路径）不计入求和。

相关衍生量：
- **平均路径长度（Average Path Length）** = W(G) / 可达对数
- **可达对数（Reachable Pairs）** = 满足 d(u,v)<∞ 的有序对 (u,v) 数量（u≠v）

对于**强连通图**，所有 n(n-1) 个有序对均可达；
对于**有向无环图（DAG）**，许多对不可达，仅计算存在路径的对。

| 图结构         | W(G)                    | 可达对数        |
|----------------|-------------------------|-----------------|
| 空图 / 孤立节点 | 0                       | 0               |
| 有向链 A→B→C    | 1+2+1 = 4               | 3               |
| 有向三角 A→B→C→A | 1+2+1+2+1+2 = 9        | 6               |
| 完全有向图 K3   | 6 (全距为 1)            | 6               |

### 内核意义

在 GOS 内核图中，Wiener 指数直接量化**进程间信号传输效率**：

- **W 越小**：信号在图中需要的跳数越少，内核响应越快（"紧凑"拓扑）；
- **W 越大**：信号路径较长，可能存在瓶颈或过度间接的依赖链；
- **平均路径长度**：Wiener 指数除以可达对数，给出"平均需要几跳"的直觉量；
- **可达对数 vs. 理论最大值 n(n-1)**：反映图的连通程度（接近最大值 = 强连通）。

配合已有的离心率/直径/半径（V2.41）和 SCC（V2.34），三者共同刻画图的路径结构：
- `graph_eccentricity` → 各节点的最远可达距离（局部视角）
- `graph_scc` → 哪些节点强连通
- `graph_wiener` → 全局路径代价总和（系统视角）

---

## 算法 / Algorithm

```
对每个源节点 s 做 BFS（无权，有向）：
  dist[s] = 0，入队
  处理节点 cur (dist = d)：
    对每条有向出边 cur→nbr：
      若 dist[nbr] == u32::MAX（未访问）→ dist[nbr] = d+1，入队

BFS 结束后：
  对所有 t ≠ s，若 dist[t] < u32::MAX：
    wiener_index += dist[t]
    reachable_pairs += 1

返回 (wiener_index: u64, reachable_pairs: usize, node_count: usize)
```

**自环的处理**：自环 A→A 在 BFS 中不影响结果——`dist[s]=0` 已设置，
当枚举出边 A→A 时 `dist[s]` 已访问，直接跳过，不重入队列。

**复杂度**：O(V × (V + E))，对于 V ≤ 128、E ≤ 512 完全可接受（≤ 131072 次操作）。

---

## 返回值 / Return values

```rust
pub fn graph_wiener() -> (u64, usize, usize)
// (wiener_index, reachable_pairs, node_count)
```

| 字段              | 类型    | 含义                                          |
|-------------------|---------|-----------------------------------------------|
| `wiener_index`    | u64     | 所有有限有向对的最短路径距离之和               |
| `reachable_pairs` | usize   | 满足 d(u,v)<∞ 的有序对 (u,v) 数（u≠v）        |
| `node_count`      | usize   | 活跃节点总数                                   |

**设计说明**：`wiener_index` 使用 `u64` 以避免溢出——理论最坏情况为
128 个节点的线性链：W = Σ_{k=1}^{127} k × (128-k) ≈ 350_000，远低于 u64::MAX。

### 边界情况 / Edge cases

| 场景                   | wiener_index | reachable_pairs | node_count |
|------------------------|-------------|-----------------|------------|
| 空图                   | 0           | 0               | 0          |
| 单节点（无边）         | 0           | 0               | 1          |
| n 个孤立节点           | 0           | 0               | n          |
| 单节点自环 A→A         | 0           | 0               | 1          |
| A→B（单条边）          | 1           | 1               | 2          |
| 有向链 A→B→C           | 4           | 3               | 3          |
| 有向三角               | 9           | 6               | 3          |
| 完全有向 K3            | 6           | 6               | 3          |
| 断连：A→B + 孤立 C     | 1           | 1               | 3          |

---

## Shell 命令 / Shell command

```
graph wiener   → 计算并显示 Wiener 指数
gwiener        → 别名
```

示例输出（有向三角 A→B→C→A）：
```
 graph wiener
  W(G) = 9
  reachable pairs = 6
  avg path length = 1.500
  nodes=3
```

示例输出（空图）：
```
 graph wiener
  wiener: undefined (empty graph)
```

示例输出（有向链 A→B→C）：
```
 graph wiener
  W(G) = 4
  reachable pairs = 3
  avg path length = 1.333
  nodes=3
```

---

## VectorAddress 命名空间扩展

VectorAddress L4=46 分配给 gos-graph-wiener-harness（测试隔离用）

完整 L4 命名空间：
```
29=node-attr, 32=pal-boot, 33=pal-render, 34=node-attr-list, 35=graph-density,
36=node-attr-list-u8, 37=graph-clustering, 38=pal-full, 39=graph-transitivity,
40=graph-kcore, 41=graph-assortativity, 42=graph-reciprocity, 43=graph-modularity,
44=graph-rich-club, 45=graph-girth, 46=graph-wiener
```

---

## 修改文件 / Changed files

| 文件 | 修改内容 |
|------|----------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_wiener_inner()` 方法 + 公共 `graph_wiener()` 函数 |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_wiener()` shell 显示函数（含整数小数点格式化） |
| `crates/k-shell/src/proc.rs` | 新增路由 `graph wiener` / `gwiener` + help 文本 |
| `host-tests/gos-graph-wiener-harness/Cargo.toml` | 新建测试 harness |
| `host-tests/gos-graph-wiener-harness/.cargo/config.toml` | host 目标覆盖 (x86_64-pc-windows-msvc) |
| `host-tests/gos-graph-wiener-harness/tests/graph_wiener.rs` | 10 个测试 |

---

## 测试矩阵 / Test matrix (10 tests, all passing)

| # | 场景 | W(G) | reachable_pairs | node_count |
|---|------|------|-----------------|------------|
| 1 | 空图 | 0 | 0 | 0 |
| 2 | 单节点无边 | 0 | 0 | 1 |
| 3 | 两个孤立节点 | 0 | 0 | 2 |
| 4 | A→B（单边）| 1 | 1 | 2 |
| 5 | 有向链 A→B→C | 4 | 3 | 3 |
| 6 | 三角 A→B→C→A | 9 | 6 | 3 |
| 7 | 完全有向 K3（6条边）| 6 | 6 | 3 |
| 8 | 单节点自环 A→A | 0 | 0 | 1 |
| 9 | 断连：A→B + 孤立 C | 1 | 1 | 3 |
| 10 | 互惠对 A↔B + 链 C→D | 3 | 3 | 4 |

**关键测试用例数学验证：**
- 测试 5（链 ABC）：d(A,B)=1, d(A,C)=2, d(B,C)=1 → W=4 ✓
- 测试 6（三角）：6 条方向距离 = {1,2,1,2,1,2} → W=9 ✓
- 测试 7（K3）：全部 6 对距离=1 → W=6 ✓
- 测试 8（自环）：dist[A]=0 已设置，自环边不引入新节点 → W=0 ✓
- 测试 10（互惠对+链）：d(A,B)=1, d(B,A)=1, d(C,D)=1 → W=3 ✓

---

## 核心指标全貌 / Complete metric set (through V2.70)

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
| 环结构 | 围长 (shortest directed cycle) | V2.69 |
| 路径代价 | Wiener 指数 (sum of pairwise distances) | **V2.70** |
