# GOS 硬化日志 — V2.98（2026-07-06）

## 功能：最小支配集（贪心 ln(Δ)+1 近似算法）

### 变更摘要

在 gos-runtime 中新增 `graph_dominating_set<N>()`——内核中第一个图监控布点（monitoring-placement）度量。支配集 D ⊆ V 满足：每个不在 D 中的节点至少有一个邻居在 D 中。γ(G) = |D_min|。

贪心算法可达到多项式时间近似的最优比率：≤ H(Δ)+1 ≈ ln(Δ)+1（其中 Δ 为最大度），与 NP-难下界（Johnson 1974）相匹配。

### 操作系统类比

最小监控部署：在尽可能少的子系统上放置健康监视器，使得每个未插桩的模块都直接与至少一个已插桩的邻居相邻。等价于生产网络中经典的设施选址/覆盖问题。

至此完成"覆盖三件套"：
- τ(G) 顶点覆盖（V2.97）：每条边至少触及 1 个被覆盖节点（IPC 审计检查点）
- α(G) 独立集（V2.96）：无共享边的最大集合（并行启动前沿）
- γ(G) 支配集（V2.98）：每个非监控节点都有一个监控邻居（遥测网络）

### 实现细节

**gos-runtime/src/lib.rs**
- 新方法：`GraphRuntime::graph_dominating_set_inner<N>()`
  - 步骤 1：压缩活跃节点槽位（与顶点覆盖相同模式）
  - 步骤 2：槽位→紧凑索引映射，用于位掩码运算
  - 步骤 3：构建 `dominated[ci]` = {ci} ∪ 无向邻居，表示为 u128 位掩码
  - 步骤 4：贪心循环——选取 `(dominated[ci] & undominated).count_ones()` 最大的节点
  - 步骤 5：收集结果并按 `vector.as_u64()` 升序插入排序
- 新公开函数：`graph_dominating_set<N>() -> ([VectorAddress; N], usize, usize)`
  - 返回 `(dom_vecs, dom_size, node_count)`
  - `dom_vecs[0..dom_size]` = 按 `as_u64()` 升序排列的支配集

**crates/k-shell/src/lib.rs**
- 新增 `dispatch_graph_dominating_set()`：
  - 亮黄色表头（颜色 14），亮青色成员（颜色 11）
  - 页脚：`γ(G)=N  greedy ≤ ln(Δ)+1 approx`

**crates/k-shell/src/proc.rs**
- 在"graph vertex cover"之后新增路由：
  `"graph domset" || "gdomset" || "dominating set" || "graph dominating set" || "gdominate" || "min domset"`

**host-tests/gos-graph-domset-harness/**
- VectorAddress L4=74
- 10 个测试，全部通过：
  1. 空图 → γ=0，node_count=0
  2. 单个孤立节点 → γ=1（必须包含自身）
  3. 两个孤立节点 → γ=2（无相互覆盖）
  4. K_2 边 → γ=1（一个端点覆盖两者）
  5. 三角形 K_3 → γ=1（任意节点覆盖全部）
  6. 路径 P_4 → γ=2（有效性：每个节点均被覆盖）
  7. 星形 K_{1,4} → γ=1（中心节点总是被优先选中，覆盖全部 5 个节点）
  8. 完全图 K_4 → γ=1（任意节点覆盖全部 4 个）
  9. 混合图（2 个孤立节点 + 1 条边）→ γ=3（孤立×2 + 一条边的端点）
  10. γ ≤ τ 交叉验证 K_3 → dom_size=1 ≤ cover_size=2

### 关键不变量

| 不变量 | 值 | 说明 |
|-----------|-------|-------|
| K_n 支配 | γ=1 | 任意节点支配全部 |
| 星形 K_{1,k} | γ=1 | 中心节点支配全部 |
| 路径 P_n | γ=⌈n/3⌉ | 贪心可达最优 |
| 孤立节点 | 被强制纳入 D | 只有自身能覆盖自己 |
| γ(G) ≤ τ(G) | 恒成立 | 任意顶点覆盖都是支配集 |
| 近似比 | ≤ H(Δ)+1 | 多项式时间最优保证（Johnson 1974） |

### 文献

- Ore 1962：支配数概念的提出
- Johnson 1974：贪心 ln(n)+1 近似算法，NP-难下界
- Garey & Johnson 1979：最小支配集的 NP-完全性
- Hedetniemi & Laskar 1990：图支配理论综述

### 宿主测试套件总计

**953 个测试**（此前 943 个，+10 来自 gos-graph-domset-harness）

### VectorAddress L4 命名空间更新

```
73=graph-vc (V2.97)
74=graph-domset (V2.98，新增)
```
