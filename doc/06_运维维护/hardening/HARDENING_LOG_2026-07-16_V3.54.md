# GOS 强化日志 — V3.54

**日期**: 2026-07-16  
**版本**: V3.54  
**分支**: feat/vk-auto-live-surface  
**提交**: feat(v3.54): NHEPTC + NHSTC + NKSO Neighborhood S-variant indices + gos-graph-topo43-harness (10 tests)

---

## 新增内容

### 图论拓扑指数 topo43：NHEPTC + NHSTC + NKSO（S-变体族）

新增三个 Neighborhood S-变体拓扑指数，均基于 S(v) = Σ_{w∈N(v)} deg(w) 邻度和：

#### NHEPTC — S-十七次顶点幂和
```
NHEPTC(G) = Σ_v S(v)^17   （精确 u64，u128 累加器饱和截断）
```
- 延伸 S-幂次顶点系列：NM₁(topo18)→…→NSTC=ΣS¹⁶(topo42)→NHEPTC=ΣS¹⁷(topo43)
- S-正则图：NHEPTC = n·S^17
- 实现：`s^17 = s^16 × s`（`s16 = s8×s8`，`s8 = s4×s4`，`s4 = s2×s2`）

#### NHSTC — S-十六次边幂和
```
NHSTC(G) = Σ_{uv∈E} (S_u+S_v)^16   （精确 u64，u128 累加器饱和截断）
```
- 延伸 S-幂次边系列：NHM1(topo23)→…→NHPTC=Σ(S+S)¹⁵(topo42)→NHSTC=Σ(S+S)¹⁶(topo43)
- S-正则图：NHSTC = |E|·(2S)^16 = 65536|E|·S^16
- 实现：`ss^16 = ss8×ss8`（ss = S_u+S_v）

#### NKSO — S-Docosic Sombor（α=22）
```
NKSO(G) = Σ_{uv∈E} (S_u²+S_v²)^11   （精确 u64，无需 isqrt）
```
- 广义 S-变体 Sombor SO^α，α=22：(S²+S²)^11 为整数幂，无需开平方
- 系列：NSO(α=1)→NCSO(α=3)→…→NJSO(α=20,topo42)→NKSO(α=22,topo43)
- S-正则图：NKSO = |E|·(2S²)^11 = 2048|E|·S^22
- 实现：`s2s^11 = s2s^8 × s2s^2 × s2s`（s2s = S_a²+S_b²）

---

## 解析验证表

| 图        | NHEPTC                    | NHSTC                        | NKSO                         | 边数 | 节点数 |
|-----------|---------------------------|------------------------------|------------------------------|------|--------|
| 空图      | 0                         | 0                            | 0                            | 0    | 0      |
| 单节点    | 0                         | 0                            | 0                            | 0    | 1      |
| K₂        | 2                         | 65_536                       | 2_048                        | 1    | 2      |
| P₃        | 393_216                   | 8_589_934_592                | 17_179_869_184               | 2    | 3      |
| K₃        | 51_539_607_552            | 844_424_930_131_968          | 108_086_391_056_891_904      | 3    | 3      |
| K_{1,4}   | 85_899_345_920            | 1_125_899_906_842_624        | 144_115_188_075_855_872      | 4    | 5      |
| P₄        | 258_542_470               | 3_126_285_688_706            | 67_852_730_867_306           | 3    | 4      |
| K₄        | 66_708_726_798_666_276    | u64::MAX（饱和）              | u64::MAX（饱和）              | 6    | 4      |
| 双孤立    | 0                         | 0                            | 0                            | 0    | 2      |
| K_{2,3}   | 84_633_297_223_680        | 1_109_305_553_370_218_496    | u64::MAX（饱和）              | 6    | 5      |

**注**：K₄ NHEPTC = 4×9^17 = 66_708_726_798_666_276（适合 u64，不饱和）；K_{2,3} NHSTC = 1_109_305_553_370_218_496 < u64::MAX（精确）。

---

## 关键推导

### S-正则公式验证
- `NHEPTC = n·S^17` ✓  
- `NHSTC = |E|·(2S)^16 = 65536|E|·S^16` ✓  
- `NKSO = |E|·(2S²)^11 = 2048|E|·S^22` ✓

### K₄ NHEPTC 精确计算
9^16 = 1_853_020_188_851_841（来自 topo42）  
9^17 = 9 × 1_853_020_188_851_841 = 16_677_181_699_666_569  
NHEPTC = 4 × 16_677_181_699_666_569 = 66_708_726_798_666_276 < u64::MAX ✓

### K_{2,3} NHSTC 精确计算
12^15 = 15_407_021_574_586_368（来自 topo42）  
12^16 = 12 × 15_407_021_574_586_368 = 184_884_258_895_036_416  
NHSTC = 6 × 184_884_258_895_036_416 = 1_109_305_553_370_218_496 < u64::MAX ✓

---

## 修改文件

| 文件 | 变更 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices43_inner()`（内部实现）及 `graph_topo_indices43()`（公开 API） |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices43()` 显示函数 |
| `crates/k-shell/src/proc.rs` | 新增 topo43 路由（"graph topo43"/"gtopo43"/"neighborhood heptadecic"/"gnheptc"/"neighborhood hexadecic edge"/"gnhstc"/"neighborhood docosic sombor"/"gnkso"/"gnheptcnhstcnkso"） |
| `host-tests/gos-graph-topo43-harness/` | 新建完整测试套件（10 个测试） |

---

## Shell 命令

```
graph topo43
gtopo43
neighborhood heptadecic    → gnheptc
neighborhood hexadecic edge → gnhstc
neighborhood docosic sombor → gnkso
gnheptcnhstcnkso
```

---

## VectorAddress 命名空间（更新后）

L4=88（graph-topo）到 L4=129（graph-topo42）延续：  
**L4=130 = graph-topo43（gos-graph-topo43-harness）**

---

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

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**累计 host 测试数：1513**（前累计 1503，topo43-harness 新增 10）

---

## 图论 OS 意义

- **NHEPTC**：S-十七次顶点聚合压力——高次顶点幂函数提取 S 分布中的极端值，放大枢纽节点的拓扑贡献
- **NHSTC**：S-十六次边耦合强度——对等对称边的功率以 65536 倍率放大，敏感捕捉对称与不对称边的差异
- **NKSO**：S-Docosic Sombor（α=22）——广义 Sombor 族最新成员，精确整数计算（无 isqrt），为图的几何距离特性提供高阶量化
