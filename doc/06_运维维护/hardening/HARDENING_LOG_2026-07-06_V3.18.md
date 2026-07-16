# GOSKernel 硬化日志 — V3.18
**日期：** 2026-07-06
**分支：** feat/vk-auto-live-surface
**提交：** 89f7342
**宿主测试套件总计：** 1153 个测试（全部通过）

---

## 摘要

V3.18 在 GOSKernel 中引入了首批**基于距离的拓扑指数**，跨入了新的算法类别。此前所有拓扑指数（V3.12–V3.17）都使用纯粹的度数扫描 O(m) 算法。V3.18 则需要 **BFS 全源最短路径** O(n·(n+m))，产出三个具有直接操作系统类比意义的经典分子图指数。

---

## 新功能：`graph topo7` — Wiener W + Harary H + 超-Wiener WW

### API

```rust
pub fn graph_topo_indices7() -> (u64, u64, u64, usize, usize)
// 返回值：(wiener, harary_ppm, hyper_wiener, edge_count, node_count)
```

### 指数说明

| 指数 | 公式 | 类型 | 文献 |
|-------|---------|------|-----------|
| W  | Σ_{u<v} d(u,v) | 精确 u64 | Wiener 1947 |
| H  | Σ_{u<v} 1/d(u,v) × 10⁶ | 向下取整 ppm | Plavšić et al. 1993 |
| WW | (1/2) Σ_{u<v} [d + d²] = Σ d(d+1)/2 | 精确 u64 | Klein & Randić 1993 |

**不连通节点对：** d=∞ → 对三个指数均贡献 0。

### 算法

对无向投影图从每个源节点执行 BFS，复杂度 O(n·(n+m))。
仅使用整数运算：无浮点数、无堆分配。
BFS 使用栈上分配的 `dist[MAX_NODES: u8]` 和 `queue[MAX_NODES: u8]`（INF=255，对 128 节点图最大 BFS 深度=126）。
Harary：每个连通节点对贡献 `1_000_000 / d`（向下取整）。
超-Wiener：每对贡献 `d * (d + 1) / 2`（恒为整数：对所有 d≥1，d*(d+1) 为偶数）。

### 关键不变量

```
W(K_n)  = H_ppm/10^6 = WW(K_n) = n*(n-1)/2   （所有节点对 d=1）
W(P_n)  = n*(n²-1)/6                            （路径公式；P₃=4，P₄=10）
WW ≥ W  恒成立（当且仅当图为完全图时取等号）
H ≥ W   在 ppm 意义下恒成立
不连通图：W=H=WW=0
```

### 交叉校验表（解析计算）

| 图    | W   | H_ppm     | WW  | 边数 | 节点数 |
|----------|-----|-----------|-----|-------|-------|
| 空图    | 0   | 0         | 0   | 0     | 0     |
| 单节点   | 0   | 0         | 0   | 0     | 1     |
| 边 A-B | 1   | 1_000_000 | 1   | 1     | 2     |
| P₃       | 4   | 2_500_000 | 5   | 2     | 3     |
| K₃       | 3   | 3_000_000 | 3   | 3     | 3     |
| K_{1,4}  | 16  | 7_000_000 | 22  | 4     | 5     |
| P₄       | 10  | 4_333_333 | 15  | 3     | 4     |
| K₄       | 6   | 6_000_000 | 6   | 6     | 4     |
| 2 个孤立节点  | 0   | 0         | 0   | 0     | 2     |
| K_{2,3}  | 14  | 8_000_000 | 18  | 6     | 5     |

### Shell 命令

```
graph topo7   gtopo7   wiener index   gwiener
harary index  gharary  hyper wiener   ghyperw   gwienerhw
```

### 操作系统类比

- **W（Wiener）：** 内核依赖图中的总消息路由开销。最小化 W 即最小化所有子系统对之间的平均 IPC 跳数成本。
- **H（Harary）：** 综合连接性得分——距离越近的子系统贡献越大。H 越高表示内核耦合越高效。对于全连通网状结构 H 趋于最大值。
- **WW（超-Wiener）：** 二次延迟惩罚项。放大长距离依赖关系（d² 项）的影响。可用于识别少数远距离模块对主导整体路由成本的内核结构。

### 显示格式

- 亮黄色标题：`graph topo7  (W + H + WW distance-based indices)`
- W：亮青色（精确值标注）
- H：亮绿色（ppm 小数显示）
- WW：亮洋红色（精确值标注；当 wiener=0 且 node_count>1 时显示不连通标注）
- 页脚：`N node(s)  M edge(s)  Wiener 1947  Plavšić et al. 1993  Klein & Randić 1993`

---

## 测试框架：`gos-graph-topo7-harness`

**位置：** `host-tests/gos-graph-topo7-harness/`
**VectorAddress L4：** 94
**插件 ID：** `TOPO_IX7`

10 个测试，全部通过：

1. 空图 → (0, 0, 0, 0, 0)
2. 单节点 → (0, 0, 0, 0, 1)
3. 单边 A→B → (1, 1_000_000, 1, 1, 2)
4. 路径 P₃ → (4, 2_500_000, 5, 2, 3)
5. 三角形 K₃ → (3, 3_000_000, 3, 3, 3)
6. 星形 K_{1,4} → (16, 7_000_000, 22, 4, 5)
7. 路径 P₄ → (10, 4_333_333, 15, 3, 4)
8. 完全图 K₄ → (6, 6_000_000, 6, 6, 4)
9. 两个孤立节点 → (0, 0, 0, 0, 2)
10. K_{2,3} 交叉校验 → (14, 8_000_000, 18, 6, 5)

---

## VectorAddress L4 命名空间（已更新）

| L4 | 测试框架 |
|----|---------|
| 88 | graph-topo（SC/GA/AZI） |
| 89 | graph-topo2（H/ABC/F） |
| 90 | graph-topo3（SDD/ISI/NI） |
| 91 | graph-topo4（Sombor/RM2/sigma） |
| 92 | graph-topo5（HM1/HM2/AG） |
| 93 | graph-topo6（EM1/ABS/RRR） |
| **94** | **graph-topo7（W/H/WW）** |

---

## 变更文件

| 文件 | 变更 |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices7_inner()` 及 `graph_topo_indices7()` 导出 |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices7()` |
| `crates/k-shell/src/proc.rs` | 新增 topo7 的 shell 路由 |
| `host-tests/gos-graph-topo7-harness/` | 新测试框架（4 个文件） |

---

## 度量指标

- **新增函数数：** 2（内部实现 + 公开导出）
- **新增 shell 别名数：** 9
- **新增测试数：** 10
- **累计宿主测试数：** 1153
- **算法类别：** 首个基于距离的指数（需要 BFS，而非单纯度数扫描）
</content>
