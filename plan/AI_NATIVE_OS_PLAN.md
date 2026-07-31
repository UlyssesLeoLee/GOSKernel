# GOS AI 原生深度结合方案 — 从图基底到认知设施

> 状态：提案 · 日期：2026-06-12 · 配套：[V3 计划](V3_DEVELOPMENT_PLAN.md)（F3"AI 原生"判据、Parity 不变式、V3.1 F3 demo——本方案是 F3 的展开与深化，**不另起版本线**）、[gos-ai-bridge](../crates/gos-ai-bridge/src/lib.rs)（H.2 分型 gate）、[k-chat](../crates/k-chat/src/lib.rs)（COM2 桥 + 工具协议）、[k-ai](../crates/k-ai/src/lib.rs)（控制平面聚合器）、ADR-005/012/014/015/016（本方案的依赖选向）
>
> 口径：市面上所有"AI OS"走的是同一条路——把模型栓在**不透明状态**上：截屏、shell 文本、accessibility tree，模型靠猜测系统此刻是什么状态、它的动作产生了什么效果。GOS 不需要这条路，因为它的全部运行态**本来就是**一张结构化、可 Cypher 查询、可订阅、经单一审计 gate 变更的活图（F1，已达成）。"Cypher is the ISA"的直接推论是：**Cypher 也是 AI 的 tool-call API**。AI 结合的三大经典难题在图基底上全部退化为已解决或可解决的图问题——context 装配 → 子图选择问题；工具安全 → Grant 边可达性（V2.4 已建）；审计与回放 → journal + source 戳（已建）。所以本方案的本质不是"给 OS 加 AI"，而是"把已经为 AI 形状长好的基底接上模型"——工作量在**连线**，不在**地基**。

## 〇、L0 基底公理——为什么"AI 化"不需要重造底层

AI 系统的每一项基础需求，对照 GOS 既有机制：

| AI 需求 | 通用 OS 的做法 | GOS 既有机制 | 状态 |
|---|---|---|---|
| **世界模型**（模型能"看见"系统） | 截屏 / shell 输出爬取 / 私有 API | F1 全态活图：`k-cypher` 只读查询 + `node_page`/`edge_page` 分页快照 + 3D 实时渲染 | ✅ V2.5 |
| **行动接口**（模型能"动手"） | 各应用私有 API / 模拟键鼠 | Parity 不变式：与人类 shell 同一条 `pre_validate → MutationGate → apply_mutation` 链，`AuditedMutation.source` 已预留 `b"K_AI"`（[gos-cypher-mut:94](../crates/gos-cypher-mut/src/lib.rs)） | ✅ 原语 |
| **权限模型**（模型被允许做什么） | 全有或全无的 API token | 能力即拓扑：Grant 边可达性（`reachable_via_grant`，V2.4a-c harness 已证） | ✅ 原语 |
| **审计与回放**（模型做过什么） | 应用层日志，格式各异 | journal：每条 AI mutation 是一条带 source 戳的 `ControlPlaneEnvelope`，重启可重放 | ✅ 原语 |
| **沙箱**（模型搞砸了怎么办） | 进程级隔离，粒度粗 | provisional 节点（ADR-005 A 已 wired）：AI 创建的节点 `Unbound`/零实例，promote 前不参与 claim/quota | ✅ 已 wired |
| **感知事件流**（模型对变化做出反应） | 轮询 / 各应用回调 | 控制平面 envelope 队列 + Subscriptions 区域订阅（V2.3 反向传播） | ✅ 已 wired |

这张表是本方案的全部论证基础：**六行需求，六行都有现成机制**。任何在 POSIX 系基底上做 AI OS 的团队，第一年的工作就是去造这张表的右列——GOS 已经把它们当作 OS 本体造完了。这是"AI 原生"作为前沿判据（F3）的真实含义：不是"预装了一个助手"，而是 AI 与人类共用同一套 OS 原语。

## 一、现状盘点——两条平行的 AI 通路（诚实清单）

### 1.1 通路 A：`k-chat`——活的，但未分型

