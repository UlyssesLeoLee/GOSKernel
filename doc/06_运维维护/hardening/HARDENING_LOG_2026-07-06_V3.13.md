# GOSKernel 硬化日志 — V3.13
**日期：** 2026-07-06
**算法：** 谐波指数 H、原子-键连接性 ABC、被遗忘指数 F
**分支：** feat/vk-auto-live-surface
**提交：** feat(v3.13): H + ABC + F topological indices + gos-graph-topo2-harness (10 tests)

---

## 摘要

V3.13 在 GOSKernel 图论运行时中新增**三个基于度数的拓扑指数**：

| 指数 | 公式 | 文献 |
|-------|---------|------------|
| **H(G)** — 谐波指数 | Σ_{uv∈E} 2/(deg(u)+deg(v)) | Zhong 2012 |
| **ABC(G)** — 原子-键连接性指数 | Σ_{uv∈E} √((deg(u)+deg(v)−2)/(deg(u)·deg(v))) | Estrada et al. 2008 |
| **F(G)** — 被遗忘拓扑指数 | Σ_v deg(v)³（精确整数） | Furtula & Gutman 2015 |

这三个指数构成了化学图论中基于度数的描述符的"第二波"。
H 是 Randić 连接性指数的调和平均类比；ABC 最初源自分子能量建模（过渡态）；
F 曾在 Zagreb 指数文献中被"遗忘"，直至 2015 年被重新发现，并证明其编码了与 M2 互补的信息。

三者均以 **O(V+E)** 复杂度、整数运算完成——无浮点、无堆分配、no_std 安全。

**操作系统类比：**
- **H** = 通道负载分布（对于正则图，均匀 IPC 对应 H = |E|/Δ）
- **ABC** = 连接脆弱度指数；ABC 越高，表示连接不等度数节点的边越多（脆弱的 IPC 拓扑）
- **F** = 立方耦合压力（比 Zagreb M1 的平方度数更激进地放大中枢偏斜）

---

## 公开 API

### `gos_runtime::graph_topo_indices2() -> (u64, u64, u64, usize, usize)`

返回 `(h_ppm, abc_ppm, f_index, edge_count, node_count)`：

- `h_ppm` — H(G) × 10^6，其中 H = Σ_{uv∈E} 2/(deg(u)+deg(v))（Zhong 2012）
- `abc_ppm` — ABC(G) × 10^6，其中 ABC = Σ_{uv∈E} √((s−2)/p)，s=度数和，p=度数积（Estrada et al. 2008）
- `f_index` — F(G) = Σ_v deg(v)³（精确整数，Furtula & Gutman 2015）
- `edge_count` — 无向边计数（有向图去重为无向，排除自环）
- `node_count` — 存活节点计数

**Shell 关键词：** `graph topo2` / `gtopo2` / `harmonic index` / `gh index` / `atom bond connectivity` / `gabc` / `forgotten index` / `gforgotten` / `ghabcf`
**VectorAddress L4=89**，对应 gos-graph-topo2-harness。

---

## 算法

三个指数共享 O(V+E) 无向边扫描模式：

```rust
// 每条边对 H 的贡献：floor(2_000_000 / (da + db))
h_acc += 2_000_000 / s;

// 每条边对 ABC 的贡献：isqrt64((s-2) * 10^12 / p)
// 其中 isqrt64(n) = floor(sqrt(n))，通过牛顿-拉夫逊法求得
// 悬挂-悬挂边（s=2，da=db=1）：贡献 = 0
if s > 2 && p > 0 {
    let numer = (s - 2).saturating_mul(1_000_000_000_000u64);
    abc_acc += isqrt64(numer / p);
}

// F 指数：边扫描之后单独进行的节点扫描
let mut f_index: u64 = 0;
for ci in 0..nc {
    f_index += deg[ci] * deg[ci] * deg[ci];
}
```

### 整数精度

**H：** 每条边贡献 = floor(2_000_000 / s)。每条边误差 ≤ 1 ppm。当 s 整除 2_000_000 时结果精确（例如 K_{2,3} 中 s=5 → H=12/5 恰好整除）。

**ABC：** 每条边贡献 = floor(√((s−2) × 10^12 / p))。
- 通过整数牛顿-拉夫逊 isqrt64 计算 floor(√((s−2)/p) × 10^6)。
- 溢出检查上限：(s−2) ≤ 254（MAX_NODES−1+MAX_NODES−1−2）；254 × 10^12 < 2^64 ✓
- **关键精度结果：** isqrt64(500_000_000_000) = **707_106**（而非 707_107）。
  - 707_106² = 499_998_895_236 < 5×10^11 ✓
  - 707_107² = 500_000_309_449 > 5×10^11 ✗
  - 所有比值 (s−2)/p = 1/2 的边（P₃、K₃、P₄、K_{2,3}）每边 ABC = 707_106。

**F：** 精确整数——Σ_v deg(v)³。最大值：128 × 127³ ≈ 262M，可容纳于 u64。

---

## 关键不变量与交叉校验

