# ADR-002：图重写引擎语义（Rewrite Engine）

> 状态：**提案（待批准 — 含一个必须由你拍板的宪法级决定，见 §六）** · 日期：2026-06-08 · 配套：[ADR-001 边代数](./ADR-001-edge-algebra-constitution.md) · [ADR-004 可见性](./ADR-004-mutation-visibility.md) · [V2 计划](../plan/V2_DEVELOPMENT_PLAN.md) Phase V2.2
>
> 口径：本 ADR 定义 V2.2 的核心——把 boot、调度、节点执行统一成**图重写**。它是 V2.2 实现的前置门禁（计划 sequencing 铁律 #3：ADR 先于实现）。**本文档由 autonomous push 起草为提案，实现尚未开始，因为 §六 的决定只有你能做。**

## 一、上下文与现状

V2.0 钉死了边代数（`Refer/Send/Bind/Grant` + 属性），V2.1 通了 edge 写路径（`MutationDispatcher` + epoch 可见性）。V2.2 要兑现"boot = 在 manifest 图上求重写不动点；调度 = 边传播"。

现状代码的相关事实：
- `hypervisor::kernel_main`（[main.rs:16-128](../crates/hypervisor/src/main.rs)）是**硬编码顺序**：CPU 特性 → hal init → supervisor bootstrap → boot_builtin_graph → realize_boot_modules → kernel-tier drivers → ring3 → 稳态 `loop { service_system_cycle(); render_frame(); hlt() }`。
- `gos_supervisor::service_system_cycle` 是当前的 dispatch loop（有 2048 hard cap，[main.rs:99](../crates/hypervisor/src/main.rs)）。
- `graph_epoch` 已存在（ADR-004）；mutation 经 `RuntimeDispatcher` 应用。

V2.2 不是推倒重来，而是**把这条硬编码链重新表达成图**，让顺序由依赖边求解而非代码写死。

## 二、重写规则形式化

一条 rewrite rule 是 `LHS → RHS`，带 guard：

- **LHS（左手模式）**= 一个 Cypher `MATCH` 子图模式。
- **guard** = 对匹配结果的谓词（fire 条件）。
- **RHS（右手）**= 一组 Cypher mutation（`MATCH`/`CREATE`/`DELETE`/`SET`，经 ADR-004 的 `MutationDispatcher` 原子提交）。

> **node "fire" ≡ 该 node 的 rewrite rule 的 LHS 命中且 guard 成立时，执行其 RHS。** 一个 node 的"代码"就是它的 rewrite rule。

这统一了一切：syscall、IPC、中断进入、capability 调用，都是"某 node fire → 发出 mutation（Send 边 / Create / Delete）"。这正是"Cypher 是 ISA"（附录 B 的 ISA 草图）在引擎层的落地。

## 三、调度 = 边传播

- **ready set** = 存在 ≥1 条满足 guard 的待处理入向 `Send` 边的 node 集合。
- engine 从 ready set 选一个 node（按 lane-class **标签**排序——lane class 从 supervisor 的调度类退化为 node 上的属性），fire 它。
- fire 产生新的 mutation（新 `Send` / `Create` / `Delete`），可能让别的 node 进入 ready set。
- 循环直到 ready set 空 = **quiescence**。

`Depend` 边（= ADR-001 的 `Refer` + fire-guard "B 必须 ready"）天然表达 boot 顺序：`(gdt)-[Depend]->(cpu_features_ready)` 意味着 gdt 的 guard 是"cpu_features_ready 已 fire"。engine 维护反向 readiness 索引，按依赖边求解 fire 顺序——**没有"先 GDT 再 IDT"的代码**。

## 四、Quiescence 不变式（吸收原 ADR-003）

> **signal queue 空 ∧ rewrite queue 空 → quiescence。任何"系统在跑但无人请求"的状态都是 bug。**

（原计划的 ADR-003 内容并入此处——quiescence 是 engine 的终止性质，与重写引擎不可分。）

作用：
- **测试方法学**：跑 N 步必达 quiescence；不达 = livelock = 治理失败。harness 可断言。
- **节能**：quiescence = `hlt`（现稳态 loop 已 hlt，但 V2.2 让"无变化即不 fire"成为结构性质而非偶然）。
- **可验证性**：每条 rewrite rule 的 termination 可单独证。
- **故障收敛**：fault 也必须最终静默（传播完成或被 Grant 拓扑 firewall）。

