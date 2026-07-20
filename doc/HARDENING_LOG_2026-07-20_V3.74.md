# HARDENING_LOG_2026-07-20_V3.74

**版本**: V3.74  
**日期**: 2026-07-20  
**分支**: feat/vk-auto-live-surface  
**主题**: NHEPTATRIACTC + NHHEPTATRIACTC + NAFSO Neighborhood S-variant topological indices + gos-graph-topo63-harness (10 tests)

---

## 概述

本次硬化引入三个新的邻域 S 变体拓扑指数，均基于邻域度和 S(v) = Σ_{w∈N(v)} deg(w) 构建。延续 topo18–topo62 家族，迈向第三十七次幂；Sombor 3rd-pass 系列推进至 AF（α=62）。

---

## 新增拓扑指数

### NHEPTATRIACTC(G) = Σ_v S(v)^37
- **类型**: S 变体顶点幂次求和（Heptatriacontic = 37 次方）
- **实现**: `s^37 = s32 × s4 × s`，其中 `s32 = s16^2`（完全平方），`s4 = s2^2`
- **S 正则公式**: `NHEPTATRIACTC = n · S^37`
- **精度**: u128 饱和累加器 → 截断至 u64::MAX（精确）

### NHHEPTATRIACTC(G) = Σ_{uv∈E} (S_u + S_v)^36
- **类型**: S 变体边幂次求和（Hexatriacontic edge-sum = 36 次方）
- **实现**: `ss^36 = ss32 × ss4`，其中 `ss32 = ss16^2`（完全平方），`ss4 = ss2^2`（完全平方）
- **S 正则公式**: `NHHEPTATRIACTC = |E| · (2S)^36 = 68_719_476_736 · |E| · S^36`
- **精度**: u128 饱和累加器（精确）

### NAFSO(G) = Σ_{uv∈E} (S_u² + S_v²)^31
- **类型**: S 变体广义 Sombor 指数，α = 62（3rd-pass AF）
- **实现**: `s2s^31 = s2s16 × s2s8 × s2s4 × s2s2 × s2s`（无 isqrt，全 1 二进制分解）
- **S 正则公式**: `NAFSO = |E| · (2S²)^31 = 2_147_483_648 · |E| · S^62`
- **精度**: u128 饱和累加器（精确，整数幂无需开方）

---

## Sombor 3rd-pass 系列进展

```
NAASO(α=52, topo58) → NABSO(α=54, topo59) → NACSO(α=56, topo60)
→ NADSO(α=58, topo61) → NAESO(α=60, topo62) → NAFSO(α=62, topo63)
```
当前双字母进度：AA → AB → AC → AD → AE → **AF**

---

## 精确测试值

| 图 | NHEPTATRIACTC | NHHEPTATRIACTC | NAFSO | 边 | 节点 |
|----|------|------|------|------|------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| K₂ | 2 | 68_719_476_736 | 2_147_483_648 | 1 | 2 |
| P₃ | 412_316_860_416 | u64::MAX | u64::MAX | 2 | 3 |
| K₃ | u64::MAX | u64::MAX | u64::MAX | 3 | 3 |
| K_{1,4} | u64::MAX | u64::MAX | u64::MAX | 4 | 5 |
| P₄ | 900_568_086_659_901_670 | u64::MAX | u64::MAX | 3 | 4 |
| K₄ | u64::MAX | u64::MAX | u64::MAX | 6 | 4 |
| 两孤立节点 | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | u64::MAX | u64::MAX | u64::MAX | 6 | 5 |

---

## P₄ 精确推导

```
S(A)=2, S(B)=3, S(C)=3, S(D)=2

NHEPTATRIACTC = 2×2^37 + 2×3^37
  3^16 = 43_046_721
  3^32 = 1_853_020_188_851_841
  3^36 = 3^32 × 81 = 150_094_635_296_999_121
  3^37 = 3 × 3^36 = 450_283_905_890_997_363
  2×3^37 = 900_567_811_781_994_726
  2×2^37 = 274_877_906_944
  总计   = 900_568_086_659_901_670  ✓

NHHEPTATRIACTC = (2+3)^36 + (3+3)^36 + (3+2)^36 = 2×5^36 + 6^36
  5^36 per-edge >> u64::MAX → 饱和

NAFSO = (4+9)^31 + (9+9)^31 + (9+4)^31 = 2×13^31 + 18^31
  13^16 < u64::MAX，但 13^17 > u64::MAX → 13^31 per-edge 饱和
```

---

## 实现要点

| 指数 | 幂次分解 | 注释 |
|------|----------|------|
| s^37 | s32 × s4 × s | 37=32+4+1；3步乘法 |
| ss^36 | ss32 × ss4 | 36=32+4；完全平方优化，2步乘法 |
| s2s^31 | s2s16 × s2s8 × s2s4 × s2s2 × s2s | 31=11111₂；全1分解，5步乘法 |

ss^36 比 ss^35（需要 ss32×ss2×ss，3步）更简洁，因为 36 可以分解为两个完全平方之积。

---

## VectorAddress L4 命名空间更新

```
88=graph-topo through 149=graph-topo62, 150=graph-topo63
```

---

## K-shell 命令

```
graph topo63 | gtopo63 | neighborhood heptatriacontic | gnheptatriactc
neighborhood hexatriacontic edge | gnnhheptatriactc
neighborhood hexahexacontyl sombor | gnafso
gnheptatriactcnhheptatriactcnafso
```

---

## 测试套件状态

- **新增**: gos-graph-topo63-harness — 10 tests（全部通过）
- **历史累计**: 1713 tests（V3.73 为 1703，本次 +10）
- **Plugin**: TOPIX_63  |  **Executor**: t63.exec
