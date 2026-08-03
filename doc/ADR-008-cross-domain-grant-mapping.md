# ADR-008：`gos_protocol::NodeId`（V2 图身份）↔ 运行时身份（`VectorAddress` / B.4.5 跨域调用）的映射——V2.4 deliverable 3 的前置问题

> 状态：**已选向：选项 B（`node_id_for_vector` 派生函数）已落地；选项 A 仍待 B.4.6** · 日期：2026-06-11 · 选向/落地日期：2026-08-03 · 配套：[V2 计划 V2.4](../plan/V2_DEVELOPMENT_PLAN.md)（deliverable 3"跨域调用走 Grant 路径，接 Phase B.4.5 已有的 cross-domain capability invocation 结构"）、[ADR-005](./ADR-005-node-mutation.md)（NodeId 稳定性）、[ADR-006](./ADR-006-capability-graph-migration.md)（`gos-supervisor` capability 表↔Grant 图同源问题）、[Phase B.4 §B.4.5](./PHASE_B4_DOMAIN_ISOLATION.md)（cross-domain capability invocation 现状）
>
> 口径：V2.4a/b/c 已经把 `capability_check(specs: &[gos_protocol::EdgeSpec], from: NodeId, to: NodeId) -> bool` 落地为可独立验证的纯函数（`gos-mutation-dispatch-harness` 28/28 绿）。V2.4 deliverable 3 要求"跨域调用走 Grant 路径"——即 B.4.5 的跨域 dispatch 在放行前应当问 `capability_check`。但 B.4.5 的 `route_signal(target: VectorAddress, ...)` 用的是 `VectorAddress`（48-bit `l4/l3/l2/offset`，pre-V2 寻址），不是 `gos_protocol::NodeId([u8;16])`——这是 ADR-006 在 `gos-supervisor` capability 表上发现的**同一类**"V2 图身份 vs. pre-V2 运行时身份，无既定映射"问题，但发生在路由层而非 claim 表层。本 ADR 处理这个映射缺口，**不替你拍板**。

## 一、冲突

### 1.1 两套身份，无既定映射

- `gos_protocol::NodeId([u8; 16])`（[lib.rs:472](../crates/gos-protocol/src/lib.rs)）—— V2/ADR-001 图身份，`EdgeSpec`/`edge_algebra`/`gos_mutation_dispatch::capability` 全部用它。
- `gos_protocol::VectorAddress { l4: u8, l3: u16, l2: u16, offset: u16 }`（[lib.rs:71-109](../crates/gos-protocol/src/lib.rs)，48-bit）—— pre-V2 "canonical vector address decomposed into graph coordinates"，每个 builtin 模块都有一个 `pub const NODE_VEC: VectorAddress`（如 [`k_vga::NODE_VEC`](../crates/k-vga/src/lib.rs)、[`k_vk_host::NODE_VEC`](../crates/k-vk-host/src/lib.rs)），`route_signal(target: VectorAddress, signal: Signal)` 用它寻址。

仓库内对 `From<VectorAddress>`/`From<NodeId>`/`NodeId::from`/`VectorAddress::from` 的 impl 搜索零命中——两者之间没有任何转换。

（旁注：`gos_mutation_dispatch::boot::BootNodeId(u32)` 与 `gos_mutation_dispatch::NodeId(u32)`（`capability.rs`）是另外两个 `u32` 小整数索引，但它们都是**有意不透明**的局部索引——只在各自函数的 `&[T]` 调用方提供的表内有意义（"Opaque boot-step identity"），不是全局身份，**不属于**本 ADR 要解决的映射缺口。)

### 1.2 B.4.5 跨域调用的确切位置

[Phase B.4 §B.4.5](./PHASE_B4_DOMAIN_ISOLATION.md)（已完成，2026-04-26）：

> 插件 A 的 `kernel_emit_signal` → `route_signal(target=B)` → B 的 `on_event` 被一个 enter(B 的 instance) 包裹；返回时 leave 还原回 A 的 CR3 ... 嵌套调用通过 token 链式保存：A→B→C 的 enter 调用产生独立 token

这就是"跨域调用"在代码里发生的精确位置——`route_signal` 的入口，target 是 `VectorAddress`。

### 1.3 即使有了映射，B.4.5 今天对所有调用一视同仁

同一节明确写道："**当前所有 builtin 共享 kernel CR3，所以"切换"是 no-op**，但 enter/leave 计数仍每次发生 ... 完整端到端验证 ... 需要至少两个外部 ELF 模块；待 B.4.6 ELF loader 落地后跑"。

也就是说：**今天 B.4.5 允许的跨域调用集合 = 全集**（任意 builtin 可调用任意 builtin，trampoline 只是计数，不拒绝）。"selective" 的跨域调用（某些调用被 Grant 拓扑拒绝）要到 B.4.6 ELF 模块给出真正不同的 `domain.root_table_phys` 之后才会**第一次产生有意义的"允许/拒绝"集合**。

## 二、选项