**因果深度计**替换 `service_system_cycle` 的 2048 hard cap：跑满不再是静默截断，而是 telemetry "本帧因果链深 N" 的告警。

## 五、迁移策略（不推倒重来）

V2.2 分三个可独立验证的子切片，每个都不破坏稳态：

1. **V2.2a — RewriteRule trait + engine 骨架**，与现 `service_system_cycle` **并存**：先让 1-2 个 builtin node（如 theme rebind）走 rewrite path，其余仍走旧 dispatch。harness 证 quiescence。
2. **V2.2b — boot manifest 静态图**：把 `kernel_main` 的硬编码序逐项搬成 `Depend` 边，engine `run_to_quiescence` 求解。`kernel_main` 逐步瘦身（目标 < 300 行）。每搬一项跑启动 smoke。
3. **V2.2c — 调度统一**：`service_system_cycle` 的 ready-queue/lane 逻辑迁入 engine 的边传播；因果深度计上线。

每子切片：harness（quiescence test、boot 拓扑序 test、livelock 检测）+ 启动 smoke 必绿才合入。

## 六、⚠️ 必须由你拍板的宪法级决定：渲染模型

V2.3（响应式 Subscribe）依赖一个 V2.2 必须先定的问题。这是我**不能替你做**的决定（之前已标记为"只有你能回答"）：

### 选项 A —— "renderer reads graph"（renderer 是图的读者）
图是数据，renderer 是独立子系统，通过 epoch + snapshot 解耦读图。
- **优点**：工程稳，renderer 可独立演化，与现 `fbtest::render_frame` 改动小。
- **代价**：GOS 的差异化退化为"内核里有个图数据库 + 一个会读它的渲染器"。**不算颠覆**。

### 选项 B —— "graph IS the scene"（图即场景）—— 我的推荐
每个 node 自带 render policy（`VectorAddress` 决定 3D 位置），边自带视觉语义（`Use` 边 → 光线，`Mount` 边 → 吸附弹簧）。renderer 是 graph subscription 的**纯函数**。
- **优点**：兑现"nodes = render units, edges = visual conduits"；theme 0 行扩散、脏矩形免费、soul demo trivial 都自然涌现（V2.3 的 5 个 killer demo 全靠它）。
- **代价**：renderer 重写量大；性能需 fast-path 标签节点兜底；fbtest.rs 那 1764 行命令式 UI 基本蒸发重组。

**为什么必须现在定**：rewrite engine 的 mutation→fire 传播机制（§三）要不要原生支持"render node 作为 Subscribe 反向传播的终点"，取决于这个选择。选 B 则 Subscribe + reactive 属性（ADR-001 §2.2）是引擎的一等机制；选 A 则 renderer 在引擎之外轮询 epoch。**选错则 V2.3-V2.5 全部返工。**

> 我的建议是 **B**——它是 GOSKernel 这个名字对得起的唯一选项，且 V2.0/V2.1 的边代数+epoch 正是为它铺的路。但这是你的宪法级决定，本 ADR 不替你锁定。请在批准本 ADR 时明确 A 或 B。

## 七、考虑过但否决

| 方案 | 否决理由 |
|---|---|
| 一步到位重写 `kernel_main` 为纯 engine | 高风险、不可增量验证；§五 的三子切片是可控路径。 |
| 保留 2048 hard cap | 静默截断无法归因；因果深度计是诚实替代。 |
| rewrite rule 用自定义 DSL 而非 Cypher | 违背"Cypher 是 ISA"；徒增第二套语言。LHS 复用 k-cypher 的 MATCH。 |

## 八、批准检查单

- [ ] **§六 渲染模型 A/B 决定**（阻塞 V2.3-V2.5；本 ADR 批准时必须明确）
- [ ] §二 rewrite rule trait 形状评审（LHS=MATCH / guard / RHS=mutation）
- [ ] §四 quiescence 不变式 + 因果深度计纳入 harness 方法学
- [ ] §五 三子切片的"每切片不破坏稳态 + smoke 绿"作为合入门禁
- [ ] 确认 ADR-003（quiescence）并入本 ADR §四，不再单列
