# GOS 当前执行 Backlog

| 项目 | 内容 |
|---|---|
| 文档编号 | GOS-DOC-04-02 |
| 所属阶段 | 04・实施计划 |
| 版本 / 状态 | v1.5 / 现行 |
| 作成 / 审核 / 批准 | GOS 核心团队 |
| 基线日期 | 2026-06-30 |
| 最终更新 | 2026-07-06 |

**变更履历**

| 版本 | 日期 | 变更内容 | 作成者 |
|---|---|---|---|
| v1.0 | 2026-06-30 | 纳入日系工程阶段目录（04_实施计划） | GOS 核心团队 |
| v1.1 | 2026-07-01 | 补充文档管理信息 | GOS 核心团队 |
| v1.2 | 2026-07-01 | **口径修正**：Phase A 两条未勾选项与 [implementation_plan_v0_1_zh.md](./implementation_plan_v0_1_zh.md)、[GOS_ARCH_v2.md](../02_基本设计/GOS_ARCH_v2.md) 记载的"Phase A 已于 2026-04-25 完成、allowlist 为零"相矛盾，予以勾选并补充说明；同步补充 Phase B 已完成项 | GOS 核心团队 |
| v1.3 | 2026-07-02 | **口径修正**：Phase C「开发者工作流、调试、观测、可视化」一项此前长期显示为完全未完成，与 V2.8~V2.42 期间已交付的进程管理 / 图论分析命令族（共 34 项硬化日志、约 340 项 host 测试）事实不符，补充展开为已完成子项清单；父项因可视化 GUI 与开发者工作流仍未开工而保持未勾选 | GOS 核心团队 |
| v1.4 | 2026-07-03 | 补充 V2.43~V2.65 已交付的第二代图论分析、图健康度指标、节点属性存储三条命令族（共 23 项硬化日志、约 190 项 host 测试）为 Phase C 已完成子项；父项仍因 GUI 可视化与开发者调试指南未开工而保持未勾选 | GOS 核心团队 |
| v1.5 | 2026-07-06 | 补充 V2.66~V3.06 已交付的网络科学扩展指标、图分析工具化、第三代结构分解与经典图论算法套件三条命令族（共 41 项硬化日志、累计 host tests 达 1033）为 Phase C 已完成子项；归档 V2.94~V3.06 共 13 份硬化日志至 `06_运维维护/hardening/`；新增识别到的文档债项：V2.95~V3.06 中多篇硬化日志仍为纯英文撰写，中文化缺口尚未清零 | GOS 核心团队 |

> 文件路径沿用历史名称，内容已切换为当前版本的执行清单。

## 已完成基线

- [x] `hypervisor` 切到 minimal bootstrap
- [x] builtin graph 启动链接管主启动路径
- [x] `gos-supervisor::service_system_cycle` 成为 steady-state 服务入口
- [x] `k-shell` 支持 graph CLI、theme.current、clipboard.mount、输入历史
- [x] `k-cypher` 提供受控 Cypher v1 子集
- [x] `k-cuda-host` 提供 host-backed CUDA bridge

## Phase A：清零 legacy island — ✅ 已完成（2026-04-25）

- [x] 为每个 legacy crate 指定迁移 owner 和迁移顺序（迁移已全部完成，历史 owner 分配记录见提交历史）
- [x] `k-panic` 已迁到 manifest-native / executor-driven
- [x] `k-serial` 已迁到 manifest-native / executor-driven
- [x] `k-gdt` 已迁到 manifest-native / executor-driven
- [x] `k-cpuid` 已迁到 manifest-native / executor-driven
- [x] `k-pic` 已迁到 manifest-native / executor-driven
- [x] `k-pit` 迁到 manifest-native / executor-driven
- [x] `k-ps2` 迁到 manifest-native / executor-driven
- [x] `k-idt` 迁到 manifest-native / executor-driven
- [x] `k-pmm` 迁到 manifest-native / executor-driven
- [x] `k-vmm` 迁到 manifest-native / executor-driven
- [x] `k-heap` 迁到 bootstrap/legacy-provider 过渡角色后继续收缩
- [x] 每迁完一个 crate，就同步收紧 verifier allowlist（allowlist 现已收敛为零，`BuiltinModule` 仅剩 `Native` 变体）