[k-chat](../crates/k-chat/src/lib.rs)（`ExecutorId("native.chat")`，B3b 已端到端验证）：shell 输入 → COM2(0x2F8)/TCP 14444 → `tools/chat-bridge.py`（openai/anthropic/gemini/ollama 四后端）→ 真实 LLM → 文本流回 VGA。工具协议（lib.rs:8-24）：`GTOOL:<tool>:<arg>` 帧触发内核侧执行，`GRSLT:` 回传结果。

**缺陷**：(1) 工具是**三个硬编码字符串匹配**（`ping`/`net:status`/`clear`，lib.rs:18-24）——增加一个工具要改内核代码，AI"能做什么"与 capability 图完全脱钩；(2) 响应是自由文本，**没有 `CypherMutation` 分型**——AI 说"我把主题切换了"和 AI 真的提交一条 `RebindUse` 之间没有任何机制连接；(3) 不经过 `MutationGate`/`pre_validate`，没有 `b"K_AI"` 戳——**通路 A 今天不满足 Parity 不变式**（它不变更图所以暂未违反，但任何沿这条路加"让 AI 改图"的功能都会违反）。

### 1.2 通路 B：`gos-ai-bridge`——分型的、被 gate 的，但死的

[gos-ai-bridge](../crates/gos-ai-bridge/src/lib.rs)（H.2）是 F3 判据引用的正主：`LlmRequest{prompt, context, mode}` / `LlmResponse{text, mutations: [Option<CypherMutation>; 8]}` 有界分型；`ask()` 对每条建议跑 `pre_validate`，**第一条非法建议导致整轮丢弃**（lib.rs:122-150——宁可拒绝整轮，不让合法与非法混进 gate）；`MutationGate` 暂存待批，操作员 `accept_index`/`reject_index` 逐条裁决；`AcceptanceMode::{DryRun, Confirmed, Auto}` 三档，Auto 明文保留——"future trust mechanism (signed AI runtime) would unlock"（lib.rs:46-50）。

**缺陷**：`install_backend` 在全仓库**零生产调用方**（grep 证据：除自身定义外仅 `hypervisor/Cargo.toml:11` 一行依赖声明）。没有任何后端被安装过，没有任何代码调用过 `ask()`。通路 B 是"为 AI 准备的宪法"，但从未开过庭。

### 1.3 `k-ai`——上下文装配的胚胎

[k-ai](../crates/k-ai/src/lib.rs)（`ExecutorId("native.ai")`，Aggregator 节点 (6,2,0,0)，导出 `ai.supervisor` + `graph.orchestrate` 两个 capability）：`AiState` 持续清点控制平面事件——`plugin_events`/`node_events`/`edge_events`/`state_deltas`/`fault_events`（lib.rs:54-72），并从 shell 捕获 API key 与 prompt。**它 drain 的恰好是 L1 感知层需要的事件窗口，但今天只计数、不喂给任何模型**。

### 1.4 结论

三个种子各占一角：A 有活的模型连接但无分型，B 有完整分型但无连接，k-ai 有事件流但无消费者。**本方案的第一步不是新功能，是把三者接成一条通路**——这与 V2 一路的"scaffolding ahead of wiring"修复（ADR-005 选向前的 provisional、ring3 的 0-caller）完全同型。

## 二、五层架构（从底往上）

```
L4 认知设施   NL shell · agent=进程=子图 · 记忆子图 · 自运维 · (远期)语义层
L3 驻留       模型在哪跑：host 桥(今) → 本地 daemon → k-wasm 微模型 → NPU(非目标)
L2 行动       tool-call 三分型：查询 / 变更(gate) / 能力调用(Grant 边)
L1 感知       上下文装配 = 子图选择（图原生 RAG）+ 事件窗口 + 快照
L0 基底       §〇 六公理（全态活图 · Parity · 能力拓扑 · journal · provisional · 订阅）
```

### L1 感知——上下文装配 = 子图选择

`LlmRequest.context` 今天是"free-form bytes; H.2 doesn't constrain encoding"（lib.rs:29-31）。深化为一个**装配器**，三个来源：

