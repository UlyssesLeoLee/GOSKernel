# GOSKernel 硬化日志 — V3.21
**日期：** 2026-07-07
**分支：** feat/vk-auto-live-surface
**提交：** feat(v3.21): Szeged Sz + Revised Szeged rSz + Mostar Mo edge-partition distance indices + gos-graph-topo10-harness (10 tests)

---

## 摘要

为 `gos_runtime` 新增三个**边划分距离拓扑指数**：**Sz**（Szeged 指数）、**rSz**（修订 Szeged 指数）、**Mo**（Mostar 指数）。它们刻画每条边如何按 BFS 邻近程度对顶点集合进行划分——对每条边 {u,v}，将顶点分类为更靠近 u 的（n_u）、更靠近 v 的（n_v）、或等距的（n_0）。这为基于距离的指数家族（V3.18 的 Wiener/Harary/超-Wiener、V3.19 的离心率指数、V3.20 的度数-距离混合指数）补充了以边为中心的划分几何视角。

宿主测试套件：**累计 1183 个测试**（gos-graph-topo10-harness 新增 10 个，全部通过）。

---

## 新算法

### `graph_topo_indices10()` → `(sz: u64, rsz_ppm: u64, mo: u64, edge_count: usize, node_count: usize)`

**Sz — Szeged 指数**
- 公式：Sz(G) = Σ_{uv∈E} n_u(uv) · n_v(uv)
- 参考文献：Gutman & Klavžar (1995)，*Journal of Chemical Information and Computer Sciences*
- 计算方式：精确整数；对每条无向边，分别统计两侧 BFS 更近的顶点数
- 树的不变量：对树的每条边 n_0 = 0 → Sz = Wiener 指数（一般情形下 Sz ≥ W）
- K_n：Sz = n(n-1)/2 × 1 × 1 = m（每条边 n_u=1, n_v=1，适用于 n≤3 的完全图；对 n≥3 的 K_n，n_u=1, n_v=1, n_0=n-2）

**rSz — 修订 Szeged 指数**
- 公式：rSz(G) = Σ_{uv∈E} (n_u + n_0/2) · (n_v + n_0/2)
- 参考文献：Pisanski & Randić (2010)，*Acta Chimica Slovenica*
- 计算方式：存储为 (4·rSz_int) × 250_000，以避免四分之一整数带来的小数问题
  - 4·rSz_int = Σ_{uv∈E} (2n_u + n_0)(2n_v + n_0) —— 恒为精确整数
  - rSz_ppm = 4·rSz_int × 250_000 = rSz × 10⁶
- 恒有 rSz ≥ Sz；当且仅当所有边的 n_0 = 0 时（例如树和二部图）rSz = Sz
- K₃：rSz = 27/4 = 6.75（四分之一整数；表中首个非整数 rSz 示例）

**Mo — Mostar 指数**
- 公式：Mo(G) = Σ_{uv∈E} |n_u(uv) − n_v(uv)|
- 参考文献：Doslić, Martinjak, Škrekovski, Tipurić Spužević & Zubac (2018)，*Journal of Mathematical Chemistry*
- 计算方式：精确整数；衡量图中所有边二分不平衡度的总和
- 顶点传递不变量：当且仅当所有边满足 n_u = n_v 时（如 K_n、C_{2k}）Mo = 0
- 得名于波斯尼亚城市 Mostar，与 Wiener 指数以化学家 Wiener 命名的方式类似

---

## 算法细节

单趟 O(n·(n+m)) 的 BFS 循环，遍历所有顶点：
1. 构建无向邻接位掩码（有向→无向去重，排除自环）
2. 构建无向边列表（a < b 规范化排序），存储为 `ue_a[]`、`ue_b[]`
3. 从每个顶点 w 出发进行 BFS（0..nc）：BFS 结束后，对每条无向边 (a,b)：
   - 若 dist[a] = INF：跳过（w 无法从该边所在分量到达）
   - 若 dist[a] < dist[b]：ue_nu[edge]++
   - 若 dist[a] > dist[b]：ue_nv[edge]++
   - 若 dist[a] = dist[b]：ue_n0[edge]++
4. 累加：sz += nu×nv；rsz_4 += (2nu+n0)(2nv+n0)；mo += |nu−nv|
5. rsz_ppm = rsz_4 × 250_000

栈上数组：`adj[MAX_NODES]`（u128 ×128 = 2KB）、`ue_a/ue_b/ue_nu/ue_nv/ue_n0[MAX_EDGES]`（u8 ×512 ×5 = 2.5KB）、`dist[MAX_NODES]`（128B）、`queue[MAX_NODES]`（128B）——零堆分配，总计约 5KB。

---

## 交叉核对表

| 图 | Sz | rSz_ppm | Mo | \|E\| | \|V\| |
|-------|-----|---------|-----|-------|-------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| 单边 A-B | 1 | 1_000_000 | 0 | 1 | 2 |
| 路径 P₃ | 4 | 4_000_000 | 2 | 2 | 3 |
| 三角形 K₃ | 3 | 6_750_000 | 0 | 3 | 3 |
| 星图 K_{1,4} | 16 | 16_000_000 | 12 | 4 | 5 |
| 路径 P₄ | 10 | 10_000_000 | 4 | 3 | 4 |
| 完全图 K₄ | 6 | 24_000_000 | 0 | 6 | 4 |
| 两个孤立节点 | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | 36 | 36_000_000 | 6 | 6 | 5 |

