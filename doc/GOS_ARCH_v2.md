# GOS 当前架构与后续路线图

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

### 4.3 当前 legacy island

当前仍在治理 allowlist 中的迁移岛为：

- `k-pit`
- `k-ps2`
- `k-idt`
- `k-pmm`
- `k-vmm`
- `k-heap`

这些 crate 仍允许保留 `NodeCell` / `PluginEntry` / `try_mount_cell` 形式，但不再被视为推荐插件模型。

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

### Phase A：清零剩余 legacy island

目标：

- 把剩余 allowlist 中的 legacy crate 全部迁出主架构模型
- native-first 启动链成为唯一推荐与唯一主要实现路线

完成标志：

- `NodeCell / PluginEntry / try_mount_cell` 不再出现在权威架构主线
- allowlist 缩到零或接近零
- 文档中的 legacy 只保留为 debt backlog

### Phase B：补齐原子化底座

重点固定为：

1. 真实 module image / domain page table / CR3 切换执行
2. `NodeTemplate -> NodeInstance` 调度模型
3. `claim / lease / revoke / epoch fencing`
4. 每实例私有 heap 与 supervisor 授页
5. fault attribution / restart / degraded mode

这一阶段完成前，不插入新的高层 feature phase。

### Phase C：底座完成后的图原生控制面

在 Phase A/B 收口后，再推进：

- shell / cypher / AI 成为纯图客户端与控制面
- host-backed CUDA / AI orchestration 继续保留，但必须建立在稳定的资源与实例模型上
- 开发者体验与更丰富 UI/查询能力作为第三优先级推进

## 九、当前默认结论

- GOS 当前的系统身份是 graph-native + supervisor-native
- legacy island 是待清零技术债，不是架构中心
- 文档与实现都必须围绕 builtin graph boot、runtime graph semantics、supervisor system cycle 这三条主轴展开
