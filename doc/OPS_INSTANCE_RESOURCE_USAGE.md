# 实例资源占用视图 — Operator Guide

> 状态：已落地
> 覆盖：`gos-supervisor` 资源快照 + `k-shell` `resources`/`res` 命令
> 关联：`perf+feat: pump() lock merge, module health visibility, ai event counters`
  （`modules`/`mods` 全局健康视图的资源侧对应物）

## 一、动机

`modules`/`mods` 已经把"哪个模块挂了、重启了几次"做成了一次性快照视图（见
`c10f877`），对标 Windows 服务管理器 / systemd `status`。但运行时真正会
OOM/打满 GPU 的资源账本——堆页配额、GPU 显存配额——之前只有逐实例 API
（`instance_heap_usage(NodeInstanceId)`、`instance_gpu_usage(NodeInstanceId)`），
调用方必须已经知道某个具体的 `NodeInstanceId` 才能查。没有一个"列出当前所有
存活实例、各自吃了多少配额"的全局视图，对应 Linux `free`/Windows 任务管理器
"性能"页那种一眼看清资源压力的入口完全缺失。

## 二、新增 API

`gos_supervisor::InstanceResourceSummary`（`Copy`、no_std/无堆分配）：

```rust
pub struct InstanceResourceSummary {
    pub instance_id: NodeInstanceId,
    pub module: ModuleHandle,
    pub lifecycle: NodeInstanceLifecycle,
    pub heap_pages_used: u32,
    pub heap_pages_max: u32,
    pub gpu_bytes_used: u64,
    pub gpu_bytes_max: u64,
}

pub fn instance_resource_summaries(out: &mut [InstanceResourceSummary]) -> usize;
```

实现直接遍历 supervisor 私有的 `instances` 表（与 `module_status_summaries`
遍历 `modules` 表完全同构的模式），按 slot 顺序写入调用方提供的缓冲区，返回
写入条数；缓冲区比存活实例数短时只填前 `out.len()` 个，不分配、不阻塞。

## 三、`resources` / `res` — 全局资源视图（本次新增）

```
gos> resources
 instance resources
  instance#3  state: ready  heap: 2/32 pages  gpu: 1024/4096 bytes
  total  heap: 2/32 pages  gpu: 1024/4096 bytes
```

`k-shell` 端（`crates/k-shell/src/proc.rs` 的 `dispatch_text_command`，紧跟在
`modules`/`mods` 分支之后）调用 `instance_resource_summaries`，逐条打印
`lifecycle`/堆用量/GPU 用量，并额外累加一行全局 `total`，方便一眼判断系统整体
资源压力而不用心算每条之和。`instance_lifecycle_label`（新增于
`crates/k-shell/src/lib.rs`，与既有 `module_lifecycle_label` 同构）把
`NodeInstanceLifecycle` 渲染成短字符串。`help` 命令索引同步补了一行。

## 四、测试

`host-tests/gos-supervisor-harness/tests/supervisor.rs` 新增
`instance_resource_summaries_reports_heap_and_gpu_usage_against_quota`：
boot 一个 provider 模块后确认其主实例的堆用量（boot 时 `test_start` 已经
charge 了 2 页，与既有 `boot_realize_builds_instance_claim_and_heap_grant`
测试里 `snap.heap_pages_used == 2` 的事实一致）、`lifecycle` 为 `Ready`
（boot 完成后实例已入队但尚未被 dequeue 成 `Running`），再设置/扣减 GPU
配额、追加 `charge_heap`，确认快照随之更新。17/17
`gos-supervisor-harness` 测试通过；`gos-supervisor`、`gos-kernel`（含
`k-shell`）`cargo check` 无警告/错误；`tools/verify-graph-architecture.ps1`
通过。

## 五、已知限制 / 后续可做

- 这是只读快照，没有"资源占用过高时主动告警/降级"的策略——对应 Linux
  cgroup OOM-kill 或 Windows 资源限制策略的那一层完全没有设计，需要先有
  ADR 决定告警阈值由谁（supervisor 自身 vs 上层 AI 观察者）触发什么动作。
- `gpu_bytes_max` 为 0 表示该实例从未被授予 GPU 配额（`set_gpu_quota` 未调用
  过），与"配额耗尽"在数值上无法区分；shell 渲染上目前统一显示
  `0/0 bytes`，没有专门标注"未启用 GPU"。如果后续要做百分号渲染
  （`used/max * 100%`），这里需要先处理除零。
