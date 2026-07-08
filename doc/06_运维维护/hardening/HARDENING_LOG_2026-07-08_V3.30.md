# HARDENING LOG — V3.30 — 2026-07-08

## 版本
**V3.30** — Reverse Wiener Λ + Reciprocal Complementary Wiener RCW + Terminal Wiener TW

## 变更摘要

新增三个 Wiener 族变种拓扑指数，均通过两阶段 BFS 实现，支持断连图（按连通分量独立计算）。

### 新增：`gos_runtime::graph_topo_indices19()`

返回 `(rw: u64, rcw_ppm: u64, tw: u64, edge_count: usize, node_count: usize)`

| 指数 | 定义 | 类型 | 文献 |
|------|------|------|------|
| Λ(G) 反 Wiener | Σ_c [C(n_c,2)×D_c − W_c] | 精确 u64 | Randić et al. 2000 |
| RCW(G) 互补倒数 Wiener | Σ_{u<v,连通} floor(10^6/(D_c+1−d)) | floor ppm | Vukičević 2010 |
| TW(G) 端点 Wiener | Σ_{u<v, 两端均为悬挂点(deg=1)} d(u,v) | 精确 u64 | Gutman et al. 2004 |

#### 数学不变量

- **Λ=0** iff 所有分量 D_c=1（完全图块）或单节点（孤立点）
- **RCW(K_n)** = C(n,2)×10^6（所有对距离=1，分母=1）
- **TW=0** iff 图中悬挂节点（deg=1）少于 2 个
- **断连图**：Λ 和 RCW 按分量独立计算；TW 跨全图统计悬挂点对

#### 精度交叉验证表

| 图 | Λ | RCW(ppm) | TW |
|----|---|----------|----|
| 空图 | 0 | 0 | 0 |
| 单孤立点 | 0 | 0 | 0 |
| 边 A-B | 0 | 1_000_000 | 1 |
| P₃ | 2 | 2_000_000 | 2 |
| K₃ | 0 | 3_000_000 | 0 |
| K_{1,4} | 4 | 8_000_000 | 12 |
| P₄ | 8 | 2_999_999 | 3 |
| K₄ | 0 | 6_000_000 | 0 |
| 两孤立点 | 0 | 0 | 0 |
| K_{2,3} | 6 | 7_000_000 | 0 |

P₄ RCW 推导：D=3；floor(10^6/3)×3 + floor(10^6/2)×2 + 10^6 = 999_999 + 1_000_000 + 1_000_000 = 2_999_999

#### 算法

- **阶段 0**（O(V+E)）：连通分量检测；comp_id[ci]，comp_size[c]
- **阶段 1**（O(n(n+m))）：每节点 BFS → ecc[]（离心率）、comp_wiener[c]（每分量 Wiener）、TW（悬挂点对）
- 计算 comp_diam[c] = max(ecc[v]) for v in c
- **阶段 2**（O(n(n+m))）：每节点 BFS → RCW（利用已知 comp_diam）
- **最终**：Λ = Σ_c [C(n_c,2)×D_c − W_c]（无下溢：d(u,v)≤D_c 恒成立）
- 栈用量：adj[128](u128=2KB) + 辅助数组(~2KB) + dist/queue(256B) ≈ 4.5KB

### Shell 命令

`"graph topo19"` / `"gtopo19"` / `"reverse wiener"` / `"grw"` / `"reciprocal complementary wiener"` / `"grcw"` / `"terminal wiener"` / `"gtw"` / `"grwrcwtw"`

### VectorAddress

L4=106 for `gos-graph-topo19-harness`

### 显示样式

- 标题：bright-yellow
- Λ：bright-cyan（精确值；=0 时标注 "Λ=0: complete blocks"）
- RCW：bright-green（ppm 小数格式）
- TW：bright-magenta（精确值；=0 时标注 "TW=0: no pendant pairs"）

### OS 类比

- **Λ**：拓扑与完全互联的差距压力（=0 代表已全连接；越大代表远离最优）
- **RCW**：互补直径分之一的调和加权（高值=所有对之间有充裕直径余量）
- **TW**：悬挂叶节点间的聚合路由距离（=0 无叶端点；高=大量孤立叶节点远距分布）

## 新增文件

- `crates/gos-runtime/src/lib.rs` — `graph_topo_indices19_inner()` + `graph_topo_indices19()`
- `crates/k-shell/src/lib.rs` — `dispatch_graph_topo_indices19()`
- `crates/k-shell/src/proc.rs` — topo19 命令路由
- `host-tests/gos-graph-topo19-harness/` — 新 harness（10 测试，全绿）

## 测试结果

```
running 10 tests
test test_01_empty          ... ok
test test_02_single_node    ... ok
test test_03_single_edge    ... ok
test test_04_path_p3        ... ok
test test_05_triangle_k3    ... ok
test test_06_star_k14       ... ok
test test_07_path_p4        ... ok
test test_08_complete_k4    ... ok
test test_09_two_isolated   ... ok
test test_10_k23_bipartite  ... ok

test result: ok. 10 passed; 0 failed
```

## 累计测试数

**1273 host tests**（V3.29 的 1263 + 本次 10）
