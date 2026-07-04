# GOS 硬化日志 — V2.45（2026-07-02）

## 版本号: V2.45
## 功能: `graph community` — 标签传播社区发现（Label Propagation）

---

## 变更摘要

新增 `graph community` —— 对 GOS 内核图执行标签传播算法（LPA）社区发现，为图分析工具集补上第一个**聚类**原语。

Shell 别名：`graph community` / `community` / `lpa` / `graph lpa` / `graph cluster` / `cluster`

OS 类比：`iproute2 bridge vlan show` + `systemd-analyze critical-chain` —— 哪些内核服务节点天然聚成紧耦合的子系统？

---

## 动机

中心度/排名系列算法（V2.38–V2.44：度数 → 介数 → 紧密度 → 离心率 → Katz → PageRank → HITS）完成后，下一个自然的方向是**社区结构**：不再问"哪个节点最重要"，而是问"哪些节点属于同一功能子系统"。

在图论 OS 中，社区发现回答以下运维问题：
- 哪些服务相互依赖，应当协同部署/协同故障处理？
- 哪些分组构成天然的隔离域？
- 一次拟议中的架构变更会拆分还是合并现有社区？

---

## 算法：异步标签传播（LPA）

```text
初始化：label[v] = slot_index(v)   // 每个节点各自成一个社区

迭代 0..20：
  按 slot 顺序遍历每个节点 v：
    freq[l] = |{v 的邻居 u（入+出）中 label[u] == l 的数量}|
    label[v] = argmax_l freq[l]         // 打平：取最小 l
    # 立即更新——本轮后续节点会看到 v 的新标签

重新编号：社区按规模降序编号为 0, 1, 2...
输出：按（社区id 升序, slot 升序）排序，便于分组展示
```

**关键设计选择：**

1. **无向处理**：入边和出边均视为无向邻居连接。使算法对服务图中的"共处"关系敏感，而不受信号方向影响，符合内核子系统的直觉。
2. **异步更新**：每个节点的标签立即更新（不等到本轮结束再统一写入）。这一点至关重要——同步版本会在二部图和链式拓扑上振荡（两节点每轮互换标签，永不收敛）。异步版本对所有连通分量在 O(迭代次数) 内收敛。
3. **打平规则：取最小标签**——两个标签频次相同时取较小者，保证确定性、可重现的输出。
4. **20 轮迭代**——与 V2 系列其他迭代算法（PageRank、Katz、HITS）保持一致。
5. **社区重新编号**：LPA 收敛后，社区按成员数降序重新编号为 0, 1, 2...，最大的社区恒为 id 0，显示为"major-community"。这使输出直观：C0 始终是最大簇。

**复杂度**：O(V × E × 20) 每次调用——与 PageRank/HITS 同阶。
**空间**：O(MAX_NODES) = O(128)——全部为固定数组，兼容 no_std/no_alloc。

---

## 实现细节

### `crates/gos-runtime/src/lib.rs`

- **`RuntimeState::graph_community_inner<const N>()`** —— 核心算法（异步 LPA、重编号、排序）
- **`pub fn graph_community<const N>()`** —— 公开包装函数（锁定 RUNTIME，调用内部实现）

### `crates/k-shell/src/lib.rs`

- **`pub fn dispatch_graph_community(sink)`** —— 展示函数：
  - 标题：青色 `graph community`
  - 每社区一块：`[C0]  N nodes  major-community / minor-community / isolated`
  - 成员节点向量每行 4 个（洋红=major，青色=minor，灰色=isolated）
  - 页脚：`N nodes  LPA/20iter  communities: M`

### `crates/k-shell/src/proc.rs`

- 路由：`"graph community" | "community" | "lpa" | "graph lpa" | "graph cluster" | "cluster"` → `dispatch_graph_community`
- 帮助文本新增两行，说明命令及别名

---

## 测试用例（10/10 通过）：`host-tests/gos-graph-community-harness`

| 编号 | 用例 | 验证点 |
|------|------|--------|
| 1 | 空图 | total=0, comm_count=0 |
| 2 | 单孤立节点 | 1 节点，1 社区，id=0 |
| 3 | 两个无边孤立节点 | 2 节点，2 社区（无边无法合并） |
| 4 | 单边 A→B | 2 节点，1 社区（无向邻居合并） |
| 5 | 有向三角环 A→B→C→A | 3 节点，1 社区 |
| 6 | 两对不连通节点（A─B, C─D） | 4 节点，2 社区；A、B 同社区；C、D 同社区，两对不同 |
| 7 | 完全二部图 K_{2,2}（A,B→C,D） | 4 节点，1 社区（全部无向可达） |
| 8 | 两个三角形，无桥接 | 6 节点，2 社区（每个三角形一个） |
| 9 | 排序输出连续性 | 输出中社区 id 非递减 |
| 10 | 全连接链 A─B─C─D | 4 节点，1 社区，全部 id=0 |

**结果：10/10 通过**

---

## 社区角色语义

| 角色 | 条件 | 颜色 |
|------|------|------|
| `major-community` | 最大社区（id=0）且节点数 >1 | 洋红 (13) |
| `minor-community` | 多节点社区，非最大 | 青色 (11) |
| `isolated` | 单节点社区（无无向邻居） | 暗灰 (8) |

---

## Shell 命令一览

```text
graph community         标签传播社区发现
community               别名
lpa                     别名（Label Propagation Algorithm）
graph lpa               别名
graph cluster           别名
cluster                 别名
```

示例输出（两个子系统 + 一个孤立服务）：

```text
 graph community
 ───────────────────────────────────────────────────────────
  [C0]  3 nodes  major-community
      1.0.1.0  1.0.2.0  1.0.3.0
  [C1]  2 nodes  minor-community
      2.0.1.0  2.0.2.0
  [C2]  1 node   isolated
      3.0.1.0
 ───────────────────────────────────────────────────────────
  6 nodes  LPA/20iter  communities: 3
```

---

## 不变量确认

- [x] 纯读操作：`graph_community` 不推进 epoch，不做任何变更
- [x] 无堆分配 / no_std：所有缓冲区为固定大小栈数组
- [x] harness 使用标准的 `TEST_LOCK + reset()` 隔离方式
- [x] 版本顺序：V2.45 紧随 V2.44（HITS）
- [x] 文档归档路径：`doc/06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.45.md`

---

## 后续建议（V2.46 候选）

- `graph spanning` —— BFS/DFS 生成树（最小连接骨架）
- `node checkpoint <vec>` —— 快照节点状态到 diff ring
- `journal ring <N>` —— 运行时可配置的 JournalRing 容量
- `graph sim <N>` —— 模拟 N 步随机游走，输出信号流量轨迹

---

*由自动强化任务生成 · 2026-07-02*
