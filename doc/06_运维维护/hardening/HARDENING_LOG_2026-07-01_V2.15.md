# GOS 硬化日志 — V2.15

| 项目 | 内容 |
|---|---|
| 版本 | V2.15 |
| 日期 | 2026-07-01 |
| 主题 | 单节点详情查询 `stat <vec>` / `node stat <vec>` |
| 前置版本 | V2.14（`proc` / `ps` 进程风格列表命令） |
| 测试套件 | gos-stat-harness（10 个 host 测试，全部通过） |

---

## 1. 变更目标

V2.15 新增 `proc_stat_for_vector()` 单节点单次查询能力，以及 `stat <vec>` shell 命令，
提供相当于 Linux `cat /proc/<pid>/status` 级别的单节点内省能力，与 V2.14 新增的
`proc` / `ps` 表格视图互为补充（表格总览 + 单点深挖）。

---

## 2. 修改清单

### `crates/gos-runtime/src/lib.rs`

新增内部方法：

```rust
pub fn proc_stat_for_vector(&self, vec: VectorAddress) -> Option<NodeProcSummary> {
    let slot = self.nodes.iter().position(|s| {
        s.map(|r| r.vector == vec).unwrap_or(false)
    })?;
    self.proc_summary_from_slot(slot)
}
```

新增公开 API 函数：

```rust
pub fn proc_stat_for_vector(vec: VectorAddress) -> Option<NodeProcSummary> {
    RUNTIME.lock().proc_stat_for_vector(vec)
}
```

- 采用 O(nodes) 线性扫描定位槽位——在启动期的节点规模下可接受。
- 若不存在具有该 vector 地址的已注册节点，返回 `None`。
- 复用已有的 `proc_summary_from_slot()` 构建 `NodeProcSummary`（signal_count、edge_out_count、lifecycle、key、plugin_name）。

### `crates/k-shell/src/lib.rs`

新增公开函数 `dispatch_node_stat(sink: &ConsoleSink, vec: VectorAddress)`：

- 调用 `gos_runtime::proc_stat_for_vector(vec)`。
- 若为 `None`：打印红色 `"not found: <vec>"` 并返回。
- 若为 `Some(s)`：打印包含全部六个字段的标签化区块，对 vector 与 lifecycle 做颜色编码
  （绿色=Running，红色=Faulted，黄色=Suspended，白色=其他），signal_count 以青色显示。

Running 状态节点的示例输出：

```
 node stat
  vector:        6.1.0.0       ← 绿色
  key:           k_shell::console
  plugin:        k-shell
  lifecycle:     running        ← 绿色
  signal_count:  1234           ← 青色
  edge_out:      3
```

### `crates/k-shell/src/proc.rs`

新增分发分支：

```rust
} else if let Some(vec_str) = cmd.strip_prefix("stat ").or_else(|| cmd.strip_prefix("node stat ")) {
    if let Some(vec) = gos_protocol::VectorAddress::parse(vec_str.trim()) {
        super::dispatch_node_stat(sink, vec);
    } else {
        // 错误：不是合法的 vector
    }
```

- `stat <vec>` —— 主要写法（对应 Linux 的 `stat` / `cat /proc/<pid>/status`）。
- `node stat <vec>` —— 便于发现的替代写法，与 `node <vec>` 系列保持一致。
- 帮助文本已同步补充新条目。

### `host-tests/gos-stat-harness/`（新建，10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `unknown_vector_returns_none` | 未注册 vector → None |
| 2 | `registered_node_returns_some` | 已注册节点 → Some |
| 3 | `stat_vector_matches` | summary.vector == 查询的 vector |
| 4 | `stat_key_matches` | summary.local_node_key == 规格中的 key |
| 5 | `stat_plugin_name_matches` | summary.plugin_name == manifest 中的名称 |
| 6 | `fresh_node_signal_count_is_zero` | 新节点 → signal_count == 0 |
| 7 | `stat_signal_count_after_one_dispatch` | 1 次分发 → signal_count == 1 |
| 8 | `stat_signal_count_after_two_dispatches` | 2 次分发 → signal_count == 2 |
| 9 | `stat_edge_out_count_zero_when_no_edges` | 无出边 → edge_out_count == 0 |
| 10 | `wrong_vector_returns_none_not_other_node` | 未注册 vec → None，即使其他节点存在 |

---

## 3. 测试结果

```
cd host-tests/gos-stat-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cd host-tests/gos-proc-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed  （回归检查）

cargo build --release
# Finished `release` profile
```

**回归验证（gos-proc-harness V2.14 原有 10 个测试）：全部通过，无回归。**

---

## 4. 架构意义

### 生产级质量依据

| 能力 | Linux/macOS 对应物 | GOS V2.15 |
|---|---|---|
| 单进程详情 | `cat /proc/<pid>/status` | `stat <vec>` shell 命令 |
| 状态字段 | Name、State、VmSize、Threads… | key、plugin、lifecycle、signal_count、edge_out |
| 按身份查找 | `ps -p <pid>` | 按 VectorAddress 执行 `stat <vec>` |
| 未找到时的处理 | exit 1 + 报错 | 红色 `"not found: <vec>"` 提示 |
| 综合视图 | `ps aux` | `proc`（V2.14） |
| 单节点视图 | `cat /proc/<pid>/status` | `stat <vec>`（V2.15，新增） |

这两个命令天然成对：`proc` 用于宽表总览，`stat <vec>` 用于单节点深挖——对应 Linux
`ps aux` / `/proc/<pid>/status` 的组合。

### 图操作系统特性的保留

`stat` 以 VectorAddress 作为主身份标识（而非裸整数 PID），并汇报出边数——把单节点视图
重新接回图底层。Vector 地址是 GOS 中稳定、可读的进程身份标识。

---

*自动化硬化流程 — GOS V2.15 — 2026-07-01*
