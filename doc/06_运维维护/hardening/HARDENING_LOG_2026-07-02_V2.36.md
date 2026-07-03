# GOS 硬化日志 — V2.36 — 2026-07-02

## 摘要

V2.36 通过 `graph reachable <vec>` 新增了传递可达性分析——这是 GOS 中
第一个图**闭包**运算。它回答的问题是"从该节点出发，经由有向边可以到达
哪些节点？"——图操作系统意义上等价于 `systemctl list-dependencies --all`、
`cargo tree -p <crate>` 或 `ldd --recursive`。这与 `graph path <from> <to>`
（点对点 BFS，V2.31）以及 `graph scc`（分量成员关系，V2.34）自然地构成
三元组。

---

## 修改内容

### 1. `graph_reachable_inner<N>` — gos-runtime（`crates/gos-runtime/src/lib.rs`）

`Runtime` 上新增的方法：

```rust
pub fn graph_reachable_inner<const N: usize>(
    &self,
    from: VectorAddress,
) -> ([VectorAddress; N], usize)
```

算法：
- 使用 `[bool; MAX_NODES]` 已访问位图的迭代式 DFS。
- 源节点最先被标记为已访问（防止在环上产生无限循环）。
- 所有新发现的邻居都被压入 DFS 栈，并加入可达集合（源节点自身除外）。
- 输出结果使用插入排序按 `VectorAddress.as_u64()` 升序排列
  （N ≤ 128，开销可忽略）。
- 若 `from` 未注册或没有出向路径，则返回 `(out, 0)`。
- 复杂度：O(V + E)，no_std 安全，仅使用固定大小的栈数组。
- 自环会被跳过（`if nbr_slot == cur_slot { continue; }`）。

### 2. `graph_reachable<N>` 公开 API — gos-runtime

```rust
pub fn graph_reachable<const N: usize>(from: VectorAddress) -> ([VectorAddress; N], usize)
```

薄封装：获取 `RUNTIME.lock()` 并委托给 `graph_reachable_inner`。
`N` 控制输出缓冲区深度；将其上限设为 `MAX_NODES = 128` 以实现全覆盖。

### 3. `dispatch_graph_reachable` — k-shell（`crates/k-shell/src/lib.rs`）

新增的展示函数：

```
 graph reachable from 15.1.1.0
 ───────────────────────────────────────────────────────────
  15.1.2.0
  15.1.3.0
  15.1.4.0
 ───────────────────────────────────────────────────────────
  3 reachable  |  use 'graph path <from> <to>' to trace a specific route
```

- 带颜色编码的标题（青色）、分隔线（暗灰色）、计数页脚。
- 空结果时打印：`(no reachable nodes — isolated or not registered)`。

### 4. Shell 路由 — k-shell（`crates/k-shell/src/proc.rs`）

新增的命令模式（分发顺序位于 `graph condensation` 之后）：

```
graph reachable <vec>   主形式
reachable <vec>         简短别名
reach <vec>             更简短的别名
graph reach <vec>       graph 前缀别名
```

`help` 文本新增两条条目：
```
  graph reachable <V>   all nodes reachable from V via directed edges (like systemctl list-dependencies --all)
  reachable <V>         alias for graph reachable
```

### 5. 测试套件 — `host-tests/gos-graph-reachable-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `unregistered_source_returns_empty` | `from` 不在 runtime 中时返回 0 |
| 2 | `isolated_node_returns_empty` | 无边节点 → 0 个可达节点 |
| 3 | `single_edge_reaches_one_node` | A→B：从 A 可达 = {B} |
| 4 | `chain_reaches_transitive_node` | A→B→C：从 A 可达 = {B, C} |
| 5 | `fan_out_reaches_both_children` | A→B, A→C：B 与 C 均可达 |
| 6 | `cycle_does_not_loop_forever` | A→B→A：能正常终止，返回 {B} |
| 7 | `triangle_reaches_all_other_members` | A→B→C→A：可达 = {B, C} |
| 8 | `reachable_from_midpoint_excludes_predecessor` | A→B→C→D 中的 B：可达 = {C,D}，不含 A |
| 9 | `disconnected_components_not_reached` | A→B 与 C→D 相互隔离：仅 {B} |
| 10 | `reachable_output_sorted_ascending` | 逆序排列的 4 节点扇出：输出已排序 |

---

## 验证

```
cd host-tests/gos-graph-reachable-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

回归验证——condensation 测试套件依然通过：
```
cd host-tests/gos-graph-condensation-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

---

## 生产质量考量

| 能力 | Linux/macOS 对应物 | GOS V2.36 |
|---|---|---|
| 传递依赖列表 | `systemctl list-dependencies --all <svc>` | `graph reachable <vec>` |
| 包闭包 | `cargo tree -p <crate>` | `graph reachable <vec>` |
| 共享库闭包 | `ldd --recursive <binary>` | `graph reachable <vec>` |
| 网络泛洪填充 | `traceroute --all-hops`（泛洪） | `graph reachable <vec>` |
| 环安全性 | BFS/DFS 终止不变量 | 已访问位图防止重复访问 |
| 排序顺序 | 稳定、可复现 | 按 VectorAddress（as_u64）升序 |

---

## 图算法套件（V2.32–V2.36）

| 版本 | 命令 | 回答的问题 | 算法 |
|---|---|---|---|
| V2.32 | `graph cycles` | "is there a cycle?" | DFS 三色法，O(V+E) |
| V2.33 | `graph toposort` | "dependency order?" | Kahn's BFS，O(V+E) |
| V2.34 | `graph scc` | "where are all cycles?" | Kosaraju 两遍 DFS，O(V+E) |
| V2.35 | `graph condensation` | "macro-structure?" | Kosaraju + 邻接矩阵 |
| **V2.36** | **`graph reachable`** | **"what can X reach?"** | **DFS 已访问位图，O(V+E)** |

至 V2.36，GOS 已拥有五个核心结构分析命令，覆盖了图论的基本问题：
连通性、排序、分量、层级结构与可达性。

---

## 图操作系统特性的保持

`graph reachable` 暴露了一个节点的**有向信号传播闭包**：即从 `<vec>`
出发的信号，沿所有出边传递后，最终会被哪些其他节点接收到。这是一种
独属于图论视角的操作系统 runtime 依赖观——传统操作系统均不提供此原语。

---

*自动化硬化流程 — GOS V2.36 — 2026-07-02*
