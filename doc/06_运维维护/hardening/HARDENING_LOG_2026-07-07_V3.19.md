# GOSKernel 硬化日志 — V3.19
**日期:** 2026-07-07
**分支:** feat/vk-auto-live-surface
**宿主测试套件总计:** 1163 个测试（全部通过）

---

## 摘要

V3.19 引入了**基于离心率（eccentricity）的拓扑指数** —— 是 V3.18 基于距离（Wiener/Harary/Hyper-Wiener）指数的天然算法延续。全部四个指标都由同一次产生逐节点离心率的 BFS 遍历计算得出，为图的"紧凑性"和"极值中心性"提供了更丰富的结构画像。

---

## 新功能: `graph topo8` — ECI + D + R + 平均离心率

### API

```rust
pub fn graph_topo_indices8() -> (u64, u64, u32, u32, usize, usize)
// 返回: (eci, avg_ecc_ppm, diameter, radius, edge_count, node_count)
```

说明：返回一个 6 元组（与 `graph_zagreb` 的模式一致），因为直径（diameter）和半径（radius）是各自独立的结构量。

### 指数

| 符号 | 公式 | 类型 | 文献 |
|--------|---------|------|-----------|
| ξ (ECI) | Σ_v deg(v) × ecc(v) | 精确 u64 | Sharma, Goswami & Madan 1997 |
| avg_ecc | (Σ_v ecc(v)) / n × 10⁶ | 向下取整 ppm | Buckley & Harary 1990 |
| D | max_{v} ecc(v) | 精确 u32 | 经典图论 |
| R | min{ecc(v) \| ecc(v) > 0} | 精确 u32 | 经典图论 |

**ecc(v)：** 从 v 到任意可达节点的最大 BFS 距离（对孤立节点或单节点图为 0）。
**非连通节点：** ecc = 0；对 ECI 和 avg_ecc 的贡献为 0。D 反映可达的最大离心率；R 反映最小的正离心率（若无连通节点对则为 0）。

### 算法

对无向投影图从每个节点做一次 BFS，复杂度 O(n·(n+m))。
仅使用整数运算：无浮点数，无堆分配。
栈上分配：`dist[MAX_NODES: u8]`、`queue[MAX_NODES: u8]`、`ecc[MAX_NODES: u32]`、`deg[MAX_NODES: u32]`。
每个源节点一次 BFS 遍历即可计算出 ecc[src]；随后对 ecc[] 数组做第二次扫描，累加得到 ECI、avg_ecc_sum、D、R。

### 关键不变量

```
完全图 K_n:      D=R=1（自中心图）；ECI=n*(n-1)；avg_ecc_ppm=1_000_000
路径 P_n:        D=n-1；R=⌈(n-1)/2⌉；端点 ecc=n-1，中心点 ecc=⌈(n-1)/2⌉
星图 K_{1,k}:    D=2（叶节点），R=1（中心）；ECI=k+2k=3k
自中心图:        D=R（例如 K_n、完全二部图 K_{m,n}）
全孤立节点:      ECI=0；avg=0；D=0；R=0
正则性线索:      对于 Δ-正则、直径为 1 的（完全）图，ECI = n * Δ * D
```

### 交叉验证表（解析计算）

| 图    | ECI | avg_ecc_ppm | D | R | 边数 | 节点数 |
|----------|-----|-------------|---|---|-------|-------|
| 空图    | 0   | 0           | 0 | 0 | 0     | 0     |
| 单节点   | 0   | 0           | 0 | 0 | 0     | 1     |
| 边 A-B | 2   | 1_000_000   | 1 | 1 | 1     | 2     |
| P₃       | 6   | 1_666_666   | 2 | 1 | 2     | 3     |
| K₃       | 6   | 1_000_000   | 1 | 1 | 3     | 3     |
| K_{1,4}  | 12  | 1_800_000   | 2 | 1 | 4     | 5     |
| P₄       | 14  | 2_500_000   | 3 | 2 | 3     | 4     |
| K₄       | 12  | 1_000_000   | 1 | 1 | 6     | 4     |
| 2 孤立点  | 0   | 0           | 0 | 0 | 0     | 2     |
| K_{2,3}  | 24  | 2_000_000   | 2 | 2 | 6     | 5     |

