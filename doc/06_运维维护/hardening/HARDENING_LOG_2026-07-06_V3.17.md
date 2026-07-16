# GOSKernel 硬化日志 — V3.17
**日期：** 2026-07-06
**分支：** feat/vk-auto-live-surface
**提交：** feat(v3.17): EM1 + ABS + RRR topological indices + gos-graph-topo6-harness (10 tests)

---

## 摘要

为 `gos_runtime` 新增三个基于度数的拓扑指数：**EM₁**（重构第一 Zagreb 指数）、**ABS**（原子-键和连通性指数）、**RRR**（约化倒数 Randić 指数）。至此拓扑指数库扩展到 6 组（V3.12–V3.17），累计覆盖 18 个以上指数。

---

## 新算法

### `graph_topo_indices6()` → `(em1: u64, abs_ppm: u64, rrr_ppm: u64, edge_count: usize, node_count: usize)`

**EM₁（重构第一 Zagreb 指数）**
- 公式：EM₁(G) = Σ_{uv∈E} (dₐ+d_b-2)²
- 参考文献：Milićević, Nikolić, Trinajstić & Tolić-Stipčević (2004)
- 计算方式：贡献 = q²，其中 q = dₐ+d_b-2；恒为精确整数
- 不变量：对 Δ-正则图 EM₁ = 4m(Δ-1)²
- 当 q=0（悬挂边对，dₐ=d_b=1）时 EM₁ = 0

**ABS（原子-键和连通性指数）**
- 公式：ABS(G) = Σ_{uv∈E} √((dₐ+d_b-2)/(dₐ+d_b))
- 参考文献：Chen et al. (2022)
- 计算方式：每条边 isqrt64(q × 10¹²/s)（下取整误差 ≤ 1 ppm）
- 对 Δ-正则图：ABS = m·√((Δ-1)/Δ)
- 当 q=0（悬挂对）时 ABS = 0——无需特判即可自然得零

**RRR（约化倒数 Randić 指数）**
- 公式：RRR(G) = Σ_{uv∈E} √((dₐ-1)(d_b-1))
- 参考文献：Li & Shi (2008)
- 计算方式：每条边 isqrt64((dₐ-1)·(d_b-1)·10¹²)（下取整误差 ≤ 1 ppm）
- 不变量：对 Δ-正则图 RRR = m(Δ-1)×10⁶（精确：isqrt((Δ-1)²) = Δ-1）
- 当且仅当所有边均为悬挂边（dₐ=1 或 d_b=1）时 RRR = 0

---

## 交叉核对表

| 图 | EM₁ | ABS_ppm | RRR_ppm | \|E\| | \|V\| |
|-------|-----|---------|---------|-----|-----|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| 单边 A→B（da=db=1） | 0 | 0 | 0 | 1 | 2 |
| 路径 P₃ | 2 | 1_154_700 | 0 | 2 | 3 |
| 三角形 K₃（Δ=2） | 12 | 2_121_318 | 3_000_000 | 3 | 3 |
| 星图 K_{1,4} | 36 | 3_098_384 | 0 | 4 | 5 |
| 路径 P₄ | 6 | 1_861_806 | 1_000_000 | 3 | 4 |
| 完全图 K₄（Δ=3） | 96 | 4_898_976 | 12_000_000 | 6 | 4 |
| 两个孤立节点 | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | 54 | 4_647_576 | 8_485_278 | 6 | 5 |

### 关键 isqrt64 数值
- isqrt64(333_333_333_333) = 577_350（√(1/3) × 10⁶；每边 s=3,q=1）
- isqrt64(500_000_000_000) = 707_106（√(1/2) × 10⁶；每边 s=4,q=2）
- isqrt64(600_000_000_000) = 774_596（√(3/5) × 10⁶；每边 s=5,q=3）
- isqrt64(666_666_666_666) = 816_496（√(2/3) × 10⁶；每边 s=6,q=4）
- isqrt64(1_000_000_000_000) = 1_000_000（√1 × 10⁶；(da-1)(db-1)=1）
- isqrt64(2_000_000_000_000) = 1_414_213（√2 × 10⁶；(da-1)(db-1)=2）
- isqrt64(4_000_000_000_000) = 2_000_000（精确：√4=2；(da-1)(db-1)=4）

