# ADR-016：gpm 包格式——Appendix B 的草图引用了一个比 H.1 receptive subset 更大的 Cypher 方言

> 状态：**提案待选向** · 提案日期：2026-06-12 · 配套：[V3 计划 Appendix B](../plan/V3_DEVELOPMENT_PLAN.md#附录-b--包子图格式草图adr-016-输入)（"包=子图格式草图，ADR-016 输入"）、[gos-cypher-mut](../crates/gos-cypher-mut/src/lib.rs)（H.1 receptive mutation subset）、`crates/k-net/src/lib.rs:12-29`（Appendix B 引用的"现有每文件 Cypher 拓扑头"precedent）、[gos-loader](../crates/gos-loader/src/lib.rs)（`.gosmod` ET_DYN ELF，B.4.6 既有）、`gos-sign`/`gos-verify`（签名，G.2 既有）、[ADR-015](./ADR-015-abi-stability-versioning-policy.md)（minor-bump checklist——本 ADR 选项 B 若选中，会是该 checklist 的第一个真实用例）
>
> 口径：V3 计划 Appendix B 给 ADR-016 准备了一个看起来相当具体的输入草图——`hello-pkg/{manifest.cypher, bin/*.gosmod, signature}`，并声称 `manifest.cypher` 与"现有每文件 Cypher 拓扑头同一词汇——k-net 等 crate 已示范该约定"。调查后发现：这个草图里**三个组件中两个是真实存在的既有机制**（`.gosmod` ET_DYN ELF = B.4.6，`signature` = G.2 gos-sign/verify），但**第三个——`manifest.cypher` 的方言——其"已示范"的 precedent 本身是从未被解析过的 `//` 注释**，而 `gos-cypher-mut`（H.1，唯一的"重放经 gate"入口）的 receptive subset 既不包含 `MERGE`，也不包含 Appendix B 点名的 `IMPORTS`/`EXPORTS`/`DEPENDS_ON` 等边类型，更**明确地、有意地拒绝**`gpm remove` 所需的 `DETACH DELETE`（节点删除）。本 ADR 把"包=子图格式"按这三个组件的真实成熟度拆开定价。

## 一、问题陈述

### 1.1 Appendix B 三组件现状对照

```
hello-pkg/
├── manifest.cypher      # 新——方言未定义
├── bin/hello.gosmod     # 既有：ET_DYN ELF + R_X86_64_RELATIVE + dynsym module_init/event/stop（B.4.6）
└── signature            # 既有：gos-sign（G.2）
```

`bin/*.gosmod` 和 `signature` 都是**已经在跑的格式**——`gos-loader` 的 ELF 加载管线（B.4.6）与 `gos-sign`/`gos-verify`（G.2）在 V2.x 期间已经实现并被内建 bundle 使用。这两项对 ADR-016 而言是"拿来用"，不是"待设计"。真正的空白是 `manifest.cypher`。

### 1.2 "已示范该约定"指向的是一段从未被解析的注释

Appendix B 称 `manifest.cypher` 的词汇——`MERGE (p:Package {name,ver})` + 节点/边声明 + `IMPORTS`/`EXPORTS` capability 需求——"与现有每文件 Cypher 拓扑头同一词汇，k-net 等 crate 已示范该约定"。`crates/k-net/src/lib.rs:12-29` 确实有这样一段头部注释：

```rust
// MERGE (p:Plugin {id: "K_NET", name: "k-net"})
// MERGE (dep_K_VGA:Plugin {id: "K_VGA"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_VGA)
// MERGE (pr_CF8:PortRange {start: "0xCF8", end: "8"})
// MERGE (p)-[:REQUIRES_PORT]->(pr_CF8)
// MERGE (cap_net_uplink:Capability {namespace: "net", name: "uplink"})
// MERGE (p)-[:EXPORTS]->(cap_net_uplink)
// MERGE (cap_console_write:Capability {namespace: "console", name: "write"})
// MERGE (p)-[:IMPORTS]->(cap_console_write)
```

这段注释**确实存在、确实是这个词汇**——但它是 `//` 行注释：**没有任何代码解析它**。它的实际作用是给人类读者和（可能）治理脚本提供"这个 crate 声明了哪些拓扑事实"的可读摘要，与 `PluginManifest.{depends_on,exports,imports}`（gos-loader 真正读取、校验的字段）是**两份独立维护的真相**——`MERGE`/`DEPENDS_ON`/`REQUIRES_PORT`/`EXPORTS`/`IMPORTS` 这些词在 `//` 注释里"已示范"，但从未被"执行"。Appendix B 把"已示范该约定"读成了"已有解析器"，这是一个台阶差。

### 1.3 `gos-cypher-mut`（H.1）receptive subset 的真实边界

[`gos-cypher-mut/src/lib.rs:47-73`](../crates/gos-cypher-mut/src/lib.rs) 的 `CypherMutation` 只有 4 个变体：

| 变体 | 对应 Cypher | 限制 |
|---|---|---|
| `AddEdge { from, to, edge_kind }` | `CREATE (a)-[:Mount\|Use]->(b)` | `edge_kind: ReceptiveEdgeKind`，**只有 `Mount=1`/`Use=2` 两种**（line 80-83）——"Spawn/Call/Return/Sync/Stream are runtime-internal and never user-mutable" |
| `RemoveEdge { edge_id }` | `MATCH (a)-[r:Mount\|Use]->(b) DELETE r` | 同上，限 Mount/Use |
| `RebindUse { from, new_target }` | 主题切换专用的 `Use` 边原子重绑 | — |
| `CreateNode` | `CREATE (n:Label {props})` | **`Label`/`{props}` 存储"isn't wired yet"**（V2.5e 状态）——只分配一个 provisional `NodeId`，`Unbound`/`NodeInstanceId::ZERO` |

crate 顶部文档（line 9-12）说得非常直白：

> "Edge mutations, `Mount`/`Use` rebinds, and ... provisional node creation are accepted. **Node *delete*, NodeId reassignment, and plugin manifest mutation are still explicitly rejected.**"

对照 Appendix B 的三个用法：

1. **`MERGE (p:Package {name,ver})`**——`CypherMutation` 没有 `Merge`（match-or-create）变体，只有无条件的 `CreateNode`，且**不存储 `{name,ver}` 这类 props**。
2. **`(p)-[:IMPORTS\|EXPORTS\|DEPENDS_ON]->(...)` 边声明**——`AddEdge.edge_kind` 的 `ReceptiveEdgeKind` 只有 `Mount`/`Use`，**不包含 `Imports`/`Exports`/`DependsOn`/`RequiresPort`**。
3. **`gpm remove` = `MATCH (p:Package {name}) ... DETACH DELETE`**——`DETACH DELETE` 删除节点（及其边），这正是 H.1 文档**明确列为"explicitly rejected"**的操作，理由是 Phase B 的 claim/quota/`restart_generation` 记账依赖 `NodeId` 稳定——这不是"还没做"，是"刻意不做"。

**三个用法，三个都在 H.1 receptive subset 之外**——其中第 3 个（节点删除）甚至与 H.1 的核心不变式直接冲突，不是简单的"加个变体"就能解决的加法变更。

### 1.4 `gpm` 本身：零代码，但 `source` 戳已经预留了身份

全仓库搜索 `crates/**/gpm*` 无结果——`gpm` 不是任何现有 crate。但 Appendix B 写 `gpm install` 重放 mutation 时"source 戳 `b"K_GPM"`"——[`AuditedMutation.source: [u8;16]`](../crates/gos-cypher-mut/src/lib.rs:96) 是自由格式的 16 字节标签（不是 closed enum，`b"K_SHELL"`/`b"K_AI"` 只是文档举例），所以 `b"K_GPM"` 本身不需要新增任何类型——但"`gpm` 是什么"（新 `k-gpm` executor crate？`k-shell` 的一个子命令、借用 shell 已有的 `apply_*_raw` 模式但换一个 source 戳？host 侧工具？）完全未定义，三种都能"stamp `b"K_GPM"`"，互斥程度不同。

## 二、选项

### 选项 A——`manifest.cypher` 收缩到今天的 receptive subset；`gpm remove` 推迟；V3.1 demo 用最小方言（倾向）

把 Appendix B 拆成"现在能做"和"需要先扩 H.1"两部分：

- **manifest.cypher v0**（V3.1 可用）：每个包声明**一个** `CreateNode`（provisional 节点代表这个包）+ 零或多个 `AddEdge{Mount}`（把包节点挂到一个约定的 `Packages` mount 点下，使其在 3D 视图可见——呼应 Appendix B"子图出现在 3D 视图"）。capability `IMPORTS`/`EXPORTS`/`DEPENDS_ON` 声明**继续写成 `//` 风格的 prose**（与 k-net 头部注释同一约定，治理脚本可读，但不经 gate 重放）——即"包的拓扑头"与"crate 的拓扑头"待遇相同：都是声明性文档,都不是可执行 mutation。
- **`gpm remove`**：V3.1 不实现。Appendix B 的 `DETACH DELETE` 与 H.1"绝不删除节点"的不变式冲突,这本身值得独立讨论（选项 B 备注）,不阻塞"第一个包能装上"这个 V3.1 milestone（V3 计划 V3.1 标准是"first ring3 .gosmod run"——是 install,不是 install+remove）。
- **`gpm` 身份**：作为 `k-shell` 的一个子命令族（`gpm install <path>`），复用 shell 已有的"调用 `gos-cypher-mut::apply_mutation` 并指定 source 戳"模式（`k-shell` 里 `emit_target_signal_raw`/`apply_theme_choice_raw` 已经是"shell 函数→打包 mutation→指定 source"的先例），只是把 source 戳从 `b"K_SHELL"` 换成 `b"K_GPM"`。不新增 crate。

- **优点**：零 `gos-cypher-mut`/`ReceptiveEdgeKind` 改动——manifest.cypher v0 完全用 V2.5e 已经交付的 `CreateNode` + 已有的 `AddEdge{Mount}` 表达,`gpm` 不增加新 crate,V3.1"第一个 `.gosmod` 跑起来"demo 的格式侧零阻塞。诚实标注:"capability 声明暂时仍是 prose"——不会让人以为 gpm 包已经有了真正声明式的依赖解析。
- **代价**：v0 的"包"实质上只是"一个 provisional 节点 + 一条 Mount 边"——比 Appendix B 描述的"manifest.cypher 声明 IMPORTS/EXPORTS"单薄得多;真正的"声明式依赖/能力契约通过 gate"要等选项 B 式的 H.1 扩展,这部分工作被推迟、不在本 ADR 门禁内。

### 选项 B——扩 `ReceptiveEdgeKind` 覆盖 capability/dependency 边，`MERGE` 的"match"半交给 `gpm` 自己查图

在选项 A 的基础上,把 `ReceptiveEdgeKind`（`#[repr(u8)]`，今天只有 `Mount=1`/`Use=2`）加 `Imports=3`/`Exports=4`/`DependsOn=5`（纯加法,不改变既有判别值——[ADR-015 §1.3](./ADR-015-abi-stability-versioning-policy.md) 的 minor-bump checklist 第一个真实用例：新枚举变体、`#[repr(u8)]`、旧值不变→若 `ReceptiveEdgeKind` 被判定属于某条 ABI 轴线,这正是"minor bump"的教科书案例）。`MERGE (p:Package {name,ver})` 的"先 MATCH 再 CREATE-if-absent"语义,不要求 `CypherMutation` 新增 `Merge` 变体——`gpm` 自己先用 `node_page`（ADR-012 的 fast-path 读)按 `name`/`ver` 查找既有 Package 节点,找不到才发 `CreateNode`,语义等价但"match"逻辑活在 `gpm` 调用方,不进 gate 的判别式里。`gpm remove` 仍推迟（不动 H.1 的节点删除禁令）。

