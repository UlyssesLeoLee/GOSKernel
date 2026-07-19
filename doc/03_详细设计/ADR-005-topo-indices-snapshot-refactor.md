# ADR-005: `graph_topo_indices*` 家族 — Snapshot-Release 重构

| 项目 | 内容 |
|---|---|
| 文档编号 | GOS-DOC-03-03 |
| 所属阶段 | 03・详细设计（ADR） |
| 版本 / 状态 | v1.0 / **提案（待批准）** |
| 作成 / 审核 / 批准 | GOS 核心团队 / 待全员评审 / 待批准（见 §八批准检查单） |
| 基线日期 | 2026-07-16 |
| 最终更新 | 2026-07-16 |

**变更履历**

| 版本 | 日期 | 变更内容 | 作成者 |
|---|---|---|---|
| v1.0 | 2026-07-16 | 提案初版；由 PR #2 batch CI 自我 review（topo19 diff）发现的系统性问题触发 | GOS 核心团队 |

> 状态：**提案（待批准）**
> 日期：2026-07-16
> 决策人：GOS 核心团队
> 前置：ADR-004（Epoch-Published 可见性语义）、`.claude/skills/gos-topology-snapshot-pattern`
> 影响范围：`crates/gos-runtime/src/lib.rs`（`graph_topo_indices` .. `graph_topo_indices22` 共 22 个函数）；不涉及 `k-shell`、`host-tests` 的对外签名

---

## 背景与问题

`graph_topo_indices` 系列函数（无编号版 + `2`..`22`，共 22 个）从 V3.x 早期开始，
以"复制上一个函数、改算法体"的方式持续由自动强化任务生成，每次新增一个化学图论
拓扑指标（Wiener 变体、Zagreb 变体、Sombor 变体等）。这个家族目前存在两个未被
先前 22 次强化循环发现的系统性问题：

**问题一 —— 锁持有时间违反既定架构约束。**
`gos-topology-snapshot-pattern`（本项目已有 skill，V2.42 起为 `graph_katz` /
`graph_pagerank` / `graph_hits` / `graph_community` 等函数确立并文档化）规定：
任何 O(V×E) 图分析函数必须先 `RUNTIME.lock().topology_snapshot()` 拷贝拓扑、
**释放锁**，再在快照上计算。原因：`RUNTIME` 是 `spin::Mutex`，不禁用中断；
`post_irq_signal()`（中断上下文代码）也会 `RUNTIME.lock()`。若分析函数在持锁
状态下跑完整 O(V×E) 计算，同核心上触发的硬件中断会在 ISR 里对同一把锁自旋等待
——而持锁方因为已被中断，永远无法恢复执行来释放锁，造成内核死锁。

这个问题曾在 katz/pagerank/hits/community 上真实出现过（CodeRabbit comment
3515944030 发现，已修复）。但 `graph_topo_indices` 家族的全部 22 个函数从第一
个（`graph_topo_indices`）到最新的 `graph_topo_indices22`（本文档撰写时仍是
未提交状态），**无一例外**全部使用 `RUNTIME.lock().graph_topo_indicesN_inner()`
直接持锁计算，是既定架构约束里唯一被系统性遗漏的函数家族。`topo19`
（Reverse Wiener + RCW + Terminal Wiener，双 BFS 阶段）是该家族里计算量最大、
因而持锁时间最长、死锁概率最高的实例。

**问题二 —— 编译期无感知的高强度代码重复。**
每个函数开头约 25～30 行"紧凑节点索引 + 无向 u128 邻接位掩码构造 + 边计数"逻辑，
逐字重复了至少 6 次以上（`topo9`、`topo18`、`topo19` 等经交叉核对完全一致）。
这不是本 ADR 的主要动因，但与问题一共享同一处代码，一并解决成本最低。

两个问题都是在为 `topo19`（V3.30）做完整性自查（10 角度并行 code review）时
发现的——不是新引入的回归，而是自动强化循环持续了 19～22 轮都未被察觉的存量债务。

---

## 候选方案

