# GOSKernel 强化日志 V3.35 — 2026-07-15

**分支**: feat/vk-auto-live-surface  
**提交**: feat(v3.35): NISI + NAZI + NEM1 Neighborhood S-variant indices + gos-graph-topo24-harness (10 tests)

---

## 摘要

新增三个 Neighborhood S-variant 拓扑指数，延续 V3.29–V3.34 引入的 S 系列家族。
本版本新增反和入度指数（ISI）、增广 Zagreb 指数（AZI）、重构第一 Zagreb 指数（EM₁）的 S-模拟量。

---

## 新增内容

### `gos_runtime::graph_topo_indices24() -> (nisi_ppm: u64, nazi_milli: u64, nem1: u64, edge_count: usize, node_count: usize)`

**S(v) = Σ_{w∈N(v)} deg(w)** — 邻居度数和（与 topo18/topo21–topo24 族相同的 S 定义）

| 指数 | 公式 | 精度 | 参考文献 |
|------|------|------|-----------|
| NISI | Σ_{uv∈E} S_u·S_v/(S_u+S_v) | 向下取整 ppm | ISI 的 S-模拟量（Sedlar et al. 2011） |
| NAZI | Σ_{uv∈E} (S_u·S_v/(S_u+S_v−2))³ | 向下取整 milli | AZI 的 S-模拟量（Furtula et al. 2010） |
| NEM1 | Σ_{uv∈E} (S_u+S_v−2)² | 精确 u64 | EM₁ 的 S-模拟量（Milićević et al. 2004） |

**关键不变量：**
- 对 S-正则图（S 均匀分布）：NISI = |E|×S/2×10⁶
- 当每条边满足 S_u+S_v=2 时（仅 K₂ 型：两端点 S=1），NAZI = 0
- 当且仅当所有边满足 S_u+S_v=2 时，NEM1 = 0
- K₃ 与 K_{1,4} 每边取值相同（均为 S-均匀 S=4，ssum=8，sp=16，q=6）
- K₄（S=9）与 K_{2,3}（S=6）取值不同（与此前部分 S 系列指数的表现不同）

**溢出安全性：**
- NISI：S_u·S_v·10⁶ ≤ 16129²×10⁶ ≈ 2.6×10¹⁷ < u64::MAX ✓
- NAZI：(S_u·S_v)³ 需要 u128 中间量；除法后每边结果 ≤ ~5.24×10¹⁴，可容纳于 u64 ✓
- NEM1：(ssum−2)² ≤ 32256² ≈ 10⁹，每边 × 8065 ≈ 8×10¹² < u64::MAX ✓

**算法：** O(V+E) — 邻接+度数遍历 → S(v) 计算 → 边扫描；无需 BFS

**交叉验证表：**

| 图 | NISI (ppm) | NAZI (milli) | NEM1 | 边数 | 点数 |
|-------|-----------|-------------|------|-------|-------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| K₂ | 500_000 | 0 | 0 | 1 | 2 |
| P₃ | 2_000_000 | 16_000 | 8 | 2 | 3 |
| K₃ | 6_000_000 | 56_886 | 108 | 3 | 3 |
| K_{1,4} | 8_000_000 | 75_848 | 144 | 4 | 5 |
| P₄ | 3_900_000 | 27_390 | 34 | 3 | 4 |
| K₄ | 27_000_000 | 778_476 | 1_536 | 6 | 4 |
| K_{2,3} | 18_000_000 | 279_936 | 600 | 6 | 5 |

**Shell 命令：** `graph topo24` / `gtopo24` / `neighborhood isi` / `gnisi` / `neighborhood azi` / `gnazi` / `neighborhood em1` / `gnem1` / `gnisinazinemm1`

**gos-graph-topo24-harness 的 VectorAddress L4=111**

**操作系统类比：**
- NISI = 每通道的 S-谐波耦合强度（S-均匀时 = |E|×S/2；负载均衡）
- NAZI = S-增广键压立方（悬挂对叶拓扑时为 0；密集枢纽时较高）
- NEM1 = 每通道 S-盈余的平方（K₂ 型时为 0；衡量高于阈值 2 的 S-盈余量）

**显示：** 亮黄色标题；NISI 亮青色（ppm）；NAZI 亮绿色（milli + "NAZI=0: all pendant-pair" 注记）；NEM1 亮品红（精确值 + "NEM1=0: all S₁+S₂=2" 注记）

**页脚：** "Sedlar et al. 2011  Furtula et al. 2010  Milicevic et al. 2004  (S-variant family)"

---

## 变更文件

| 文件 | 变更内容 |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices24_inner()` + `graph_topo_indices24()` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices24()` |
| `crates/k-shell/src/proc.rs` | 新增 topo24 命令路由 |
| `host-tests/gos-graph-topo24-harness/` | 新建 harness crate（10 个测试，VectorAddress L4=111） |
| `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-15_V3.35.md` | 本篇日志 |

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

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**宿主测试套件总计：1323 个测试**（截至 V3.34 的 1313 个 + 新增 10 个）

---

## VectorAddress L4 命名空间（更新后）

88=graph-topo, 89=graph-topo2, 90=graph-topo3, 91=graph-topo4, 92=graph-topo5,  
93=graph-topo6, 94=graph-topo7, 95=graph-topo8, 96=graph-topo9, 97=graph-topo10,  
98=graph-topo11, 99=graph-topo12, 100=graph-topo13, 101=graph-topo14, 102=graph-topo15,  
103=graph-topo16, 104=graph-topo17, 105=graph-topo18, 106=graph-topo19, 107=graph-topo20,  
108=graph-topo21, 109=graph-topo22, 110=graph-topo23, **111=graph-topo24**
