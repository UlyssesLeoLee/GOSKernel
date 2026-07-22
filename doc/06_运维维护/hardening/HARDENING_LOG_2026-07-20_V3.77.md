# 强化日志 — V3.77（2026-07-20）

## 摘要

新增 NTETRAACTC + NHTETRAACTC + NAISO 三项 Neighborhood S-variant 拓扑指数（topo66），
以及配套的 `gos-graph-topo66-harness`（10 项测试）。

**宿主测试套件总数：1743 项**（V3.76 累计 1733 项 + 本次新增 10 项）。

---

## 变更内容

### 新增 API：`gos_runtime::graph_topo_indices66()`

```rust
pub fn graph_topo_indices66() -> (u64, u64, u64, usize, usize)
// 返回 (ntetraactc, nhtetraactc, naiso, edge_count, node_count)
```

三项指数均基于 S-变体邻域度和：
`S(v) = Σ_{w∈N(v)} deg(w)`。

#### NTETRAACTC —— S-第40次幂顶点和（α=40）

```
NTETRAACTC(G) = Σ_v S(v)^40
```

- 延伸 S-幂次顶点系列：`NNONATRIACTC=ΣS^39`（topo65）→ **`NTETRAACTC=ΣS^40`（topo66）**
- S-正则图公式：`NTETRAACTC = n·S^40`
- 实现：`s^40 = s32 × s8`（40 = 32+8；效率很高，s32 之后仅需 1 次额外乘法）

#### NHTETRAACTC —— S-第39次幂边和（power=39）

```
NHTETRAACTC(G) = Σ_{uv∈E} (S_u + S_v)^39
```

- 延伸：`NHNONATRIACTC=Σ(S+S)^38`（topo65）→ **`NHTETRAACTC=Σ(S+S)^39`（topo66）**
- S-正则图公式：`NHTETRAACTC = |E|·(2S)^39 = 549_755_813_888·|E|·S^39`
- 实现：`ss^39 = ss32 × ss4 × ss2 × ss`（39 = 32+4+2+1）

#### NAISO —— S-第68次 Sombor 变体（α=68）

```
NAISO(G) = Σ_{uv∈E} (S_u² + S_v²)^34
```

- 第3轮双字母序列延续：NAASO(α=52)…NAHSO(α=66) → **NAISO(α=68)**
- 精确整数运算（无需 isqrt）：α=68 为偶数，j=34
- S-正则图公式：`NAISO = |E|·(2S²)^34 = 17_179_869_184·|E|·S^68`
- 实现：`s2s^34 = s2s32 × s2s2`（34 = 32+2；效率很高，仅需 1 次额外乘法）

---

## 测试数据

| 图 | NTETRAACTC | NHTETRAACTC | NAISO | 边数 | 节点数 |
|-----------|----------------------|------------------|------------------|-------|-------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| K₂ | 2 | 549_755_813_888 | 17_179_869_184 | 1 | 2 |
| P₃ | 3_298_534_883_328 | u64::MAX | u64::MAX | 2 | 3 |
| K₃ | u64::MAX | u64::MAX | u64::MAX | 3 | 3 |
| K_{1,4} | u64::MAX | u64::MAX | u64::MAX | 4 | 5 |
| P₄ | u64::MAX | u64::MAX | u64::MAX | 3 | 4 |
| K₄ | u64::MAX | u64::MAX | u64::MAX | 6 | 4 |
| 两孤立节点 | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | u64::MAX | u64::MAX | u64::MAX | 6 | 5 |

注：P₄ 在 topo66 出现新的饱和点。topo65 时，P₄ 的 NNONATRIACTC 为
`8_105_111_405_549_580_310`（可容纳于 u64）。到 topo66，P₄ 的 NTETRAACTC
需要 `2×3^40 = 24_315_330_918_113_857_602 > u64::MAX`，因此从 P₄ 起三项
指数均发生饱和。

---

## 新增测试 harness

**`host-tests/gos-graph-topo66-harness/`**（10 项测试）

- 插件：`TOPIX_66` / 执行器：`t66.exec`
- VectorAddress L4=153
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
graph topo66 / gtopo66 / gntetraactc / gnhtetraactc / gnnaiso
gntetraactcnhtetraactcnaiso
```

---

## VectorAddress L4 命名空间更新

88=graph-topo 至 152=graph-topo65，**153=graph-topo66**

---

## 实现说明

- `s^40 = s32 × s8`：效率很高（40 = 32+8，s2 之后仅需 2 次平方）
- `ss^39 = ss32 × ss4 × ss2 × ss`：与 topo65 的 `ss^38` 相同的 4 项分解，多 1 次 `×ss`
- `s2s^34 = s2s32 × s2s2`：效率很高（34 = 32+2，仅需 1 次额外乘法）
- 全部运算使用 u128 饱和累加器，末尾截断至 u64::MAX
