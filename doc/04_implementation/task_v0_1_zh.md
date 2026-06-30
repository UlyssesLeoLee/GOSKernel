# GOS 当前执行 Backlog

> 文件路径沿用历史名称，内容已切换为当前版本的执行清单。

## 已完成基线

- [x] `hypervisor` 切到 minimal bootstrap
- [x] builtin graph 启动链接管主启动路径
- [x] `gos-supervisor::service_system_cycle` 成为 steady-state 服务入口
- [x] `k-shell` 支持 graph CLI、theme.current、clipboard.mount、输入历史
- [x] `k-cypher` 提供受控 Cypher v1 子集
- [x] `k-cuda-host` 提供 host-backed CUDA bridge

## Phase A：清零 legacy island

- [ ] 为每个 legacy crate 指定迁移 owner 和迁移顺序
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
- [ ] 每迁完一个 crate，就同步收紧 verifier allowlist

## Phase B：补齐原子化底座

- [ ] 真实模块镜像格式与装载路径
- [ ] 独立 module image map / relocate
- [ ] 独立 domain page table 创建
- [ ] CR3 切换执行与返回 supervisor
- [ ] `NodeTemplate -> NodeInstance` 派生与销毁
- [ ] ready lane / restart lane / revoke lane 完整闭环
- [ ] `ResourceId` 注册与 claim/revoke/epoch fencing
- [ ] `HeapQuota`、授页、回收、超限拒绝
- [ ] fault attribution 与 restart / degraded mode
- [ ] host harness 与 QEMU smoke 同步覆盖这些底座能力

## Phase C：图原生控制面

- [ ] shell 只保留图客户端与控制职责
- [ ] cypher 文档与行为继续严格限制在受控子集
- [ ] AI 控制面统一走 supervisor / runtime 可审计路径
- [ ] CUDA bridge 绑定稳定的 resource / instance 模型
- [ ] 开发者工作流、调试、观测、可视化在底座稳定后再增强

## 文档与治理同步项

- [ ] 架构文档只讲当前事实 + 后续路线图
- [ ] 治理文档固定为零新增 legacy 口径
- [ ] README 与 CLI/Cypher/Network 文档持续和实现对齐
- [ ] 所有核心文档统一 `NodeId` / `VectorAddress` / `ModuleDescriptor` / `NodeInstance` 术语
