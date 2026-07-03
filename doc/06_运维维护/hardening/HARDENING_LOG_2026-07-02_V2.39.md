# GOS 硬化日志 — V2.39 — 2026-07-02

## 概述

V2.39 通过 `graph centrality` 新增介数中心性（betweenness centrality）—— 用于回答"内核服务图中哪个节点处于最多最短通信路径之上？"。

介数中心性 BC[v] = Σ_{s≠v≠t} σ(s,t,v)/σ(s,t)，其中 σ(s,t) 是从 s 到 t 的最短有向路径数量，σ(s,t,v) 是这些路径中经过 v 的数量。BC 值高的节点是结构性瓶颈：移除它会破坏最多的节点间路由路径。

实现采用 Brandes 2001 年提出的算法（O(V×E)，有向、无权），并使用定点数算术（SCALE = 1_000_000）以避免在 no_std 内核环境中使用浮点数。

操作系统类比：`traceroute` 的跳数频率分析、网络拓扑中的 BGP 介数、或 `htop` 进程树的关键路径识别。

---

## 修改内容

### 1. `graph_centrality_inner<N>` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

`GraphState` 上新增的方法（插入在 `graph_degree_inner` 之后）：

```rust
pub fn graph_centrality_inner<const N: usize>(
    &self,
) -> ([VectorAddress; N], [u32; N], usize)
```

**算法：Brandes 介数中心性算法（有向、无权）**

针对每个源节点 s：

1. **BFS 正向阶段** —— 计算 `dist[v]`（从 s 到 v 的最短路径距离）和 `sigma[v]`（从 s 到 v 的不同最短路径数量）。将 BFS 遍历顺序累积到 `bfs_ord[]` 中，供反向传播阶段使用。

2. **反向传播阶段**（按 BFS 顺序的逆序）—— 对每个节点 w ≠ s，扫描所有满足 `dist[w] == dist[v] + 1` 的入边 (v → w)（即 v 是 w 在以 s 为根的最短路径 DAG 中的前驱节点）：
   ```
   delta[v] += sigma[v] × (SCALE + delta[w]) / sigma[w]
   ```
   这是 Brandes 的成对依赖递推公式，使用整数运算结合定点数缩放（SCALE = 1_000_000）计算，以避免出现分数。
   注意：乘法必须先于除法执行 —— 若先计算 sigma[v]/sigma[w] 的比值，在多数情况下会因整数截断而得到 0。
   将结果累加进 `bc_scaled[w] += delta[w]`（针对每个 w ≠ s）。

3. **输出阶段** —— 用 `bc_scaled[slot] / SCALE` 得到 BC[v] 的整数截断值。节点按原始（未截断的）`bc_scaled` 降序排列，以确保定点数舍入导致的并列情况仍能保持自然排序。

**复杂度**：O(V × (V + E)) —— 共 O(V) 次 BFS 遍历，每次 O(V + E)。
对于 V=128、E=512 的规模：每次 BFS 约 8.1 万次操作 × 128 个源节点 ≈ 总计约 1000 万次操作。
在内核 MAX_NODES=128 / MAX_EDGES=512 的边界内可以接受。

**溢出安全**：`sigma` 使用 `u64`（从 u32 扩宽）配合 `saturating_add`，以防止在分层图中最短路径计数逐层复合导致溢出；`delta` 和 `bc_scaled` 使用 `u64` 配合 `saturating_mul`/`saturating_add`。

**返回值布局**：
- `vecs[0..total]` — 存活节点向量，按介数降序排列。
- `bc[0..total]`   — 每个节点截断后的整数介数值（raw_scaled / SCALE）。
- `total`          — 已打包的存活节点数量。

### 2. `pub fn graph_centrality<const N>()` — gos-runtime 公开 API

```rust
pub fn graph_centrality<const N: usize>() -> ([VectorAddress; N], [u32; N], usize) {
    RUNTIME.lock().graph_centrality_inner()
}
```

与 `graph_degree`、`graph_bipartite` 等风格一致的单行包装函数。

### 3. `dispatch_graph_centrality` — k-shell (`crates/k-shell/src/lib.rs`)

新增的 shell 层调度函数（插入在 `dispatch_uname` 之前）。

输出格式（颜色编码：黄色=bottleneck，青色=relay，灰色=endpoint）：
```
 graph centrality
 ───────────────────────────────────────────────────────────
  vector                bc    role
  16.1.7.0               9  bottleneck
  16.1.1.0               0  endpoint
  16.1.2.0               0  endpoint
  16.1.3.0               0  endpoint
  16.1.4.0               0  endpoint
  16.1.5.0               0  endpoint
  16.1.6.0               0  endpoint
 ───────────────────────────────────────────────────────────
  7 node(s)  max-bc: 9  bottlenecks: 1
```

角色标注：
- **bottleneck（瓶颈）** —— BC == max_bc > 0：最关键的路由中介节点。
- **relay（中继）** —— BC > 0 但非最大值：承载部分跨节点流量。
- **endpoint（端点）** —— BC = 0：叶子节点、源节点、汇点或孤立节点。

