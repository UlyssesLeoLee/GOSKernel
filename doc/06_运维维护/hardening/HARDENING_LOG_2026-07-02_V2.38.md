# GOS 硬化日志 — V2.38 — 2026-07-02

## 概述

V2.38 通过 `graph degree` 新增入/出度统计 —— 用于回答"每个节点的连接程度如何？哪些是流量枢纽？"。对每个存活节点，该命令统计有向出度（离开该节点的边数）和入度（进入该节点的边数），然后按总度数降序排列结果，使连接最紧密的枢纽节点排在最前面。

节点会自动被标注一个角色标签：
- **hub（枢纽）** —— 总度数 ≥ 3 且 ≥ ceiling(max_total/2)：连接最紧密的节点。
- **source（源头）** —— 无入边（out > 0，in == 0）：信号发起者。
- **sink（汇点）** —— 无出边（out == 0，in > 0）：终端消费者。
- **isolated（孤立）** —— 完全没有边（out == 0，in == 0）：不连通的节点。

操作系统类比：`ip -s link show`（每接口的 TX/RX 数据包计数）、`netstat -s`（每 socket 统计）或按地址细分的 `ss -s`。

---

## 修改内容

### 1. `graph_degree_inner<N>` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

`GraphState` 上新增的方法（插入在 `graph_bipartite_inner` 之后）：

```rust
pub fn graph_degree_inner<const N: usize>(
    &self,
) -> ([VectorAddress; N], [u16; N], [u16; N], usize)
```

**算法**：O(V × E) 的统计过程 —— 遍历一次所有存活边，同时解析 `from_node` 和 `to_node` 所在槽位，累加按槽位索引的度数计数器。然后按总度数降序对存活节点槽位列表进行插入排序。对于 V ≤ 128、E ≤ 512 的规模，O(V × E) 是可以接受的。

**自环**在入度和出度上都各计一次（`from_node == to_node` 的边会分别独立地递增 `slot_out[slot]` 和 `slot_in[slot]`），与标准有向图度数惯例一致。

**饱和处理**：度数使用 `u16` 类型配合 `saturating_add`，即使某节点累积超过 65535 条边也不会溢出。

**返回值布局**（与之前 graph_* 系列的约定保持一致）：
- `vecs[0..total]`        — 存活节点向量，按总度数降序排列。
- `out_degrees[0..total]` — 每个节点的有向出度。
- `in_degrees[0..total]`  — 每个节点的有向入度。
- `total`                 — 已打包的存活节点数量。

### 2. `pub fn graph_degree<const N>()` — gos-runtime 公开 API

```rust
pub fn graph_degree<const N: usize>() -> ([VectorAddress; N], [u16; N], [u16; N], usize) {
    RUNTIME.lock().graph_degree_inner()
}
```

与 `graph_scc`、`graph_condensation`、`graph_reachable`、`graph_bipartite` 风格一致的单行包装函数。

### 3. `dispatch_graph_degree` — k-shell (`crates/k-shell/src/lib.rs`)

新增的 shell 层调度函数（插入在 `dispatch_uname` 之前）。

输出格式（颜色编码：绿色=out，红色=in，黄色=hub，青色=sink）：
```
 graph degree
 ───────────────────────────────────────────────────────────
  vector           out    in   total  role
  6.1.0.0            3     2      5  hub
  1.1.0.0            2     1      3  hub
  2.1.0.0            0     2      2  sink
  7.1.0.0            1     0      1  source
  5.1.0.0            0     0      0  isolated
 ───────────────────────────────────────────────────────────
  5 node(s)  max-total-degree: 5  hubs: 2
```

纯读取操作 —— 不产生 epoch 递增，不涉及写操作。

### 4. 命令路由 — k-shell (`crates/k-shell/src/proc.rs`)

别名接在 `graph bipartite` 分支之后：

```
graph degree  |  degree  |  graph hub  |  hub
```

帮助文本已更新，加入说明与别名。

### 5. `gos-graph-degree-harness` — 新建 host-test crate

`host-tests/gos-graph-degree-harness/` —— 10 个集成测试，覆盖：

| # | 场景 | 预期结果 |
|---|----------|----------|
| 1 | 空图 | total=0，不发生 panic |
| 2 | 单个孤立节点 | out=0，in=0 |
| 3 | 单条边 A→B | A: out=1 in=0；B: out=0 in=1 |
| 4 | 路径 A→B→C | B 的总度数最高（2）；排在最前 |
| 5 | 自环 A→A | A: out=1，in=1（两侧都计数） |
| 6 | 扇出枢纽 H→{A,B,C} | H: out=3 in=0；排在最前 |
| 7 | 扇入枢纽 {A,B,C}→H | H: out=0 in=3；排在最前 |
| 8 | 双向 A⇄B | 每个节点：out=1，in=1 |
| 9 | 验证排序 | 输出按总度数严格非递增排列 |
| 10 | 菱形 A→B, A→C, B→D, C→D | 四个节点总度数均为 2；逐一验证每个节点的出/入度 |

全部 10 个测试：**通过**。

---

## 测试结果

```
running 10 tests
test bidirectional_edge_symmetric_degrees ... ok
test diamond_topology_degree_census ... ok
test empty_graph_degree_total_is_zero ... ok
test fan_in_hub_highest_degree ... ok
test fan_out_hub_highest_degree ... ok
test self_loop_counts_both_in_and_out ... ok
test output_sorted_descending_total_degree ... ok
test path_middle_node_highest_degree ... ok
test isolated_node_has_zero_degree ... ok
test single_edge_degrees ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

---

## Shell 命令一览（V2.38 新增）

| 命令 | 别名 | 说明 |
|---------|---------|-------------|
| `graph degree` | `degree`, `graph hub`, `hub` | 按总度数降序排列的入/出度统计；附带 hub/source/sink/isolated 角色标注 |

---

## 保持的不变式

- `dispatch_graph_degree` 是纯读取操作 —— 不产生 epoch 递增，不涉及写操作。
- 使用既有的 `TEST_LOCK: Mutex<()>` + `reset()` 隔离模式。
- Harness 的 `.cargo/config.toml` 设置 `target = "x86_64-pc-windows-msvc"` + `build-std = ["std", "panic_abort"]`。
- 版本号：V2.38（紧接在 V2.37 graph-bipartite 之后）。
- 节点度数数组使用 `u16` 配合 `saturating_add` 以保证溢出安全。

---

## 下一步计划

- `node checkpoint <vec>` —— 将节点状态快照到 diff ring（可观测性）
- `journal ring <N>` —— 运行时可配置的 JournalRing 容量
- `graph centrality` —— 介数中心性 / 接近中心性计算
- PAL_U32 → attribute node 重构（Demo A 前置条件）
