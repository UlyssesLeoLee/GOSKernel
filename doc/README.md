# GOS 文档索引

> 按日系软件工程生命周期分层：项目管理 → 需求定义 → 基本设计 → 详细设计 → 实施计划 → 测试验证 → 运维维护  
> 每个阶段提供 Markdown 原文 + Excel 结构化版本（中文）

**文档管理规范**：本次优化起，各阶段核心 Markdown 文档统一在标题下方附带文档管理票（文档编号 / 版本 / 状态 / 作成・审核・批准 / 变更履历），Excel 文档统一附带「文档管理」首页 sheet。硬化日志系列（06_运维维护/hardening/）作为历史记录，其内容不回溯改写事实，仅做归位与中文化；`doc/` 根目录保留的同名旧文件是硬化当时的原始存档，不做删除，仅供追溯对照。

**本轮优化摘要（2026-07-21）**：

- 硬化日志系列自上一轮（2026-07-19，索引截至 V3.65）以来已持续推进至 **V3.104**（V3.66~V3.104 共 39 次硬化迭代，其中 **V3.66、V3.102 两版本文件缺失**——在 `06_运维维护/hardening/` 与 `doc/` 根目录均未找到实体文件，仅能从相邻版本的交叉引用中还原其内容，本轮如实记录该缺口，不做内容臆造），下表已补齐全部可核实的索引条目
- 扫描全部 37 篇实体新硬化日志的 CJK 字符密度，发现 **16 篇为纯英文**（V3.77/78/84/85/86/87/88/89/90/93/94/95/97/98/99/100），本轮已逐篇核对源码引用与测试数据后翻译为中文，不改动版本号/文件路径/函数名/测试名/测试计数
- **处理上一轮标记的高优先级待跟进事项**：[GRAPH_CLI_COMMANDS_zh.md](03_详细设计/GRAPH_CLI_COMMANDS_zh.md)（v1.7 → v1.8）对照硬化日志补齐 V3.66~V3.104 新增的 Neighborhood S-variant 拓扑指数命令族 `graph topo55`~`graph topo93`（39 组、117 个指数），新增 §十五完整索引，原 §十五/§十六 顺延为 §十六/§十七
- [GOS_ARCH_v2.md](02_基本设计/GOS_ARCH_v2.md)（v2.7 → v2.8）§5.1 补充上述命令族概述，指向 GRAPH_CLI_COMMANDS_zh.md §十五 作为权威口径；累计 host tests 由 1623 更新为约 2021
- V3.66~V3.104 区间新增的核心内容是 Neighborhood S-variant 拓扑指数族由 30 次幂顶点/边幂次推进至 67 次幂（依次跨越 triacontic 尾段 → tetracontic 段 → pentacontic 段 → hexacontic 段前 8 个），Sombor 变体 α 由 46 推进至 122（命名法历经第2轮单字母复用、第3轮双字母 "AA"~"AZ"、第4轮双字母 "BA"~"BJ" 三次进位），全部译名并纳入索引
- **新发现两处口径缺陷，本轮均如实标注、未擅自修正**：① V3.66（topo55）、V3.102（topo91）硬化日志文件缺失（详见上文）；② V3.100（topo89）硬化日志自述"宿主测试总数 1963（此前 1953）"，与 V3.99（topo88）自述的累计总数 1963 矛盾（两者不能同时成立，差 10 项，疑似记录笔误）——已在 V3.100 中文译文与 GRAPH_CLI_COMMANDS_zh.md §十五中加注说明
- 标记本轮新发现的待跟进缺口：见文末「待跟进事项」

**上一轮优化摘要（2026-07-19，存档）**：

- 硬化日志系列自上一轮（2026-07-15，索引截至 V3.30）以来已持续推进至 **V3.65**（累计 35 篇新增硬化记录，涵盖 V3.31~V3.65），下表已补齐全部缺口索引条目
- 扫描全部 35 篇新增硬化日志的 CJK 字符密度，发现 12 篇为纯英文（V3.35/36/38/39/40/41/42/47/48/49/50/51），本轮已逐篇核对源码与测试数据后翻译为中文，不改动版本号/文件路径/函数名/测试名/测试计数
- 发现 2 篇硬化日志（V3.60、V3.63）滞留于 `doc/` 根目录且从未归位至 `06_运维维护/hardening/`——本轮已核对内容（原为纯中文，无需翻译）后归档，根目录原文件补充跳转说明
- **处理上一轮标记的高优先级待跟进事项**：[GRAPH_CLI_COMMANDS_zh.md](03_详细设计/GRAPH_CLI_COMMANDS_zh.md)（v1.6 → v1.7）对照源码及硬化日志补齐 V3.31~V3.65 新增的 Neighborhood S-variant 拓扑指数命令族 `graph topo20`~`graph topo54`（35 组、105 个指数），新增 §十四，原 §十四/§十五顺延为 §十五/§十六
- [GOS_ARCH_v2.md](02_基本设计/GOS_ARCH_v2.md)（v2.5 → v2.6）§5.1 补充上述命令族概述，指向 GRAPH_CLI_COMMANDS_zh.md §十四 作为权威口径；累计 host tests 由约 1273 更新为 1623
- V3.31~V3.65 区间新增的核心内容是 Neighborhood S-variant 拓扑指数族的持续扩展：自 topo18（V3.29，邻域 Zagreb 指数 NM₁/NM₂/GA₂）起，将「邻域度和 S(v) = Σ_{w∈N(v)} deg(w)」代入 §13.2 已有的经典/拓扑指数公式，衍生出 Sombor 变体、顶点幂次序列（S²~S¹⁴）、边幂次序列等 35 组新指数，全部译名并纳入索引
- 标记本轮新发现的待跟进缺口：见文末「待跟进事项」

**上一轮优化摘要（2026-07-15，存档）**：

- 硬化日志系列自上一轮（2026-07-03，索引截至 V2.65）以来已持续推进至 **V3.30**（累计 65 篇新增硬化记录，涵盖 V2.66~V3.30），下表已补齐全部缺口索引条目
- 发现 5 篇硬化日志（V3.16 / V3.17 / V3.20 / V3.21 / V3.23）此前只散落在 `doc/` 根目录且为英文，从未归位至 `06_运维维护/hardening/`——本轮已逐篇核对源码与测试数据后翻译为中文并归位，原根目录英文存档保留但补充跳转说明指向中文版
- 发现 3 篇硬化日志（V3.04、V3.28、V3.29）虽已正确归位于 `06_运维维护/hardening/`，但内容仍为纯英文，与「硬化日志统一中文化」规范矛盾——本轮已就地中文化，不改动版本号/文件路径/函数名/测试名/测试计数
- 扫描全部 126 篇硬化日志的 CJK 字符密度后，另发现 3 篇纯英文遗漏（V3.10、V3.11、V3.27），本轮一并中文化，共计翻译 11 篇
- V2.66~V3.30 区间新增的核心内容是**拓扑指数命令族的大规模扩展**：从 `graph topo`（V3.12，SC/GA/AZI）到 `graph topo19`（V3.30，反向 Wiener/RCW/终端 Wiener），累计 19 组、60 余个分子图拓扑描述符（Zagreb 系、Randić 系、距离系 Wiener/Szeged/Harary、离心率系、传输量系、补图指数等），全部译名并纳入索引
- 标记本轮新发现的待跟进缺口：见文末「待跟进事项」

**上一轮优化摘要（2026-07-03，存档）**：

