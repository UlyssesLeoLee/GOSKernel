# GOS V2 分阶段开发计划 — 产品级涌现式图论 OS

> 状态：提案 · 日期：2026-06-08 · 配套：[ADR-001 边代数宪法](../doc/ADR-001-edge-algebra-constitution.md)
>
> 本计划把 GOS 从"图数据库 + capability + 软光栅的精致 hobbyist OS"推进到 **Plan 9 之后第二个提出新计算抽象的 OS**。Plan 9 的 *everything is a file* 在 GOS 等价于 ***everything is a graph rewrite***。
>
> **产品级铁律（继承自项目记忆）**：每个 phase 必须随附 harness / golden test 一同合入。**无 harness = 不合入**。每个 phase 必须交付一个 killer demo——"系统做了一件没人显式编码的事"。做不出 demo，涌现就是 PPT。

## 〇、Prime Invariant（系统唯一不变式）

> **Quiescence：signal queue 空 ∧ rewrite queue 空 → 静默。任何"系统在跑但无人请求"的状态都是 bug。**

它同时是：测试方法学（跑 N 步必达静默，否则 = livelock = 治理失败）、节能策略（静默 = `hlt`）、可验证性入口（每条 rewrite rule 的 termination 可单独证明）、故障收敛判据（fault 也必须最终静默——要么传播完成，要么被 Grant 拓扑 firewall 截停）。

## 一、阶段总览

| 阶段 | 主题 | 核心交付 | Killer Demo | 退出判据 |
|---|---|---|---|---|
| **V2.0** ✅ 地基已落 | 立宪 & 地基 | primitive 边代数进 `gos-protocol`（`edge_algebra.rs`）；9 边 → primitive lowering；harness `gos-protocol-harness` | 9 边全部机械分解忠实于 ADR §2.3；6 边往返恒等、`{Signal,Return,Sync}` 形稳定（property test 8/8 绿） | ✅ 零行为变更（内核 `cargo check` 干净 + 治理门禁 OK）；edge-algebra property test 全绿。ADR-001/002/003 批准仍待办 |
| **V2.1** ✅ 完成（待桌面目视） | Cypher = ISA（edge 写路径） | ✅ 真实 `RuntimeDispatcher` 接 runtime edge table（替换 harness `Stub`）+ 原子 `rebind_exclusive_use` + `GraphSnapshot.graph_epoch`；可见性语义 [ADR-004](../doc/ADR-004-mutation-visibility.md)；**k-shell theme 切换已改用原子 `rebind_use`、edge-key 统一为 `"use"`** | theme 切换 **原子**提交（一次 epoch），reader 全程见恰好一条 `Use` 边——harness 已证 | ✅ 原子性 harness 绿（1/1）+ epoch 暴露 + dispatcher 接真表 + k-shell 集成；headless 启动 smoke 干净到 steady-state（modules 22/22、nodes 26 edges 50、无 panic）。⏳ 仅剩**桌面目视**（切 theme 不闪/不崩）；node-create 仍被拒（ADR-005） |
| **V2.2** 🟡 进行中 | Rewrite Engine & Boot-as-fixpoint | [ADR-002](../doc/ADR-002-rewrite-engine.md)（§六 渲染模型 ✅=B 图即场景）：`RewriteEngine`（LHS=MATCH→guard→RHS=mutation）；boot manifest 静态图；`kernel_main` < 300 行；调度 = 边传播；因果深度计替换 2048 hardcap；quiescence（吸收 ADR-003） | 打乱 boot 依赖顺序，系统照常正确启动（序被求解，不是被编码）——✅ 已由 `boot_order` harness 证明（打乱依赖声明仍解出有效序） | ✅ **V2.2a 引擎骨架**（`gos-rewrite`：ready-set 传播 + quiescence + 因果深度计 + livelock）+ ✅ **V2.2b 核心**（`boot::resolve_boot_order`：从 Depend 图拓扑解出 boot 序 + 环检测，harness 证明匹配真实 `kernel_main` 序，7/7 绿）。**剩** V2.2b **接线**（引擎驱动 `kernel_main`、拆硬编码序——改 boot 路径，需逐步启动 smoke）/ V2.2c（调度统一）——需现场/桌面验证 |
| **V2.3** | 响应式 Subscribe & 渲染统一 | `Subscribe` 反向传播索引；`theme.wabi/shoji` 变调色板数据 node（杀掉 `PAL_U32` 常量）；`fbtest.rs` → `k-render`（纯光栅）+ `k-desktop`（场景图构造） | **Demo C**：切 theme 0 行代码扩散；静止画面 0 帧重绘；脏矩形免费 | theme 扩散 0 行；idle = 0 帧；鼠标更新独立于渲染 tick（修掉 PIT→shell heartbeat lag 根因） |
| **V2.4** | 能力即可达性 & 显示 HAL | capability 检查 = Grant 路径图查询；`gos-hal::display` trait（Bochs-VBE backend #1）；跨域调用走 Grant 路径 | 热插拔 node，外部持有的 capability 仍工作；子图 fault 被 Grant 拓扑天然 firewall | 5 个 killer demo 全绿；capability-path / hot-swap / fault-containment test 绿 |
| **V2.5** | Phase I 图形（Vulkan host-bridged） | Vulkan Gen-1 落在 rewrite 基底上（scene 子图 → `k-vk-host`）；F.5（FAT32 write + journal fsync）并入 | **Soul demo**：`MATCH...CREATE` → 下一帧 3D 出现新 node（此时已 trivial——只是一条 Subscribe） | gfx golden frame（lavapipe）+ gfx-fuzz 绿；截图功能依赖的 F.5 落盘可用 |
| **V2.6** | 硬化 & 产品收尾 | 真机显示（UEFI GOP / virtio-gpu）；`persistent` 属性接真实 FS-backed 边；installer 真机验证；fast-path node 性能 pass；`hypervisor` → `gos-graph-engine` 改名 | Boot 自调优：换 CPU、图不变，boot 时长缩短（rewrite engine 找到更宽并发 fire 层） | 全系统测试报告；产品级 V2.0 发布 |

