# 模块重启计数清零（reset-failed）— Operator Guide

> 状态：已落地
> 覆盖：`gos-supervisor` 故障/重启状态机的运维收尾
> 关联：`perf+feat: pump() lock merge, module health visibility, ai event counters`
> (c10f877，已在 main) 引入的 `ModuleStatusSummary`/`modules` 命令；
> `7e03b78` 引入的事务性 bring-up + 自动回滚。

## 一、问题

`gos-supervisor` 给每个模块维护一个**生命周期累计**的 `restart_generation`
计数器：每次自动重启（`Restart`/`RestartAlways` 故障策略）或人工
`restart_module` 调用都会让它自增 1，从未自减或归零。一旦达到
`MAX_RESTARTS_BEFORE_DEGRADE`（=5），下一次故障会让模块永久 `degrade_module`
进入 `Faulted` 状态——之后新的 claim/heap charge 都会被拒绝。

这在真实运维场景里有个明显缺口：**这个计数器分不清"正在死循环崩溃"和
"几个月前崩过几次、之后一直稳定运行"**。一个模块如果在过去某个时间点
（哪怕相隔数周）累计崩溃过 5 次，即使之后长期健康运行，下一次哪怕是孤立的、
无关的故障也会让它被判定为永久降级——这与 Windows 服务管理器 / systemd
（`StartLimitIntervalSec` 滑动窗口、`systemctl reset-failed`）的语义不一致，
是产品化路上的一个真实缺口，不是假设性的。

精确的"滑动时间窗口"修复需要给 `gos-supervisor` 接入一个全局单调时钟，而
现有的 `on_tick()` 只针对*当前调度中的实例*递减时间片，不是全局计数器——
引入新时钟源属于更大改动，本次先补上可立即落地、风险更小的一环：**操作员
显式确认"根因已修复"的手动清零路径**，对标 `systemctl reset-failed`。

## 二、新增

- `gos_supervisor::clear_restart_history(handle: ModuleHandle) -> Result<(), SupervisorError>` —
  把目标模块的 `restart_generation` 归零。**不会**触碰 `ModuleLifecycle`，
  也不会尝试把模块拉起来——它只清空历史计数，让该模块不再被
  `module_status_summaries` 标记为 `degraded`，也让 `apply_fault_policy`
  在下次故障时重新有完整的重启预算。模块如果仍处于 `Faulted`，仍需要再调用
  一次 `restart_module` 才会真正回到 `Running`。
- `k-shell` `unfault <name>` 命令：把输入名字大写、定长截断到 16 字节、
  构造 `ModuleId`，在 `module_status_summaries()` 快照里按名字找到对应
  handle（不新增任何 supervisor 端"按名字查 handle"的公开 API，复用已有的
  健康快照即可定位 handle），调用 `clear_restart_history`，回显结果。
- `help` 命令的索引补一行 `unfault <name>`。

```
gos> modules
 module health
  K_AI    state: faulted  policy: restart  restarts: 5  DEGRADED

gos> unfault K_AI
 restart history cleared for K_AI

gos> modules
 module health
  K_AI    state: faulted  policy: restart  restarts: 0

gos> restart K_AI   (若该分支已合并；否则用 restart_module 等价路径)
```

## 三、测试

`host-tests/gos-supervisor-harness/tests/supervisor.rs` 新增两个用例：

- `clear_restart_history_unblocks_a_permanently_degraded_module`：把模块
  打到 `MAX_RESTARTS_BEFORE_DEGRADE` 之上进入 degraded，确认
  `clear_restart_history` 后 `restart_generation == 0`、`degraded == false`、
  但 `state` 仍是 `Faulted`（清零不等于复活）；再调用 `restart_module`
  验证模块确实能重新回到 `Running`，且计数器从 0 重新开始计。
- `clear_restart_history_rejects_unknown_handle`：未安装的 handle 返回
  `SupervisorError::ModuleNotFound`，与其它按 handle 操作的 supervisor API
  行为一致。

18/18 `gos-supervisor-harness` 测试通过；`gos-supervisor`、`gos-kernel`
（含 `k-shell`）`cargo build` 均无警告/错误；
`tools/verify-graph-architecture.ps1` 通过。

## 四、已知限制 / 后续可做

- 这是**手动**清零，不是自动衰减。真正对标 systemd
  `StartLimitIntervalSec` 的滑动窗口（"N 次故障发生在 M 秒内才计入"）需要
  给 `gos-supervisor` 接入一个不依赖硬件（host-testing 也能跑）的全局单调
  tick 源——现有 `on_tick()` 是调度器时间片专用，范围太窄，复用会引入耦合。
  这是一个独立的、值得单开 ADR 的设计点，本次不在范围内。
- `k-shell` 端用快照线性扫描名字找 handle，而不是新增一个
  `module_handle_for_id`-类公开 API——这是有意的最小化选择，避免和另一条
  未合并分支（`claude/module-restart-shell-command`，同样需要"按名字找
  handle"）在合并时产生重复符号冲突；两条分支真正合并时，应该把这两处
  按名字查找的逻辑合并成一个共享 helper。
