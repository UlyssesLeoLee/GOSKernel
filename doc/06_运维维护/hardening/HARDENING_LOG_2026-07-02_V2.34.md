# Hardening Log — V2.34 · 2026-07-02

## 摘要

实现 `graph scc` / `scc` / `graph components` shell 命令，通过 **Kosaraju 两遍 DFS** 算法计算图的**强连通分量（SCC）**。这是图论操作系统的核心图算法体系的又一重要扩展，与 V2.31（BFS 路径）、V2.32（DFS 环检测）、V2.33（Kahn 拓扑排序）共同构成完整的图分析工具集。

---

## 变更内容

### 1. `crates/gos-runtime/src/lib.rs`

#### 新增：`graph_scc_inner<const N: usize>()` 方法

- **算法**：Kosaraju 两遍 DFS（O(V+E)，no_std 安全，仅使用固定大小数组）
- **第一遍**：正向图 DFS，记录各节点完成顺序（finish_stack）
- **第二遍**：按逆完成顺序在**转置图**上 DFS，每棵 DFS 树 = 一个 SCC
- **自环处理**：自环被正确跳过，不影响 SCC 分组
- **输出格式**：`([VectorAddress; N], [u8; N], usize, usize)`
  - `nodes[0..total]` — 所有节点按 SCC 分组排列（SCC 0 优先）
  - `labels[0..total]` — 每个节点的 SCC 编号（单调非递减）
  - `total` — 活跃节点总数
  - `scc_count` — SCC 数量

#### 新增：`pub fn graph_scc<const N: usize>()` 公共 API

- 加锁调用 `RUNTIME.lock().graph_scc_inner()`
- 文档说明：当 `scc_count == total` 时图为 DAG（无有向环）

### 2. `crates/k-shell/src/lib.rs`

#### 新增：`pub fn dispatch_graph_scc(sink: &ConsoleSink)` 显示函数

- 调用 `gos_runtime::graph_scc::<128>()`
- **头部**：黑底青色 `GRAPH SCC` 标题
- **汇总行**：显示 `N 组件 / M 节点`，若为 DAG 则标注 `(graph is a DAG)`
- **每个 SCC 显示**：
  - 分组标题（SCC #N，节点数）
  - 多节点 SCC 标注 `◆ cycle`（红色警示）
  - 最多 4 列紧凑排列 vector 地址
  - 每个节点的 local_node_key + plugin_name
- **页脚提示**：有环时提示 `graph cycles`，无环时提示 `graph toposort`

### 3. `crates/k-shell/src/proc.rs`

#### 新增路由

```rust
} else if cmd == "graph scc" || cmd == "scc" || cmd == "graph components" || cmd == "components" {
    super::dispatch_graph_scc(sink);
```

支持别名：
- `graph scc`（标准格式）
- `scc`（快捷）
- `graph components` / `components`（语义别名）

### 4. `host-tests/gos-graph-scc-harness/`（新建）

新测试套件，10 个测试用例，全部通过：

| # | 测试名 | 验证内容 |
|---|--------|----------|
| 1 | `empty_graph_has_zero_components` | 空图 → 0 个 SCC，0 个节点 |
| 2 | `single_node_is_one_scc` | 孤立节点 → 1 个 SCC，大小为 1 |
| 3 | `self_loop_does_not_merge_sccs` | 自环节点 → 仍为大小 1 的 SCC |
| 4 | `two_node_mutual_cycle_is_one_scc` | A↔B 互向边 → 1 个 SCC，大小为 2 |
| 5 | `linear_chain_gives_singleton_sccs` | A→B→C 链 → 3 个单点 SCC |
| 6 | `triangle_cycle_is_one_scc_of_three` | A→B→C→A 三角环 → 1 个 SCC，大小为 3 |
| 7 | `diamond_dag_gives_four_singleton_sccs` | 菱形 DAG → 4 个单点 SCC |
| 8 | `triangle_plus_isolated_gives_two_sccs` | 三角环 + 孤立节点 → 2 个 SCC |
| 9 | `two_separate_cycles_give_two_sccs` | 两个独立环（A↔B 和 C→D→E→C）→ 2 个 SCC |
| 10 | `scc_count_equals_total_confirms_dag` | scc_count == 节点数 ↔ 图为 DAG |

---

## 图论背景

**强连通分量（SCC）** 是有向图中最强的连通概念：
- 一个 SCC 内所有节点两两互相可达
- **SCC 数量 == 节点数** → 图是 DAG（无有向环）
- **SCC 内节点数 > 1** → 该 SCC 包含有向环

Kosaraju 算法是最简洁的 SCC 实现之一，适合 no_std 环境：
1. 正向 DFS 记录完成时间（finish order）
2. 逆序处理，在转置图上 DFS，每棵 DFS 树即一个 SCC

---

## 与现有命令的关系

| 命令 | 功能 | 版本 |
|------|------|------|
| `graph cycles` | 检测是否有环，返回一条环路径 | V2.32 |
| `graph toposort` | Kahn BFS 拓扑排序（DAG 依赖顺序） | V2.33 |
| **`graph scc`** | **Kosaraju SCC 分解（找出所有强连通分量）** | **V2.34** |

三个命令形成完整的图结构分析三件套：
- "有没有环？" → `graph cycles`
- "所有的环在哪里？" → `graph scc`
- "依赖顺序是什么？" → `graph toposort`

---

## Shell 使用示例

```
# 计算图的强连通分量
graph scc

# 快捷别名
scc

# 语义别名
graph components
```

**输出示例**（含环图）：
```
 GRAPH SCC
 
 3 components  /  5 nodes

 SCC #0   3 nodes  ◆ cycle
   1.1.1.0  1.1.2.0  1.1.3.0
   svc.alpha  (my-plugin)
   svc.beta   (my-plugin)
   svc.gamma  (my-plugin)

 SCC #1   1 node
   1.1.4.0
   svc.delta  (my-plugin)

 SCC #2   1 node
   1.1.5.0
   svc.epsilon  (my-plugin)

  hint: graph cycles to trace a specific cycle path
```

---

## 测试结果

```
running 10 tests
test diamond_dag_gives_four_singleton_sccs ... ok
test empty_graph_has_zero_components ... ok
test linear_chain_gives_singleton_sccs ... ok
test scc_count_equals_total_confirms_dag ... ok
test triangle_cycle_is_one_scc_of_three ... ok
test single_node_is_one_scc ... ok
test self_loop_does_not_merge_sccs ... ok
test triangle_plus_isolated_gives_two_sccs ... ok
test two_node_mutual_cycle_is_one_scc ... ok
test two_separate_cycles_give_two_sccs ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

回归检查：
- `gos-graph-cycles-harness`: 10/10 ✅
- `gos-graph-toposort-harness`: 10/10 ✅
- `gos-runtime-harness`: 26/26 ✅

---

## 累计测试数量

**V2.34 后：343 个 host 测试（+ 10）**
