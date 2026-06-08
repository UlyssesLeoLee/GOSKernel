# ADR-004：Mutation 可见性与原子性语义

> 状态：**提案（待批准）** · 日期：2026-06-08 · 配套：[ADR-001 边代数宪法](./ADR-001-edge-algebra-constitution.md) · [V2 开发计划](../plan/V2_DEVELOPMENT_PLAN.md) Phase V2.1
>
> 口径：本 ADR 决定"一条 Cypher mutation 被应用后，何时、以何种一致性对 reader（render / shell / host bridge）可见"。这个决定**反向约束 V2.3 renderer 怎么读图**，所以必须先于 MutationDispatcher 的实现钉死（V2 计划 sequencing 铁律 #3：ADR 先于实现）。

## 一、上下文（基于现状代码，非凭空设计）

勘察 `gos-runtime` / `gos-cypher-mut` 现状，三个事实决定了本 ADR 的形状：

1. **`graph_epoch` 机制已存在**。`gos-runtime` 持有一个单调递增的 `graph_epoch`（[lib.rs:297](../crates/gos-runtime/src/lib.rs)），注释明言"每次结构性 mutation（node 或 edge）递增……host bridge 读它来跳过无变化的重渲染"。`register_edge` / `unregister_edge` / `register_node` 各自在末尾 `graph_epoch += 1`（[lib.rs:539/555/585](../crates/gos-runtime/src/lib.rs)）。`graph_epoch()` 已作为 pub 函数暴露（[lib.rs:1680](../crates/gos-runtime/src/lib.rs)）。**所以"epoch-published 可见性"不是新发明，而是把既有机制形式化成契约。**

2. **`MutationDispatcher` trait + `apply_mutation` 已存在**（[gos-cypher-mut/src/lib.rs:136-175](../crates/gos-cypher-mut/src/lib.rs)），但**没有任何真实 impl**——唯一实现是 runtime-harness 里的 `Stub`（[runtime.rs:147](../host-tests/gos-runtime-harness/tests/runtime.rs)）。V2.1 的真实缺口是：把这个 trait 接到 `gos-runtime` 的真实 edge table 上。

3. **mutation 现在是 edge-only，且这是有意的**。`CypherMutation` 只有 `AddEdge` / `RemoveEdge` / `RebindUse`，且只允许 `Mount` / `Use`（[lib.rs:8-21](../crates/gos-cypher-mut/src/lib.rs)）。理由写在代码里："允许 Cypher 凭空创建或销毁 node 会让下游每一个 claim 和 restart_generation 计数失效"——因为 Phase B 的 instance/quota/fault 模型挂在稳定 `NodeId` 上。**本 ADR 尊重这条约束，不扩张到 node create/delete。**

## 二、决定

### 2.1 可见性 = epoch-published（不是 immediate）

一条逻辑 mutation 应用后，**在它提交的 epoch 之前对 reader 不可见，在之后整体可见**。reader 通过比较 `graph_epoch` 判定"自上次读取以来图是否变化"。

形式化契约：

> 设 mutation `m` 在 epoch `e` 提交。任何在 `graph_epoch >= e` 时刻发起的读，都观察到 `m` 的**完整**效果；任何在 `graph_epoch < e` 时刻完成的读，都观察不到 `m` 的**任何**效果。不存在"观察到 `m` 一半"的读。

### 2.2 原子批提交 = 一条逻辑 mutation 一次 epoch 递增

这是本 ADR 的**核心**，由 `RebindUse` 这个动机案例逼出来：

`RebindUse`（theme 切换）= 删除旧 `Use` 边 + 创建新 `Use` 边。当前 `unregister_edge` 和 `register_edge` **各自**递增 `graph_epoch`（[lib.rs:555/585](../crates/gos-runtime/src/lib.rs)）。若 dispatcher 朴素地"先删后增"，则中间存在一个 epoch，`theme.current` **持有零条 `Use` 边**——直接违反 ADR-001 钉死的 `Use` 排他不变式（`exclusive=true` 意味着恰好一条）。一个在该窗口读图的 renderer 会看到"无主题"状态并可能渲染崩坏。

