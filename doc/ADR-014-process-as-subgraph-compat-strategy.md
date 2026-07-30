# ADR-014：进程＝子图模型 + 兼容层策略选向（WASI-first / POSIX-native-first / 推迟）

> 状态：**提案待选向** · 提案日期：2026-06-12 · 配套：[V3 计划](../plan/V3_DEVELOPMENT_PLAN.md)（§生态/兼容两支柱、附录 A/C、Parity 不变式、铁律 2/3）、[ADR-001 §2.2/§2.3/§三](./ADR-001-edge-algebra-constitution.md)（`Grant` 位的宪法定义）、[ADR-005 §五/§七](./ADR-005-node-mutation.md)（provisional node + 未接线的"promote"机制）、[ADR-006](./ADR-006-capability-graph-migration.md)（capability_check↔claim_resource 统一，选向待定）、[ADR-010](./ADR-010-f5-persistent-storage-path.md)（F.5，文件型 fd 的真实存储后端）
>
> 口径：V3 计划把本 ADR 称为"V3 最大的分叉点"——`crates/k-wasm`/`crates/k-libc` 等任何兼容层代码动笔前，必须先有"外来进程在图里是什么"的答案，并满足 Parity 不变式（人类/AI/gpm/外来进程共用同一条 `gos-cypher-mut` 门禁 + `capability_check`，禁止第二条特权路径）。本 ADR 分两部分：**§二"进程＝子图"映射是选项无关的——它同时是 [ADR-005 §七遗留 2](./ADR-005-node-mutation.md)"promote 机制缺触发点"问题的第一个具体触发场景，建议无论 A/B/C 如何选向都可以先行接线**；§三/四是 WASI-first / POSIX-native-first / 推迟 三个选项及建议，**不替你拍板**——这是继 ADR-005 之后第二个"不拍板"级别的分叉。

## 一、问题陈述

V3 计划的兼容支柱（V3.2 WASI、V3.5 POSIX-lite）和生态支柱（V3.1 `gpm`，附录 B）都需要回答同一个前置问题："一个外来程序在 GOS 的图里，对应什么节点/边结构？" 在回答这个问题之前：

- `gpm install hello.wasm`（附录 B）写下"子图出现在 3D 视图"，但子图的*形状*未定义。
- 兼容层的资源访问（文件/网络/内存）如果发明自己的权限表，就违反 Parity 不变式（V3 风险表："兼容层把身份拖成 Unix 克隆"，致命级，对策是"ADR-014 必须先给出'进程=子图、fd=边'的完整映射才准写实现代码")。
- 现有 substrate 调研（见下）显示：**V2.4a/b 已经把"capability 检查＝Grant 路径图查询"实现完毕**（`gos-mutation-dispatch::capability::reachable_via_grant`/`capability_check`，[ADR-001 §2.2](./ADR-001-edge-algebra-constitution.md)）；`RuntimeEdgeType::Use`/`Call` 已经携带 `Grant` 位（`edge_algebra.rs` `lower()`，`grant_edges_from_specs` 据此过滤）；`gos_runtime::create_provisional_node` + `CypherMutation::CreateNode`（[ADR-005 §六/§七](./ADR-005-node-mutation.md)）已经能在 mutation gate 后铸造新节点。**这三块拼起来，"进程=子图、fd=边、access=capability_check"在今天的代码里已经有 90% 的地基**——缺的是把它们组装成一个具体的"进程"形状，以及谁/何时触发 ADR-005 §五 step 3 设想的"promote"（加一条 Grant 边）。本 ADR 的核心贡献是给出这个组装方式。

## 二、进程＝子图映射（选项无关，建议先行接线）

### 2.1 进程节点

一个外来程序实例 = 一个通过 `gos_runtime::create_provisional_node`（[ADR-005 §六](./ADR-005-node-mutation.md)）铸造的 `NodeSpec`：

- `node_type: RuntimeNodeType::Compute`（`0x20`）——语义上最贴近"一个正在运行的计算"，区别于 `Driver`/`Service`（常驻）、`PluginEntry`（启动期）。
- `entry_policy: EntryPolicy::OnDemand`（`0x02`）——按需创建，不在 boot 时实例化。
- `executor_id`：兼容层解释器/运行时自己的 `ExecutorId`（如选项 A 则为 `ExecutorId::from_ascii("native.wasm")`），与 `k-cypher` 的 `EXECUTOR_ID = ExecutorId::from_ascii("native.cypher")`（`k-cypher/src/lib.rs:41`）同一机制——**外来进程就是"又一种 node executor"**，不是新的特权层级。
- 创建路径：`CypherMutation::CreateNode` 经 `gos-cypher-mut` 门禁（source 戳标记为发起者——shell/`gpm`/AI 均可，门禁逻辑不感知"这是不是一个进程"）。

