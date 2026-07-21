# GOSKernel 强化日志 V3.41 — NVQ + NRGS + NHCS S-variant 拓扑指数

**日期：** 2026-07-16
**分支：** feat/vk-auto-live-surface
**提交：**（见 git log）
**宿主测试总计：** 1383（此前 1373 + 新增 10）

---

## 摘要

三个新的 S-variant 拓扑指数 —— NVQ、NRGS、NHCS，在 `gos_runtime` 中暴露为 `graph_topo_indices30()`，在 k-shell 中对应 `dispatch_graph_topo_indices30`，并由 `gos-graph-topo30-harness` 验证（10 项测试，全部通过）。

本次扩展了 S-variant 系列（topo18、topo21–topo30）的高阶幂次指数：NVQ 扩展顶点幂次序列（NM₁=Σ S²，NF=Σ S³ → NVQ=Σ S⁴），NRGS 是指数为 3/2 的广义 Randić 指数的 S-模拟量，NHCS 是 NHM₁=Σ(S+S)² 的三次方扩展。

---

## 新功能：`graph topo30` — NVQ + NRGS + NHCS

### 定义

其中 S(v) = Σ_{w∈N(v)} deg(w)（邻居度数和，"S-variant"）：

| 指数 | 公式 | 类型 | 说明 |
|-------|---------|------|-------------|
| **NVQ** | Σ_v S(v)⁴ | 精确 u64 | S-四次方顶点和；将 NM₁=Σ S² 与 NF=Σ S³ 扩展至四次方 |
| **NRGS** | Σ_{uv∈E} (S_u·S_v)^{3/2} | ppm（向下取整） | 指数 α=3/2 的 S-广义 Randić；χ_{3/2}(G) 的 S-模拟量 |
| **NHCS** | Σ_{uv∈E} (S_u+S_v)³ | 精确 u64 | S-三次方边和；将 NHM₁=Σ(S+S)²（topo23）扩展至三次方 |

### 实现

- **算法**：O(V+E) — 度数遍历 → S(v) 计算 → 顶点扫描（NVQ） + 边扫描（NRGS、NHCS）
- **无需 BFS**（与全部 topo18–topo30 属于同一 O(V+E) 复杂度类）
- **溢出安全性**：
  - NVQ：S(v)⁴ ≤ 16129⁴ ≈ 6.77×10^16 < u64::MAX；总和 ≤ 128 × 6.77×10^16 ≈ 8.67×10^18 < u64::MAX ✓
  - NRGS：`sp = (S_u·S_v) as u128; isqrt128(sp³×10^12)` —— 中间量 ≤ ~1.76×10^37 < u128::MAX ✓
  - NHCS：每边 (S_u+S_v)³ ≤ 32258³ ≈ 3.36×10^13；总和 ≤ 2.73×10^17 < u64::MAX ✓
- **返回值**：`(nvq: u64, nrgs_ppm: u64, nhcs: u64, edge_count: usize, node_count: usize)`

### Shell 命令

```
graph topo30 / gtopo30
neighborhood quartic / gnvq
neighborhood randic32 / gnrgs
neighborhood cubic sum / gnhcs
gnvqnrgsnhcs
```

### 关键不变量

- 对 S-正则图：NVQ = n·S⁴
- 对 S-正则图：NRGS = |E|·S³·10^6（当 S 为完全平方数时精确；NRGS = |E|·(S²)^{3/2}·10^6 = |E|·S³·10^6）
- 对 S-正则图：NHCS = 8·|E|·S³（因为 (2S)³ = 8S³）
- K₃ 与 K_{1,4}：均为 S-均匀 S=4 → 每边 NRGS（64_000_000）与 NHCS（512）相同；NVQ（768 对 1280）及总 NRGS/NHCS 因边数不同而不同
- K₄（S=9）：每边 NRGS = 729_000_000；每边 NHCS = 5832
- K_{2,3}（S=6）：每边 NRGS = 216_000_000；每边 NHCS = 1728

### S-正则性（常见测试图）

- K₂：S=1。P₃：S=2。K₃、K_{1,4}：S=4。K₄：S=9。K_{2,3}：S=6。P₄：混合（2,3,3,2）。

### 交叉验证表

| 图 | NVQ | NRGS (ppm) | NHCS | 边数 | 点数 |
|-------|-----|-----------|------|-------|-------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| K₂ | 2 | 1_000_000 | 8 | 1 | 2 |
| P₃ | 48 | 16_000_000 | 128 | 2 | 3 |
| K₃ | 768 | 192_000_000 | 1_536 | 3 | 3 |
| K_{1,4} | 1_280 | 256_000_000 | 2_048 | 4 | 5 |
| P₄ | 194 | 56_393_876 | 466 | 3 | 4 |
| K₄ | 26_244 | 4_374_000_000 | 34_992 | 6 | 4 |
| K_{2,3} | 6_480 | 1_296_000_000 | 10_368 | 6 | 5 |

### 推导要点

**P₄ NRGS 交叉验证**（混合 S=2,3,3,2）：
- {A,B}：(2·3)^{3/2}·10^6 = isqrt128(216·10^12) = 14_696_938（√216 = 14.6969...）
- {B,C}：(3·3)^{3/2}·10^6 = isqrt128(729·10^12) = 27_000_000（精确值：27²=729）
- {C,D}：同 {A,B} = 14_696_938
- 总计：56_393_876 ✓

**K₄ NRGS**（S=9 均匀）：9³ = 729；6 × 729_000_000 = 4_374_000_000 ✓

**K_{2,3} NRGS**（S=6 均匀）：6³ = 216；6 × 216_000_000 = 1_296_000_000 ✓

### 操作系统类比

- **NVQ** = 四阶邻域路由压力（相较三次方 NF，对高 S 枢纽中枢节点放大更明显）
- **NRGS** = 3/2 阶 S-几何平均通道耦合（介于线性 NM₂ 与二次方 NHM₂ 之间）
- **NHCS** = 三次方 S-边和压力（对非对称枢纽-辐条边取值高；S-正则时为 NHM₁ 的 8 倍）

---

## VectorAddress L4 命名空间（更新后）

……, 115=graph-topo28, 116=graph-topo29, **117=graph-topo30**

---

## 变更文件

- `crates/gos-runtime/src/lib.rs` — `graph_topo_indices30_inner` + `graph_topo_indices30()`
- `crates/k-shell/src/lib.rs` — `dispatch_graph_topo_indices30`
- `crates/k-shell/src/proc.rs` — topo30 路由（5 个别名）
- `host-tests/gos-graph-topo30-harness/` — 新建 harness（10 项测试，全部通过）
- `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.41.md` — 本篇日志
