# GOS V3 分阶段开发计划 — 生态・兼容・前沿

> 状态：提案 · 日期：2026-06-12 · 配套：[V2 计划](V2_DEVELOPMENT_PLAN.md)（V2.6 为本计划入口门）、[ADR-001 边代数宪法](../doc/ADR-001-edge-algebra-constitution.md)、[ADR-005](../doc/ADR-005-node-mutation.md)（provisional nodes，已选向 A）、[ADR-010](../doc/ADR-010-f5-persistent-storage-path.md)（F.5 三段拆解）
>
> V2 计划把 GOS 推到"产品级涌现式图论 OS"；本计划回答下一个问题：**怎么让它成为最先进的操作系统、长出生态、并能跑别人的软件**。三个目标不是三条并行愿望，而是一条因果链：先进性来自图抽象的代差 → 代差要有人用才算数（生态）→ 生态冷启动靠"别人已有的软件直接能跑"（兼容）。
>
> **继承产品级铁律**：每阶必带 harness 一同合入，每阶必有一个 killer demo——"系统做了一件没人显式编码的事"。

## 〇、不变式

**Prime Invariant（继承 V2）**：Quiescence——signal queue 空 ∧ rewrite queue 空 → 静默。

**V3 新增 Parity Invariant（同一性不变式）**：人类 shell、AI 建议（H.2）、包安装器、外部模块、兼容层里的外来进程——**所有来源对图的一切变更走同一条 mutation gate（`gos-cypher-mut::pre_validate` → `MutationGate` → `apply_mutation`）与同一个 capability 检查（`capability_check`，V2.4）**。不存在第二条特权路径。`AuditedMutation.source` 已为此预留（`b"K_SHELL"` / `b"K_AI"` / "future external admin tools stamp their own id"——包安装器与兼容层就是那个 future）。这条不变式同时是安全模型与"没走偏成 Unix 克隆"的机械判据。

## 一、"最先进"的可度量定义

口号便宜，证据贵（继承 V2 §四）。"最先进"不是功能数量对标 Linux——那条路 50 年也追不上，而是**抽象代差**：Plan 9 的 *everything is a file* → GOS 的 *everything is a graph rewrite*。五条前沿判据，每条都是"其他 OS 做不到/没做过"且可机械验收：

| # | 前沿判据 | 现状 | 落地阶段 |
|---|---|---|---|
| F1 | **OS 全态即活图**：整个运行中系统是一张可 Cypher 查询、可 Cypher 变更、实时 3D 渲染的图 | ✅ 已达成（`k-cypher` + `gos-cypher-mut` + `k-vk-host::render_live_graph`，V2.5） | 已有，V3 扩大可变更面 |
| F2 | **能力即拓扑**：权限 = Grant 边可达性，revoke = 删边，热插拔/故障隔离 0 行 | 原语已证（V2.4a-c harness），端到端 demo 待 B.4.6 真隔离 | V3.0 |
| F3 | **AI 原生**：LLM 经与人类相同的 mutation gate 改写 OS 自身的图 | 管线已存在（`gos-ai-bridge` H.2：`LlmResponse.mutations` → 同一 `pre_validate`/`MutationGate`） | V3.1 端到端 demo |
| F4 | **外来代码 0 行沙箱**：外来程序的每次资源访问都是一次图可达性查询，无专用安全代码 | 未开始（本计划兼容支柱） | V3.2 |
| F5 | **位置透明图**：一条 Cypher 跨多台机器 MATCH；node 迁移后外部 capability 照常工作 | 类型系统已就绪（`gos-cluster` H.3：`HostId`/`RemoteVector`/`RemoteTransportVTable` 等真实总线） | V3.4 |

**诚实清单（不回避）**：GOS 当前缺 SMP、抢占式公平调度、真机显示、可写存储、内存隔离成熟度——这些是 table stakes，不是前沿。V3 的态度：**用兼容层让 table-stakes 差距不阻塞采用**（别人的软件带着别人的生态来），table stakes 本身按 V2.6 硬化轨 + V3.3/V3.4 渐进补齐，性能逃生舱走 fast-path 标签节点（ADR-012 待起草，任务 #43）。

## 二、阶段总览

入口门 = V2.6 完成判据（产品级 V2.0 发布）。**doc 轨（ADR-014/015/016 起草）可立即并行启动**，不等 V2.6——与 V2.4d/e 在 V2.4 期间预研 ADR-007/008 同型。