### 2.2 fd＝边

WASI 的 capability 句柄模型和 POSIX 的 fd 表，都是"一个整数 → 一份具名权限"。在图里：

- **`RuntimeEdgeType::Use`**（`Refer+Bind+Grant`，exclusive，`edge_algebra.rs:290-292`）：适配"进程独占持有、生命周期耦合"的 fd——典型如一个打开的文件描述符（进程退出应连带释放）。`Bind` 位天然表达"目标的生命周期与本边绑定"。
- **`RuntimeEdgeType::Call`**（`Refer+Send+Grant`，`edge_algebra.rs:256-258`）：适配"可调用的能力引用"——典型如一个 socket 端点或一个预开放目录的"打开"能力（WASI 的 `fd_prestat`）。`Send` 位天然表达"经此边发起调用/消息"。
- 两者都携带 `Grant` 位——`grant_edges_from_specs`（`gos-mutation-dispatch/src/capability.rs:136-148`）已经把"`edge_type.lower().bits.grant` 为真的边"自动收进 `GrantEdge` 表。**WASI 的 capability 句柄和 Grant 边不是"类比"，是同一个枚举位**——V3 附录 A"同构"的说法在今天的代码里已经成立，不需要新发明。
- fd 的整数索引 = 进程节点在其 `ExecutorContext`（`NodeExecutorVTable`，`gos-protocol/src/lib.rs:1507-1523`）里维护的 `[EdgeId; N]` 本地投影——边本身是事实来源，整数表只是解释器侧的缓存，镜像 `capability.rs` "the table is the graph" 的既有风格（`grant_edges_from_specs` 已经是这种"interned 投影"的先例）。
- 目标节点：文件型资源指向 [ADR-010](./ADR-010-f5-persistent-storage-path.md) F.5-graph-integration 设想的 `:File{path}` 节点（带 `EdgeAttrs::persistent`）；socket 型指向 `k-net` 暴露的资源节点（`RESOURCE_SOCKET`）；两者今天的真实后端成熟度不同（`k-net` 已是真实 e1000/virtio 驱动；文件写入依赖 ADR-010 F.5-wiring，目前 0 接线）——这是后端完备性问题，不影响图层映射本身。

### 2.3 打开新 fd ＝ mutation（这是 ADR-005 §七遗留 2 的第一个具体触发场景）

`path_open`/`sock_open` 等"申请新能力"调用 = `CREATE (proc)-[:Use]->(target)`（或 `:Call`；若 `target` 尚不存在则同语句 `CreateNode`，复用 [ADR-005 §七遗留 3](./ADR-005-node-mutation.md) 设想的同语句边接线），经 `gos-cypher-mut` 门禁——**与人类在 `k-cypher` 里手敲 `CREATE` 走同一个函数**。

**鉴权前提**：该 mutation 仅在 `reachable_via_grant(nodes, edges, proc_node, target_node)`（或更准确地：`proc` 的容器/沙箱祖先到 `target` 存在 Grant 路径——即 capability.rs 文档注释描述的"委派链"）为真时被门禁接受。这正是 [ADR-005 §五 step 3](./ADR-005-node-mutation.md)"给 provisional node 加一条 Grant 边即视为 promoted"设想的**第一个具体触发点**：V2.4b/c 已经让 `capability_check` 有能力判定结果（"缺的是触发点：谁在什么时候加这条边"——ADR-005 §七原话），本 ADR 给出答案："一次经过门禁、且鉴权前提满足的 capability-granting mutation"。

`gos-supervisor` 的 `ClaimRecord`/`ClaimPolicy`（Shared/Exclusive 租约仲裁，`gos-supervisor/src/lib.rs:454-478`/`1566-1675`）位于 Grant 边层**之下**：Grant 边回答"`proc` 是否*曾被授权*接触 `target`"（鉴权，经门禁，Parity 不变式覆盖范围）；`claim_resource` 回答"`proc` 现在能不能拿到 `target`，考虑并发持有者"（租约仲裁，非安全边界，今天就不经门禁审计）。二者正交——[ADR-006](./ADR-006-capability-graph-migration.md) 选项 B（`capability_check` 是否应接入 `claim_resource` 实时路径，选向待定）无论选哪边，§2.3 都能正确组合：选 B 则鉴权+租约合并一次调用；不选则维持本节描述的两步。本 ADR 不依赖 ADR-006 选向。

