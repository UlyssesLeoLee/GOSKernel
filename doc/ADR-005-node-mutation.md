# ADR-005：节点创建/销毁 vs claim/quota 稳定性

> 状态：**已选向：选项 A（provisional nodes）** · 提案日期：2026-06-08 · 选向日期：2026-06-12 · 配套：[ADR-004 §三](./ADR-004-mutation-visibility.md)（node-create 推迟到此）· [V2 计划](../plan/V2_DEVELOPMENT_PLAN.md)
>
> 口径：V2.1 把 mutation 锁在 edge-only。本 ADR 处理被推迟的硬问题——**Cypher 能否创建/销毁 node**。这是 soul demo（`MATCH...CREATE` 出新 3D node）和涌现愿景的前提，但与 Phase B 的实例模型冲突。本 ADR 只陈述问题与选项，**不替你拍板**。

## 一、冲突

`gos-cypher-mut` 现在硬拒 node create/delete，理由写在代码里（[lib.rs:18-21](../crates/gos-cypher-mut/src/lib.rs)）：

> "允许 Cypher 凭空创建或销毁 node 会让下游每一个 claim 和 restart_generation 计数失效"——Phase B 的 instance binding / HeapQuota / fault attribution 全挂在**稳定 `NodeId`** 上。

但涌现愿景要求 `CREATE (n)` 能真造出 node（否则 graph 不能生长）。矛盾在于：**Phase B 的 node 是"有 claim、有 quota、有 instance 生命周期"的重实体；而涌现需要的是"能随手创建的轻图元"。**

## 二、选项

### 选项 A —— Provisional nodes（临时节点，我倾向的方向）
Cypher 创建的 node 是 **provisional** 的：可见、可连边、可渲染，但**不能持有 claim / quota / instance**，直到被显式 `promote` 成正式模块 node。
- **优点**：图能自由生长（soul demo 通），Phase B 不变式不被破坏（provisional node 进不了 claim/quota 表）。两类 node 共存，按能力分层。
- **代价**：需要 node 生命周期的二级状态（provisional → promoted）；runtime 要区分两类 node 的能力门。

### 选项 B —— 双命名空间
正式模块 node（Phase B 拥有，NodeId 由 plugin manifest 派生、稳定）与用户/Cypher node（独立 NodeId 空间，无 claim 资格）物理隔离。
- **优点**：隔离最干净，互不污染。
- **代价**：两套 NodeId 空间增加复杂度；跨空间连边的语义要额外定义。

### 选项 C —— 永不允许 node mutation（维持现状）
一切表达为 edge mutation；node 集合在 boot 时固定。
- **优点**：零风险，Phase B 完全不受影响。
- **代价**：图不能生长 → 涌现愿景和 soul demo 落空。**与 GOS 长期方向冲突**。

## 三、建议与门禁

倾向 **A（provisional nodes）**——它在"图自由生长"与"Phase B 不变式"之间取得平衡，且与 ADR-001 的 node/edge 一等公民模型自洽。但这需要先确认：
1. provisional node 的渲染策略（接 ADR-002 §六 的渲染模型决定）。
2. promote 的触发者与权限（capability = Grant 路径，接 ADR-001 §五）。

**本 ADR 不在 V2.1/V2.2 范畴**——它在 soul demo（V2.5）前必须定。列为待你选向的 backlog 决定，不阻塞当前主线。

## 四、2026-06-11 状态更新：§三两项前置确认现已就绪（V2.5a，影子验证）

本 ADR 写于 2026-06-08，彼时 V2.2/V2.3/V2.4 均未落地。§三 倾向 A 但列了两项"需要先确认"——现状如下：

1. **"provisional node 的渲染策略，接 ADR-002 §六的渲染模型决定"**——ADR-002 §六已批准 B「图即场景」，V2.3c 落地的 `propagate_with`（[`gos_rewrite::reactive`](../crates/gos-rewrite/src/reactive.rs)）就是"renderer 是 graph subscription 的纯函数"的具体实现：它只看 `Subscription{target,subscriber,region}` 表，不查询、也无法查询任何"是否已 promote"标志。**渲染策略对 provisional/promoted 一视同仁，因为 B 选型本身就让渲染对 promote 状态不可知**——这不是新设计，是 B 已经蕴含的推论。

