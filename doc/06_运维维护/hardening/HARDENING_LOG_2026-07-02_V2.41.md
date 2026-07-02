# GOS 硬化日志 — V2.41

**日期：** 2026-07-02
**分支：** feat/vk-auto-live-surface
**作者：** 计划性硬化任务（自动化）
**范围：** 图偏心率 —— 每节点最坏情况跳数 + 图半径/直径

---

## 1. 本次新增内容

### Shell 命令一览

| 命令别名 | 说明 |
|---|---|
| `graph eccentricity` / `eccentricity` / `graph ecc` / `ecc` / `graph radius` / `radius` | 每节点有向偏心率、图半径、图直径 |

输出格式（按偏心率升序排列，中心节点排最前）：

```text
 graph eccentricity
 ───────────────────────────────────────────────────────────
  vector              ecc   role
  6.1.3.0               1   center
  6.1.1.0               2   relay
  6.1.2.0               4   periphery
  6.1.4.0               0   isolated
 ───────────────────────────────────────────────────────────
  4 node(s)  radius: 1  diameter: 4  center: 1
```

**角色分类：**

| 角色 | 条件 | 颜色 |
|---|---|---|
| `center` | ecc == radius（且 radius > 0） | 亮黄色 |
| `relay` | 0 < ecc < diameter，且 ecc ≠ radius | 青色 |
| `periphery` | ecc == diameter（且 diameter ≠ radius） | 红色 |
| `isolated` | ecc == 0（没有可达的出邻居） | 深灰色 |

当 radius == diameter 时（例如一个有向环），所有非孤立节点都标注为 `center`。

---

## 2. 算法 —— `graph_eccentricity_inner<const N>`

**定义：**
```text
ecc[v] = max d(v, u)   对所有从 v 可达的 u（u ≠ v，经由有向边）
ecc[v] = 0             若没有任何 u 可达（孤立节点 / 纯汇点）

radius   = min ecc[v]  对 ecc[v] > 0 的 v（若所有节点均孤立则为 0）
diameter = max ecc[v]                            （若所有节点均孤立则为 0）
```

**方法：** 对每个源节点沿出向有向边执行一次 BFS。
**复杂度：** O(V × (V+E))，no_std 安全，仅使用静态数组。

**操作系统类比：** 类似于 `traceroute` 的最坏情况跳数 —— 哪个内核节点能保证到其所有可达对等节点的最大延迟最紧凑？

**排序方式：** 按偏心率升序排列，使中心节点排在最前。孤立节点（ecc=0）使用 u32::MAX 作为排序哨兵值，使其排在输出末尾。

---

## 3. 修改文件

| 文件 | 修改内容 |
|---|---|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_eccentricity_inner<N>`（impl 方法）+ `graph_eccentricity<N>`（公开函数） |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_eccentricity` |
| `crates/k-shell/src/proc.rs` | 为 `graph eccentricity` / `eccentricity` / `graph ecc` / `ecc` / `graph radius` / `radius` 新增调度分支 |
| `host-tests/gos-graph-eccentricity-harness/` | 新建 harness crate（10 个测试，全部通过） |

---

## 4. 测试套件 —— gos-graph-eccentricity-harness（10 个测试）

全部 10 个测试通过：`test result: ok. 10 passed; 0 failed`

| # | 测试 | 关键断言 |
|---|---|---|
| 1 | `empty_graph_eccentricity_total_is_zero` | total=0, radius=0, diameter=0 |
| 2 | `isolated_node_has_zero_eccentricity` | ecc[A]=0, radius=0, diameter=0 |
| 3 | `two_node_edge_eccentricity` | ecc[A]=1, ecc[B]=0; radius=diameter=1 |
| 4 | `path_abc_eccentricity` | ecc[A]=2, ecc[B]=1, ecc[C]=0; radius=1, diameter=2; 排序为 B,A,C |
| 5 | `star_center_eccentricity` | ecc[A]=1, 叶节点=0; radius=diameter=1 |
| 6 | `directed_cycle_all_nodes_same_eccentricity` | 所有 ecc=2; radius=diameter=2 |
| 7 | `diamond_eccentricity` | ecc[A]=2, ecc[B/C]=1, ecc[D]=0; radius=1, diameter=2 |
| 8 | `linear_five_node_chain_eccentricity_ordering` | ecc[D]=1..ecc[A]=4; 排序为 D,C,B,A,E |
| 9 | `disconnected_pairs_eccentricity` | ecc[A]=ecc[C]=1, 汇点=0; radius=diameter=1 |
| 10 | `self_loop_does_not_contribute_to_eccentricity` | ecc[A]=0, ecc[B]=1, ecc[C]=0; B 排最前 |

---

## 5. 保持的不变式

- 所有调度函数均为纯读取操作 —— 不产生 epoch 递增，不涉及写操作。
- 新 harness 使用 `TEST_LOCK: Mutex<()>` + `reset()`，配合 `unwrap_or_else(|e| e.into_inner())`。
- Harness 拥有自己的 `.cargo/config.toml`，设置 `target = "x86_64-pc-windows-msvc"` 及 `build-std`。
- 版本序列：V2.40=closeness → **V2.41=eccentricity**。下一版本：V2.42。

---

## 6. 图算法套件状态（V2.32–V2.41）

| 版本 | 命令 | 算法 |
|---|---|---|
| V2.32 | `graph cycles` | DFS 三色标记环检测 |
| V2.33 | `graph toposort` | Kahn BFS 拓扑排序 |
| V2.34 | `graph scc` | Kosaraju 两遍 DFS |
| V2.35 | `graph condensation` | SCC 缩点 DAG |
| V2.36 | `graph reachable <vec>` | 迭代式 DFS 可达性 |
| V2.37 | `graph bipartite` | BFS 二染色 |
| V2.38 | `graph degree` | 入/出度统计 |
| V2.39 | `graph centrality` | Brandes 介数中心性 |
| V2.40 | `graph closeness` | BFS 出向接近中心性 |
| **V2.41** | **`graph eccentricity`** | **BFS 偏心率 + 半径/直径** |

---

## 7. 下一步候选项（V2.42+）

- `node checkpoint <vec>` —— 将节点状态快照到 diff ring（可观测性）
- `journal ring <N>` —— 运行时可配置的 JournalRing 容量
- `graph katz` —— Katz 中心性（衰减因子 α，游走长度加权）
- PAL_U32 → attribute node 重构（Demo A 前置条件）
