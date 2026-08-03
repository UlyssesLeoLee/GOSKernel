# ADR-017：AI principal 与通路统一——把活的 k-chat 接到分型的 gos-ai-bridge 上

> 状态：**已选向：选项 A · 已落地**（2026-08-03）· 提案日期：2026-06-12 · 配套：[AI 原生方案](../plan/AI_NATIVE_OS_PLAN.md)（本 ADR gates 其 AI.1 切片）、[V3 计划 F3](../plan/V3_DEVELOPMENT_PLAN.md)（"AI 原生"前沿判据 + V3.1 F3 demo 收尾）、[gos-ai-bridge](../crates/gos-ai-bridge/src/lib.rs)（H.2 分型 gate，0 生产调用方）、[k-chat](../crates/k-chat/src/lib.rs)（COM2 桥，B3b 已验证）、[gos-cypher-mut](../crates/gos-cypher-mut/src/lib.rs)（H.1 receptive subset + `b"K_AI"` source 戳）
>
> 口径：AI 原生方案 §一 已盘点清楚：`k-chat` 有活的模型连接但无分型（自由文本 + 3 个硬编码 `GTOOL:` 工具），`gos-ai-bridge` 有完整分型（`LlmRequest/LlmResponse/MutationGate/pre_validate` 整轮拒绝）但 `install_backend` 全仓库零调用。本 ADR 决定**接线的具体形状**——五个待定点：(a) `MutationGate` 实例归属；(b) mutation 建议的 wire 编码（`GMUT:` 帧）；(c) `LlmBackend` 的安装者是谁；(d) k-ai 事件窗口的归属；(e) AI 节点的初始权限面。目标状态：F3 demo（"说一句话改写 OS"）的每一步都走 Parity 不变式的同一条 gate，journal 里留 `b"K_AI"` 记录。

## 一、问题陈述

### 1.1 接线点 (c)：`LlmBackend` 的正确安装者是一次结构反转

今天 k-chat 的数据流是"k-chat **就是**对话主体"：`on_event` 收 shell 输入 → 直接写 COM2 `GCHAT:` 帧 → 轮询收 `GRESP:`/`GTOOL:`/`GDONE:` → 渲染 VGA。`gos_ai_bridge::ask()` 完全不在环上。

而 `LlmBackend` 的签名（[lib.rs:93-101](../crates/gos-ai-bridge/src/lib.rs)）是一个 C-ABI `query(prompt, context, out_response) -> i32`——它期望的形状恰好是"**把一轮往返封装成一次调用**"。所以接线不是"k-chat 调一下 ask()"那么简单，而是一次职责反转：

- **k-chat 的串口收发逻辑下沉为 `LlmBackend` 实现**——`query()` 内部做 `GCHAT:` 发送 + `GRESP:`/`GMUT:`/`GDONE:` 收集，装配 `LlmResponse` 返回；
- **k-chat 的 `on_event` 改为调用 `gos_ai_bridge::ask()`**——它从"对话主体"退役为"传输后端 + 批准 UI"。

这样 `ask()` 的整轮拒绝语义（首条非法建议丢弃全轮，lib.rs:146-148）和未来任何 tier 1/2 驻留替换（AI 方案 §L3）都自动获得——这正是 `LlmBackend` hook 被设计出来的用法（"host-side k-aiinstalls one that proxies to a real LLM service"，lib.rs:88-91 注释的原意）。

### 1.2 接线点 (b)：`GMUT:` 帧——wire 上怎么携带 `CypherMutation`

现有协议是行分帧文本（`GCHAT:`/`GRESP:`/`GTOOL:<t>:<a>`/`GDONE:`/`GRSLT:`，[k-chat lib.rs:8-17](../crates/k-chat/src/lib.rs)）。`CypherMutation` 只有 4 个变体且字段全是定长整数（[gos-cypher-mut:47-73](../crates/gos-cypher-mut/src/lib.rs)：`AddEdge{from,to,edge_kind}`/`RemoveEdge{edge_id}`/`RebindUse{from,new_target}`/`CreateNode`），天然适合同风格的文本帧：