### 选项 A —— 等价性证明（mirrors [ADR-006](./ADR-006-capability-graph-migration.md) 选项 A），但**依赖 B.4.6**

写 host-harness property test：对 boot manifest 中已知的模块对，手写 `EdgeSpec`（`Call`/`Use`）构造 Grant 图，证明 `capability_check` 的结果与"B.4.5 当前允许该跨域调用"一致。

- **前置条件未满足**：按 §1.3，"B.4.5 当前允许的调用集合"今天是**全集**——`capability_check` 与"全集"的等价性证明要么 vacuous（任何非平凡 Grant 图都会"不等价于全集"，因为 `capability_check` 设计上就是选择性的），要么需要构造一个同样是全连通的 `EdgeSpec` 图（没有信息量）。**这个等价性证明只有在 B.4.6 落地、B.4.5 第一次产生非平凡的允许/拒绝集合之后才有意义**——这是一个值得记录的依赖，类似 ADR-006 选项 B 依赖 ADR-005。
- **优点**（一旦 B.4.6 落地后）：复用 V2.4c 手法，零运行时改动，立即可做。
- **代价**：今天做不了——不是设计选择被推迟，而是被验证对象（B.4.5 的非平凡行为）还不存在。

### 选项 B —— 派生 `NodeId([u8;16]) ⇄ VectorAddress` 的纯函数映射，先证明映射本身正确（不接 `route_signal`）

为每个 boot-manifest 模块的 `VectorAddress`（如 `k_vga::NODE_VEC`）确定性地派生一个 `gos_protocol::NodeId([u8;16])`——例如 `node_id_for_vector(v: VectorAddress) -> NodeId`：16 字节 = 8 字节固定前缀 + `v.as_u64()` 的 8 字节大端编码，纯函数、`no_std`、零存储。新增到 `gos_mutation_dispatch::capability`，配 harness property test（双射性：不同 `VectorAddress` → 不同 `NodeId`；往返一致）。**不**改 `route_signal`。

- **优点**：给"如何用 `NodeId`/`EdgeSpec` 指代一个现有运行时模块"一个具体、可计算的答案——为选项 A（一旦 B.4.6 落地）、ADR-006 的等价性证明、以及任何想给现有 22 个 builtin 写 `EdgeSpec` 的 harness 测试，提供"不是手写占位 `NodeId([1u8;16])`"的真实推导规则；纯函数 + harness 证明，复用 V2.4a→b"先证明派生正确"模式，不依赖 B.4.6，**立即可做**。
- **代价**：编码方案（"8 字节前缀 + `as_u64()` 大端"）本身是一个**新的、未经审视的设计决定**——若 ADR-005 后续确定 `NodeId` 应有不同的全局编码规则（例如基于内容寻址的哈希、或运行时分配而非派生），这个派生函数可能要废弃重做。只回答"如何指代"，不回答"指代之后 `capability_check` 的结果该不该影响 `route_signal`"（仍是选项 A 的范围，仍依赖 B.4.6）。

### 选项 C —— 推迟，记录依赖（不展开新方案）

确认 V2.4 deliverable 3 与 ADR-005/006 共享同一个未解决的底层问题——"V2 图身份与运行时身份的关系"——但不提出具体映射方案。三者的选向应放在一起考虑（例如 ADR-005 选向时一并评估对 ADR-006/008 的影响），避免分别推进互相冲突的映射方案。

## 三、建议与门禁

V2.4 deliverable 3 的核心障碍**不是一个尚待选择的设计方案，而是一个尚未满足的前置条件**：B.4.5 今天对所有 builtin 调用一视同仁（§1.3），"`capability_check` 与 B.4.5 当前行为等价"在全集下无信息量。**选项 A 要等 B.4.6（ELF 模块、真实 per-domain CR3）落地、B.4.5 第一次产生非平凡允许/拒绝集合之后才有意义**——建议诚实记录这个依赖，而不是为了"V2.4 也交付点什么"而造一个 vacuous test。

**选项 B（`node_id_for_vector` 派生函数）不依赖 B.4.6**，可以现在做——但其编码方案是一个新设计决定，按"ADR before implementation"铁律，应该是本 ADR 选向的一部分，不应在选向前固化。

**本阶段（V2.4）建议措辞**：deliverable 3"跨域调用走 Grant 路径"的"等价性证明"前置条件（B.4.6 非平凡 cross-domain 调用集合）尚未满足，相关 harness 证明推迟到 B.4.6 之后；`gos-mutation-dispatch::capability`（V2.4a/b/c）提供的查询原语本身已经是这一证明的"可用半成品"——一旦 B.4.6 落地，缺的只是选项 A 的等价性测试本身（小工作量）。

本 ADR 范围**不含**已由 [ADR-007](./ADR-007-display-hal-scope.md) 处理的显示 HAL 问题，也不含 [ADR-006](./ADR-006-capability-graph-migration.md) 已经处理的 `gos-supervisor` claim 表问题（本 ADR 是其在路由层的对应物，但两者可独立选向）。