依赖链：**V2.0 → V2.1 → V2.2 → V2.3 → V2.4 → V2.5 → V2.6**，主线串行（每阶地基是下一阶前提）。并行轨见 §三。

## 二、各阶段详述

### Phase V2.0 — 立宪 & 地基（约 2–3 周）

**目标**：把宪法变成代码与机械约束，零行为变更。这是最便宜也最重要的一步——primitive 选错是宪法级代价。

**交付**：
- ADR-001（边代数）批准；ADR-002（Rewrite Engine 语义）、ADR-003（Quiescence 不变式 + 测试方法学）起草批准。
- `gos-protocol`：编码 `EdgeBits{ refer, send, bind, grant }` + `EdgeAttrs{ persistent, exclusive, cardinality, reactive }`。
- 9 条现有边 → primitive 的 lowering 兼容层（命名点保留为构造器，内部 lower 成 primitive）。
- **Harness**：edge-algebra property test —— 往返分解（命名点 → primitive → 命名点恒等）、组合封闭、正交性反例固化。

**退出判据**：现有 runtime 行为逐位不变（diff 仅在表示层）；property test 全绿；治理脚本新增"禁止第 5 primitive"红线。

**风险**：lowering 层引入表示层 bug → 用"9 边行为快照对比"兜底（V2.0 前后 snapshot 必须逐字节相同）。

### Phase V2.1 — Cypher = ISA（约 3–4 周）

**目标**：兑现"Cypher 是 GOS 的机器码"——先通 **edge 写路径**。勘察修正（2026-06-08）：[`gos-cypher-mut`](../crates/gos-cypher-mut/) 的 `MutationDispatcher` trait + `apply_mutation` + `AuditedMutation` **已存在**，且 `gos-runtime` 的 `graph_epoch` 可见性机制**已存在**；真实缺口是**没有任何接真实 edge table 的 impl**（唯一实现是 harness `Stub`）。同时尊重现有约束：mutation 是 **edge-only**（`AddEdge/RemoveEdge/RebindUse`，仅 `Mount/Use`），node create/delete 被有意禁止（保护 claim/quota/NodeId 稳定），**推迟到 ADR-005**，V2.1 不碰。

