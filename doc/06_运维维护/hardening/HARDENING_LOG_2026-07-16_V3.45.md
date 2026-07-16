# HARDENING LOG — V3.45
**Date**: 2026-07-16  
**Branch**: feat/vk-auto-live-surface  
**Author**: Automated hardening task (Claude Sonnet 4.6)

---

## 变更摘要

新增三个 S-变体拓扑指数族（topo34）：**NOC**、**NHHS**、**NFSO**，继续延伸 V3.44 的幂次多项式序列，并首次引入 S-四次 Sombor 指数（无需 isqrt，全精确整数）。所有三个指数采用纯整数运算（u128 累加器），无浮点、无 isqrt，算法效率最优。

---

## 新增指数定义

### NOC — Neighborhood Octic Index（顶点八次幂）
```
NOC(G) = Σ_{v∈V} S(v)^8
```
- S(v) = Σ_{w∈N(v)} deg(w)（邻居度之和）
- 返回类型：`u64`（u128 累加器，饱和截断）
- 延续 S-幂次顶点序列：NM₁=Σ S²→NF=Σ S³→NVQ=Σ S⁴→NPS=Σ S⁵→NSH=Σ S⁶→NSHP=Σ S⁷→NOC=Σ S⁸
- NOC = n·S⁸ for S-regular
- 实现：`s4 = s*s*s*s; s8 = s4*s4`（2次 u128 乘法，无分支）

### NHHS — Neighborhood Hyper-Hepta-Sum（边七次幂）
```
NHHS(G) = Σ_{uv∈E} (S_u + S_v)^7
```
- 返回类型：`u64`（u128 累加器，饱和截断）
- 延续 S-幂次边序列：NHM1→NHCS→NHQS→NHPS→NHSE→NHHS=Σ(S+S)⁷
- NHHS = |E|·(2S)⁷ = 128|E|·S⁷ for S-regular
- 实现：`ss7 = ss4 · ss2 · ss`（ss = S_u + S_v；3次 u128 乘法）

### NFSO — Neighborhood Fourth Sombor Index（S-四次 Sombor）
```
NFSO(G) = Σ_{uv∈E} (S_u² + S_v²)²
```
- 返回类型：`u64`（u128 累加器，饱和截断）
- Sombor SO^α 系列（S-variant，α=4）：NSO(topo21,α=1) → NCSO(topo33,α=3) → NFSO(topo34,α=4)
- **α=4 时为精确整数，无需 isqrt**（与 NSO/NCSO 不同）
- NFSO = |E|·(2S²)² = 4|E|·S⁴ for S-regular
- 实现：`s2s = sa*sa + sb*sb; nfso += s2s*s2s`（2次 u128 乘法）

---

## 算法特性

| 特性 | NOC | NHHS | NFSO |
|------|-----|------|------|
| 复杂度 | O(V+E) | O(V+E) | O(V+E) |
| 精度 | 精确整数 | 精确整数 | 精确整数 |
| 累加器 | u128 | u128 | u128 |
| isqrt | 无 | 无 | 无（α=4 无需）|
| BFS | 无 | 无 | 无 |

三个指数均在同一遍 S(v) 扫描中计算，无额外分配。

---

## 解析验证表

| 图形 | NOC | NHHS | NFSO | edges | nodes |
|------|-----|------|------|-------|-------|
| Empty | 0 | 0 | 0 | 0 | 0 |
| 1 node | 0 | 0 | 0 | 0 | 1 |
| K₂ | 2 | 128 | 4 | 1 | 2 |
| P₃ | 768 | 32_768 | 128 | 2 | 3 |
| K₃ | 196_608 | 6_291_456 | 3_072 | 3 | 3 |
| K_{1,4} | 327_680 | 8_388_608 | 4_096 | 4 | 5 |
| P₄ | 13_634 | 436_186 | 662 | 3 | 4 |
| K₄ | 172_186_884 | 3_673_320_192 | 157_464 | 6 | 4 |
| 2 isolated | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | 8_398_080 | 214_990_848 | 31_104 | 6 | 5 |

**K₃ 与 K_{1,4} 同源**（S-uniform S=4）：per-edge NHHS=2_097_152、NFSO=1_024 相同，NOC 因节点数不同而异（196_608 vs 327_680）。

---

## S-regular 公式验证

```
NOC  = n·S^8          （K₄: 4×9^8=172_186_884 ✓）
NHHS = 128·|E|·S^7    （K₄: 6×128×9^7=128×6×4_782_969=3_673_320_192 ✓）
NFSO = 4·|E|·S^4      （K₄: 4×6×9^4=4×6×6_561=157_464 ✓）
```

---

## 修改文件

| 文件 | 变更 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices34_inner()` + `pub fn graph_topo_indices34()` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices34()` |
| `crates/k-shell/src/proc.rs` | 新增 topo34 shell 命令路由 |
| `host-tests/gos-graph-topo34-harness/` | 新建 harness（10 测试用例） |

---

## Shell 命令

```
graph topo34 / gtopo34
neighborhood octic / gnoc
neighborhood septic edge / gnhhs
neighborhood fourth sombor / gnfso
gnocnhhsnfso
```

---

## VectorAddress 命名空间

L4=121 分配给 gos-graph-topo34-harness（TOPIX_34 / t34.exec）。  
**更新后命名空间**：88=graph-topo 至 121=graph-topo34（共 34 个图拓扑指数族）。

---

## 测试结果

- 新增测试：10（gos-graph-topo34-harness）
- 累积主机测试：**1423**（含本次 10 条）
- 所有测试通过 ✓

---

## OS 类比

| 指数 | 图论 OS 语义 |
|------|------------|
| NOC | S-octic 节点压力指数（8次幂极化放大高负载 IPC hub）|
| NHHS | S-septic 通道耦合压力（7次幂边聚合；S-uniform 时 = 128|E|S⁷）|
| NFSO | S-fourth Sombor 几何耦合强度（平方后的欧几里得 S-范数平方；|E|=0 时 = 0）|

三者协同形成图论 OS 拓扑健康度的高次多项式视角：NOC 捕获顶点 S-极端非均匀性，NHHS 捕获边级 S-耦合强度，NFSO 提供精确的几何 Sombor 度量（无浮点误差）。
