# GOS 当前实施路线图（v0.2+）

> 文件路径沿用历史名称，但本文档已经不是早期 bare-metal 初始计划，而是当前系统的实施路线图。

## 一、当前基线

当前仓库已经完成的基线包括：

- `hypervisor` 采用最小引导模型
- 启动主链使用 `boot_builtin_graph + gos_supervisor::service_system_cycle`
- builtin graph 已经承载 shell、cypher、AI、network、theme.current、clipboard.mount 等用户可见节点
- `gos-supervisor` 已具备 module descriptor、domain、instance lane、claim、heap grant 等控制面骨架

当前仍未完成的核心问题：

- legacy island 尚未清零
- 真实模块镜像执行尚未完全落地
- 资源仲裁、私有 heap、fault 恢复尚未完成到底层闭环

## 二、Phase A：清零剩余 legacy island

### 目标

- 把当前 allowlist 中的 legacy crate 全部迁出主架构叙事
- 让 native-first 插件模型成为唯一推荐模型

### 交付物

- 剩余 legacy crate 逐个改为 `PluginManifest + NodeSpec + NodeExecutorVTable`
- `RULE_GRAPH_PRIME` 与治理文档改成“零新增 legacy，现有 island 按顺序清零”
- 文档中不再把 legacy 作为推荐路径描述

### 退出条件

- allowlist 显著缩小，最好收敛到零
- `kernel_main` / builtin graph / runtime / supervisor 的主链不再依赖 legacy 叙事
- 架构文档能够不提 legacy 也完整描述系统

## 三、Phase B：补齐真正的原子化底座

### 目标

在不插入新高层 feature phase 的前提下，完成原子化解耦内核底座。

### 必须落地的能力

1. 真实 module image / domain page table / CR3 切换执行
2. `NodeTemplate -> NodeInstance` 调度模型
3. `claim / lease / revoke / epoch fencing` 资源仲裁
4. 每实例私有 heap 与 supervisor 授页
5. fault attribution / restart / degraded mode

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

### 退出条件

- 至少一个真实模块能在独立 domain 中执行
- supervisor 能够管理实例、资源 claim、heap grant、fault restart 的完整闭环
- 底座能力不再依赖 legacy 兼容路径解释

## 四、Phase C：底座完成后的图原生控制面

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