1. **Cypher 查询结果**——`k-cypher` 的只读子集已能浏览节点/边；AI 的"检索"就是发 Cypher，不需要外部向量库、不需要 RAG 索引管线：**the graph IS the index**，且永远与真实状态零延迟一致（这是对所有外置 RAG 方案的结构性优势——它们的索引永远在追赶现实）。
2. **事件窗口**——k-ai 已经 drain 的控制平面 envelope 流，变成环形窗口随 context 附带："自上轮以来图发生了什么"。
3. **批量快照**——`node_page`/`edge_page` 分页读，即 [ADR-012](../doc/ADR-012-fast-path-node-tagging.md) 的 `FastPathSnapshot` 权限所命名的机制——**AI 节点是该 permission 的第二个声明者**（第一个是 `k-vk-host::render_live_graph`），ADR-012 的等价性义务同样适用。

**预算问题**：token 上限 → "选哪个子图进 context"是一个带预算的图选择问题（从 prompt 提到的节点出发的 k 跳邻域 + 事件窗口 + 全局摘要计数）。有界缓冲已经在分型里（`MAX_PROMPT_BYTES=4096`）。装配策略的具体选向留给 ADR-018。

### L2 行动——tool-call 的三种分型

| 分型 | 载体 | 安全闸 | 现状 |
|---|---|---|---|
| **查询**（只读） | Cypher 文本 → k-cypher | 只读子集天然安全；`DryRun` 模式 | k-cypher 已有 |
| **变更**（写图） | `CypherMutation` → `pre_validate` → `MutationGate` → `apply_mutation`，戳 `b"K_AI"` | H.1 receptive subset + 操作员批准 + journal | gos-ai-bridge 已建，0 接线 |
| **能力调用**（驱动设备/服务） | 沿 Grant 边 `emit_signal`（`GTOOL:ping` 的正规化） | `capability_check`：AI 节点 Grant 不可达的 capability 调不动 | k-chat 硬编码 3 个，待图派生 |

**工具发现即图查询**：AI 可见的工具列表 = `MATCH (ai)-[:Grant*]->(cap:Capability)` 的结果——"AI 能看到什么工具"与"AI 被允许用什么工具"是**同一个事实**，不可能漂移。对比函数调用式 tool-list（一份 JSON schema 与一份权限检查各自维护、可以不一致），这是图基底的又一次结构性胜利。`GTOOL:` 三个硬编码字符串退役，换成从 k-chat/k-ai 节点的 Grant 边集合生成工具清单。

**批准层级**：`AcceptanceMode` 三档已定义。`Auto` 解锁是**双条件**：(a) 签名 AI runtime（G.2 `gos-sign`/`gos-verify` 验证模型侧组件身份）；(b) 配额机制（每 tick/每 epoch mutation 上限）。两者齐备前 `Auto` 映射到 `Confirmed`（现行为，lib.rs:48-50 已如此实现）。

### L3 驻留——模型在哪跑（可替换性优先）

| Tier | 形态 | 状态 |
|---|---|---|
| 0 | **host 桥**：COM2/TCP → `chat-bridge.py` → 云端/本地 ollama | ✅ 活（B3b 验证，四后端） |
| 1 | **本地推理 daemon**：同一桥协议，模型权重在 host 本地 | 接口就绪即达（ollama 后端已是此形态） |
| 2 | **k-wasm 微模型**：模型作为 node executor 在图内跑（ADR-014 的 `k-wasm` 解释器 + `ExecutorId("ai.wasm")`）——分类/路由级小模型，不是对话模型 | 远期，依赖 ADR-014 选向 + 解释器成熟 |
| 3 | NPU/GPU offload | **V4+ 非目标**（与 V3 计划"裸金属 Vulkan 非目标"同型） |

关键设计判断：`LlmBackend` 的 C-ABI hook（lib.rs:93-101）**已经是**正确的可替换边界——每个 tier 就是一个 `install_backend` 实现，上层（L1/L2/L4）对驻留位置零感知。本方案只承诺 tier 0 完整接线 + tier 1 顺带可用；tier 2 是 ADR-014 通过后的自然延伸，不在近期门禁内。

### L4 认知设施——每一项都是既有机制的组合，零新原语

