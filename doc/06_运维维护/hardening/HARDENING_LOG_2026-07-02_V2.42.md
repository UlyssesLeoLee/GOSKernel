# GOS 硬化日志 — V2.42（2026-07-02）

## 版本号: V2.42
## 功能: `graph katz` — 入向 Katz 中心性 (Incoming Katz Centrality)

---

## 变更摘要

新增 `graph katz` 命令及配套算法，完善图论算法套件（V2.32–V2.42）。
Katz 中心性通过对所有长度的有向游走计数，比传统最短路径度量（closeness、eccentricity）
更全面地刻画节点的间接影响力。

---

## 算法理论

### 定义

**入向 Katz 中心性 (Incoming Katz Centrality)**：

```text
KC[v] = Σ_{k=1}^{∞} α^k × (从任意节点出发、长度为 k 且终止于 v 的有向游走数)
```

其中 α = 1/8（衰减因子），保证级数对所有最大入度 < 8 的图收敛。

### 迭代计算（固定点法，20步，整数算术）

```text
x^(0)[v]   = 0
x^(t+1)[v] = Σ_{u: u→v 边} (SCALE + x^(t)[u]) / ALPHA_DEN
```

- `SCALE = 1_000_000`（×10⁻⁶ 单位）
- `ALPHA_DEN = 8`（即 α = 1/8）
- `K_ITERS = 20`（整数算术下约 8 步即收敛至稳态）

### 典型收敛值

| 场景 | KC（×10⁻⁶）|
|------|-----------|
| 无入边（孤立源节点）| 0 |
| 1条来自孤立源的入边 | 125_000 |
| 2条来自孤立源的入边 | 250_000 |
| 3条来自孤立源的入边 | 375_000 |
| 互相环 / 3-环节点 | ≈142_857 = SCALE/7 |
| 自环节点 | ≈142_857 = α/(1-α)×SCALE |

### 角色标注阈值

| 分值 | 角色 | 颜色 |
|------|------|------|
| `kc == 0` | `leaf`（无游走到达） | 暗灰 (8) |
| `0 < kc ≤ 1_000_000` | `relay`（中等影响力） | 青色 (11) |
| `kc > 1_000_000` | `hub`（高影响力，超过 α⁻¹ 归一化）| 亮黄 (14) |

---

## 实现细节

### 修改文件

| 文件 | 变更 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_katz_inner<const N>()` 方法（双缓冲迭代）+ `graph_katz<const N>()` 公开函数 |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_katz()` 调度函数（含颜色渲染） |
| `crates/k-shell/src/proc.rs` | 新增路由分支 + 帮助文本 |
| `host-tests/gos-graph-katz-harness/` | 全新 harness（10 测试，均通过） |
| `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.42.md` | 本文档 |

### Shell 命令别名

```text
graph katz      — 完整命令
katz            — 简写
kz              — 最短别名
graph influence — 语义别名
influence       — 语义简写
```

---

## 测试用例（10/10 通过）

| 编号 | 用例 | 验证点 |
|------|------|--------|
| 1 | 空图 | `total = 0` |
| 2 | 单孤立节点 | `kc[A] = 0` |
| 3 | 单边 A→B | `kc[B] = 125_000`，`kc[A] = 0`，B 排序第一 |
| 4 | 链 A→B→C | `kc[C]=140_625 > kc[B]=125_000 > kc[A]=0`，C 排序第一 |
| 5 | 扇入 {A,B,C}→D | `kc[D]=375_000`，D 排序第一 |
| 6 | 3-环 A→B→C→A | 三节点 kc 相等 ≈142_857 |
| 7 | 互向边 A↔B | `kc[A]=kc[B]` ≈142_857 |
| 8 | 分叉+链 A→B, A→C, B→D | `kc[D]>kc[B]=kc[C]>kc[A]`，D 排序第一 |
| 9 | 入度正比性 | 2条入边 = 2× 单条入边得分 |
| 10 | 自环 A→A | `kc[A] ≈ 142_857`（`α/(1-α)` 固定点） |

---

## 与 OS 层的类比

| GOS 命令 | Linux/BSD 类比 | 含义 |
|---------|---------------|------|
| `graph katz` | `netstat -s` (hop weight) | 哪个内核服务接收了跨所有路径长度累积的最多信号流量 |
| KC = 0（leaf）| 仅出站流量的端点 | 纯源节点，无传入依赖 |
| 0 < KC ≤ 1M（relay）| 中间路由节点 | 接收适度间接影响 |
| KC > 1M（hub）| 核心服务节点 | 高间接影响力，超过 α⁻¹ 归一化阈值 |

---

## 不变量确认

- [x] `dispatch_graph_katz` 为纯读操作，不触发 epoch bump，不写入运行时状态
- [x] 所有测试使用 `TEST_LOCK: Mutex<()>` + `reset()` 进行隔离，支持并发测试安全
- [x] harness 包含独立 `.cargo/config.toml`（`target = "x86_64-pc-windows-msvc"` + `build-std`）
- [x] 溢出安全：使用 `saturating_add`，高入度（≥8）图值截断至 u64::MAX→u32::MAX，排序仍正确
- [x] 向量地址空间：19.1.x.0（不与其他 harness 冲突）

---

## 图论算法套件完整状态 (V2.32–V2.42)

| 版本 | 命令 | 算法 | 复杂度 |
|------|------|------|--------|
| V2.32 | `graph cycles` | DFS 3-色有向环检测 | O(V+E) |
| V2.33 | `graph toposort` | Kahn BFS 拓扑排序 | O(V+E) |
| V2.34 | `graph scc` | Kosaraju 强连通分量 | O(V+E) |
| V2.35 | `graph condensation` | Kosaraju + 邻接扫描 | O(V+E) |
| V2.36 | `graph reachable` | 迭代 DFS 可达性 | O(V+E) |
| V2.37 | `graph bipartite` | BFS 2-着色 | O(V+E) |
| V2.38 | `graph degree` | 度数普查 | O(V×E) |
| V2.39 | `graph centrality` | Brandes 介数中心性 | O(V×E) |
| V2.40 | `graph closeness` | BFS 出向近接中心性 | O(V×(V+E)) |
| V2.41 | `graph eccentricity` | BFS 离心率+半径+直径 | O(V×(V+E)) |
| **V2.42** | **`graph katz`** | **迭代 Katz 入向中心性** | **O(K×V×E)** |

---

## 测试执行记录

```text
running 10 tests
test chain_katz_descending_ordering ... ok
test directed_cycle_all_nodes_same_katz ... ok
test fan_in_hub_has_highest_katz ... ok
test empty_graph_katz_total_is_zero ... ok
test fork_and_chain_katz_ordering ... ok
test mutual_edge_both_nodes_equal_katz ... ok
test isolated_node_has_zero_katz ... ok
test indegree_proportionality ... ok
test self_loop_contributes_to_katz ... ok
test single_edge_katz_receiver_is_nonzero ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

---

*由自动强化任务生成 · 2026-07-02*
