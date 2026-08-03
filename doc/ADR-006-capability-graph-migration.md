# ADR-006：capability 检查从 `gos-supervisor` 表查询迁向 Grant 图查询

> 状态：**已选向：选项 A（影子验证层）已落地** · 日期：2026-06-11 · 选向/落地日期：2026-08-03 · 配套：[ADR-001 §五](./ADR-001-edge-algebra-constitution.md)（"capability 即图可达性"，明文"写明以正式化 Phase V2.4"）、[ADR-005](./ADR-005-node-mutation.md)（claim/quota 与 NodeId 稳定性的同源冲突）、[V2 计划 V2.4](../plan/V2_DEVELOPMENT_PLAN.md)
>
> 口径：V2.4a/b 已经在 `gos_mutation_dispatch::capability` 落地了 ADR-001 §五 的查询原语——`reachable_via_grant`（抽象图）+ `grant_edges_from_specs`/`capability_check`（接真实 `gos_protocol::EdgeSpec`，按 `edge_type.lower().bits.grant` 过滤 `Use`/`Call`）。本 ADR 处理 V2.4c 被推迟的硬问题——**这个查询原语如何接入 `gos-supervisor` 真实的 capability/claim 路径**（V2.4 遗留："尚未接入真实 capability/trap 检查路径"）。本 ADR 只陈述问题与选项，**不替你拍板**。

## 一、冲突

ADR-001 §五（[lib.rs:88-94](./ADR-001-edge-algebra-constitution.md)）的推论是：

> "node A 能否调用 node B 的能力 C？" ≡ "在允许的边类型上是否存在 A→B 的 **Grant 路径**……claim / revoke 退化为 Grant 边的 create / delete。"

但 `gos-supervisor`（[lib.rs](../crates/gos-supervisor/src/lib.rs)）当前的 capability/claim 实现是一套独立的、手写的 O(1) 表系统，与 `gos_protocol::EdgeSpec`/`RuntimeEdgeType` **没有任何关联**：

- `ClaimRecord`（[lib.rs:454-478](../crates/gos-supervisor/src/lib.rs)）按 `ClaimId`/`ResourceId`/`NodeInstanceId`/`ModuleHandle` 索引，定容 `MAX_CLAIMS=128`；`HeapGrantRecord` 同理 `MAX_HEAP_GRANTS=256`。这些 ID 类型与 `gos_protocol::NodeId([u8;16])` 之间**无既定映射**。
- `resolve_capability`/`claim_resource`/`release_claim_internal`/`revoke_capabilities`（线性扫描定容数组）是真实内核 syscall ABI（`abi_resolve_capability`/`abi_claim_resource`/`abi_release_claim`，[lib.rs:2094-2230](../crates/gos-supervisor/src/lib.rs)）背后的实现——这是热路径，每次 trap 都可能调用。
- `gos_mutation_dispatch::capability` 的 `MAX_CAPABILITY_NODES=32` 比 `MAX_CLAIMS=128`/`MAX_HEAP_GRANTS=256` 小一个数量级；`reachable_via_grant` 是 BFS（O(V+E)），而 `resolve_capability` 当前是表查找。

V2.4 的 exit criteria（"capability-path / hot-swap / fault-containment test 绿"）字面上要求**真实路径**绿灯，但直接把 `capability_check` 塞进 `abi_resolve_capability` 意味着：syscall 热路径从 O(1)/O(n) 表查找变成 O(V+E) BFS，且 `ClaimRecord` 携带的 `claim_policy`/`preempt_policy`/`epoch`（lease 语义，Phase B 不变式）在纯 Grant-edge 模型里没有自然对应——这与 ADR-005 未决的"claim/quota 挂在哪种 NodeId 上"问题同源耦合。

## 二、选项

### 选项 A —— 影子验证层（Shadow-verification，我倾向的方向）
`capability_check`/`grant_edges_from_specs` **不进入 syscall 热路径**，而是作为 host-harness / 治理时的**等价性证明**：对一组合成的（或未来：从真实 boot 后 `ClaimRecord` 表派生的）`EdgeSpec` Grant 边表，断言"`capability_check(specs, A, B) == true` 当且仅当存在对应的活跃 `ClaimRecord`/`CapabilityToken` 授权 A 访问 B 暴露的能力"。