1. **NL shell**（F3 demo 本体）：自然语言 → LLM → `LlmResponse.mutations` → shell 里逐条展示待批 → 批准 → 图变更下一帧可见。
2. **Agent = 进程 = 子图**：[ADR-014](../doc/ADR-014-process-as-subgraph-compat-strategy.md) 给外来进程定义了"进程=子图、fd=边、访问=capability_check"；一个 AI agent **就是一个 executor 为 LLM 的进程**——同一个子图形状、同一套 Grant 边权限、同一个故障域/重启策略/配额记账，零新概念。更进一步：**agent 是 gpm 包**（[ADR-016](../doc/ADR-016-package-as-subgraph-format.md)）——`gpm install watcher-agent` 装一个 agent，`gpm remove` 卸掉，与装任何软件同一条路径。"安装一个 AI 能力"不再是特殊事件。
3. **记忆 = 子图**：对话历史、习得事实、用户偏好是带 provenance 边的持久节点。短期记忆 = provisional 节点（会话级，重启即逝——ADR-005 语义白送）；长期记忆 = promote 后的持久节点（依赖 F.5/V3.3 持久化）。"AI 的记忆"与"OS 的状态"第一次是同一种东西，可被同一条 Cypher 审计："这个 AI 关于我记住了什么？"= `MATCH (ai)-[:Memory]->(m) RETURN m`。
4. **自运维**：k-ai 已在清点 `fault_events`；深化为 fault envelope → 装配故障上下文（journal 因果历史 + 故障节点邻域）→ LLM 诊断 → 修复建议 mutations → 操作员批准。GOS 的故障本来就是图事件，诊断上下文不需要爬日志。
5. **语义层**（研究性，不承诺）：嵌入向量作为节点属性、语义相似边——留给 V4 探索，本方案仅记录方向。

## 三、阶段与交付（挂在 V3 时间线上）

**铁律：本方案从属于 V3 计划**——AI.x 切片挂靠 V3.x 阶段执行，不抢占 V3.0（隔离）/V3.2（兼容）的主线资源；继承"每阶 harness + killer demo 否则不合入"。

| 切片 | 挂靠 | 交付 | Killer Demo | 退出判据 |
|---|---|---|---|---|
| **AI.0**（现在，doc 轨） | V2.6 并行 | 本方案 + ADR-017（通路统一 + AI principal）起草 | — | ADR-017 提案待选向 |
| **AI.1**（= V3.1 的 F3 demo 收尾） | V3.1 | 三种子接线：k-chat 改走 `gos_ai_bridge::ask()`；桥协议加 `GMUT:` 帧（host 侧把模型的结构化建议编码为 `CypherMutation`，过不了 `pre_validate` 的服务端丢弃——lib.rs:59-62 注释已预设此行为）；shell 加 `ai approve <i>`/`ai reject <i>` 裁决 `MutationGate` | **说一句话改写 OS**："把主题切到 shoji" → AI 提议 `RebindUse` → 批准 → 下一帧生效，journal 里一条 `b"K_AI"` 记录 | `install_backend` 0-caller 状态终结；Parity harness：AI 路径与 shell 路径产生 byte-identical 的 `AuditedMutation`（除 source 戳）；DryRun/整轮拒绝各有 harness |
| **AI.2** | V3.1-3.2 | L1 装配器（Cypher 结果 + k-ai 事件窗口 + 快照，ADR-018 选向后）；工具注册表图派生，`GTOOL:` 硬编码退役 | AI 正确回答"刚才系统发生了什么"（事件窗口）并列出它此刻可用的工具（= 它的 Grant 边） | 装配器 host harness：合成图 → 确定性 context；工具清单与 Grant 边集合的等价性 harness |
| **AI.3** | V3.2-3.3 | 第一个 watcher agent（订阅一个图区域，变化时经 gate 反应）；agent=子图 harness；记忆子图（F.5 后获得持久性） | **装一个 agent**：`gpm install watcher` → agent 子图出现 → 注入区域变化 → agent 的反应 mutation 带自己的 source 戳出现在 journal | agent 故障注入（越权 mutation 被拒、配额爆）全部静默收敛；重启后长期记忆存活、provisional 短期记忆正确消失 |
| **AI.4** | V3.3+ | 自运维闭环：故障注入 → AI 诊断 → 修复建议；tier 1 本地模型为默认推荐配置 | **OS 解释自己的故障**：杀掉一个模块 → AI 给出因果链（引 journal 证据）+ 修复建议 → 批准 → 系统回到静默 | 诊断上下文装配 harness；修复建议仍 100% 过 gate（无诊断特权路径） |

