# GOSKernel 硬化日志 — V3.02
**日期：** 2026-07-06
**算法：** 全局最小割——Stoer-Wagner 1997
**分支：** feat/vk-auto-live-surface
**提交：** feat(v3.02): global min cut -- Stoer-Wagner 1997 + gos-graph-min-cut-harness (10 tests)

---

## 变更摘要

V3.02 为 GOSKernel 图论运行时新增了**全局最小边割**（Stoer-Wagner 1997）算法。最小割 κ'(G) 是使图断开所需移除的最少无向边数——即边连通度。

至此完成了**故障隔离工具箱**：
- V2.86 — 图桥：单条割边（1-边连通性缺口）
- V2.93 — 2-边连通分量：能抵御任意单链路故障的簇
- V3.02 — 全局最小割：精确的最小划分代价 κ'(G)

---

## 公开 API

### `gos_runtime::graph_min_cut<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, u32, usize)`

返回 `(vecs, sides, node_count, min_cut, side_b_size)`：
- `vecs[0..node_count]`——所有活跃节点；A 侧（sides==0）在前，B 侧（sides==1）在后
- `sides[0..node_count]`——分区归属：0=A 侧，1=B 侧
- `node_count`——活跃节点总数
- `min_cut`——最小无向边割 = 边连通度 κ'(G)
- `side_b_size`——B 侧节点数量

**无向投影：** A→B 与 B→A 算作同一条边（通过 seen_adj u128 位掩码去重）。
**不连通图：** `min_cut = 0`（本就以零代价分区）。

---

## 算法：Stoer-Wagner 1997

**复杂度：** 采用边列表表示时为 O(V² × E)——共 V-1 个阶段，每阶段 O(V × E)。
对于 MAX_NODES=128、MAX_EDGES=512 的 GOSKernel：≤ 128² × 512 = 800 万次操作。

**每个阶段：**
1. **最大邻接排序：** 贪心地将 key[best] 最高的非 A 活跃节点 `best` 加入 A（key 为该节点到已加入 A 的节点的边权之和），直到所有活跃节点都加入 A。平局时选紧凑索引最小者。
2. **本阶段割值：** `key[last_t]` = 最后加入的节点到其余节点的总边权。若此值为新的最小值，记录为 `min_cut` 并保存 `group_members[last_t]` 作为 `best_b_mask`。
3. **合并：** 将 `last_t` 的所有边重定向到 `last_s`；消除自环；对并行边求和去重。

**分区追踪：** `group_members[si]` 是超级节点 `si` 中原始紧凑索引组成的 u128 位掩码。发现新的最小值时，`best_b_mask = group_members[last_t]` 捕获该阶段的 B 侧分区。

**栈使用（无堆分配）：**
- `uf, ut: [u8; MAX_EDGES]` = 各 512 字节（紧凑索引端点，因 ci < 128 故用 u8）
- `uw: [u16; MAX_EDGES]` = 1024 字节（权重，因累加最大值 = N-1 < 65535 故用 u16）
- `ue_live: [bool; MAX_EDGES]` = 512 字节
- `seen_adj: [0u128; MAX_NODES]` = 2048 字节（无向边对去重位掩码）
- `group_mbrs: [0u128; MAX_NODES]` = 2048 字节（超级节点成员追踪）
- `key, in_a`：每阶段数组 ≈ 384 字节
- 总计：约 8 KB（完全在内核栈预算之内）

**关键不变量：**
- 本阶段割值 = `key[last_t]` = 精确的 Stoer-Wagner 阶段值
- `best_b_mask` 始终对应取得 `min_cut` 的那个阶段
- 对于 K_n：min_cut = n-1（每个节点的度为 n-1，孤立任意一个节点代价为 n-1 条边）
- 对于路径/树：min_cut = 1（桥即为最小割）
- 对于不连通图：min_cut = 0（第一阶段即给出 key[last_t]=0）

---

## Shell 命令

- `graph min cut` — 显示带 A/B 分区的全局最小边割
- `gmincut` — 别名
- `min cut` — 别名
- `edge connectivity` — 别名
- `gedge connectivity` — 别名
- `graph cut` / `gcut` — 别名

**显示：** 亮青色表头；A 侧节点亮绿色，B 侧节点亮洋红色；页脚显示 `κ'(G)=<值>  Stoer-Wagner 1997`。

---

## VectorAddress L4 命名空间

`gos-graph-min-cut-harness` 对应 L4=78

---

## 操作系统类比

`graph min cut` = **最小故障隔离边界**——将内核划分为两个完全独立的故障域所需切断的最少 IPC 通道数。类似于对最少的一组网络接口执行 `ip link set <iface> down`，将集群划分为两个隔离的分段。

补充：
- `graph bridges`（V2.86）— 单条 1-边割（λ=1 瓶颈）
- `graph 2ecc`（V2.93）— 能抵御任意单链路故障的分量
- `graph flow`（V2.50）— 特定源汇对之间的最大流/最小割

---

## 文献

- Stoer & Wagner 1997 — "A simple min-cut algorithm", J. ACM 44(4):585–591
- Nagamochi & Ibaraki 1992 — Stoer-Wagner 使用的 MA 排序（最大邻接）
- Ford & Fulkerson 1956 — 最大流最小割定理（全局割 ≤ 最大 s-t 流）
- Whitney 1932 — 边连通度的定义；κ'(G) = 所有顶点对上最大 s-t 流的最小值

---

## 测试套件

`gos-graph-min-cut-harness` 中的 10 个宿主测试：

| # | 图 | 预期 min_cut |
|---|-------|-----------------|
| 1 | 空图 | 0 |
| 2 | 单节点 | 0 |
| 3 | 两个节点，无边 | 0（不连通） |
| 4 | K2（一条边） | 1 |
| 5 | 路径 A-B-C | 1（桥） |
| 6 | 三角形 K3 | 2 |
| 7 | 完全图 K4 | 3 |
| 8 | 两个三角形 + 桥 | 1（桥） |
| 9 | 星形 K_{1,4} | 1（叶子度） |
| 10 | 正方形 C4 + 分区不变量 | 2 |

全部 10 个测试通过（在测试目录中运行 `cargo test`）。