| 阶段 | 主题 | 核心交付 | Killer Demo | 退出判据 |
|---|---|---|---|---|
| **V3.0**（4-6 周） | 外部代码执行线（E.3 完成） | B.4.6.x 收尾：`.gosmod` 映射进隔离地址空间（`k_vmm::create_isolated_address_space` 接通）+ 第一次 ring3 运行（`ring3.rs` 的 MSR 已编好，0 caller 状态终结）+ G.2 签名强制（`gos-sign`/`gos-verify` 接入 install 路径）+ syscall 表 v1（现 4 条 + 图查询/变更 syscall，见附录 C） | **热安装 0 行**：仓库外编译的 `.gosmod` 运行时安装，下一帧出现在 3D 图视图，capability 经 Grant 边可调；全程零内核重编译 | 第一个 ring3 模块在 QEMU 端到端跑通 + 故障注入（坏 ELF/坏签名/越权 syscall）全被拒并静默；harness：loader/reloc/syscall 回归 |
| **V3.1**（3-4 周） | SDK & 包模型 | `gos-sdk`（仓库外 cargo 模板，pin 住 `gos-protocol` 版本）+ `gpm`（图包管理器，host 侧先行）+ 包格式 = 子图（ADR-016：manifest.cypher + .gosmod + 签名，见附录 B）；F3 demo 收尾：自然语言 → H.2 gate → 图变更可视 | **卸载即子图删除**：`gpm remove` = `MATCH (pkg 子图) DETACH DELETE`，系统达静默，无卸载器框架代码 | 外部目录从模板到安装跑通全流程；install/uninstall/upgrade 均为 Cypher 重放经同一 mutation gate（Parity 不变式 harness 验证） |
| **V3.2**（6-10 周） | 兼容层 #1（[ADR-014](待起草) 选向；建议 WASI-on-graph，见附录 A） | 进程=子图模型（ADR-014 核心）：外来进程是一个子图，fd=边，每次 WASI 调用 → capability_check + 图遍历；`k-wasm` 解释器（`no_std`，host harness 先行可独立测）作为一种 node executor | **外来代码 0 行沙箱**（F4）：未修改的 `wasm32-wasi` 二进制直接跑；在 3D 视图实时看到它的文件访问 = Grant 边；`MATCH ... DELETE` 那条边 → 程序实时收到 permission denied——无一行 wasm 专用安全代码 | 任意语言编译的 hello/cat 级 wasi 程序未修改运行；解释器 fuzz harness 绿；Parity 不变式对外来进程成立 |
| **V3.3**（4-6 周） | 持久生态（F.5 全段 + M3"持久 OS"） | [ADR-010](../doc/ADR-010-f5-persistent-storage-path.md) 三段全落：F.5-logic（FAT32 write/journal fsync，合成 ramdisk harness）→ F.5-wiring（首个真实 `BlockDeviceVTable` 后端 + boot mount/replay）→ F.5-graph-integration（`EdgeAttrs::persistent` 首次为真，promote 状态机接 ADR-005 遗留 (2)） | **重启后图自愈**：`gpm install` → 重启 → journal replay 让包子图原样重现；provisional（未 promote）节点正确地*不*复活 | 拔电 journal 完整（F.5 原始判据）；持久/临时语义 harness 全覆盖；包安装跨重启留存 |
| **V3.4**（6-8 周） | 网络与分布式图 | `k-net`（e1000 寄存器级驱动 + virtio 探测 + tcp 模块已存在 1316 行，成熟度盘点后接入 runtime 图：socket=节点、连接=Stream 边）+ `gos-cluster` 的 `RemoteTransportVTable` 接真实传输 → 跨机 `RemoteVector` 解析 | **跨机 MATCH**（F5）：两个 QEMU 实例，一条 Cypher 跨主机查询；迁移一个 node 到另一台机器，外部持有的 capability 照常工作（V2 demo #3 的跨机版） | 跨机查询/迁移 harness（双实例脚本化）；网络故障注入后系统仍收敛到静默 |
| **V3.5**（6-10 周） | 兼容层 #2：POSIX-lite 移植面（依 ADR-014 选向） | 薄 libc（优先复用 wasi-libc 路径或最小自研）映射到 syscall 表 v2；进程=子图模型复用 V3.2 的同一套；移植 1-2 个真实世界 C 程序（候选：lua / sbase 子集） | **重编译即移植**：未修改源码的真实 C 程序 recompile 后运行，其 syscall 轨迹在图视图可见 | 目标程序自测套件通过；无绕过 capability_check 的旁路（治理脚本红线） |
| **V3.6**（4 周） | 发布与社区 | 文档站（架构导览 + SDK 教程 + ADR 索引）；ABI 稳定性声明（ADR-015）；可下载 demo 镜像（installer 已有）；CONTRIBUTING + 包提交流程 | **陌生人测试**：一位从未接触项目的开发者仅凭文档独立写出并安装一个包 | 文档站上线；陌生人测试 ≥1 次真实通过；V3.0 发布公告 |