**交付**：
- 真实 `impl MutationDispatcher`（接 `gos-runtime` 的 `register_edge`/`unregister_edge` + 新增**原子 rebind** 入口），替换 harness `Stub`。约束：不改 `register_edge`/`unregister_edge` 旧单次递增语义（零行为变更回归）。附录 B 的 `CreateNode/DeleteNode/SetProp` 是 V2.2 rewrite-engine 的远期 ISA，非 V2.1 范畴。
- **ADR-004 决定 mutation 可见性语义**：epoch-published（下一 cycle 可见，reader snapshot 隔离）vs immediate。**这个决定反向约束 V2.3 renderer 怎么读图**——必须在此钉死，否则 Phase I 建在沙上。推荐 epoch-published：reader 永远看一致 snapshot，writer 批量提交一个 epoch。
- audit envelope（已存在）接到真实 mutation；fault attribution（哪个 node 发起的 mutation 失败）。

**退出判据**（= [ADR-004 §七](../doc/ADR-004-mutation-visibility.md) 检查单）：`RebindUse` / `AddEdge` / `RemoveEdge` 端到端经真实 dispatcher 写入 edge table；`RebindUse` **原子性** harness 绿（全程不暴露零/双 `Use` 边的可观察 epoch）；`GraphSnapshot` 暴露 `graph_epoch`；audit envelope 与 mutation 同临界区入队；node-create 仍被 `pre_validate` 拒绝。Harness：rebind 原子性 test、epoch 可见性 test、dispatcher-接真实-runtime 的回归（旧 edge API 行为不变）。

**风险**：可见性语义选错会波及全栈——故先于一切 UI 工作完成 ADR-004。

### Phase V2.2 — Rewrite Engine & Boot-as-fixpoint（约 4–6 周）

**目标**：boot 不再是硬编码函数序列（当前 [`main.rs:17-128`](../crates/hypervisor/src/main.rs)），而是在 boot manifest 图上求重写不动点。

**交付**：
- `RewriteEngine`：rule = `LHS pattern`（Cypher MATCH）→ `guard`（fire predicate）→ `RHS`（Cypher mutation 集）。node "fire" ≡ 其 rewrite rule 命中并执行 RHS。
- boot manifest 编译期静态图；`kernel_main` 缩成：`load_boot_manifest → engine.run_to_quiescence → engine.steady_state`，目标 < 300 行。
- 调度 = 边传播：ready set = 有 ≥1 条满足谓词的待处理入向 `Send` 的 node；按 lane-class 标签选一个 fire；fire 产生新 Send/mutation；循环至 ready set 空（quiescence）。
- 因果深度计：替换 [`service_system_cycle` 的 2048 hardcap](../crates/hypervisor/src/main.rs)。跑满不再是静默截断，而是 telemetry 报警"本帧因果链深 N"。

**退出判据**：GDT/IDT/PIC 经 `Depend` 边（= Refer + fire-guard）按 readiness 自然 fire，`main.rs` 无"先 GDT 再 IDT"硬编码序；打乱 manifest 依赖声明顺序系统照常正确启动。Harness：quiescence test（N 步必达静默）、boot-graph 拓扑排序 test、livelock 检测。

**风险**：涌现行为难调试 → 每次 fire 记录触发边（强 telemetry）；quiescence 不变式做兜底断言。

### Phase V2.3 — 响应式 Subscribe & 渲染统一（约 4–6 周）

**目标**：消灭 render loop。当前 [`main.rs:112` 的 `loop { service_cycle(); render_frame(); hlt() }`](../crates/hypervisor/src/main.rs) 把渲染绑死在内核 tick 上，且 [`fbtest.rs`](../crates/hypervisor/src/fbtest.rs) 1764 行命令式 UI 内嵌在 hypervisor crate——违背"最小引导"边界。

