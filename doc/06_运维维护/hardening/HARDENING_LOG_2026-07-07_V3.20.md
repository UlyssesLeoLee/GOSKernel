# GOSKernel 硬化日志 — V3.20
**日期：** 2026-07-07
**分支：** feat/vk-auto-live-surface
**提交：** feat(v3.20): Schultz MTI W_S + Gutman W_G + Connective Eccentric CxiE degree-distance hybrid topological indices + gos-graph-topo9-harness (10 tests)

---

## 摘要

为 `gos_runtime` 新增三个**度数-距离混合型拓扑指数**：**W_S**（Schultz 分子拓扑指数）、**W_G**（Gutman 指数）、**CξE**（连通离心指数）。它们在 V3.18 的纯距离指数（Wiener/Harary/超-Wiener）与 V3.19 的离心率指数（ECI/直径/半径）之间架起桥梁，通过度数乘积或比值对成对距离加权。

宿主测试套件：**累计 1173 个测试**（gos-graph-topo9-harness 新增 10 个，全部通过）。

---

## 新算法

### `graph_topo_indices9()` → `(ws: u64, wg: u64, cxe_ppm: u64, edge_count: usize, node_count: usize)`

**W_S — Schultz 分子拓扑指数（MTI）**
- 公式：W_S(G) = Σ_{u<v} (deg(u)+deg(v)) × d(u,v)
- 参考文献：Schultz (1989)，*Journal of Chemical Information and Computer Sciences*
- 计算方式：在对所有节点对（src < v）的 BFS 过程中累加；恒为精确整数（对现实规模图不会溢出）
- 不变量：对 Δ-正则图 W_S = 2Δ × W(G)（所有节点对的度数和均为 2Δ）
- K_n：W_S = 2(n-1) × n(n-1)/2 = n(n-1)²
- 不连通节点对（d=∞）：贡献为 0

**W_G — Gutman 指数**
- 公式：W_G(G) = Σ_{u<v} deg(u) × deg(v) × d(u,v)
- 参考文献：Gutman (1994)，*Journal of Mathematical Chemistry*
- 计算方式：在同一趟 BFS 中累加；恒为精确整数
- 不变量：对 Δ-正则图 W_G = Δ² × W(G)
- K_n：W_G = (n-1)² × n(n-1)/2 = n(n-1)³/2
- 不连通节点对（d=∞）：贡献为 0

**CξE — 连通离心指数**
- 公式：CξE(G) = Σ_v deg(v)/ecc(v) × 10⁶
- 参考文献：Gupta, Singh & Madan (2000)，*Journal of Chemical Information and Computer Sciences*
- 计算方式：从每个节点执行 BFS 得到 ecc[v]；随后 CξE = Σ_v floor(deg[v] × 10⁶ / ecc[v])
- 孤立节点（ecc=0，deg=0）：贡献为 0——不会发生除零
- 正则自中心图（D=R）：CξE = n × Δ/D × 10⁶
- K_n：CξE = n × (n-1)/1 × 10⁶ = n(n-1) × 10⁶

---

## 算法细节

三个指数共用一趟 O(n·(n+m)) 的 BFS 循环：
1. 构建无向邻接位掩码 + 度数数组（有向→无向去重，排除自环）
2. 从每个源节点 `src` 出发进行 BFS（0..nc）：
   - BFS 结束后，对每个 dist[v] ≠ INF 的 `v > src`：
     - `ws += (deg[src]+deg[v]) × dist[v]`
     - `wg += deg[src] × deg[v] × dist[v]`
   - 记录 `ecc[src]` = 从 src 出发的最大有限距离
3. BFS 循环结束后：`cxe_ppm = Σ_v (若 ecc[v]>0 则 deg[v]×10⁶/ecc[v]，否则为 0)`

栈上数组：`adj[MAX_NODES]`（u128）、`deg[MAX_NODES]`（u32）、`ecc[MAX_NODES]`（u32）、`dist[MAX_NODES]`（u8）、`queue[MAX_NODES]`（u8）——零堆分配。

---

## 交叉核对表

| 图 | W_S | W_G | CξE_ppm | \|E\| | \|V\| |
|-------|-----|-----|---------|-------|-------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| 单边 A-B | 2 | 1 | 2_000_000 | 1 | 2 |
| 路径 P₃ | 10 | 6 | 3_000_000 | 2 | 3 |
| 三角形 K₃ | 12 | 12 | 6_000_000 | 3 | 3 |
| 星图 K_{1,4} | 44 | 28 | 6_000_000 | 4 | 5 |
| 路径 P₄ | 28 | 19 | 2_666_666 | 3 | 4 |
| 完全图 K₄ | 36 | 54 | 12_000_000 | 6 | 4 |
| 两个孤立节点 | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | 66 | 78 | 6_000_000 | 6 | 5 |

