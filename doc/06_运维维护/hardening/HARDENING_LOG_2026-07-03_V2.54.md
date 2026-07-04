# GOS 硬化日志 — V2.54（2026-07-03）

## 版本号: V2.54
## 功能: `graph attractor` — 吸引子集合分类

---

## 变更摘要

实现 `graph attractor` —— 基于 SCC 分解的缩点 DAG，将每个活跃内核节点分类为三种角色之一的**吸引子集合分类**。

**吸引子**（底部 SCC / 汇 SCC）是一个没有任何出边指向该分量之外节点的强连通分量。信号或执行流一旦进入吸引子便永远无法逃离——它是有向图的"陷阱"或"稳定不动点"。

**图论 OS 的关键洞察：** 每个有限有向图至少存在一个吸引子 SCC。孤立节点和只有自环的节点是平凡的吸引子 SCC。`graph attractor` 命令揭示哪些内核服务节点构成稳定环（吸引子）、哪些距离稳定一步之遥（drain），哪些远离任何稳定环（transient）。

---

## 节点角色分类

| 角色 | 值 | 定义 |
|------|----|------|
| **attractor** | 0 | 属于底部 SCC——无缩点出边；流无法离开 |
| **drain** | 1 | 该 SCC 有直接缩点边指向至少一个吸引子 SCC（距离稳定一步） |
| **transient** | 2 | 该 SCC 有出边，但没有一条直接指向吸引子 SCC（距离稳定 ≥2 跳） |

输出按角色升序排序（吸引子在前，drain 其次，transient 最后）。

---

## 算法

**Kosaraju 两遍 DFS + 两次缩点边扫描，总计 O(V+E)。**

1. **阶段1 —— 正向 DFS**：构建完成顺序栈（标准 Kosaraju 第一遍）
2. **阶段2 —— 转置图 DFS**：按逆完成顺序处理节点，分配 SCC ID（标准 Kosaraju 第二遍）
3. **阶段3a —— 缩点扫描**：对每条 `scc_id[from] ≠ scc_id[to]` 的活跃边，标记 `scc_has_out[scc_id[from]] = true`。自环边和 SCC 内部边被跳过。`scc_has_out == false` 的 SCC 即为吸引子 SCC。
4. **阶段3b —— drain 扫描**：对每条跨 SCC 边，若目标 SCC 是吸引子（`!scc_has_out[scc_id[to]]`），标记 `scc_adj_attract[scc_id[from]] = true`
5. **阶段4 —— 打包输出**：在稳定 slot 顺序内按角色顺序（0→1→2）输出节点

**正确性说明：**
- 自环不产生缩点边（`from_slot == to_slot` 保护）——只有自环的节点是平凡吸引子 SCC，分类正确
- 双向对 `A↔B` 构成单一 SCC；`A→B` 和 `B→A` 均为 SCC 内部边，不作为缩点边出现
- 孤立节点（无边）恒为吸引子 SCC

---

## 修改文件

### `crates/gos-runtime/src/lib.rs`

- **`GraphRuntime::graph_attractor_inner<N>()`** —— 私有方法，插入在 `graph_between_inner` 之后。实现 Kosaraju SCC + 两次缩点边扫描。返回 `([VectorAddress; N], [u8; N], usize, usize)` —— 节点、角色、总数、吸引子计数。
- **`pub fn graph_attractor<N>()`** —— 公开 API 包装，插入在 `pub fn graph_between` 之后，路由到 `RUNTIME.lock().graph_attractor_inner()`。

### `crates/k-shell/src/lib.rs`

- **`pub fn dispatch_graph_attractor(sink)`** —— 新展示函数，插入在 `dispatch_uname` 之前，调用 `gos_runtime::graph_attractor::<128>()`。配色：亮绿（10）=attractor，亮黄（14）=drain，暗灰（8）=transient。页脚显示各角色计数。

### `crates/k-shell/src/proc.rs`

- 在 Shell 命令分发链中新增 `graph attractor` 路由（紧随 `graph between` 分支之后）：
  `"graph attractor" | "attractor" | "gattractor" | "graph attract" | "attract"`

### `host-tests/gos-graph-attractor-harness/`（新建 harness，10 测试，L4=31 VectorAddress 命名空间）

| 编号 | 用例 | 关键断言 |
|------|------|----------|
| 1 | 空图 | total=0, attractor_count=0 |
| 2 | 单孤立节点 | role=0（attractor），attractor_count=1 |
| 3 | A→B 路径 | B=attractor(0)，A=drain(1) |
| 4 | A→B→C 路径 | C=attractor(0)，B=drain(1)，A=transient(2) |
| 5 | A↔B 双向 | 均为 role=0（单一吸引子 SCC） |
| 6 | 环 A→B→A + C→A | A、B=attractor(0)；C=drain(1) |
| 7 | 菱形 A→{B,C}→D | D=attractor(0)；B、C=drain(1)；A=transient(2) |
| 8 | 两个不连通环 | 全部4节点 attractor(0)；attractor_count=4 |
| 9 | 排序 | roles[i-1] ≤ roles[i]；attractor 在 drain 前，drain 在 transient 前 |
| 10 | 自环 A→A + 孤立 B | 均为 attractor(0)（自环 ≠ 缩点边） |

全部10项测试：**通过**（0.01秒）

---

## Shell 命令一览

| 命令 | 别名 |
|------|------|
| `graph attractor` | `attractor`、`gattractor`、`graph attract`、`attract` |

---

## OS 类比

`systemctl list-units --state=running` 的服务稳定性审计：
- **attractor** —— 永续运行的服务环，一旦进入永不离开（如 init、PID 1、定时器轮）
- **drain** —— 一步即可收敛到稳定环（如把控制权交给 init 的一次性初始化任务）
- **transient** —— 必须经过多个中间服务才能到达稳定状态

---

## 版本序列

```
V2.51  node checkpoint    — 快照节点状态到 diff ring
V2.52  graph sim          — xorshift32 随机游走模拟
V2.53  graph between      — 加权介数中心性（Brandes + Dijkstra）
V2.54  graph attractor    — 吸引子集合分类（Kosaraju + 缩点） ← 本次
```

**下一步（V2.55）**：PAL_U32 → attribute node 重构（Demo A 前置条件）
**累计宿主测试数**：513（503 + 10 新增）

---

*由自动强化任务生成 · 2026-07-03*
