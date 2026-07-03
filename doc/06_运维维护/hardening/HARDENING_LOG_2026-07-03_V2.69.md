# HARDENING LOG — V2.69: graph girth (shortest directed cycle length)

**Date:** 2026-07-03  
**Version:** V2.69  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.69 新增 `graph_girth()` API，计算有向图的围长（girth）——图中最短有向环的长度。
围长是图论中最基本的结构参数之一，直接度量图中是否存在反馈环路以及最短环路的规模。
纯 BFS 实现，无浮点，no_std 安全。

V2.69 adds `graph_girth()` to gos_runtime, computing the **girth** of the directed graph —
the length of its shortest directed cycle.  Girth is a fundamental structural parameter
that directly measures whether feedback loops exist in the kernel graph and how small the
tightest cycle is.  Pure BFS implementation, no floating point, no_std safe.

Shell: `graph girth` / `ggirth`

---

## 背景 / Background

### 图论定义

**围长（Girth）** 是图中最短环路的边数：

| 场景                         | 围长值     |
|------------------------------|-----------|
| 自环 A→A                     | 1         |
| 互惠对 A↔B（两条有向边）        | 2         |
| 有向三角 A→B→C→A              | 3         |
| 有向 k-环                     | k         |
| 无环有向图（DAG）              | u32::MAX  |

对于无向图，围长通常从 3 开始（无多重边时）。对于有向图，允许 girth = 1（自环）或 2（互惠对）。

### 内核意义

在 GOS 内核图中，围长具有直接的系统含义：
- **girth = u32::MAX（DAG）**：内核子系统依赖关系无环，可安全拓扑排序；
- **girth = 1（自环）**：某子系统向自身发送信号（反应式循环可能产生无限信号风暴）；
- **girth = 2（互惠对）**：两个子系统相互依赖/互发信号（ping-pong 模式）；
- **girth = k（k 环）**：存在长度为 k 的依赖循环，可能产生死锁或震荡。

配合已有的 SCC（V2.34）和拓扑排序（已有 `graph_toposort`），
三者共同刻画内核图的有向结构健康度：
- `graph_toposort` → 无环检测 + 拓扑序
- `graph_scc` → 强连通分量识别
- `graph_girth` → 最短反馈环定量

---

## 算法 / Algorithm

```
对每个源节点 s 做 BFS：
  dist[s] = 0，入队
  处理节点 cur (dist = d)：
    若 d+1 ≥ min_girth → 剪枝（不可能找到更短的环）
    对每条有向出边 cur→nbr：
      若 nbr == s → 找到环，长度 d+1，更新 min_girth
      否则若 nbr 未访问且 d+1 < min_girth → 入队

返回 min_girth（= u32::MAX 表示无环）
```

剪枝策略：
- 当 `min_girth == 1`（自环）时立即终止全局搜索
- 当 BFS 前沿的 `dist+1 ≥ min_girth` 时跳过该节点（最关键的优化）

复杂度：O(V × (V + E))，对于 V ≤ 128、E ≤ 512 完全可接受。

### 自环的处理

自环 A→A 在 BFS 中通过以下路径检测：
- 处理节点 A（dist=0），扫描出边 A→A
- nbr_id == s_id（均为 A），cycle = 0+1 = 1 ✓

### 互惠对的处理

互惠对 A↔B（edges A→B, B→A）：
- BFS 从 A 出发，访问 B（dist=1），扫描出边 B→A
- nbr_id = A = s_id，cycle = 1+1 = 2 ✓

---

## 返回值 / Return values

```rust
pub fn graph_girth() -> (u32, bool, usize)
// (girth, is_acyclic, node_count)
```

| 字段           | 含义                                      |
|----------------|-------------------------------------------|
| `girth`        | 最短有向环的边数；`u32::MAX` 表示无环      |
| `is_acyclic`   | `true` 当且仅当 `girth == u32::MAX`（DAG）|
| `node_count`   | 活跃节点总数                               |