- **优点**：`manifest.cypher` 的 `IMPORTS`/`EXPORTS`/`DEPENDS_ON` 边第一次变成**可重放、可审计、可被 AI 看到**的 mutation（Appendix B 承诺的"审计、journal 持久化、AI 可见性全部免费"对这部分才成立）——选项 A 这部分仍是 prose。同时是 ADR-015 minor-bump 规则落地后的第一个真实案例,为后续类似变更（如 ADR-012 的 `PermissionKind::FastPathSnapshot`）立一个先例。
- **代价**：触碰 `gos-cypher-mut` 的 H.1 receptive subset——这是一个被文档明确论证过"刻意收窄"的边界,扩展前要确认"声明 Imports/Exports/DependsOn 边"不会引入 H.1 想避免的那类问题（这些边连接的是 Package 节点与 Capability/Plugin 节点,不触碰 Phase B 的 `NodeId`/claim/quota 记账——初步看与"绝不删除节点"的理由不冲突,但需要在落地时复核）。比选项 A 多一轮 `gos-cypher-mut` 的设计/审查。

### 选项 C——`manifest.cypher` 不走 Cypher 文本，改用 Appendix B 之外的序列化格式（如直接是 `PluginManifest` 的某种文本投影）

放弃"`manifest.cypher` = Cypher 文本"这个前提本身——既然 `gos-loader` 已经有 `PluginManifest`（含 `depends_on`/`imports`/`exports`/`permissions` 等字段，§1.2 提到的"两份真相"之一），`manifest.cypher` 可以是这个结构体的某种文本序列化（TOML/RON/...），`gpm install` 直接反序列化为 `PluginManifest`-shaped 数据交给 loader,完全不经过 `gos-cypher-mut`。

