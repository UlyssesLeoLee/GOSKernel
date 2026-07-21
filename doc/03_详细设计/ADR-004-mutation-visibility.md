# ADR-004: Cypher Mutation Visibility Semantics — Epoch-Published

| 项目 | 内容 |
|---|---|
| 文档编号 | GOS-DOC-03-02 |
| 所属阶段 | 03・详细设计（ADR） |
| 版本 / 状态 | v1.1 / **已批准** |
| 作成 / 审核 / 批准 | GOS 核心团队 |
| 基线日期 | 2026-06-30 |
| 最终更新 | 2026-07-01 |

**变更履历**

| 版本 | 日期 | 变更内容 | 作成者 |
|---|---|---|---|
| v1.0 | 2026-06-30 | 自动硬化任务生成，批准为 V2.1 最终可见性语义 | GOS 核心团队 |
| v1.1 | 2026-07-01 | 纳入日系工程阶段目录（03_详细设计）；补充文档管理信息 | GOS 核心团队 |

> 状态：**已批准**  
> 日期：2026-06-30  
> 决策人：GOS 核心团队  
> 前置：ADR-001（边代数宪法），ADR-002（Rewrite Engine 语义草稿，**注：ADR-002/ADR-003 目前未在 doc/03_详细设计 中检索到独立文档**，如已归档请补充索引链接）  
> 影响范围：`gos-cypher-mut`，`gos-runtime`，V2.3 renderer，V2.4 capability check

---

## 背景与问题

V2.1 的核心交付是 `MutationDispatcher` 接通真实 runtime edge table。在此之前，
必须决定一个宪法级问题：

> **一条 Cypher mutation 何时对 reader 可见？**

这个决定反向约束 V2.3 renderer 的读图方式、V2.3 Subscribe 反向传播的触发时机，
以及 V2.4 capability 检查读 Grant 路径的快照策略。**在所有 UI 工作开始前钉死它。**

---

## 候选方案

### 方案 A — Immediate（立即可见）

mutation 完成后，下一次任何读操作（`edge_page`、`edge_vector_for_id`、
`node_page` 等）立即看到新状态。

**优点**：实现简单；当前 `register_edge` / `unregister_edge` 已是 immediate。  
**缺点**：
- Reader 可能看到"写了一半"的复合 mutation（如 RebindUse = delete + create）。
- V2.3 renderer 在同一帧内可能看到两个不一致快照（删旧 Use 可见，但新 Use 未建）。
- 并发 subscription fan-out 的触发顺序不确定（哪个 reader 先看到变化？）。
- **订阅触发时机模糊**：Subscribe 反向传播何时 fire？

### 方案 B — Epoch-Published（纪元发布，推荐）

mutation 提交后，runtime `graph_epoch` 原子递增一次。Reader 永远看一个 **一致
的 epoch snapshot**，不会看到 epoch 边界内的中间态。V2.3 renderer、Subscribe
反向传播、capability 路径查询，全部基于 epoch 读图。

写路径：`apply_cypher_mutation` → 所有 runtime 变更在同一锁持有期内完成 →
释放锁时 `graph_epoch` 已递增 → 下一个 reader 看到完整新图。

**优点**：
- 复合 mutation（RebindUse = delete + create）对 reader 不可见中间态（两步都在同一个锁持有期内完成，epoch 只增一次）。
- Subscribe 触发时机明确：`graph_epoch` 变化 → engine 遍历 reactive 反向索引 → 向订阅者发 Send。
- V2.3 renderer 可以缓存上一次渲染的 epoch，`graph_epoch() == last_epoch` 时跳过重绘（Demo #2"最小重绘 0 行"的基础）。
- capability 路径查询天然在稳定 epoch 上运行（不会看到 Grant 边 add/remove 的中间态）。

