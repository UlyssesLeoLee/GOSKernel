# HARDENING LOG — V2.76 | 2026-07-03

## 版本 / Version
**V2.76** — Graph Local Efficiency (Latora–Marchiori 2001)

## 变更摘要 / Change Summary

### 新增功能 / New Feature

**图论操作系统新增：图局部效率指标 (Graph Local Efficiency)**

本次迭代实现 Latora–Marchiori (2001) 定义的图局部效率：

```
E_loc(G) = (1/n) × Σ_v E(G_v)
```

其中 G_v 是节点 v 的（无向）邻居集合在原图上导出的**有向**子图，
E(G_v) 是 G_v 的全局效率：

```
E(G_v) = Σ_{i≠j ∈ N(v)} 1/d_{G_v}(i,j) / (|N(v)| × (|N(v)|−1))
```

与全局效率 E(G) 互补：E(G) 衡量网络整体信息传输能力，E_loc(G) 衡量局部容错能力——当任意节点失效时，其邻居之间仍能互相通信的程度。

### 实现详情 / Implementation Details

**crates/gos-runtime/src/lib.rs**
- 新增 `graph_local_efficiency_inner()` → `(u32, usize, usize)`
  - 返回 `(eloc_ppm, nodes_computed, node_count)`
  - `eloc_ppm`: E_loc × 1_000_000（0=完全断开，1_000_000=双向完全图）
  - `nodes_computed`: 无向度数 ≥ 2 的节点数
  - `node_count`: 全体存活节点数（分母）
- 新增 `pub fn graph_local_efficiency()` 公开包装函数

**算法 (O(n × k² × E) where k = avg degree):**
1. 对每个节点 v，收集无向邻居集合 N(v)（去重、去自环）
2. 若 |N(v)| < 2，贡献 0，跳过
3. 对 N(v) 中每个节点 i 做 BFS，**仅在 N(v) 导出的有向子图中游走**
4. 对可达对 (i,j) 累计 1_000_000/d(i,j)，除以 |N(v)|×(|N(v)|−1) 得 E(G_v)
5. 累计所有 E(G_v) 除以 n

**整数算术，no_std 安全，无浮点运算。**

**crates/k-shell/src/lib.rs**
- 新增 `dispatch_graph_local_efficiency(sink)` 格式化输出

**crates/k-shell/src/proc.rs**
- 新增命令路由：`"graph local efficiency"` / `"graph local eff"` / `"gleff"` / `"local efficiency"`
- 更新 help 文本

**host-tests/gos-graph-local-eff-harness/**
- `Cargo.toml` — 标准独立 workspace
- `.cargo/config.toml` — x86_64-pc-windows-msvc + build-std
- `tests/graph_local_efficiency.rs` — 10 个测试用例

### 典型值 / Key Values

| 图结构 | E_loc |
|--------|-------|
| 空图 / 孤立节点 / 度数 < 2 的图 | 0.000000 |
| 有向三角形 A→B,B→C,A→C | 0.500000 |
| 双向完全三角形（6 条有向边） | 1.000000 |
| 有向 K4（6 条单向边） | 0.500000 |
| 四环 A→B→C→D→A | 0.000000 |
| 星形 A→{B,C,D} | 0.000000 |

## 测试结果 / Test Results

```
running 10 tests
test bidirectional_triangle_local_eff ... ok
test complete_k4_local_eff ... ok
test directed_triangle_local_eff ... ok
test empty_graph_zero_local_eff ... ok
test four_cycle_zero_local_eff ... ok
test path_three_nodes_zero_local_eff ... ok
test single_node_zero_local_eff ... ok
test star_zero_local_eff ... ok
test triangle_plus_isolated_local_eff ... ok
test two_nodes_one_edge_zero_local_eff ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured
```

## VectorAddress 命名空间 / VectorAddress Namespace

**L4=52** — gos-graph-local-eff-harness

## 关键不变量 / Key Invariants

- **BFS 严格限制在邻居子图**：只遍历两端均在 N(v) 内的有向边
- **分母安全**：nb × (nb-1) ≥ 2，因为 nb ≥ 2 时才进入计算
- **整数除法精度**：用 1_000_000/dist 而非浮点，与全局效率保持一致
- **denominator = n**（全部存活节点），而非 nodes_computed
- **孤立节点（度 < 2）贡献 0** 但仍计入分母 n

## 图论意义 / Graph Theory Significance

局部效率是 Watts-Strogatz 小世界网络理论的效率对偶量：
- E_loc ≈ 1：小世界网络的标志（高聚类，容错性强）
- E_loc ≈ 0：树形结构或稀疏无三角形网络
- E_loc > E_global：典型小世界特征

与已有指标关系：
- 互补于 `graph_global_efficiency` (V2.74) — 两者共同刻画网络的信息传输特性
- 关联 `graph_avg_clustering` (V2.75) — 均基于邻居子图，但 E_loc 使用最短路径而非三角形计数
- 关联 `graph_clustering` / `graph_transitivity` (V2.61/V2.63) — E_loc=1.0 ⟹ 邻居完全双向互连

## 下一步 / Next Steps

- Graph local efficiency per-node breakdown (`graph local efficiency verbose`)
- Graph small-world coefficient σ = (CC/CC_rand) / (L/L_rand)
- Graph network diameter summary view (combined center + peripheral)
