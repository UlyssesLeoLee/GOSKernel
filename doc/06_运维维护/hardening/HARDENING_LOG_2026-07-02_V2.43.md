# GOS 硬化日志 — V2.43（2026-07-02）

## 版本号: V2.43
## 功能: `graph pagerank` — 经典 PageRank 随机游走中心性

---

## 变更摘要

新增 `graph pagerank` shell 命令及底层 `gos_runtime::graph_pagerank` API，实现经典 PageRank 随机游走中心性算法，并新建 `gos-graph-pagerank-harness`（10 个宿主测试）。

---

## 动机

V2.42 的 Katz 中心性把每个节点的贡献视为等权：高出度节点与低出度节点对下游节点的贡献相同。PageRank 通过将每个节点的 rank 按出度均分来修正这一问题，更准确地刻画"随机游走者最终落在哪个节点"的概率分布。Katz 与 PageRank 因此构成互补的两个视角：

- **Katz**：所有长度的 walk 终点计数总和（原始影响力）
- **PageRank**：随机游走时间在各节点上的分布比例（归一化权威性）

OS 类比：按入站信号权重排序的 `top` —— 哪些内核节点在实时图拓扑的随机游走中占主导地位？

---

## 算法

带吸收悬挂节点（dangling node）的经典 PageRank：

```text
PR[v] = (1-d) × SCALE + d × Σ_{u→v, outdeg(u)>0}  PR[u] / outdeg(u)
```

参数：
- `d = 0.85`（标准阻尼系数）
- `SCALE = 1_000_000`（1.0 的定点整数表示）
- `TELE = 150_000`（`= (1-d) × SCALE`，传送地板值）
- `PR_ITERS = 20`（固定迭代次数，收敛性已由 harness 验证）
- 悬挂节点（`outdeg = 0`）吸收自身 rank（自环语义——只接收信号不转发，符合终端消费者的 GOS 模型）

**复杂度**：O(K × V × E)，K=20，V ≤ 128 节点，E ≤ 128 边。
**定点算术**：u64 中间结果，截断为 u32 输出。

### 稳态值（解析推导，测试验证）

| 图结构 | PR 值 |
|--------|-------|
| 孤立节点 | 150,000（传送地板值） |
| 源节点（无入边） | 150,000 |
| 单边接收者 A→B 中的 B | 277,500 |
| 链尾 A→B→C 中的 C | 385,875 |
| 扇入枢纽 {A,B,C}→D 中的 D | 532,500 |
| 环 / 互相环（任意大小） | 1,000,000（权威） |
| 分叉目标 A→{B,C}（出度=2） | 213,750 |

---

## 实现细节

### 修改文件

| 文件 | 变更 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | 新增 `GosRuntime::graph_pagerank_inner<N>()`（双缓冲迭代，~80行）+ `graph_pagerank<N>()` 公开函数 |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_pagerank()`：列布局 `vector(16) \| pagerank(6k) \| role`；角色 **authority**（≥1,000k，亮黄）/ **relay**（>300k，青色）/ **sink**（≤300k，暗灰） |
| `crates/k-shell/src/proc.rs` | 新增路由分支 + 帮助文本 |
| `host-tests/gos-graph-pagerank-harness/` | 全新 harness（10 测试，均通过） |

### Shell 命令别名

```text
graph pagerank    完整命令
pagerank          简写
pr                最短别名
graph rank        语义别名
rank              语义简写
```

### 示例输出

```text
 graph pagerank
 ─────────────────────────────────────────────────────────
  vector           pagerank  role
  [20:0:1:0]            999k  authority
  [20:0:2:0]            532k  relay
  [20:0:3:0]            213k  sink
  [20:0:4:0]            150k  sink
 ─────────────────────────────────────────────────────────
  4 node(s)  d=0.85  max-pr: 999k (×1e-3)  authorities: 1
```

---

## 角色语义

| 角色 | 阈值 | 含义 |
|------|------|------|
| **authority** | PR ≥ 1,000,000 | 主导随机游走流量；出现于环节点或接收多个高分节点的场景 |
| **relay** | 300,000 < PR < 1,000,000 | 高于地板值的链接贡献；结构性枢纽但非环节点 |
| **sink** | PR ≤ 300,000 | 接近传送地板值；入站链接极少或没有 |

---

## Katz 与 PageRank 对比

| 指标 | Katz（V2.42） | PageRank（V2.43） |
|------|--------------|-------------------|
| 计数方式 | 所有长度的全部游走 | 随机游走平衡态 |
| 归一化 | 无（有出度偏差） | 除以出度（选民模型） |
| 高出度节点 | 贡献更多 | 每条边贡献更少 |
| 最佳提问 | "哪个节点接收最多流量？" | "哪个节点在结构上最具权威性？" |
| OS 类比 | `netstat -s` | `top` |

---

## 测试用例（10/10 通过）

| 编号 | 用例 | 验证点 |
|------|------|--------|
| 1 | 空图 | node_count=0 |
| 2 | 孤立节点 | PR=150,000 |
| 3 | 单边 A→B | B 的 PR=277,500 |
| 4 | 链 A→B→C | 排序正确 |
| 5 | 扇入星形 | D 排序第一 |
| 6 | 3-环权威 | 全部 PR=1,000,000 |
| 7 | 互向环权威 | 全部 PR=1,000,000 |
| 8 | 出度分裂 | 按出度均分验证 |
| 9 | 排序验证 | 输出降序 |
| 10 | 总数一致性 | total 与节点数吻合 |

---

## 测试摘要

**宿主测试套件总计：433 个**（此前 423，+10）

```text
gos-graph-pagerank-harness: 10/10 ✓
```

---

## 不变量确认

- [x] `graph_pagerank_inner` 为纯读操作，不推进 epoch，不写运行时状态
- [x] 孤立节点返回 PR = 150,000（传送地板值，非零）——与 Katz（孤立节点为零）不同
- [x] 输出恒定降序：`pr[0] ≥ pr[1] ≥ ... ≥ pr[total-1]`
- [x] 无堆分配，no_std 安全，O(K×V×E)，K=20

---

## 后续建议

- `node checkpoint <vec>` — 快照节点状态到 diff ring（观测性）
- `journal ring <N>` — 运行时可配置的 JournalRing 容量
- `graph hits` — HITS 算法（hub/authority 二部图分解），与 PageRank 互补
- PAL_U32 → attribute node 重构（Demo A 前置条件）

---

*由自动强化任务生成 · 2026-07-02*