- 硬化日志系列存在中文化缺口：V2.43~V2.49、V2.51~V2.54 共 11 篇此前以英文写入 `06_运维维护/hardening/`，与文档管理规范「硬化日志统一中文化」矛盾。本轮已逐篇核对源码与测试数据后改写为纯中文版，仅译语言，不改动版本号 / 文件路径 / 函数名 / 测试名 / 测试计数等既成事实
- V2.50、V2.55~V2.65 共 12 篇已核对：V2.50 本轮撰写时即为纯中文，无需改动；V2.55~V2.65 共 11 篇为中英双语格式（中文段落在前、英文复述在后），中文内容完整可读，本轮判定满足「中文书写」要求，保留双语格式供代码/API 交叉核对，未做删改
- 下表补齐此前遗漏的 V2.43~V2.65 硬化日志索引条目（此前索引仅收录至 V2.42）
- [GRAPH_CLI_COMMANDS_zh.md](03_详细设计/GRAPH_CLI_COMMANDS_zh.md) 对照 `k-shell` 源码补齐 V2.43~V2.65 新增的图论分析命令族（PageRank/HITS/community/spanning/color/mst/shortest/flow/between/attractor）与属性存储/图健康度命令族（node attr/pal/density/clustering/transitivity/kcore/assortativity）
- [GOS_ARCH_v2.md](02_基本设计/GOS_ARCH_v2.md)（v2.2 → v2.3）更新 §5.1 图论/图健康命令族概述，指向 GRAPH_CLI_COMMANDS_zh.md 作为权威口径
- 核对 [implementation_plan_v0_1_zh.md](04_实施计划/implementation_plan_v0_1_zh.md)、[task_v0_1_zh.md](04_实施计划/task_v0_1_zh.md) 与最新硬化进度（V2.65，623 host tests）的一致性
- 上一轮遗留的「V2.42 硬化日志格式统一」「doc/ 根目录旧文件跳转提示」两项待跟进事项已处理，见下文
- 标记本轮新发现的待跟进缺口：见文末「待跟进事项」

**再上一轮优化摘要（2026-07-02，存档）**：

- 硬化日志系列存在大面积中文化缺口：V2.16~V2.18、V2.21~V2.33、V2.35~V2.41 共 23 篇此前以英文写入 `06_运维维护/hardening/`，与文档管理规范「硬化日志统一中文化」矛盾。本轮已逐篇核对源码与测试数据后中文化，仅译语言，不改动版本号 / 文件路径 / 函数名 / 测试名 / 测试计数等既成事实
- 发现 V2.15（`stat <vec>` 单节点详情命令）硬化日志此前只停留在 `doc/` 根目录（英文），从未归位至 `06_运维维护/hardening/`，本轮已补建中文版 [hardening/HARDENING_LOG_2026-07-01_V2.15.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.15.md)
- [GRAPH_CLI_COMMANDS_zh.md](03_详细设计/GRAPH_CLI_COMMANDS_zh.md)（v1.2 → v1.3）对照 `k-shell` 源码补齐 V2.16~V2.42 新增的 `graph topo`/`graph health`/`plugins`/`kill`/`resume`/`node info`/`node trace`/`node log`/`uname`/`watch` 及 `graph path`/`cycles`/`toposort`/`scc`/`condensation`/`reachable`/`bipartite`/`degree`/`centrality`/`closeness`/`eccentricity`/`katz` 图论分析命令族（新增 §八「进程与拓扑运维扩展」、§九「图论分析命令族」）
- 下表补齐此前遗漏的 V2.15~V2.42 硬化日志索引条目（此前索引仅收录至 V2.19，且中间跳过 V2.15~V2.18）
- 标记本轮新发现的待跟进缺口：见文末「待跟进事项」

**更早轮次优化摘要（2026-07-01，存档）**：

- 修正 [implementation_plan_v0_1_zh.md](04_实施计划/implementation_plan_v0_1_zh.md)、[task_v0_1_zh.md](04_实施计划/task_v0_1_zh.md) 与 [GOS_ARCH_v2.md](02_基本设计/GOS_ARCH_v2.md) 之间关于 Phase A/B 完成状态的口径矛盾
- [GITHUB_DESCRIPTION_020.md](00_项目管理/GITHUB_DESCRIPTION_020.md) 由英文全文改写为中文，并修正 `gos-loader` 定位与 crate 数量（25 → 39）
- [GRAPH_CLI_COMMANDS_zh.md](03_详细设计/GRAPH_CLI_COMMANDS_zh.md) 对照 `k-shell` 源码补齐 V2.8~V2.14 新增的 `nodes`/`edges`/`graph diff`/`journal`/`metrics export`/`boot verify`/`proc` 命令族
- 散落在 `doc/` 根目录的 `HARDENING_LOG_2026-07-01_V2.14.md`（英文）已中文化并归位至 [06_运维维护/hardening/](06_运维维护/hardening/)，原文件保留跳转说明，不做破坏性删除
- 修复 [GOS测试报告书.xlsx](05_测试验证/GOS测试报告书.xlsx) 中 `Boot序列详单!B15` 因文本以 `=` 起始被误判为公式导致的错误单元格
- 标记两处待跟进的文档缺口：`GOS_ARCH_v2.md` 未逐一说明的 14 个 V2 新增 crate；ADR-004 引用但未归档的 ADR-002/ADR-003

---

## 00 · 项目管理

| 文件 | 格式 | 内容 |
|------|------|------|
| [GOS_GOVERNANCE_v0_2.md](00_项目管理/GOS_GOVERNANCE_v0_2.md) | MD | 治理规则 v0.2 — 分支策略 / 代码口径 / 提交规范 |
| [RULE_GRAPH_PRIME.md](00_项目管理/RULE_GRAPH_PRIME.md) | MD | Graph Prime 规则 — 仓库不可谈判约束，所有变更的最终裁判 |
| [GITHUB_DESCRIPTION_020.md](00_项目管理/GITHUB_DESCRIPTION_020.md) | MD | GitHub 项目主页对外说明（v0.2 Vector Mesh 架构） |
| [GOS项目管理台账.xlsx](00_项目管理/GOS项目管理台账.xlsx) | **XLSX** | 里程碑状态表 / Prime 规则台账 / 治理规则摘要 |

---

## 01 · 需求定义

| 文件 | 格式 | 内容 |
|------|------|------|
| [GOS_RUNTIME_v0.1_SPEC.md](01_需求定义/GOS_RUNTIME_v0.1_SPEC.md) | MD | Graph Runtime v0.1 完整规范 — boot / 调度 / 存储 / 隔离 / 可观测性 |
| [GOS系统需求规格书.xlsx](01_需求定义/GOS系统需求规格书.xlsx) | **XLSX** | 功能需求 20 条（FR / NFR）× 优先级 / 阶段 / 状态 + 关键设计决策清单 |

---

## 02 · 基本设计

| 文件 | 格式 | 内容 |
|------|------|------|
| [GOS_ARCH_v2.md](02_基本设计/GOS_ARCH_v2.md) | MD | **主线架构文档** — 系统全景 + 路线图（已完成 vs 未完成明确区分） |
| [design_v0_1_master_zh.md](02_基本设计/design_v0_1_master_zh.md) | MD | 工作区设计总览 — 模块划分 / 边代数 / 向量寻址空间 |
| [GRAPH_NATIVE_SCHEDULING_zh.md](02_基本设计/GRAPH_NATIVE_SCHEDULING_zh.md) | MD | 图原生执行模型 — CPU 上下文切换映射为纯 graph mutation |
| [GOS基本设计书.xlsx](02_基本设计/GOS基本设计书.xlsx) | **XLSX** | 模块职责表 / 启动序列 7 步骤 / 边语义表（10 条边 × primitive 分解） |

---

## 03 · 详细设计