### 2.4 实际 I/O（`fd_read`/`fd_write`/...）

`capability_check(proc_node, target_node)`（常见情况是直接边，O(1)；委派链才需要 BFS）→ `gos-supervisor::claim_resource`（租约）→ 真实后端：

- 文件：`gos-vfs`/`k-fat32`（读路径今天可用于只读单 cluster；写路径依赖 [ADR-010](./ADR-010-f5-persistent-storage-path.md) F.5-wiring，目前 0 接线——选项 A/B 的"hello world"演示如果只需 `fd_write` 到 console/stdout，可以完全绕开这条依赖，见 §3.1）。
- 网络：`k-net`（e1000/virtio 驱动 + tcp 模块已是约 1300 行真实代码——是兼容层第一个有真实后端的资源类型）。
- 内存：`AllocPages`/`FreePages`（syscall `0x01`/`0x02`，`ring3.rs:36-46`，已编程但 0 调用方）直接对应 WASI 线性内存增长/POSIX `mmap`/`brk`。

### 2.5 进程退出 ＝ RemoveEdge（不引入新的删除原语）

逐条 `CypherMutation::RemoveEdge`（[ADR-005 §七](./ADR-005-node-mutation.md)已允许的变体）撤销该进程节点的所有 `Use`/`Call` 出边；进程节点本身保持"无出边的孤立 provisional node"——不触发节点删除（ADR-005 明确推迟了节点删除/NodeId 回收）。`capability.rs` 的"fault containment"文档注释（line 18-20）保证：无 Grant 出边的节点对图的其余部分零授权，孤立节点是无害的，节点删除/回收的统一方案留给 ADR-005 后续一并处理——本 ADR 不引入新的缺口。

## 三、选项（兼容策略）

### 选项 A —— WASI-first（V3 附录 A 倾向）

新 crate `crates/k-wasm`：`no_std` wasm 解释器（字节码校验 + 解释循环），作为 §二 的一种 `ExecutorId`（如 `native.wasm`）。`wasi_snapshot_preview1` host-call 经 §二 映射落地，初版只需覆盖最小子集（`fd_write`/`environ_sizes_get`/`environ_get`/`proc_exit`/`random_get`）即可支撑"hello world"。

- **不依赖 ring3 成熟度**：解释器本身就是沙箱（字节码校验保证内存安全），可在 supervisor 上下文作为 node executor 运行——镜像 `k-vk-host`/`k-cuda-host` 的 host-bridged-first 先例（但角色反转：这里 wasm 字节码自身是隔离边界，ring3 MSR 机制——已编程、0 调用方——是*额外*的纵深防御层，留给 B.4.6.x/E.3，不阻塞首个 demo）。
- **杀手级 demo 候选**：`wasm32-wasi` 编译的 hello-world，经 `gpm install`（附录 B）后子图出现在 3D 视图（`(:Process)-[:Use]->(:Console)`），运行后输出"Hello from wasm"到 `k-vga`/console，退出后边消失——V3 的第二个"Soul demo"，且 `fd_write→console` 完全不依赖 ADR-010 F.5-wiring。
- **syscall 表 v1**（[V3 附录 C](../plan/V3_DEVELOPMENT_PLAN.md)）与解释器关系：`0x05 GraphSnapshot`（进程自省自己的出边=已授予的 capability 列表，对应 `fd_prestat_get`/`fd_readdir` 类自省）、`0x06 SubmitMutation`（`path_open`/`sock_open` 类"申请新 fd"，即 §2.3 的 mutation）、`0x07 SubscribeRegion`（`poll_oneoff` 类异步就绪，复用 V2.3 反向传播机制）——三者是解释器 host-call shim 的实现基础，但**初版 demo 不需要全部三个**（`fd_write→console` 只需既有的 `EmitSignal`/直接 console capability）。
- **代价**：解释执行慢（V2.6c/ADR-012"fast-path 标签节点"是长期答案，本 ADR 因此提升 ADR-012 优先级，见 §四）；解释器本体预估 5-10k 行。
- **Plan B（逃生门，须写入选向）**：若"hello world .wasm"在 8 周预算内未跑通，**不**就地补救——停止选项 A 的实现投入（保留已写代码与教训），转向选项 C（兼容暂缓），并把 V3.5（POSIX-lite）提前。§二映射在两种结局下都已沉没成本最小化——它是 A/B 共享的部分。

