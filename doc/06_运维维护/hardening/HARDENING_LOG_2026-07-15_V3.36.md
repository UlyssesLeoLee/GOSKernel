# GOSKernel 强化日志 V3.36 — 2026-07-15

**分支**: feat/vk-auto-live-surface  
**提交**: feat(v3.36): NHM₂ + NAG + NABS Neighborhood S-variant indices + gos-graph-topo25-harness (10 tests)

---

## 摘要

新增三个 Neighborhood S-variant 拓扑指数，延续 V3.29–V3.35 引入的 S 系列家族。
本版本新增超-第二 Zagreb 指数（HM₂）、算术-几何比（AG）、原子-键和连通性（ABS）的 S-模拟量。

---

## 新增内容

### `gos_runtime::graph_topo_indices25() -> (nhm2: u64, nag_ppm: u64, nabs_ppm: u64, edge_count: usize, node_count: usize)`

**S(v) = Σ_{w∈N(v)} deg(w)** — 邻居度数和（与 topo18/topo21–topo25 族相同的 S 定义）

| 指数 | 公式 | 精度 | 参考文献 |
|-------|-----------|-------|-----------|
| NHM₂ | Σ_{uv∈E} (S_u·S_v)² | 精确 u64 | HM₂ 的 S-模拟量（Das & Trinajstić 2011） |
| NAG  | Σ_{uv∈E} (S_u+S_v)/(2√(S_u·S_v)) | 向下取整 ppm | AG 比值的 S-模拟量（Zheng et al. 2020） |
| NABS | Σ_{uv∈E} √((S_u+S_v−2)/(S_u+S_v)) | 向下取整 ppm | ABS 的 S-模拟量（Chen et al. 2022） |

**关键不变量：**
- NAG ≥ |E|×10⁶ 恒成立（对 S_u, S_v ≥ 1 有 AM≥GM）；当且仅当 S-均匀（每条边 S_u=S_v）时取等
- 仅当每条边满足 S_u+S_v=2 时 NABS = 0（仅 K₂：两端均为 S=1）
- K₃ 与 K_{1,4} 在每边的 NAG、NABS 上重合（均为 S-均匀 S=4；ssum=8，sp=16）
- K₄（S=9）与 K_{2,3}（S=6）均给出 NAG=|E|×10⁶=6_000_000（S-均匀，|E|=6），但 NHM₂ 与 NABS 不同

**实现公式（无浮点、no_std）：**
- NHM₂ 每边：`(sp as u128) * (sp as u128)`，其中 sp=S_u·S_v；u128 累加器 → 转换为 u64
- NAG 每边：`floor(ssum·10¹² / (2·isqrt128(sp·10¹²)))`
  — 对最大度数图，sp·10¹² 可达 ~2.6×10²⁰ → isqrt128 入参需要 u128
- NABS 每边：`isqrt64((ssum-2)·10¹² / ssum)`
  — (ssum-2)·10¹² ≤ 32256·10¹² ≈ 3.2×10¹⁶ < u64::MAX ✓；ssum=2 时天然为 0

**溢出安全性：**
- NHM₂：sp=S_u·S_v ≤ 16129²=260_144_641；每边 sp²≤6.77×10¹⁶ < u64::MAX ✓；求和使用 u128 累加器
- NAG：ssum·10¹² ≤ 32258·10¹² ≈ 3.2×10¹⁶ < u64::MAX ✓；sp·10¹² 使用 u128（可能超过 u64::MAX）
- NABS：(ssum-2)·10¹² ≤ 3.2×10¹⁶ < u64::MAX ✓

**算法：** O(V+E) — 邻接+度数遍历 → S(v) 计算 → 边扫描；无需 BFS

**交叉验证表：**

| 图 | NHM₂ | NAG (ppm) | NABS (ppm) | 边数 | 点数 |
|-------|------|-----------|-----------|-------|-------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| K₂ | 1 | 1_000_000 | 0 | 1 | 2 |
| P₃ | 32 | 2_000_000 | 1_414_212 | 2 | 3 |
| K₃ | 768 | 3_000_000 | 2_598_075 | 3 | 3 |
| K_{1,4} | 1024 | 4_000_000 | 3_464_100 | 4 | 5 |
| P₄ | 153 | 3_041_242 | 2_365_688 | 3 | 4 |
| K₄ | 39366 | 6_000_000 | 5_656_854 | 6 | 4 |
| K_{2,3} | 7776 | 6_000_000 | 5_477_220 | 6 | 5 |

注：K₄ 的 NABS 使用 `isqrt64(888_888_888_888)=942_809`（验证：942_809²=888_888_810_481 ≤ 目标值 < 942_810²）。

**Shell 命令：** `graph topo25` / `gtopo25` / `neighborhood hm2` / `gnhm2` / `neighborhood ag` / `gnag` / `neighborhood abs` / `gnabs` / `gnhm2nagnabs`

**gos-graph-topo25-harness 的 VectorAddress L4=112**

**操作系统类比：**
- NHM₂ = S-乘积耦合强度的平方（放大枢纽间的 S-权重；(S_u·S_v)²=0 仅在空图时成立）
- NAG  = S-算术-几何通道均衡度（S-均匀时 = |E|；由 AM≥GM，混合 S 时 > |E|）
- NABS = S-原子-键和广度比（K₂ 拓扑时为 0；随高于阈值 2 的 S-盈余增大）

**显示：** 亮黄色标题；NHM₂ 亮青色（精确值）；NAG 亮绿色（ppm + "≡|E| (S-regular)" 注记）；NABS 亮品红（ppm + "NABS=0: all S₁+S₂=2" 注记）

**页脚：** "Das & Trinajstić 2011  Zheng et al. 2020  Chen et al. 2022  (S-variant family)"

---

## 变更文件

| 文件 | 变更内容 |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices25_inner()` + `graph_topo_indices25()` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices25()` |
| `crates/k-shell/src/proc.rs` | 新增 topo25 命令路由 |
| `host-tests/gos-graph-topo25-harness/` | 新建 harness crate（10 个测试，VectorAddress L4=112） |
| `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-15_V3.36.md` | 本篇日志 |

---

## 测试结果

```
running 10 tests
test test_01_empty ... ok
test test_02_single_node ... ok
test test_03_single_edge ... ok
test test_04_path_p3 ... ok
test test_05_triangle_k3 ... ok
test test_06_star_k14 ... ok
test test_07_path_p4 ... ok
test test_08_complete_k4 ... ok
test test_09_two_isolated ... ok
test test_10_k23_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 filtered out; 0 measured
```

**宿主测试套件总计：1333 个测试**（截至 V3.35 的 1323 个 + 新增 10 个）

---

## VectorAddress L4 命名空间（更新后）

88=graph-topo, 89=graph-topo2, 90=graph-topo3, 91=graph-topo4, 92=graph-topo5,  
93=graph-topo6, 94=graph-topo7, 95=graph-topo8, 96=graph-topo9, 97=graph-topo10,  
98=graph-topo11, 99=graph-topo12, 100=graph-topo13, 101=graph-topo14, 102=graph-topo15,  
103=graph-topo16, 104=graph-topo17, 105=graph-topo18, 106=graph-topo19, 107=graph-topo20,  
108=graph-topo21, 109=graph-topo22, 110=graph-topo23, 111=graph-topo24, **112=graph-topo25**
