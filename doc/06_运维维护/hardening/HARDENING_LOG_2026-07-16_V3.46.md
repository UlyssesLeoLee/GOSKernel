# HARDENING LOG — V3.46
**Date**: 2026-07-16  
**Branch**: feat/vk-auto-live-surface  
**Author**: Automated hardening task (Claude Sonnet 4.6)

---

## 变更摘要

新增三个 S-变体拓扑指数族（topo35）：**NNC**、**NHOC**、**NHSO**，继续延伸 V3.45 的幂次多项式序列，并引入 S-六次 Sombor 指数（精确整数，无需 isqrt）。所有三个指数采用纯整数运算（u128 累加器），无浮点、无 isqrt，算法效率最优。同步新增 `gos-graph-topo35-harness`（10 项测试，全部通过）。

---

## 新增指数定义

### NNC — Neighborhood Nonic Index（顶点九次幂）
```
NNC(G) = Σ_{v∈V} S(v)^9
```
- S(v) = Σ_{w∈N(v)} deg(w)（邻居度之和）
- 返回类型：`u64`（u128 累加器，饱和截断）
- 延续 S-幂次顶点序列：NM₁=Σ S²→NF=Σ S³→NVQ=Σ S⁴→NPS=Σ S⁵→NSH=Σ S⁶→NSHP=Σ S⁷→NOC=Σ S⁸→NNC=Σ S⁹
- NNC = n·S⁹ for S-regular
- 实现：`s4 = s*s*s*s; s8 = s4*s4; s9 = s8.saturating_mul(s)`（3次 u128 乘法）

### NHOC — Neighborhood Hyper-Octic-Sum（边八次幂）
```
NHOC(G) = Σ_{uv∈E} (S_u + S_v)^8
```
- 返回类型：`u64`（u128 累加器，饱和截断）
- 延续 S-幂次边序列：NHM1→NHCS→NHQS→NHPS→NHSE→NHHS→NHOC=Σ(S+S)⁸
- NHOC = |E|·(2S)⁸ = 256|E|·S⁸ for S-regular
- 实现：`ss2 = ss*ss; ss4 = ss2*ss2; ss8 = ss4.saturating_mul(ss4)`（3次 u128 乘法）

### NHSO — Neighborhood Hextic Sombor Index（S-六次 Sombor）
```
NHSO(G) = Σ_{uv∈E} (S_u² + S_v²)^3
```
- 返回类型：`u64`（u128 累加器，饱和截断）
- Sombor SO^α 系列（S-variant，α=6）：NSO(topo21,α=1) → NCSO(topo33,α=3) → NFSO(topo34,α=4) → NHSO(topo35,α=6)
- **α=6 时为精确整数，无需 isqrt**（整数立方）
- NHSO = |E|·(2S²)³ = 8|E|·S⁶ for S-regular
- 实现：`s2s = sa*sa + sb*sb; s2s3 = s2s.saturating_mul(s2s.saturating_mul(s2s))`（2次 u128 乘法）

---

## 算法特性

| 特性 | NNC | NHOC | NHSO |
|------|-----|------|------|
| 复杂度 | O(V+E) | O(V+E) | O(V+E) |
| 精度 | 精确整数 | 精确整数 | 精确整数 |
| 累加器 | u128 | u128 | u128 |
| isqrt | 无 | 无 | 无（α=6 无需）|
| BFS | 无 | 无 | 无 |

三个指数均在同一遍 S(v) 扫描中计算，无额外分配。

---

## 交叉验证表

| 图 | NNC(精确) | NHOC(精确) | NHSO(精确) | 边数 | 节点数 |
|----|-----------|------------|------------|------|--------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| K₂ | 2 | 256 | 8 | 1 | 2 |
| P₃ | 1_536 | 131_072 | 1_024 | 2 | 3 |
| K₃ | 786_432 | 50_331_648 | 98_304 | 3 | 3 |
| K_{1,4} | 1_310_720 | 67_108_864 | 131_072 | 4 | 5 |
| P₄ | 40_390 | 2_460_866 | 10_226 | 3 | 4 |
| K₄ | 1_549_681_956 | 66_119_763_456 | 25_509_168 | 6 | 4 |
| 两孤立点 | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | 50_388_480 | 2_579_890_176 | 2_239_488 | 6 | 5 |

---

## S-regular 公式验证

- **NNC** = n·S⁹ for S-regular ✓
- **NHOC** = |E|·(2S)⁸ = 256·|E|·S⁸ for S-regular ✓
- **NHSO** = |E|·(2S²)³ = 8·|E|·S⁶ for S-regular ✓

K₃/K_{1,4} S=4 一致性：每条边的 NHOC 和 NHSO 相同（均为 16_777_216 和 32_768）；总量因边数不同而相差。

---

## 测试结果

新增 `gos-graph-topo35-harness`（10 项宿主测试）：

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

---

## Shell 命令

新增 k-shell 路由：
```
graph topo35 | gtopo35 | neighborhood nonic | gnnc
neighborhood octic edge | gnhoc | neighborhood hextic sombor | gnhso
gnncnhocnhso
```

---

## VectorAddress 命名空间

- L4=122：`gos-graph-topo35-harness`（topo35，本版本新增）
- L4 命名空间累计：88=graph-topo 至 122=graph-topo35

---

## 关键指标

| 指标 | 数值 |
|------|------|
| 宿主测试总数 | 1433（较 V3.45 +10）|
| 新增文件 | 3（Cargo.toml、.cargo/config.toml、tests/graph_topo35.rs）|
| 版本号 | V3.46 |
