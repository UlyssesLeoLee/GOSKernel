# GOS 当前实施路线图（v0.2+）

| 项目 | 内容 |
|---|---|
| 文档编号 | GOS-DOC-04-01 |
| 所属阶段 | 04・实施计划 |
| 版本 / 状态 | v2.2 / 现行 |
| 作成 / 审核 / 批准 | GOS 核心团队 |
| 基线日期 | 2026-06-30 |
| 最终更新 | 2026-07-06 |

**变更履历**

| 版本 | 日期 | 变更内容 | 作成者 |
|---|---|---|---|
| v1.0 | 2026-06-30 | 纳入日系工程阶段目录（04_实施计划） | GOS 核心团队 |
| v2.0 | 2026-07-01 | **重大口径修正**：本文档此前把 Phase A/B 描述为"待完成"，但 [GOS_ARCH_v2.md §八](../02_基本设计/GOS_ARCH_v2.md) 已记录 Phase A 于 2026-04-25、Phase B（含 B.5）于 2026-04-26 完成，且 V2.0~V2.14 的一系列硬化任务已在 Phase B 骨架之上持续交付可观测性能力。本次按 GOS_ARCH_v2.md 权威口径同步更新 Phase A/B 状态，并将当前实施重心前移至 Phase B 残留缺口与 Phase C 准备 | GOS 核心团队 |
| v2.1 | 2026-07-03 | **口径修正**：§一「当前仍未完成的核心问题」此前称"Phase C 的图原生控制面收口工作尚未开始"，与 [task_v0_1_zh.md](./task_v0_1_zh.md) 记载的 V2.8~V2.65（共 65 项硬化日志）已交付的进程管理 / 图论分析 / 图健康度 / 节点属性存储命令族事实不符；已更正为"观测性命令族已大量交付，GUI 可视化与开发者调试工作流文档尚未开始" | GOS 核心团队 |
| v2.2 | 2026-07-06 | 补充 V2.66~V3.06（共 41 项硬化日志）已交付的网络科学扩展指标、图分析工具化、第三代结构分解与经典图论算法套件三条命令族，累计 host tests 由 623 更新为 1033；同步归档 V2.94~V3.06 共 13 份硬化日志至 `06_运维维护/hardening/`；识别到新增文档债项：该批次内多篇硬化日志（V2.95~V3.06）尚未中文化 | GOS 核心团队 |

> **口径说明**：文件路径沿用历史名称，但本文档已经不是早期 bare-metal 初始计划，而是当前系统的实施路线图。凡与 [GOS_ARCH_v2.md](../02_基本设计/GOS_ARCH_v2.md) 冲突之处，以架构文档为准；本文档负责把架构状态转译为可执行的实施顺序与退出条件。

---

## 一、当前基线

当前仓库已经完成的基线包括：

- `hypervisor` 采用最小引导模型
- 启动主链使用 `boot_builtin_graph + gos_supervisor::service_system_cycle`
- builtin graph 已经承载 shell、cypher、AI、network、theme.current、clipboard.mount 等用户可见节点
- `gos-supervisor` 已具备 module descriptor、domain、instance lane、claim、heap grant 等控制面骨架
- Phase A（legacy island 清零）与 Phase B（原子化底座主干：fault attribution、instance 调度、heap quota、domain 隔离 B.4.1~B.4.5、ELF loader 首切片 B.4.6、degraded mode B.5）均已完成（详见 §二、§三）
- V2.0~V2.14 硬化系列在 Phase B 骨架之上持续交付：`nodes`/`edges`/`graph diff`/`journal`/`metrics export`/`boot verify`/`proc` 等可观测性命令族（详见 [GRAPH_CLI_COMMANDS_zh.md §七](../03_详细设计/GRAPH_CLI_COMMANDS_zh.md)）
- V2.16~V2.65 硬化系列持续扩展：进程生命周期管理、第一代/第二代图论分析（含 PageRank/HITS/community/MST/最短路径/最大流/介数中心性/吸引子分类）、图健康度指标（density/clustering/transitivity/kcore/assortativity）、节点属性存储（PAL_U32 图原生化重构）等命令族，累计 623 个 host tests（详见 [GRAPH_CLI_COMMANDS_zh.md §八~§十](../03_详细设计/GRAPH_CLI_COMMANDS_zh.md)）
- V2.66~V3.06 硬化系列（共 41 项硬化日志）进一步扩展：网络科学扩展指标（reciprocity/modularity/rich-club/girth/wiener/harmonic/peripheral/center/efficiency/small-world/scale-free/power-law 等）、图分析工具化（指标快照对比、链路预测）、第三代结构分解与经典图论算法套件（割点/割边、欧拉路径、DAG 关键路径与分层、支配树、反馈弧集/点集、二分匹配、2ECC/BCC、k-truss、最大团/独立集、最小顶点覆盖/支配集/路径覆盖/生成树形图/全局最小割、Hamiltonian 检测、弦图识别、边介数中心性等）等命令族，累计 **1033** 个 host tests（详见 [GRAPH_CLI_COMMANDS_zh.md §十一~§十二](../03_详细设计/GRAPH_CLI_COMMANDS_zh.md)）

当前仍未完成的核心问题：

- ELF relocation 之外的完整外部模块装载路径（B.4.6.2~B.4.6.4：符号解析、`.gosmod` 装载、manifest 完整性校验）尚未落地
- 真实的跨模块越权访问端到端验证（构造越界 plugin 触发 #PF → DEGRADED）仍待补充
- Phase C 的图原生控制面**部分**推进：观测性 / 图论分析 / 图健康度 / 网络科学 / 结构分解算法命令族已大量交付（V2.8~V3.06，累计 106 项硬化日志），但 shell/cypher/AI 转为纯图客户端、GUI 可视化界面、面向外部开发者的调试工作流文档三项仍未开始
- 文档债：V2.95~V3.06 期间新增的多篇硬化日志仍为纯英文撰写，此前已收口一次的"硬化日志中文化"缺口重新出现（详见 [task_v0_1_zh.md](./task_v0_1_zh.md)）

