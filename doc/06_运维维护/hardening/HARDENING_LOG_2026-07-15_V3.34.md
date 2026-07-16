# HARDENING LOG — V3.34
**Date**: 2026-07-15  
**Session**: Automated 2h hardening run  
**Branch**: feat/vk-auto-live-surface  
**Commit**: (pending)

---

## 变更摘要

实现 V3.34：`graph topo23` 命令——NHM1（邻域 HM₁ 超 Zagreb）+ NSDD（邻域 SDD 对称除法度）+ NM3（邻域 M₃ 不规则性）三个基于邻域度和 S(v) 的 S-变体拓扑指数，并创建 gos-graph-topo23-harness 10 个测试全部通过。

---

## 新增功能

### V3.34 — NHM1 + NSDD + NM3 S-变体拓扑指数

**核心定义**（S(v) = Σ_{w∈N(v)} deg(w)，与 topo18/topo21/topo22 同族）：

| 指数 | 公式 | 文献类比 | 实现精度 |
|------|------|------|----------|
| NHM1 | Σ_{uv∈E} (S_u+S_v)²            | S-analogue of HM₁ (Shirdel et al. 2013) | 精确 u64 |
| NSDD | Σ_{uv∈E} (S²_u+S²_v)/(S_u·S_v) | S-analogue of SDD (Vasilyev 2014)        | floor ppm (整除) |
| NM3  | Σ_{uv∈E} \|S_u−S_v\|           | S-analogue of M₃/Albertson irregularity  | 精确 u64 |

**实现公式**（无浮点，no_std 安全）：
- NHM1 per edge = `(S_u+S_v)²`（精确；max=(32258)²≈10^9；512 边总和 < u64::MAX）
- NSDD per edge = `floor((S_u²+S_v²)×10^6 / (S_u·S_v))`（整数除法；≥2×10^6；S 均匀时=2×10^6 精确）
- NM3  per edge = `if S_u≥S_v { S_u−S_v } else { S_v−S_u }`（精确）

**关键不变量**：
- NSDD ≥ 2|E|×10^6 always（AM-GM：(S²_u+S²_v)/(S_u·S_v)≥2，当 S_u=S_v 时取等号）
- NSDD = 2|E|×10^6 iff S-regular（所有节点 S 值相等）
- NM3 = 0 iff S-regular（S-均匀图）
- K₃ 和 K_{1,4} 的 NSDD/NHM1/NM3 均由同一 S=4 均匀性导出（与 topo21/topo22 相同的 S-均匀重合）
- K₄ 和 K_{2,3} 都有 NSDD=12_000_000=2×6×10^6（均为 S-均匀 6 边图，尽管 S 值不同：9 vs 6）

**算法**：O(V+E) 度扫描——第一遍 adj+deg，第二遍 S(v)，第三遍边扫描；无需 BFS，无平方根。

### 分析验证表

| 图       | NHM1  | NSDD(ppm)   | NM3 | 边数 | 节点数 |
|----------|-------|-------------|-----|------|--------|
| 空图     | 0     | 0           | 0   | 0    | 0      |
| 孤立点   | 0     | 0           | 0   | 0    | 1      |
| K₂       | 4     | 2_000_000   | 0   | 1    | 2      |
| P₃       | 32    | 4_000_000   | 0   | 2    | 3      |
| K₃       | 192   | 6_000_000   | 0   | 3    | 3      |
| K_{1,4}  | 256   | 8_000_000   | 0   | 4    | 5      |
| P₄       | 86    | 6_333_332   | 2   | 3    | 4      |
| K₄       | 1944  | 12_000_000  | 0   | 6    | 4      |
| 2 孤立点 | 0     | 0           | 0   | 0    | 2      |
| K_{2,3}  | 864   | 12_000_000  | 0   | 6    | 5      |