### 方案 A — 仅修锁持有（最小改动）

只把 22 个 wrapper 函数从 `RUNTIME.lock().xxx_inner()` 改成
`let snap = RUNTIME.lock().topology_snapshot(); GraphRuntime::xxx_inner(&snap)`，
`_inner` 函数体（含重复的邻接构造块）原样保留，只是参数从 `&self` 换成
`snap: &GraphTopologySnapshot`。

**优点**：改动面最小，风险最低。
**缺点**：不解决问题二；下一次自动强化循环大概率继续复制"25 行手写邻接构造"
这个旧模板，因为它仍然是文件里最新、最常见的写法。

### 方案 B — 锁模式 + 共享邻接构造辅助函数（推荐）

在方案 A 基础上，新增一个独立自由函数：

```rust
fn snapshot_compact_adjacency(snap: &GraphTopologySnapshot)
    -> ([u128; MAX_NODES], usize, usize)  // (adj, node_count, edge_count)
```

放在 `GraphTopologySnapshot` 的 `impl` 块（`node_slot_by_id` 所在处，约第
306～326 行）附近。每个 `_inner` 函数的重复构造块替换为一行调用，算法本体
（BFS / Wiener / Zagreb / Sombor 数学）完全不动。

**优点**：
- 彻底解决问题一（锁持有）。
- 同时消除问题二的存量重复（22 处 → 1 处）。
- 输出可证明不变（纯重构，相同输入产生相同输出），现有 220+ 个 host-test
  即为回归测试，无需新写测试。
- 独立自由函数（而非绑定在 `GraphTopologySnapshot` 的 `impl` 方法上）保持
  与快照结构体本身解耦，符合"快照只负责存数据，计算逻辑外置"的既有分工。

**缺点**：改动面比方案 A 大（22 处调用点 + 22 处函数体开头），但每处改动都是
机械替换，无算法逻辑风险。

### 方案 C — 完整 BFS 引擎抽象

在方案 B 基础上，进一步把 BFS 遍历本体、连通分量检测、偏心率计算也抽象成
共享的 `GraphBfsContext`，让未来所有图指标函数都基于它构建。

**优点**：从根源上消除"每个新函数手写一遍 BFS"的模式，是最彻底的方案。
**缺点**：改动最大；BFS 在不同函数里的使用方式有细微差异（如 topo19 需要
两阶段 BFS 且第二阶段依赖第一阶段算出的全局直径），强行统一到一个上下文
结构可能引入新的抽象层不匹配问题。且当前 22 个函数里只有 topo19/20/21/22
这类"多指标聚合"函数明确需要连通分量/直径这类跨节点聚合状态，其余大多数
只需要单纯的邻接位掩码——方案 C 的收益不如方案 B 边际清晰。

---

## 决定

**选择方案 B。**

理由：
1. 方案 A 不解决问题二，且不阻止未来的自动强化循环继续复制旧模板——
   相当于只治标不治本，问题会在 topo23、topo24… 持续累积。
2. 方案 C 的额外抽象收益不确定（BFS 使用模式在不同函数间差异较大），
   且改动面显著更大，不符合 YAGNI。
3. 方案 B 的 `snapshot_compact_adjacency` 提取，是从 22 份逐字相同的代码
   中提炼出的最大公约数——这不是预先设计的抽象，而是已被验证过 22 次的
   实际重复模式，风险最低、收益最确定。

---

## 实现约束（落地规范）

1. **辅助函数签名与位置**：
   ```rust
   fn snapshot_compact_adjacency(snap: &GraphTopologySnapshot)
       -> ([u128; MAX_NODES], usize, usize)
   ```
   独立自由函数，紧邻 `impl GraphTopologySnapshot { fn node_slot_by_id(...) }`
   之后定义。

