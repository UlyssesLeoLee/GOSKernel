# GOS 硬化日志 — V2.47（2026-07-02）

## 版本号: V2.47
## 功能: `graph color` — Welsh-Powell 贪心图着色

---

## 变更摘要

新增 `graph color` —— 在活跃 GOS 内核图的无向投影上执行 Welsh-Powell 贪心图着色。每个活跃节点被分配一个（从0开始的）颜色索引，使任意两个直接相连的节点颜色不同。节点按总度数降序处理（Welsh-Powell 启发式），然后贪心分配最小可用颜色。

Shell 别名：`graph color` / `color` / `gcolor` / `graph colour` / `colour`

OS 类比：颜色 = 无冲突调度域 / CPU 亲和分组——类似 Linux 的 `cgroups cpuset.cpus` 分配或 NUMA 节点绑定。每种颜色代表一组可以调度、锁定或隔离而不产生资源冲突的内核子系统。

---

## 动机

结构骨架视图（V2.46 生成森林）完成后，下一个自然的原语是**图着色**：将节点划分为无冲突分组。在内核图 OS 语境中：

- **图着色 = 调度域分配**——共享边（信号路由）的子系统不能位于同一域，以避免优先级反转或锁竞争。
- **色度数**（chromatic number）= 所有子系统无冲突运行所需的最少隔离域数量。
- 提供紧凑的答案："这个内核拓扑需要多少条独立调度通道？"
- 有助于资源规划：若色度数=2，内核可以在两个完全隔离的执行环境中运行。

---

## 算法：Welsh-Powell 贪心图着色

```text
第一步 —— 计算每个活跃节点的总（无向）度数：
  对每条活跃边 (u→v)：
    degree[u] += 1
    degree[v] += 1（自环除外）

第二步 —— 按度数降序排序节点（Welsh-Powell 排序）：
  order[] = 活跃 slot 按 degree[slot] 降序排列

第三步 —— 按排序顺序贪心分配：
  color_slot[] = NOT_COLORED
  对 order[] 中的每个 slot s：
    标记 s 的无向邻居已使用的所有颜色为禁用
    为 s 分配最小的非禁用颜色
    追踪目前已分配的最大颜色

第四步 —— 按排序顺序打包输出：
  out_vecs[i]   = snap.slot_vec[order[i]]
  out_colors[i] = color_slot[order[i]]
  chromatic     = max_color + 1（无节点时为 0）

输出：(vecs, colors, node_count, chromatic_number)
```

**关键设计选择：**

1. **无向处理**：将每条有向边视为无向（与 `graph community`、`graph bipartite`、`graph spanning` 一致）。调度冲突分析中信号方向无关紧要。
2. **Welsh-Powell 排序**：度数降序——最高度数（连接最多）的节点先着色，保证中心节点获得颜色 0（最低索引颜色 = 最高优先级域）。
3. **禁用颜色追踪**：每次迭代重置（`forbidden[ci] = false`），而非对全部 256 字节 `memset(0)`。只重置邻居实际使用的颜色，使内层循环在所有迭代中保持 O(E) 而非 O(V × 256)。
4. **色度数是贪心上界**：最优色度数在一般情况下是 NP-hard 的。Welsh-Powell 对路径图、二部图、完全图、星形图（内核拓扑的常见模式）是最优的，对真实拓扑提供实用的上界。
5. **角色标签**：`isolated`（度数0=无边）、`center`（颜色0 且度数>0=最高度数节点，最先分配）、`domain-N`（颜色N>0=冲突组N）。孤立节点从中心检查中单独区分，避免混淆——两者都获得颜色0，但角色不同。

**复杂度**：O(V·E) 每次调用——O(E) 度数扫描 + O(V²) 排序（n≤128）+ O(V·E) 贪心分配。
**空间**：O(MAX_NODES) = O(128)——固定大小栈数组，兼容 no_std/no_alloc。

---

## 实现细节

### `crates/gos-runtime/src/lib.rs`

