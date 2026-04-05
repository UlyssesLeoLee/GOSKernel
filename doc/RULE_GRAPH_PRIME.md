# GOS Graph Prime Rules v0.2

本文档是仓库的不可谈判规则。任何子系统、插件、runtime 变更、supervisor 变更、工具链变更，都必须满足这些规则。

## 1. Prime Directive

GOS 的一切系统行为，都必须表达成：

- graph structure
- graph execution
- supervisor-owned control

持久状态属于 node。  
关系、挂载、使用、能力绑定、激活路径属于 edge。  
steady-state 不能退化成手写过程控制。

## 2. Hard Invariants

### 2.1 Bootstrap Rule

- `hypervisor` 只做最小引导
- 系统启动必须经过 `builtin_bundle::boot_builtin_graph`
- steady-state 必须委托给 `gos_supervisor::service_system_cycle`
- `kernel_main` 不得重新引入 `gos_loader::load_bundle` 或 direct `gos_runtime::pump`

### 2.2 Stable Identity Rule

- 每个插件必须拥有稳定 `PluginId`
- 每个 node 必须拥有由 `plugin_id + local_node_key` 派生的稳定 `NodeId`
- `VectorAddress` 是运行位置，不是逻辑身份
- 任何重启、迁移、热替换设计都必须优先保持 `NodeId`

### 2.3 Native-First Rule

- 所有新增插件必须是 manifest-native、executor-driven
- 不得为新功能引入新的 `NodeCell` / `PluginEntry` / `try_mount_cell`
- legacy 支持只服务于当前明确列出的迁移岛

### 2.4 Graph-Mediated Cooperation Rule

- 节点不得直接写别的节点状态页
- 跨插件协作必须经过 edge、capability、runtime routing、或 supervisor 控制面
- 不得在 `main` 或任意 plugin 中重建隐式启动顺序

### 2.5 Vector-Carrying Graph Rule

- 所有 `NodeSpec` literal 必须显式声明 `vector_ref`
- 所有 `EdgeSpec` literal 必须显式声明 `vector_ref`
- semantic 节点与 semantic 边默认应当携带真实 vector policy
- `None` 只能用于明确说明过的非语义 glue 或低层兼容路径

### 2.6 Public Semantics Rule

用户可见系统关系必须在图中真实存在：

- 主题状态必须通过 `theme.current -[use]-> theme.*`
- 共享剪贴板必须通过 `node -[mount]-> clipboard.mount`
- capability 消费必须能被解释为 import/export + mount 关系

任何用户可见行为如果只能靠“代码特判”解释，而不能靠图解释，都是架构违规。

### 2.7 Zero-New-Legacy Rule

当前 allowlist 只是一份迁移债务清单，不是长期政策。  
从现在起：

- 不新增 legacy crate
- 不扩大 allowlist
- 不把 legacy 路径写成推荐架构

## 3. Current Legacy Island

当前允许暂时保留 legacy 路径的 crate 只有：

- `k-panic`
- `k-serial`
- `k-gdt`
- `k-cpuid`
- `k-pic`
- `k-pit`
- `k-ps2`
- `k-idt`
- `k-pmm`
- `k-vmm`
- `k-heap`

除此之外，任何 crate 如果仍使用 legacy traits，都是策略违规。

## 4. Mandatory Artifacts

任何新插件或重大 graph feature 必须同时提供：

- `PluginManifest`
- `NodeSpec`
- `EdgeSpec` 或明确的图生成规则
- permission declaration
- import / export declaration
- 稳定 `local_node_key`
- `NodeExecutorVTable`
- 明确 vector policy

## 5. Review Checklist

- 系统行为是否真的用 nodes / edges 表达了？
- bootstrap 是否仍然保持最小？
- 新插件是否全部走 native-first 模型？
- 逻辑身份与运行位置是否仍然分离清晰？
- 图中的 `mount` / `use` / capability 关系是否真实可浏览？
- 是否又给 legacy 岛增加了面积？
- orchestration 是否仍通过 runtime / supervisor，而不是旁路调用？

## 6. Enforcement

仓库当前的机械执行器是：

- `tools/verify-graph-architecture.ps1`

配套最低检查命令：

```powershell
pwsh -File .\tools\verify-graph-architecture.ps1
cargo check -p gos-kernel
```

任何违反这些 prime rules 的变更，都不应该被合并。
