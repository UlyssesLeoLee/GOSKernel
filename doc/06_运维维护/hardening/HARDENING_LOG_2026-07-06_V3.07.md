# 硬化日志 V3.07 — 顶点连通度 κ(G)

**日期**：2026-07-06  
**分支**：feat/vk-auto-live-surface  
**提交**：3fd566f  
**先前基线**：V3.06（边介数中心性，1033 个宿主测试）  
**新总计**：1043 个宿主测试（+10）

---

## 算法：顶点连通度 κ(G)

图 G 的**顶点连通度** κ(G) 是使 G 断连（或变为平凡图）所需移除的最少顶点数量。它是图论中最基本的健壮性度量之一。

### 理论背景

**Whitney 定理（1932）：**

```
κ(G) ≤ κ'(G) ≤ δ(G)
```

其中 κ'(G) 是边连通度，δ(G) 是最小度数。这提供了一种天然的交叉验证：κ 永远不会超过最小度数。

**Menger 定理（1927）：** κ(G) 等于任意两个不相邻顶点之间内部顶点不相交路径的最大数量（在存在不相邻顶点的前提下）。

**Even 算法（1975）：** 固定 s = argmin(度数)。则：
- 若 G 不连通，κ(G) = 0
- 若 G = Kₙ（完全图），κ(G) = n−1
- κ(G) = 对所有与 s 不相邻的 t，取 max-顶点不相交路径数(s, t) 的最小值

关键洞察：固定最小度顶点 s 就已经足够。任何顶点分割集都必须包含 s 的所有邻居（共 δ(G) 个），因此只需检查非邻居即可。

### 节点拆分网络变换

为了通过最大流计算顶点不相交路径，每个内部顶点 ci（≠ s，≠ t）被拆分为两个虚拟节点：
- ci_in  = 2·ci
- ci_out = 2·ci + 1
- 内部边 ci_in → ci_out，容量为 1（用以强制顶点不相交性）

跨边（从每条原始边 ci–cj 对应的 ci_out 到 cj_in）容量为 INF（=127，因为最大流 ≤ δ ≤ 126）。

源点 s 和汇点 t：各自只有一个虚拟节点（s_out = 2·s+1，t_in = 2·t），不受内部容量约束。

### 实现细节

**常量**（no_std，纯栈分配）：
- ME = 2560（边槽位数），MV = 256（虚拟节点槽位数）  
- MAX_NODES=128 → 至多 256 个虚拟节点，理论上 ≤2·128·128=32768 条跨边，但实际受限于真实边数
- 实际最坏情形：2·MAX_EDGES 条跨边 + 2·MAX_NODES 条内部边 ≤ 2·512 + 256 = 1280 < 2560 ✓

**栈占用**：ef[2560] + et[2560] + ec[2560]（u8 数组）+ BFS 数组 ≈ 8 KB，远低于 16 KB 限制。

**ei^1 反向边技巧**：正向边位于偶数索引（ne 从 0 开始，每次递增 2），因此 `ef[ei^1]` 始终是 `ef[ei]` 的反向边。无需 HashMap。

**Edmonds-Karp BFS**：单位容量的内部边意味着每条增广路径恰好为流量增加 1。总增广轮数 ≤ κ ≤ δ ≤ 127。

### 新增 API

```rust
// gos-runtime/src/lib.rs
pub fn graph_vertex_connectivity<const N: usize>(
) -> ([VectorAddress; N], usize, u32, u32)
// 返回：（排序后的节点地址，节点数，κ 值，最小度数）
```

### K-Shell 集成

- **命令别名**：`graph kappa`、`gkappa`、`vertex connectivity`、`vertex conn`、`gvertconn`、`graph vertex conn`、`graph vconn`
- **显示**：亮青色标题（颜色 11），亮绿色节点列表（颜色 10）
- **页脚**：`κ(G)=N  δ(G)=M  Whitney: κ≤δ  Menger 1927`

---

## 新增测试装置：gos-graph-vconn-harness

**位置**：`host-tests/gos-graph-vconn-harness/`  
**L4 命名空间**：83  
**插件**：`KL_GRAPH_VCON_H`  
**执行器**：`vconn.exec`

### 测试用例（共 10 个）

| # | 图 | n | 期望 κ | 说明 |
|---|-------|---|------------|-------|
| 01 | 空图 | 0 | 0 | 无节点 → 不连通 |
| 02 | 单节点 | 1 | 0 | 单节点 → 平凡图 |
| 03 | K₂ | 2 | 1 | 一条边，移除任一顶点都会断连 |
| 04 | 路径 A–B–C | 3 | 1 | B 是割点 |
| 05 | C₄（4 环） | 4 | 2 | 必须移除 2 个顶点才能断连 |
| 06 | K₄ | 4 | 3 | 完全图：κ = n−1 |
| 07 | 星形 K₁,₄ | 5 | 1 | 移除中心会使所有叶子断连 |
| 08 | 沙漏形 | 5 | 1 | 两个三角形共享顶点 A；A 是割点 |
| 09 | 不连通图 | 4 | 0 | 两条孤立边：本已不连通 |
| 10 | K₃,₃ | 6 | 3 | 完全二部图；Whitney 紧界：κ = δ = 3 |

全部 10 个测试通过，退出码 0。

---

## 修改的文件

| 文件 | 变更 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_vertex_connectivity_inner`、`vertex_conn_maxflow`、`graph_vertex_connectivity` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_vertex_connectivity` |
| `crates/k-shell/src/proc.rs` | 新增 `graph kappa` / `gkappa` / `graph vconn` 等命令的路由 |
| `host-tests/gos-graph-vconn-harness/` | 新增测试装置（Cargo.toml、.cargo/config.toml、tests/graph_vconn.rs） |

---

## 操作系统类比

顶点连通度映射到操作系统设计中的**容错能力**：

- κ=0 的内核已经处于分裂状态——组件之间无法通信。
- κ=1 意味着存在单一的关键组件（例如单一的调度器或内存分配器），一旦失效便会导致系统隔离。
- κ 值更高的图代表冗余、具韧性的架构——类似于多路径 I/O、RAID 或复制状态机。

对 GOSKernel 运行时图计算 κ(G)，即可即时得到当前拓扑结构的健壮性评分。
