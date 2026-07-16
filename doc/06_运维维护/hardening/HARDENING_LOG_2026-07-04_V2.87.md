# 硬化日志 V2.87 — 欧拉路径/回路检测

**日期：** 2026-07-04
**分支：** feat/vk-auto-live-surface
**提交：** c001568
**宿主测试总计：** 843（此前 833，+10）

---

## 功能：`graph eulerian` / `geulerian` / `eulerian` / `euler`

### 动机

V2.85 与 V2.86 新增了**结构容错性**原语（割点、割边）。V2.87 新增了一个
互补的**遍历完备性**原语：**欧拉路径/回路检测**——回答有向内核图
是否存在一条能恰好遍历每条边一次的路径。

这是图论中的一个经典结果（欧拉，1736 年，哥尼斯堡七桥问题），
与操作系统直接相关：

| 问题 | 操作系统类比 |
|---|---|
| 是否存在欧拉回路？ | 维护守护进程能否恰好访问每条 IPC 通道一次并返回起点？ |
| 是否存在欧拉路径？ | 一次单遍审计能否遍历每条依赖边而不重复经过？ |
| 两者都不存在？ | 该图存在孤立的子系统集群或度数不平衡——路由是不完整的。 |

在生产级图平台（NetworkX、igraph）中，欧拉检测是电路设计、DNA 组装、
网络审计调度中使用的核心原语。

---

## 算法：度数平衡 + 弱连通性（O(V+E)）

有向图的欧拉检测可归约为两个 O(V+E) 检查：

### 第一步 —— 度数统计

对每个存活节点，通过一次扫描边表来计算 `out_degree` 与 `in_degree`。
孤立节点（out+in = 0）被排除在后续所有检查之外。

### 第二步 —— 度数平衡分类

| 条件 | 分类 |
|---|---|
| 所有活跃节点：`out == in` | 潜在回路 |
| 恰好一个节点：`out - in = +1`（起点），恰好一个：`in - out = +1`（终点），其余平衡 | 潜在路径 |
| 任意节点：`|out - in| ≥ 2`，或存在超过 1 个起点/终点候选 | 两者皆非 |

### 第三步 —— 弱连通性检查

从第一个活跃节点开始进行无向 BFS，将每条有向边都当作无向边处理。
所有活跃节点都必须可达。如果任何活跃节点未被到达，则不论度数条件
如何，结果都是"两者皆非"。

**关键洞察：** 对于有向图，条件为：

```
欧拉回路：∀v: in_degree(v) == out_degree(v)  且  弱连通
欧拉路径：∃! s: out(s)−in(s)=1，∃! t: in(t)−out(t)=1，∀v≠s,t: 平衡  且  弱连通
```

**平凡情形：** 若不存在任何边，空路径平凡地满足回路条件
（`has_circuit = true`，`has_path = false`）。这适用于空图以及仅含孤立节点的图。

**复杂度：** O(V + E) —— 一次度数扫描边 + 一次无向 BFS。
**内存：** 所有数组均为栈上分配；无堆分配，no_std 安全。

---

## 返回值

```rust
pub fn graph_eulerian() -> (bool, bool, VectorAddress, VectorAddress, usize)
//                          has_circuit  has_path  start_vec  end_vec  node_count
```

| 字段 | 含义 |
|---|---|
| `has_circuit` | 是否存在欧拉回路（遍历所有边的闭合路径） |
| `has_path` | 是否存在欧拉路径（开放路径）；与 `has_circuit` 互斥 |
| `start_vec` | 路径起点顶点向量；若为回路或两者皆非，则为 `VectorAddress::new(0,0,0,0)` |
| `end_vec` | 路径终点顶点向量；若为回路或两者皆非，则为 `VectorAddress::new(0,0,0,0)` |
| `node_count` | 图中存活节点总数 |

---

## 实现

### crates/gos-runtime/src/lib.rs

**新增方法**（位于 `GraphRuntime` 内，即 `impl GraphRuntime` 中）：
```rust
pub fn graph_eulerian_inner(&self)
    -> (bool, bool, VectorAddress, VectorAddress, usize)
```

**新增公开函数：**
```rust
/// V2.87：对实时内核图进行欧拉路径/回路检测。
pub fn graph_eulerian() -> (bool, bool, VectorAddress, VectorAddress, usize) {
    RUNTIME.lock().graph_eulerian_inner()
}
```

**内部数组（均为栈上分配）：**

| 数组 | 类型 | 用途 |
|---|---|---|
| `node_slots[MAX_NODES]` | `[usize; 128]` | 存活节点的槽位索引 |
| `out_deg[MAX_NODES]` | `[u16; 128]` | 每个槽位的出度 |
| `in_deg[MAX_NODES]` | `[u16; 128]` | 每个槽位的入度 |
| `active_slots[MAX_NODES]` | `[usize; 128]` | 非孤立节点的槽位 |
| `visited[MAX_NODES]` | `[bool; 128]` | BFS 访问标记 |
| `bfs_queue[MAX_NODES]` | `[usize; 128]` | BFS 队列（基于数组实现） |