## 四、安全铁律（威胁模型）

1. **模型永不被信任，gate 是唯一执法者**。整轮拒绝语义已实现（`ask()` 首条非法建议丢弃全轮）；幻觉 mutation 撞上 `UnknownEndpoint`/`UnsupportedMutation` 被机械拒绝——**幻觉在 GOS 里不是安全问题，只是失败的提案**。
2. **AI 创建的节点一律 provisional**（ADR-005）：promote 需要显式 Grant——AI 可以"画"出新结构，但让它"通电"永远是人的决定（直到 Auto 档解锁）。
3. **提示注入是 L1 的主威胁**：图里含不可信字符串（gpm 包名、外来进程输出、未来的文件名）。两道防线：(a) 装配器对 context 内容做 provenance 标注（哪些字节来自不可信源）；(b) **触及 capability 拓扑的 mutation（Grant 边的增删）永远走最高批准档**，不进任何 Auto 白名单——注入最多让模型"想"越权，gate 让它做不到。
4. **`Auto` 档双条件锁**（§L2）：签名 AI runtime + 配额，缺一不开。
5. **LLM 永不在热路径**：模型调用不进 trap handler、不阻塞 rewrite 循环。诚实记录已知债：今天 k-chat 在 `on_event` 里带超时轮询 COM2（`LINE_TIMEOUT ≈ 5s`，lib.rs:90）——AI.1 接线时改为非阻塞分段收集（轮询多次 on_event 之间让出），此债不带入 AI.1 之后。
6. **治理脚本红线**（机械化 Parity）：AI 系 crate（k-chat/k-ai/gos-ai-bridge 及未来 agent）禁止出现 `apply_mutation` 之外的图写路径调用——与"兼容层禁止旁路资源表"同一条红线的 AI 版。

## 五、风险登记册

| 风险 | 严重度 | 缓解 |
|---|---|---|
| 模型延迟拖垮交互（云端往返秒级） | 中 | 异步分段收集（§四5）；tier 1 本地模型缩短往返；AI 不在任何系统关键路径上——它失联时 OS 完整可用 |
| 提示注入经图内容操纵 AI | 高 | §四3 双防线；记忆子图写入也过 gate（注入不能静默改写长期记忆） |
| host 桥单点（COM2 断 = AI 全失能） | 低 | 本来就是 host-bridged-first 铁律的预期形态；失联降级为"无 AI 的 GOS"，零功能损失 |
| 范围蔓延吞掉 V3 主线 | 高 | §三铁律：AI.x 从属 V3.x；AI.3/4 依赖的 F.5/ADR-014 不为 AI 提前；本方案不新增 V3 退出判据 |
| 本地小模型质量不够（tier 1 体验差） | 中 | 分型协议天然容错：模型差 → 建议被拒得多 → 系统无损；质量是体验问题不是安全问题 |
| 双通路统一期间 B3b 已验证功能回归 | 中 | AI.1 保持 wire 协议向后兼容（`GMUT:` 是新增帧，旧帧语义不变）；B3b 回归 harness 先行 |

## 六、ADR 分叉点

- **ADR-017（gates AI.1）**：AI principal 与通路统一——k-chat×gos-ai-bridge 接线的具体形状（谁持有 `MutationGate` 实例、`GMUT:` 帧编码、shell 批准 UI、k-ai 的事件窗口归属）；AI 节点的 `NodeSpec.permissions` 与 Grant 边初始集（AI 默认能看什么、调什么）。
- **ADR-018（gates AI.2）**：上下文装配与预算——子图选择策略（k 跳邻域 vs 订阅区域 vs 全局摘要的配比）、provenance 标注格式、与 ADR-012 `FastPathSnapshot` 的声明关系。
- **ADR-019（gates AI.3 记忆半部，远期）**：记忆子图的 schema、provenance 边、GC/衰减策略——依赖 F.5 选向落地后再起草。
- **不需要新 ADR 的**：agent=子图（ADR-014 的直接推论）、agent=包（ADR-016 的直接推论）、批准层级（gos-ai-bridge 已定义）——这三处只需在各自 ADR 选向时确认"AI agent 是其适用对象"。

## 七、一句话总结

