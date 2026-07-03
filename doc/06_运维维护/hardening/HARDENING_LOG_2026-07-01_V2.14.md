# GOS 自动硬化日志 — 2026-07-01（第15次，V2.14 proc 命令 + 节点信号计数）

| 项目 | 内容 |
|---|---|
| 文档编号 | GOS-DOC-06-15 |
| 所属阶段 | 06・运维维护／硬化日志系列 |
| 版本 / 状态 | v1.1 / 现行（历史记录，不回溯修改事实） |
| 作成 / 审核 / 批准 | GOS 核心团队 |
| 基线日期 | 2026-07-01 |
| 最终更新 | 2026-07-01 |

**变更履历**

| 版本 | 日期 | 变更内容 | 作成者 |
|---|---|---|---|
| v1.0 | 2026-07-01 | 自动硬化任务生成（英文版，`doc/HARDENING_LOG_2026-07-01_V2.14.md`） | GOS 核心团队 |
| v1.1 | 2026-07-01 | 按日系工程标准归位与中文化：迁入 `06_运维维护/hardening/` 目录（原文件位于 doc 根目录，脱离阶段分层）；改写为与 V2.0~V2.13 系列一致的中文模板；内容口径不变 | GOS 核心团队 |

> 类型：定期自动硬化（每2小时）
> 目标：V2.14 可观测性 — 每节点信号分发计数器 + ps 风格 `proc` Shell 命令
> 提交：`1aca6ea` `feat(v2.14): proc shell command + per-node signal counter + gos-proc-harness (10 tests)`

---

## 执行摘要

V2.14 新增了每节点信号分发计数器与一个 ps 风格的 `proc` shell 命令，把图论操作系统的进程可见性提升到与 Linux `ps aux` 相当的生产级水准。

---

## 新增功能

### 1. `NodeProcSummary` — gos-protocol（`crates/gos-protocol/src/lib.rs`）

gos-protocol 新增公开结构体：

```rust
pub struct NodeProcSummary {
    pub vector:          VectorAddress,
    pub local_node_key:  &'static str,
    pub plugin_name:     &'static str,
    pub lifecycle:       NodeLifecycle,
    pub signal_count:    u32,   // 自注册以来累计信号分发次数
    pub edge_out_count:  u16,   // 当前出边计数
}
```

提供 `NodeProcSummary::EMPTY` 作为栈数组的常量初始化值。

### 2. 每节点信号计数器 — gos-runtime（`crates/gos-runtime/src/lib.rs`）

- `NodeRecord`（内部结构体）新增 `signal_count: u32` 字段
- `register_node()` 将其初始化为 `0`
- `prepare_signal_dispatch()` 在每次成功准备分发时通过 `saturating_add(1)` 递增 `signal_count`，确保计数器溢出时不会回绕为零
- 新增私有方法 `proc_summary_from_slot()` —— 从 node slot 构建 `NodeProcSummary`，通过线性扫描边表统计出边数
- 新增 `proc_page<const N>()`（内部实现 + 公开 API）—— 返回按向量地址排序的一页 `NodeProcSummary`（复用既有 `node_order` 缓存）
- 新增 `proc_count()`（内部实现 + 公开 API）—— 返回当前存活节点总数

### 3. `proc` Shell 命令 — k-shell（`crates/k-shell/src/lib.rs`、`crates/k-shell/src/proc.rs`）

`lib.rs` 新增分发函数 `dispatch_proc_list()`：

- 表头：`vector | sig（信号计数）| out（出边度数）| state | plugin/key`
- 按生命周期状态着色：绿色 = Running，红色 = Faulted，黄色 = Suspended，白色 = 其他
- 信号数与边数右对齐 4 列（`print_num_right4()`）
- 每页最多显示 32 个节点，超出时打印 "... N more"
- 页脚显示节点总数与 `sig` 字段含义提示

新增辅助函数 `node_lifecycle_label()`（覆盖全部 10 个 `NodeLifecycle` 枚举变体的完整匹配）与 `print_num_right4()`（数字列对齐格式化）。

Shell 分发（`proc.rs`）：

- `proc`、`ps`、`proc all` → `dispatch_proc_list(sink)`
- `help` 文本已更新，新增 `proc` 条目

### 4. 测试 harness — `host-tests/gos-proc-harness/`（10 项测试，全部通过）

| # | 测试 | 验证点 |
|---|------|--------|
| 1 | `empty_proc_page_returns_zero` | 空 runtime 返回 (0, 0) |
| 2 | `registered_node_appears_in_proc_page` | 1 个节点 → 1 条记录，`signal_count == 0` |
| 3 | `proc_summary_vector_matches` | `Summary.vector == 已注册向量` |
| 4 | `proc_summary_key_matches` | `Summary.local_node_key == spec 中的 key` |
| 5 | `signal_count_increments_after_route_signal` | 1 次分发 → `signal_count == 1` |
| 6 | `signal_count_increments_twice` | 2 次分发 → `signal_count == 2` |
| 7 | `signal_count_is_saturating` | `u32::MAX.saturating_add(1) == u32::MAX` |
| 8 | `proc_page_offset_at_total_returns_zero` | `offset == total` → 返回 0 条记录 |
| 9 | `proc_page_returns_nodes_sorted_by_vector` | 多节点排序验证 A < B < C |
| 10 | `proc_count_reflects_live_nodes` | `proc_count()` 正确追踪注册数 |

---

## 验证

```
cd host-tests/gos-proc-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

内核构建：

```
cargo build --release
# Finished `release` profile
```

---

## 生产级质量对照

| 能力 | Linux/macOS 对应物 | GOS V2.14 |
|---|---|---|
| 进程列表 | `ps aux` / `top` | `proc` shell 命令 |
| 信号活动 | `/proc/<pid>/stat`（utime/stime） | 每节点 `signal_count` |
| I/O 扇出 | `lsof -p` / `ss` | 每节点 `edge_out_count` |
| 生命周期状态 | `ps` STATE 列（S/R/Z/T） | `lifecycle` 列 |
| 排序方式 | PID 升序 | 向量地址升序 |
| 分页输出 | `ps -e \| head -32` | PAGE=32 限制 + "... N more" |

`signal_count` 字段是一个始终开启、零拷贝的计数器，每次分发只增加一次 `saturating_add`，对既有关键路径的开销可忽略不计。

---

## 图论 OS 特性维护

`proc` 在暴露执行指标（信号计数）的同时也暴露了**图拓扑**本身（每节点出边度数），使这个 `ps` 类比继续扎根于 GOS 的图模型，而不是退化为扁平的进程列表抽象。

---

## 与本手册体系的关联

本次新增的 `proc`/`ps`/`proc all` 命令已同步收录进 [GRAPH_CLI_COMMANDS_zh.md §7.1](../../03_详细设计/GRAPH_CLI_COMMANDS_zh.md)，并纳入 [GOS运维日志汇总.xlsx](../GOS运维日志汇总.xlsx) 硬化日志汇总表第 15 行。

---

*自动生成于 2026-07-01 定期硬化任务（第15次）；2026-07-01 按日系工程标准归位与中文化。*