**缺点**：
- Reader 必须重新获取锁才能看到 mutation（而非直接观察内存）；在 `no_std` 单核内核中这个代价几乎为零（`spin::Mutex`）。
- Epoch 语义需要所有 composite mutation 在单次锁持有内完成——当前 `rebind_use` 已满足此约束（delete + create 在同一 `&mut self` 调用内）。

---

## 决定

**选择方案 B — Epoch-Published。**

理由：
1. `graph_epoch` 已存在于 `GraphRuntime`（`graph_epoch: u64` 字段，每次 `register_edge` / `unregister_edge` / `register_node` 调用时递增）。不需要额外机制，只需遵守约定。
2. Subscribe 反向传播（V2.3）触发条件 = epoch 发生变化，这是最简单的可实现语义，无需 per-field dirty tracking。
3. `apply_cypher_mutation` 当前在单次 `RUNTIME.lock()` 持有内完成全部 runtime 修改，天然满足 epoch 边界约束。
4. 与 `AuditedMutation`（`tick` + `source` 字段）正交：audit trail 记的是写入时刻，epoch 记的是结构版本。

---

## 实现约束（落地规范）

1. **所有 Cypher composite mutation 必须在单次 `RUNTIME.lock()` 持有期内完成全部 runtime 修改**，不得在持有期外进行第二次写入。当前 `rebind_use`（delete + create Use 边）满足此约束。未来 `MutationBatch`（V2.1+ 附录 B）同样必须满足。

2. **`graph_epoch` 仅由结构性 mutation 递增**（`register_node`、`unregister_node`、`register_edge`、`unregister_edge`）。`SetProp` 等属性写（V2.1+ 未来扩展）需要单独评估是否递增 epoch——推荐仅在影响图拓扑时递增。

3. **Reader 侧约定**（V2.3 renderer、V2.4 capability check）：  
   - 在读取前记录 `let epoch = gos_runtime::graph_epoch()`。  
   - 读完后验证 `gos_runtime::graph_epoch() == epoch`（如有需要）——单核下此检查通常冗余，但在 host-bridge 多线程读图时有价值。  
   - Subscribe 反向传播的触发由 rewrite engine 负责，reader 不需要主动轮询 epoch。

4. **`apply_cypher_mutation` 的返回值语义**：返回 `AuditedMutation`，其 `tick` 字段是写入时的 `runtime.tick`（dispatch cycle 计数器，非 `graph_epoch`）。调用方可用此记录 mutation 的发生顺序；epoch 单独通过 `gos_runtime::graph_epoch()` 查询。

---

## 与 V2.x 路线图的关联

| 依赖方 | 影响 |
|---|---|
| **V2.2 RewriteEngine** | `fire` 检查 guard 时读 epoch-consistent 快照；trigger = epoch 变化 |
| **V2.3 Subscribe** | 反向传播索引在 epoch 变化时触发；renderer 比较 epoch 跳过空帧（Demo #2） |
| **V2.4 capability check** | Grant 路径查询基于最新 epoch 快照；revoke = epoch 递增触发重新验证 |
| **V2.5 Soul demo** | `MATCH ... CREATE` → epoch 变化 → Subscribe → render node fire → 下一帧出节点 |

---

## 反驳意见及回应

**"Immediate 更简单，epoch 是过度设计。"**  
→ Immediate 在单条 mutation 时等同于 epoch，但在 composite mutation（RebindUse、未来 MutationBatch）时行为不同。Subscribe 触发时机在 Immediate 下无法定义清晰。代价是在写路径的注释里声明一个约束，不是新代码。

**"graph_epoch 和 runtime.tick 混用会让人困惑。"**  
→ 两者明确分工：`graph_epoch` = 图拓扑版本（结构 mutation 专用），`tick` = dispatch cycle 计数（scheduler 和 audit 专用）。ADR-004 在此钉死此分工。

---

*本 ADR 由 2026-06-30 自动硬化任务生成，批准为 V2.1 最终可见性语义。*