### 关键推导

**K₃（Δ=2，ecc=1 全部）：**
W_S = 3 对 × (2+2)×1 = 12；W_G = 3×4 = 12；CξE = 3×2/1×10⁶ = 6_000_000

**K₄（Δ=3，ecc=1 全部）：**
W_S = 6×6 = 36；W_G = 6×9 = 54；CξE = 4×3×10⁶ = 12_000_000

**K_{1,4}（中心 deg=4 ecc=1；叶子 deg=1 ecc=2）：**
W_S = 4×(4+1)×1 + 6×(1+1)×2 = 20+24 = 44
W_G = 4×4×1×1 + 6×1×1×2 = 16+12 = 28
CξE = 4/1×10⁶ + 4×(1/2×10⁶) = 4_000_000+2_000_000 = 6_000_000

**P₄（deg=[1,2,2,1]，ecc=[3,2,2,3]）：**
W_S = 3+6+6+4+6+3 = 28；W_G = 2+4+3+4+4+2 = 19
CξE = ⌊10⁶/3⌋×2 + 10⁶×2 = 333_333×2+2_000_000 = 2_666_666

**K_{2,3}（左侧 deg=3 ecc=2，右侧 deg=2 ecc=2）：**
W_S = 12+8+8+8+5×6 = 66；W_G = 18+8+8+8+6×6 = 78
CξE = ⌊3×10⁶/2⌋×2 + ⌊2×10⁶/2⌋×3 = 1_500_000×2+1_000_000×3 = 6_000_000

---

## Shell 接口

**命令路由**（k-shell/proc.rs）：
```
"graph topo9" | "gtopo9" | "schultz mti" | "gws" |
"gutman index" | "gwg" | "connective eccentric" | "gcxe" | "gwsgwgcxe"
```

**显示**（`dispatch_graph_topo_indices9`）：
- 标题：亮黄色 "graph topo9 (W_S + W_G + CxiE degree-distance hybrid indices)"
- W_S：亮青色，精确整数，公式注释 [Σ (dᵤ+d_v)·d(u,v), u<v]
- W_G：亮绿色，精确整数，公式注释 [Σ dᵤ·d_v·d(u,v), u<v]
- CξE：亮洋红色，ppm 小数（3 位小数），公式 [Σ deg(v)/ecc(v)]
- 页脚："N node(s)  M edge(s)  Schultz 1989  Gutman 1994  Gupta et al. 2000"

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
| **96** | **graph-topo9**（V3.20，新增） |

---

## 操作系统类比

- **W_S（Schultz MTI）**：总度数加权路由负载——每一跳都会被两端节点的度数之和放大；枢纽附近的长路径会受到双重惩罚（度数高 且 距离远）
- **W_G（Gutman 指数）**：乘积度数路由压力——对枢纽到枢纽的远距离连接呈二次方放大；比 W_S 对枢纽集中现象更敏感
- **CξE（连通离心指数）**：每节点吞吐量与可达范围之比——度数高但离心率小（全局性枢纽）的节点贡献最大；对半径大的孤立/叶子节点贡献为 0

---

## 测试覆盖

`gos-graph-topo9-harness` 新增 10 项测试：
1. 空图 → 全零
2. 单个孤立节点 → 全零
3. 单边 A-B → (2, 1, 2_000_000, 1, 2)
4. 路径 P₃ → (10, 6, 3_000_000, 2, 3)
5. 三角形 K₃ → (12, 12, 6_000_000, 3, 3)
6. 星图 K_{1,4} → (44, 28, 6_000_000, 4, 5)
7. 路径 P₄ → (28, 19, 2_666_666, 3, 4)
8. 完全图 K₄ → (36, 54, 12_000_000, 6, 4)
9. 两个孤立节点 → 全零
10. K_{2,3} 二部图交叉核对 → (66, 78, 6_000_000, 6, 5)

全部 10 项测试通过。宿主测试套件总计：**1173 个测试**（此前 1163 个 + 新增 10 个）。

---

*本文件由 doc/ 根目录同名英文原始存档（`HARDENING_LOG_2026-07-07_V3.20.md`）翻译归位而来，仅译语言，不改动版本号 / 文件路径 / 函数名 / 测试名 / 测试计数等既成事实。*
