# GOS 硬化日志 — V2.48（2026-07-02）

## 版本号: V2.48
## 功能: `graph mst` — Prim 最小生成森林

---

## 变更摘要

新增 `graph mst` —— 在活跃 GOS 内核图的无向投影上执行 Prim 最小生成森林算法。每条有向边被视为带其注册 `weight`（默认1.0）的无向边。不连通的分量各自获得独立的 MST 根，因此产出的是生成**森林**而非单一生成树。MST 总权重（所有选中边之和）以定点整数（× 1000）形式报告，确保 no_std/no_alloc 兼容。

Shell 别名：`graph mst` / `mst` / `gmst` / `graph tree mst` / `min spanning`

OS 类比：`ip route show metric` —— 保持所有内核子系统可达的最小成本路由骨架，类比为以最小总延迟/带宽成本构建的路由表。

**基础设施变更（V2.48）**：扩展 `GraphTopologySnapshot`，新增 `edge_weight: [f32; MAX_EDGES]` 字段，从 `topology_snapshot()` 中的 `EdgeRecord.spec.weight` 填充。未来需要权重的算法（流量、最短路径等）可直接使用该字段，无需额外的运行时加锁。

---

## 动机

结构分析套件（V2.41–V2.47）完成后，下一个自然的原语是**带权图算法**。内核图上每条边本已存储 `weight: f32`（`EdgeSpec.weight`，默认1.0），但在 V2.48 之前没有任何 API 将这些权重暴露给分析函数。

MST 在内核图 OS 中回答：
- 保持所有子系统连通的最小成本信号路由集合是什么？
- 哪些边是承重的（在 MST 中）、哪些是冗余的（不在 MST 中）？
- 内核子系统间通信的总最小带宽成本是多少？
- 哪些子系统处于独立的网络分区（多个 MST 根）？

MST 是后续带权原语的基础：最短路径（Dijkstra）、最大流（Ford-Fulkerson）、最小成本流。

---

## 算法：Prim 最小生成森林（无向投影）

```text
初始化：
  in_mst[v]      = false，对所有 v
  key[v]         = ∞，对所有 v
  parent_slot[v] = ∅，对所有 v
  remaining      = node_count

当 remaining > 0：
  u = argmin{ key[v] : v 不在 MST 中 }

  若 key[u] == ∞（新分量——u 没有到已有 MST 的边）：
    parent_slot[u] = u      // u 是新分量的根
    key[u] = 0.0

  in_mst[u] = true
  将 u 加入输出；记录 out_key[u] = key[u]
  remaining -= 1

  对每条与 u 相邻的活跃边 e（无向）：
    v = e 的另一端
    w = edge_weight[e]
    若 v 不在 MST 中 且 w < key[v]：
      key[v]         = w
      parent_slot[v] = u

构建输出：
  out_vecs[i]    = slot_vec[order[i]]
  out_parents[i] = slot_vec[parent_slot[order[i]]]
  out_weights[i] = (out_key[i] × 1000) as u32
  total_mst_w    = 所有非根 i 的 out_key[i] 之和 × 1000
```

**关键设计选择：**

1. **无向处理**：入边和出边均作为无向邻居连接，与 `graph spanning`、`graph community`、`graph bipartite` 一致。
2. **Prim 而非 Kruskal**：Prim 给出自然的访问顺序（节点按加入 MST 的顺序输出，按分量分组），且无需边排序——对 `no_std` 固定大小数组很重要。
3. **权重默认1.0**：未显式设置权重（`EdgeSpec.weight`）的边默认存储为1.0，使无权图上的 MST 等价于 BFS 生成树（但由于打平顺序的度数差异，结构未必完全相同）。
4. **定点输出（× 1000）**：避免在 `no_std` 内核显示层进行 f32 格式化打印，整数算术足以支持权重展示。
5. **打平规则**：多个节点 key 相同时最小 slot 索引者胜出，保证等权图上的确定性输出。
6. **根检测**：`parents[i] == vecs[i]` 且 `weights[i] == 0` 标识分量根节点，其余节点均为带正权重的子节点。

**复杂度**：O(V·E) —— O(V) 次外层迭代 × 每次迭代 O(E) 邻居扫描。n≤128、E≤512 时最多 65,536 次操作。
**空间**：O(MAX_NODES + MAX_EDGES) —— 固定大小栈数组，兼容 no_std/no_alloc。

---

## 实现细节

### `crates/gos-runtime/src/lib.rs`

**快照扩展：**
- `GraphTopologySnapshot.edge_weight: [f32; MAX_EDGES]` —— 新字段，初始化为 `1.0f32`（默认权重）
- `topology_snapshot()` 现在为每条活跃边把 `e.spec.weight` 拷贝到 `snap.edge_weight[i]`