| 文件 | 格式 | 内容 |
|------|------|------|
| [ADR-001-edge-algebra-constitution.md](03_详细设计/ADR-001-edge-algebra-constitution.md) | MD | 边代数宪法 — 4 primitive + 4 属性，宪法级，批准后不可向后兼容修改 |
| [ADR-004-mutation-visibility.md](03_详细设计/ADR-004-mutation-visibility.md) | MD | Cypher Mutation 可见性语义 — Epoch-Published 模型（已批准） |
| [PHASE_B4_DOMAIN_ISOLATION.md](03_详细设计/PHASE_B4_DOMAIN_ISOLATION.md) | MD | Phase B.4 地址空间隔离设计 — CR3 / 模块镜像隔离（B.4.1~B.4.6） |
| [GRAPH_CLI_COMMANDS_zh.md](03_详细设计/GRAPH_CLI_COMMANDS_zh.md) | MD | k-shell CLI 完整指令手册（口径：仅记录已实现命令） |
| [CYPHER_NODE_zh.md](03_详细设计/CYPHER_NODE_zh.md) | MD | K_CYPHER 节点手册 — Cypher v1 子集语法 / 权限模型 |
| [NETWORK_NODE_zh.md](03_详细设计/NETWORK_NODE_zh.md) | MD | K_NET 网络节点说明 — QEMU 虚拟网卡接入与链路状态 API |
| [GOS详细设计书.xlsx](03_详细设计/GOS详细设计书.xlsx) | **XLSX** | ADR 决策记录台账 / CLI 命令规格（37条）/ 节点规格表 / 地址空间隔离设计 |

---

## 04 · 实施计划

| 文件 | 格式 | 内容 |
|------|------|------|
| [implementation_plan_v0_1_zh.md](04_实施计划/implementation_plan_v0_1_zh.md) | MD | 当前实施路线图 — Phase A/B/C 阶段目标 / 退出条件 |
| [task_v0_1_zh.md](04_实施计划/task_v0_1_zh.md) | MD | 执行 Backlog — 已完成基线 + 待办任务清单 |
| [GOS实施计划书.xlsx](04_实施计划/GOS实施计划书.xlsx) | **XLSX** | 阶段路线图（Phase A/B/C）/ 任务 Backlog（23 条任务 × 优先级 / 状态） |

---

## 05 · 测试验证

| 文件 | 格式 | 内容 |
|------|------|------|
| [SYSTEM_TEST_REPORT.md](05_测试验证/SYSTEM_TEST_REPORT.md) | MD | 系统测试报告 2026-04-28 — QEMU 启动全流程验收 / PluginGroup 架构验证 |
| [GOS测试报告书.xlsx](05_测试验证/GOS测试报告书.xlsx) | **XLSX** | 测试结果总览（8项）/ Boot 序列详单（17 milestone）/ 测试环境配置 |

---

## 06 · 运维维护