**交付**：
- `Subscribe` 边（= Refer + reactive 属性 + region 谓词）+ engine 反向传播索引：任何 mutation 触及 node X，engine 沿反向 reactive 边向订阅者发 `Send(repaint, region)`。
- `theme.wabi / theme.shoji` 从硬编码 [`PAL_U32` 常量](../crates/hypervisor/src/fbtest.rs)变成持有调色板数据的 node；render node 经图遍历 `theme.current -[Use]-> 调色板` 读色，不再读常量。
- `fbtest.rs` 拆分：`k-render`（纯 rasterizer，无 UI 逻辑）+ `k-desktop`（scene 子图构造）。`hypervisor` 不再直调渲染——渲染是一个 Subscribe 到 `scene_root` 的 node。

**退出判据 / Demo C**（逆向详见附录 C）：切 theme 0 行代码扩散；mutation 只激活受影响 region 内的 render node（脏矩形免费）；静止画面 0 帧（`hlt` 真省电）；鼠标走自己的 Subscribe 路径、不抢 ring 0 时间（根除 `fix(desktop): throttle PIT→shell heartbeat` 修的那个 mouse lag）。Harness：golden frame（lavapipe）、repaint-region test、idle-frame-count test。

**关键涌现证据**：theme 扩散与脏矩形渲染共用**同一个** Subscribe + 反向传播机制。一个机制喂出两个特性——这就是"不是 ad hoc"的证明。

### Phase V2.4 — 能力即可达性 & 显示 HAL（约 3–4 周）

**交付**：
- capability 检查 = Grant 路径图查询（正式化 ADR-001 §五）：跨域调用前 engine 验证 `from -[Grant*]-> target` 可达且终点暴露该 cap。claim/revoke = Grant 边 create/delete。
- `gos-hal::display` trait：`init(prefer: ResolutionHint) -> Surface` / `surface.lfb()` / `surface.flip()`。Bochs-VBE 为 backend #1（封装当前 [`fbtest.rs:296` 的 DISPI 直写](../crates/hypervisor/src/fbtest.rs)），为 virtio-gpu / UEFI GOP 留口。
- 跨域调用走 Grant 路径（接 Phase B.4.5 已有的 cross-domain capability invocation 结构）。

**退出判据**：5 个 killer demo 全绿（见 §四）。Harness：capability-path test、hot-swap test、fault-containment test。

### Phase V2.5 — Phase I 图形（Vulkan host-bridged）（约 6–8 周）

**目标**：原 Phase I Vulkan Gen-1 现在**落在稳固的 rewrite 基底上**，而非建在沙上。host-bridged（像 `k-cuda-host`），非 bare-metal Vulkan。

**交付**：scene 子图 → `k-vk-host` 桥接 host Vulkan；F.5（FAT32 write + journal fsync）并入（截图功能依赖落盘）。

**退出判据 / Soul demo**：`MATCH ... CREATE (n)` → 下一帧 3D 显示新 node。**此时已 trivial**——CREATE 改 scene 子图 → Subscribe 反向传播 → render node 激活 → 下一帧出节点。不写一行 demo 专用代码。Harness：gfx golden frame、gfx-fuzz。

### Phase V2.6 — 硬化 & 产品收尾（约 4–6 周）

**交付**：真机显示（UEFI GOP / virtio-gpu backend）；`persistent` 属性接真实 FS-backed 边（接 `gos-vfs` / journal）；installer 真机验证（已有 `tools/build-installer.ps1` 流程）；fast-path node（rasterizer / DMA 仍走快路径——只要快路径也是图里一个挂 fast-path 标签的 node）；`.gitignore` 收拾根目录 20+ 个 `log*.txt`；`hypervisor` → `gos-graph-engine` 改名（不只是改名，是 identity shift——它已缩成 rewrite engine 本身）。

**退出判据 / 终极 Demo**：换 CPU、图不变，boot 时长缩短（rewrite engine 找到更宽的并发 fire 层——没有性能调优 PR，性能来自图算法）。全系统测试报告；产品级 V2.0 发布。

## 三、并行轨

主线串行，但以下可与主线并行推进，互不阻塞：