依赖链：**V3.0 → V3.1 → V3.2 → V3.3 → … 非严格串行**——V3.3（F.5）只依赖 V2.6 的 ADR-010 选向，可与 V3.0-2 并行（继承 V2 计划 F.5 并行轨）；V3.4 的 k-net 盘点也可提前。严格串行的只有：V3.0 → V3.1 → V3.2（兼容层必须踩在真实隔离 + SDK/ABI 冻结之上）与 V3.2 → V3.5（POSIX 面复用进程=子图模型）。

## 三、三大支柱与阶段的映射

- **前沿（最先进）**：F1 已有；F2→V3.0；F3→V3.1；F4→V3.2；F5→V3.4。五条全绿之日，"Plan 9 之后第二个新计算抽象"的主张第一次有完整证据链。
- **生态**：V3.0（能跑外部代码）→ V3.1（好写、好装、好卸）→ V3.3（装了不丢）→ V3.6（有人来装）。生态的本体是 `.gosmod` + 包=子图，**不是**兼容层——兼容层是获客渠道。冷启动自举：把现有 builtin 中非内核必需的（`k-chat`、`k-nim` 游戏等）改造为第一批 out-of-tree 包，SDK 的第一个用户是我们自己。
- **兼容**：V3.2（WASI：一次实现，所有编译到 wasm 的语言生态即得）→ V3.5（POSIX-lite：真实 C 软件移植面）。**明确非目标（V4+ 再议）**：Linux 二进制级兼容（syscall 翻译层，Starnix/Linuxulator 路线）、裸金属 VM guest——两者工程量都在"再造半个 OS"量级，且在 WASI+POSIX-lite 覆盖 80% 需求前没有性价比。

## 四、风险登记册

| 风险 | 严重度 | 缓解 |
|---|---|---|
| **兼容层把身份拖成 Unix 克隆** | 致命 | Parity 不变式 + 铁律 3（兼容层必须是图的消费者）；ADR-014 必须先给出"进程=子图、fd=边"的完整映射才准写实现代码；治理脚本加红线：兼容层 crate 禁止直接持有资源表 |
| ABI 冻结过早（背包袱）/过晚（生态没法建） | 高 | ADR-015 先行：`gos-protocol` 语义化版本 + syscall 号"永不复用"（`ring3.rs` 已承诺）+ 明确"V3.0-V3.2 期间 ABI 为 beta，V3.6 冻结 v1" |
| wasm 解释器工程量失控 | 高 | `no_std` 纯逻辑 = 本项目 harness 文化的完美适配对象，host 侧先行全覆盖再进内核；预设逃生门：若 8 周未达 hello 级，降级为"WASI 推迟，V3.5 POSIX-lite 提前"（ADR-014 写明 plan B） |
| 外部代码执行的安全面 | 高 | G.2 签名强制先于 SDK 公开（V3.0 内完成）；故障注入 harness（坏 ELF/越权/配额爆）是 V3.0 退出判据的一部分，不是事后补 |
| 生态冷启动失败（没人来） | 中 | 自举（builtin 改包）保证生态机制先于社区被真实使用；V3.6 陌生人测试是硬判据 |
| 单人带宽 | 中 | 每阶切片化（继承 V2.5a-e 模式）；doc 轨 ADR 先行让"选向"与"实现"解耦、可批量决策 |

## 五、sequencing 铁律