2. **"promote 的触发者与权限，接 ADR-001 §五"**——V2.4b/c 落地的 `capability_check`/`grant_edges_from_specs`（[`gos_rewrite::capability`](../crates/gos-rewrite/src/capability.rs)）+ V2.4c 的"claim/revoke 退化为 Grant 边的 create/delete"（参见 [ADR-006](./ADR-006-capability-graph-migration.md)）已把"谁能把一条 Grant 边接到 X"这件事完全机制化。**promote(provisional_node) 可以是同一机制的特例：promoter 对 provisional_node 新增一条 `Call`/`Use` 边，`capability_check` 从 `false` 变 `true`，"是否已 promote"无需独立状态位——它就是"capability_check(claim_authority, node) 是否为真"本身（与 V2.4c 的 claim≡边 完全同形）。

新增 host harness（`gos-rewrite-harness/tests/provisional_render.rs`，2 条 property test，30/30 全绿）把以上两点机械证明出来：一个在 `capability_check` 下不可达（"未 promote"）的 `gos_protocol::NodeId`，其对应的 render `Subscription` 照常 `propagate_with`；而"promote"——给它新增一条 Grant 边——除了让 `capability_check` 变真之外，不触碰渲染表分毫。**两个前置确认现已是已验证事实，而非待证假设**——但这不等于"选向 A"：上述事实对 B/C 同样成立（B/C 下 provisional 节点压根不会以这种"未 promote"形态出现，第 1/2 点自动真空满足）。**A/B/C 仍待你选向**；本更新只是把选 A 的"实现成本"从"未知"降到"零新原语，CREATE 接线本身才是新工作"。

## 五、2026-06-12 决定：选项 A（provisional nodes）

选定 **A**。理由：§四确认的两项前置（渲染策略、promote 权限）已是既有 API 的零成本推论，是三个选项中实现成本最低、且唯一不与"图自由生长"的涌现愿景冲突的选项（C 在 §二已自承"与 GOS 长期方向冲突"；B 的双命名空间在没有具体跨命名空间需求时是过早的复杂度）。

**后续工作**（V2.5d+，遵循"先证明纯原语，再接线"模式，逐步执行，每步独立验证）：

1. 给 `gos_runtime` 的 node 记录加上 provisional/promoted 区分（最小形式：复用既有 `NodeBinding::Unbound` 状态作为"provisional"标记，还是需要新枚举值——取决于 boot 注册的 22 个 builtin 当前是否已经全部 `Bound`，需先读 `gos-runtime/src/lib.rs` 的 `register_node` 调用点确认）。
2. `gos-cypher-mut` 新增 `CreateNode` mutation 变体：识别 `CREATE (n:Label {props})` 中未被 `MATCH` 绑定的 node pattern，调用 `gos_runtime::register_node` 注册一个 provisional 节点（`graph_epoch` 自动 bump，V2.5c 已确认 `vk_auto_refresh` 自动捕获，零新 k-vk-host 代码）。
3. "promote" 机制：给 provisional node 新增一条 Grant（`Call`/`Use`）边即视为 promoted（`gos_rewrite::capability::capability_check` 由假变真）——是否需要把 `capability_check` 接入 `gos-supervisor` 的实时 claim 路径（ADR-006 选项 B，此前因"依赖 ADR-005 先选向"而推迟）现在可以重新评估，但建议作为独立步骤，不与 CreateNode 初版耦合。
4. Soul demo 收尾：`MATCH...CREATE` 跑通 → 下一帧 `k-vk-host` 自动出现新 node（V2.5c 确认的管线）。

## 六、2026-06-12 V2.5d：步骤 1 落地——`gos_runtime::create_provisional_node`

读 `gos-protocol::NodeSpec`（`local_node_key: &'static str`、`permissions`/`exports: &'static [..]`）+ `gos-runtime::NodeRecord`（`register_node` 的实现，[gos-runtime/src/lib.rs:511-545](../crates/gos-runtime/src/lib.rs)）后，**步骤 1 原本设想的"二级状态"问题不存在**：

- `register_node` 对**任何**调用者都无条件把新节点设为 `lifecycle: Allocated, binding: NodeBinding::Unbound, instance_id: NodeInstanceId::ZERO`——这就是 §四 所说的"provisional"状态本身，不需要新枚举值或新字段。22 个 boot builtin 也从这个状态起步（之后由 supervisor 按需 `bind_native_executor`/`bind_instance`）。
- 真正缺的是**"怎么给 Cypher CREATE 的新节点分配一个全新、不撞车的 `NodeId`/`VectorAddress`，并填出一份 `NodeSpec`"**——boot builtin 的 `NodeSpec` 是编译期 `&'static` 常量（`derive_node_id(plugin_id, "compile.time.key")`），Cypher 的节点名是运行时字符串，无法直接套用同一构造方式。