**关键不变量：**
- `active_count == 0` → 平凡回路情形（在 BFS 之前提前返回）。
- 度数差值使用 `i32` 运算；`match diff` 清晰地分派 0 / 1 / -1 / 其他情形。
- BFS 使用无向投影：同时跟随 `from_node == cur_id` 和 `to_node == cur_id` 的边。
- 自环保护：BFS 中跳过 `nbr_slot == cur_slot`。
- `circuit_degree_ok` 与 `path_degree_ok` 互斥：若 `imbalanced == 0` 则为回路；
  若 `imbalanced == 2` 且起点/终点有效则为路径。

### crates/k-shell/src/lib.rs

**新增函数** `dispatch_graph_eulerian(sink: &ConsoleSink)`：
- 标题：` graph eulerian`（青色）
- 若 `node_count == 0`：打印 `(no nodes registered)`（未注册任何节点）。
- 若 `has_circuit`：以绿色打印 ✓ `Eulerian circuit exists`（存在欧拉回路）+ 说明
  任意节点均可作为起点/终点。
- 若 `has_path`：以黄色打印 ✓ `Eulerian path exists (not a circuit)`（存在欧拉路径，
  非回路）+ `start <vec>  end <vec>`。
- 否则：以红色打印 ✗ `no Eulerian path or circuit`（不存在欧拉路径或回路）+ 诊断说明。
- 脚注：`nodes: N`（节点数：N）

**使用的 Unicode 字符：**
- `\u{2713}`（✓）—— 成功勾号
- `\u{2717}`（✗）—— 失败叉号
- `\u{2500}`（─）—— 水平分隔线

### crates/k-shell/src/proc.rs

**新增路由**（插入在 `graph bridges` / `gcute` 分发之后）：
```
graph eulerian  →  dispatch_graph_eulerian
geulerian       →  别名
eulerian        →  别名
euler           →  别名
```

---

## 测试装置：`host-tests/gos-graph-eulerian-harness`

**VectorAddress L4=63** 标识本装置的命名空间。

| 测试 | 图拓扑 | 期望结果 |
|---|---|---|
| 1 | 空图 | has_circuit=true（平凡情形），has_path=false |
| 2 | 单个孤立节点 A（无边） | has_circuit=true（平凡情形），has_path=false |
| 3 | 三角形 A→B→C→A | has_circuit=true（全部平衡） |
| 4 | 单边 A→B | has_path=true, start=A, end=B |
| 5 | 路径 A→B→C | has_path=true, start=A, end=C |
| 6 | 反向平行 A→B + B→A | has_circuit=true（两者 in=out=1） |
| 7 | 两条互不相连的边 A→B, C→D | 两者皆非（非弱连通） |
| 8 | 方形 A→B→C→D→A | has_circuit=true（全部平衡） |
| 9 | 棒棒糖形：三角形 A→B→C→A + 尾部 C→D | has_path=true, start=C, end=D |
| 10 | Hub→A, Hub→B, C→Hub（两个起点候选/两个终点候选） | 两者皆非（存在两个起点候选） |

**结果：** 10/10 通过。

---

## VectorAddress L4 命名空间更新

| L4 | 装置 |
|---|---|
| 61 | gos-graph-articulation-harness (V2.85) |
| 62 | gos-graph-bridges-harness (V2.86) |
| **63** | **gos-graph-eulerian-harness (V2.87)** |

---

## 关键图论事实

**欧拉定理（1736 年）：** 一个无向连通图存在欧拉回路，当且仅当每个顶点的度
均为偶数。这是图论中最古老的结果——哥尼斯堡七桥问题的解答。

**有向图欧拉条件（Hierholzer，1873 年）：**
- **回路：** 强连通（等价于弱连通 + 所有节点平衡）且
  ∀v: `in_degree(v) == out_degree(v)`。
- **路径：** 弱连通且恰好一个顶点满足 `out − in = 1`（源点），恰好一个满足
  `in − out = 1`（汇点），其余全部平衡。

**与其他 V2.x 指标的关系：**

| 指标 | 关系 |
|---|---|
| 桥（V2.86） | 存在任意桥的图不可能存在欧拉回路（移除桥会使图断开） |
| 传递性 / 聚类系数（V2.63/V2.75） | 高聚类系数 ≠ 欧拉图；度数平衡才是关键 |
| SCC（V2.34） | 欧拉回路 ↔ 弱连通 + 平衡；SCC 数 > 1 会阻止回路但不总是阻止路径 |
| 度中心性（V2.38） | 欧拉性 ↔ ∀v: `in_deg(v) == out_deg(v)` —— 纯粹的度数条件 |
| 围长（V2.69） | DAG（围长=∞）可以存在欧拉路径但永远不会存在欧拉回路 |

**平凡欧拉情形：** 空图（无边）平凡地满足回路条件，因为全称量词
`∀v: in == out` 在零个活跃节点上是空真的（vacuously true）。这在图论中
是标准约定，与 NetworkX 的行为一致（`nx.is_eulerian(empty) → True`）。

---

## 文献参考

- L. Euler，《与位置几何相关问题的解法》，Commentarii Academiae
  Scientiarum Imperialis Petropolitanae 8，1736。哥尼斯堡七桥问题——
  首次证明欧拉回路需要所有顶点度数均为偶数。
- C. Hierholzer & C. Wiener，《论如何不重复、不间断地遍历一条闭合折线》，
  Mathematische Annalen 6，1873。证明了有向图的欧拉条件，并给出了
  一种构造性的 O(E) 回路查找算法。
