# GOS Graph Runtime v0.1 规范设计文档

## 执行摘要
本规范提出 GOS Graph Runtime v0.1 的可实现内核原型设计：以 Graph 作为统一内核抽象，将传统 OS 的“进程/线程/文件/设备/权限/IPC”统一映射为 node + edge，并在此之上引入 **向量寻址（Vector Addressing）** 与 **插件化内核**（可热插拔策略/驱动/运行时扩展）。核心目标是在 v0.1 阶段做到：可启动（boot）、可运行（调度与执行语义闭合）、可存储（图化文件系统与快照）、可隔离（能力模型与沙箱）、可观测（调试与指标）、可扩展（插件与设备模型），并为后续分布式/一致性与更强兼容层打下接口稳定基础。

### 关键设计决策如下
1. **统一执行语义**：节点执行采用“事件驱动 + 数据流可选”的混合模型；确定性语义参考 Kahn Process Networks（KPN）。
2. **调度器**：默认 Hybrid Scheduler：事件优先队列 + 数据就绪队列 + 批处理器。
3. **内存模型**：采用三层寻址（Physical / Logical / Vector）。引用语义以“句柄/能力（handle/capability）”为核心。
4. **安全模型**：以 edge 作为授权边界，构建 capability-based ACL；内核策略以“可插拔 hook 框架”组织。
5. **兼容层**：v0.1 提供“POSIX 子集→图 syscalls 映射”的可跑通路径。
6. **Graph FS**：图化文件系统采用“内容寻址对象库（Merkle-DAG）+ CoW 快照/克隆 + 流式增量复制”。

## 概览与目标范围
### 目标范围
v0.1 的“必须可实现”边界（以“可做内核原型”为准）：

**必须**：
- Graph Runtime 的对象模型与执行语义闭包；
- Scheduler 最小可用；
- Memory Model 最小闭环；
- Security Model 最小闭环；
- Boot Model；
- Graph FS；
- Developer Model；
- 一套可跑的 Compatibility Layer 子集。

**可选/延后**：
- GPU 深度优化；
- 分布式强一致数据面；
- 现成 Windows/ELF 二进制“直接运行”。

### 假设与未指定项清单
- **目标硬件**：x86_64 或 AArch64；支持 IOMMU 与至少一类 GPU。
- **内核实现语言**：Rust 优先。
- **节点代码形态**：v0.1 同时支持“内核内原生节点”与“用户态沙箱节点”（WASM作为主要候选）。
- **向量寻址**：v0.1 采用“特权向量服务节点 + 内核 syscalls 转发”。

## Graph Runtime 统一执行语义
### 设计决策
- 决策一：node 是“可调度执行单元”，edge 是“可授权通信与依赖关系”。
- 决策二：统一执行模型采用“事件驱动 + 可选数据流确定性层”。
- 决策三：node 以显式状态机执行，保证可暂停/可快照/可迁移。

*(详细ABI定义和节点回调参见规范正文的Rust草案)*

## Scheduler 调度与并发
### 设计决策
- 决策一：混合调度模型（dataflow/event/priority）。
- 决策二：CPU/GPU 分工：CPU 负责“图控制面”，GPU 负责“批处理数据面”。
- 决策三：并发控制与确定性分离。

## Memory Model 与持久化
### 设计决策
- 决策一：三层寻址（Physical / Logical / Vector）。
- 决策二：数据位置策略（RAM/GPU/持久化）以“Buffer 对象 + placement policy”表达。
- 决策三：引用语义统一为 Capability Handle。

### GC/回收策略
v0.1 不建议做“全局追踪 GC”，建议组合：内核对象引用计数（RC）+ 循环引用采用强/弱引用分层所有权设计。

## Security Model
### 设计决策
- 决策一：以 capability 为唯一权能，edge ACL 为主要访问控制面。
- 决策二：沙箱分层：用户态 WASM 为默认不可信执行容器，eBPF 为可选管控层。
- 决策三：策略框架采用“hooks + 策略插件”结构（类似 LSM）。

## 生态与兼容与设备与启动
- **Compatibility Layer**：建立 POSIX/syscall 的映射。
- **Device Model**：驱动是 node，I/O 模型采用 ring/queue (类似 io_uring)，虚拟设备优先。
- **Graph FS**：对象存储采用内容寻址，提供 CoW 与快照。
- **Boot Model**：root node、init graph 与基于证书验证的安全启动。

*文档最终将作为该仓库 Phase B/C 的准则依据*