**新内部函数：**
- **`RuntimeState::graph_mst_inner<const N>()`** —— Prim 生成森林：三阶段结构（找最小 key 未访问节点、标记入 MST 并输出、松弛邻居）；不连通分量检测（若所选节点 key 未初始化即为 INF，则开启新根，key=0）

**新公开函数：**
```rust
pub fn graph_mst<const N: usize>(
) -> ([VectorAddress; N], [VectorAddress; N], [u32; N], usize, u32)
```
锁定 RUNTIME，调用 `topology_snapshot()`，委托给 `graph_mst_inner`。

### `crates/k-shell/src/lib.rs`

- **`pub fn dispatch_graph_mst(sink)`** —— 展示函数：
  - 标题：青色 `graph mst`
  - 列标题：`role  weight  vector  parent`
  - 逐节点显示角色（洋红 `root` / 青色 `child`）、黄色 `W.mmm` 权重、白色向量、父节点（根节点为灰色 `(root)`）
  - 页脚：`N node(s)  Prim MST  total weight: W.mmm`

### `crates/k-shell/src/proc.rs`

- 路由（2行）：`"graph mst" | "mst" | "gmst" | "graph tree mst" | "min spanning"` → `dispatch_graph_mst`
- 帮助文本新增2行

---

## 测试用例（10/10 通过）：`host-tests/gos-graph-mst-harness`

| 编号 | 用例 | 验证点 |
|------|------|--------|
| 1 | 空图 | node_count=0, total_mst_w=0 |
| 2 | 单节点 | node_count=1, weight=0, parent=self |
| 3 | 两个孤立节点（无边） | total_mst_w=0，均为根 |
| 4 | K₂ 边权重=1.0 | total_mst_w=1000，一根一子 |
| 5 | K₂ 边权重=2.5 | total_mst_w=2500 |
| 6 | 路径 A─B─C，权重均=1.0 | total_mst_w=2000 |
| 7 | K₃ 三角形（权重1、2、3） | MST 选择 1+2=3（最重边被排除）；total=3000 |
| 8 | 两分量（A─B，C孤立） | total_mst_w=1000；C 为第二个根 |
| 9 | 根不变量 | weights[i]==0 时 parents[i]==vecs[i] |
| 10 | 连通性 | 每个非根节点的父节点均出现在输出向量中 |

**结果：10/10 通过，零告警**

---

## Shell 命令一览

```text
graph mst          Prim 最小生成森林 —— 最小成本路由骨架
mst                别名
gmst               别名
graph tree mst     别名
min spanning       别名
```

示例输出（路径 A─2─B─3─C）：

```text
 graph mst
 ─────────────────────────────────────────────────────────────
  role    weight    vector           parent
  root    0.000     [25:1:1:0]       (root)
  child   2.000     [25:1:2:0]       [25:1:1:0]
  child   3.000     [25:1:3:0]       [25:1:2:0]
 ─────────────────────────────────────────────────────────────
 3 node(s)  Prim MST  total weight: 5.000
```

---

## 基础设施：`GraphTopologySnapshot` 扩展

`edge_weight: [f32; MAX_EDGES]` 现已纳入在 RUNTIME 锁下捕获的拓扑快照，是一次**承重的基础设施变更**，使所有未来的带权图算法都能访问边权重而无需额外的运行时查询：

| 算法 | 使用 `edge_weight` |
|------|---------------------|
| V2.48 `graph_mst_inner` | 是 |
| 未来 `graph_shortest_path`（Dijkstra） | 是 |
| 未来 `graph_flow`（Ford-Fulkerson/Edmonds-Karp） | 是 |

---

## 不变量确认

- [x] 纯读操作：`graph_mst` 不推进 epoch，不做任何变更
- [x] 无堆分配 / no_std：所有缓冲区为固定大小栈数组
- [x] harness 使用标准的 `TEST_LOCK + reset()` 隔离方式
- [x] 版本顺序：V2.48 紧随 V2.47（图着色）
- [x] 文档归档路径：`doc/06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.48.md`

---

## 后续建议（V2.49 候选）

- `graph shortest <vec>` —— 从指定节点出发的 Dijkstra 最短路径树
- `graph flow <from> <to>` —— 两节点间最大流（Ford-Fulkerson）
- `node checkpoint <vec>` —— 快照节点状态到 diff ring
- `graph sim <N>` —— 模拟 N 步随机游走，输出信号流量轨迹

---

*由自动强化任务生成 · 2026-07-02*
