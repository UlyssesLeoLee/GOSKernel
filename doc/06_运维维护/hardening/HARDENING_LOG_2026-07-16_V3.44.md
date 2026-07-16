# HARDENING LOG — V3.44
**Date**: 2026-07-16  
**Branch**: feat/vk-auto-live-surface  
**Author**: Automated hardening task (Claude Sonnet 4.6)

---

## 变更摘要

新增三个 S-变体拓扑指数族（topo33）：**NSHP**、**NHSE**、**NCSO**，延续 V3.43 的幂次多项式序列，向图论 OS 拓扑感知子系统注入更丰富的结构描述能力。

---

## 新增指数定义

### NSHP — Neighborhood S-Heptic Index（顶点七次幂）
```
NSHP(G) = Σ_{v∈V} S(v)^7
```
- S(v) = Σ_{w∈N(v)} deg(w)（邻居度之和）
- 返回类型：`u64`（u128 累加器，饱和截断）
- 延续 S-幂次顶点序列：NM₁=Σ S²→NF=Σ S³→NVQ=Σ S⁴→NPS=Σ S⁵→NSH=Σ S⁶→NSHP=Σ S⁷
- NSHP = n·S⁷ for S-regular

### NHSE — Neighborhood Hyper-S-Edge Sextic（边六次幂）
```
NHSE(G) = Σ_{uv∈E} (S_u + S_v)^6
```
- 返回类型：`u64`（u128 累加器，饱和截断）
- 延续 S-幂次边序列：NHM1=Σ(S+S)²→NHCS=Σ(S+S)³→NHQS=Σ(S+S)⁴→NHPS=Σ(S+S)⁵→NHSE=Σ(S+S)⁶
- NHSE = |E|·(2S)^6 = 64|E|S⁶ for S-regular

### NCSO — Neighborhood Cubic Sombor Index（S-Cubic Sombor）
```
NCSO(G) × 10^6 = Σ_{uv∈E} (S_u² + S_v²)^{3/2} × 10^6
```
- 以 ppm 编码（× 10^6，floor）存储为 `u64`
- S-变体广义 Sombor 指数 SO^α，取 α=3
- NSO(topo21) = Σ√(S_u²+S_v²) = SO^1; NCSO = Σ(S_u²+S_v²)·√(S_u²+S_v²) = SO^3
- 纯整数计算恒等式：
  ```
  floor((S_u²+S_v²)^{3/2} × 10^6)
    = isqrt128((S_u²+S_v²)^3 × 10^12)
  ```
- 溢出检查：(S_u²+S_v²)^3 ≤ (2×16129²)^3 ≈ 1.41×10^26; ×10^12 ≈ 1.41×10^38 < u128::MAX ✓
- NCSO = |E|·2√2·S³·10^6 for S-regular（等价于 S^2·NSO_per_edge 的加权关系）

---

## 解析对照表

| 图         | NSHP（精确） | NHSE（精确）  | NCSO（ppm）    |
|------------|-------------|--------------|----------------|
| K₂         | 2           | 64           | 2_828_427      |
| P₃         | 384         | 8_192        | 45_254_832     |
| K₃         | 49_152      | 786_432      | 543_058_005    |
| K_{1,4}    | 81_920      | 1_048_576    | 724_077_340    |
| P₄         | 4_630       | 77_906       | 170_111_864    |
| K₄         | 19_131_876  | 204_073_344  | 12_371_540_238 |
| K_{2,3}    | 1_399_680   | 17_915_904   | 3_665_641_548  |

**S-regular 校验（NCSO）**：per-edge = floor(2√2·S³·10^6)
- S=1: 2_828_427; S=2: 22_627_416; S=4: 181_019_335; S=6: 610_940_258; S=9: 2_061_923_373

---

## 文件变更

| 文件 | 变更 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices33_inner()` + `graph_topo_indices33()` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices33()` 显示函数 |
| `crates/k-shell/src/proc.rs` | 注册 `graph topo33` / `gtopo33` / `gnshp` / `gnhse` / `gncso` 等别名 |
| `host-tests/gos-graph-topo33-harness/` | 新建独立 Cargo workspace harness |

---

## 测试用例（10/10 通过）

VectorAddress L4=120，插件 `TOPIX_33`，执行器 `t33.exec`

| # | 测试场景 | NSHP | NHSE | NCSO |
|---|----------|------|------|------|
| 1 | 空图 | 0 | 0 | 0 |
| 2 | 单孤立节点 | 0 | 0 | 0 |
| 3 | K₂（单边）| 2 | 64 | 2_828_427 |
| 4 | P₃（路径）| 384 | 8_192 | 45_254_832 |
| 5 | K₃（三角）| 49_152 | 786_432 | 543_058_005 |
| 6 | K_{1,4}（星形）| 81_920 | 1_048_576 | 724_077_340 |
| 7 | P₄（路径）| 4_630 | 77_906 | 170_111_864 |
| 8 | K₄（完全图）| 19_131_876 | 204_073_344 | 12_371_540_238 |
| 9 | 两孤立节点 | 0 | 0 | 0 |
| 10 | K_{2,3}（二分图）| 1_399_680 | 17_915_904 | 3_665_641_548 |

---

## VectorAddress L4 命名空间（更新）

88=graph-topo … 119=graph-topo32, **120=graph-topo33**

---

## Shell 命令别名

```
graph topo33 | gtopo33 | neighborhood heptic | gnshp
neighborhood sextic edge | gnhse | neighborhood cubic sombor | gncso
gnshpnhsencso
```

---

## 版本信息

- **V3.44** — 新增 NSHP + NHSE + NCSO (topo33)
- 累计 host 测试：1413（1403 + 10）
- 累计 L4 命名空间：88–120（33 个 topo 扩展）
