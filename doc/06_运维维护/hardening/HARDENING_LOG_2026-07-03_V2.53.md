# GOS 硬化日志 — V2.53（2026-07-03）

## 版本号: V2.53
## 功能: `graph between` — 加权介数中心性（Brandes + Dijkstra）

---

## 变更摘要

实现 `graph between` —— 通过 Brandes 算法结合每个源节点的 O(V²) Dijkstra 计算**加权介数中心性**。与现有的 `graph centrality`（V2.39，无权 BFS Brandes）互补，在寻找最短路径时遵循 `edge.spec.weight`。

**核心区别：**
- `graph centrality`（V2.39）—— 最小*跳数*路径（BFS），不感知权重
- `graph between`（V2.53）—— 最小*加权*路径（Dijkstra），感知权重

当一条低权重的间接路径比一条高权重的直接边更便宜时，两种算法会给出不同结果；在等权图上两者结果一致。

---

## 算法

**结合 Dijkstra 的 Brandes 算法**（有向、加权介数）：

```
WBC[v] = Σ_{s≠v≠t} σ_w(s,t,v) / σ_w(s,t)
```

其中 `σ_w(s,t)` 计数从 s 到 t 的最小权重有向路径数。

对每个源节点 s：
1. **前向遍历** —— O(V²) Dijkstra（无堆）：
   - 求 `dist[v]` = 从 s 到 v 的最短加权距离
   - 追踪 `sigma[v]` = 从 s 到 v 的最小权重路径数
   - 记录 `stk[]` = 按 dist 非递减顺序排列的节点（Brandes 栈）
2. **反向传播** —— 按 `stk` 逆序：
   - 对每个节点 w，通过入边找到前驱 v，条件为 `dist[v]+weight ≈ dist[w]`
   - `delta[v] += sigma[v] × (SCALE + delta[w]) / sigma[w]`
   - `bc[w] += delta[w]`（w ≠ s）
3. 降序排序；输出 `bc_scaled[v] / 1_000_000` 作为 `u32`

**复杂度**：O(V² × (V+E))——每个源节点一次 O(V²) Dijkstra。
**浮点精度**：前驱判定使用 1e-6 误差（`dist[v]+weight ≈ dist[w]`）。

---

## 修改文件

### `crates/gos-runtime/src/lib.rs`

- **`GraphRuntime::graph_between_inner<N>()`** —— 私有方法，位于 `graph_sim_inner` 之后。使用 `self.edges[ei].spec.weight` 作为路径权重实现 Brandes+Dijkstra。输出：`([VectorAddress; N], [u32; N], usize)`，与 `graph_centrality_inner` 形状一致。
- **`pub fn graph_between<N>()`** —— 公开 API 包装，位于 `pub fn graph_sim` 之后。获取 `RUNTIME.lock()` 并委托给 `graph_between_inner`。

### `crates/k-shell/src/lib.rs`

- **`pub fn dispatch_graph_between(sink)`** —— 展示函数，插入在 `dispatch_graph_closeness` 之前。配色方案：亮洋红（13）表示关键节点（相对于无权中心度的亮黄 14），青色（11）表示中继节点，灰色（8）表示端点。标题：`graph between  (weighted Dijkstra)`。页脚：`N node(s)  max-wbc: X  keystones: Y`。

### `crates/k-shell/src/proc.rs`

- 在 `graph sim` 分支之后插入路由（约第993行）：
  `graph between | between | gbetween | graph wbc | wbc | weighted betweenness`

### `host-tests/gos-graph-between-harness/`（新建 harness，10 测试，L4=30 VectorAddress 命名空间）

| 编号 | 用例 | 关键断言 |
|------|------|----------|
| 1 | 空图 | `total=0`，无 panic |
| 2 | 单孤立节点 | `WBC[A]=0`，`total=1` |
| 3 | 两节点 A→B | `WBC[A]=WBC[B]=0` |
| 4 | 路径 A→B→C（w=1.0） | `WBC[B]=1` |
| 5 | 瓶颈 {A,B}→X→{C,D} | `WBC[X]=4` |
| 6 | **权重敏感**：A→C(w=0.5)，C→B(w=0.5)，A→B(w=2.0) | `WBC[C]=1`（不同于 BFS 给出的0） |
| 7 | 瓶颈 {A,B,C}→X→{D,E,F} | `WBC[X]=9` |
| 8 | 线性5节点（w=1.0） | `WBC[C]=4`，`WBC[B]=WBC[D]=3` |
| 9 | 排序 | 对所有 i：`wbc[i-1] >= wbc[i]` |
| 10 | 自环+孤立节点 | 不崩溃，`WBC[A]=0` |

---

## Shell 命令一览

| 命令 | 别名 |
|------|------|
| `graph between` | `between` |
| `gbetween` | `graph wbc` |
| `wbc` | `weighted betweenness` |

---

## 不变量确认

- [x] 纯读操作：不推进 epoch，不做任何写操作
- [x] 所有栈数组以 `MAX_NODES=128` / `MAX_EDGES=512` 为界
- [x] Dijkstra 松弛中显式跳过自环（`v == u`）
- [x] 零权重边通过 `weight.max(0.0)` 处理
- [x] 浮点误差 1e-6 用于前驱检测（与 `graph_flow` 的 1e-9 一致的设计模式）
- [x] `sigma[v]=0` 保护反向传播中的除法（避免除零）
- [x] 输出缩放：`bc_scaled[v] / 1_000_000` 作为 u32（与 `graph_centrality` 一致）

---

## OS 类比

带测量延迟的 `traceroute` —— `graph between` 回答"哪个内核服务节点位于其他服务对之间最多的最小延迟路径上？"

与 `graph centrality`（跳数介数）不同，本算法能正确识别 BFS 会忽略的低延迟中继节点——因为它们所需跳数比直接但高延迟的替代路径更多。

---

## 宿主测试套件累计

**503 个测试，跨 50 个 harness**（全绿）：
- 此前（V2.52）：493 测试 / 49 个 harness
- **新增（V2.53）：+10 测试**，来自 `gos-graph-between-harness`

---

*由自动强化任务生成 · 2026-07-03*