```
GMUT:add_edge:<from_hex>:<to_hex>:<mount|use>\n
GMUT:remove_edge:<edge_id_hex>\n
GMUT:rebind_use:<from_hex>:<to_hex>\n
GMUT:create_node\n
```

两道解析防线（纵深）：host 侧 `chat-bridge.py` 把模型的结构化输出（tool-call/JSON）翻译为 `GMUT:` 帧时**预过滤**到 receptive subset（`LlmResponse` 文档注释 lib.rs:59-62 预设的"dropped server-side"行为）；内核侧 `query()` 实现解析帧后逐条进 `ask()` 的 `pre_validate`——host 侧被攻破/有 bug 也越不过内核 gate。一行解析失败 → 整轮拒绝（与 `ask()` 语义一致，不静默跳过）。

### 1.3 接线点 (a)：`MutationGate` 实例归谁持有

`MutationGate` 是纯逻辑结构（定长数组 + len，无锁无分配），gos-ai-bridge 定义了它但没有实例。候选归属：k-chat 的 `ChatState`（`#[repr(C)]` 状态块，随节点生命周期）、gos-ai-bridge 内部 static（mirror 它自己的 `BACKEND: Mutex<Option<LlmBackend>>` 模式，lib.rs:116）、或 k-ai。

### 1.4 接线点 (d)(e)：事件窗口与 AI 权限面

(d) `k-ai` 已在 drain 控制平面事件并计数（[AiState](../crates/k-ai/src/lib.rs) `plugin_events`/`fault_events` 等）——AI.2 的装配器（ADR-018）需要的是"计数升级为有界环形窗口"。本 ADR 只需**不堵住**这条路：窗口归属 k-ai，格式留给 ADR-018。

(e) AI 节点（k-chat/k-ai）今天的 `NodeSpec.permissions` 与 Grant 边是按普通插件配的。AI 方案 §四6 的治理红线（AI 系 crate 禁止 `apply_mutation` 之外的写路径）需要一条可机械检查的规则落进 `verify-graph-architecture.ps1`。

## 二、选项

### 选项 A——k-chat = 传输后端 + 批准 UI；gate 实例为 gos-ai-bridge 内部 static；`GMUT:` 文本帧（倾向）

按 §1.1 的反转执行：k-chat 注册 `LlmBackend`（串口往返封装进 `query()`），`on_event` 走 `ask()`；`MutationGate` 作为 gos-ai-bridge 的模块级 `Mutex<MutationGate>`（mirror `BACKEND` 模式）——批准状态住在 no_std 可 harness 的核心 crate 里，k-chat 只是调 `accept_index`/`reject_index` 的 UI；shell 加 `ai approve <i>`/`ai reject <i>`/`ai pending` 三个子命令（走 k-shell 既有的"函数→信号→目标节点"模式）；`GMUT:` 按 §1.2 编码；k-ai 本切片不动。

- **优点**：每个种子留在自己最擅长的角色——k-chat 的 B3b 已验证串口逻辑原样复用（只是换了调用方向）、gos-ai-bridge 从 0 调用变成唯一入口、批准状态可在 host harness 里用 deterministic stub 后端全覆盖测试（`LlmBackend` 注释明示这是设计意图）。diff 集中在 k-chat 一个 crate + bridge.py，wire 协议纯增量（旧帧语义不变，B3b 回归可先行）。
- **代价**：k-chat 内部结构改动不小（对话主循环反转）；`Mutex<MutationGate>` 的锁与 `on_event` 的并发关系需要复核（与 `BACKEND` 同型，低风险但非零）。

### 选项 B——合并 k-chat + k-ai 为新 crate `k-agent`，一步到位

承认 k-chat（对话）与 k-ai（事件聚合）终将融合为"agent 前端"，现在就合并成一个新节点，gate/窗口/批准全部内聚。