落地为 [`gos_runtime::create_provisional_node()`](../crates/gos-runtime/src/lib.rs)（新增，零新类型/枚举/trait）：

- `NodeId`：`[0xC0, 0,0,0,0,0,0,0, seq.to_be_bytes()]`——固定标签字节 `0xC0` + 单调序列号 `seq`（`AtomicU64`），与 `derive_node_id` 的哈希输出空间不重叠（任意精确判定见 `is_provisional_node_id`）。
- `VectorAddress`：`l4 = 0xC0`（现有 builtin 的 `l4` 取值范围是十进制 0-30，0xC0=192 不冲突），`l3/l2/offset` 由 `seq` 拆分而来。
- `NodeSpec`：`node_type: RuntimeNodeType::Vector`（与 `theme.current`/`theme.wabi`/`theme.shoji` 同型——"被动数据节点"既有用法，非新语义）、`entry_policy: EntryPolicy::Manual`、`executor_id: ExecutorId::ZERO`、`permissions`/`exports: &[]`、`local_node_key: "cypher.provisional"`（共享值；Cypher 提供的节点名/属性的存储是 V2.5e 范围，不阻塞"下一帧出现新 node"这条 Soul demo 判据——`render_live_graph` 按 `RuntimeNodeType` 着色，不读节点名）。

新增 host harness [`host-tests/gos-runtime-harness/tests/provisional_node.rs`](../host-tests/gos-runtime-harness/tests/provisional_node.rs)（2 条测试，27/27 全绿，含既有 24+1 条）：每次调用分配不同的 `NodeId`/`VectorAddress`；新节点立即出现在 `node_page`（V2.5c `vk_auto_refresh` 的读取路径）；`graph_epoch` 按调用次数递增；记录的 `node_type`/`lifecycle`/`entry_policy`/`executor_id`/`plugin_id`/`local_node_key` 与上述设计一致。`cargo check --workspace` 与图治理脚本均通过。

**遗留**：步骤 2（`gos-cypher-mut::CreateNode` 变体 + `k-cypher` 的 `CREATE` 模式识别，目前仍按 [lib.rs:18-21](../crates/gos-cypher-mut/src/lib.rs) 硬拒）尚未开始——是 V2.5e 的范围。

## 七、2026-06-12 V2.5e：步骤 2 落地——`CypherMutation::CreateNode` + `k-cypher` 的 `CREATE` 识别

读完 [`gos-ai-bridge/src/lib.rs`](../crates/gos-ai-bridge/src/lib.rs) 确认：`gos-cypher-mut::CypherMutation`/`pre_validate` 是 **AI 建议-确认管线**（`LlmResponse.mutations` → `pre_validate` → `MutationGate::accept_index` → `apply_mutation`）的载体，而 `k-cypher` 的 `CALL activate(n)`/`spawn(n)`/`route(e)` 从不经过 `gos-cypher-mut`，直接调用 `gos_runtime::*`（[lib.rs:476-569](../crates/k-cypher/src/lib.rs)）。两条路径都需要接 V2.5d 的 `create_provisional_node`，但接法不同——本步同时做了两件事：

1. **`gos-cypher-mut`（[lib.rs](../crates/gos-cypher-mut/src/lib.rs)）**：`CypherMutation` 新增 `CreateNode`（unit variant，无 `Label`/`{props}` 载荷——与 §六 末尾"Cypher 节点名/属性存储"是分离问题，见下方遗留）。
   - `pre_validate`：新增 `CreateNode => Ok(())`——模块文档同步更新，"node create... 全部硬拒"改为"node *delete*/NodeId 重分配/plugin manifest mutation 仍硬拒；CreateNode（仅产出 provisional node）现在接受"。
   - `to_envelope`：`kind` 从硬编码 `EdgeUpsert` 改为按 variant 派生；`CreateNode` → `(ControlPlaneMessageKind::NodeUpsert, 0, 0)`——提案时新节点尚不存在，无 id 可打包；envelope 仅审计"`source` 请求了一次 create"，真正的 `NodeUpsert`（带真实 id/vector）由 `register_node` 在 dispatch 时另行发出。
   - `MutationDispatcher` 新增 `fn create_node(&mut self) -> Result<NodeId, u32>`；`apply_mutation` 返回类型从 `Result<(), MutationError>` 改为 `Result<Option<NodeId>, MutationError>`——其余三个 variant 返回 `Ok(None)`（既有调用点都是 `.expect(...)` 丢弃返回值，无需改动），`CreateNode` 返回 `Ok(Some(新 NodeId))`，供同语句后续 `CREATE (a)-[:Mount]->(n)` 引用。