2. **每个 `graph_topo_indicesN` 的三处机械改动**：
   - wrapper：`RUNTIME.lock().graph_topo_indicesN_inner()` →
     `let snap = RUNTIME.lock().topology_snapshot(); GraphRuntime::graph_topo_indicesN_inner(&snap)`。
   - `_inner` 签名：`impl GraphRuntime { pub fn graph_topo_indicesN_inner(&self) -> (...) }` →
     `impl GraphRuntime { fn graph_topo_indicesN_inner(snap: &GraphTopologySnapshot) -> (...) }`
     （私有 `fn`，不是 `pub fn`——避免 `&GraphTopologySnapshot` 这个私有类型
     触发 Rust 的 `private_interfaces` lint，与 katz/pagerank/hits/community
     已确立的做法一致）。
   - 函数体：原 ~25 行邻接构造块 → `let (adj, nc, edge_count) = snapshot_compact_adjacency(snap);`，
     其后代码不变。

3. **范围**：`graph_topo_indices`（无编号）到 `graph_topo_indices22` 共 22 个函数，
   含撰写本文档时仍处于未提交状态的 `topo22`——它是修复成本最低的时机，直接
   按新模板落地，不先以旧模板提交再单独补丁。

4. **零对外 API 变更**：`pub fn graph_topo_indicesN()` 的参数与返回类型不变，
   `k-shell` 的 22 个 `dispatch_graph_topo_indicesN` 与 `proc.rs` 路由、
   22 个 host-test harness（220+ 测试）均不需要改动。

5. **验证门槛**：改动完成后，`cargo check -p gos-kernel`、
   `./tools/verify-graph-architecture.ps1`、全部 host-test harness 三者全绿
   才可提交。

---

## 未来防护 —— 让下一次自动强化循环自然沿用正确模式

只修复现有 22 个函数不足以阻止 topo23、topo24… 继续复制旧模板，因为自动强化
循环的实际生成方式就是"复制上一个 topoN 函数、改算法体"。本 ADR 落地后，
须同步更新 `.claude/skills/gos-topology-snapshot-pattern`，把
`snapshot_compact_adjacency` 的调用方式写入"新建 topoN 函数标准起手式"一节，
使正确模式成为下一次生成时自然复制的对象，而不是依赖人工每次提醒——这是把
架构约束从"文档里的规则"升级为"代码库里唯一存在的可复制范例"，是本项目
"涌现式设计"方向下让正确性自然涌现、而非依赖持续人工审计的具体实践。

---

## 与 V3.x 路线图的关联

| 依赖方 | 影响 |
|---|---|
| **中断安全** | 22 个函数持锁时间从 O(V×E) 降到 O(V+E)（快照拷贝），消除 `post_irq_signal` 死锁风险 |
| **未来 topo23+** | 新函数起手式变为一行 `snapshot_compact_adjacency` 调用，而非 25 行手写重复 |
| **`@gos.vk` 可视化管线** | 若未来 VK 渲染需要高频轮询多个图指标（如仪表盘），锁持有时间缩短直接降低渲染帧的调度延迟 |
| **k-rope 物理集成（Phase R2）** | 物理 tick 与图指标查询若共享调度周期，缩短的锁持有窗口降低两者互相阻塞的概率 |

---

## 反驳意见及回应

**"这不是新 bug，为什么现在优先修？"**
→ 正是因为它不是孤立个案，而是 22 个函数、持续 19～22 轮强化循环都在复制的
系统性模式。越晚修，需要改的函数越多，且下次真正触发死锁前不会有任何测试信号
——host-test 完全没有中断上下文，无法覆盖这类 bug。

**"为什么不做方案 C 的完整抽象？"**
→ 见候选方案 C 的缺点：当前证据（22 个函数的实际差异）不支持"BFS 遍历本体
也该统一"这个更激进的判断；方案 B 已经消除了已验证的、逐字重复的那部分。

**"22 个函数一次性改完风险会不会太高？"**
→ 每处改动都是纯机械替换（签名 + 一行调用），不改变任何算法逻辑；输出
可证明不变。220+ 个既有 host-test 就是完整回归测试网，全绿即可信。

---

*本 ADR 由 2026-07-16 会话中的架构债务自查与用户确认的涌现式设计方向共同产出。*