- **优点**：终态更干净，少一次未来迁移。
- **代价**：丢掉 B3b 已验证的 k-chat 稳定性基线去换一个推测的终态；AI 方案 AI.3 的"agent=子图"到来时，agent 的正确形状是**多实例子图**（每个 agent 一个），不是一个更大的单体节点——现在合并很可能是朝错误方向的提前优化。diff 跨三个 crate，违背"切片最小可验证"。

### 选项 C——批准决策放在 host 侧（bridge.py 持有 pending 列表，内核只收已批准的 mutation）

UI 上最省事：host 终端里 y/n，内核侧零新命令。

- **代价**：**违反 Parity 不变式的精神**——批准这个动作本身必须发生在图能看见的地方（journal 记录"谁在何 tick 批准了什么"）；host 侧批准让审计链在最关键一环断在内核外，且"signed AI runtime 解锁 Auto"（AcceptanceMode 文档预设的信任机制）将无处安放——你无法对一个 python 脚本做 G.2 验签。直接排除，列出仅为完整性。

## 三、建议与门禁

倾向 **A**：职责反转让三个种子各归其位——分型核心（gos-ai-bridge）成为唯一入口，传输（k-chat）可替换，批准状态可 harness。这是 AI 方案 §三 AI.1 行的直接执行形状。

**门禁**：
- **B3b 回归先行**：改 k-chat 前，现有 `GCHAT/GRESP/GTOOL/GDONE` 往返的回归 harness 必须存在并绿（保护已验证功能）。
- **Parity harness**（AI.1 退出判据）：同一逻辑变更走 shell 路径与走 AI 路径，产生的 `AuditedMutation` 除 `source` 戳外 byte-identical；DryRun 模式零应用、整轮拒绝（一条非法建议毒化全轮）各有用例。
- **锁纪律**：`Mutex<MutationGate>` 不得在持锁状态下做串口 I/O（`query()` 在锁外执行，结果在锁内 enqueue）——mirror `route_signal` 释放锁再调 executor 的既有纪律。
- **阻塞债不带出 AI.1**：k-chat 现 `on_event` 内 `LINE_TIMEOUT≈5s` 轮询（AI 方案 §四5 已记录）——反转时一并改为非阻塞分段收集，或在 ADR 选向时明确接受"再欠一个切片"并记录,二选一,不允许沉默继承。
- **治理红线落地**：`verify-graph-architecture.ps1` 加规则——`crates/k-chat`、`crates/k-ai`、`crates/gos-ai-bridge` 及未来 `agent` 系 crate 中,出现对 `gos_runtime::` 写路径（`apply_mutation` 链之外）的直接调用即 CI 红。
- (d)(e) 的窗口格式与 Grant 初始集**不在本 ADR 门禁内**——分别留给 ADR-018 与 AI.2 的工具注册表工作,本 ADR 只固定归属（窗口归 k-ai）与红线形状。

## 四、落地状态（选项 A，2026-08-03）

**接线点 (a) MutationGate 归属**：`gos_ai_bridge::GATE`，模块级 `Mutex<MutationGate>`，mirror `BACKEND` 的既有模式（[lib.rs](../crates/gos-ai-bridge/src/lib.rs)）。新增访问器 `gate_enqueue`/`gate_pending_snapshot`/`gate_len`/`gate_accept_index`/`gate_reject_index`/`gate_clear`，均不持锁做 I/O（锁纪律门禁满足——`ask()` 的串口往返发生在 `gate_enqueue` 调用之前，两者不共享临界区）。

**接线点 (b) `GMUT:` 帧**：落在 `gos_ai_bridge::wire`（不是 k-chat）——纯逻辑、无硬件依赖，因此是真实可 host-test 的代码，不需要像 k-chat 其余部分那样在 harness 里镜像。`add_edge` 只接受 `mount|use`（`depend`/`link` 虽然通过 `pre_validate`，但 wire 解析器直接拒绝——这是 AI 面的实际执行点，门禁按 §1.2 原样落地）。