GOS 不是"要做 AI 化"的 OS——它的图基底从第一天起就是 AI 的理想接口形状；本方案做的，是把三个已经存在但互不相连的种子（活的 k-chat、分型的 gos-ai-bridge、清点事件的 k-ai）接成一条满足 Parity 不变式的通路，然后让"agent、记忆、自运维"全部作为既有图机制的组合自然长出——**AI 化的深度，等于图抽象的深度**。

## 八、AI.1 分阶段实施分解（2026-06-12）

承接 §三 AI.1 行的判据，按"harness 先行 → 核心接线 → 操作面 → 等价性收口 → 集成"顺序拆成六个可独立验证的切片。

**新发现**（grep 全 crates 确认）：`gos_cypher_mut::apply_mutation` 与 `gos_runtime::RuntimeDispatcher` 全仓库**零生产调用**——今天 k-cypher 的 `CREATE (` 分支（[k-cypher/src/lib.rs:577](../crates/k-cypher/src/lib.rs)）直接调 `gos_runtime::create_provisional_node()`，完全绕开 H.1 的通用 4-变体分发；`RuntimeDispatcher`（[gos-runtime/src/lib.rs:1857](../crates/gos-runtime/src/lib.rs)）自定义后从未被实例化。这意味着 **AI.1d 不只是接 AI——它是 H.1 通用 dispatch 路径的第一次产线激活**，与"scaffolding ahead of wiring"系列（ADR-005 的 provisional、ring3 的 0-caller）同型收口：H.1/H.2 不是要新建什么，是要把已经写好但从未跑过的代码接上电。

| 切片 | 目标 | 触及文件 | 验证 |
|---|---|---|---|
| **AI.1a** B3b 回归 harness 先行 | 把今天 `collect_bridge_response`/`collect_text_only`/`execute_tool`（[k-chat/src/lib.rs:352-476](../crates/k-chat/src/lib.rs)）的精确行为——`GRESP:` 累积+`\n`分隔、`GTOOL:` 内联执行+`GRSLT:` 回传、`GDONE:` 终止、超时即终止——固化为基线 harness。AI.1c 反转前必须存在且绿（ADR-017 门禁①） | 新增 host-side harness：确定性 stub 喂固定 `GRESP`/`GTOOL`/`GDONE` 序列，断言 `resp_buf` 内容与 `GRSLT` 输出序列 | `cargo test` 绿，作为 AI.1c 后重跑的基线 |
| **AI.1b** `MutationGate` 落地 + `GMUT:` 编解码 | `MutationGate`（纯 logic struct，[gos-ai-bridge/src/lib.rs:155-233](../crates/gos-ai-bridge/src/lib.rs)）升级为模块级 `Mutex<MutationGate>` static + 薄存取 API（mirror `BACKEND: Mutex<Option<LlmBackend>>`，lib.rs:116）；按 ADR-017 §1.2 语法实现 4 变体 `GMUT:` 帧 `encode`/`decode` | `crates/gos-ai-bridge/src/lib.rs`（新增 static + accessor fns + 编解码函数，no_std 可 host 测） | 单测：4 变体编解码 round-trip；`decode`→`pre_validate` 全 `Ok`；畸形帧→`Err`（非 panic） |
| **AI.1c** k-chat `LlmBackend::query()` + `on_event` 反转 + 阻塞债清偿 | ADR-017 §1.1 反转：串口收发下沉为 `query()` 实现；`on_event` 聊天分支改调 `gos_ai_bridge::ask()`；`LINE_TIMEOUT=50_000_000`（[lib.rs:90](../crates/k-chat/src/lib.rs)，`com2_read_line` 单次最长 ~5s 阻塞）改为跨多次 `on_event` 的非阻塞分段状态机——§四5 安全铁律记录的债在此清偿，不带入 AI.1 之后（ADR-017 门禁，二选一中选"清偿"） | `crates/k-chat/src/lib.rs`（新增 `query()` extern "C" fn + `ChatState` 状态机字段 + 初始化时 `install_backend`）；`tools/chat-bridge.py`（模型结构化建议→`GMUT:` 帧，host 侧预过滤至 receptive subset） | AI.1a harness 重跑仍绿（旧 `GCHAT`/`GRESP`/`GTOOL`/`GDONE` 帧语义不变，只是调用方向反转）；新增"单次 `on_event` 预算内不跑满 `LINE_TIMEOUT`"用例 |
| **AI.1d** `k-shell ai approve/reject/pending` + `apply_mutation` 首次产线落地 | mirror `proc.rs` 既有 `cmd ==` 分支（如 614 行 `theme`、949 行 `clear`）新增 `ai pending` / `ai reject <i>` / `ai approve <i>`；`approve` 对 `accept_index(i)` 取出的 `CypherMutation` 调用 `gos_cypher_mut::apply_mutation(&mut gos_runtime::RuntimeDispatcher, mutation)`——见上方新发现，这是该函数的第一个产线调用点；成功后构造 `AuditedMutation{mutation, source: b"K_AI", tick}` 送入既有 control-plane envelope 队列（`to_envelope()`） | `crates/k-shell/src/proc.rs` | host harness：4 变体各一条端到端——approve 后 `RuntimeDispatcher` 侧图状态变化 + `AuditedMutation.source == b"K_AI"` |
| **AI.1e** Parity harness + 治理红线 | ADR-017 退出判据：同一 `CypherMutation`，`source` 分别为 `b"K_SHELL"`/`b"K_AI"`，`to_envelope()` 输出除 source 派生字节外 byte-identical；`DryRun` 零 enqueue；整轮拒绝（一条非法建议毒化全轮）各一用例；`tools/verify-graph-architecture.ps1` 新增规则——`crates/k-chat`、`crates/k-ai`、`crates/gos-ai-bridge` 及未来 `agent` 系 crate 中出现 `gos_runtime::` 写路径直调（`apply_mutation` 链除外）即 CI 红 | gos-protocol-harness 或 gos-cypher-mut tests + `tools/verify-graph-architecture.ps1` | `cargo test` + governance 脚本全绿 |
| **AI.1f** 集成验证 + F3 demo + 文档收尾 | 全量 build+harness+governance+QEMU boot-smoke；COM2 端到端："把主题切到 shoji" → `ask()` 返回含 `RebindUse` 建议 → `ai pending` 显示待批 → `ai approve 0` → 下一帧主题切换 + journal 一条 `b"K_AI"` 记录——**F3 killer demo 本体（"说一句话改写 OS"）** | 无新增代码；更新本文件 §三 AI.1 行状态 + `gos_roadmap_direction.md` | 全 pipeline 跑通，日志留存 |