注：
- P₄ NSDD：edge A-B (S=2,3): floor(13×10^6/6)=2_166_666；edge B-C (S=3,3): 2_000_000；cumulative=6_333_332 > 6_000_000（非 S-regular 验证 ✓）
- K₄ 和 K_{2,3} NSDD 均为 12_000_000：两图均为 6 边 S-均匀图，NSDD=2×|E|×10^6 与 S 的具体值无关
- NM3=0 在所有 S-均匀图中（K₂, P₃, K₃, K_{1,4}, K₄, K_{2,3}），P₄ NM3=2 验证了 S 不均匀性

---

## 代码变更

### `crates/gos-runtime/src/lib.rs`
- 新增 `graph_topo_indices23_inner()` 内函数：
  - 4 步：compact-index → adj bitmask + edge_count → deg[] → S(v) → NHM1/NSDD/NM3 边扫描
  - 无需 isqrt64，所有计算为整数运算（乘法和整除）
  - 溢出安全分析：NHM1/NSDD/NM3 均在 u64 范围内（见注释）
  - 返回 `(nhm1, nsdd_ppm, nm3, edge_count, node_count)`
- 新增 `graph_topo_indices23()` 公开接口及完整 doc 注释（V3.34 标签）

### `crates/k-shell/src/lib.rs`
- 新增 `dispatch_graph_topo_indices23()` 显示函数：
  - 亮黄色标题：`graph topo23 (NHM1 + NSDD + NM3 S-variant indices)`
  - NHM1 亮青色（精确整数）
  - NSDD 亮绿色（ppm，3 位小数；S-regular 时显示 "≡2|E| (S-regular)" 注释）
  - NM3  亮洋红色（精确整数；=0 时显示 "NM3=0: S-regular" 注释）
  - 页脚：`Shirdel et al. 2013  Vasilyev 2014  (S-variant family)`

### `crates/k-shell/src/proc.rs`
- 新增 shell 路由：
  - `"graph topo23"` / `"gtopo23"` / `"neighborhood hm1"` / `"gnhm1"` / `"neighborhood sdd"` / `"gnsdd"` / `"neighborhood m3"` / `"gnm3"` / `"gnhm1nsddnm3"`

### `host-tests/gos-graph-topo23-harness/`（新建）
- `.cargo/config.toml`：host target `x86_64-pc-windows-msvc`
- `Cargo.toml`：独立 workspace，依赖 gos-protocol / gos-cypher-mut / gos-runtime / gos-supervisor
- `tests/graph_topo23.rs`：10 个测试全部通过（VectorAddress L4=110 命名空间）

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

test result: ok. 10 passed; 0 failed; 0 ignored
```

**累计 host 测试数：1313**（上版 1303 + 本次 10）

---

## OS 类比

| 指数 | OS 含义 |
|------|---------|
| NHM1 | S-超 Zagreb：邻域度和的平方和（高=高 S 值且多边；S-均匀时 NHM1 = m×(2c)²=4mc²） |
| NSDD | S-对称除法度：(S²_u+S²_v)/(S_u·S_v) 的和（≥2|E|；=2|E| iff S-regular；测量 S-不均匀性）|
| NM3  | S-不规则性：|S_u−S_v| 之和（=0 iff S-regular；是 topo18 NM₁/NM₂ 家族的不规则性补充）|

---

## VectorAddress L4 命名空间（更新）

…108=graph-topo21, 109=graph-topo22, **110=graph-topo23**

---

## 下一步建议

可继续实现 topo24，候选：
- **NRGG (S-变体 Reciprocal GA)**: Σ_{uv∈E} √(S_u·S_v)/(S_u+S_v) × 10^6  (S-analogue of reciprocal GA; isqrt128)
- **NEZI (S-变体 Estrada-like)**: Σ_v e^{S(v)/max_S} (需要 ln 表，更复杂)
- **NS-Index (S-变体 S index)**: Σ_{uv∈E} S(u)+S(v) (=NM₁(G)/2 的边版本; 极简; 等于 2·NM₂/NM₁)

或跳出 S-变体家族，实现：
- **图路径枚举 / Hückel 分子轨道指标** (需要 BFS)
- **Perron centrality** (需要特征向量迭代)