同时新增 `print_num_right6()` 辅助函数，用于 6 列右对齐数字的打印。

纯读取操作 —— 不产生 epoch 递增，不涉及写操作。

### 4. 命令路由 — k-shell (`crates/k-shell/src/proc.rs`)

别名接在 `graph degree` 分支之后：

```
graph centrality  |  centrality  |  graph central  |  central  |  betweenness
```

### 5. `gos-graph-centrality-harness` — 新建 host-test crate

`host-tests/gos-graph-centrality-harness/` —— 10 个集成测试，覆盖：

| # | 场景 | 预期结果 |
|---|----------|----------|
| 1 | 空图 | total=0，不发生 panic |
| 2 | 单个孤立节点 | BC=0，total=1 |
| 3 | 两个节点 A→B | BC[A]=BC[B]=0（不可能存在中介节点） |
| 4 | 路径 A→B→C | BC[B]=1（B 是 A→C 唯一的中介节点） |
| 5 | 瓶颈 {A,B}→X→{C,D} | BC[X]=4（4 组跨层节点对全部经过 X） |
| 6 | 瓶颈 {A,B,C}→X→{D,E,F} | BC[X]=9（9 组跨层节点对全部经过 X） |
| 7 | 线性 5 节点 A→B→C→D→E | BC[C]=4，BC[B]=BC[D]=3，BC[A]=BC[E]=0 |
| 8 | 分叉-汇合 A→{B,C}→E→F | BC[E]=3（分叉后所有通往 F 的路径的瓶颈） |
| 9 | 验证排序 | 输出按 BC 值严格非递增排列 |
| 10 | 自环 A→A + A→B | BC[A]=0（自环不构成合法的 s≠v≠t 路径） |

全部 10 个测试：**通过**。

---

## 测试结果

```
running 10 tests
test bottleneck_three_into_three_centrality_nine ... ok
test bottleneck_two_into_two_centrality_four ... ok
test empty_graph_centrality_total_is_zero ... ok
test fork_join_bottleneck_centrality ... ok
test isolated_node_has_zero_centrality ... ok
test linear_five_node_path_centrality_values ... ok
test output_sorted_descending_by_bc_score ... ok
test path_abc_middle_node_centrality_is_one ... ok
test self_loop_does_not_panic_and_bc_is_zero ... ok
test two_nodes_one_edge_both_zero_centrality ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

---

## Shell 命令一览（V2.39 新增）

| 命令 | 别名 | 说明 |
|---------|---------|-------------|
| `graph centrality` | `centrality`, `graph central`, `central`, `betweenness` | Brandes 介数中心性算法，按降序排列；附带 bottleneck/relay/endpoint 角色标注 |

---

## 介数中心性 —— 实例演算

### 路径图 A→B→C→D→E（测试 7）

BC[B] = 3：节点对 (A,C)、(A,D)、(A,E) 各自唯一的最短路径都经过 B。从任何其他源节点出发，B 都不在最短路径上（在有向路径图中，从 C、D 或 E 都无法到达 B）。

BC[C] = 4：节点对 (A,D)、(A,E)、(B,D)、(B,E) —— C 是结构上的中点。

BC[D] = 3：节点对 (A,E)、(B,E)、(C,E) —— D 是倒数第二个节点。

### 瓶颈图 {A,B,C}→X→{D,E,F}（测试 6）

X 是从任意源层节点到任意汇层节点的*唯一*路径。
9 组有序节点对 (A,D)、(A,E)、(A,F)、(B,D)、(B,E)、(B,F)、(C,D)、(C,E)、(C,F) 各自贡献 σ(s,t,X)/σ(s,t) = 1/1 = 1 到 BC[X]。总和 = 9。

---

## 保持的不变式

- `dispatch_graph_centrality` 是纯读取操作 —— 不产生 epoch 递增，不涉及写操作。
- 使用既有的 `TEST_LOCK: Mutex<()>` + `reset()` 隔离模式。
- Harness 的 `.cargo/config.toml` 设置 `target = "x86_64-pc-windows-msvc"` + `build-std = ["std", "panic_abort"]`。
- 版本号：V2.39（紧接在 V2.38 graph-degree 之后）。
- 所有运算均使用饱和运算以防止在稠密图中发生溢出。
- `sigma` 为 `u64` 类型，以防止在存在大量平行最短路径的分层图中发生溢出。
- SCALE = 1_000_000 为存在多条等长路径（菱形拓扑、平行路由）的图保留了小数精度。

---

## 下一步计划

- `node checkpoint <vec>` —— 将节点状态快照到 diff ring（可观测性）
- `journal ring <N>` —— 运行时可配置的 JournalRing 容量
- `graph closeness` —— 接近中心性（最短路径距离之和的倒数）
- PAL_U32 → attribute node 重构（Demo A 前置条件）