---

## 实现细节

**算法**（`graph_topo_indices6_inner`）：O(V+E) 单趟无向边扫描。
- 三个指数在同一趟扫描中计算完成（与既有指数相同的 a < b 规范化去重方式）
- 无需特判分支：q=0 或 p1/p2=0 时 isqrt64(0)=0 自然成立
- EM₁ 为精确整数累加；ABS 与 RRR 使用牛顿-拉夫逊 isqrt64

**溢出安全性：**
- EM₁ 贡献：q² ≤ 252² = 63504；|E| ≤ 512 项之和：最大约 32M → 在 u64 范围内
- ABS 分子：q×10¹² ≤ 252×10¹² < u64::MAX ✓
- RRR：p1×p2×10¹² ≤ 126×126×10¹² = 1.59×10¹⁶ < u64::MAX ✓

---

## 变更文件

| 文件 | 变更内容 |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices6_inner` 方法 + `graph_topo_indices6` 公开函数 |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices6`，带彩色显示 |
| `crates/k-shell/src/proc.rs` | 新增 "graph topo6"/"gtopo6" 及 8 个别名的路由 |
| `host-tests/gos-graph-topo6-harness/` | 新建 harness：Cargo.toml + .cargo/config.toml + Cargo.lock + 10 项测试 |

---

## Shell 命令

```
graph topo6              (主命令)
gtopo6                   (简称别名)
reformulated zagreb      (EM₁ 按名称调用)
gem1                     (EM₁ 指数)
atom bond sum            (ABS 按名称调用)
gabs                     (ABS 指数)
reduced reciprocal randic (RRR 按名称调用)
grrr                     (RRR 指数)
gem1absrrr               (组合命令)
```

---

## 操作系统类比

- **EM₁**：每条 IPC 通道的平方超额耦合压力——衡量枢纽连接超出悬挂阈值的程度；对全叶子拓扑为 0，随枢纽度数增长而增大
- **ABS**：原子-键耦合广度比——非对称链路利用率的归一化指标；对高度数网格逼近每边 √(1/2)
- **RRR**：内部耦合几何密度——只要图中每条边都触及叶子节点即为 0；对网格图精确等于 m(Δ-1)×10⁶；衡量越过叶子层的内部连接"深度"

---

## 测试结果

```
test test_01_empty         ... ok
test test_02_single_node   ... ok
test test_03_single_edge   ... ok
test test_04_path_p3       ... ok
test test_05_triangle_k3   ... ok
test test_06_star_k14      ... ok
test test_07_path_p4       ... ok
test test_08_complete_k4   ... ok
test test_09_two_isolated  ... ok
test test_10_k23_cross_check ... ok

test result: ok. 10 passed; 0 failed; 0 ignored
```

**宿主测试套件总计：1143 个测试**（较 V3.16 的 1133 个新增 10 个）

---

## VectorAddress L4 命名空间（更新）

```
88=graph-topo   (SC + GA + AZI, V3.12)
89=graph-topo2  (H + ABC + F, V3.13)
90=graph-topo3  (SDD + ISI + NI, V3.14)
91=graph-topo4  (SO + RM2 + Sigma, V3.15)
92=graph-topo5  (HM1 + HM2 + AG, V3.16)
93=graph-topo6  (EM1 + ABS + RRR, V3.17)  ← 新增
```

---

*本文件由 doc/ 根目录同名英文原始存档（`HARDENING_LOG_2026-07-06_V3.17.md`）翻译归位而来，仅译语言，不改动版本号 / 文件路径 / 函数名 / 测试名 / 测试计数等既成事实。*
