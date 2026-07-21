# GOSKernel 强化日志 V3.50 — 2026-07-16

## 摘要

新增三个 S-variant Neighborhood 拓扑指数 —— NTC、NHDOC、NESO，以及
`gos-graph-topo39-harness`（10 项测试）。宿主测试套件现总计 **1473 个测试**。

## 新增指数：NTC + NHDOC + NESO（S-variant 家族，topo39）

### 数学定义

设 S(v) = Σ_{w∈N(v)} deg(w)（邻居度数和，"S-variant"）。

| 指数 | 公式 | 类型 | 所属序列 |
|-------|---------|------|--------|
| NTC | Σ_v S(v)^13 | S-十三次方顶点和 | 扩展 NDoC=Σ S¹²（topo38） |
| NHDOC | Σ_{uv∈E} (S_u+S_v)^12 | S-十二次方边和 | 扩展 NHUC=Σ(S+S)¹¹（topo38） |
| NESO | Σ_{uv∈E} (S_u²+S_v²)^7 | S-十四次方 Sombor α=14 | 扩展 NDSO=Σ(S²+S²)⁶（topo38） |

### S-正则公式

- NTC   = n·S^13                       （对 S-正则图）
- NHDOC = |E|·(2S)^12 = 4096|E|·S^12  （对 S-正则图）
- NESO  = |E|·(2S²)^7 = 128|E|·S^14   （对 S-正则图）

### 交叉验证表

| 图 | NTC | NHDOC | NESO | 边数 | 点数 |
|-------|-----|-------|------|-------|-------|
| K₂ | 2 | 4_096 | 128 | 1 | 2 |
| P₃ | 24_576 | 33_554_432 | 4_194_304 | 2 | 3 |
| K₃ | 201_326_592 | 206_158_430_208 | 103_079_215_104 | 3 | 3 |
| K_{1,4} | 335_544_320 | 274_877_906_944 | 137_438_953_472 | 4 | 5 |
| P₄ | 3_205_030 | 2_665_063_586 | 737_717_066 | 3 | 4 |
| K₄ | 10_167_463_313_316 | 6_940_988_288_557_056 | 17_569_376_605_410_048 | 6 | 4 |
| K_{2,3} | 65_303_470_080 | 53_496_602_689_536 | 60_183_678_025_728 | 6 | 5 |

### 实现要点

- 三者均使用带饱和运算的 u128 累加器；全程无需 isqrt（均为精确整数）
- NTC：s^13 = s^8 × s^4 × s（均为 saturating_mul）
- NHDOC：ss^12 = ss^8 × ss^4（均为 saturating_mul）
- NESO：s2s^7 = s2s^4 × s2s^2 × s2s（均为 saturating_mul）
- NESO 为精确整数，因为 (S_u²+S_v²)^7 不含分数次幂
- gos-graph-topo39-harness 的 VectorAddress L4=126；插件 TOPIX_39；执行器 t39.exec

### Shell 命令

```
graph topo39 | gtopo39
neighborhood tridecic | gntc
neighborhood dodecic edge | gnhdoc
neighborhood tetradecic sombor | gneso
gntcnhdocneso
```

## 变更文件

| 文件 | 变更内容 |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices39_inner()` 方法 + 公共函数 `graph_topo_indices39()` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices39()` |
| `crates/k-shell/src/proc.rs` | 新增 topo39 路由分支 |
| `host-tests/gos-graph-topo39-harness/` | 新建 harness（Cargo.toml、.cargo/config.toml、tests/graph_topo39.rs） |
| `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.50.md` | 本篇日志 |

## 测试结果

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

## 累计状态

- **版本**：V3.50
- **分支**：feat/vk-auto-live-surface
- **宿主测试套件总计**：1473 个测试（截至 V3.49 的 1463 个 + 新增 10 个）
- **VectorAddress L4 命名空间**：88=graph-topo 至 126=graph-topo39
- **S-variant 幂次-顶点序列**：NM₁(2)→NF(3)→NVQ(4)→NPS(5)→NSH(6)→NSHP(7)→NOC(8)→NNC(9)→NDC(10)→NUC(11)→NDoC(12)→NTC(13)
- **S-variant 幂次-边序列**：NHM1(2)→NHCS(3)→NHQS(4)→NHPS(5)→NHSE(6)→NHHS(7)→NHOC(8)→NHNC(9)→NHDC(10)→NHUC(11)→NHDOC(12)
- **S-variant Sombor α-序列**：NSO(1)→NCSO(3)→NFSO(4)→NHSO(6)→NOSO(8)→NTSO(10)→NDSO(12)→NESO(14)