2. **`gos-runtime`（[lib.rs](../crates/gos-runtime/src/lib.rs)）**：`create_provisional_node()` 返回类型改为 `Result<(NodeId, VectorAddress), RuntimeError>`（原先只返回 `NodeId`；`VectorAddress` 在函数内本就已算出，现一并交还——`k-cypher` 的打印路径需要它，详见下条）。`RuntimeDispatcher::create_node` 新增，调用 `create_provisional_node()` 并丢弃 vector，经 `dispatcher_reject_tag` 映射错误。
3. **`k-cypher`（[lib.rs:571-590](../crates/k-cypher/src/lib.rs)）**：`run_query` 新增一支——`match`-前缀通过后，若 query 含字面子串 `"create ("`（大小写不敏感），直接调用 `gos_runtime::create_provisional_node()`（**不经过** `gos-cypher-mut`，与 `spawn`/`activate`/`route` 的直调风格一致），打印新节点的 `vector`；失败则计入 `state.faults`，与其余分支同形。`print_help` 新增一行 `MATCH (n) CREATE (m)`。

**为什么两条路径都要接，而不是二选一**：`gos-cypher-mut` 模块文档原本就声明"parser 和 AI suggestion path（H.2）feed the same gate"——`k-cypher` 的交互式 `CREATE` 走直调（与既有 CALL 动词同形，零新架构），但若不给 `gos-cypher-mut::CypherMutation` 加 `CreateNode`，AI 建议管线就永远无法提议"创建节点"，与 H.2 的既有意图（AI 建议受 `pre_validate` 同一道闸）相悖，且与本 ADR"选向 A"的决定（Cypher CREATE 应被允许）不一致。两处改动都很小、都镜像既有同类 variant/分支的形状，ADR-005 §五 step 2 已预先授权，未触发新 ADR。

**验证**：`cargo check --workspace`（kernel 目标，含 `crates/hypervisor`）干净；`gos-runtime-harness` 28/28（`provisional_node.rs` 由 2 条增至 3 条——新增 `create_node_mutation_dispatches_to_provisional_node` 验证 `apply_mutation(RuntimeDispatcher, CreateNode)` 与 `RuntimeDispatcher::create_node` 直调均产出可见、`graph_epoch`+1 的 provisional node；`runtime.rs` 的 `cypher_mutation_pre_validate_and_dispatch` 扩展 `CreateNode` 的 pre_validate + Stub dispatch 断言）；图治理脚本 OK。

**Soul demo（§五 step 4）现状**：在 `k-cypher` 输入 `MATCH (n) CREATE (m)` 现在会同步分配并注册一个真实 provisional node、`graph_epoch` +1——V2.5c 的 `vk_auto_refresh` 下一次轮询即可见。端到端原语链路（CREATE 输入 → `create_provisional_node` → `node_page`/`graph_epoch` → `vk_auto_refresh` → `k-vk-host` 渲染）现在每一段都有 harness 覆盖；`k-cypher` UI 分支本身的交互式 QEMU 验证（键入 `MATCH (n) CREATE (m)` 并观察 viewer 下一帧）留待与 B3b 同类的人工/QEMU verification pass，未阻塞本步。

**遗留**（候选 V2.5f 或并入 V2.6，与 `/goal 推进到v2.6` 主线一致评估）：
1. Cypher 提供的 `Label`/`{props}` 持久化——`local_node_key`/`plugin_id` 目前仍是 `create_provisional_node` 的共享占位值（`"cypher.provisional"`/`PROVISIONAL_PLUGIN_ID`），不区分"哪条 CREATE 创建了哪个节点"。`no_std`/`&'static str` 约束下需要新的存储原语（有界属性表？侧路 ECS？），范围明显大于本步，且 §六已确认不阻塞 Soul demo 判据——值得单独 ADR。
2. "promote" 机制（§五 step 3）：给 provisional node 加一条 Grant 边即视为 promoted——`capability_check` 已具备判定能力（V2.4b/c），缺的是触发点（谁在什么时候加这条边）。
3. `CREATE (a)-[:Mount]->(n)` 同语句边接线——`apply_mutation` 现在已经把新 `NodeId` 传回调用者，原语就绪，但 `k-cypher` 的子串 dispatcher 与 `gos-cypher-mut` 的 `AddEdge` 尚未在"同一语句内"组合；目前两者各自独立可用。