- **测试基建轨**（贯穿全程）：lavapipe golden frame、gfx-harness、gfx-fuzz、quiescence harness。每阶强制随附，不单列阶段。
- **F.5 持久化轨**：FAT32 write + journal fsync 可在 V2.1 后任意时点推进，**V2.5 前必须并入**（截图依赖）。不阻塞主线 UI。
- **文档轨**：ADR 持续产出；`GOS_ARCH_v2.md` 在 V2.2 / V2.3 后各更新一次以反映新身份。

## 四、五个 Killer Demo（涌现的验收标准）

口号便宜，证据贵。每个 demo 都是"系统做了一件没人显式编码的事"。**这五个不全绿之前，涌现就是 PPT。**

| # | Demo | 落地阶段 | 证明的不变式 |
|---|---|---|---|
| 1 | **Theme 0 行扩散**：改 `theme.current -[Use]->` target，所有路径上含 theme.current 的 node 自动重渲。无 broadcast、无 observer、无 `redraw_all()`。 | V2.3 | Subscribe + 反向传播 足够替代显式发布订阅 |
| 2 | **最小重绘 0 行**：mutation 只激活受影响 region 内 render node。无脏矩形表。 | V2.3 | region 谓词使重绘自动微分 |
| 3 | **热插拔 0 行**：销毁 node、换新版本替换，外部持有的 capability 仍工作。无 handoff 代码。 | V2.4 | NodeId/VectorAddress 分离 + Grant 路径稳定性 |
| 4 | **故障隔离 0 行**：子图 fault 沿 Bind 边传播至子图内、被 Grant 边天然 firewall 在子图外。 | V2.4 | Bind ≠ Grant 的拓扑隔离 |
| 5 | **Boot 自调优**：换 CPU、图不变，boot 时长缩短（更宽并发 fire 层）。 | V2.6 | 性能来自图算法，无调优 PR |

## 五、风险登记册

| 风险 | 严重度 | 缓解 |
|---|---|---|
| **走偏成"图数据库 + 调度器"** | 致命 | 钥匙是宪法（ADR-001）+ 5 个 killer demo。立宪 + demo 通了才算真涌现。每阶评审强制问"这阶有没有 demo 证明 0 行涌现"。 |
| Rewrite engine 自身 6–12 月工程量 | 高 | 比"先做 Phase I 再回头重做"便宜。V2.0–V2.2 是不可压缩的地基投资。 |
| primitive 选错（宪法级代价） | 高 | ADR-001 §三完备性/正交性证明逐条对抗性评审后方批准。 |
| 涌现行为难调试 | 中 | quiescence 不变式 + 每次 fire 记录触发边的强 telemetry。Erlang/actor 社区已解过同类问题。 |
| 性能上限低于命令式 | 中 | GOS 重可表达性 > 极限性能。真热路径（rasterizer/DMA）走 fast-path 标签节点。 |
| mutation 可见性语义选错 | 中 | ADR-004 先于一切 UI 工作完成；推荐 epoch-published + snapshot isolation。 |
| 招人难 | 低 | 反转为 hiring filter——能理解这套设计的人正是要找的人。 |

## 六、sequencing 铁律

1. **V2.1（写路径）通之前不推 V2.5（Phase I）**。Phase I 的 trait shape 取决于 V2.1 定的可见性语义。
2. **每阶必带 harness 与一个 killer demo 一同合入**。无 harness = 不合入；无 demo = 这阶没证明涌现。
3. **ADR 先于实现**。V2.0 批 ADR-001/002/003，V2.1 批 ADR-004。没有 ADR 的宪法级决定不得进代码。
4. **不在 Gen-1 scope 内提 bare-metal Vulkan / virtio-gpu**（继承项目记忆约束）；它们是 V2.6+ 的事。
5. **F.5 不阻塞主线 UI，但 V2.5 前必须并入**。

---

## 附录 B — Cypher-as-ISA trait 草图（V2.1 落地形状）

把"每个 syscall / IPC / 中断进入 / capability 调用 = 一条 Cypher mutation"具象化。这是 V2.1 的设计输入，批准后毕业为 ADR-002。