- **代价**：与"Parity Invariant"（每个 mutation 来源——shell/AI/gpm/外部进程——都走同一 `gos-cypher-mut` gate)直接冲突——这是 V3 计划新写下的不变式,gpm 是 Parity Invariant 列出的四个来源之一。选 C 等于 gpm 成为第二个特权路径,正是 V3 sequencing 铁律第 3 条明确禁止的"旁路状态表"的 manifest 版本。仅在选项 A/B 因某种未预见原因不可行时才考虑。

## 三、建议与门禁

倾向 **A**，并把 **B 列为 A 之后的自然第二步**（不是互斥选项，是排序）：A 让 V3.1 的"第一个 `.gosmod` 包安装"demo 在零 `gos-cypher-mut` 改动下成立——`manifest.cypher v0` = 一个 `CreateNode` + 若干 `AddEdge{Mount}`，capability 声明继续是 k-net 同款 prose；`gpm` 是 `k-shell` 新增的一个子命令族，复用既有"function → mutation → source 戳"模式，`b"K_GPM"` 替换 `b"K_SHELL"`。**A 完成后**，`ReceptiveEdgeKind` 扩 `Imports`/`Exports`/`DependsOn`（选项 B）把 capability 声明从 prose 升级为可重放 mutation——这一步可以独立于 A 排期，且是 ADR-015 minor-bump checklist 的第一个练习对象。`gpm remove`/`DETACH DELETE` 与 H.1"节点不可删除"不变式的冲突，本 ADR 不裁决，记录为"已识别、需要 H.1 自己的后续设计"——结构上类似 ADR-013 把 UEFI GOP 拆给独立的 bootloader 迁移 ADR。

