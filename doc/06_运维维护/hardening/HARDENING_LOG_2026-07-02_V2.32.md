# GOS 硬化日志 — V2.32 — 2026-07-02

## 摘要

V2.32 通过迭代式三色 DFS，为 GOS runtime 图新增了**有向环检测**能力，
并暴露出 `graph cycles` / `cycles` shell 命令，类似于 `tsort` 检测
循环依赖，或 `cargo` 的依赖环错误。这是图操作系统最基础的图论安全检查：
循环信号路由会造成死锁；循环依赖声明会阻碍确定性的启动顺序；循环重写
规则链会导致无限振荡。新 API 使运维人员能够随时确认当前活跃图是否为 DAG。

---

## 修改内容

### 1. `find_graph_cycle_inner<const N>` + `is_cyclic_inner` — gos-runtime

`GraphRuntime` 上新增方法（`crates/gos-runtime/src/lib.rs`）：

```rust
pub fn find_graph_cycle_inner<const N: usize>(&self) -> ([VectorAddress; N], usize)
pub fn is_cyclic_inner(&self) -> bool
```

**算法**：带三色节点标记的迭代式 DFS：
- `WHITE`（0）= 未访问
- `GRAY`（1）= 位于当前 DFS 路径上（祖先节点）
- `BLACK`（2）= 已完全展开

*回边*——从当前 GRAY 节点指向任一 GRAY 祖先的边——会闭合一个环。检测到
回边后，通过在当前路径中找到回边目标节点的下标，并从该下标切片至当前
位置，再将目标节点追加一次以闭合环路（使得 `path[0] == path[len-1]`），
从而在 DFS 栈中重建环路径。

特性：
- **O(V+E)** 时间复杂度，与 V2.31 的 BFS 路径查找渐进代价相同。
- **no_std 安全**——所有工作存储均为固定大小的栈数组。
- **无递归**——显式 DFS 栈避免了大型图上的栈溢出。
- 能正确处理自环（A→A）、多节点环，以及带有孤立环状分量的非连通图。

`is_cyclic_inner` 委托给 `find_graph_cycle_inner::<2>()`（容量为 2 的
路径），以最小的栈内存分配检测任意环的存在。

### 2. 公开 API — gos-runtime

```rust
pub fn find_graph_cycle<const N: usize>() -> ([VectorAddress; N], usize)
pub fn is_cyclic() -> bool
```

`find_graph_cycle` 锁定 `RUNTIME`，调用 `find_graph_cycle_inner`，并返回
`(path, length)`，其中 `length == 0` 表示图是无环的。
`is_cyclic` 是仅返回布尔值的便捷包装函数。

### 3. `dispatch_graph_cycles` shell 命令 — k-shell（`crates/k-shell/src/lib.rs`）

输出格式：
- 横幅：`GRAPH CYCLES`（青色标题）
- **DAG 情形**：绿色 "no cycles detected  (directed acyclic graph)"
- **有环情形**：红色 "CYCLE DETECTED  N nodes"，随后逐个列出环中的节点
  编号并以箭头展示流向，在回边节点处以 ↩ 符号闭合

颜色编码：
- 无环时标题为绿色
- 检测到环时为红色
- 环中间节点为黄色
- vector 地址为青色
- 非闭合跳转之间用 ↓ 箭头，闭合回边处用 ↩

### 4. Shell 路由 — k-shell（`crates/k-shell/src/proc.rs`）

命令分发器中新增的分支：

```
"graph cycles" | "cycles" | "graph cyclic" | "cyclic"
    → dispatch_graph_cycles(sink)
```

help 文本已更新：
```
graph cycles       detect directed cycles in the graph (like tsort cycle-check)
```

### 5. 测试套件 — `host-tests/gos-graph-cycles-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `empty_graph_no_cycle` | 空 runtime 返回 cycle_len == 0 |
| 2 | `single_node_no_cycle` | 无边的孤立节点 → 无环 |
| 3 | `linear_chain_is_acyclic` | A→B→C 链 → DAG，无环 |
| 4 | `self_loop_is_cyclic` | A→A 自环被检测到，长度 >= 2 |
| 5 | `two_node_cycle_detected` | A→B→A 被检测到，长度 >= 3 |
| 6 | `three_node_cycle_detected` | A→B→C→A 被检测到，长度 >= 4 |
| 7 | `diamond_dag_is_acyclic` | A→B, A→C, B→D, C→D（菱形 DAG）→ 无环 |
| 8 | `mixed_dag_and_cycle_detected` | DAG 子图 + 孤立的 D→E→D 环 → 被检测到 |
| 9 | `is_cyclic_false_for_dag` | 对纯 DAG，`is_cyclic()` 返回 false |
|10 | `is_cyclic_true_when_cycle_exists` | 存在 A→B→C→A 时，`is_cyclic()` 返回 true |

---

## 验证

```
cd host-tests/gos-graph-cycles-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

回归验证（graph-path harness）：
```
cd host-tests/gos-graph-path-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

---

## 生产质量考量

| 能力 | Linux/macOS 对应物 | GOS V2.32 |
|---|---|---|
| 循环依赖检测 | `tsort` / `cargo check` | `graph cycles` shell 命令 |
| DAG 验证 | 构建系统中的 `toposort` 断言 | `is_cyclic()` API + shell |
| 死锁路径检测 | `systemd` 依赖环检查 | 在信号图上执行 `graph cycles` |
| 算法 | DFS 三色法 | 迭代式 DFS，O(V+E)，no_std |
| 输出 | `tsort: input contains a loop:` | 带 vector 地址 + 节点 key 的环路径 |

`find_graph_cycle` 函数采用了与 BFS 路径查找（V2.31）及 diff-ring（V2.13）
相同的栈数组方案——无堆分配、无递归、编译期常量工作内存——从而保持了
图操作系统确定性的资源占用。

---

## 图操作系统特性的保持

`graph cycles` 直接作用于活跃图的**有向边拓扑**——而非进程列表或文件
系统层级结构。环路径输出展示节点 vector 与插件 key，将抽象的图论概念
落地于 runtime 的实际拓扑之中。这使可观测性面始终扎根于定义 GOS 的
图模型。

---

*自动化硬化流程 — GOS V2.32 — 2026-07-02*