- **`RuntimeState::graph_color_inner<const N>()`** —— 核心 Welsh-Powell 着色算法（度数扫描 → 降序排序 → 贪心分配 → 按序打包输出）
- **`pub fn graph_color<const N>() -> ([VectorAddress; N], [u8; N], usize, u8)`** —— 公开函数：锁定 RUNTIME，调用 `topology_snapshot()`，委托给 `graph_color_inner`

### `crates/k-shell/src/lib.rs`

- **`pub fn dispatch_graph_color(sink)`** —— 展示函数：
  - 标题：青色 `graph color`
  - 摘要行：`chromatic number: N   nodes: M`
  - 列标题：`color  vector  role`
  - 逐节点显示颜色索引（`C0`、`C1`……）、向量、角色（`center` 对应 C0，`domain-N` 对应 CN）
  - 终端颜色从调色板 `[11, 14, 10, 13, 12, 15, 6, 2]` 循环取 mod 8

- **角色标签：**
  - `isolated` —— 度数为0（无边；在颜色判断前检查以避免与 `center` 混淆）
  - `center` —— 颜色0 且度数>0（Welsh-Powell 首个分配的最高度数节点）
  - `domain-N` —— 颜色N>0（第N个冲突组）

### `crates/k-shell/src/proc.rs`

- 路由（4行）：`"graph color" | "color" | "gcolor" | "graph colour" | "colour"` → `dispatch_graph_color`
- 帮助文本新增2行

---

## 测试用例（10/10 通过）：`host-tests/gos-graph-color-harness`

| 编号 | 用例 | 验证点 |
|------|------|--------|
| 1 | 空图 | node_count=0, chromatic=0 |
| 2 | 单孤立节点 | chromatic=1, color=0 |
| 3 | 两个孤立节点（无边） | chromatic=1, 均为 color 0 |
| 4 | K₂：单边 A→B | chromatic=2, A≠B 颜色 |
| 5 | 路径 A→B→C | chromatic≤2, A≠B, B≠C |
| 6 | K₃ 三角形 | chromatic=3, 三者互不相同 |
| 7 | K₄ 完全图 | chromatic=4, 使用颜色 {0,1,2,3} |
| 8 | 二部图 K_{2,2} | chromatic≤2, 跨集合对颜色不同 |
| 9 | 有效性：无相邻对同色 | 对所有边(u,v)：color[u]≠color[v] |
| 10 | 降序验证：星形图 K_{1,3} | 中心 B 为 index 0 且 color 0；叶子共享一种颜色 |

**本轮修复**：`graph_color.rs` 第350行的 `K_{1,3}` 出现在格式化字符串字面量中——花括号被 Rust 解析为格式参数，已转义为 `K_{{1,3}}`。

**结果：10/10 通过，零告警**

---

## Shell 命令一览

```text
graph color        Welsh-Powell 贪心着色 —— 无冲突调度域
color              别名
gcolor             别名
graph colour       别名（英式拼写）
colour             别名
```

示例输出（三角形 K₃ + 孤立节点 D）：

```text
 graph color
 ─────────────────────────────────────────────────────────────
  chromatic number: 3   nodes: 4

  color  vector           role
  C0     [24:1:1:0]       center
  C1     [24:1:2:0]       domain-1
  C2     [24:1:3:0]       domain-2
  C0     [24:1:4:0]       isolated
 ─────────────────────────────────────────────────────────────
```

---

## 不变量确认

- [x] 纯读操作：`graph_color` 不推进 epoch，不做任何变更
- [x] 无堆分配 / no_std：所有缓冲区为固定大小栈数组
- [x] harness 使用标准的 `TEST_LOCK + reset()` 隔离方式
- [x] 版本顺序：V2.47 紧随 V2.46（生成森林）
- [x] 文档归档路径：`doc/06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.47.md`

---

## 后续建议（V2.48 候选）

- `graph mst` —— 使用 Prim 算法的最小生成树（带权边）
- `node checkpoint <vec>` —— 快照节点状态到 diff ring
- `graph sim <N>` —— 模拟 N 步随机游走，输出信号流量轨迹
- `journal ring <N>` —— 运行时可配置的 JournalRing 容量
- `graph flow` —— 加权内核图上的最大流 / Ford-Fulkerson

---

*由自动强化任务生成 · 2026-07-02*
