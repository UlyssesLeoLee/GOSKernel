# GOS 硬化日志 — V2.30 — 2026-07-01

## 摘要

V2.30 新增实时 proc 监视面板——`watch` / `graph watch` 命令将 VECTOR
DECK 面板切换为由现有心跳 tick 驱动的持续刷新 proc 表，类似于 Linux 上的
`watch -n1 proc` 或 `htop`。任意按键即可退出 watch 模式，恢复正常的
VECTOR DECK 视图。

---

## 修改内容

### 1. `WATCH_PROC_MODE` 静态变量 — k-shell（`crates/k-shell/src/lib.rs`）

```rust
pub(crate) static WATCH_PROC_MODE: AtomicU8 = AtomicU8::new(0);
```

- `0` = 正常 VECTOR DECK 视图
- `1` = 实时 proc watch 模式

### 2. `dispatch_watch_proc()` 与 `dispatch_watch_stop()` — k-shell（`crates/k-shell/src/lib.rs`）

```rust
pub fn dispatch_watch_proc(sink: &ConsoleSink) { ... }
pub fn dispatch_watch_stop(sink: &ConsoleSink) { ... }
```

- `dispatch_watch_proc`：将 `WATCH_PROC_MODE` 置为 1，打印确认信息。
- `dispatch_watch_stop`：将 `WATCH_PROC_MODE` 置为 0，打印 "watch stopped"。

### 3. `draw_watch_proc_panel()` — k-shell（`crates/k-shell/src/lib.rs`）

新增函数，用于在 watch 模式下渲染 VECTOR DECK 方框。固定布局（适配 47 × 10 字符）：

```
╔═══════════[ PROC WATCH ]══════════════╗
║ tick 12345   nodes 8   any key stops  ║
║ vector           sig  out  lifecycle  ║
║ 6.1.0.0          145   3  running     ║
║ 6.1.1.0            0   1  running     ║
║ 6.1.2.0            0   1  running     ║
║ 6.1.3.0            0   1  running     ║
║ 6.1.4.0            0   1  running     ║
║ ... 2 more                            ║
╚═══════════════════════════════════════╝
```

- 按 vector 地址显示前 6 个节点，数据均来自 `proc_page::<6>()`。
- 生命周期状态按颜色区分：绿色 = Running，红色 = Faulted，黄色 = Suspended，灰色 = 其他。
- 显示每个节点的累计 `signal_count` 与 `edge_out_count`。
- 将 `snapshot().tick` 作为实时心跳计数器展示。
- 当总数 > 6 时渲染 `... N more`。

### 4. `draw_command_deck_panel()` — k-shell（`crates/k-shell/src/lib.rs`）

早退委托：若 `WATCH_PROC_MODE != 0`，则调用 `draw_watch_proc_panel` 并直接返回。
否则渲染正常的图统计面板（对既有逻辑无改动）。

### 5. 心跳在 watch 模式下始终重绘 — k-shell（`crates/k-shell/src/proc.rs`）

```rust
let watch_active = super::WATCH_PROC_MODE.load(...) != 0;
if watch_active || current_epoch != state.last_rendered_epoch {
    ...
    super::draw_command_deck_panel(...);
}
```

V2.3 的 epoch-diff 空闲跳过逻辑在正常模式下保持不变。在 watch 模式下，面板
每 4 个心跳 tick 重绘一次，使 tick 计数器与信号计数实时更新。

### 6. 任意按键退出 watch 模式 — k-shell（`crates/k-shell/src/proc.rs`）

```rust
if source == DataSource::Keyboard
    && super::WATCH_PROC_MODE.load(...) != 0
{
    super::WATCH_PROC_MODE.store(0, ...);
    state.last_rendered_epoch = u64::MAX;  // force deck repaint
    ...
    return ExecStatus::Done;
}
```

在 watch 模式下任意键盘字节都会清除 `WATCH_PROC_MODE`，强制立即使 epoch
缓存失效，使正常面板在下一次心跳时重绘，并向滚动区域打印 "watch stopped"。

### 7. 已注册的 Shell 命令 — k-shell（`crates/k-shell/src/proc.rs`）

| 命令 | 行为 |
|---|---|
| `watch` | 进入实时 proc watch 模式 |
| `graph watch` | `watch` 的别名 |
| `watch proc` | `watch` 的别名 |
| `watch nodes` | `watch` 的别名 |
| `watch stop` | 显式退出 watch 模式 |
| `watch exit` | `watch stop` 的别名 |

`help` 文本已更新，包含全部六种变体。

### 8. 测试套件 — `host-tests/gos-watch-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `proc_page_is_idempotent` | 连续两次调用 proc_page 返回相同的总数 |
| 2 | `proc_page_empty_on_empty_runtime` | 空 runtime → watch 显示 "(no nodes)" |
| 3 | `proc_page_reflects_registration_immediately` | 注册后在下一次 proc_page 调用中立即可见 |
| 4 | `proc_count_consistent_with_proc_page_total` | proc_count() 与 proc_page 总数一致 |
| 5 | `proc_page_reflects_signal_count_after_dispatch` | 一次 dispatch 后 signal_count 实时更新 |
| 6 | `repeated_proc_page_reads_stable_after_dispatch` | 只读：重复读取不产生变更 |
| 7 | `proc_page_shows_faulted_after_fault_node` | fault_node() 反映在 lifecycle 中 |
| 8 | `proc_page_shows_running_after_resume` | resume_node() 清除 Faulted 状态 |
| 9 | `snapshot_node_count_matches_proc_count` | snapshot().node_count == proc_count() |
| 10 | `snapshot_tick_advances_after_pump` | snapshot().tick 为实时值（随 pump 递增） |

---

## 验证

```
cd host-tests/gos-watch-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

内核构建：
```
cargo build --release
# Finished `release` profile
```

---

## 生产质量考量

| 能力 | Linux/macOS 对应物 | GOS V2.30 |
|---|---|---|
| 实时进程监视 | `watch -n1 ps` / `htop` | `watch` / `graph watch` shell 命令 |
| 自动刷新 | 定时器驱动的重绘 | 心跳驱动的重绘（无线程） |
| 任意键退出 | Ctrl+C / q | 任意按键退出 watch 模式 |
| Tick 计数器 | 系统时钟 | `snapshot().tick`（图操作系统心跳） |
| 节点状态 | STAT 列 | `lifecycle` 列（Running/Faulted/Suspended） |
| 信号活动 | utime/stime | `signal_count`（每节点累计值） |
| 拓扑扇出 | 打开文件数 | `edge_out_count`（出边计数） |
| Watch 模式标志 | 进程状态 | `WATCH_PROC_MODE: AtomicU8`（零开销） |

watch 面板复用了既有的固定位置 VECTOR DECK 方框——不占用额外的终端行，
不与滚动区域冲突。该实现在非 watch 模式下每个心跳 tick 仅增加一次
`AtomicU8::load`（开销可忽略不计）。

---

## 图操作系统特性的保持

watch 面板在展示信号吞吐量（signal_count）的同时，暴露了**图拓扑指标**
（每节点出度），使实时监视器始终扎根于 GOS 的图模型，而非扁平的进程表。
VECTOR DECK 面板的 PROC WATCH 模式，映照了 `htop` 覆盖终端的方式，同时
在结构上依然保持图原生。

---

*自动化硬化流程 — GOS V2.30 — 2026-07-01*