**门禁继承**（ADR-017 §三，逐切片落实）：AI.1a 必须先于 AI.1c 完成且绿；AI.1c 的锁纪律（`query()` 在 `Mutex<MutationGate>` 锁外执行，结果在锁内 `enqueue`）在代码审查时核对；AI.1e 是 AI.1 的硬退出判据，不可与 AI.1f 合并跳过。

## 九、AI.2-4 形状预览（暂不展开）

AI.1 完成后，AI.2-4 各自第一步的大致形状——细节留给各自的 ADR-018/019 选向后再做 §八 式分解，此处仅记录依赖链，避免下一轮重新定向：

- **AI.2 第一步**（依赖 ADR-018）：L1 装配器从"k-ai 计数器"升级为"有界环形事件窗口"——这是 ADR-018 唯一要选向的格式问题，选定后可独立于 AI.1 其余部分实现；工具注册表从 `GTOOL:` 硬编码迁移到 `MATCH (ai)-[:Grant*]->(cap:Capability)` 派生，是 AI.1d 之后的纯增量（`approve` 路径不变，只是"建议从哪来"变了）。
- **AI.3 第一步**（依赖 ADR-014 选向 + V3.0 k-wasm 落地）：第一个 watcher agent 作为"agent=子图"的具体实例——本质是 ADR-014 选向后"装一个 `.gosmod`"流程的第一个真实包，AI.1 的 `MutationGate`/`apply_mutation` 路径直接复用，无新接线。
- **AI.4 第一步**（依赖 AI.2 事件窗口 + 一个故障注入 harness）：故障 envelope → 诊断 context（journal 因果链 + 故障节点邻域，复用 AI.2 装配器）→ 修复建议 mutations → AI.1d 的 `approve` 路径——**AI.4 不新增任何 mutation 通路，只是 AI.1 通路的另一个调用者**。
