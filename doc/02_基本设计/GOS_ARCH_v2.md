# GOS 当前架构与后续路线图

| 项目 | 内容 |
|---|---|
| 文档编号 | GOS-DOC-02-01 |
| 所属阶段 | 02・基本设计（主线架构文档） |
| 版本 / 状态 | v2.7 / 现行 |
| 作成 / 审核 / 批准 | GOS 核心团队 |
| 基线日期 | 2026-06-30 |
| 最终更新 | 2026-07-20 |

**变更履历**

| 版本 | 日期 | 变更内容 | 作成者 |
|---|---|---|---|
| v2.0 | 2026-06-30 | 纳入日系工程阶段目录（02_基本设计），确立为主线架构权威文档 | GOS 核心团队 |
| v2.1 | 2026-07-01 | 补充文档管理信息；与 [implementation_plan_v0_1_zh.md](../04_实施计划/implementation_plan_v0_1_zh.md)、[task_v0_1_zh.md](../04_实施计划/task_v0_1_zh.md) 的 Phase A/B 完成状态交叉核对一致 | GOS 核心团队 |
| v2.2 | 2026-07-02 | §5.1「终端与图导航」此前只列出核心图导航命令，未反映 V2.8~V2.42 新增的进程管理 / 拓扑巡检 / 图论分析命令族（约 34 项硬化日志），补充概述并指向 [GRAPH_CLI_COMMANDS_zh.md](../03_详细设计/GRAPH_CLI_COMMANDS_zh.md) 作为权威命令清单，避免双写口径漂移 | GOS 核心团队 |
| v2.3 | 2026-07-03 | §5.1 补充 V2.43~V2.65 新增的第二代图论分析（PageRank/HITS/community/spanning/color/mst/shortest/flow/between/attractor/sim）、图健康度指标（density/clustering/transitivity/kcore/assortativity）、节点属性存储（node attr，PAL_U32 图原生化重构）三条命令族概述，累计 623 host tests | GOS 核心团队 |
| v2.4 | 2026-07-06 | §5.1 补充 V2.66~V3.06（41 项硬化日志）新增的网络科学扩展指标、图分析工具化能力、第三代结构分解与经典图论算法套件三条命令族概述；累计 host tests 由 623 更新为 1033；同步归档 06_运维维护/hardening 下 V2.94~V3.06 共 13 份硬化日志 | GOS 核心团队 |
| v2.5 | 2026-07-15 | §5.1 补充 V3.07~V3.30（24 项硬化日志）新增的连通性/边染色/谱分析/信息熵/经典 Zagreb 指数补完，以及 `graph topo`~`graph topo19` 拓扑指数命令族（19 组、57 个分子图拓扑描述符）概述；累计 host tests 由 1033 更新为约 1273；此前 v2.4 概述截至 V3.06，V3.07 之后长期未同步至本文档 | GOS 核心团队 |
| v2.6 | 2026-07-19 | §5.1 补充 V3.31~V3.65（35 项硬化日志）新增的 Neighborhood S-variant 拓扑指数命令族 `graph topo20`~`graph topo54`（105 个指数）概述，指向 [GRAPH_CLI_COMMANDS_zh.md §十四](../03_详细设计/GRAPH_CLI_COMMANDS_zh.md) 作为权威口径；累计 host tests 由约 1273 更新为 1623（截至 V3.65）；同步归档 `doc/` 根目录孤儿硬化日志 V3.60、V3.63 至 06_运维维护/hardening，并将其中 12 篇纯英文硬化日志（V3.35/36/38/39/40/41/42/47/48/49/50/51）就地中文化 | GOS 核心团队 |
| v2.7 | 2026-07-20 | 文档体系维护：doc/ 根目录下 10 份孤儿硬化日志（V3.68~V3.81，含自动强化流水线并发生成的 V3.82）归位至 [06_运维维护/hardening](../06_运维维护/hardening/)，其中 3 份纯英文日志（V3.76/V3.79/V3.82）就地中文化，其余混合中英标题统一为「GOSKernel 强化日志 — Vx.xx」格式；doc/ 根目录下 17 份与 00~06 编号子目录内容重复、且缺少文档管理头/变更履历的过期设计文档（含本文件旧版）改写为指向权威版本的归位存根，消除单一信息源（SSOT）漂移风险；同步修正 AGENTS.md 中指向旧根路径的 3 处链接 | GOS 核心团队 |

---

> 口径说明：本文档描述的是 **当前仓库已经采用的系统主线**，并在文末单列后续路线图。未完成能力不会伪装成已完成能力。