**推导过程：**
- P₃ (A-B-C)：ecc(A)=2, ecc(B)=1, ecc(C)=2；deg(A)=deg(C)=1, deg(B)=2 → ECI=1×2+2×1+1×2=6；avg=5/3→1_666_666
- K_{1,4}：中心 ecc=1 deg=4；4 个叶节点 ecc=2 deg=1 → ECI=4+8=12；avg=9/5=1.8→1_800_000
- P₄：ecc={3,2,2,3}, deg={1,2,2,1} → ECI=3+4+4+3=14；avg=10/4=2.5→2_500_000
- K_{2,3}：全部 ecc=2；ECI=3×2+3×2+2×2+2×2+2×2=24；avg=2→2_000_000；D=R=2（自中心图）

### Shell 命令

```
graph topo8          gtopo8        eccentric connectivity   geci
graph eci            graph diameter gdiameter               graph radius
gradius              gecidrc
```

### OS 类比

- **ECI (ξ)：** 加权"到达压力" —— 高度数且同时远离其他节点的节点会带来不成比例的路由成本。ECI 较高的内核枢纽子系统是复制或缓存的候选对象。
- **直径 D：** 最坏情况下的 IPC 延迟（任意节点对之间的最大跳数）。在无环内核依赖图中，D 是故障传播的瓶颈。
- **半径 R：** 最佳情况下的"中心"可达性 —— 任意子系统到达最中心节点所需的最小跳数。R=1 意味着存在一个可在一跳内到达的全局枢纽。
- **avg_ecc：** 从任意节点到其最远可达对等节点的平均结构距离。avg_ecc 越低表示子系统图聚合越紧密。对于完全正则的 D=1 图（完全网状图），avg_ecc=1。

### 显示

- 亮黄色标题：`graph topo8  (ECI + D + R + avg-ecc eccentricity-based indices)`
- ξ：亮青色（精确值）
- D：亮绿色；若 D=0 且 nc>1 则标注 "(all isolated)"；若 D=R>0 则标注 "(self-centered)"
- R：亮品红色
- avg_ecc：亮蓝色（ppm 小数显示）
- 页脚：`N node(s)  M edge(s)  Sharma et al. 1997  Buckley & Harary 1990`

---

## 测试套件: `gos-graph-topo8-harness`

**位置：** `host-tests/gos-graph-topo8-harness/`
**VectorAddress L4：** 95
**插件 ID：** `TOPO_IX8`

10 个测试，全部通过：

1. 空图 → (0, 0, 0, 0, 0, 0)
2. 单节点 → (0, 0, 0, 0, 0, 1)
3. 单边 A→B → (2, 1_000_000, 1, 1, 1, 2)
4. 路径 P₃ → (6, 1_666_666, 2, 1, 2, 3)
5. 三角形 K₃ → (6, 1_000_000, 1, 1, 3, 3)
6. 星图 K_{1,4} → (12, 1_800_000, 2, 1, 4, 5)
7. 路径 P₄ → (14, 2_500_000, 3, 2, 3, 4)
8. 完全图 K₄ → (12, 1_000_000, 1, 1, 6, 4)
9. 两个孤立节点 → (0, 0, 0, 0, 0, 2)
10. K_{2,3} 交叉验证 → (24, 2_000_000, 2, 2, 6, 5)

---

## VectorAddress L4 命名空间（更新）

| L4 | 测试套件 |
|----|---------|
| 88 | graph-topo (SC/GA/AZI) |
| 89 | graph-topo2 (H/ABC/F) |
| 90 | graph-topo3 (SDD/ISI/NI) |
| 91 | graph-topo4 (Sombor/RM2/sigma) |
| 92 | graph-topo5 (HM1/HM2/AG) |
| 93 | graph-topo6 (EM1/ABS/RRR) |
| 94 | graph-topo7 (W/H/WW) |
| **95** | **graph-topo8 (ECI/D/R/avg-ecc)** |

---

## 变更文件

| 文件 | 变更 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | +`graph_topo_indices8_inner()` + `graph_topo_indices8()` 导出 |
| `crates/k-shell/src/lib.rs` | +`dispatch_graph_topo_indices8()` |
| `crates/k-shell/src/proc.rs` | +topo8 的 shell 路由（9 个别名） |
| `host-tests/gos-graph-topo8-harness/` | 新增测试套件（5 个文件） |

---

## 指标

- **新增函数：** 2（内部实现 + 公共导出）
- **新增 shell 别名：** 9
- **新增测试：** 10
- **累计宿主测试数：** 1163
- **算法类别：** 基于离心率（全点对 BFS，O(n·(n+m))，与 V3.18 相同）
- **返回类型：** 6 元组 `(u64, u64, u32, u32, usize, usize)` —— 与 `graph_zagreb` 模式一致
