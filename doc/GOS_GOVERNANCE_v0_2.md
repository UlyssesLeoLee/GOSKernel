# GOS 治理规则 v0.2

本文档定义仓库在当前阶段的治理口径，用来保证所有后续开发继续服从：

- graph-native 表达
- supervisor-owned steady-state
- native-first 插件模型
- 零新增 legacy

## 一、治理目标

- 保持 `hypervisor` 的最小引导边界
- 强制系统通过 builtin graph 与 runtime 公开结构表达自身
- 把 `gos-supervisor` 固定为 steady-state 控制面，而不是让 `main` 重新长出业务逻辑
- 阻止新的 legacy island 进入主线
- 把 vector、capability、`mount`、`use` 维持为公开图语义

## 二、强制设计规则

### 2.1 Bootstrap Boundary

`hypervisor::kernel_main` 允许做的事仅限于：

- CPU / 平台最低限度初始化
- `gos-hal` 地址与元数据空间初始化
- `gos_supervisor::bootstrap(...)`
- 安装 builtin supervisor module descriptors
- `builtin_bundle::boot_builtin_graph(...)`
- `gos_supervisor::realize_boot_modules()`
- 把 steady-state 委托给 `gos_supervisor::service_system_cycle()`

禁止：

- 重新回到 `gos_loader::load_bundle`
- 直接 `gos_runtime::pump`
- 直接 `plugin_main(...)`
- 直接 `post_signal(...)` 手工拼业务启动顺序

### 2.2 Native-First Plugin Rule

所有新插件必须是 manifest-native、executor-driven：

- `PluginManifest`
- `NodeSpec`
- 权限声明
- import / export 声明
- 稳定 `local_node_key`
- `NodeExecutorVTable`
- 明确的 vector policy

任何新代码都不得再引入新的 `NodeCell` / `PluginEntry` / `try_mount_cell` 路径。

### 2.3 Graph-Mediated Interaction

节点之间的协作必须通过图结构表达：

- `Depend`
- `Mount`
- `Use`
- capability import / export
- runtime signal routing

禁止：

- 直接改写另一个 node 的状态页
- 用隐藏的 imperative startup call order 替代图关系
- 在 UI 或控制节点里偷偷固化系统结构

### 2.4 Identity and Vector Rule

统一口径：

- `PluginId` 是插件归属身份
- `NodeId` 是稳定逻辑身份
- `VectorAddress` 是运行位置
- `EdgeId` 是稳定逻辑边身份
- `EdgeVector` 是控制台和图浏览使用的边地址

任何重启、迁移、模块切换，都必须优先保证 `NodeId` 稳定。

### 2.5 Public Graph Semantics Rule

图中的公开控制面必须保持真实语义，不允许“UI 特判覆盖图”：

- 主题状态由 `theme.current -[use]-> theme.*` 表达
- 共享剪贴板由 `node -[mount]-> clipboard.mount` 表达
- capability 依赖应能在图中被浏览和解释

如果一个用户可见系统关系不能在 graph summary 中解释清楚，设计就是错误的。

### 2.6 Supervisor Rule

`gos-supervisor` 是 steady-state 控制面，负责：

- module descriptor install
- boot module realization
- instance lanes
- claims / revokes
- heap grants
- system cycle

AI、shell、cypher 可以观察或驱动控制面，但不能绕过 supervisor / runtime 的生命周期与权限约束。

## 三、legacy island 政策

当前 legacy allowlist 仅作为迁移债务存在：

- `k-pit`
- `k-ps2`
- `k-idt`
- `k-pmm`
- `k-vmm`
- `k-heap`

治理结论固定为：

- 不允许新增 legacy crate
- 不允许扩大 allowlist
- 权威文档不得把 legacy 路径描述为推荐模型
- 后续路线图必须明确以清零该岛为优先事项

## 四、仓库级机械约束

仓库目前依赖以下机械约束：

- `tools/verify-graph-architecture.ps1`
- `cargo check -p gos-kernel`
- `cargo check -p k-shell`
- `gos-supervisor` host harness

治理脚本当前会校验：

- `kernel_main` 使用 `boot_builtin_graph`
- `kernel_main` 使用 `gos_supervisor::service_system_cycle`
- `kernel_main` 不再出现 `gos_loader::load_bundle`
- `kernel_main` 不再直接 `gos_runtime::pump`
- 非 allowlist crate 不得使用 legacy traits
- `NodeSpec` / `EdgeSpec` literal 必须显式带 `vector_ref`

## 五、合并检查清单

任何影响图结构、启动链、插件模型、supervisor 的变更，在合并前都必须确认：

1. graph 关系是否比 hidden control flow 更清晰
2. `kernel_main` 是否仍然保持最小引导边界
3. 是否引入了新的 legacy island
4. 是否破坏了 `NodeId` 与 `VectorAddress` 的统一口径
5. 新的 node / edge 是否显式声明 vector policy
6. 新的 orchestration 是否仍经由 runtime / supervisor 路径

## 六、当前治理结论

从现在起，仓库权威口径固定为：

- 推荐架构是 graph-native + supervisor-native
- legacy 只是 debt backlog
- 任何未来设计都必须先服务于“清零 legacy + 补齐原子化底座”，而不是绕开底座继续堆高层功能