## 一、系统身份

GOS 当前的推荐架构已经不是“loader 先行、运行时补图”的原型模型，而是：

- `hypervisor` 只负责最小引导
- builtin graph 是启动时注册的第一份系统图
- `gos-runtime` 负责图登记、激活、路由、能力解析和图摘要
- `gos-supervisor` 负责模块描述符、模块域、能力发布、实例与资源控制，以及 steady-state system cycle
- node / edge / vector / capability / `mount` / `use` 是公开的一等执行结构

现有 legacy island 仍存在，但它只代表迁移中的技术债，不代表系统长期形态。

## 二、启动主链

当前启动链固定如下：

1. `hypervisor::kernel_main`
   - 开启 CPU 必要特性
   - 初始化 `gos-hal::vaddr` 与 `gos-hal::meta`
2. `gos_supervisor::bootstrap(...)`
   - 建立 supervisor 控制面
   - 安装 builtin module descriptors
3. `builtin_bundle::boot_builtin_graph(...)`
   - 发现 builtin plugins
   - 注册 manifest、node、edge、capability import/export
   - 启动 builtin graph 中需要引导的节点
4. `gos_supervisor::realize_boot_modules()`
   - 为模块准备 domain / capability / control-plane 视角
5. `gos_supervisor::service_system_cycle()`
   - 成为 steady-state 的统一服务入口

当前治理规则已经明确禁止：

- `kernel_main` 再走 `gos_loader::load_bundle`
- `kernel_main` 直接 `gos_runtime::pump`
- `kernel_main` 直接 `plugin_main(...)`
- `kernel_main` 手工 `post_signal(...)` 做业务启动

## 三、核心对象模型

### 3.1 身份与位置

| 概念 | 作用 |
|---|---|
| `PluginId` | 插件或模块的稳定归属身份 |
| `NodeId` | 由 `plugin_id + local_node_key` 派生出的逻辑身份 |
| `VectorAddress` | 运行时位置与图访问地址，不等于逻辑身份 |
| `EdgeId` | 由 `from_node + to_node + edge_key` 派生出的稳定边身份 |
| `EdgeVector` | 边在图控制台中的可读寻址形式 |

当前统一术语是：

- `NodeId` 是逻辑身份
- `VectorAddress` 是运行位置
- 任何热切换、迁移、重启方案都必须优先保持 `NodeId` 稳定

### 3.2 边的公开语义

当前 runtime 对以下语义边有一等支持：

- `Depend`
- `Call`
- `Spawn`
- `Signal`
- `Return`
- `Mount`
- `Sync`
- `Stream`
- `Use`

其中两个关键公开模型已经进入用户可见层：

- `theme.current -[use]-> theme.wabi|theme.shoji`
- `node -[mount]-> clipboard.mount`

它们不是 UI 特判，而是图中的真实节点关系。

### 3.3 capability 与挂载关系

跨插件协作遵循：

1. provider 通过 `exports` 暴露 capability
2. consumer 通过 `imports` 声明依赖
3. builtin graph 或 manifest 同步生成 `Mount` edges
4. runtime 通过 capability 解析与信号路由完成协作

因此，shell 访问网络、cypher、AI、clipboard、console，都是图结构上的依赖与挂载，而不是硬编码函数调用链。

## 四、当前工作区职责图

### 4.1 核心 crate

| crate | 当前职责 |
|---|---|
| `hypervisor` | 最小引导、builtin graph bootstrap、steady-state handoff |
| `gos-protocol` | 公共 ABI、graph 类型、module / instance / resource / heap 协议 |
| `gos-runtime` | 图登记、节点激活、边路由、capability 解析、图摘要 |
| `gos-supervisor` | module/domain 控制面、instance lanes、claims、heap grants、system cycle |
| `gos-hal` | 向量地址、元数据、低层兼容桥 |
| `gos-loader` | 仍在 workspace 中，但已不在 `kernel_main` 主启动路径 |

### 4.2 原生图节点 crate

| crate | 当前定位 |
|---|---|
| `k-shell` | 图控制终端、graph CLI、theme.current、clipboard.mount |
| `k-cypher` | 受控 Cypher v1 子集 |
| `k-ai` | AI supervisor client / control-plane consumer |
| `k-cuda-host` | host-backed CUDA bridge |
| `k-net` | 原生 uplink driver node |
| `k-ime` | 输入法控制 node |
| `k-mouse` | 指针与显示输入 node |
| `k-vga` | 显示与调色板输出 node |