| 文件 | 格式 | 内容 |
|------|------|------|
| [INSTALL_BARE_METAL_zh.md](06_运维维护/INSTALL_BARE_METAL_zh.md) | MD | 裸机安装指南 — 无开发环境目标机器的完整安装路径 |
| [GOS运维日志汇总.xlsx](06_运维维护/GOS运维日志汇总.xlsx) | **XLSX** | 硬化日志汇总表（V2.0~V2.14，15次）/ 裸机安装操作规程（7步骤） |
| [hardening/HARDENING_LOG_2026-06-30.md](06_运维维护/hardening/HARDENING_LOG_2026-06-30.md) | MD | V2.0 — 消除全工作区 clippy 警告 |
| [hardening/HARDENING_LOG_2026-06-30_V2.1.md](06_运维维护/hardening/HARDENING_LOG_2026-06-30_V2.1.md) | MD | V2.1 — MutationDispatcher 接入真实 runtime |
| [hardening/HARDENING_LOG_2026-06-30_V2.2.md](06_运维维护/hardening/HARDENING_LOG_2026-06-30_V2.2.md) | MD | V2.2 — fault attribution audit 路径 + ADR-004 |
| [hardening/HARDENING_LOG_2026-06-30_V2.3.md](06_运维维护/hardening/HARDENING_LOG_2026-06-30_V2.3.md) | MD | V2.3 — RewriteEngine match→guard→emit 骨架 |
| [hardening/HARDENING_LOG_2026-06-30_V2.4.md](06_运维维护/hardening/HARDENING_LOG_2026-06-30_V2.4.md) | MD | V2.4 — Supervisor 接入 RewriteEngine + Quiescence |
| [hardening/HARDENING_LOG_2026-06-30_V2.5.md](06_运维维护/hardening/HARDENING_LOG_2026-06-30_V2.5.md) | MD | V2.5 — Subscribe 反应式机制 + epoch-diff 空帧跳过 |
| [hardening/HARDENING_LOG_2026-06-30_V2.6.md](06_运维维护/hardening/HARDENING_LOG_2026-06-30_V2.6.md) | MD | V2.6 — metrics 命令 + epoch-diff idle skip |
| [hardening/HARDENING_LOG_2026-06-30_V2.7.md](06_运维维护/hardening/HARDENING_LOG_2026-06-30_V2.7.md) | MD | V2.7 — Boot Manifest 静态图 + 27 条 EdgeAbsent 自愈规则 |
| [hardening/HARDENING_LOG_2026-07-01_V2.8.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.8.md) | MD | V2.8 — nodes / nodes faulted / nodes summary 命令 |
| [hardening/HARDENING_LOG_2026-07-01_V2.9.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.9.md) | MD | V2.9 — boot verify / boot status 命令 |
| [hardening/HARDENING_LOG_2026-07-01_V2.10.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.10.md) | MD | V2.10 — metrics export 命令 + telemetry API harness |
| [hardening/HARDENING_LOG_2026-07-01_V2.11.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.11.md) | MD | V2.11 — journal 命令 + decode_kind 修复 + 14 测试 |
| [hardening/HARDENING_LOG_2026-07-01_V2.12.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.12.md) | MD | V2.12 — edges 命令 + gos-edge-inspect-harness 10 测试 |
| [hardening/HARDENING_LOG_2026-07-01_V2.13.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.13.md) | MD | V2.13 — graph diff 命令 + 结构突变差分环 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-01_V2.14.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.14.md) | MD | V2.14 — proc/ps 命令 + 每节点信号计数器 + gos-proc-harness 10 测试（原英文版已归位并中文化） |
| [hardening/HARDENING_LOG_2026-07-01_V2.15.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.15.md) | MD | V2.15 — stat/node stat 单节点详情命令 + proc_stat_for_vector + gos-stat-harness 10 测试（本轮从 doc 根目录补建中文版并归位） |
| [hardening/HARDENING_LOG_2026-07-01_V2.16.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.16.md) | MD | V2.16 — graph diff \<epoch\> 命令 + parse_epoch_decimal + gos-graph-diff-epoch-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-01_V2.17.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.17.md) | MD | V2.17 — graph topo 命令 + L4-domain 拓扑 API + gos-graph-topo-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-01_V2.18.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.18.md) | MD | V2.18 — graph health 命令 + faulted_node_count + diff_ring_fill + gos-graph-health-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-01_V2.19.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.19.md) | MD | V2.19 — Theme Palette Nodes + Subscribe 自动重绘 + fire_subscribers Signal 投递 + gos-theme-node-harness 10 测试 |
| [hardening/HARDENING_LOG_2026-07-01_V2.20.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.20.md) | MD | V2.20 — plugins/lsmod 命令 + Plugin Inventory API + gos-plugin-list-harness 10 测试 |
| [hardening/HARDENING_LOG_2026-07-01_V2.21.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.21.md) | MD | V2.21 — kill/node-fault 命令 + fault_node API + gos-kill-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-01_V2.22.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.22.md) | MD | V2.22 — resume/node-resume 命令 + resume_node API + gos-resume-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-01_V2.23.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.23.md) | MD | V2.23 — node info/ninfo 命令 + dispatch_node_info + gos-node-info-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-01_V2.24.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.24.md) | MD | V2.24 — node trace/ntrace 命令 + 每节点信号追踪环 + gos-node-trace-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-01_V2.25.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.25.md) | MD | V2.25 — node log/nlog 命令 + NodeLogEntry 生命周期日志环 + gos-node-log-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-01_V2.26.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.26.md) | MD | V2.26 — node log clear/nlog clear 命令 + clear_node_log API + gos-node-log-clear-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-01_V2.27.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.27.md) | MD | V2.27 — node trace clear/ntrace clear 命令 + clear_node_trace API + gos-node-trace-clear-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-01_V2.28.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.28.md) | MD | V2.28 — uname/ver 命令（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-01_V2.29.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.29.md) | MD | V2.29 — node stat clear/nstat clear 命令 + 20 项 harness 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-01_V2.30.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.30.md) | MD | V2.30 — graph watch / watch proc 命令 + 实时 VECTOR DECK proc 面板 + gos-watch-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.31.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.31.md) | MD | V2.31 — graph path \<from\> \<to\> 命令 + BFS 最短路径追踪 + gos-graph-path-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.32.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.32.md) | MD | V2.32 — graph cycles / cycles 命令 + 有向环检测 + gos-graph-cycles-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.33.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.33.md) | MD | V2.33 — graph toposort / toposort 命令 + Kahn BFS 拓扑排序 + gos-graph-toposort-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.34.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.34.md) | MD | V2.34 — graph scc / scc 命令 + Kosaraju 强连通分量分解 + gos-graph-scc-harness 10 测试 |
| [hardening/HARDENING_LOG_2026-07-02_V2.35.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.35.md) | MD | V2.35 — graph condensation / condense 命令 + 缩点 DAG + gos-graph-condensation-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.36.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.36.md) | MD | V2.36 — graph reachable / reachable 命令 + 传递可达性 DFS + gos-graph-reachable-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.37.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.37.md) | MD | V2.37 — graph bipartite / bipartite 命令 + BFS 二染色检测 + gos-graph-bipartite-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.38.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.38.md) | MD | V2.38 — graph degree / degree 命令 + 入/出度普查 + hub 识别 + gos-graph-degree-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.39.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.39.md) | MD | V2.39 — graph centrality / centrality 命令 + Brandes 介数中心性 + gos-graph-centrality-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.40.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.40.md) | MD | V2.40 — graph closeness / closeness 命令 + 出向紧密中心性 + gos-graph-closeness-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.41.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.41.md) | MD | V2.41 — graph eccentricity / radius / diameter 命令 + gos-graph-eccentricity-harness 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.42.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.42.md) | MD | V2.42 — graph katz 命令 + 入向 Katz 中心性 + 图论算法套件（V2.32~V2.42）收官 |
| [hardening/HARDENING_LOG_2026-07-02_V2.43.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.43.md) | MD | V2.43 — graph pagerank 命令 + 经典 PageRank 随机游走中心性 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.44.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.44.md) | MD | V2.44 — graph hits 命令 + Kleinberg HITS hub/authority 分解 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.45.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.45.md) | MD | V2.45 — graph community 命令 + 标签传播社区发现 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.46.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.46.md) | MD | V2.46 — graph spanning 命令 + BFS 生成森林 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.47.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.47.md) | MD | V2.47 — graph color 命令 + Welsh-Powell 贪心图着色 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.48.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.48.md) | MD | V2.48 — graph mst 命令 + Prim 最小生成森林 + edge_weight 快照基础设施 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.49.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.49.md) | MD | V2.49 — graph shortest 命令 + Dijkstra 单源最短路径 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.50.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.50.md) | MD | V2.50 — graph flow 命令 + Edmonds-Karp 最大流 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-02_V2.51.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.51.md) | MD | V2.51 — node checkpoint 命令 + 节点状态快照写入 diff ring + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-02_V2.52.md](06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.52.md) | MD | V2.52 — graph sim 命令 + xorshift32 随机游走信号流量模拟 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-03_V2.53.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.53.md) | MD | V2.53 — graph between 命令 + Brandes+Dijkstra 加权介数中心性 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-03_V2.54.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.54.md) | MD | V2.54 — graph attractor 命令 + Kosaraju 缩点吸引子集合分类 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-03_V2.55.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.55.md) | MD | V2.55 — node attr set/get 命令 + 每节点 u32 属性存储（PAL_U32 图原生化第1步）+ 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.56.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.56.md) | MD | V2.56 — 引导时为 theme.wabi/theme.shoji 节点写入调色板 u32 属性（第2步）+ 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.57.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.57.md) | MD | V2.57 — Desktop 渲染路径改由 node_attr_get 读取调色板（第3步）+ 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.58.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.58.md) | MD | V2.58 — node attr list 命令 + u32 属性表诊断枚举 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.59.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.59.md) | MD | V2.59 — graph density 命令 + E/(N·(N-1)) 图密度指标 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.60.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.60.md) | MD | V2.60 — node attr list u8 命令 + u8 属性表枚举（与 u32 表对称）+ 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.61.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.61.md) | MD | V2.61 — graph clustering 命令 + Watts-Strogatz 全局聚类系数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.62.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.62.md) | MD | V2.62 — palette.cyan/palette.gold 图节点补全调色板图原生化（第4步）+ 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.63.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.63.md) | MD | V2.63 — graph transitivity 命令 + 原始三角形/三元组计数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.64.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.64.md) | MD | V2.64 — graph kcore 命令 + Batagelj-Zaversnik k-核分解/退化度 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.65.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.65.md) | MD | V2.65 — graph assortativity 命令 + Newman(2002) 度同配系数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.66.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.66.md) | MD | V2.66 — graph reciprocity 命令 + 有向图互惠性度量 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.67.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.67.md) | MD | V2.67 — graph modularity 命令 + Newman-Girvan 模块度 Q（LPA 分区）+ 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.68.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.68.md) | MD | V2.68 — graph rich-club 系数命令 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.69.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.69.md) | MD | V2.69 — graph girth 命令 + 有向图最短环长度 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.70.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.70.md) | MD | V2.70 — graph Wiener 指数命令 + 节点对 BFS 距离和 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.71.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.71.md) | MD | V2.71 — graph harmonic 命令 + 调和中心性（倒数 BFS 距离和）+ 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.72.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.72.md) | MD | V2.72 — graph peripheral 命令 + 离心率=直径的外围节点集 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.73.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.73.md) | MD | V2.73 — graph center 命令 + 离心率=半径的中心节点集 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.74.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.74.md) | MD | V2.74 — graph global efficiency 命令 + 全局效率 E(G) + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.75.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.75.md) | MD | V2.75 — graph avg clustering 命令 + 平均聚类系数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.76.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.76.md) | MD | V2.76 — graph local efficiency 命令 + 局部效率 E_loc + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.77.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.77.md) | MD | V2.77 — graph small-world 命令 + 小世界系数 σ + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.78.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.78.md) | MD | V2.78 — graph scale-free 命令 + 度异质性指数 κ=⟨k²⟩/⟨k⟩ + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.79.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.79.md) | MD | V2.79 — graph summary 命令 + 拓扑一站式报告（密度+CC+效率+σ+κ）+ 10 测试 |
| [hardening/HARDENING_LOG_2026-07-03_V2.80.md](06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.80.md) | MD | V2.80 — power-law exponent MLE 命令 + 幂律指数极大似然估计 γ̂ + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-04_V2.81.md](06_运维维护/hardening/HARDENING_LOG_2026-07-04_V2.81.md) | MD | V2.81 — γ̂ 集成进 graph summary 面板 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-04_V2.82.md](06_运维维护/hardening/HARDENING_LOG_2026-07-04_V2.82.md) | MD | V2.82 — graph diameter 组合视图（中心+外围合并面板）+ 10 测试 |
| [hardening/HARDENING_LOG_2026-07-04_V2.83.md](06_运维维护/hardening/HARDENING_LOG_2026-07-04_V2.83.md) | MD | V2.83 — graph metric snapshot 保存与比较命令 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-04_V2.84.md](06_运维维护/hardening/HARDENING_LOG_2026-07-04_V2.84.md) | MD | V2.84 — graph link prediction 命令 + CN/Jaccard/Adamic-Adar/RA 节点对链路预测 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-04_V2.85.md](06_运维维护/hardening/HARDENING_LOG_2026-07-04_V2.85.md) | MD | V2.85 — graph articulation 命令 + Tarjan 割点检测 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-04_V2.86.md](06_运维维护/hardening/HARDENING_LOG_2026-07-04_V2.86.md) | MD | V2.86 — graph bridges 命令 + Tarjan low-link 割边检测 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-04_V2.87.md](06_运维维护/hardening/HARDENING_LOG_2026-07-04_V2.87.md) | MD | V2.87 — Eulerian path/circuit 检测命令 + O(V+E) 度数+BFS 判定 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-04_V2.88.md](06_运维维护/hardening/HARDENING_LOG_2026-07-04_V2.88.md) | MD | V2.88 — DAG 最长路径（关键路径）命令 + Kahn BFS+DP + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-04_V2.89.md](06_运维维护/hardening/HARDENING_LOG_2026-07-04_V2.89.md) | MD | V2.89 — DAG 拓扑分层命令 + Kahn BFS+DP 并行执行层级 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-05_V2.90.md](06_运维维护/hardening/HARDENING_LOG_2026-07-05_V2.90.md) | MD | V2.90 — graph dominator tree 命令 + Cooper et al. 2001 迭代 RPO 支配树 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-05_V2.91.md](06_运维维护/hardening/HARDENING_LOG_2026-07-05_V2.91.md) | MD | V2.91 — feedback arc set 命令 + 迭代 DFS 三染色 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-05_V2.92.md](06_运维维护/hardening/HARDENING_LOG_2026-07-05_V2.92.md) | MD | V2.92 — maximum bipartite matching 命令 + Kuhn 迭代 DFS 匹配 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-05_V2.93.md](06_运维维护/hardening/HARDENING_LOG_2026-07-05_V2.93.md) | MD | V2.93 — 2-edge-connected components 命令 + Tarjan 桥检测 + BFS + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-05_V2.94.md](06_运维维护/hardening/HARDENING_LOG_2026-07-05_V2.94.md) | MD | V2.94 — k-truss 分解命令 + 边剥离算法（k-core 的边级精化）+ 10 测试 |
| [hardening/HARDENING_LOG_2026-07-05_V2.95.md](06_运维维护/hardening/HARDENING_LOG_2026-07-05_V2.95.md) | MD | V2.95 — maximum clique 命令 + Bron-Kerbosch + Tomita 主元优化 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-05_V2.96.md](06_运维维护/hardening/HARDENING_LOG_2026-07-05_V2.96.md) | MD | V2.96 — maximum independent set 命令 + 补图上的 Bron-Kerbosch + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-05_V2.97.md](06_运维维护/hardening/HARDENING_LOG_2026-07-05_V2.97.md) | MD | V2.97 — minimum vertex cover 命令 + König 精确解（二部图）+ 2-近似（一般图）+ 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V2.98.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V2.98.md) | MD | V2.98 — minimum dominating set 命令 + 贪心 ln(Δ)+1 近似 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V2.99.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V2.99.md) | MD | V2.99 — minimum path cover 命令 + König/Dilworth DAG 算法 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V3.00.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.00.md) | MD | V3.00 — minimum spanning arborescence 命令 + Chu-Liu/Edmonds 1967 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V3.01.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.01.md) | MD | V3.01 — feedback vertex set 命令 + 贪心 Kahn FVS + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V3.02.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.02.md) | MD | V3.02 — global min cut 命令 + Stoer-Wagner 1997 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V3.03.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.03.md) | MD | V3.03 — Hamiltonian path/circuit 命令 + 迭代回溯 DFS + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V3.04.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.04.md) | MD | V3.04 — chordal graph recognition 命令 + LexBFS PEO 验证 + 10 测试（本轮从 doc 根目录中文化归位） |
| [hardening/HARDENING_LOG_2026-07-06_V3.05.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.05.md) | MD | V3.05 — biconnected components 命令 + Tarjan 边栈 BCC + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V3.06.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.06.md) | MD | V3.06 — edge betweenness centrality 命令 + Brandes 边介数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V3.07.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.07.md) | MD | V3.07 — vertex connectivity 命令 + Even 1975 节点分裂最大流 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V3.08.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.08.md) | MD | V3.08 — edge coloring 命令 + 贪心 Vizing χ'(G) + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V3.09.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.09.md) | MD | V3.09 — graph spectral analysis 命令 + 谱半径 ρ(A) + 代数连通度 λ₂(L) + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V3.10.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.10.md) | MD | V3.10 — graph entropy 命令 + 度数分布香农熵 H(G) + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-06_V3.11.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.11.md) | MD | V3.11 — graph zagreb 命令 + Zagreb M1/M2 + Randić R + Albertson I + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-06_V3.12.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.12.md) | MD | V3.12 — graph topo 命令 + SC/GA/AZI 拓扑指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V3.13.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.13.md) | MD | V3.13 — graph topo2 命令 + H/ABC/F 拓扑指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V3.14.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.14.md) | MD | V3.14 — graph topo3 命令 + SDD/ISI/Nirmala 拓扑指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V3.15.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.15.md) | MD | V3.15 — graph topo4 命令 + Sombor/RM₂/Sigma 拓扑指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-06_V3.16.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.16.md) | MD | V3.16 — graph topo5 命令 + HM₁/HM₂/AG 拓扑指数 + 10 测试（本轮从 doc 根目录中文化归位） |
| [hardening/HARDENING_LOG_2026-07-06_V3.17.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.17.md) | MD | V3.17 — graph topo6 命令 + EM₁/ABS/RRR 拓扑指数 + 10 测试（本轮从 doc 根目录中文化归位） |
| [hardening/HARDENING_LOG_2026-07-06_V3.18.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.18.md) | MD | V3.18 — graph topo7 命令 + Wiener/Harary/超-Wiener 基于距离拓扑指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-07_V3.19.md](06_运维维护/hardening/HARDENING_LOG_2026-07-07_V3.19.md) | MD | V3.19 — graph topo8 命令 + ECI/直径/半径/平均离心率拓扑指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-07_V3.20.md](06_运维维护/hardening/HARDENING_LOG_2026-07-07_V3.20.md) | MD | V3.20 — graph topo9 命令 + Schultz MTI/Gutman/连通离心指数 度数-距离混合指数 + 10 测试（本轮从 doc 根目录中文化归位） |
| [hardening/HARDENING_LOG_2026-07-07_V3.21.md](06_运维维护/hardening/HARDENING_LOG_2026-07-07_V3.21.md) | MD | V3.21 — graph topo10 命令 + Szeged/修订 Szeged/Mostar 边划分距离指数 + 10 测试（本轮从 doc 根目录中文化归位） |
| [hardening/HARDENING_LOG_2026-07-07_V3.22.md](06_运维维护/hardening/HARDENING_LOG_2026-07-07_V3.22.md) | MD | V3.22 — graph topo11 命令 + Balaban J/传输不规则度/顶点 PI 传输类指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-07_V3.23.md](06_运维维护/hardening/HARDENING_LOG_2026-07-07_V3.23.md) | MD | V3.23 — graph topo12 命令 + Zagreb 离心率 M1\*/M2\*/M3\* + 10 测试（本轮从 doc 根目录中文化归位） |
| [hardening/HARDENING_LOG_2026-07-07_V3.24.md](06_运维维护/hardening/HARDENING_LOG_2026-07-07_V3.24.md) | MD | V3.24 — graph topo13 命令 + 传输 Zagreb TM1/TM2/GA_t 指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-08_V3.25.md](06_运维维护/hardening/HARDENING_LOG_2026-07-08_V3.25.md) | MD | V3.25 — graph topo14 命令 + 总离心率/离心距离和/几何-算术离心率 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-08_V3.26.md](06_运维维护/hardening/HARDENING_LOG_2026-07-08_V3.26.md) | MD | V3.26 — graph topo15 命令 + 跳跃 Zagreb LM1/LM2/LM3（二距离度数）指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-08_V3.27.md](06_运维维护/hardening/HARDENING_LOG_2026-07-08_V3.27.md) | MD | V3.27 — graph topo16 命令 + 乘积连通性 R_{1/2}/倒数 Randić R_{-1}/兰州指数 Lz + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-08_V3.28.md](06_运维维护/hardening/HARDENING_LOG_2026-07-08_V3.28.md) | MD | V3.28 — graph topo17 命令 + Zagreb 补图指数 M̄₁/M̄₂/F̄ + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-08_V3.29.md](06_运维维护/hardening/HARDENING_LOG_2026-07-08_V3.29.md) | MD | V3.29 — graph topo18 命令 + 邻域 Zagreb NM₁/NM₂/GA₂ 指数 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-08_V3.30.md](06_运维维护/hardening/HARDENING_LOG_2026-07-08_V3.30.md) | MD | V3.30 — graph topo19 命令 + 反向 Wiener Λ/RCW/终端 Wiener TW 指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-15_V3.31.md](06_运维维护/hardening/HARDENING_LOG_2026-07-15_V3.31.md) | MD | V3.31 — graph topo20 命令 + SO\*/RSO/rSO Sombor 族变体指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-15_V3.32.md](06_运维维护/hardening/HARDENING_LOG_2026-07-15_V3.32.md) | MD | V3.32 — graph topo21 命令 + ABC₄/NH/NSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-15_V3.33.md](06_运维维护/hardening/HARDENING_LOG_2026-07-15_V3.33.md) | MD | V3.33 — graph topo22 命令 + NR/NF/NSC 邻域指数（S-variant 族首组）+ 10 测试 |
| [hardening/HARDENING_LOG_2026-07-15_V3.34.md](06_运维维护/hardening/HARDENING_LOG_2026-07-15_V3.34.md) | MD | V3.34 — graph topo23 命令 + NHM1/NSDD/NM3 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-15_V3.35.md](06_运维维护/hardening/HARDENING_LOG_2026-07-15_V3.35.md) | MD | V3.35 — graph topo24 命令 + NISI/NAZI/NEM1 邻域指数 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-15_V3.36.md](06_运维维护/hardening/HARDENING_LOG_2026-07-15_V3.36.md) | MD | V3.36 — graph topo25 命令 + NHM2/NAG/NABS 邻域指数 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-16_V3.37.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.37.md) | MD | V3.37 — graph topo26 命令 + NPC/NRM₂/NRSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-16_V3.38.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.38.md) | MD | V3.38 — graph topo27 命令 + NRR/NSO\*/NrSO 邻域指数 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-16_V3.39.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.39.md) | MD | V3.39 — graph topo28 命令 + NNI/NNMI/NSM1 邻域 Nirmala 指数 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-16_V3.40.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.40.md) | MD | V3.40 — graph topo29 命令 + NZ₀/NEM₂/NSe 邻域指数 + 修复 topo28 k-shell 缺口 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-16_V3.41.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.41.md) | MD | V3.41 — graph topo30 命令 + NVQ/NRGS/NHCS 邻域高阶幂次指数 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-16_V3.42.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.42.md) | MD | V3.42 — graph topo31 命令 + NSig/NHQS/NPS 邻域指数 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-16_V3.43.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.43.md) | MD | V3.43 — graph topo32 命令 + NSH/NHPS/NWSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-16_V3.44.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.44.md) | MD | V3.44 — graph topo33 命令 + NSHP/NHSE/NCSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-16_V3.45.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.45.md) | MD | V3.45 — graph topo34 命令 + NOC/NHHS/NFSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-16_V3.46.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.46.md) | MD | V3.46 — graph topo35 命令 + NNC/NHOC/NHSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-16_V3.47.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.47.md) | MD | V3.47 — graph topo36 命令 + NDC/NHNC/NOSO 邻域指数（S¹⁰系）+ 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-16_V3.48.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.48.md) | MD | V3.48 — graph topo37 命令 + NUC/NHDC/NTSO 邻域指数（S¹¹系）+ 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-16_V3.49.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.49.md) | MD | V3.49 — graph topo38 命令 + NDoC/NHUC/NDSO 邻域指数（S¹²系）+ 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-16_V3.50.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.50.md) | MD | V3.50 — graph topo39 命令 + NTC/NHDOC/NESO 邻域指数（S¹³系）+ 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-16_V3.51.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.51.md) | MD | V3.51 — graph topo40 命令 + NQTC/NHTC/NGSO 邻域指数（S¹⁴系）+ 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-16_V3.52.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.52.md) | MD | V3.52 — graph topo41 命令 + NPTC/NHQTC/NIOSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-16_V3.53.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.53.md) | MD | V3.53 — graph topo42 命令 + NSTC/NHPTC/NJSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-16_V3.54.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.54.md) | MD | V3.54 — graph topo43 命令 + NHEPTC/NHSTC/NKSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-16_V3.55.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.55.md) | MD | V3.55 — graph topo44 命令 + NOCTC/NHOCTC/NLSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-16_V3.56.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.56.md) | MD | V3.56 — graph topo45 命令 + NNONTC/NHNONTC/NMSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-16_V3.57.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.57.md) | MD | V3.57 — graph topo46 命令 + NEICTC/NHEICTC/NNSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-16_V3.58.md](06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.58.md) | MD | V3.58 — graph topo47 命令 + NHENTC/NHHENTC/NPSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-17_V3.59.md](06_运维维护/hardening/HARDENING_LOG_2026-07-17_V3.59.md) | MD | V3.59 — graph topo48 命令 + NDOCTC/NHDOCTC/NQSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-17_V3.60.md](06_运维维护/hardening/HARDENING_LOG_2026-07-17_V3.60.md) | MD | V3.60 — graph topo49 命令 + NTRICTC/NHTRICTC/NRSO 邻域指数 + 10 测试（本轮从 doc 根目录归位） |
| [hardening/HARDENING_LOG_2026-07-17_V3.61.md](06_运维维护/hardening/HARDENING_LOG_2026-07-17_V3.61.md) | MD | V3.61 — graph topo50 命令 + NTETRTC/NHTETRTC/NSSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-17_V3.62.md](06_运维维护/hardening/HARDENING_LOG_2026-07-17_V3.62.md) | MD | V3.62 — graph topo51 命令 + NPENTTC/NHPENTTC/NUSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-17_V3.63.md](06_运维维护/hardening/HARDENING_LOG_2026-07-17_V3.63.md) | MD | V3.63 — graph topo52 命令 + NHEXATC/NHHEXATC/NVSO 邻域指数 + 10 测试（本轮从 doc 根目录归位） |
| [hardening/HARDENING_LOG_2026-07-17_V3.64.md](06_运维维护/hardening/HARDENING_LOG_2026-07-17_V3.64.md) | MD | V3.64 — graph topo53 命令 + NHEPTATC/NHHEPTATC/NXSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-17_V3.65.md](06_运维维护/hardening/HARDENING_LOG_2026-07-17_V3.65.md) | MD | V3.65 — graph topo54 命令 + NOCTATC/NHOCTATC/NYSO 邻域指数 + 10 测试（累计 1623 host tests） |
| *（V3.66 缺失）* | — | topo55 — NNONATC/NHNONATC/NZSO（α=46）邻域指数，仅见于 V3.67 等后续引用，文件未在仓库中找到 |
| [hardening/HARDENING_LOG_2026-07-19_V3.67.md](06_运维维护/hardening/HARDENING_LOG_2026-07-19_V3.67.md) | MD | V3.67 — graph topo56 命令 + NTRIACTC/NHTRIACTC/NASO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-19_V3.68.md](06_运维维护/hardening/HARDENING_LOG_2026-07-19_V3.68.md) | MD | V3.68 — graph topo57 命令 + NHENTRIACTC/NHHENTRIACTC/NBSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-19_V3.69.md](06_运维维护/hardening/HARDENING_LOG_2026-07-19_V3.69.md) | MD | V3.69 — graph topo58 命令 + NDOTRIACTC/NHDOTRIACTC/NAASO（双字母序列起点）邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-19_V3.70.md](06_运维维护/hardening/HARDENING_LOG_2026-07-19_V3.70.md) | MD | V3.70 — graph topo59 命令 + NTRITRIACTC/NHTRITRIACTC/NABSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-19_V3.71.md](06_运维维护/hardening/HARDENING_LOG_2026-07-19_V3.71.md) | MD | V3.71 — graph topo60 命令 + NTETRTRIACTC/NHTETRTRIACTC/NACSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-19_V3.72.md](06_运维维护/hardening/HARDENING_LOG_2026-07-19_V3.72.md) | MD | V3.72 — graph topo61 命令 + NPENTTRIACTC/NHPENTTRIACTC/NADSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-20_V3.73.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.73.md) | MD | V3.73 — graph topo62 命令 + NHEXATRIACTC/NHHEXATRIACTC/NAESO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-20_V3.74.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.74.md) | MD | V3.74 — graph topo63 命令 + NHEPTATRIACTC/NHHEPTATRIACTC/NAFSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-20_V3.75.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.75.md) | MD | V3.75 — graph topo64 命令 + NOCTATRIACTC/NHOCTATRIACTC/NAGSO 邻域指数 + 10 测试（累计 1723 host tests） |
| [hardening/HARDENING_LOG_2026-07-20_V3.76.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.76.md) | MD | V3.76 — graph topo65 命令 + NNONATRIACTC/NHNONATRIACTC/NAHSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-20_V3.77.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.77.md) | MD | V3.77 — graph topo66 命令 + NTETRAACTC/NHTETRAACTC/NAISO 邻域指数 + 10 测试（累计 1743 host tests，本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-20_V3.78.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.78.md) | MD | V3.78 — graph topo67 命令 + NHENTETRAACTC/NHHENTETRAACTC/NAJSO 邻域指数 + 10 测试（本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-20_V3.79.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.79.md) | MD | V3.79 — graph topo68 命令 + NDOTETRAACTC/NHDOTETRAACTC/NAKSO 邻域指数（补齐 topo66/67 k-shell 派发函数）+ 10 测试 |
| [hardening/HARDENING_LOG_2026-07-20_V3.80.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.80.md) | MD | V3.80 — graph topo69 命令 + NTRITETRAACTC/NHTRITETRAACTC/NALSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-20_V3.81.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.81.md) | MD | V3.81 — graph topo70 命令 + NTETRATETRAACTC/NHTETRATETRAACTC/NAMSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-20_V3.82.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.82.md) | MD | V3.82 — graph topo71 命令 + NPENTETRAACTC/NHPENTETRAACTC/NANSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-20_V3.83.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.83.md) | MD | V3.83 — graph topo72 命令 + NHEXTETRAACTC/NHHEXTETRAACTC/NAOSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-20_V3.84.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.84.md) | MD | V3.84 — graph topo73 命令 + NHEPTETRAACTC/NHHEPTETRAACTC/NAPSO 邻域指数 + 10 测试（累计 1813 host tests，本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-20_V3.85.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.85.md) | MD | V3.85 — graph topo74 命令 + NOCTOTETRAACTC/NHOCTOTETRAACTC/NAQSO 邻域指数 + 10 测试（累计 1823 host tests，本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-20_V3.86.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.86.md) | MD | V3.86 — graph topo75 命令 + NNONATETRAACTC/NHNONATETRAACTC/NARSO 邻域指数 + 10 测试（累计 1833 host tests，本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-20_V3.87.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.87.md) | MD | V3.87 — graph topo76 命令 + NPENTAACTC/NHPENTAACTC/NASSO 邻域指数（pentacontic 系列首个）+ 10 测试（累计 1843 host tests，本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-20_V3.88.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.88.md) | MD | V3.88 — graph topo77 命令 + NHENPENTAACTC/NHHENPENTAACTC/NATSO 邻域指数 + 10 测试（累计 1853 host tests，本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-20_V3.89.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.89.md) | MD | V3.89 — graph topo78 命令 + NDOPENTAACTC/NHDOPENTAACTC/NAUSO 邻域指数 + 10 测试（累计 1863 host tests，本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-20_V3.90.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.90.md) | MD | V3.90 — graph topo79 命令 + NTRIPENTAACTC/NHTRIPENTAACTC/NAVSO 邻域指数 + 10 测试（累计 1873 host tests，本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-20_V3.91.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.91.md) | MD | V3.91 — graph topo80 命令 + NTETRAPENTAACTC/NHTETRAPENTAACTC/NAWSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-20_V3.92.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.92.md) | MD | V3.92 — graph topo81 命令 + NPENTAPENTAACTC/NHPENTAPENTAACTC/NAXSO 邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-20_V3.93.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.93.md) | MD | V3.93 — graph topo82 命令 + NHEXPENTAACTC/NHHEXPENTAACTC/NAYSO（Centyl Sombor）邻域指数 + 10 测试（累计 1903 host tests，本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-20_V3.94.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.94.md) | MD | V3.94 — graph topo83 命令 + NHEPTPENTAACTC/NHHEPTPENTAACTC/NAZSO（第3轮双字母收官）邻域指数 + 10 测试（累计 1913 host tests，本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-20_V3.95.md](06_运维维护/hardening/HARDENING_LOG_2026-07-20_V3.95.md) | MD | V3.95 — graph topo84 命令 + NOCTOPENTAACTC/NHOCTOPENTAACTC/NBASO（第4轮双字母起点）邻域指数 + 10 测试（累计 1923 host tests，本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-21_V3.96.md](06_运维维护/hardening/HARDENING_LOG_2026-07-21_V3.96.md) | MD | V3.96 — graph topo85 命令 + NNONAPENTAACTC/NHNONAPENTAACTC/NBBSO（pentacontic 系列收官）邻域指数 + 10 测试 |
| [hardening/HARDENING_LOG_2026-07-21_V3.97.md](06_运维维护/hardening/HARDENING_LOG_2026-07-21_V3.97.md) | MD | V3.97 — graph topo86 命令 + NHEXAACTC/NHHEXAACTC/NBCSO（hexacontic 系列首个）邻域指数 + 10 测试（累计 1943 host tests，本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-21_V3.98.md](06_运维维护/hardening/HARDENING_LOG_2026-07-21_V3.98.md) | MD | V3.98 — graph topo87 命令 + NHEXAENACTC/NHHEXAENACTC/NBDSO 邻域指数 + 10 测试（累计 1953 host tests，本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-21_V3.99.md](06_运维维护/hardening/HARDENING_LOG_2026-07-21_V3.99.md) | MD | V3.99 — graph topo88 命令 + NHEXADYACTC/NHHEXADYACTC/NBESO 邻域指数 + 10 测试（累计 1963 host tests，本轮中文化） |
| [hardening/HARDENING_LOG_2026-07-21_V3.100.md](06_运维维护/hardening/HARDENING_LOG_2026-07-21_V3.100.md) | MD | V3.100 — graph topo89 命令 + NHEXATRIACTC/NHHEXATRIACTC/NBFSO 邻域指数（首个三位数版本号里程碑）+ 10 测试（本轮中文化；自述累计 host tests 与 V3.99 存在 10 项口径矛盾，已如实标注） |
| [hardening/HARDENING_LOG_2026-07-21_V3.101.md](06_运维维护/hardening/HARDENING_LOG_2026-07-21_V3.101.md) | MD | V3.101 — graph topo90 命令 + NHEXATETRAACTC/NHHEXATETRAACTC/NBGSO 邻域指数 + 10 测试 |
| *（V3.102 缺失）* | — | topo91 — NHEXAPENTAACTC/NHHEXAPENTAACTC/NBHSO（α=118）邻域指数，仅见于 V3.101/V3.103/V3.104 交叉引用，文件未在仓库中找到 |
| [hardening/HARDENING_LOG_2026-07-21_V3.103.md](06_运维维护/hardening/HARDENING_LOG_2026-07-21_V3.103.md) | MD | V3.103 — graph topo92 命令 + NHEXAHEXAACTC/NHHEXAHEXAACTC/NBISOS 邻域指数 + k-rope 应变限制双向扫描修复 + rope 物理测试套件首次入库（gos-rope-harness 12 测试 + gos-rope-material-harness 6 测试）+ 10 测试（累计约 2011 host tests） |
| [hardening/HARDENING_LOG_2026-07-21_V3.104.md](06_运维维护/hardening/HARDENING_LOG_2026-07-21_V3.104.md) | MD | V3.104 — graph topo93 命令 + NHEXAHEPTACTC/NHHEXAHEPTACTC/NBJSO 邻域指数 + 10 测试（累计约 2021 host tests） |
| [hardening/HARDENING_LOG_2026-07-21_V3.105.md](06_运维维护/hardening/HARDENING_LOG_2026-07-21_V3.105.md) | MD | V3.105 — graph topo94 命令 + NHEXAOCTACTC/NHHEXAOCTACTC/NBKSO（NB第11个，α=124）邻域指数 + 10 测试（累计约 2031 host tests） |
| [hardening/HARDENING_LOG_2026-07-21_V3.106.md](06_运维维护/hardening/HARDENING_LOG_2026-07-21_V3.106.md) | MD | V3.106 — graph topo95 命令 + NHEXAENNACTC/NHHEXAENNACTC/NBLSO（NB第12个，α=126，hexacontic 系列收官）邻域指数 + 10 测试（累计约 2041 host tests） |