## Phase B：补齐原子化底座 — ✅ 主干已完成（2026-04-26），子分片持续收口

- [x] 真实模块镜像格式与装载路径（ELF 最小切片，见 B.4.6）
- [x] 独立 module image map / relocate（`R_X86_64_RELATIVE` 重定位已实现；其余重定位类型待续）
- [x] 独立 domain page table 创建（B.4.1 `create_isolated_address_space`）
- [x] CR3 切换执行与返回 supervisor（B.4.4 trampoline + `DomainGuard` RAII）
- [x] `NodeTemplate -> NodeInstance` 派生与销毁（B.2）
- [x] ready lane / restart lane / revoke lane 完整闭环
- [x] `ResourceId` 注册与 claim/revoke/epoch fencing
- [x] `HeapQuota`、授页、回收、超限拒绝（B.3）
- [x] fault attribution 与 restart / degraded mode（B.1、B.5）
- [x] host harness 与 QEMU smoke 同步覆盖这些底座能力
- [ ] B.4.6.2 — dynamic symbol table 解析（`module_init`/`module_event`/`module_stop` 入口偏移）
- [ ] B.4.6.3 — 外部 `.gosmod` 文件从 FAT32 装载并启动
- [ ] B.4.6.4 — manifest 嵌入 `.gos.manifest` section + 完整性校验
- [ ] 真实跨 domain 越权访问端到端验证（需至少两个外部 ELF 模块）

## Phase C：图原生控制面

- [ ] shell 只保留图客户端与控制职责
- [ ] cypher 文档与行为继续严格限制在受控子集
- [ ] AI 控制面统一走 supervisor / runtime 可审计路径
- [ ] CUDA bridge 绑定稳定的 resource / instance 模型
- [ ] 开发者工作流、调试、观测、可视化在底座稳定后再增强
  - [x] 进程 / 边巡检：`nodes` `edges` `proc/ps` `stat`（V2.8~V2.15，5 项硬化日志）
  - [x] 拓扑变更与健康巡检：`graph diff <epoch>` `graph topo` `graph health`（V2.13、V2.16~V2.18，4 项硬化日志）
  - [x] 遥测 / 日志 / 启动校验：`metrics export` `journal` `boot verify`（V2.9~V2.11，3 项硬化日志）
  - [x] 节点生命周期管理：`plugins/lsmod` `kill` `resume` `node info` `node trace(+clear)` `node log(+clear)` `uname` `node stat clear`（V2.20~V2.29，10 项硬化日志）
  - [x] 实时监视：`graph watch / watch proc`（V2.30）
  - [x] 图论分析命令族（第一代）：`path` `cycles` `toposort` `scc` `condensation` `reachable` `bipartite` `degree` `centrality` `closeness` `eccentricity` `katz`（V2.31~V2.42，12 项硬化日志，第一代图论算法套件收官）
  - [x] 排名与结构分析（第二代）：`pagerank` `hits` `community` `spanning` `color` `mst` `shortest` `flow` `between` `attractor` `sim`（V2.43~V2.54，12 项硬化日志）
  - [x] 节点属性存储（PAL_U32 图原生化重构）：`node attr set/get/list` `node attr list u8`（V2.55~V2.62，8 项硬化日志）
  - [x] 图健康度指标：`density` `clustering` `transitivity` `kcore` `assortativity`（V2.59~V2.65，5 项硬化日志）
  - [x] 网络科学扩展指标：`reciprocity` `modularity` `rich club` `girth` `wiener` `harmonic` `peripheral` `center` `efficiency` `avg clustering` `local efficiency` `small world` `scale free` `power law` `summary` `diameter`（V2.66~V2.82，17 项硬化日志）
  - [x] 图分析工具化：指标快照留存 / 对比（`graph snapshot` `graph compare`）、链路预测（`graph predict`，CN/Jaccard/Adamic-Adar/RA）（V2.83~V2.84，2 项硬化日志）
  - [x] 第三代结构分解与经典图论算法套件：割点/割边、欧拉路径、DAG 关键路径与分层、支配树、反馈弧集、二分匹配、2ECC、k-truss、最大团、最大独立集、最小顶点覆盖、最小支配集、最小路径覆盖、最小生成树形图、反馈点集、全局最小割、Hamiltonian 检测、弦图识别、双连通分量、边介数中心性（V2.85~V3.06，22 项硬化日志）
  - [ ] 图形化 / 可视化界面（当前仅有文本终端 `k-shell` + VECTOR DECK 面板，无独立 GUI 工具）
  - [ ] 面向外部开发者的调试工作流文档（当前 CLI 手册见 [GRAPH_CLI_COMMANDS_zh.md](../03_详细设计/GRAPH_CLI_COMMANDS_zh.md)，但尚无面向新贡献者的调试指南）
  - [ ] 硬化日志中文化：V2.95~V3.06 期间新增的多篇硬化日志（如 V2.95 最大团、V2.98 最小支配集、V3.00 最小生成树形图等）仍为纯英文撰写，与既有"硬化日志系列中文化缺口清零"治理目标（见下节）不一致，需在后续维护窗口补齐中文版
  - 完整命令清单与版本对应关系见 [GRAPH_CLI_COMMANDS_zh.md](../03_详细设计/GRAPH_CLI_COMMANDS_zh.md) §七~§十二