### 4.3 legacy island 状态

**Phase A 已完成（2026-04-25）**。所有 crate 已完成迁出：

- `k-pit` — 最后一个 NodeCell/PluginEntry 使用者，已迁移至 `NodeExecutorVTable`
- `k-ps2`, `k-idt`, `k-pmm`, `k-vmm`, `k-heap` — 此前已完成迁出

`builtin_bundle::BuiltinModule` 枚举现在仅包含 `Native` 变体。`LegacyModule`、`LegacyNodeTemplate`、`legacy_node()`、`synchronize_legacy_graph()` 等 legacy 基础设施已完全移除。allowlist 为零。

## 五、当前用户可见图控制面

### 5.1 终端与图导航

`k-shell` 当前已经支持：

- `show`
- `back`
- `node <vector>`
- `edge <vector>`
- `where`
- `select clear`
- `activate`
- `spawn`
- `PgUp` / `PgDn`
- `Up` / `Down` 历史输入回放

自 V2.8 起，`k-shell` 在此基础上持续扩展了三类命令族（合计约 34 项硬化日志、约 340 项 host 测试覆盖）：

- **进程 / 拓扑巡检**：`nodes` `edges` `proc/ps` `stat` `graph diff` `graph topo` `graph health` `metrics export` `journal` `boot verify`（V2.8~V2.20）
- **节点生命周期管理**：`plugins/lsmod` `kill` `resume` `node info` `node trace` `node log` `uname` `graph watch`（V2.21~V2.30）
- **图论分析命令族**：`graph path` `cycles` `toposort` `scc` `condensation` `reachable` `bipartite` `degree` `centrality` `closeness` `eccentricity` `katz`（V2.31~V2.42，第一代图论算法套件已收官）

V2.43 起，图论分析与图健康度诊断进一步扩展为第二代命令族（V2.43~V2.65，共 23 项硬化日志、约 190 项 host 测试覆盖）：

- **排名与结构分析**：`graph pagerank` `graph hits` `graph community` `graph spanning` `graph color` `graph mst` `graph shortest` `graph flow` `graph between` `graph attractor` `graph sim`（V2.43~V2.54）
- **节点属性存储（PAL_U32 图原生化重构）**：`node attr set/get/list` `node attr list u8`（V2.55~V2.62，将硬编码调色板常量迁移为图节点属性）
- **图健康度指标**：`graph density` `graph clustering` `graph transitivity` `graph kcore` `graph assortativity`（V2.59~V2.65）

V2.66 起，命令族建设进入第三阶段，先后覆盖网络科学扩展指标、分析工具化、以及以经典图论 NP-hard 问题近似/精确算法为主的结构分解套件（V2.66~V3.06，共 41 项硬化日志、约 410 项 host 测试覆盖）：

- **网络科学与拓扑健康度扩展**：`graph reciprocity` `graph modularity` `graph rich-club` `graph girth` `graph wiener` `graph harmonic` `graph peripheral` `graph center` `graph efficiency` `graph avg clustering` `graph local efficiency` `graph small-world` `graph scale-free` `graph summary` `graph powerlaw` `graph diameter/gdiameter`（V2.66~V2.82，17 项）
- **图分析工具化**：`graph snapshot save/compare`（指标快照留存与差分对比）、`graph predict`（CN / Jaccard / Adamic-Adar / Resource Allocation 四种链路预测算法）（V2.83~V2.84，2 项）
- **第三代结构分解与经典图论算法套件**：`graph articulation`（割点，Tarjan）`graph bridges`（割边，Tarjan）`graph eulerian`（欧拉路径/回路）`graph dag longest`（DAG 最长路/关键路径）`graph dag layers`（拓扑层级/并行执行层）`graph domtree`（支配树，Cooper et al. 2001）`graph feedback arc`（反馈弧集，DFS 三染色）`graph bipartite match`（最大二分匹配，Kuhn 算法）`graph 2ecc`（2 边连通分量）`graph truss`（k-truss 分解）`graph clique`（最大团，迭代 Bron-Kerbosch + Tomita pivot）`graph indep`（最大独立集，BK 补图法）`graph vertex cover`（最小顶点覆盖，König + 2-近似）`graph dominating set`（最小支配集，贪心 ln(Δ)+1 近似）`graph min path cover`（DAG 最小路径覆盖，König/Dilworth）`graph arborescence`（最小生成树形图，Chu-Liu/Edmonds 1967）`graph fvs`（最小反馈点集，贪心 Kahn 法）`graph min cut`（全局最小割，Stoer-Wagner 1997）`graph hamiltonian`（Hamiltonian 路径/回路检测，迭代回溯 DFS）`graph chordal`（弦图识别，LexBFS + PEO 验证）`graph bcc`（双连通分量，Tarjan 迭代边栈法）`graph ebc`（边介数中心性，Brandes 2001）（V2.85~V3.06，22 项）