### 边界情况 / Edge cases

| 场景               | girth     | is_acyclic | node_count |
|--------------------|-----------|------------|------------|
| 空图               | u32::MAX  | true       | 0          |
| 孤立节点（无边）   | u32::MAX  | true       | n          |
| DAG（线性链等）    | u32::MAX  | true       | n          |
| 自环               | 1         | false      | 1+         |
| 互惠对 A↔B         | 2         | false      | 2+         |
| 有向三角           | 3         | false      | 3+         |
| C4 单向环          | 4         | false      | 4          |

---

## Shell 命令 / Shell command

```
graph girth   → 计算并显示围长
ggirth        → 别名
```

示例输出（有向三角 A→B→C→A）：
```
 graph girth
  girth: 3  (shortest directed cycle)
  nodes=3
```

示例输出（DAG）：
```
 graph girth
  girth: acyclic (no directed cycle)
  nodes=4
```

示例输出（自环）：
```
 graph girth
  girth: 1  (self-loop)
  nodes=1
```

---

## VectorAddress 命名空间扩展

VectorAddress L4=45 分配给 gos-graph-girth-harness（测试隔离用）

完整 L4 命名空间：
```
29=node-attr, 32=pal-boot, 33=pal-render, 34=node-attr-list, 35=graph-density,
36=node-attr-list-u8, 37=graph-clustering, 38=pal-full, 39=graph-transitivity,
40=graph-kcore, 41=graph-assortativity, 42=graph-reciprocity, 43=graph-modularity,
44=graph-rich-club, 45=graph-girth
```

---

## 修改文件 / Changed files

| 文件 | 修改内容 |
|------|----------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_girth_inner()` 方法 + 公共 `graph_girth()` 函数 |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_girth()` shell 显示函数 |
| `crates/k-shell/src/proc.rs` | 新增路由 `graph girth` / `ggirth` + help 文本 |
| `host-tests/gos-graph-girth-harness/Cargo.toml` | 新建测试 harness |
| `host-tests/gos-graph-girth-harness/.cargo/config.toml` | host 目标覆盖 (x86_64-pc-windows-msvc) |
| `host-tests/gos-graph-girth-harness/tests/graph_girth.rs` | 10 个测试 |

---

## 测试矩阵 / Test matrix (10 tests, all passing)

| # | 场景 | girth | is_acyclic | node_count |
|---|------|-------|------------|------------|
| 1 | 空图 | u32::MAX | true | 0 |
| 2 | 3 个孤立节点（无边）| u32::MAX | true | 3 |
| 3 | 线性链 A→B→C（DAG）| u32::MAX | true | 3 |
| 4 | 自环 A→A | 1 | false | 1 |
| 5 | 链 A→B→C + 自环 A→A | 1 | false | 3 |
| 6 | 互惠对 A↔B | 2 | false | 2 |
| 7 | 三角 A→B→C→A | 3 | false | 3 |
| 8 | 三角 + 额外节点 D→A | 3 | false | 4 |
| 9 | C4 单向环 A→B→C→D→A | 4 | false | 4 |
| 10 | 三角 + 互惠对 D↔E（不同连通分量）| 2 | false | 5 |

**关键测试用例数学验证：**
- 测试 4（自环）：BFS 从 A 出发，edge A→A，nbr=A=s，cycle=0+1=1 ✓
- 测试 6（互惠对）：BFS 从 A 出发，访问 B(1)，edge B→A，nbr=A=s，cycle=1+1=2 ✓
- 测试 7（三角）：BFS 从 A，访问 B(1)→C(2)，edge C→A，cycle=2+1=3 ✓
- 测试 9（C4）：BFS 从 A，访问 B(1)→C(2)→D(3)，edge D→A，cycle=3+1=4 ✓
- 测试 10（三角+互惠对）：min(3, 2) = 2 ✓（互惠对更短）

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
| 环结构 | 围长 (shortest directed cycle) | V2.69 |
