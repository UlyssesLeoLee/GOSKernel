# 强化日志 — V3.78（2026-07-20）

## 摘要

新增 NHENTETRAACTC + NHHENTETRAACTC + NAJSO 三项 Neighborhood S-variant 拓扑指数（topo67），
以及配套的 `gos-graph-topo67-harness`（10 项测试）。

**宿主测试套件总数：1753 项**（V3.77 累计 1743 项 + 本次新增 10 项）。

---

## 变更内容

### 新增 API：`gos_runtime::graph_topo_indices67()`

```rust
pub fn graph_topo_indices67() -> (u64, u64, u64, usize, usize)
// 返回 (nhentetraactc, nhhentetraactc, najso, edge_count, node_count)
```

三项指数均基于 S-变体邻域度和：
`S(v) = Σ_{w∈N(v)} deg(w)`。

#### NHENTETRAACTC —— S-第41次幂顶点和（power=41）

```
NHENTETRAACTC(G) = Σ_v S(v)^41
```

- 延伸 S-幂次顶点系列：`NTETRAACTC=ΣS^40`（topo66）→ `NHENTETRAACTC=ΣS^41`（topo67）
- S-正则图公式：`NHENTETRAACTC = n·S^41`
- 实现：`s^41 = s32 × s8 × s`（41 = 32+8+1）

#### NHHENTETRAACTC —— S-第40次幂边和（power=40）

```
NHHENTETRAACTC(G) = Σ_{uv∈E} (S_u + S_v)^40
```

- 延伸：`NHTETRAACTC=Σ(S+S)^39`（topo66）→ `NHHENTETRAACTC=Σ(S+S)^40`（topo67）
- S-正则图公式：`NHHENTETRAACTC = |E|·(2S)^40 = 1_099_511_627_776·|E|·S^40`
- 实现：`ss^40 = ss32 × ss8`（40 = 32+8；效率很高，只需两次平方之和）

#### NAJSO —— S-第70次 Sombor 变体（α=70）

```
NAJSO(G) = Σ_{uv∈E} (S_u² + S_v²)^35
```

- 第3轮双字母序列延续：NAISO(α=68, topo66) → **NAJSO(α=70)**
- 精确整数运算（无需 isqrt）：α=70 为偶数，j=35
- S-正则图公式：`NAJSO = |E|·(2S²)^35 = 34_359_738_368·|E|·S^70`
- 实现：`s2s^35 = s2s32 × s2s2 × s2s`（35 = 32+2+1）

---

## 测试数据

| 图 | NHENTETRAACTC | NHHENTETRAACTC | NAJSO | 边数 | 节点数 |
|-----------|----------------------|----------------------|------------------|-------|-------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| K₂ | 2 | 1_099_511_627_776 | 34_359_738_368 | 1 | 2 |
| P₃ | 6_597_069_766_656 | u64::MAX | u64::MAX | 2 | 3 |
| K₃ | u64::MAX | u64::MAX | u64::MAX | 3 | 3 |
| K_{1,4} | u64::MAX | u64::MAX | u64::MAX | 4 | 5 |
| P₄ | u64::MAX | u64::MAX | u64::MAX | 3 | 4 |
| K₄ | u64::MAX | u64::MAX | u64::MAX | 6 | 4 |
| 两孤立节点 | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | u64::MAX | u64::MAX | u64::MAX | 6 | 5 |

关键推导：
- K₂（S=1）：NHENTETRAACTC=2；NHHENTETRAACTC=2^40=1_099_511_627_776；NAJSO=2^35=34_359_738_368
- P₃（S=2 均匀）：NHENTETRAACTC=3×2^41=6_597_069_766_656；其余两项饱和（每边 4^40=2^80>u64::MAX）
- P₄ 起三项指数继续饱和（与 topo66 起的规律一致）

---

## 新增测试 harness

**`host-tests/gos-graph-topo67-harness/`**（10 项测试）

- 插件：`TOPIX_67` / 执行器：`t67.exec`
- VectorAddress L4=154
- 全部 10 项测试：通过

```
test test_01_empty          ... ok
test test_02_single_node    ... ok
test test_03_k2_edge        ... ok
test test_04_path_p3        ... ok
test test_05_triangle_k3    ... ok
test test_06_star_k14       ... ok
test test_07_path_p4        ... ok
test test_08_complete_k4    ... ok
test test_09_two_isolated   ... ok
test test_10_k23_bipartite  ... ok

test result: ok. 10 passed; 0 failed
```

---

## Shell 命令

```
graph topo67 / gtopo67 / gnhentetraactc / gnhhentetraactc / gnnajso
gnhentetraactcnhhentetraactcnajso
```

---

## VectorAddress L4 命名空间更新

88=graph-topo 至 153=graph-topo66，**154=graph-topo67**

---

## 实现说明

- `ss^40 = ss32 × ss8`：效率很高（40 = 32+8，两个2的幂次 —— 平方后只需最后1次乘法）
- `s^41 = s32 × s8 × s`：41 = 32+8+1（3项分解）
- `s2s^35 = s2s32 × s2s2 × s2s`：35 = 32+2+1（3项分解）
- 全部运算使用 u128 饱和累加器，末尾截断至 u64::MAX
- 注：`NHHENTETRAACTC` 在指数 40 处的实现尤为高效（40=32+8，两者均为2的幂次）
