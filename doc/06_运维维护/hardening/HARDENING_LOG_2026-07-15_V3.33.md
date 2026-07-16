# HARDENING LOG — V3.33
**Date**: 2026-07-15  
**Session**: Automated 2h hardening run  
**Branch**: feat/vk-auto-live-surface  
**Commit**: (pending)

---

## 变更摘要

实现 V3.33：`graph topo22` 命令——NR（邻域 Randić 指数）+ NF（邻域 Forgotten 指数）+ NSC（邻域 Sum-Connectivity 指数）三个基于邻域度和 S(v) 的 S-变体拓扑指数，并创建 gos-graph-topo22-harness 10 个测试全部通过。

---

## 新增功能

### V3.33 — NR + NF + NSC S-变体拓扑指数

**核心定义**（S(v) = Σ_{w∈N(v)} deg(w)，即顶点 v 的邻域度和，与 topo18/topo21 同族）：

| 指数 | 公式 | 文献类比 | 实现精度 |
|------|------|------|----------|
| NR  | Σ_{uv∈E} (S_u·S_v)^{-1/2} | S-analogue of Randić R (1975) | floor ppm (isqrt64) |
| NF  | Σ_v S(v)³                  | S-analogue of Forgotten F (Furtula & Gutman 2015) | 精确 u64 |
| NSC | Σ_{uv∈E} (S_u+S_v)^{-1/2} | S-analogue of Sum-Conn. SC (Zhou & Trinajstić 2009) | floor ppm (isqrt64) |

**实现公式**（无浮点，no_std 安全）：
- NR  per edge = `isqrt64(10^12 / (S_u · S_v))`（S≥1 对边端点始终成立）
- NF  per node = `S(v)³`（精确；S ≤ 127² = 16129；S³ ≤ 4.2×10^12；128 个节点累计 < u64::MAX）
- NSC per edge = `isqrt64(10^12 / (S_u + S_v))`

**关键不变量**：
- NR = NSC 当所有边 S-值满足 S_u·S_v = S_u+S_v（S=2 均匀图，如 P₃）
- S-均匀情形（S=c）：NR = m·floor(10^6/c)（c 整除 10^6 时精确）
- NF = 0 当图无边且所有节点孤立（S=0 for all）
- K₃ 和 K_{1,4}：两者 S=4，逐边 NR 和 NSC 均完全相同（S-均匀性同 topo21/topo18）

**算法**：O(V+E) 度扫描——第一遍 adj+deg，第二遍 S(v)，第三遍边扫描；无需 BFS。

### 分析验证表

| 图       | NR(ppm)   | NF    | NSC(ppm)  | 边数 | 节点数 |
|----------|-----------|-------|-----------|------|--------|
| 空图     | 0         | 0     | 0         | 0    | 0      |
| 孤立点   | 0         | 0     | 0         | 0    | 1      |
| K₂       | 1_000_000 | 2     | 707_106   | 1    | 2      |
| P₃       | 1_000_000 | 24    | 1_000_000 | 2    | 3      |
| K₃       | 750_000   | 192   | 1_060_659 | 3    | 3      |
| K_{1,4}  | 1_000_000 | 320   | 1_414_212 | 4    | 5      |
| P₄       | 1_149_829 | 70    | 1_302_674 | 3    | 4      |
| K₄       | 666_666   | 2916  | 1_414_212 | 6    | 4      |
| 2 孤立点 | 0         | 0     | 0         | 0    | 2      |
| K_{2,3}  | 999_996   | 1080  | 1_732_050 | 6    | 5      |

注：
- K₂：S(A)=S(B)=1，sp=1，ssum=2；NR=isqrt64(10^12)=10^6；NSC=isqrt64(10^12/2)=707_106
- P₃：S-均匀 S=2，NR=NSC=1_000_000（因 sp=ssum=4 使 NR per edge = NSC per edge = 500_000）
- K₃：S=4；NR per edge=isqrt64(10^12/16)=250_000；NSC per edge=isqrt64(10^12/8)=353_553
- K_{1,4}：S=4（hub 和 leaf 均为 4）；与 K₃ 每边 NR/NSC 完全相同（S-均匀性）
- P₄：S(A)=S(D)=2, S(B)=S(C)=3；混合 sp/ssum，累计精确（见 harness 注释推导）
- K₄：S=9；NR per edge=111_111；NSC per edge=235_702
- K_{2,3}：S=6（全节点）；NR=999_996（6×floor(10^6/6)=6×166_666，下取整损失 4 ppm）

---

## 代码变更

### `crates/gos-runtime/src/lib.rs`
- 新增 `graph_topo_indices22_inner()` 内函数：
  - 5 步：compact-index → adj bitmask → deg[] → S(v) → NF 节点扫描 + NR/NSC 边扫描
  - 内联 `isqrt64`（Newton-Raphson，无 float，no_std 安全）
  - 返回 `(nr_ppm, nf, nsc_ppm, edge_count, node_count)`
- 新增 `graph_topo_indices22()` 公开接口及完整 doc 注释（V3.33 标签）

### `crates/k-shell/src/lib.rs`
- 新增 `dispatch_graph_topo_indices22()` 显示函数：
  - 亮黄色标题：`graph topo22 (NR + NF + NSC S-variant indices)`
  - NR  亮青色（ppm，3 位小数）
  - NF  亮绿色（精确整数）
  - NSC 亮洋红色（ppm，3 位小数）
  - 页脚：`Randic 1975 Furtula & Gutman 2015 Zhou & Trinajstic 2009  (S-variant family)`

### `crates/k-shell/src/proc.rs`
- 新增 shell 路由：
  - `"graph topo22"` / `"gtopo22"` / `"neighborhood randic"` / `"gnr"` / `"neighborhood forgotten"` / `"gnf"` / `"neighborhood sumconn"` / `"gnsc"` / `"gnrnfnsc"`

### `host-tests/gos-graph-topo22-harness/`（新建）
- `.cargo/config.toml`：host target `x86_64-pc-windows-msvc`
- `Cargo.toml`：独立 workspace，依赖 gos-protocol / gos-cypher-mut / gos-runtime / gos-supervisor
- `tests/graph_topo22.rs`：10 个测试全部通过（VectorAddress L4=109 命名空间）

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

**累计 host 测试数：1303**（上版 1293 + 本次 10）

---

## OS 类比

| 指数 | OS 含义 |
|------|---------|
| NR  | S-变体 Randić：邻域度积的倒数平方根和（高=端点 S-均匀且较小；星图叶层 S=4 与三角图等价） |
| NF  | 邻域 Forgotten 立方压力（= Σ_v S³；无边时=0；高=存在大量高 S 节点，即 hub-of-hub 拓扑） |
| NSC | S-变体 Sum-Connectivity：邻域度和的倒数平方根（高=S-值较小且均匀；P₃ NR=NSC 为 S=2 均匀性的极简验证） |

---

## VectorAddress L4 命名空间（更新）

…108=graph-topo21, **109=graph-topo22**

---

## 下一步建议

可继续实现 topo23，候选 S-变体家族指数：
- **NA (Neighborhood Augmented Zagreb)**: Σ_{uv∈E} (S_u·S_v/(S_u+S_v−2))³  (S-analogue of AZI)
- **NHMI (Neighborhood HM₂)**: Σ_{uv∈E} (S_u·S_v)²  (S-analogue of HM₂)
- **NAG (Neighborhood AG)**: Σ_{uv∈E} (S_u+S_v)/(2√(S_u·S_v))  (S-analogue of AG; ≥|E| always)

均为 O(V+E) 度扫描，与当前框架完全兼容。