至此宿主测试总数累计 **1033 个**（106 次硬化迭代，截至 V3.06）。完整命令语法、别名与逐版本对应关系以 [GRAPH_CLI_COMMANDS_zh.md](../03_详细设计/GRAPH_CLI_COMMANDS_zh.md) 为唯一权威口径，本文档不重复维护逐条命令细节，避免多处口径漂移。

V3.07 起，命令族建设进入谱分析与化学图论「拓扑指数」阶段（V3.07~V3.30，共 24 项硬化日志、约 240 项 host 测试覆盖）：

- **连通性 / 染色 / 谱 / 信息熵 / 经典指数补完**：`graph vconn`（点连通度，Even 1975）`graph ecolor`（边染色，贪心 Vizing）`graph spectral`（谱半径 ρ(A) + 代数连通度 λ₂(L)）`graph entropy`（度数分布香农熵）`graph zagreb`（Zagreb M1/M2 + Randić R + Albertson I）（V3.07~V3.11，5 项）
- **拓扑指数命令族 `graph topo` ~ `graph topo19`**：自 V3.12 起每组固定新增 3 个分子图拓扑描述符并配套 10 项 harness 测试，累计 19 组、57 个指数，覆盖 Zagreb 系（含超-Zagreb、补图指数、邻域 Zagreb、跳跃 Zagreb）、Randić 系（含乘积连通性、倒数 Randić）、距离系（Wiener、Szeged、Harary、超-Wiener）、离心率系（含 Zagreb 离心率）、传输量系（Balaban J、传输 Zagreb）等（V3.12~V3.30，19 项）

至此宿主测试总数累计约 **1273 个**（130 次硬化迭代，截至 V3.30）。完整命令语法、别名与逐版本对应关系以 [GRAPH_CLI_COMMANDS_zh.md §十三](../03_详细设计/GRAPH_CLI_COMMANDS_zh.md) 为唯一权威口径，本文档不重复维护逐条命令细节。

V3.31 起，拓扑指数命令族的构造方式发生系统性变化：不再直接对顶点度数 `d(v)` 取指数公式，而是先计算**邻域度和** `S(v) = Σ_{w∈N(v)} deg(w)`，再对 `S(v)` 套用已有的拓扑指数公式，得到"Neighborhood S-variant"（邻域 S-变体）版本。该模式自 topo18（V3.29）起步，V3.31~V3.65（35 项硬化日志）持续扩展 `graph topo20`~`graph topo54` 共 35 组、105 个 S-变体指数，覆盖 Sombor 系列变体（修正/约化/简化/α=3~40 广义 Sombor）、顶点幂次序列（S²~S¹⁴）、边幂次序列（(S+S)²~(S+S)¹³）等。至此宿主测试总数累计 **1623 个**（截至 V3.65）。完整命令语法、别名、逐组对应关系与公式推导以 [GRAPH_CLI_COMMANDS_zh.md §十四](../03_详细设计/GRAPH_CLI_COMMANDS_zh.md) 为唯一权威口径，本文档不重复维护逐条命令细节。

### 5.2 主题图

当前主题系统是一个显式的图模型：

| 向量 | 节点 |
|---|---|
| `6.1.1.0` | `theme.wabi` |
| `6.1.2.0` | `theme.shoji` |
| `6.1.3.0` | `theme.current` |

只有 `theme.current` 持有排他的 `Use` edge。  
主题切换的本质是重新指向：

- `theme.current -[use]-> theme.wabi`
- 或 `theme.current -[use]-> theme.shoji`

### 5.3 共享剪贴板图

当前共享剪贴板是独立的挂载节点：

| 向量 | 节点 |
|---|---|
| `6.1.4.0` | `clipboard.mount` |

它的关系是非排他的 `Mount`：

- 任意 node 都可以同时挂载到 `clipboard.mount`
- shell 当前支持 `clipboard mount <vector>` / `clipboard unmount <vector>`
- `Ctrl+C / Ctrl+X / Ctrl+V` 通过该挂载节点复用复制、剪切、粘贴能力

### 5.4 Cypher 与控制查询