---

*最终更新：2026-07-21 · GOS V3.106 · 累计约 2041 个 host tests（详见各硬化日志逐篇累计计数；V3.99→V3.100 存在 10 项口径矛盾，尚未核实，见「待跟进事项」）· 图论分析命令族自 V3.29 起以「邻域度和 S(v) 替代顶点度数 d(v)」的 Neighborhood S-variant 模式持续扩展，V3.31~V3.104 新增 graph topo20~topo93 共 74 组、222 个邻域 S-变体拓扑指数，加上 V3.12~V3.30 的 graph topo1~topo19（19 组、57 个指数），拓扑指数命令族累计 93 组、约 279 个分子图拓扑描述符，全部纳入中文文档索引 · V3.103 起新增 k-rope 绳索物理子系统（XPBD 应变限制 + 材质属性测试，18 项测试），为拓扑指数系列之外的新分支，尚未纳入 GRAPH_CLI_COMMANDS_zh.md（该模块目前无 CLI 命令，属内部物理引擎，暂不需要）*

## 待跟进事项

- **[已处理]** 06_运维维护/hardening 目录内 16 篇纯英文/双语硬化日志（V3.77/78/84/85/86/87/88/89/90/93/94/95/97/98/99/100）——本轮已就地中文化，不改动版本号/文件路径/函数名/测试名/测试计数。
- **[已处理，优先级高]** [GRAPH_CLI_COMMANDS_zh.md](03_详细设计/GRAPH_CLI_COMMANDS_zh.md)（v1.7→v1.8）与 [GOS_ARCH_v2.md](02_基本设计/GOS_ARCH_v2.md)（v2.7→v2.8）此前未收录 V3.66~V3.104 新增的 Neighborhood S-variant 拓扑指数命令族 `graph topo55`~`graph topo93`（39 组、117 个指数）——本轮已在 GRAPH_CLI_COMMANDS_zh.md 新增 §十五完整汇总，GOS_ARCH_v2.md §5.1 补充概述并指向该章节为权威口径。
- **[新发现，待核实]** V3.66（topo55）、V3.102（topo91）两篇硬化日志文件在 `06_运维维护/hardening/` 与 `doc/` 根目录均未找到实体文件，仅能从相邻版本的交叉引用中还原内容摘要；本轮已在 README 索引表、GRAPH_CLI_COMMANDS_zh.md §十五中如实标注为缺失条目，未编造内容。建议下一轮核查源代码仓库的提交历史（如 `git log --all --diff-filter=A -- 'doc/**/*V3.66*' 'doc/**/*V3.102*'` 或等效方式），确认是否为归档遗漏、误删除，或版本号本身从未生成对应日志（例如被后续 commit 直接覆盖）。
- **[新发现，待核实]** V3.100（topo89）硬化日志自述"宿主测试总数 1963（此前 1953）"，与 V3.99（topo88）自述的累计总数 1963 相矛盾（两者不能同时成立，差 10 项）；本轮未擅自修正数值，仅在 V3.100 中文译文与 GRAPH_CLI_COMMANDS_zh.md §十五中加注说明。建议下一轮通过实际运行 `cargo test --workspace` 统计口径，核实 V3.100 前后的真实累计测试数，并视情况订正本文档引用的约 2021 总数。
- **[已核实，解除阻塞]** 此前多轮标记为"待处理"的 [GOS测试报告书.xlsx](05_测试验证/GOS测试报告书.xlsx)、[GOS运维日志汇总.xlsx](06_运维维护/GOS运维日志汇总.xlsx) 及其余 5 份 xlsx 文档管理台账的 `.~lock` 锁定文件，本轮核查其内部时间戳均为 **2026-06-30 21:18**（来自会话 `trusting-pensive-meitner`），距今已逾三周，可判定为陈旧残留锁（编辑会话早已结束但未正常释放），**并非当前活跃编辑冲突**。本轮出于时间与篇幅限制仍未展开 xlsx 内容更新，但该锁定顾虑已解除，建议下一轮直接更新汇总表（测试总数、硬化日志汇总行）而无需再以"锁定中"为由推迟，或联系用户确认可否清除陈旧锁文件。
- **[持续遗留，已跨多轮]** `implementation_plan_v0_1_zh.md`（v2.2）与 `task_v0_1_zh.md`（v1.5）均仍停留在 2026-07-06（V3.06 基线，1033 host tests），此后 V3.07~V3.104（98 次硬化迭代、累计约 2021 host tests，含 §十三/§十四/§十五全部拓扑指数命令族与 §5.1 网络科学/结构分解算法套件）均未登记为 Backlog 已完成任务。该缺口已连续多轮标记但一直未处理，规模已显著扩大，建议作为独立任务安排专门轮次处理，而非依赖后续 doc 优化轮次顺带补齐。
- 5 篇滞留于 `doc/` 根目录的英文硬化日志（V3.16/V3.17/V3.20/V3.21/V3.23）历史遗留——沿用既往轮次的处理原则（详见 06_运维维护/hardening 内对应文件）。
- 本轮 GRAPH_CLI_COMMANDS_zh.md §十五沿用 §十四的汇总索引形式（版本/指数/硬化日志三列），未逐条列出每个指数的完整公式与别名（该细节以对应硬化日志为权威口径，避免双写口径漂移）。
- V3.103 起新增的 k-rope 绳索物理子系统（XPBD 应变限制双向扫描、材质属性测试，共 18 项测试）为拓扑指数系列之外的新分支，目前无对应 CLI 命令，暂未纳入 GRAPH_CLI_COMMANDS_zh.md；若后续该模块暴露 shell 命令，需在设计文档中补充章节。
- 本轮中文化的 16 篇硬化日志均为逐句人工核对翻译并交叉核对源码引用的函数名/公式；受限于单轮篇幅，未对 V3.66~V3.104 中已判定为中文的 21 篇逐篇重新核对格式模板一致性，仅确认其 CJK 密度达标。
