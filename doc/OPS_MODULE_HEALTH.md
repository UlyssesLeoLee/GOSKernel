# 模块健康与故障恢复 — Operator Guide

> 状态：已落地
> 覆盖：`gos-supervisor` 故障/重启状态机 + `k-shell` 操作员命令
> 关联：`feat(supervisor): atomic plugin bring-up with automatic rollback`、
> `perf+feat: pump() lock merge, module health visibility, ai event counters`

GOS 把"哪个服务挂了、为什么挂了、还能不能救"当作图上可观察、可操作的状态，
而不是只能靠串口日志反推。这份文档记录目前从 shell 能直接看到和操作的部分，
对标 Windows 服务管理器 / systemd 的 `status` + `restart`，但落在图模型上。

## 一、模块生命周期与故障策略（已有,本次补全文档)

`gos-supervisor` 给每个模块维护:

- `ModuleLifecycle`: `Installed -> Running -> Faulted -> ...`
- `ModuleFaultPolicy`: `Manual` / `Restart` / `RestartAlways` / `FaultKernelDegraded`
- `restart_generation`: 该模块被重启过多少次
- `degraded` (派生字段): `restart_generation >= MAX_RESTARTS_BEFORE_DEGRADE` 且处于 `Faulted`

故障发生时 (`fault_module`) 按 `apply_fault_policy` 分流:

- `Restart`/`RestartAlways` 且未到重启上限 -> 排队重启 (`enqueue_restart_module` ->
  `process_next_restart`),`restart_generation` 自增
- 到达上限 -> 放弃自动重启,降级 (`degrade_module`),不再无限重启风暴
- `FaultKernelDegraded` -> 直接降级,不重启
- `Manual` -> 保持 `Faulted`,等待人工 `restart`

模块的 bring-up (`validate -> map -> instantiate -> start`) 本身是事务性的:
任一阶段失败都会回滚该阶段之前创建的一切 (capability、消息端点、实例、独立
地址空间),模块落到干净的 `Faulted`,不会泄漏资源,也不会拖垫整个 boot —
`realize_boot_modules` 会跳过坏模块继续启动其余模块。

## 二、`modules` / `mods` — 全局健康视图

```
gos> modules
 module health
  K_NET   state: running  policy: restart-always  restarts: 0
  K_AI    state: faulted  policy: restart          restarts: 3  DEGRADED
  restart <name>  manually restart a faulted/degraded module
```

数据来自 `gos_supervisor::module_status_summaries()` — 一次性 `no_std`/无堆分配
快照,遍历所有已安装模块,带 derived `degraded` 标记,供 shell 渲染,不需要
调用方先枚举 handle。

## 三、`restart <name>` — 人工恢复 (本次新增)

之前 `restart_module(ModuleHandle)` 这个 API 已经存在 (用于 fault-policy 自动
重启路径),但操作员只知道模块的 *名字* (例如 `modules` 列表里的 `K_AI`),
没有把名字换成 handle 的入口。

新增:

- `gos_supervisor::module_handle_for_id(ModuleId) -> Option<ModuleHandle>` —
  按名字查 handle 的公开包装 (内部 `find_module_by_module_id` 早就有,只是
  之前只有一个 boot-time 内部调用点)。
- `k-shell` `restart <name>` 命令: 把输入名字大写、定长截断到 16 字节、
  构造 `ModuleId`,查 handle,调用 `restart_module`,把新状态打印回 shell。
  对标 `systemctl restart <unit>` / Windows `sc start <service>`,但作用在
  图上的模块节点而不是进程表。

```
gos> restart k_ai
 restarting k_ai  state: running
```

未知名字或重启失败都给出明确反馈 (`unknown module: ...` /
`restart failed (see modules for fault policy)`),不会静默吞掉。

## 四、覆盖的测试

`host-tests/gos-supervisor-harness/tests/supervisor.rs`:

- `module_status_summaries_reports_lifecycle_and_degraded_state` — 既有,
  验证健康快照本身。
- `module_handle_for_id_resolves_installed_module_by_name` — 本次新增,
  验证「按名字解析 handle -> 经 handle 重启 -> 模块恢复 Running」的完整
  操作员路径,以及未知名字正确返回 `None`。

## 五、后续可做 (未在本次实现)

- 重启带 backoff/冷却时间,而不是立即重试到封顶 (类似 k8s CrashLoopBackOff
  的指数退避),目前封顶后直接降级,没有"先等几秒再试"的中间档。
- 把每次 fault/restart 写入 `gos-journal`,让 `modules` 历史可追溯而不只是
  当前状态的一帧快照。
- 卡死 (而非崩溃) 的模块目前没有看门狗/心跳检测 — 协作式调度下一个不让出
  的模块只能等它自己出错或被动观察,没有主动检测机制。