## 二、Phase A：清零剩余 legacy island — ✅ 已完成（2026-04-25）

### 目标

- 把当前 allowlist 中的 legacy crate 全部迁出主架构叙事
- 让 native-first 插件模型成为唯一推荐模型

### 交付结果

- 所有曾列入 allowlist 的 crate（`k-pit`、`k-ps2`、`k-idt`、`k-pmm`、`k-vmm`、`k-heap`）已迁移至 `PluginManifest + NodeSpec + NodeExecutorVTable`
- `builtin_bundle::BuiltinModule` 枚举现仅剩 `Native` 变体；`LegacyModule`、`LegacyNodeTemplate`、`legacy_node()`、`synchronize_legacy_graph()` 等 legacy 基础设施已从主线代码库完全移除
- allowlist 收敛为零
- 架构文档（GOS_ARCH_v2.md、GOS_GOVERNANCE_v0_2.md、RULE_GRAPH_PRIME.md）已不再把 legacy 描述为推荐路径

### 退出条件核对

| 退出条件 | 状态 |
|---|---|
| allowlist 收敛到零 | ✅ 已达成 |
| 主链不再依赖 legacy 叙事 | ✅ 已达成 |
| 架构文档不提 legacy 也能完整描述系统 | ✅ 已达成 |

> 注：[task_v0_1_zh.md](./task_v0_1_zh.md) 此前仍保留两条 Phase A 未勾选项（"指定迁移 owner/顺序"、"同步收紧 allowlist"），已随本次同步更新为已完成勾选，避免与本文档口径矛盾。

## 三、Phase B：补齐真正的原子化底座 — ✅ 主干已完成（2026-04-26），子分片持续收口

### 目标

在不插入新高层 feature phase 的前提下，完成原子化解耦内核底座。

### 已落地能力

1. Domain PML4 构造与 domain-aware page mapping（B.4.1、B.4.2 ✅）
2. IST stacks + CPU-fault dispatch hook（B.4.3 ✅）
3. CR3 trampoline + native dispatch RAII 包装（B.4.4 ✅）
4. 跨 domain capability invocation 结构（B.4.5 ✅，端到端验证待续）
5. ELF loader 最小切片：解析 + `R_X86_64_RELATIVE` 重定位（B.4.6 首切片 ✅）
6. `NodeTemplate -> NodeInstance` 绑定、supervisor lane-class 调度（B.2 ✅）
7. per-instance `HeapQuota` + `k-heap` backend + 不变式扫描（B.3 ✅）
8. fault attribution bridge：runtime `ExecStatus::Fault` → supervisor `fault_module`（B.1 ✅）
9. Degraded mode + restart cap + fault telemetry envelope（B.5 ✅）

详细设计与实施分片见 [PHASE_B4_DOMAIN_ISOLATION.md](../03_详细设计/PHASE_B4_DOMAIN_ISOLATION.md)。

### 公开术语

该阶段所有设计与文档统一使用：

- `ModuleDescriptor`
- `ModuleDomain`
- `NodeTemplateId`
- `NodeInstanceId`
- `ExecutionLaneClass`
- `ResourceId`
- `ClaimId`
- `LeaseEpoch`
- `HeapQuota`

### 退出条件核对

| 退出条件 | 状态 |
|---|---|
| 至少一个真实模块能在独立 domain 中执行 | ✅ 已达成（builtin 模块经 domain PML4 + trampoline 路径） |
| supervisor 能管理实例、资源 claim、heap grant、fault restart 的完整闭环 | ✅ 已达成 |
| 底座能力不再依赖 legacy 兼容路径解释 | ✅ 已达成 |

### 残留缺口（未阻塞 Phase B 视为主干完成，但需在 B.4.6.x 独立收口）

- B.4.6.2 — dynamic symbol table 解析，定位 `module_init`/`module_event`/`module_stop` 入口偏移
- B.4.6.3 — 外部 `.gosmod` 文件从 FAT32（Phase F）装载并启动
- B.4.6.4 — manifest 嵌入 `.gos.manifest` section + 完整性校验
- 真实跨 domain 越权访问的端到端验证（需至少两个外部 ELF 模块，见 [PHASE_B4_DOMAIN_ISOLATION.md §八](../03_详细设计/PHASE_B4_DOMAIN_ISOLATION.md)）

## 四、Phase C：底座完成后的图原生控制面 — 下一阶段重心

### 目标

把 shell、cypher、AI、CUDA bridge、开发者体验全部建立在稳定底座之上，而不是让这些层继续承担底层兼容逻辑。

### 范围

- shell / cypher / AI 彻底转为图客户端与控制面
- host-backed CUDA / AI orchestration 继续保留，但绑定到稳定的 resource / instance 模型
- 观测、可视化、开发工作流、查询体验作为第三优先级推进

### 不在本阶段之前提前做的事

- 不为了 UI/AI/GPU 体验绕开底座
- 不为了短期功能继续扩大 legacy island
- 不再新增 procedure-first 启动或旁路执行链

## 五、统一验证要求

每个 phase 都必须至少满足：

```powershell
pwsh -File .\tools\verify-graph-architecture.ps1
cargo check -p gos-kernel
```

并且：

- 文档与当前实现一致
- 新增的 node / edge / capability / vector 关系可被 shell 或 cypher 浏览
- 术语在 README、架构文档、治理文档、操作文档中保持统一
