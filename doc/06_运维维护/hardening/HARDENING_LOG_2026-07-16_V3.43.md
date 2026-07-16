# HARDENING LOG — V3.43
**Date**: 2026-07-16  
**Branch**: feat/vk-auto-live-surface  
**Commit**: 92a4fbc  
**Author**: Automated hardening task (Claude Sonnet 4.6)

---

## 变更摘要

新增三个 S-变体拓扑指数族（topo32）：**NSH**、**NHPS**、**NWSO**，延续 V3.42 的幂次多项式序列，向图论 OS 拓扑感知子系统注入更丰富的结构描述能力。

---

## 新增指数定义

### NSH — Neighborhood S-Hextic Index
```
NSH(G) = Σ_{v∈V} S(v)^6
```
- S(v) = Σ_{w∈N(v)} deg(w)（邻居度之和）
- 返回类型：`u64`（u128 累加器，饱和截断）
- 顶点六次幂求和，比较图的高次结构差异

### NHPS — Neighborhood Hyperpentic Sombor（边五次幂）
```
NHPS(G) = Σ_{uv∈E} (S_u + S_v)^5
```
- 返回类型：`u64`（u128 累加器，饱和截断）
- 延续 NHQS（四次）的幂级数序列

### NWSO — Neighborhood Weighted Sombor Index
```
NWSO(G) = Σ_{uv∈E} S_u · S_v · √(S_u² + S_v²)
```
- 以 ppm 编码（× 10^6，floor）存储为 `u64`
- 纯整数计算恒等式：
  ```
  floor(S_u · S_v · √(S_u² + S_v²) · 10^6)
    = isqrt128(S_u² · S_v² · (S_u² + S_v²) · 10^12)
  ```
- 无浮点，no_std 安全

---

## 文件变更

| 文件 | 变更 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices32_inner()` + `graph_topo_indices32()` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices32()` 显示函数 |
| `crates/k-shell/src/proc.rs` | 注册 `graph topo32` / `gtopo32` / `gnsh` / `gnhps` / `gnwso` 等别名 |
| `host-tests/gos-graph-topo32-harness/` | 新建独立 Cargo workspace harness |

---

## 测试用例（10/10 通过）

VectorAddress L4=119，插件 `TOPIX_32`，执行器 `t32.exec`

| 测试 | 图类型 | NSH | NHPS | NWSO_ppm | E | N |
|------|--------|-----|------|----------|---|---|
| test_01 | 空图 | 0 | 0 | 0 | 0 | 0 |
| test_02 | 单节点 | 0 | 0 | 0 | 0 | 1 |
| test_03 | K₂ | 2 | 32 | 1_414_213 | 1 | 2 |
| test_04 | P₃ | 192 | 2_048 | 22_627_416 | 2 | 3 |
| test_05 | K₃ | 12_288 | 98_304 | 271_529_001 | 3 | 3 |
| test_06 | K_{1,4} | 20_480 | 131_072 | 362_038_668 | 4 | 5 |
| test_07 | P₄ | 1_586 | 14_026 | 81_450_380 | 3 | 4 |
| test_08 | K₄ | 2_125_764 | 11_337_408 | 6_185_770_116 | 6 | 4 |
| test_09 | 2×孤立点 | 0 | 0 | 0 | 0 | 2 |
| test_10 | K_{2,3} | 233_280 | 1_492_992 | 1_832_820_774 | 6 | 5 |

所有值经 Python `math.isqrt` 独立验证。

---

## 数学关键验证（S-regular 图）

对于 S-正则图（每条边两端 S 值相同 = S），NWSO 每条边贡献：
```
floor(S³ · √2 · 10^6)
  S=1 → 1_414_213  ✓
  S=2 → 11_313_708
  S=4 → 90_509_667
  S=6 → 305_470_129
  S=9 → 1_030_961_686 (K₄ 每边) ✓
```

---

## 验证步骤

```
# 运行 topo32 harness
cd host-tests/gos-graph-topo32-harness && cargo test
# → 10 passed

# 检查 gos-kernel 集成
cd E:/GOSKernel && cargo check -p gos-kernel
# → Finished (warnings only, 0 errors)
```

---

## 累计指标

- 本次新增测试：10
- 历史 host-test 总数：≈1403（V3.43 后）
- 本次新增指数：3（NSH, NHPS, NWSO）
- topo 函数族序号：topo32（L4=119）
