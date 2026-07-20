# 强化日志 V3.71 — NTETRTRIACTC + NHTETRTRIACTC + NACSO 邻域 S-变体拓扑指数

**日期**: 2026-07-19  
**版本**: V3.71  
**分支**: feat/vk-auto-live-surface  

---

## 概述

新增三项邻域 S-变体拓扑指数（topo60 家族），延续 V3.70（topo59）的 S-幂次系列：

| 指数 | 定义 | 类型 |
|------|------|------|
| NTETRTRIACTC | Σ_v S(v)^34 | S-第34次幂顶点和（S-Tetratriacontic） |
| NHTETRTRIACTC | Σ_{uv∈E} (S_u+S_v)^33 | S-第33次幂边和（S-Tritriacontic） |
| NACSO | Σ_{uv∈E} (S_u²+S_v²)^28 | S-变体广义 Sombor 指数（α=56，双字母 AC） |

其中 S(v) = Σ_{w∈N(v)} deg(w) 为邻居度数和（S-变体）。

---

## 数学定义

### NTETRTRIACTC(G) — S-第34次幂顶点和

```
NTETRTRIACTC(G) = Σ_v S(v)^34
```

- **S-规则图公式**: NTETRTRIACTC = n·S^34
- **实现**: s^34 = s^32 × s^2 = s16 × s16 × s2
- **溢出处理**: 饱和 u128 累加器 → 截断至 u64::MAX

延伸 S-幂次顶点系列：NM₁(topo18) → ... → NTRITRIACTC=ΣS³³(topo59) → **NTETRTRIACTC=ΣS³⁴(topo60)**

### NHTETRTRIACTC(G) — S-第33次幂边和

```
NHTETRTRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^33
```

- **S-规则图公式**: NHTETRTRIACTC = |E|·(2S)^33 = 8_589_934_592·|E|·S^33
- **实现**: ss^33 = ss^32 × ss = ss16 × ss16 × ss
- **溢出处理**: 饱和 u128 累加器 → 截断至 u64::MAX

延伸 S-幂次边系列：NHM1(topo23) → ... → NHTRITRIACTC=Σ(S+S)³²(topo59) → **NHTETRTRIACTC=Σ(S+S)³³(topo60)**

### NACSO(G) — S-变体五六 Sombor 指数（α=56）

```
NACSO(G) = Σ_{uv∈E} (S_u²+S_v²)^28
```

- **S-规则图公式**: NACSO = |E|·(2S²)^28 = 268_435_456·|E|·S^56
- **实现**: s2s^28 = s2s^16 × s2s^8 × s2s^4
- **精确整数**（无 isqrt，(S²+S²)^28 无分数幂）
- **命名**: 3rd-pass 双字母系列 AC（NAASO α=52 → NABSO α=54 → **NACSO α=56**）

α 系列完整路径：NSO(α=1) → ... → NAASO(α=52,topo58) → NABSO(α=54,topo59) → **NACSO(α=56,topo60)**

---

## 关键测试值

| 图 | NTETRTRIACTC | NHTETRTRIACTC | NACSO | 边数 | 节点数 |
|----|-------------|--------------|-------|------|--------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| K₂ (S=1) | 2 | 8_589_934_592 | 268_435_456 | 1 | 2 |
| P₃ (S=2) | 51_539_607_552 | u64::MAX(饱和) | u64::MAX(饱和) | 2 | 3 |
| K₃ (S=4) | u64::MAX(饱和) | u64::MAX(饱和) | u64::MAX(饱和) | 3 | 3 |
| K_{1,4} (S=4) | u64::MAX(饱和) | u64::MAX(饱和) | u64::MAX(饱和) | 4 | 5 |
| P₄ (混合) | 33_354_397_759_071_506 | u64::MAX(饱和) | u64::MAX(饱和) | 3 | 4 |
| K₄ (S=9) | u64::MAX(饱和) | u64::MAX(饱和) | u64::MAX(饱和) | 6 | 4 |
| K_{2,3} (S=6) | u64::MAX(饱和) | u64::MAX(饱和) | u64::MAX(饱和) | 6 | 5 |

**K₂ 精确值**：
- NTETRTRIACTC = 2×1^34 = 2 ✓
- NHTETRTRIACTC = (1+1)^33 = 2^33 = 8_589_934_592 ✓
- NACSO = (1+1)^28 = 2^28 = 268_435_456 ✓

**P₃ 推导**（S=2 均匀）：
- NTETRTRIACTC = 3×2^34 = 3×17_179_869_184 = 51_539_607_552 ✓
- NHTETRTRIACTC = 2×4^33 = 2×2^66 > u64::MAX → 饱和 ✓
- NACSO = 2×8^28 = 2×2^84 > u64::MAX → 饱和 ✓

**P₄ 推导**（S(A)=2, S(B)=3, S(C)=3, S(D)=2）：
- 3^32 = 1_853_020_188_851_841; 3^34 = 3^32×9 = 16_677_181_699_666_569
- NTETRTRIACTC = 2×2^34 + 2×3^34 = 34_359_738_368 + 33_354_363_399_333_138 = 33_354_397_759_071_506 ✓

---

## 实现细节

**运行时函数**: `gos_runtime::graph_topo_indices60()` → `(ntetrtriactc, nhtetrtriactc, nacso, edge_count, node_count)`

**内部方法**: `GoSRuntime::graph_topo_indices60_inner()`

**Shell 命令**:
```
graph topo60 / gtopo60 / neighborhood tetratriacontic / gntetrtriactc
neighborhood tritriacontic edge / gnhtetrtriactc
neighborhood hexapentacontyl sombor / gnnacso
gntetrtriactcnhtetrtriactcnacso
```

**VectorAddress L4 命名空间**: L4=147（gos-graph-topo60-harness）

**插件/执行器**: TOPIX_60 / t60.exec

---

## 变更文件

| 文件 | 变更 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices60_inner()` + `graph_topo_indices60()` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices60()` |
| `crates/k-shell/src/proc.rs` | 新增 shell 命令路由 |
| `host-tests/gos-graph-topo60-harness/` | 新建测试线束（10 项测试） |

---

## 测试结果

```
running 10 tests
test test_01_empty ... ok
test test_02_single_node ... ok
test test_03_k2_edge ... ok
test test_04_path_p3 ... ok
test test_05_triangle_k3 ... ok
test test_06_star_k14 ... ok
test test_07_path_p4 ... ok
test test_08_complete_k4 ... ok
test test_09_two_isolated ... ok
test test_10_k23_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

**累计宿主测试数**: 1683（前 V3.70: 1673，新增 10）

---

## VectorAddress L4 命名空间（更新）

88=graph-topo 至 146=graph-topo59，**147=graph-topo60**