## 文档与治理同步项

- [x] 架构文档只讲当前事实 + 后续路线图（GOS_ARCH_v2.md）
- [x] 治理文档固定为零新增 legacy 口径（GOS_GOVERNANCE_v0_2.md、RULE_GRAPH_PRIME.md）
- [x] README 与 CLI/Cypher/Network 文档持续和实现对齐（2026-07-01 已对照 `k-shell/src/proc.rs` 源码补齐 `nodes`/`edges`/`graph diff`/`journal`/`metrics export`/`boot verify`/`proc` 命令族；2026-07-02 补齐 V2.16~V2.42 图拓扑 / 进程管理 / 图论分析命令族；2026-07-03 补齐 V2.43~V2.65 第二代图论分析 / 图健康度 / 节点属性存储命令族；2026-07-06 补齐 V2.66~V3.06 网络科学扩展指标 / 图分析工具化 / 第三代结构分解算法套件命令族，详见 [GRAPH_CLI_COMMANDS_zh.md](../03_详细设计/GRAPH_CLI_COMMANDS_zh.md)）
- [ ] 硬化日志系列中文化缺口清零（2026-07-02 发现 V2.15~V2.41 共 23 篇硬化日志仍为英文、V2.15 未归位至 `06_运维维护/hardening/`，已全部核对源码后中文化并补齐索引；2026-07-03 补齐 V2.43~V2.49、V2.51~V2.54 共 11 篇纯英文硬化日志的中文化；2026-07-06 归档 V2.94~V3.06 时发现该批次内 V2.95~V3.06 等多篇硬化日志再次以纯英文撰写，缺口重新出现，尚未中文化，详见 [doc/README.md](../README.md)）
- [x] 所有核心文档统一 `NodeId` / `VectorAddress` / `ModuleDescriptor` / `NodeInstance` 术语
- [ ] `GOS_ARCH_v2.md` 补齐 `gos-ai-bridge`/`gos-cluster`/`gos-cypher-mut`/`gos-journal`/`gos-log`/`gos-rewrite`/`gos-sign`/`gos-verify`/`gos-vfs`/`k-core`/`k-chat`/`k-fat32`/`k-nim`/`k-vk-host` 等 V2 阶段新增 crate 的职责说明（2026-07-01 审阅发现的文档缺口，详见 [GITHUB_DESCRIPTION_020.md](../00_项目管理/GITHUB_DESCRIPTION_020.md)）
- [ ] 补齐 ADR-002 / ADR-003 文档或在 ADR-004 中移除失效引用（2026-07-01 审阅发现：ADR-004 引用 ADR-002 但该文档未在 doc/03_详细设计 中检索到）