**门禁**：
- A 范围内的工作（`manifest.cypher v0` 的具体文本语法——多大程度复用真实 Cypher `CREATE`/`CREATE ... -[:Mount]->...` 语法 vs 进一步简化为固定模板，`k-shell` 的 `gpm install/list` 子命令）可在选向后随时开始，建议配一个"安装一个最小 hello-pkg、子图出现在 `render_live_graph` 输出里"的 host harness（mirrors V2.5c 的"确认已交付"验证手法）作为 V3.1 milestone 的 killer demo 素材。
- B（`ReceptiveEdgeKind` 新变体）排期在 A 之后，落地前应有一个一句话的 ADR-015 式判定记录（"`ReceptiveEdgeKind` 是否属于 `GOS_ABI`/`MODULE_ABI` 任一轴线"——若 gpm/未来外部模块直接构造 `CypherMutation` 值跨编译边界，则属于；若 `gos-cypher-mut` 的调用方始终与内核同编译，则不属于、minor-bump checklist 不强制但仍是良好实践）。
- `gpm remove`/节点删除不在本 ADR 门禁内——明确记录为"暂不支持"，避免 V3.1 demo 范围蔓延到一个与 Phase B 不变式冲突、需要独立设计的问题。
- 选项 C 不落地，但其指出的"两份真相"（k-net 头部 `//` 注释 vs `PluginManifest.{depends_on,imports,exports}`）观察值得记录：本 ADR 选 A/B 后，`manifest.cypher` 的 capability 声明最终目标是成为**第三份**与前两者一致的真相来源——三者长期应通过治理脚本互相校验，而非各自维护，但这是 V3.1 之后的收尾，不在本 ADR 门禁内。
