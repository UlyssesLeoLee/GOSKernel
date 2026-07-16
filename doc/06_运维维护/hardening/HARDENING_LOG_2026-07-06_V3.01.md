# GOSKernel 硬化日志 — V3.01
**日期：** 2026-07-06
**算法：** 反馈顶点集（FVS）——基于 Kahn 算法的贪心方法
**分支：** feat/vk-auto-live-surface
**提交：** feat(v3.01): feedback vertex set -- greedy Kahn FVS + gos-graph-fvs-harness (10 tests)

---

## 变更摘要

V3.01 为 GOSKernel 图论运行时新增了**反馈顶点集（FVS）**——一个经典的 NP-难图问题。FVS 是使有向图移除后变为无环图（DAG）的最小顶点集合。

至此完成了**破环工具箱**：
- V2.91 — 反馈弧集（FAS）：打破所有环所需移除的最少*边*数
- V3.01 — 反馈顶点集（FVS）：打破所有环所需移除的最少*顶点*数

---

## 公开 API

### `gos_runtime::graph_fvs<const N: usize>() -> ([VectorAddress; N], usize, usize)`

返回 `(fvs_vecs, fvs_size, node_count)`：
- `fvs_vecs[0..fvs_size]`——按 `VectorAddress.as_u64()` 升序排列的 FVS 节点
- `fvs_size`——贪心 FVS 中的节点数（最小 FVS 的上界）
- `node_count`——图中活跃节点总数

**无环性保证：** 移除返回的 FVS 节点后，图始终变为 DAG。

---

## 算法：迭代式 Kahn BFS

**复杂度：** 每次 FVS 调用 O(V × (V + E))——V 次迭代，每次 O(V + E)。
对于 MAX_NODES=128、MAX_EDGES=512 的 GOSKernel：≤ 65K · 640 = 4100 万次操作，完全在预算之内。

**每一轮：**
1. 计算活跃节点的 `in_deg[ci]` 和 `out_deg[ci]`（计入自环；但自环不进入 `adj` 位掩码）
2. 构建 `adj[ci]` = 出边（不含自环）的 u128 位掩码
3. Kahn BFS：以入度为 0 的节点作为种子，通过递减后继节点的入度进行排空
4. 若 `processed == live_count` → 无环，完成
5. 否则：在未排空（成环）的节点中，选取 `in_deg × out_deg` 得分最高者，加入 FVS，标记为死亡

**得分启发式：** `in_deg[ci] × out_deg[ci]`——处于众多入路径和出路径交汇处的节点最可能出现在多个环中；移除它们能高效地在每一步打破多个环。

**自环处理：** 自环 A→A 会使 `in_deg[A] += 1`，但**不会**把 A→A 加入 `adj[A]`。因此 Kahn 算法永远无法出队 A（in_deg 始终 ≥ 1）→ A 总是被归类为成环节点 → 正确地进入 FVS。

**使用的栈数组：**
- `live[MAX_NODES]`——记录存活节点的布尔数组
- `fvs_cis[MAX_NODES]`——收集的 FVS 紧凑索引
- `in_deg[MAX_NODES]`、`out_deg[MAX_NODES]`、`adj[MAX_NODES]`——每轮重新计算
- `queue[MAX_NODES]`、`in_queue[MAX_NODES]`——Kahn BFS 使用的数组
- `tmp[MAX_NODES]`——用于按 VectorAddress.as_u64() 最终排序

**无堆分配**——所有状态均位于内核栈上（约 5 KB）。

---

## Shell 接口

| 命令 | 别名 |
|---------|---------|
| `graph fvs` | `gfvs`、`feedback vertex set`、`graph fvset`、`gfvset`、`graph feedback vertex` |

**显示：** 亮红色表头（颜色 12）；每个节点标注 `fvs-member` 角色标签；页脚显示 `FVS=N dag-status: cyclic/acyclic`。

---

## 操作系统类比

**打破所有启动顺序依赖环所需暂停/隔离的最少内核子系统集合。**
类似于在 `systemd-analyze verify` 识别出循环依赖后，对造成环的服务执行 `systemctl mask`。

与 `feedback arc`（V2.91）互补：FAS 移除边（IPC 通道），FVS 移除顶点（子系统）。
两者都能使依赖图变为 DAG，但攻击的是不同的结构元素。

---

## 关键不变量

- 当且仅当图已经是 DAG（无环）时，`fvs_size == 0`
- 自环 → `in_deg ≥ 1` → 永不被排空 → 始终进入 FVS
- 移除所有 `fvs_vecs[0..fvs_size]` 后必然得到 DAG（无环性保证）
- `fvs_size ≤ node_count`（显然成立）
- 对于有向完全图 K_n：`fvs_size == n-1`（最优——移除任意 n-2 个节点都会留下互成环的 2 个节点）
- 输出按 `VectorAddress.as_u64()` 升序排列
- 纯读取操作——不会推进图的 epoch

---

## 测试套件：gos-graph-fvs-harness（10 个测试）

| 编号 | 图 | 预期结果 |
|------|-------|----------|
| 1 | 空图 | `fvs_size=0` |
| 2 | 单节点，无边 | `fvs_size=0` |
| 3 | DAG 链 A→B→C→D | `fvs_size=0` |
| 4 | 自环 A→A | `fvs_size=1`，FVS={A} |
| 5 | 互相成环 A↔B | `fvs_size=1` |
| 6 | 三角形 A→B→C→A | `fvs_size=1` |
| 7 | 两个不相交的环 A↔B、C↔D | `fvs_size=2` |
| 8 | 菱形 A→{B,C}→D + 返回边 D→A | `fvs_size=1`，FVS∈{A,D} |
| 9 | 有向完全图 K4（12 条边） | `fvs_size=3（=n-1）` |
| 10 | 交叉验证：有环 vs DAG vs 混合自环 | 全部断言通过 |

**结果：** 10/10 测试通过。

---

## VectorAddress L4 命名空间（更新）

```
72=graph-indep, 73=graph-vc, 74=graph-domset, 75=graph-mpc,
76=graph-arborescence, 77=graph-fvs
```

---

## 宿主测试套件总计

| 里程碑 | 测试数 | 备注 |
|-----------|-------|-------|
| V3.00 | 973 | MSA（Chu-Liu/Edmonds） |
| V3.01 | **983** | +10 个 FVS 测试 |

---

## 文献

- **Karp 1972** —— 最小 FVS 与 FAS 的 NP-完全性（原始 21 个 NP-完全问题之一）
- **Erdős & Pósa 1965** —— Erdős–Pósa 定理：无向图中 FVS ≤ O(log n · OPT)
- **Bafna, Berman & Fujito 1999** —— 基于 LP 松弛的 FVS 2-近似算法
- **Garey & Johnson 1979** —— NP-难分类（问题 [GT7]）

---

## 与既有算法的关系

| 算法 | 版本 | 移除对象 | 目标 |
|-----------|---------|---------|------|
| 反馈弧集 | V2.91 | 边 | 打破所有有向环 |
| **反馈顶点集** | **V3.01** | **顶点** | **打破所有有向环** |
| 支配树 | V2.90 | — | 单入口支配结构 |
| DAG 分层 | V2.89 | — | 假定为 DAG；寻找并行度 |
| 最小路径覆盖 | V2.99 | — | 假定为 DAG；最小链覆盖 |

FVS 与 FAS 一同完成了破环工具对，让运维人员可以在移除边（IPC 通道）与移除顶点（子系统）之间选择，以实现无环性。