`k-cypher` 当前不是通用图数据库解释器，而是受控的 runtime 查询与激活客户端。  
支持的能力集中在：

- 浏览 node
- 浏览 edge
- `CALL activate(n)`
- `CALL spawn(n)`
- `CALL route(e)`

不支持图结构写入、属性写回、事务或任意 mutation。

## 六、当前 supervisor 控制面

`gos-supervisor` 当前已经引入的控制面对象包括：

- `ModuleDescriptor`
- `ModuleDomain`
- `NodeTemplateId`
- `NodeInstanceId`
- `ExecutionLaneClass`
- `ResourceId`
- `ClaimId`
- `LeaseEpoch`
- `HeapQuota`

当前状态可概括为：

- module descriptor install 已存在
- boot module realization 已存在
- instance lane / ready queue / restart queue 已存在
- claim / revoke / heap grant 控制表已存在
- host 测试 harness 已存在

但以下底座仍未完全闭环：

- 真实模块镜像装载与重定位
- 独立 CR3 下的真实模块执行
- 所有 legacy island 的完全迁出
- 私有 heap 的全面替代

## 七、当前验证与治理

当前仓库的权威机械约束来自：

- `tools/verify-graph-architecture.ps1`
- `cargo check -p gos-kernel`
- `cargo check -p k-shell`
- `gos-supervisor` host harness

治理脚本当前强制：

- `kernel_main` 必须经过 `boot_builtin_graph`
- `kernel_main` 必须委托 `gos_supervisor::service_system_cycle`
- 新 crate 不得引入新的 legacy trait 路径
- `NodeSpec` / `EdgeSpec` literal 必须显式声明 `vector_ref`

## 八、后续路线图

### Phase A：清零剩余 legacy island ✅ 已完成（2026-04-25）

- 所有 crate 已迁移至 `NodeExecutorVTable` 原生插件模型
- `NodeCell / PluginEntry / try_mount_cell` 已从主线代码库清除
- `BuiltinModule` 仅剩 `Native` 变体，allowlist 为零
- 文档已更新，legacy 仅作为历史记录存在

### Phase B：补齐原子化底座 ✅ 已完成（2026-04-26）

| 子阶段 | 内容 | 状态 |
|---|---|---|
| B.1 | Fault attribution bridge: runtime ExecStatus::Fault → supervisor fault_module | ✅ |
| B.2 | NodeTemplate → NodeInstance binding；supervisor lane-class 调度 → runtime pump | ✅ |
| B.3 | Per-instance HeapQuota + k-heap backend + 不变式扫描 | ✅ |
| B.4.1 | Domain PML4 构造 + 可观察性 | ✅ |
| B.4.2 | Domain-aware page mapping | ✅ |
| B.4.3 | IST stacks + CPU-fault dispatch hook | ✅ |
| B.4.4 | CR3 trampoline + native dispatch RAII 包装 | ✅ |
| B.4.5 | Cross-domain capability invocation | ✅（结构已具备，端到端验证待 B.4.6.x） |
| B.4.6 | ELF loader 最小切片（解析） | ✅（relocation / symbol / loading 后续独立切片） |
| B.5 | Degraded mode + restart cap + fault telemetry envelope | ✅ |

### Phase B.5：Degraded mode + fault telemetry ✅ 已完成（2026-04-25）

- `ModuleFaultPolicy::FaultKernelDegraded` 走真实路径：撤销能力、清空消息、销毁实例、保持 Faulted 状态
- `Restart / RestartAlways` 在连续重启达到 `MAX_RESTARTS_BEFORE_DEGRADE`（5）后自动降级
- `claim_resource` 与 `charge_heap` 对 Faulted 模块返回 `ModuleRejected`
- `fault_module` 在每次失败时发出 `ControlPlaneMessageKind::Fault` envelope
- shell `where` 视图显示 `DEGRADED` marker、`restarts=N`、`audit: boot-fallback allocs N`

### Phase C：底座完成后的图原生控制面

在 Phase A/B 收口后，再推进：

- shell / cypher / AI 成为纯图客户端与控制面
- host-backed CUDA / AI orchestration 继续保留，但必须建立在稳定的资源与实例模型上
- 开发者体验与更丰富 UI/查询能力作为第三优先级推进

## 九、当前默认结论

- GOS 当前的系统身份是 graph-native + supervisor-native
- legacy island 是待清零技术债，不是架构中心
- 文档与实现都必须围绕 builtin graph boot、runtime graph semantics、supervisor system cycle 这三条主轴展开
