# 图原生执行模型 (Graph-Native Execution Model)

在传统操作系统中，往往由隐式的调度器以及深层的队列状态（如 Ready Queue、Wait Queue）来管理上下文和多任务。由于 `GOS` 的核心理念是基于图的节点激活与路由控制，这种传统的基于隐式排队的上下文管理将造成明显的架构违和（Hidden State）并且破坏系统拓扑的一致性（Idempotence）。

为此，GOS 采用彻底的**图原生执行模型**，即硬件核心资源（CPU Core）的挂起、切换与调度，映射为纯粹的**图结构运算（Graph Mutation）**。

## 一、基础系统映射关系

### 1. 实体节点（Nodes）

- **核节点（Core Node）**：`k-core` 插件实例映射了真实的物理计算单元。例如向量 `[2, 0, 0, 0]` 代表 `native.core.0`。
- **实例节点（Instance Node）**：由 `gos-supervisor` 统一注册管理的执行单元（例如 `native.vga` 或 `native.net` 的某个运行时实例）。这些实例挂载着自己的 `TaskContext` （寄存器状态、物理执行栈和分页表 CR3 等等硬件环境快照），它们是以静态属性（Payload Data）形式存储在其宿主节点描述里的。

### 2. 关系边（Edges）

系统的执行状态不再依赖于“状态变量更新”，而是取决于核心节点的显式有向边指向何处：

- **`[EXECUTES]` 边**：只有 `Core Node` 可以发起。当核心图查询中存在 `Core.0 -[EXECUTES]-> Instance.X` 时，它不仅意味着 CPU "*正分配给实例 X*"，更意味着当前系统层面的硬件所有权正被此实体控制。
- **`[READY]` 边**：执行实例想要获取 CPU 资源时所创建的请求边，方向指向 `Core Scheduler Queue` 集合。
- **`[WAIT_FOR]` 边**：当执行体触发阻塞机制（例如网络 IO）时，不再进入“阻塞队列”，而是发生删边和连边操作，表现为 `Instance.X -[WAIT_FOR]-> Resource.Net` 的存在。

## 二、拓扑突变引发的系统调度机制

在这个模型中，“**上下文切换（Context Switch）**”在宏观上是执行一次完全的事务性边操作，而在微观（边缘拦截器层）会映射成直接的硬件切换副作用：

1. **原执行体放弃或被抢占**
   当时钟中断到达、或者当前实例主动调用 Yield 逻辑。
2. **图操作评估与应用**
   引擎会进行对应的拓扑替换：
   - 析构原来的所有权边（Delete `[EXECUTES]`）。
   - 建立新的执行权边（Merge `[EXECUTES]` 连向原先位于 `[READY]` 的另一极）。
3. **副作用执行（硬件层）**
   图处理逻辑检测到 `[EXECUTES]` 的改变后，直接通过读取目标节点附带的 `TaskContext` payload，触发底层的 Ring 0 级汇编寄存器交换指令 `switch_context`。 

## 三、带来的架构收益与不变式维护

1. **完全零不可见状态（All State Explicit）**
   任何时候在终端执行 `MATCH (c:Core)-[e:EXECUTES]->(i:Instance) RETURN i`，便可以真实、精确查询整个芯片调度矩阵现状，没有任何隐藏数组状态存在。
2. **极简操作隔离（Loose Coupling & Operator Isolation）**
   控制 CPU 强杀、挂起或者提权，完全脱离传统 `suspend_process` API 的层层调用链路，仅需对图进行结构层面的断开和重建即生效。
3. **去中心化追踪（Observability）**
   所有死锁问题、超时和状态机的调试，可以转化为一次基于图连通性的死循环图查找算法。

## 四、k-core 的工作流

- 提供具有确定身份标识的图节点接入规范（含有 `NODE_VEC` 和 `EXECUTOR_VTABLE`）。
- 解析图内核事件中心关于 `EXECUTES` 指向目标的变动。
- 作为汇编基础库层面上 `TasksContext` 的唯一操作者和管理者，实现真正的物理环境控制反转。
