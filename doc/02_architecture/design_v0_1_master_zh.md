# GOS 工作区设计总览

> 文件路径沿用历史名称，内容已改为当前工作区的设计总览，不再作为旧版分层设计草案。

## 一、设计总原则

当前工作区按以下原则组织：

- `hypervisor` 只保留最小引导职责
- graph 是系统公开结构
- runtime 负责图执行
- supervisor 负责模块与资源控制面
- 插件优先采用 manifest-native / executor-driven 模型

## 二、工作区责任地图

### 2.1 引导与控制核心

| crate | 职责 |
|---|---|
| `hypervisor` | 最小引导、builtin graph bootstrap、steady-state handoff |
| `gos-protocol` | graph ABI、module / instance / resource / heap 公共协议 |
| `gos-runtime` | node/edge 注册、激活、路由、capability 解析、图摘要 |
| `gos-supervisor` | module/domain/instance/resource/heap/system cycle 控制面 |
| `gos-hal` | 向量地址与元数据空间、兼容性底层支持 |
| `gos-loader` | 仍保留在 workspace，但不再是 `kernel_main` 主路径的一部分 |

### 2.2 原生图节点

| crate | 职责 |
|---|---|
| `k-shell` | 图终端、graph CLI、theme.current、clipboard.mount |
| `k-cypher` | 受控图查询与激活客户端 |
| `k-ai` | AI supervisor client / control-plane consumer |
| `k-cuda-host` | host-backed CUDA bridge |
| `k-net` | 原生网络状态与控制节点 |
| `k-ime` | 输入法控制节点 |
| `k-mouse` | 指针与显示输入节点 |
| `k-vga` | 文本显示、调色板、终端输出 |

### 2.3 当前迁移岛

以下 crate 仍处于 legacy migration island：

- `k-pit`
- `k-ps2`
- `k-idt`
- `k-pmm`
- `k-vmm`
- `k-heap`

它们的存在只代表迁移阶段，不代表推荐结构。

## 三、从 boot 到 steady-state 的责任流

### 3.1 Boot

`hypervisor` 负责：

1. CPU / 平台最低限度初始化
2. `gos-hal::vaddr` / `gos-hal::meta`
3. `gos_supervisor::bootstrap(...)`
4. 安装 builtin module descriptors
5. `builtin_bundle::boot_builtin_graph(...)`
6. `gos_supervisor::realize_boot_modules()`
7. 委托 `gos_supervisor::service_system_cycle()`

### 3.2 Runtime

`gos-runtime` 负责：

- graph registration
- node activation
- edge routing
- capability resolution
- graph summary / page view

### 3.3 Supervisor

`gos-supervisor` 负责：

- module descriptors
- module domains
- template / instance lifecycle
- ready lanes / restart queue
- resource claims
- heap grants
- steady-state service cycle

## 四、当前系统里的显式图语义

当前有两个重要的公开范例：

### 4.1 主题关系

- `theme.current -[use]-> theme.wabi`
- `theme.current -[use]-> theme.shoji`

主题切换不是 shell 私有状态，而是图中的排他 `Use` 关系。

### 4.2 共享剪贴板关系

- `node -[mount]-> clipboard.mount`

共享剪贴板不是某个窗口特判，而是图中的非排他 `Mount` 关系。

## 五、后续设计重心

工作区后续演进顺序固定为：

1. 清零 legacy island
2. 完成真实模块执行与域隔离
3. 完成实例调度、资源仲裁、私有 heap、fault 恢复
4. 再增强 shell / cypher / AI / CUDA / DX

## 六、设计结论

当前工作区的唯一推荐理解方式是：

- workspace = minimal bootstrap + graph runtime + supervisor control plane + native graph plugins
- 旧版“分层内核草案”已经不再是权威设计入口