```rust
// ── ISA：一切状态变更都是 mutation ───────────────────────────────
// 对应 ADR-001 的 primitive 边代数：CreateEdge 携带 EdgeBits + EdgeAttrs。
pub enum Mutation {
    CreateNode { kind: NodeKind, props: Props, vector: VectorHint },
    DeleteNode { id: NodeId },
    SetProp    { id: NodeId, key: PropKey, val: PropVal },
    CreateEdge { from: NodeId, to: NodeId, bits: EdgeBits, attrs: EdgeAttrs },
    DeleteEdge { id: EdgeId },
}

// ── rewrite rule = match + guard + emit；这就是一个 node 的"代码" ──
// LHS 是 Cypher MATCH，RHS 是 Cypher CREATE/DELETE/SET。
pub trait RewriteRule {
    fn lhs(&self) -> Pattern;                    // Cypher MATCH 模式
    fn guard(&self, m: &Match) -> bool;          // fire 谓词（Depend 的依赖语义住这里）
    fn rhs(&self, m: &Match) -> Vec<Mutation>;   // fire 时发出的 mutation
}

// ── dispatcher：带可见性 + 审计 + 故障归因地应用 mutation ─────────
// ADR-004 决定 apply 是 epoch-published（推荐）还是 immediate。
pub trait MutationDispatcher {
    /// 批量提交一个 epoch；返回新 epoch 或带故障归因的拒绝。
    fn apply(&mut self, batch: MutationBatch, by: NodeId) -> Result<Epoch, MutFault>;
    /// reader 永远看一致 snapshot（snapshot isolation）。
    fn snapshot(&self, at: Epoch) -> GraphSnapshot;
}
```

**"Cypher 是 ISA"的具象**：
- PS/2 IRQ → `CreateNode{kind: InputEvent, ...}` + `CreateEdge{ from: ps2, to: central_irq, bits: Send }`。中断进入 = 一条 mutation。
- syscall → 一个 `MutationBatch`（如 `CreateNode(request)` + `CreateEdge(Send to handler)`）。
- 切 theme → `DeleteEdge(旧 Use)` + `CreateEdge(新 Use)`（见附录 C）。
- 跨域 cap 调用 → engine 先验 Grant 路径可达，再 `CreateEdge{ bits: Send|Grant }`。

quiescence ≡ 无 node 的 RewriteRule guard 被满足 ≡ 无待处理 Send。

## 附录 C — Demo C（Theme 0 行扩散）逆向

证明"切 theme 0 行代码"需要什么前置，以及它如何复用通用 Subscribe 机制。

**当前态**：`theme.current -[Use]-> theme.wabi`（vector 6.1.3.0 → 6.1.1.0），`Use` = Refer+Bind+Grant，exclusive。色值硬编码在 [`fbtest.rs` 的 `const PAL_U32`](../crates/hypervisor/src/fbtest.rs)。

**目标动作**：执行一条 mutation 即让所有受 theme 影响的 node 重渲，零 theme 专用代码。

**传播 trace**：
1. render node 持 `(render)-[Subscribe{reactive}]->(theme.current)`。
2. mutation（一条）：`MATCH (c:theme.current)-[u:Use]->(:theme.wabi) DELETE u CREATE (c)-[:Use{exclusive}]->(shoji)`。
3. 该 mutation 触及 `theme.current`（其出边变了）。
4. engine 反向 reactive 索引：谁 Subscribe 了 theme.current？→ 所有 render node。→ 向各自发 `Send(repaint)`。
5. render node fire，重新经图遍历读 `theme.current -[Use]-> shoji` 的调色板，按新色渲染。

**零 theme 专用代码**——传播是通用 Subscribe 机制，与脏矩形渲染（Demo #2）是**同一机制**。

**前置（即 V2.3 必须先做的具体重构）**：
- V2.1：`DELETE u / CREATE edge` 的 mutation 必须真生效（`MutationDispatcher`）。
- V2.3：`Subscribe` 边 + engine 反向传播索引。
- V2.3：**杀掉 `PAL_U32` 常量**——`theme.wabi/shoji` 变持调色板数据的 node，render node 经图遍历读色而非读常量。这是 Demo C 暴露出的第一个落地重构，也是整条路的起点。
