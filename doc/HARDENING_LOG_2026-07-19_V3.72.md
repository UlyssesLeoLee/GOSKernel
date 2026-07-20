# GOSKernel Hardening Log — V3.72

**日期**: 2026-07-19  
**版本**: V3.72  
**分支**: feat/vk-auto-live-surface  
**提交**: 654224c  

---

## 本次变更摘要

新增邻域S-变体拓扑指标三元组 (topo61)：**NPENTTRIACTC + NHPENTTRIACTC + NADSO**，继续强化图论操作系统的图论计算核心。

---

## 新增内容

### 拓扑指标：NPENTTRIACTC + NHPENTTRIACTC + NADSO（topo61）

**数学定义**（S(v) = Σ_{w∈N(v)} deg(w) 为邻域度和）：

| 指标 | 公式 | 含义 |
|------|------|------|
| NPENTTRIACTC | Σ_v S(v)^35 | S-三十五次幂顶点和（Pentatriacontic） |
| NHPENTTRIACTC | Σ_{uv∈E} (S_u+S_v)^34 | S-三十四次幂边和（Tetratriacontic） |
| NADSO | Σ_{uv∈E} (S_u²+S_v²)^29 | S-变体广义Sombor指标 α=58（Octopentacontyl） |

**S正则图公式**：
- NPENTTRIACTC = n·S^35
- NHPENTTRIACTC = |E|·(2S)^34 = 17_179_869_184·|E|·S^34
- NADSO = |E|·(2S²)^29 = 536_870_912·|E|·S^58

**实现方法**（全部使用 u128 饱和累加器）：
- s^35 = s16 × s16 × s2 × s（s^32完全平方再乘s^2再乘s）
- ss^34 = ss16 × ss16 × ss2（ss^32完全平方再乘ss^2）
- s2s^29 = s2s16 × s2s8 × s2s4 × s2s（分解为16+8+4+1）

**Sombor字母序列进展**（3rd-pass双字母系列）：
- NAASO(α=52,topo58) → NABSO(α=54,topo59) → NACSO(α=56,topo60) → **NADSO(α=58,topo61)**

**典型图测试值**：

| 图 | NPENTTRIACTC | NHPENTTRIACTC | NADSO | 边数 | 点数 |
|----|-------------|--------------|-------|------|------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单点 | 0 | 0 | 0 | 0 | 1 |
| K₂ | 2 | 17_179_869_184 | 536_870_912 | 1 | 2 |
| P₃ | 103_079_215_104 | u64::MAX(饱和) | u64::MAX(饱和) | 2 | 3 |
| K₃ | u64::MAX(饱和) | u64::MAX(饱和) | u64::MAX(饱和) | 3 | 3 |
| K_{1,4} | u64::MAX(饱和) | u64::MAX(饱和) | u64::MAX(饱和) | 4 | 5 |
| P₄ | 100_063_158_917_476_150 | u64::MAX(饱和) | u64::MAX(饱和) | 3 | 4 |
| K₄ | u64::MAX(饱和) | u64::MAX(饱和) | u64::MAX(饱和) | 6 | 4 |
| 2孤立点 | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | u64::MAX(饱和) | u64::MAX(饱和) | u64::MAX(饱和) | 6 | 5 |

**P₄精确推导**（S(A)=2, S(B)=3, S(C)=3, S(D)=2）：
- 3^32 = 1_853_020_188_851_841
- 3^35 = 3^32 × 27 = 50_031_545_098_999_707
- 2×3^35 = 100_063_090_197_999_414
- 2^36 = 68_719_476_736
- NPENTTRIACTC(P₄) = 100_063_090_197_999_414 + 68_719_476_736 = **100_063_158_917_476_150** ✓

---

## 变更文件

### 运行时核心
- `crates/gos-runtime/src/lib.rs`
  - 新增 `graph_topo_indices61_inner()` 内部方法
  - 新增 `graph_topo_indices61()` 公共API

### Shell 层
- `crates/k-shell/src/lib.rs`
  - 新增 `dispatch_graph_topo_indices61()` 调度函数
- `crates/k-shell/src/proc.rs`
  - 新增路由：`"graph topo61"` / `"gtopo61"` / `"neighborhood pentatriacontic"` / `"gnpenttriactc"` / `"neighborhood tetratriacontic edge"` / `"gnhpenttriactc"` / `"neighborhood octopentacontyl sombor"` / `"gnadso"` / `"gnpenttriactcnhpenttriactcnadso"`

### 测试套件
- 新增 `host-tests/gos-graph-topo61-harness/`（10个测试，全部通过）
  - `Cargo.toml`
  - `.cargo/config.toml`
  - `tests/graph_topo61.rs`

---

## VectorAddress 命名空间

| L4 | 含义 |
|----|------|
| 88 | graph-topo |
| ... | ... |
| 147 | graph-topo60 |
| **148** | **graph-topo61（本版本新增）** |

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

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## 累计测试数量

| 版本 | 主机测试总数 |
|------|------------|
| V3.71 | 1683 |
| **V3.72** | **1693** |

---

## 与上一版本对比

| 项目 | V3.71 | V3.72 |
|------|-------|-------|
| topo 系列最高编号 | topo60 | **topo61** |
| S-Sombor 3rd-pass字母 | AC (α=56) | **AD (α=58)** |
| 主机测试总数 | 1683 | **1693** |
| VectorAddress L4最大值 | 147 | **148** |
