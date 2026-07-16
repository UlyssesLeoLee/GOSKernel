# GOSKernel 硬化日志 — V3.16
**日期：** 2026-07-06
**分支：** feat/vk-auto-live-surface
**提交：** feat(v3.16): HM₁ + HM₂ + AG topological indices + gos-graph-topo5-harness (10 tests)

---

## 摘要

新增三个成熟的基于度数的拓扑指数，作为 `gos_runtime::graph_topo_indices5()` 提供，延续 V3.11–V3.15 建立的拓扑指数系列。宿主测试套件目前累计 **1133 个测试**。

---

## 新算法

### `graph_topo_indices5()` → `(hm1: u64, hm2: u64, ag_ppm: u64, edge_count: usize, node_count: usize)`

**HM₁ — 第一超-Zagreb 指数**（Shirdel、Rezapour & Sayadi，2013）
- HM₁(G) = Σ_{uv∈E} (d(u) + d(v))²
- 精确整数；每条边贡献 = s²，其中 s = d_u + d_v
- 对任意 Δ-正则图：HM₁ = 4·|E|·Δ²（因 s = 2Δ → s² = 4Δ²）
- K₃（Δ=2）：3×16 = 48；K₄（Δ=3）：6×36 = 216 ✓

**HM₂ — 第二超-Zagreb 指数**（Das & Trinajstić，2011）
- HM₂(G) = Σ_{uv∈E} (d(u) × d(v))²
- 精确整数；每条边贡献 = p²，其中 p = d_u × d_v
- 对任意 Δ-正则图：HM₂ = |E|·Δ⁴（因 p = Δ² → p² = Δ⁴）
- K₃（Δ=2）：3×16 = 48；K₄（Δ=3）：6×81 = 486 ✓

**AG — 算术-几何指数**（Zheng、Li & Liu，2020）
- AG(G) = Σ_{uv∈E} (d(u) + d(v)) / (2√(d(u)·d(v)))
- 每条边 ag_ppm = floor(s × 10¹² / (2 × isqrt64(p × 10¹²)))，逐边累加
- 关键不变量：当且仅当图为正则图时 AG = |E|（AM-GM 等号成立：d_u = d_v → AM = GM = d_u）
- 恒有 AG ≥ |E|（AM ≥ GM，等号当且仅当每条边的 d_u = d_v）
- 这是 GA 指数（已于 V3.12 实现为 `ga_ppm`）的乘性对偶指数

### 整数精度

| 指数 | 逐边计算 | 误差界 |
|-------|------------------------------|-------------------|
| HM₁ | s²（精确 u64） | 精确 |
| HM₂ | p²（精确 u64） | 精确 |
| AG | floor(s·10¹²/(2·isqrt64(p·10¹²))) | ≤1 ppm/边 |

### 溢出边界（MAX_NODES=128，MAX_EDGES=512）
- HM₁：s ≤ 254；s² ≤ 64516；× 512 条边 ≈ 33M → 在 u64 范围内绰绰有余
- HM₂：p ≤ 127² = 16129；p² ≤ 260M；× 512 ≈ 133B → 在 u64 范围内绰绰有余
- AG：每条边 ≤ 约 1.25×10⁶（最坏非对称情形）；× 512 ≈ 640M → 在 u64 范围内绰绰有余

---

## 解析交叉核对表

| 图 | HM₁ | HM₂ | AG_ppm | 边数 | 备注 |
|--------------|------|-----|-----------|-------|---------------------------|
| 空图 | 0 | 0 | 0 | 0 | |
| 单边 A-B | 4 | 1 | 1_000_000 | 1 | da=db=1；正则（AG=m） |
| 路径 P₃ | 18 | 8 | 2_121_320 | 2 | 每边 s=3,p=2 |
| 三角形 K₃ | 48 | 48 | 3_000_000 | 3 | Δ=2 正则；AG=m=3 |
| 星图 K_{1,4} | 100 | 64 | 5_000_000 | 4 | s=5,p=4；(4+1)/(2√4)=5/4 精确 |
| 路径 P₄ | 34 | 24 | 3_121_320 | 3 | 混合边 |
| 完全图 K₄ | 216 | 486 | 6_000_000 | 6 | Δ=3 正则；AG=m=6 |
| K_{2,3} | 150 | 216 | 6_123_726 | 6 | s=5,p=6；6×1_020_621 |

