# GOS 硬化日志 — V3.26

**日期**: 2026-07-08
**分支**: feat/vk-auto-live-surface
**会话**: 自动化定时硬化任务（每 2 小时一次）

---

## 摘要

V3.26 新增三个**跳跃 Zagreb 指数**（Leap Zagreb indices）—— LM₁、LM₂、LM₃ —— 以及一个新的 10 项测试宿主套件（`gos-graph-topo15-harness`），同时补全了 shell 路由层（`k-shell`）对新命令的分发。

跳跃 Zagreb 指数由 Naji、Soner 与 Gutman（2017）提出，以顶点的**2-距离度**（2-distance degree）d₂(v) 为核心量——即图中与 v 距离恰好为 2 的顶点个数。它们是经典 Zagreb 指数的高阶推广，对图的整体"双跳连通性"进行量化。

**宿主测试套件总计：1233 个测试**（V3.25 累计的 1223 个 + 新增 10 个）。

---

## 新功能: `graph_topo_indices15()` — LM₁ + LM₂ + LM₃

### API

```rust
pub fn graph_topo_indices15() -> (u64, u64, u64, usize, usize)
//                                lm1  lm2  lm3   edges  nodes
```

### 定义

- **LM₁(G) = Σ_v d₂(v)²** —— 第一跳跃 Zagreb 指数（精确 u64；Naji et al. 2017）
- **LM₂(G) = Σ_{uv∈E} d₂(u)·d₂(v)** —— 第二跳跃 Zagreb 指数（精确 u64）
- **LM₃(G) = Σ_{uv∈E} (d₂(u)+d₂(v))** —— 第三跳跃 Zagreb 指数（精确 u64）

其中：
- `d₂(v)` = |{w : d(v,w) = 2}| = 顶点 v 的 2-距离度
- `d(v,w)` = 无向投影上的 BFS 最短路距离

### 关键不变量

- **完全图 K_n**：所有顶点对均直接相邻（距离 = 1），无距离为 2 的节点，`d₂(v) = 0`，故 LM₁ = LM₂ = LM₃ = 0
- **星图 K_{1,k}**：中心节点 d₂ = 0（所有叶节点均在距离 1 处）；每个叶节点 d₂ = k-1（其余 k-1 个叶节点在距离 2 处）
  - LM₁(K_{1,k}) = k·(k-1)²
  - LM₂(K_{1,k}) = 0（每条边一端为中心，d₂ = 0）
  - LM₃(K_{1,k}) = k·(k-1)
- **路径 P₄**：所有顶点 d₂ = 1（A→C、B→D），故 LM₁ = 4、LM₂ = 3、LM₃ = 6

### 分析对照表

| 图 | LM₁ | LM₂ | LM₃ | 边 | 节点 |
|------|-----|-----|-----|----|------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| 边 A-B | 0 | 0 | 0 | 1 | 2 |
| 路径 P₃ | 2 | 0 | 2 | 2 | 3 |
| 三角形 K₃ | 0 | 0 | 0 | 3 | 3 |
| 星图 K_{1,4} | 36 | 0 | 12 | 4 | 5 |
| 路径 P₄ | 4 | 3 | 6 | 3 | 4 |
| 完全图 K₄ | 0 | 0 | 0 | 6 | 4 |
| 二分图 K_{2,3} | 14 | 12 | 18 | 6 | 5 |

### 算法

1. 构建无向邻接位掩码：O(E)
2. 从每个源节点做 BFS，统计距离恰好为 2 的节点数得 d₂[]：O(n·(n+m))
3. 节点扫描：LM₁ = Σ d₂(v)²
4. 边扫描（a < b）：LM₂ = Σ d₂(a)·d₂(b)；LM₃ = Σ (d₂(a)+d₂(b))

全程无浮点、无堆内存分配，no_std 安全。

### 栈内存占用

- adj[128]（u128 × 128 = 2 KB）
- dist[128]（u8 × 128 = 128 B）
- queue[128]（u8 × 128 = 128 B）
- d2[128]（u32 × 128 = 512 B）
- 总计 ≈ 2.8 KB

---

## Shell 集成

### proc.rs 路由

```
"graph topo15" | "gtopo15" | "leap zagreb" | "gleapzagreb"
| "lm1 index" | "glm1" | "lm2 index" | "glm2"
| "lm3 index" | "glm3" | "glm1lm2lm3"
→ dispatch_graph_topo_indices15(sink)
```

### 显示格式

```
 graph topo15 (LM₁ + LM₂ + LM₃ Leap Zagreb indices)
 ───────────────────────────────────────────────────────────
  first leap zagreb    LM₁ =  <exact>  [Σ_v d₂(v)²]  (exact)
  second leap zagreb   LM₂ =  <exact>  [Σ_{uv∈E} d₂(u)·d₂(v)]  (exact)
  third leap zagreb    LM₃ =  <exact>  [Σ_{uv∈E} d₂(u)+d₂(v)]  (exact)
 ───────────────────────────────────────────────────────────
N node(s)  M edge(s)  Naji, Soner & Gutman 2017
```

- LM₁ 以亮青色显示；LM₂ 以亮绿色显示；LM₃ 以亮洋红色显示
- 当 LM₁=0 且有边时，注释"d₂=0: complete graph"

---

## 操作系统类比

- **LM₁（第一跳跃 Zagreb）**：二跳路由压力——节点 v 的 d₂(v) 代表可通过恰好两跳到达的模块数量；LM₁ 量化整个系统的二跳覆盖范围平方和。高 LM₁ = 系统在"间接邻居层"有大量连接压力。
- **LM₂（第二跳跃 Zagreb）**：二跳通道协同负载——同一 IPC 通道两端节点的 d₂ 乘积之和。高 LM₂ = 跨通道的二跳路由热点。
- **LM₃（第三跳跃 Zagreb）**：二跳度总和——每条 IPC 信道两端二跳度之和；量化系统整体的间接连通性密度。

---

## VectorAddress 命名空间

- L4=102 分配给 `gos-graph-topo15-harness`

**完整 L4 命名空间（更新后）：**

```
88=graph-topo    89=graph-topo2   90=graph-topo3   91=graph-topo4
92=graph-topo5   93=graph-topo6   94=graph-topo7   95=graph-topo8
96=graph-topo9   97=graph-topo10  98=graph-topo11  99=graph-topo12
100=graph-topo13  101=graph-topo14  102=graph-topo15
```

---

## 测试详情

### gos-graph-topo15-harness（10 测试）

| # | 测试场景 | 预期 (lm1, lm2, lm3) | 结果 |
|---|---------|----------------------|------|
| 1 | 空图 | (0, 0, 0) | ✅ |
| 2 | 单孤立节点 | (0, 0, 0) | ✅ |
| 3 | 单有向边 A→B | (0, 0, 0) | ✅ |
| 4 | 路径 P₃ | (2, 0, 2) | ✅ |
| 5 | 三角形 K₃ | (0, 0, 0) | ✅ |
| 6 | 星图 K_{1,4} | (36, 0, 12) | ✅ |
| 7 | 路径 P₄ | (4, 3, 6) | ✅ |
| 8 | 完全图 K₄ | (0, 0, 0) | ✅ |
| 9 | 两孤立节点 | (0, 0, 0) | ✅ |
| 10 | 二分图 K_{2,3} | (14, 12, 18) | ✅ |

**10/10 全部通过**。

---

## 文献

- Naji, A. M., Soner, N. D., & Gutman, I. (2017). On leap Zagreb indices of graphs. *Communication in Combinatorics and Optimization*, 2(2), 99–117.