1. **E.3（第一次 ring3 运行）先于一切兼容层实现**——没有真实隔离，沙箱 demo 是表演。
2. **ADR 先于实现**（继承）：ADR-014（进程=子图 + 兼容策略选向）先于任何 wasm/libc 代码；ADR-015（ABI/版本政策）先于 SDK 对外发布；ADR-016（包=子图格式）先于 `gpm` 实现。
3. **兼容层必须是图的消费者**：外来进程是子图、资源访问走 `capability_check`、变更走 mutation gate——禁止任何旁路状态表。这是 Parity 不变式的实施细则。
4. **host-bridged-first 继承**：新硬件面（网络真实传输、真机显示）先 host 桥接验证语义，再真驱动。
5. **每阶 harness + killer demo 强制**（继承，无例外）。
6. **Linux 二进制兼容 / 裸金属 VM 是 V3 非目标**——出现在任何 V3 PR 里都是 scope 蔓延。
7. **V2.6 未完成项不被 V3 吞并稀释**：ADR-006 选项 B、ADR-007/008 选向、ADR-011/012/013 起草（任务 #42-44）、installer 真机验证（#45）保持独立追踪。

## 六、立即可做（doc 轨，与 V2.6 并行）

1. **ADR-014**：进程=子图模型 + 兼容策略选向（WASI-first vs POSIX-native-first vs 推迟，含 plan B）——V3 最大的分叉点，建议最先起草。
2. **ADR-015**：ABI 稳定性与版本政策（`gos-protocol` semver、syscall 编号、`.gosmod` 格式版本、beta→v1 冻结时间线）。
3. **ADR-016**：包=子图格式（manifest.cypher 语法、签名布局、`gpm` 与 mutation gate 的接线）。

## 附录 A — ADR-014 选项预览（兼容策略）

- **选项 A：WASI-first（倾向）**。`k-wasm` 解释器（`no_std`）作为一种 node executor；WASI 的 capability 句柄模型与 Grant 边**同构**（wasi 本来就是 capability-based——天作之合，映射是自然的而非削足适履）；一次实现获得 C/Rust/Go/Zig/... 全部 wasm 生态；解释器纯逻辑可 host harness 全覆盖；不依赖 ring3 成熟度（解释执行天然内存安全，ring3 隔离是纵深防御而非唯一防线）。代价：解释执行慢（fast-path 标签节点是后续答案）、解释器本体 5-10k 行。
- **选项 B：POSIX-native-first**。直接扩 syscall 表到 POSIX 子集 + 薄 libc。代价：表面积巨大（fork/exec/signal 语义与图模型冲突点多），身份漂移风险最高。
- **选项 C：双轨推迟**。只做 ADR 与映射设计，实现等 V3.0/3.1 反馈。代价：兼容支柱空转，生态冷启动少一条腿。
- 推荐 A，B 的核心成果（进程=子图映射）在 A 中同样要做且可复用于 V3.5。

## 附录 B — 包=子图格式草图（ADR-016 输入）

```
hello-pkg/
├── manifest.cypher      # MERGE (p:Package {name,ver}) + 节点/边声明 + capability 需求（IMPORTS/EXPORTS，
│                        #  与现有每文件 Cypher 拓扑头同一词汇——k-net 等 crate 已示范该约定）
├── bin/hello.gosmod     # ET_DYN ELF（gos-loader B.4.6 管线既有格式）
└── signature            # gos-sign（G.2）
```

`gpm install` = 验签 → 重放 manifest.cypher 经 `gos-cypher-mut` gate（source 戳 `b"K_GPM"`）→ `.gosmod` 经 loader 安装 → 子图出现在 3D 视图。`gpm remove` = `MATCH (p:Package {name}) ... DETACH DELETE`。审计、journal 持久化（V3.3 后）、AI 可见性全部免费——因为走的是同一条 gate。

## 附录 C — syscall 表 v1 草图（ADR-015 输入）

现有（`ring3.rs`，编号永不复用）：`0x01 AllocPages`、`0x02 FreePages`、`0x03 EmitSignal`、`0x04 ResolveCapability`。V3.0 增补候选（图即系统调用面——这是"Cypher 是 ISA"在用户态的延伸）：

- `0x05 GraphSnapshot`（`node_page`/`edge_page` 只读分页，k-vk-host 已内核侧验证的同一读路径）
- `0x06 SubmitMutation`（`CypherMutation` 批量提交，经 mutation gate，带 source 戳）
- `0x07 SubscribeRegion`（注册 `Subscription`，外部模块从此能挂进反向传播——V2.3 机制的用户态出口）

外来进程不需要更多：文件/网络等一切资源访问最终都是"沿 Grant 边的 capability 调用"，这正是 F4 demo 的机制本体。
