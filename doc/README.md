# GOS ドキュメント索引 / Document Index

> **口径**：本索引按软件工程生命周期分层，反映仓库真实文档状态。  
> 阅读顺序建议：`00_project` → `01_spec` → `02_architecture` → `03_adr` → `04_implementation` → `05_reference` → `06_testing` → `07_operations`

---

## 00 · プロジェクト概要 / 项目元信息

| 文件 | 说明 |
|------|------|
| [RULE_GRAPH_PRIME.md](00_project/RULE_GRAPH_PRIME.md) | **Graph Prime 规则**——仓库不可谈判约束，所有变更的最终裁判 |
| [GOVERNANCE.md](00_project/GOVERNANCE.md) | 治理规则 v0.2——分支策略、代码评审、版本管理 |
| [GITHUB_DESCRIPTION_020.md](00_project/GITHUB_DESCRIPTION_020.md) | GitHub 项目主页描述（v0.2 Vector Mesh 架构对外说明） |

---

## 01 · 仕様 / 规格与需求

| 文件 | 说明 |
|------|------|
| [GOS_RUNTIME_v0.1_SPEC.md](01_spec/GOS_RUNTIME_v0.1_SPEC.md) | Graph Runtime v0.1 完整规范——boot / 调度 / 存储 / 隔离 / 可观测性接口定义 |
| [GRAPH_NATIVE_SCHEDULING_zh.md](01_spec/GRAPH_NATIVE_SCHEDULING_zh.md) | 图原生执行模型——CPU 上下文切换映射为纯 graph mutation 的语义规格 |

---

## 02 · アーキテクチャ / 架构设计

| 文件 | 说明 |
|------|------|
| [GOS_ARCH_v2.md](02_architecture/GOS_ARCH_v2.md) | **主线架构文档**——当前系统全景 + 后续路线图（已完成 vs 未完成明确区分） |
| [design_v0_1_master_zh.md](02_architecture/design_v0_1_master_zh.md) | 工作区设计总览——模块划分、边代数、向量寻址空间设计 |
| [PHASE_B4_DOMAIN_ISOLATION.md](02_architecture/PHASE_B4_DOMAIN_ISOLATION.md) | Phase B.4 地址空间隔离设计——CR3 / 模块镜像隔离（设计阶段） |

---

## 03 · ADR / 架构决策记录

> ADR（Architecture Decision Record）记录**不可逆或高影响**的架构决策，批准后构成系统约束。

| 编号 | 文件 | 状态 | 摘要 |
|------|------|------|------|
| ADR-001 | [ADR-001-edge-algebra-constitution.md](03_adr/ADR-001-edge-algebra-constitution.md) | 提案 | 边代数宪法——定义最小正交 primitive 集，封顶系统可表达性 |
| ADR-004 | [ADR-004-mutation-visibility.md](03_adr/ADR-004-mutation-visibility.md) | 已批准 | Cypher Mutation 可见性语义——Epoch-Published 写后读一致性模型 |

---

## 04 · 実装計画 / 实施计划

| 文件 | 说明 |
|------|------|
| [implementation_plan_v0_1_zh.md](04_implementation/implementation_plan_v0_1_zh.md) | 当前实施路线图（v0.2+）——阶段目标、里程碑、依赖关系 |
| [task_v0_1_zh.md](04_implementation/task_v0_1_zh.md) | 执行 Backlog——已完成基线 + 当前 Sprint 任务清单 |

---

## 05 · リファレンス / 参考手册

| 文件 | 说明 |
|------|------|
| [GRAPH_CLI_COMMANDS_zh.md](05_reference/GRAPH_CLI_COMMANDS_zh.md) | `k-shell` 图控制 CLI 完整指令手册（口径：仅记录已实现命令） |
| [CYPHER_NODE_zh.md](05_reference/CYPHER_NODE_zh.md) | `K_CYPHER` 节点手册——Cypher v1 子集语法、权限模型、查询接口 |
| [NETWORK_NODE_zh.md](05_reference/NETWORK_NODE_zh.md) | `K_NET` 网络节点说明——QEMU 虚拟网卡接入与链路状态 API |
| [INSTALL_BARE_METAL_zh.md](05_reference/INSTALL_BARE_METAL_zh.md) | 裸机安装指南——无开发环境目标机器的完整安装路径 |