### K_{2,3} 的关键精度说明
- isqrt64(6×10¹²) = 2_449_489（√6 × 10⁶ = 2_449_489.742... 的下取整）
- 2x = 4_898_978
- floor(5×10¹² / 4_898_978) = 1_020_621 ←（不是 1_020_620）
  - 4_898_978 × 1_020_621 = 4_999_999_825_338 < 5×10¹² ✓
  - 4_898_978 × 1_020_622 = 5_000_004_724_316 > 5×10¹² ✓
- 总计 = 6 × 1_020_621 = 6_123_726

---

## Shell 命令

| 命令 | 路由至 |
|----------------------------------------|-------------------|
| `graph topo5` | topo5 分发 |
| `gtopo5` | topo5 分发 |
| `hyper zagreb` | topo5 分发 |
| `ghm1` | topo5 分发 |
| `hm2 index` | topo5 分发 |
| `ghm2` | topo5 分发 |
| `arithmetic geometric` | topo5 分发 |
| `gag` | topo5 分发 |
| `ghm1hm2ag` | topo5 分发 |

---

## 显示格式

```
 graph topo5  (HM₁ + HM₂ + AG degree-based indices)
 ───────────────────────────────────────────────────────────
  hyper-zagreb 1st   HM₁=  48        [Σ (d+d)²]
  hyper-zagreb 2nd   HM₂=  48        [Σ (d·d)²]
  arith-geo index    AG  =  3.000     [Σ (d+d)/(2√d·d)]  (regular: AG=m)
 ───────────────────────────────────────────────────────────
3 node(s)  3 edge(s)  Shirdel et al. 2013  Das & Trinajstić 2011  Zheng et al. 2020
```

- 标题：亮黄色（color 14）
- HM₁：亮青色（color 11）
- HM₂：亮绿色（color 10）
- AG：亮洋红色（color 13）；当 AG = m × 10⁶ 时以亮绿色附加 `(regular: AG=m)` 注释

---

## VectorAddress 命名空间（更新）

```
88=graph-topo   (V3.12 SC+GA+AZI)
89=graph-topo2  (V3.13 H+ABC+F)
90=graph-topo3  (V3.14 SDD+ISI+NI)
91=graph-topo4  (V3.15 SO+RM₂+σ)
92=graph-topo5  (V3.16 HM₁+HM₂+AG)
```

---

## 测试结果

**gos-graph-topo5-harness：10/10 测试通过**

```
test test_01_empty           ... ok
test test_02_single_node     ... ok
test test_03_single_edge     ... ok  (regular: AG=1.0=|E|×1.0)
test test_04_path_p3         ... ok  (non-regular: AG>m)
test test_05_triangle_k3     ... ok  (regular invariants: HM1=4|E|Δ², HM2=|E|Δ⁴, AG=m)
test test_06_star_k14        ... ok  (exact AG=5.0; (4+1)/(2√4)=5/4)
test test_07_path_p4         ... ok  (mixed edges; inner edge B-C regular)
test test_08_complete_k4     ... ok  (regular invariants; x=3_000_000 exact)
test test_09_two_isolated    ... ok  (no edges)
test test_10_k23_cross_check ... ok  (precision: 6×1_020_621=6_123_726)
```

**累计宿主测试套件：1133 个测试**（V3.15 后为 1123 个）

---

## 操作系统类比

- **HM₁**：聚合平方和耦合压力——放大以枢纽为中心的拓扑结构（HM₁/|E| 越高，说明越少数量的高连接网关节点主导了 IPC 图）
- **HM₂**：聚合平方积枢纽密度——衡量共枢纽耦合强度；对正则网格 HM₂/|E| = Δ⁴，对枢纽-辐射型拓扑则会陡增
- **AG**：算术-几何比值指数——衡量各通道间的度数不对称程度；对均衡网格（类似 NUMA 均匀访问域）AG = |E|；AG > |E| 则提示存在不对称的辐射-枢纽型 IPC（类似 I/O 枢纽与计算节点的关系）

---

## 参考文献

- Shirdel, Rezapour & Sayadi (2013): "The hyper-Zagreb index of graph operations". *Iranian J. Math. Chem.*
- Das & Trinajstić (2011): "Relationship between the Eccentric Connectivity Index and Zagreb Indices". *Computers & Math. with Applications*
- Zheng, Li & Liu (2020): "New bounds on the arithmetic-geometric index". *J. Math. Chem.*

---

*本文件由 doc/ 根目录同名英文原始存档（`HARDENING_LOG_2026-07-06_V3.16.md`）翻译归位而来，仅译语言，不改动版本号 / 文件路径 / 函数名 / 测试名 / 测试计数等既成事实。*
