# GOSKernel 强化日志 V3.48 — 2026-07-16

## 摘要

新增三个 Neighborhood S-variant 拓扑指数，将 S-幂次序列扩展至第 11 次幂、广义 Sombor 序列扩展至 α=10，完成 topo37：

- **NUC(G)** = Σ_v S(v)^11 — S-十一次方顶点和（11th power）
- **NHDC(G)** = Σ_{uv∈E} (S_u+S_v)^10 — S-十次方边和（10th power）
- **NTSO(G)** = Σ_{uv∈E} (S_u²+S_v²)^5 — S-第十 Sombor（SO^α，α=10；精确整数）

三者均使用 S(v) = Σ_{w∈N(v)} deg(w)（邻居度数和），即度数的 S-变体。

## 变更内容

### crates/gos-runtime/src/lib.rs

新增 `graph_topo_indices37_inner()`（Runtime 实现）与公共函数 `graph_topo_indices37()`：

```
pub fn graph_topo_indices37() -> (u64, u64, u64, usize, usize)
  返回 (nuc, nhdc, ntso, edge_count, node_count)
```

算法：O(V+E) — 度数遍历 → S(v) 计算 → 顶点扫描（NUC） + 边扫描（NHDC、NTSO）。
三者均使用带 saturating_mul/add 的 u128 累加器；无需 BFS；无需 isqrt。

### crates/k-shell/src/lib.rs

新增 `dispatch_graph_topo_indices37()` —— 亮黄色标题，NUC 亮青色，
NHDC 亮绿色，NTSO 亮品红。

### crates/k-shell/src/proc.rs

新增以下路由：
- `"graph topo37"` / `"gtopo37"` / `"neighborhood undecic"` / `"gnuc"`
- `"neighborhood decic edge"` / `"gnhdc"`
- `"neighborhood tenth sombor"` / `"gntso"`
- `"gnucnhdcntso"`

### host-tests/gos-graph-topo37-harness/

新建 10 项测试的 harness（VectorAddress L4=124，插件 TOPIX_37，执行器 t37.exec）。
10 项测试全部通过。

## 数学定义

**NUC(G) = Σ_v S(v)^11**（S-十一次方顶点和）

扩展 S-幂次-顶点序列：
NM₁=Σ S² → NF=Σ S³ → NVQ=Σ S⁴ → NPS=Σ S⁵ → NSH=Σ S⁶ → NSHP=Σ S⁷
→ NOC=Σ S⁸ → NNC=Σ S⁹ → NDC=Σ S¹⁰ → **NUC=Σ S¹¹**（topo37）

- 对 S-正则图：NUC = n·S^11
- 溢出：S^11 ≤ 16129^11 ≈ 4.2×10^45 > u128::MAX → 使用饱和运算

**NHDC(G) = Σ_{uv∈E} (S_u+S_v)^10**（S-十次方边和）

扩展 S-幂次-边序列：
NHM₁=Σ(S+S)² → NHCS → NHQS → NHPS → NHSE → NHHS → NHOC=Σ(S+S)^8
→ NHNC=Σ(S+S)^9 → **NHDC=Σ(S+S)^10**（topo37）

- 对 S-正则图：NHDC = |E|·(2S)^10 = 1024|E|·S^10
- 每边溢出：(2×16129)^10 ≈ 5.6×10^44 > u128::MAX → 使用饱和运算

**NTSO(G) = Σ_{uv∈E} (S_u²+S_v²)^5**（S-第十 Sombor，α=10）

将广义 Sombor 指数 SO^α 应用于 S 值，α=10（精确整数，无需 isqrt）：
NSO(α=1) → NCSO(α=3) → NFSO(α=4) → NHSO(α=6) → NOSO(α=8) → **NTSO(α=10)**（topo37）

- 对 S-正则图：NTSO = |E|·(2S²)^5 = 32|E|·S^10
- 每边溢出：(2×16129²)^5 ≈ 3.8×10^43 > u128::MAX → 使用饱和运算

## 交叉验证表

| 图    | NUC              | NHDC                | NTSO            | 边数 | 点数 |
|----------|------------------|---------------------|-----------------|-------|-------|
| K₂       | 2                | 1_024               | 32              | 1     | 2     |
| P₃       | 6_144            | 2_097_152           | 65_536          | 2     | 3     |
| K₃       | 12_582_912       | 3_221_225_472       | 100_663_296     | 3     | 3     |
| K_{1,4}  | 20_971_520       | 4_294_967_296       | 134_217_728     | 4     | 5     |
| P₄       | 358_390          | 79_997_426          | 2_632_154       | 3     | 4     |
| K₄       | 125_524_238_436  | 21_422_803_359_744  | 669_462_604_992 | 6     | 4     |
| K_{2,3}  | 1_813_985_280    | 371_504_185_344     | 11_609_505_792  | 6     | 5     |

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

test result: ok. 10 passed; 0 failed; 0 ignored
```

## VectorAddress L4 命名空间（更新后）

88=graph-topo 至 123=graph-topo36，**124=graph-topo37**

## 宿主测试套件总计

**1453 个测试**（截至 V3.47 为 1443；gos-graph-topo37-harness 新增 10 个）
