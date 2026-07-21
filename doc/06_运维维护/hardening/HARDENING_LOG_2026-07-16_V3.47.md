# GOSKernel 强化日志 V3.47 — 2026-07-16

## 摘要

新增三个 Neighborhood S-variant 拓扑指数，将 S-幂次序列扩展至第 10 次幂，完成 topo36：

- **NDC(G)** = Σ_v S(v)^10 — S-十次方顶点和（10th power）
- **NHNC(G)** = Σ_{uv∈E} (S_u+S_v)^9 — S-九次方边和（9th power）
- **NOSO(G)** = Σ_{uv∈E} (S_u²+S_v²)^4 — S-八次方 Sombor（SO^α，α=8；精确整数）

三者均使用 S(v) = Σ_{w∈N(v)} deg(w)（邻居度数和），即度数的 S-变体。

## 变更内容

### crates/gos-runtime/src/lib.rs

新增 `graph_topo_indices36_inner()`（Runtime 实现）与公共函数 `graph_topo_indices36()`：

```
pub fn graph_topo_indices36() -> (u64, u64, u64, usize, usize)
  返回 (ndc, nhnc, noso, edge_count, node_count)
```

算法：O(V+E) — 度数遍历 → S(v) 计算 → 顶点扫描（NDC） + 边扫描（NHNC、NOSO）。
三者均使用带 saturating_mul/add 的 u128 累加器；无需 BFS；无需 isqrt。

### crates/k-shell/src/lib.rs

新增 `dispatch_graph_topo_indices36()` —— 亮黄色标题，NDC 亮青色，
NHNC 亮绿色，NOSO 亮品红。

### crates/k-shell/src/proc.rs

新增以下路由：
- `"graph topo36"` / `"gtopo36"` / `"neighborhood decic"` / `"gndc"`
- `"neighborhood nonic edge"` / `"gnhnc"`
- `"neighborhood octic sombor"` / `"gnoso"`
- `"gndcnhncnoso"`

### host-tests/gos-graph-topo36-harness/

新建 10 项测试的 harness（VectorAddress L4=123，插件 TOPIX_36，执行器 t36.exec）。
10 项测试全部通过。

## 数学定义

**NDC(G) = Σ_v S(v)^10**（S-十次方顶点和）

扩展 S-幂次-顶点序列：
NM₁=Σ S² → NF=Σ S³ → NVQ=Σ S⁴ → NPS=Σ S⁵ → NSH=Σ S⁶ → NSHP=Σ S⁷
→ NOC=Σ S⁸ → NNC=Σ S⁹ → **NDC=Σ S¹⁰**（topo36）

- 对 S-正则图：NDC = n·S^10
- 溢出：S^10 ≤ 16129^10 ≈ 2.6×10^41 > u128::MAX → 使用饱和运算

**NHNC(G) = Σ_{uv∈E} (S_u+S_v)^9**（S-九次方边和）

扩展 S-幂次-边序列：
NHM₁=Σ(S+S)² → NHCS → NHQS → NHPS → NHSE → NHHS → NHOC=Σ(S+S)^8
→ **NHNC=Σ(S+S)^9**（topo36）

- 对 S-正则图：NHNC = |E|·(2S)^9 = 512|E|·S^9
- 每边溢出：(2×16129)^9 ≈ 3.5×10^40 > u128::MAX → 使用饱和运算

**NOSO(G) = Σ_{uv∈E} (S_u²+S_v²)^4**（S-八次方 Sombor，α=8）

将广义 Sombor 指数 SO^α 应用于 S 值，α=8（精确整数，无需 isqrt）：
NSO(α=1) → NCSO(α=3) → NFSO(α=4) → NHSO(α=6) → **NOSO(α=8)**（topo36）

- 对 S-正则图：NOSO = |E|·(2S²)^4 = 16|E|·S^8
- 每边最大值：(2×16129²)^4 ≈ 7.3×10^34 < u128::MAX ✓

## 交叉验证表

| 图    | NDC             | NHNC               | NOSO          | 边数 | 点数 |
|----------|-----------------|--------------------|---------------|-------|-------|
| K₂       | 2               | 512                | 16            | 1     | 2     |
| P₃       | 3_072           | 524_288            | 8_192         | 2     | 3     |
| K₃       | 3_145_728       | 402_653_184        | 3_145_728     | 3     | 3     |
| K_{1,4}  | 5_242_880       | 536_870_912        | 4_194_304     | 4     | 5     |
| P₄       | 120_146         | 13_983_946         | 162_098       | 3     | 4     |
| K₄       | 13_947_137_604  | 1_190_155_742_208  | 4_132_485_216 | 6     | 4     |
| K_{2,3}  | 302_330_880     | 30_958_682_112     | 161_243_136   | 6     | 5     |

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

88=graph-topo 至 122=graph-topo35，**123=graph-topo36**

## 宿主测试套件总计

**1443 个测试**（截至 V3.46 为 1433；gos-graph-topo36-harness 新增 10 个）
