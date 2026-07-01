# GOS 文档索引

> 按日系软件工程生命周期分层：项目管理 → 需求定义 → 基本设计 → 详细设计 → 实施计划 → 测试验证 → 运维维护  
> 每个阶段提供 Markdown 原文 + Excel 结构化版本（中文）

**文档管理规范**：本次优化起，各阶段核心 Markdown 文档统一在标题下方附带文档管理票（文档编号 / 版本 / 状态 / 作成・审核・批准 / 变更履历），Excel 文档统一附带「文档管理」首页 sheet。硬化日志系列（06_运维维护/hardening/）作为历史记录，其内容不回溯改写事实，仅做归位与中文化。

**本轮优化摘要（2026-07-01）**：

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
| [hardening/HARDENING_LOG_2026-07-01_V2.15.md](06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.15.md) | MD | V2.15 — Theme Palette Nodes + Subscribe 自动重绘 + fire_subscribers Signal 投递 + gos-theme-node-harness 10 测试 |

---

*最终更新：2026-07-01 · GOS V2.15 · Demo C（Theme 0 行代码扩散）先决条件全部满足*
