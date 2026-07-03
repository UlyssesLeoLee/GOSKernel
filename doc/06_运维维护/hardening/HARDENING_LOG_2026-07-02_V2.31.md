# GOS 硬化日志 — V2.31 — 2026-07-02

## 摘要

V2.31 为 runtime 新增了基于 BFS 的图路径查找，以及 `graph path <from> <to>`
shell 命令，使运维人员能够追踪任意两个节点 vector 地址之间的最短有向跳转
序列——图论意义上等价于 `traceroute`。

---

## 修改内容

### 1. `find_graph_path_inner<const N>` — gos-runtime（`crates/gos-runtime/src/lib.rs`）

`GraphRuntime` 上新增的私有方法：

```rust
pub fn find_graph_path_inner<const N: usize>(
    &self,
    from: VectorAddress,
    to: VectorAddress,
) -> ([VectorAddress; N], usize)
```

算法：
- **BFS** 遍历扁平边表，仅使用固定大小的栈数组（无堆分配，`no_std` 安全）。
- `visited: [bool; MAX_NODES]` + `prev: [usize; MAX_NODES]` 前驱追踪。
- 环形队列 `q: [usize; MAX_NODES]` 用作 BFS 前沿。
- 特殊情形处理：`from == to` → 平凡的单元素路径；`from`/`to` 未注册 → 返回 0。
- 路径重建：从 `to_slot` 沿 `prev[]` 回溯至 `from_slot`，再原地反转。
- 返回 `(path_array, path_length)`，其中 `path_length == 0` 表示未找到路径。

### 2. `find_graph_path<const N>` — 公开 API

```rust
pub fn find_graph_path<const N: usize>(
    from: VectorAddress,
    to: VectorAddress,
) -> ([VectorAddress; N], usize)
```

委托给 `RUNTIME.lock().find_graph_path_inner(from, to)`。

### 3. `dispatch_graph_path()` — k-shell（`crates/k-shell/src/lib.rs`）

新增公开函数 `dispatch_graph_path(sink, from, to)`：

- **标题横幅**：`GRAPH PATH  <from> → <to>`（黑底青色高亮）。
- **跳转列表**：每一跳一行，包含跳数编号、vector 地址、节点 key 与插件名。
  - 首尾两跳（端点）着绿色；中间跳转着黄色。
- **错误情形**：`no path found (nodes unreachable or not registered)`，红色显示。
- **页脚**：`N hop(s) | from: <vec> | to: <vec>`。

输出示例（A→B→C 链路）：
```
 GRAPH PATH  10.3.1.0 → 10.3.3.0

  hop  1   10.3.1.0    gp.alpha  (kl-graph-path-harness)
  hop  2   10.3.2.0    gp.beta   (kl-graph-path-harness)
  hop  3   10.3.3.0    gp.gamma  (kl-graph-path-harness)

 3 hops  |  from: 10.3.1.0  |  to: 10.3.3.0
```

### 4. Shell 路由 — k-shell（`crates/k-shell/src/proc.rs`）

- 在命令分发链中新增 `graph path <from> <to>` 分支。
- 解析以空白分隔的两个 `VectorAddress::parse()` 词元。
- 针对缺失或格式错误的 vector 参数给出错误提示。
- 在 `help` 输出中新增 `graph path` 条目。

### 5. 测试套件 — `host-tests/gos-graph-path-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `empty_graph_no_path` | 无边 → 路径长度为 0 |
| 2 | `self_path_returns_one_hop` | from == to → 长度为 1，包含 from |
| 3 | `direct_edge_returns_two_hops` | A→B → [A, B]，长度为 2 |
| 4 | `path_starts_with_from_vector` | path[0] == from_vector |
| 5 | `path_ends_with_to_vector` | path[len-1] == to_vector |
| 6 | `two_hop_chain_path` | A→B→C，查询 A→C → [A, B, C]，长度为 3 |
| 7 | `bfs_finds_shorter_path` | 同时存在 A→B→C 与 A→C 直连；BFS 选择长度为 2 的路径 |
| 8 | `unregistered_from_returns_zero` | 未知的 from_vec → 0 |
| 9 | `unregistered_to_returns_zero` | 未知的 to_vec → 0 |
|10 | `reverse_direction_not_traversable` | 有向图：仅存在 A→B；查询 B→A 返回 0 |

---

## 验证

```
cd host-tests/gos-graph-path-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

回归验证：
```
cd host-tests/gos-graph-diff-harness && cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cd host-tests/gos-edge-inspect-harness && cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

---

## 生产质量考量

| 能力 | Linux/macOS 对应物 | GOS V2.31 |
|---|---|---|
| 网络路径追踪 | `traceroute <host>` / `pathping <host>` | `graph path <from> <to>` |
| 图可达性 | `ip route get <dst>` | 边表上的 BFS |
| 逐跳可见性 | `traceroute -n` | 每跳显示 vector + 节点 key + 插件 |
| 方向感知 | 路由表方向 | 遵循边方向（不可逆向） |
| 最短路径 | 路由守护进程中的 dijkstra | BFS（单位权重） |

该 BFS 仅使用固定大小的栈数组（`visited[128]`、`prev[128]`、`q[128]`），
无堆分配，在 `no_std` 内核环境下是安全的，时间复杂度 O(V + E)，空间复杂度 O(V)。

---

## 图操作系统特性的保持

`graph path` 将**有向图拓扑**暴露为一等运维原语：图中每一条边都是可搜索
连通结构的一部分。GOS 并非追踪路由器中的 IP 数据包，而是在图节点之间
追踪**信号分发路径**——使图可达性成为与已有的 `ps`、`top` 及
traceroute 类似物并列的核心 shell 能力。

---

## Shell 命令面（V2.31 新增）

```
graph path <from> <to>    从 <from> 处的节点到 <to> 处的节点的 BFS 最短路径
                          （类似 traceroute / pathping）
```

---

*自动化硬化流程 — GOS V2.31 — 2026-07-02*