**决定**：一条逻辑 mutation（尤其 `RebindUse`）的所有子编辑必须在**一次** `graph_epoch` 递增下原子提交。reader 永远观察不到 `theme.current` 处于"零 `Use` 边"或"两条 `Use` 边"的中间态。

落地含义（V2.1 实现切片要做的）：

- `gos-runtime` 新增一个**延迟递增**的内部编辑路径，或一个原子 `rebind_exclusive_edge(from, kind, new_target)` 方法：在锁内完成 remove+insert，末尾只 `graph_epoch += 1` 一次。
- `register_edge` / `unregister_edge` 的单次递增语义**不变**（零行为变更）；新增的是"批"入口，不是改旧入口。

### 2.3 Snapshot isolation = 锁内一致读 + epoch 标记（不是 MVCC）

reader 在 `RUNTIME.lock()` 持有期间看到的图是自洽的（当前 `snapshot()` 已在锁内计算，[lib.rs:1134](../crates/gos-runtime/src/lib.rs)）。本 ADR **不引入多版本并发控制（MVCC）**——Gen-1 是单会话，render / shell / supervisor 都在同一个 `service_system_cycle` 节律里串行读写，不需要为并发 reader 保留历史版本。

唯一的加法：`GraphSnapshot` 当前**不含 epoch**。本 ADR 要求 `snapshot()` 附带它取样时的 `graph_epoch`，使 reader 能落实 §2.1 的契约（"我这份 snapshot 是哪个 epoch 的"）。这是一个 additive 字段，零破坏。

### 2.4 审计与提交同 epoch

`AuditedMutation`（[lib.rs:74](../crates/gos-cypher-mut/src/lib.rs)）的 envelope 必须在 mutation 提交的**同一**临界区内入队，使"图已变"与"审计已记"对 reader 同时成立。不存在"图变了但审计还没记"或反之的可观察窗口。

## 三、明确的范围外（防止 scope creep）

| 范围外项 | 去向 | 理由 |
|---|---|---|
| **node create / delete** | 未来 ADR-005 | 破坏 claim/quota/NodeId 稳定（§一.3）。这是独立的硬问题，需要先解决"如何在不失效 Phase B 模型的前提下创建 node"。V2.1 不碰。 |
| **跨 cycle 事务 / 回滚** | 未来 | 单条逻辑 mutation 原子即可满足 Gen-1；多步事务（"要么全成要么全败的 N 条 mutation"）是后话。 |
| **MVCC / 历史版本** | 不做 | 单会话串行节律下是过度工程（§2.3）。 |
| **属性写回（SetProp）** | 未来 ADR | 当前 `CypherMutation` 无 SetProp；node 属性 mutation 与 node create 同类问题。 |

## 四、动机案例走查：theme 切换（Demo C 的写路径地基）

`RebindUse { from: theme.current, new_target: theme.shoji }` 在本 ADR 下的执行：

1. dispatcher 进入 `RUNTIME.lock()` 临界区。
2. 找到 `theme.current` 现有的 `Use` 出边（指向 `theme.wabi`），原地解绑。
3. 插入 `theme.current -[Use{exclusive}]-> theme.shoji`。
4. **一次** `graph_epoch += 1`。
5. 同临界区内入队 `AuditedMutation` envelope。
6. 释放锁。

任何 reader 要么看到旧 epoch（指向 wabi，恰好一条 Use），要么看到新 epoch（指向 shoji，恰好一条 Use）。排他不变式全程成立。这正是 V2.3 让 theme "0 行扩散"的写路径前提——V2.3 的 Subscribe 反向传播挂在 §2.1 的 epoch 契约上。