**接线点 (c) LlmBackend 反转**：`k-chat::llm_backend_query`（`crates/k-chat/src/lib.rs`）是新的 `LlmBackend::query` 实现——串口收发下沉，`proc.rs` 的 `Send` 分支改为调用 `send_via_ai_bridge` → `gos_ai_bridge::ask()`。`GTOOL:` 帧仍在 `llm_backend_query` 内联派发（副作用，不进入分型 `LlmResponse`），行为与反转前一致。

**顺带修复的真实 bug**：`com2_ready` 此前初始化为 0 后**永远不会被置 1**（`com2_probe()` 定义了但从未被调用）——COM2 桥接路径在合并后的内核里从未真正工作过。反转顺手把探测调用接上（`send_via_ai_bridge` 首次发送时惰性探测），这是本 ADR 触及同一段代码时发现并修的独立缺陷,不是本 ADR 的设计目标。

**接线点 (d)(e)**：未动——按门禁明确排除在外。

**命令名与 ADR 原文的偏差**：原文建议 `ai approve/ai reject/ai pending`，但落地时发现 `ai`/`ask` 在 k-shell 里已被一个无关的既有功能（`enter_ai_api_mode`，一个底部 AI 编辑面板）占用。改用 `chat pending`/`chat approve <i>`/`chat reject <i>`，与既有 `chat key`/`chat model`/`chat api`/`chat http` 子命令族同款——`function → signal → target node` 模式（`CHAT_CONTROL_AI_PENDING/APPROVE/REJECT`，`0xC8`-`0xCA`）。k-chat 是实际调用 `accept_index`/`reject_index` 的一方（§1.3 原文"k-chat 只是调 accept_index/reject_index 的 UI"），shell 侧只发信号,不影子维护 gate 状态——结果通过 k-chat 自己的控制台 sink 渲染（同一块 VGA）。

**锁纪律**：满足——见接线点 (a)。

**阻塞债**：`LINE_TIMEOUT`（约 5s 轮询）原样保留，未在本切片修复——显式记录为遗留债务，不是沉默继承（门禁二选一里选择"记录"这一支）。

**治理红线**：`tools/verify-graph-architecture.ps1` 新增规则——`crates/k-chat`、`crates/k-ai`、`crates/gos-ai-bridge` 中禁止出现 `gos_runtime::{register_node,create_provisional_node,rebind_use,register_node_prop_u8,register_node_prop_u32,register_node_routes,apply_cypher_mutation,add_edge,remove_edge}(` 直接调用；`gos_runtime::RuntimeDispatcher`（喂给 `gos_cypher_mut::apply_mutation` 的标准适配器）不在禁止之列。

**Parity harness**：`host-tests/gos-shadow-kernel-harness/tests/shadow_kernel.rs` 新增 13 个 `adr017_*` 测试——`wire::parse_gmut_line` 直接测试（真实代码，非镜像）；`ask()`/`MutationGate` 端到端生命周期（stub `LlmBackend` + 真实 `gos_supervisor::apply_cypher_mutation` 落图验证）；`llm_backend_query` 帧分类循环的镜像测试（k-chat 本身不可 host 编译,镜像是既定模式），含 B3b 回归证明（GRESP/GTOOL/GDONE 累积行为在引入 GMUT 后不变）与 GMUT 单行解析失败拒绝整轮的证明。DryRun 零应用、reject 不落图两个用例也已覆盖。全套 19 个测试（6 个既有 + 13 个新增）通过；`cargo check --workspace`、`cargo check -p gos-kernel`、`tools/verify-graph-architecture.ps1` 均绿。

**明确未做（不在本 ADR 门禁内，留给后续切片）**：`tools/chat-bridge.py` 未新增任何 `GMUT:` 帧的生成逻辑——给模型设计一套"建议图变更"的自然语言/工具调用语法，并让它安全地引用裸 `NodeId`/`EdgeId` 十六进制值，属于 AI.2 工具注册表（ADR-018）的范畴，不是本 ADR 的接线形状问题；仅更新了协议表文档。(d) 事件窗口、(e) 初始权限面同样留给 ADR-018/AI.2。
