# GOS 硬化日志 — V2.23
**日期：** 2026-07-01
**分支：** main
**Commit 范围：** V2.22 → V2.23

---

## 概述

V2.23 新增了 `node info <vec>` —— 一个综合性的单节点状态视图，将 `stat` 与
内联的边列表合并为一条命令，类比 Linux 上的 `systemctl status <unit>` 或
Kubernetes 中的 `kubectl describe pod <name>`。该命令为操作者提供了针对
任意节点的单一视窗：身份、生命周期、累计信号计数，以及触及该节点的全部边
（出边和入边）。

---

## 新增 Shell 命令一览

| 命令 | 别名 | 对应的 Linux 命令 | 描述 |
|---------|---------|---------------------|-------------|
| `node info <vec>` | `ninfo <vec>` | `systemctl status <unit>` | 综合性单节点视图：stat + 边 |

### 输出格式

```
 node info
  vector:        6.1.0.0
  key:           shell.main
  plugin:        K_SHELL
  lifecycle:     ready
  signal_count:  0
  edge_out:      2
  edges (3):
    out  6.1.0.0 -[use]-> 6.1.1.0  theme.use
    out  6.1.0.0 -[mount]-> 6.1.4.0  clipboard.mount
    in   6.1.3.0 -[use]-> 6.1.0.0
  hint: stat <vec> for counters | edges <type> for type filter
```

- **out**（绿色）= 从该节点发出的边（出边）
- **in**（品红色）= 指向该节点的边（入边）
- 生命周期颜色编码：绿色 = running，黄色 = suspended，红色 = faulted

---

## 使用的 API 面（无新增 API）

V2.23 将两个既有的 V2.x API 组合为单一的分发函数：

| API | 引入版本 | `node info` 中的用途 |
|-----|-----------|------------------------|
| `gos_runtime::proc_stat_for_vector(vec)` | V2.15 | stat 区块：key、plugin、lifecycle、signal_count、edge_out_count |
| `gos_runtime::edge_page_for_node(vec, 0, &mut edges)` | V2.12 | 带方向标签的内联边列表 |

未新增任何 runtime 状态。所有分发逻辑均为纯读取 —— 不推进 epoch，不产生
写操作。

---

## 代码修改

### `crates/k-shell/src/lib.rs`

- 新增 `pub fn dispatch_node_info(sink: &ConsoleSink, vec: VectorAddress)`
  - 调用 `proc_stat_for_vector` → 打印身份 + 生命周期区块
  - 调用 `edge_page_for_node` → 打印带出/入方向标签的内联边列表
  - 配色方案：出边绿色，入边品红色，错误红色，提示灰色
  - 优雅处理 "not found"（红色）和 "no edges"（灰色）情形

### `crates/k-shell/src/proc.rs`

- 在 `dispatch_text_command` 中新增命令路由：
  - `node info <vec>` → `dispatch_node_info`
  - `ninfo <vec>` → `dispatch_node_info`（简短别名）
- 更新 `help`，列出 `node info <vector>` 和 `ninfo <vector>`

---

## Host 测试套件

**Crate：** `host-tests/gos-node-info-harness`
**测试文件：** `tests/node_info.rs`
**测试数：** 10 / 10 通过

| # | 测试 | 验证内容 |
|---|------|-----------------|
| 1 | `node_info_stat_unknown_returns_none` | 未注册的 vector → `proc_stat_for_vector` 返回 None |
| 2 | `node_info_stat_registered_node_returns_correct_key` | stat 返回正确的 `local_node_key` |
| 3 | `node_info_edge_page_unknown_returns_not_found` | 未注册的 vec → `edge_page_for_node` 返回 `NodeNotFound` |
| 4 | `node_info_no_edges_returns_zero` | 无边节点 → (total=0, returned=0) |
| 5 | `node_info_one_edge_returned_for_source_node` | `register_edge` 之后 → 源节点可见 1 条边 |
| 6 | `node_info_edge_directions_correct` | 源节点 → Outbound；目标节点 → Inbound |
| 7 | `node_info_signal_count_starts_at_zero` | 新节点 `signal_count == 0` |
| 8 | `node_info_edge_out_count_matches_registered_edges` | `register_edge` 之后 `edge_out_count` 递增 |
| 9 | `node_info_edges_visible_after_fault` | 故障节点的边依然可见 |
| 10 | `node_info_edges_visible_after_resume` | 恢复的节点：lifecycle=Ready，边保持完整 |

---

## Host 测试套件总计

| 套件 | 测试数 | 备注 |
|---------|-------|-------|
| gos-runtime-harness | 26 | |
| gos-supervisor-harness | 16 | |
| gos-rewrite-harness | 12 | |
| gos-rewrite-integration-harness | 6 | |
| gos-subscribe-harness | 10 | |
| gos-metrics-harness | 10 | |
| gos-boot-harness | 11 | |
| gos-node-inspect-harness | 8 | |
| gos-journal-harness | 14 | |
| gos-edge-inspect-harness | 10 | |
| gos-graph-diff-harness | 10 | |
| gos-proc-harness | 10 | V2.14 |
| gos-stat-harness | 10 | V2.15 |
| gos-graph-diff-epoch-harness | 10 | V2.16 |
| gos-graph-topo-harness | 10 | V2.17 |
| gos-graph-health-harness | 10 | V2.18 |
| gos-theme-node-harness | 10 | V2.19 |
| gos-plugin-list-harness | 10 | V2.20 |
| gos-kill-harness | 10 | V2.21 |
| gos-resume-harness | 10 | V2.22 |
| **gos-node-info-harness** | **10** | **V2.23（新增）** |
| **总计** | **233** | **全部通过** |

---

## 不变式的保留

- `dispatch_node_info` 是纯读取 —— 不推进 epoch，不产生写操作
- `edge_page_for_node` 以 `N=16` 调用；当 total > 16 时打印翻页提示
- 全部 10 个测试均使用 `TEST_LOCK: Mutex<()>` + `reset()` 以实现隔离
- 该测试套件拥有自己的 `.cargo/config.toml`，其中 `target = "x86_64-pc-windows-msvc"`

---

## 后续步骤（V2.24 候选项）

- `graph watch` / `watch nodes` —— 自动刷新的节点表（类似 `watch -n1 proc`）
- `journal ring <N>` —— 运行时可配置的 JournalRing 容量
- PAL_U32 → 属性节点重构（Demo A 前置条件）
- `node trace <vec>` —— 单节点的信号分发历史
