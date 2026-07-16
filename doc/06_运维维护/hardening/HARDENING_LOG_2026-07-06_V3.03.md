# GOSKernel 硬化日志 — V3.03
**日期：** 2026-07-06
**算法：** 哈密顿路径/回路检测——迭代式回溯 DFS
**分支：** feat/vk-auto-live-surface
**提交：** feat(v3.03): Hamiltonian path/circuit -- iterative backtracking DFS + gos-graph-hamiltonian-harness (10 tests)

---

## 变更摘要

V3.03 为 GOSKernel 图论运行时新增了**哈密顿路径与回路检测**功能。

**哈密顿路径**恰好经过图中每个节点一次。
**哈密顿回路**是返回起点的哈密顿路径。

这是**欧拉**（V2.87，每条**边**恰好经过一次）的自然顶点遍历对偶：

| 概念 | 遍历对象 | 条件 | 复杂度 |
|---------|--------|-----------|------------|
| 欧拉路径 | 每条**边**一次 | 出/入度为奇数的节点 ≤2 个 | O(V+E) |
| 哈密顿路径 | 每个**顶点**一次 | 一般情形下 NP-完全 | 回溯法 |

---

## 公开 API

### `gos_runtime::graph_hamiltonian<const N: usize>() -> ([VectorAddress; N], usize, bool, bool, usize)`

返回 `(path_vecs, path_len, has_circuit, has_path, node_count)`：
- `path_vecs[0..path_len]`——找到的哈密顿路径/回路，按遍历顺序排列的节点
- `path_len`——找到哈密顿路径时等于 `node_count`；未找到时为 0
- `has_circuit`——当且仅当找到有向哈密顿回路时为 true（path_len > 0 且存在末尾→起点的边）
- `has_path`——当且仅当找到有向哈密顿路径时为 true（`has_circuit` ⇒ `has_path`）
- `node_count`——活跃节点总数

**有向图：** 边 A→B 并不意味着 B→A。
**自环：** 不计入邻接关系（不算作哈密顿遍历的一部分）。
**单节点：** 平凡情形下 `has_circuit = has_path = true`（退化情形）。
**步数上限：** 5,000,000——防止在对抗性图上挂起；操作系统子系统图的运行远低于此限制。

---

## 算法：带死端剪枝的迭代式回溯 DFS

**方法：** 迭代式 DFS（无递归，无堆分配——仅使用内核栈）。

**核心状态：**
- `path[0..depth]`——当前部分路径（紧凑节点索引，`u8`）
- `visited: u128`——路径中当前节点的位掩码
- `cand[d]: u128`——`path[d]` 尚未在位置 `d+1` 尝试过的剩余后继节点

**外层循环：** 依次尝试每个节点作为起点（一旦找到回路即提前跳出）。

**内层循环：**
1. 若 `depth == nc`：所有节点已放置 → 找到哈密顿路径；检查是否成环（末尾→起点的边）；然后回溯。
2. 若 `cand[depth-1] == 0`：没有更多候选 → 回溯（从 visited 中移除 `path[depth-1]`）。
3. 否则：从 `cand[depth-1]` 中选取下一个候选 `v`，应用**死端剪枝**，然后压入 `v`。

**死端剪枝：**
在暂时压入节点 `v` 后，统计满足 `adj[w] & unvisited_after == 0` 的未访问节点 `w`
（即 `w` 在剩余未访问集合中没有后继——它只能作为路径的终点）。
若存在**两个及以上**这样的节点，则最多只有一个能是终点 → 剪掉此分支。
这是可靠的：剪枝仅在 `remaining > 1` 时应用，而存在两个"死端"节点本身就是矛盾。

**栈使用（无堆分配）：**
- `adj: [u128; MAX_NODES]` = 2,048 字节（有向邻接位掩码）
- `path: [u8; MAX_NODES]` = 128 字节
- `cand: [u128; MAX_NODES]` = 2,048 字节
- `best_path: [u8; MAX_NODES]` = 128 字节
- 总计：约 4.5 KB（完全在内核栈预算之内）

---

## Shell 命令

- `graph hamiltonian` — 检测哈密顿路径/回路，显示遍历顺序
- `gham` — 别名
- `hamiltonian` — 别名
- `graph ham` — 别名
- `ghamiltonian` — 别名
- `ham circuit` — 别名
- `hamiltonian path` — 别名

**显示：**
- 找到回路时：亮绿色（颜色 10）表头与路径节点
- 仅找到路径（无回路）时：亮黄色（颜色 14）路径节点
- 未找到哈密顿路径时：亮红色（颜色 12）
- 页脚显示节点数、回路/路径/无 三种状态，以及 `↺ back to start (circuit)` 标注

---

## VectorAddress L4 命名空间

`gos-graph-hamiltonian-harness` 对应 L4=79

---

## 操作系统类比

`graph hamiltonian` = **单遍维护扫描**——恰好经过每个内核子系统一次的最小开销固件更新或审计流程。

- **哈密顿回路：** 维护守护进程可以从同一基准模块出发并返回（类似遍历所有服务后将控制权交还给 `init` 的 `systemd` oneshot）
- **仅有哈密顿路径：** 扫描能访问所有模块但无法返回起点（类似一次性的破坏性单向升级链）
- **无哈密顿路径：** 不存在顺序单遍方案——需要并行或重复访问（类似存在互斥写依赖、阻止线性审计顺序的服务）

对比：
- `graph eulerian`（V2.87）— 每条 IPC **通道**遍历一次（边覆盖扫描）
- `graph dag layers`（V2.89）— **并行**批量执行层级（非顺序单遍）
- `graph toposort` — 遵循依赖关系的线性排序（不保证存在哈密顿结构）

---

## 文献

- Hamiltonian 1859 — "Icosian game"（在十二面体的所有顶点上寻找一个环）
- Ore 1960 — 充分条件：对所有不相邻的 u,v，deg(u)+deg(v)≥n ⇒ 存在哈密顿回路
- Dirac 1952 — 对所有 v，deg(v) ≥ n/2 ⇒ 存在哈密顿回路
- Karp 1972 — 哈密顿路径/回路是 NP-完全的（由 3-SAT 归约得出）
- Held & Karp 1962 — O(2ⁿ·n²) 动态规划算法（对 n≤20 用位掩码精确求解）
- 对比：欧拉（V2.87）— O(V+E) 多项式时间；哈密顿——NP-完全

---

## 测试套件

`gos-graph-hamiltonian-harness` 中的 10 个宿主测试：

| # | 图 | has_path | has_circuit | 说明 |
|---|-------|----------|-------------|-------|
| 1 | 空图 | false | false | 无节点 |
| 2 | 单节点 | true | true | 平凡哈密顿情形 |
| 3 | 两个节点，无边 | false | false | 不连通 |
| 4 | A→B（单向） | true | false | 有路径但无返回边 |
| 5 | A↔B（双向） | true | true | A→B→A 回路 |
| 6 | A→B→C（链） | true | false | 仅路径，无返回边 |
| 7 | A→B→C→A（三角形） | true | true | 有向 3-环 |
| 8 | 有向完全图 K4 | true | true | 全部 12 条有向边 |
| 9 | 菱形 A→B, A→C, B→D, C→D | false | false | 分叉-汇合结构阻止哈密顿路径 |
| 10 | 两对孤立节点 A↔B, C↔D | false | false | 不连通 → 无全局路径 |

全部 10 个测试通过（测试目录中运行 `cargo test`：0.01 秒）。
