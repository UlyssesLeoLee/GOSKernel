# GOSKernel 强化日志 V3.51 — 2026-07-16

## 摘要

新增三个 S-variant Neighborhood 拓扑指数 —— NQTC、NHTC、NGSO，以及
`gos-graph-topo40-harness`（10 项测试）。宿主测试套件现总计 **1483 个测试**。

## 新增指数：NQTC + NHTC + NGSO（S-variant 家族，topo40）

### 数学定义

设 S(v) = Σ_{w∈N(v)} deg(w)（邻居度数和，"S-variant"）。

| 指数 | 公式 | 类型 | 所属序列 |
|-------|---------|------|--------|
| NQTC | Σ_v S(v)^14 | S-十四次方顶点和 | 扩展 NTC=Σ S¹³（topo39） |
| NHTC | Σ_{uv∈E} (S_u+S_v)^13 | S-十三次方边和 | 扩展 NHDOC=Σ(S+S)¹²（topo39） |
| NGSO | Σ_{uv∈E} (S_u²+S_v²)^8 | S-十六次方 Sombor α=16 | 扩展 NESO=Σ(S²+S²)⁷（topo39） |

### S-正则公式

- NQTC = n·S^14                        （对 S-正则图）
- NHTC = |E|·(2S)^13 = 8192|E|·S^13   （对 S-正则图）
- NGSO = |E|·(2S²)^8 = 256|E|·S^16    （对 S-正则图）

### 交叉验证表

| 图 | NQTC | NHTC | NGSO | 边数 | 点数 |
|-------|------|------|------|-------|-------|
| K₂ | 2 | 8_192 | 256 | 1 | 2 |
| P₃ | 49_152 | 134_217_728 | 33_554_432 | 2 | 3 |
| K₃ | 805_306_368 | 1_649_267_441_664 | 3_298_534_883_328 | 3 | 3 |
| K_{1,4} | 1_342_177_280 | 2_199_023_255_552 | 4_398_046_511_104 | 4 | 5 |
| P₄ | 9_598_706 | 15_502_100_266 | 12_651_422_018 | 3 | 4 |
| K₄ | 91_507_169_819_844 | 124_937_789_194_027_008 | 2_846_239_010_076_427_776 | 6 | 4 |
| K_{2,3} | 391_820_820_480 | 641_959_232_274_432 | 4_333_224_817_852_416 | 6 | 5 |

### 实现要点

- 三者均使用带饱和运算的 u128 累加器；全程无需 isqrt（均为精确整数）
- NQTC：s^14 = s^8 × s^4 × s^2（s8.saturating_mul(s4).saturating_mul(s2)）
- NHTC：ss^13 = ss^8 × ss^4 × ss（ss8.saturating_mul(ss4).saturating_mul(ss)）
- NGSO：s2s^8 = (s2s^4)^2（s2s4.saturating_mul(s2s4)）
- NGSO 为精确整数，因为 (S_u²+S_v²)^8 不含分数次幂
- K₃ 与 K_{1,4} 共享 S=4 → 每边 NHTC、NGSO 相同；NQTC 因节点数不同而不同
- gos-graph-topo40-harness 的 VectorAddress L4=127；插件 TOPIX_40；执行器 t40.exec

### 关键数值

- K₂（S=1）：NQTC=2，NHTC=2^13=8_192，NGSO=2^8=256
- P₃（S=2）：NQTC=3×2^14=49_152，NHTC=2×4^13=134_217_728，NGSO=2×8^8=33_554_432
- K₃（S=4）：4^14=268_435_456；8^13=549_755_813_888；32^8=1_099_511_627_776
- K₄（S=9）：9^14=22_876_792_454_961；18^13=20_822_964_865_671_168；162^8=474_373_168_346_071_296
- K_{2,3}（S=6）：6^14=78_364_164_096；12^13=106_993_205_379_072；72^8=722_204_136_308_736

### Shell 命令

```
graph topo40 | gtopo40
neighborhood tetradecic | gnqtc
neighborhood tridecic edge | gnhtc
neighborhood hexadecic sombor | gngso
gnqtcnhtcngso
```

## 变更文件

| 文件 | 变更内容 |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices40_inner()` 方法 + 公共函数 `graph_topo_indices40()` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices40()` |
| `crates/k-shell/src/proc.rs` | 新增 topo40 路由分支 |
| `host-tests/gos-graph-topo40-harness/` | 新建 harness（Cargo.toml、.cargo/config.toml、tests/graph_topo40.rs） |
| `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.51.md` | 本篇日志 |

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

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

## 累计状态

- **版本**：V3.51
- **分支**：feat/vk-auto-live-surface
- **宿主测试套件总计**：1483 个测试（截至 V3.50 的 1473 个 + 新增 10 个）
- **VectorAddress L4 命名空间**：88=graph-topo 至 127=graph-topo40
- **S-variant 幂次-顶点序列**：NM₁(2)→NF(3)→NVQ(4)→NPS(5)→NSH(6)→NSHP(7)→NOC(8)→NNC(9)→NDC(10)→NUC(11)→NDoC(12)→NTC(13)→NQTC(14)
- **S-variant 幂次-边序列**：NHM1(2)→NHCS(3)→NHQS(4)→NHPS(5)→NHSE(6)→NHHS(7)→NHOC(8)→NHNC(9)→NHDC(10)→NHUC(11)→NHDOC(12)→NHTC(13)
- **S-variant Sombor α-序列**：NSO(1)→NCSO(3)→NFSO(4)→NHSO(6)→NOSO(8)→NTSO(10)→NDSO(12)→NESO(14)→NGSO(16)