---

## 06 · テスト / 测试与验证

| 文件 | 说明 |
|------|------|
| [SYSTEM_TEST_REPORT.md](06_testing/SYSTEM_TEST_REPORT.md) | 系统测试报告 2026-04-28——PluginGroup 重构后 QEMU 启动全流程验收 |

---

## 07 · オペレーション / 运维日志

### Hardening 自动化硬化日志

> 每次 2h 自动硬化周期的交付物记录，按版本号排序。

| 版本 | 日期 | 文件 | 交付摘要 |
|------|------|------|---------|
| V2.0 | 2026-06-30 | [HARDENING_LOG_2026-06-30.md](07_operations/hardening/HARDENING_LOG_2026-06-30.md) | 消除全工作区 clippy 警告 |
| V2.1 | 2026-06-30 | [HARDENING_LOG_2026-06-30_V2.1.md](07_operations/hardening/HARDENING_LOG_2026-06-30_V2.1.md) | MutationDispatcher 接入真实 runtime |
| V2.2 | 2026-06-30 | [HARDENING_LOG_2026-06-30_V2.2.md](07_operations/hardening/HARDENING_LOG_2026-06-30_V2.2.md) | fault attribution audit 路径 + ADR-004 语义 |
| V2.3 | 2026-06-30 | [HARDENING_LOG_2026-06-30_V2.3.md](07_operations/hardening/HARDENING_LOG_2026-06-30_V2.3.md) | RewriteEngine `match→guard→emit` 骨架 |
| V2.4 | 2026-06-30 | [HARDENING_LOG_2026-06-30_V2.4.md](07_operations/hardening/HARDENING_LOG_2026-06-30_V2.4.md) | Supervisor 接入 RewriteEngine + Quiescence |
| V2.5 | 2026-06-30 | [HARDENING_LOG_2026-06-30_V2.5.md](07_operations/hardening/HARDENING_LOG_2026-06-30_V2.5.md) | Subscribe 反应式机制 + epoch-diff 空帧跳过 |
| V2.6 | 2026-06-30 | [HARDENING_LOG_2026-06-30_V2.6.md](07_operations/hardening/HARDENING_LOG_2026-06-30_V2.6.md) | `metrics` 命令 + epoch-diff idle skip |
| V2.7 | 2026-06-30 | [HARDENING_LOG_2026-06-30_V2.7.md](07_operations/hardening/HARDENING_LOG_2026-06-30_V2.7.md) | Boot Manifest 静态图（27 条 EdgeAbsent 自愈规则） |
| V2.8 | 2026-07-01 | [HARDENING_LOG_2026-07-01_V2.8.md](07_operations/hardening/HARDENING_LOG_2026-07-01_V2.8.md) | `nodes` / `nodes faulted` / `nodes summary` 命令 |
| V2.9 | 2026-07-01 | [HARDENING_LOG_2026-07-01_V2.9.md](07_operations/hardening/HARDENING_LOG_2026-07-01_V2.9.md) | `boot verify` / `boot status` 命令 |
| V2.10 | 2026-07-01 | [HARDENING_LOG_2026-07-01_V2.10.md](07_operations/hardening/HARDENING_LOG_2026-07-01_V2.10.md) | `metrics export` 命令 + telemetry API harness |
| V2.11 | 2026-07-01 | [HARDENING_LOG_2026-07-01_V2.11.md](07_operations/hardening/HARDENING_LOG_2026-07-01_V2.11.md) | 文档体系重组（`doc/` 按生命周期分层为 8 类目录）+ 悬空链接修复 |

---

*最終更新 / Last updated：2026-07-01*