### 关键推导

**P₃（树）：**
边 {A,B}：n_u=1(A)，n_v=2(B,C)，n_0=0 → Sz+=2；rsz_4+=8；mo+=1
边 {B,C}：n_u=2(A,B)，n_v=1(C)，n_0=0 → Sz+=2；rsz_4+=8；mo+=1
Sz=4 = Wiener(P₃) ✓（树不变量）；rSz=16/4=4.0 = Sz ✓

**K₃（三角形，每边 n_0=1）：**
每条边：n_u=1, n_v=1, n_0=1 → Sz=3；rsz_4=3×9=27；rsz_ppm=6_750_000；Mo=0
rSz=27/4=6.75 是四分之一整数（表中唯一实例）

**K_{1,4}（星图，树）：**
4 条边各自：n_u=4（中心+3 个叶子），n_v=1（该叶子），n_0=0 → Sz=16=Wiener ✓；Mo=4×3=12

**K₄（每边 n_0=2）：**
6 条边各自：n_u=1, n_v=1, n_0=2 → Sz=6；rsz_4=6×16=96；rsz_ppm=24_000_000；Mo=0

**K_{2,3}（二部图，所有边 n_0=0）：**
6 条跨侧边各自：n_u=3, n_v=2, n_0=0 → Sz=36；rsz_ppm=36_000_000（=Sz，因 n_0=0）；Mo=6
Mo=6>0 印证 K_{2,3} 并非顶点传递图（左侧度数=3 ≠ 右侧度数=2）

---

## Shell 接口

**命令路由**（k-shell/proc.rs）：
```
"graph topo10" | "gtopo10" | "szeged index" | "gszeged" |
"revised szeged" | "grszg" | "mostar index" | "gmostar" | "gszgrsmo"
```

**显示**（`dispatch_graph_topo_indices10`）：
- 标题：亮黄色 "graph topo10 (Sz + rSz + Mo edge-partition distance indices)"
- Sz：亮青色，精确整数，公式注释 [Σ nᵤ·n_v, uv∈E]
- rSz：亮绿色，ppm 小数（3 位小数），公式 [(Σ (nᵤ+n₀/2)·(n_v+n₀/2))]
- Mo：亮洋红色，精确整数，公式 [Σ |nᵤ−n_v|]；为零时附加 "(Mo=0: vertex-transitive)" 注释
- 页脚："N node(s)  M edge(s)  Gutman & Klavžar 1995  Pisanski & Randić 2010  Doslić et al. 2018"

---

## VectorAddress 命名空间

| L4 | Harness |
|----|---------|
| 88 | graph-topo |
| 89 | graph-topo2 |
| 90 | graph-topo3 |
| 91 | graph-topo4 |
| 92 | graph-topo5 |
| 93 | graph-topo6 |
| 94 | graph-topo7 |
| 95 | graph-topo8 |
| 96 | graph-topo9 |
| **97** | **graph-topo10**（V3.21，新增） |

---

## 操作系统类比

- **Sz（Szeged）**：总边二分体积——每条边（IPC 通道）将内核图划分为两侧，Sz 累加两侧规模的乘积。Sz 越高，说明存在越多大规模划分边（结构性负载均衡瓶颈）。对树状依赖图（无环），Sz = Wiener 指数。
- **rSz（修订 Szeged）**：修正后的二分体积，将等距顶点对称地各计一半。仅当图存在使等距顶点出现的环（n_0 > 0）时，rSz > Sz。对环形拓扑的内核，rSz 捕获了普通 Sz 所忽略的"共享边界"开销。
- **Mo（Mostar）**：总二分不平衡度——每条边划分两侧规模差的绝对值之和。对顶点传递图（完全对称的 IPC 拓扑：每条通道均等二分）Mo = 0。Mo 越高，说明部分通道是高度不对称的负载分割者（如某一侧几乎包含所有顶点的枢纽-辐射星图）。

---

## 测试覆盖

`gos-graph-topo10-harness` 新增 10 项测试：
1. 空图 → 全零
2. 单个孤立节点 → 全零
3. 单边 A-B → (1, 1_000_000, 0, 1, 2)
4. 路径 P₃ → (4, 4_000_000, 2, 2, 3)
5. 三角形 K₃ → (3, 6_750_000, 0, 3, 3)
6. 星图 K_{1,4} → (16, 16_000_000, 12, 4, 5)
7. 路径 P₄ → (10, 10_000_000, 4, 3, 4)
8. 完全图 K₄ → (6, 24_000_000, 0, 6, 4)
9. 两个孤立节点 → 全零
10. K_{2,3} 二部图交叉核对 → (36, 36_000_000, 6, 6, 5)

全部 10 项测试通过。宿主测试套件总计：**1183 个测试**（此前 1173 个 + 新增 10 个）。

---

*本文件由 doc/ 根目录同名英文原始存档（`HARDENING_LOG_2026-07-07_V3.21.md`）翻译归位而来，仅译语言，不改动版本号 / 文件路径 / 函数名 / 测试名 / 测试计数等既成事实。*
