# GOSKernel 强化日志 V3.39 — 2026-07-16

## 摘要

新增 **NNI + NNMI + NSM1** Neighborhood Nirmala S-variant 拓扑指数，作为 `gos-graph-topo28-harness`（L4=115）。延续 V3.29–V3.38 建立的 Neighborhood S-index 系列扩展。

## 版本信息

- **版本**：V3.39
- **分支**：feat/vk-auto-live-surface
- **日期**：2026-07-16
- **此前测试数**：1353（V3.38）
- **新测试数**：1363（+10）

## 新增指数：NNI + NNMI + NSM1（gos-graph-topo28）

函数签名：`gos_runtime::graph_topo_indices28() -> (nni_ppm: u64, nnmi_ppm: u64, nsm1: u64, edge_count: usize, node_count: usize)`

S(v) = Σ_{w∈N(v)} deg(w) = 邻居度数和（与 topo18/topo21–topo28 族相同的 S 定义）

- **nni_ppm**  = NNI(G) × 10^6  = Σ_{uv∈E} isqrt64((S_u+S_v)×10^12)              （向下取整 ppm；S-Nirmala）
- **nnmi_ppm** = NNMI(G) × 10^6 = Σ_{uv∈E} (S_u+S_v)×isqrt64((S_u+S_v)×10^12)   （向下取整 ppm；S-Modified Nirmala）
- **nsm1**     = NSM1(G)         = Σ_{uv∈E} (S_u+S_v)                             （精确 u64；S-edge M₁ = Σ_v S(v)·deg(v)）

### 定义

- NNI(G)  = Σ_{uv∈E} √(S_u+S_v)            — Nirmala 指数 N 的 S-模拟量（Nirmala, Mathad & Usha 2021）
- NNMI(G) = Σ_{uv∈E} (S_u+S_v)^{3/2}       — 修正 Nirmala 指数 N* 的 S-模拟量（Kumar et al. 2022）
- NSM1(G) = Σ_{uv∈E} (S_u+S_v)             — M₁ 边形式的 S-模拟量（= Σ_v S(v)·deg(v)）

### 关键恒等式

每边 NNMI = (S_u+S_v) × 每边 NNI
因为当 (S_u+S_v)∈ℤ 时，floor((S_u+S_v)^{3/2}×10^6) = (S_u+S_v)×floor(√(S_u+S_v)×10^6)。
这意味着 NNMI 与 NNI 共用同一次 isqrt64 计算 —— 单个 `nni_e` 值同时服务两者。

### 关键不变量

- 对 S-正则图（所有 S 相等）：NNI  = |E|·√(2S)·10^6
- 对 S-正则图：NNMI = |E|·(2S)^{3/2}·10^6
- 对 S-正则图：NSM1 = 2|E|·S
- K₃ 与 K_{1,4}：均为 S-均匀 S=4 → 每边 NNI、NNMI 相同；总值因边数比例不同而不同
- 空图上三个指数均为零；NSM1 为精确值（无近似）

### 交叉验证表

| 图      | NNI (ppm)  | NNMI (ppm)  | NSM1 | 边数 | 点数 |
|------------|------------|-------------|------|-------|-------|
| 空图      | 0          | 0           | 0    | 0     | 0     |
| 单节点     | 0          | 0           | 0    | 0     | 1     |
| K₂         | 1_414_213  | 2_828_426   | 2    | 1     | 2     |
| P₃         | 4_000_000  | 16_000_000  | 8    | 2     | 3     |
| K₃         | 8_485_281  | 67_882_248  | 24   | 3     | 3     |
| K_{1,4}    | 11_313_708 | 90_509_664  | 32   | 4     | 5     |
| P₄         | 6_921_623  | 37_057_604  | 16   | 3     | 4     |
| K₄         | 25_455_840 | 458_205_120 | 108  | 6     | 4     |
| 两孤立点 | 0          | 0           | 0    | 0     | 2     |
| K_{2,3}    | 20_784_606 | 249_415_272 | 72   | 6     | 5     |

### 算法

O(V+E) — 邻接+度数遍历 → S(v) 计算 → 边扫描（a<b）；仅使用 isqrt64；无需 BFS，无需 u128。
每边仅一次 isqrt64 调用即可算出 NNI_e；NNMI_e = ssum × NNI_e 复用该值。

### 溢出安全性

- ssum×10^12：最大 ssum=32258（K₁₂₈）；32258×10^12 = 3.23×10^16 < u64::MAX ✓
- NNMI 累加器：≤8128 条边 × 32258 × 179_606_381 ≈ 4.71×10^16 < u64::MAX ✓
- NSM1：≤8128 × 32258 ≈ 2.62×10^8 << u64::MAX ✓

### Shell 命令

"graph topo28" / "gtopo28" / "neighborhood nirmala" / "gnni" / "neighborhood modified nirmala" / "gnnmi" / "neighborhood sm1" / "gnsm1" / "gnninnminsm1"

### VectorAddress L4

gos-graph-topo28-harness 的 L4=115

### 参考文献

Nirmala, Mathad & Usha 2021（Nirmala 指数 N）
Kumar et al. 2022（修正 Nirmala 指数 N*）
（S-variant 系列）

## 变更文件

- `crates/gos-runtime/src/lib.rs`：新增 `graph_topo_indices28_inner()` + `graph_topo_indices28()` 公共函数
- `host-tests/gos-graph-topo28-harness/Cargo.toml`：新建 workspace
- `host-tests/gos-graph-topo28-harness/.cargo/config.toml`：宿主目标覆盖配置
- `host-tests/gos-graph-topo28-harness/tests/graph_topo28.rs`：10 项测试（全部通过）

## 测试结果

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