### 选项 B —— POSIX-native-first

直接扩 syscall 表到 POSIX 子集（`open`/`read`/`write`/`close`/`mmap`/`exit` 起步）+ 薄 libc（`crates/k-libc`）。§二映射不变（fd=边等），但宿主是真实机器码，**前提是 ring3 隔离先成熟**（B.4.6.x/E.3 从"0 调用方"变为真实可用）——比选项 A 多一层硬前提。

- **代价**：`fork`/`exec`/信号是图模型里没有先例的尖锐问题——`fork`="子图克隆"，CoW/边复制语义未定义（需要自己的 ADR）；`exec`=原地替换节点的 `executor_id`+镜像，同样novel；信号可复用既有 `RuntimeEdgeType::Signal`（`0x04`，已存在）但投递/屏蔽/handler 语义未定义。V3 风险表"兼容层把身份拖成 Unix 克隆"评级**致命**，B 是三个选项里命中此风险概率最高的。

### 选项 C —— 双轨推迟

只接线 §二（进程=子图映射 + ADR-005 §七遗留 2 的 promote 触发点），不选 A/B；A vs B 留给 V3.1 反馈后再评估（V3 计划本身的依赖链：V3.0→V3.1→V3.2，兼容层本来就排在 SDK/ABI 冻结之后）。

- **代价**：V3 风险表"生态冷启动少一条腿"——`gpm`（附录 B）在 V3.0/3.1 期间只能分发原生 `.gosmod`（既有 B.4.6 ET_DYN 格式），"编译到 wasm 的语言生态"这条生态杠杆暂不可用。

## 四、建议与门禁

**§二（进程=子图映射）建议无论 A/B/C 选向如何，都可作为 V2.6→V3.0 的连接性工作先行接线**——它的核心产出（"capability-granting mutation 触发 promote"）正是 [ADR-005 §七遗留 2](./ADR-005-node-mutation.md)悬而未决的"缺触发点"问题的答案，且可以像 V2.5d/e 一样 harness 验证（构造一个 provisional 资源节点 + 一次模拟的 `path_open`-class mutation + 断言 `capability_check` 由假转真），不需要等任何 wasm/libc 代码。

**A vs B vs C 的选向，本 ADR 倾向 A**——理由：

1. §二的地基（Grant 边=capability、provisional node、mutation gate）已经为 A 量身存在，V2.4a/b 当时并非"为兼容层预留"，但客观上已经把 A 的核心机制实现了 90%。
2. A 的隔离边界来自解释器本身，不依赖 B 的强前提（ring3 成熟）。
3. A 的 `fork`/`exec`/信号问题不存在（wasm 没有这些）——B 的"致命级"风险在 A 不出现。
4. B 的核心成果（§二映射）在 A 中同样产出且可复用于 V3.5——选 A 不是"放弃 B 的价值"，只是排序。

**但本 ADR 不替你拍板**——V3 计划称之为"V3 最大的分叉点"，门禁逻辑（铁律 2）要求 `crates/k-wasm`/`crates/k-libc` 或任何新增编译目标在本文档 §五 记录选向之前不得出现在 `Cargo.toml` workspace members 中。门禁范围**不含**§二的接线工作（见上，可先行）。

**关联待选向项的交互说明**（均不阻塞本 ADR 的选向，仅供选向时参考）：[ADR-006](./ADR-006-capability-graph-migration.md) 选项 B（capability_check↔claim_resource 统一）与 §2.3 正交，两种结局都兼容；[ADR-010](./ADR-010-f5-persistent-storage-path.md)（F.5-wiring）决定文件型 fd 的 `fd_read`/`fd_write` 何时触达真实存储，但不阻塞 console/网络型 fd 的首个 demo；**建议下一步起草 [ADR-012](../plan/V2_DEVELOPMENT_PLAN.md)（V2.6c，fast-path 节点标注）**——本 ADR §3.1 已经指出它是选项 A 解释执行开销的长期答案，优先级因此上升。
