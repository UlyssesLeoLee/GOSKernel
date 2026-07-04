# GOS 硬化日志 — V2.44（2026-07-02）

## 版本号: V2.44
## 功能: `graph hits` — Kleinberg HITS hub/authority 二部图分解

---

## 变更摘要

新增 `graph hits` shell 命令及底层 `gos_runtime::graph_hits` API，实现 Kleinberg HITS（Hyperlink-Induced Topic Search）算法的 hub/authority 二部图分解，并新建 `gos-graph-hits-harness`（10 个宿主测试）。

---

## 动机

至此图论中心性系列算法覆盖三种经典视角：

| 版本 | 算法 | 含义 |
|------|------|------|
| V2.42 Katz | 所有路径长度的 walk 数量 | 原始影响力 |
| V2.43 PageRank | 随机游走稳定分布 | 归一化权威性 |
| V2.44 HITS | 二部图分解 | 哪些节点是最好的"指针"，哪些节点是最被引用的"目标" |

HITS 与 PageRank 的核心区别：PageRank 只给每个节点一个分数；HITS 给每个节点**两个**分数（hub + authority），将图分解为"转发者"和"目标"两类，在有向二部结构（发送节点 → 服务节点）中尤为有价值。

OS 类比：`vmstat` / `top` 的二部视图——哪些内核节点是最好的信号转发者（**hub**），哪些是被引用最多的信号目的地（**authority**）？

---

## 算法

带 L∞ 归一化的 Kleinberg HITS：

```text
初始化：h[v] = a[v] = SCALE，对所有活跃节点

每轮迭代（共 20 轮）：
  new_a[v] = Σ_{u→v} h[u]          （authority = 入邻居 hub 分数之和）
  new_h[v] = Σ_{v→w} a[w]          （hub = 出邻居 authority 分数之和）
  [同步更新——使用旧的 h、a 值]

  max_a = 所有 v 中 new_a[v] 的最大值
  max_h = 所有 v 中 new_h[v] 的最大值

  a[v] = new_a[v] × SCALE / max_a   （max_a > 0 时，否则为 0）
  h[v] = new_h[v] × SCALE / max_h   （max_h > 0 时，否则为 0）

输出按 authority 分数降序排序。
```

参数：
- `SCALE = 1_000_000`
- `ITERS = 20`
- 悬挂节点：无出边 → hub=0；无入边 → authority=0

**复杂度**：O(K × V × E)，K=20，V ≤ 128，E ≤ 512。

### 收敛值（harness 验证）

| 图结构 | hub | authority |
|--------|-----|-----------|
| 孤立节点 | 0 | 0 |
| 纯源节点 A（A→B） | 1,000,000 | 0 |
| 纯汇节点 B（A→B） | 0 | 1,000,000 |
| 链中间节点 B（A→B→C） | 1,000,000 | 1,000,000 |
| 扇出中心 A→{B,C,D} | 1,000,000 | 0 |
| 扇入中心 {A,B,C}→D | 0 | 1,000,000 |
| 互向环 A↔B | 1,000,000 | 1,000,000 |
| 3-环（任意节点） | 1,000,000 | 1,000,000 |
| 二部图 hub（无入边） | 1,000,000 | 0 |
| 二部图 authority（无出边） | 0 | 1,000,000 |

---

## 实现细节

### 修改文件

| 文件 | 变更 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | 新增 `GosRuntime::graph_hits_inner<N>()`（~100行，双缓冲同步更新 + L∞ 归一化）+ `graph_hits<N>()` 公开函数 |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_hits()`（~110行）：列布局 `vector \| hub \| authority \| role`；角色颜色见下表 |
| `crates/k-shell/src/proc.rs` | 新增路由分支 `"graph hits"`、`"hits"`、`"graph ha"`、`"ha"`、`"hub authority"` |
| `host-tests/gos-graph-hits-harness/` | 全新 harness（10 测试，均通过） |

角色颜色编码：
- **hub+authority** ≥ 800k：洋红（如环节点）
- **authority** ≥ 800k：亮黄
- **hub** ≥ 800k：青色
- **isolated**（两者均 < 200k）：暗灰
- **relay**：白色（有分数但两个维度都非最高）

### Shell 命令别名

```text
graph hits          HITS hub+authority 二部图分解
hits                简写
graph ha            别名
ha                  别名
hub authority       别名
```

### 示例输出

```text
 graph hits
 ─────────────────────────────────────────────────────────
  vector             hub   authority  role
  [21:0:3:0]         0k      1000k  authority
  [21:0:4:0]         0k      1000k  authority
  [21:0:1:0]      1000k         0k  hub
  [21:0:2:0]      1000k         0k  hub
 ─────────────────────────────────────────────────────────
  4 node(s)  HITS/20iter  hubs: 2  authorities: 2
```

---

## 角色语义

| 角色 | hub 阈值 | authority 阈值 | 含义 |
|------|----------|----------------|------|
| **hub+authority** | ≥ 800k | ≥ 800k | 对称角色：环节点，稠密结构中的中继 |
| **authority** | 任意 | ≥ 800k | 被顶级 hub 引用；最佳的信号目的地 |
| **hub** | ≥ 800k | 任意 | 指向顶级 authority；最佳的信号转发者 |
| **relay** | 200k–800k | 200k–800k | 部分结构角色 |
| **isolated** | < 200k | < 200k | 无入边也无边指向已评分节点 |

---

## HITS / PageRank / Katz 三者对比

| 指标 | Katz（V2.42） | PageRank（V2.43） | HITS（V2.44） |
|------|--------------|--------------------|----------------|
| 每节点分数数 | 1（authority） | 1（authority） | 2（hub + authority） |
| 归一化 | 无 | 除以出度 | 每轮 L∞ |
| 孤立节点 | 0 | 150,000（传送地板值） | 0, 0 |
| 环节点 | SCALE/7 | 1,000,000 | hub=auth=1M |
| 最佳提问 | "最多游走流量？" | "随机游走频率？" | "指针 vs 被引用目标？" |
| OS 类比 | `netstat -s` | `top` | `vmstat` 二部视图 |

---

## 测试用例（10/10 通过）

| 编号 | 用例 | 验证点 |
|------|------|--------|
| 1 | 空图 | node_count=0 |
| 2 | 孤立节点 | hub=0, auth=0 |
| 3-6 | 单边 / 链 / 扇出 / 扇入 | hub/auth 分布正确 |
| 7-8 | 互向环 / 3-环 | hub=auth=1,000,000 |
| 9 | 二部结构 | hub、auth 分离正确 |
| 10 | 排序验证 | 按 authority 降序 |

---

## 测试摘要

**宿主测试套件总计：443 个**（此前 433，+10）

```text
gos-graph-hits-harness: 10/10 ✓
```

---

## 不变量确认

- [x] `graph_hits_inner` 为纯读操作，不推进 epoch，不写运行时状态
- [x] 孤立节点：hub=0, auth=0（区别于 Katz=0 与 PageRank=150k）
- [x] 输出恒按 authority 降序：`auth[0] ≥ auth[1] ≥ ...`
- [x] 无堆分配，no_std 安全，O(K×V×E)，K=20

---

## 后续建议

- `node checkpoint <vec>` — 快照节点状态到 diff ring（观测性）
- `journal ring <N>` — 运行时可配置的 JournalRing 容量
- `graph sim <N>` — 模拟 N 步随机游走，输出信号流量轨迹
- PAL_U32 → attribute node 重构（Demo A 前置条件）

---

*由自动强化任务生成 · 2026-07-02*