- **优点**：零 ABI/性能风险；`abi_*` 热路径不变；可立即用 harness 证明 ADR-001 §五"claim ≡ Grant 边"的等价性断言（mirrors V2.2a "`gos-mutation-dispatch::Engine` coexists with `service_system_cycle`"、V2.3c 的行为等价证明手法）；为后续真正替换提供回归网。
- **代价**：不是"运行时"授权检查——`capability_check` 仍不参与实际 trap 决策，V2.4 exit 的"capability-path test 绿"只能算**等价性**达成，非**热路径**达成。

### 选项 B —— 容量提升 + 直接替换热路径
把 `gos_mutation_dispatch::capability` 的容量常数提到 `MAX_CLAIMS`/`MAX_HEAP_GRANTS` 量级（128/256+），让 `resolve_capability`/`claim_resource` 的核心判定直接调用 `capability_check`，`CapabilityToken`/`ClaimRecord` 退化为缓存/索引层。

- **优点**：字面实现 ADR-001 §五（"claim/revoke = Grant 边 create/delete"）；V2.4 exit criteria 字面达成。
- **代价**：每次 syscall 一次 O(V+E) BFS（128+ 节点）——kernel 热路径性能特征未知/未测；`ClaimRecord` 的 `claim_policy`/`preempt_policy`/`epoch`（lease 语义）在纯 Grant-edge 图里无自然字段，迁移面巨大；与 ADR-005 未决的 node/claim 模型问题强耦合（先有 ADR-005 的 NodeId 稳定性结论，才能谈 claim 记录如何挂到 Grant 边上）。

### 选项 C —— 重新措辞 V2.4 exit criteria
承认"capability 检查 = Grant 路径图查询"在 V2.4a/b 已经在 `gos_mutation_dispatch` 层"成为可能"（查询原语存在，对真实 `EdgeSpec` 数据可用，host-harness 25/25 绿），但"接入 `gos-supervisor` 真实 syscall 路径"重新归类为 V2.6（硬化）范畴——因为它牵涉 ADR-005 未决问题，性质上更接近"产品收尾时的性能/迁移 pass"而非"V2.4 的图查询语义证明"。

- **优点**：诚实反映 V2.4a/b 已完成的范围（语义原语 + 真实数据桥接）；不在 ADR-005 未决前做大手术。
- **代价**：需要修改 V2 计划文档对 V2.4 exit criteria 的措辞；可能被视为"重新定义范围以宣称完成"，需要在计划文档里写清楚理由（而非悄悄改）。

## 三、建议与门禁

倾向 **A（影子验证层）**作为 V2.4c 立即可做、不依赖 ADR-005 决议、不改 ABI、有 harness 可证的部分；**A 与 C 不互斥**——做完 A 之后，V2.4 exit criteria 的"capability-path test 绿"可如实标注为"等价性证明绿，热路径替换是 V2.6 范畴"（即 C 的措辞调整建立在 A 的证明之上）。**B（直接替换热路径）依赖**：

1. ADR-005 先选向（claim 记录与 NodeId/provisional-node 模型的关系）。
2. `gos_mutation_dispatch::capability` 在 128+ 节点规模下的 BFS 性能特征评估（kernel 热路径，目前完全未测）。

本 ADR 范围**不含** V2.4 其余两项遗留（`gos-hal::display` trait、跨域调用 + 剩余 4 个 killer demo）——它们是不同性质的设计问题（硬件驱动 / IPC 机制），需要各自独立的 ADR，不与本 ADR 的"capability 表→图迁移"问题共享决策依据。

列为待你选向的 backlog 决定，**选项 A 不阻塞主线**：一旦确认，可在 `gos-mutation-dispatch-harness` 或新增 `gos-supervisor-harness` 等价性 property test 中立即实现，无需先等 ADR-005。

**落地状态**：`gos-mutation-dispatch-harness/tests/capability_specs.rs` 早先已有 3 条测试（`claim_is_grant_edge_create_revoke_is_delete` 等）用手写 `EdgeSpec` 恢复 ADR-001 §5 的字面陈述，但从未接触真实 `gos-supervisor::ClaimRecord`——只是 `capability_check` 的自洽性证明，不是本 ADR 要求的跨系统等价性证明。本次补上真正的部分：`gos-supervisor-harness/tests/supervisor.rs` 新增 `capability_check_agrees_with_real_claim_resource_lifecycle`，驱动真实 `claim_resource`/`release_claim` 调用，同步维护一份手工映射的 Grant 边表，断言 `capability_check` 在 claim 建立前/建立后/释放后三个真实状态点都与预期一致。`abi_resolve_capability`/`claim_resource` 本身的实现未被触碰——仍是治理时等价性证明，非热路径替换。