| 图 | H_ppm | ABC_ppm | F_index | 说明 |
|-------|-------|---------|---------|-------|
| 空图 | 0 | 0 | 0 | — |
| 单节点 | 0 | 0 | 0 | deg=0 → F=0 |
| 边 A-B | 1_000_000 | 0 | 2 | H=1 精确；ABC=0（s-2=0）；F=1+1=2 |
| P₃ | 1_333_332 | 1_414_212 | 10 | H=4/3；ABC=2×707_106；F=1+8+1 |
| K₃ | 1_500_000 | 2_121_318 | 24 | H=3/2 精确；ABC=3×707_106；F=3×8 |
| K_{1,4} 星形 | 1_600_000 | 3_464_100 | 68 | H=8/5 精确；ABC=4×866_025；F=64+4 |
| P₄ | 1_833_332 | 2_121_318 | 18 | H 为混合值；ABC=3×707_106（所有边比值均为 1/2） |
| K₄ | 1_999_998 | 3_999_996 | 108 | H≈2 向下取整；ABC=6×666_666；F=4×27 |
| K_{2,3} | 2_400_000 | 4_242_636 | 78 | H 精确（s=5）；ABC=6×707_106；F=2×27+3×8 |

**H 正则图不变量：** 对于 Δ-正则图，H = |E| / Δ（当 s 整除 2_000_000 时精确）。
- K₃：H = 3/2，H_ppm = 1_500_000 = 3 × 10^6 / 2 ✓
- K₄：H = 6/3 = 2，H_ppm ≈ 1_999_998（向下取整：6 × 333_333）— 偏差 2 ppm

**ABC 悬挂边不变量：** da=db=1（s=2）的边：(s−2)/p = 0 → ABC 贡献 = 0（与 AZI 相同的跳过规则）。

**ABC 比值不变量：** P₃、K₃、P₄、K_{2,3} 中的边均满足 (s−2)/p = 1/2 → 每边 707_106 ppm。这是一个跨图的数值巧合，源于不同度数对共享同一比值：
- P₃ 外边 (1,2)：(1+2−2)/(1×2) = 1/2
- K₃ (2,2)：(2+2−2)/(2×2) = 2/4 = 1/2
- P₄ 内边 (2,2)：同样 = 1/2
- K_{2,3} (3,2)：(3+2−2)/(3×2) = 3/6 = 1/2

---

## 测试套件（gos-graph-topo2-harness）

10 个宿主测试，全部通过：

| # | 图 | H_ppm | ABC_ppm | F | ec | nc |
|---|-------|-------|---------|---|----|----|
| 1 | 空图 | 0 | 0 | 0 | 0 | 0 |
| 2 | 单节点 | 0 | 0 | 0 | 0 | 1 |
| 3 | 边 A-B | 1_000_000 | 0 | 2 | 1 | 2 |
| 4 | P₃ | 1_333_332 | 1_414_212 | 10 | 2 | 3 |
| 5 | K₃ | 1_500_000 | 2_121_318 | 24 | 3 | 3 |
| 6 | K_{1,4} | 1_600_000 | 3_464_100 | 68 | 4 | 5 |
| 7 | P₄ | 1_833_332 | 2_121_318 | 18 | 3 | 4 |
| 8 | K₄ | 1_999_998 | 3_999_996 | 108 | 6 | 4 |
| 9 | 2 个孤立节点 | 0 | 0 | 0 | 0 | 2 |
| 10 | K_{2,3} | 2_400_000 | 4_242_636 | 78 | 6 | 5 |

测试 10 包含两个交叉校验断言：
1. `H exact`：`h == 6 * 2_000_000 / 5 = 2_400_000`（s=5 恰好整除 2_000_000）
2. `ABC ratio`：`abc == 6 * 707_106`（所有边比值 (s−2)/p = 1/2）

---

## Shell 显示

```
 graph topo2  (H + ABC + F degree-based indices)
 ───────────────────────────────────────────────────────────
  harmonic index     H   =  X.XXX   [Σ 2/(deg(u)+deg(v))]
  atom-bond conn     ABC =  X.XXX   [Σ √((d+d−2)/(d·d))]
  forgotten index    F   =  N       [Σ_v deg(v)³]  (exact)
 ───────────────────────────────────────────────────────────
 N node(s)  M edge(s)  Zhong 2012  Estrada et al. 2008  Furtula & Gutman 2015
```

颜色：标题亮黄色（14）；H 亮青色（11）；ABC 亮绿色（10）；F 亮洋红色（13）。

---

## 累计宿主测试数

| 版本 | 新增 | 累计 |
|---------|-------|-----------|
| V3.12（SC+GA+AZI） | 10 | 1093 |
| **V3.13（H+ABC+F）** | **10** | **1103** |

---

## 参考文献

- Zhong L. (2012). "The harmonic index for graphs." *Applied Mathematics Letters*, 25(3):561-566.
- Estrada E., Torres L., Rodríguez L., Gutman I. (2008). "An atom-bond connectivity index: Modelling the enthalpy of formation of alkanes." *Indian Journal of Chemistry*, 47A:711-717.
- Furtula B., Gutman I. (2015). "A forgotten topological index." *Journal of Mathematical Chemistry*, 53(4):1184-1190.
</content>