### 四.1 集成阻塞发现（2026-06-08，edge-key 两套约定）

把上述原子原语真正接到生产 theme 切换（[`k-shell::sync_theme_use_edges`](../crates/k-shell/src/lib.rs)，它现在做 `unregister`×2 + `register` = 3 次 epoch 递增，有零-`Use` 窗口）时，发现一个**预先存在的不一致**：

- k-shell 的 theme Use 边用 `THEME_EDGE_KEY = "theme.use"` 派生 edge_id（[k-shell:174](../crates/k-shell/src/lib.rs)）。
- `rebind_exclusive_use` 与 `RuntimeDispatcher::add_edge` 用 `"use"`（k-cypher 约定，[k-cypher:175](../crates/k-cypher/src/lib.rs)）。

直接把 `sync_theme_use_edges` 改调 `rebind_use` 会产生 **edge_id 不匹配**：rebind 按"扫描 from+type"删旧边（仍能正确清掉 `"theme.use"` 边）并按 `"use"` 建新边，但此后 `theme_edge_id()`（按 `"theme.use"` 计算）不再等于实际边的 id。`linked_theme_kind()` 靠扫描不靠 id，仍能工作；但任何按 `theme_edge_id()` 精确查/删的代码会失配。

**结论**：k-shell 集成被**两件事**阻塞，autonomous push 不擅自改：(1) 统一 edge-key 约定（把 theme 改成 `"use"`，或让 rebind 接受 key 参数——涉及边身份稳定性，是一个设计决定）；(2) 改动运行中桌面行为，需一次 QEMU 桌面目视验证（切 theme 不闪/不崩）。原子原语本身已就绪并被 harness 证明，只是尚未在生产 caller 上启用。

## 五、考虑过但否决

| 方案 | 否决理由 |
|---|---|
| **immediate 可见（每次子编辑即可见）** | 直接产生 §2.2 的 `RebindUse` 中间态，违反 `Use` 排他不变式。被动机案例直接证伪。 |
| **全 MVCC + 读快照保留旧版本** | Gen-1 单会话串行，无并发 reader 需要旧版本；过度工程（§2.3）。 |
| **把原子性推给 caller（让 k-cypher 自己保证不被读打断）** | caller 无法控制 `graph_epoch` 与锁；原子性必须由持锁的 runtime 提供。 |
| **V2.1 直接做 node create** | 违反现有 §一.3 的 claim/quota 约束；是独立 ADR-005 的范畴。 |

## 六、后果

**正面**：
- `RebindUse`（theme 切换）成为可被 V2.3 安全订阅的原子事件——Demo C 的写路径地基就位。
- 复用既有 `graph_epoch`，新增面极小（一个原子 rebind 入口 + `GraphSnapshot` 加一个 epoch 字段）。
- 可写 harness：构造 rebind，断言"不存在零/双 Use 边的可观察 epoch"。

**代价**：
- `gos-runtime` 需新增原子 rebind 路径（约束：不改 `register_edge`/`unregister_edge` 旧语义）。
- reader 端约定俗成要"比较 epoch 再决定是否重读"——V2.3 renderer 会正式依赖它。

## 七、批准检查单（V2.1 退出条件的前置）

- [ ] §2.2 原子批提交：harness 证明 `RebindUse` 全程不暴露零/双 `Use` 边
- [ ] §2.1 epoch 契约：`GraphSnapshot` 暴露 `graph_epoch`；reader 可据此判定可见性
- [ ] §2.4 审计同 epoch：mutation 提交与 envelope 入队在同一临界区
- [ ] 真实 `impl MutationDispatcher`（接 `gos-runtime` edge table）替换 harness `Stub`，且不改 `register_edge`/`unregister_edge` 旧行为（零行为变更回归）
- [ ] §三范围外项未被偷偷实现（治理：node create/delete 仍被 `pre_validate` 拒绝）
