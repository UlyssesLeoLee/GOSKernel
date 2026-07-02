# GOS 硬化日志 — V2.20

| 项目 | 内容 |
|---|---|
| 版本 | V2.20 |
| 日期 | 2026-07-01 |
| 主题 | `plugins` / `lsmod` 命令 + Plugin Inventory API |
| 前置版本 | V2.19（Subscribe 信号投递 + node_prop_u8 + gos-theme-node-harness） |
| 测试套件 | gos-plugin-list-harness（10 个 host 测试，全部通过） |
| Demo 进度 | 运维工具面完整度 ↑（plugins = lsmod，覆盖 Linux 运维基线） |

---

## 1. 变更目标

V2.20 填补了 **插件/模块自省** 的运维空白：

**变更前（V2.19）**：
- `graph health`、`proc`、`nodes`、`edges` 等命令均聚焦于图节点和边的可观测性。
- 没有任何命令能列出当前加载的插件/模块（类似 Linux `lsmod`），运维人员无法直接查询插件状态。

**变更后（V2.20）**：
- 新增 `plugins` / `lsmod` / `plugin list` shell 命令，输出已注册插件的名称、版本、加载状态（Discovered / Loaded / Faulted）以及各插件名下的节点数量。
- 新增 `PluginState` 公开枚举（`gos-protocol`）+ `PluginSummary` 结构体，为 shell 和测试层提供插件摘要数据。
- 新增 `plugin_page<N>()` + `plugin_count()` 运行时 API（`gos-runtime`），遵循既有 `proc_page`/`proc_count` 分页模式。

---

## 2. 修改清单

### `crates/gos-protocol/src/lib.rs`
- 新增 `pub enum PluginState { Discovered = 0x00, Loaded = 0x01, Faulted = 0xFF }` + `as_str()` 方法
- 新增 `pub struct PluginSummary { plugin_id, name, version, state: PluginState, node_count }` + `EMPTY` 关联常量

### `crates/gos-runtime/src/lib.rs`
- 导入 `PluginState`, `PluginSummary`
- `GraphRuntime` 新增私有方法 `plugin_summary_from_slot(slot) -> Option<PluginSummary>`：将内部 `PluginLoadState` 映射至公开 `PluginState`，并统计该插件名下已注册节点数
- `GraphRuntime` 新增 `plugin_page<N>(offset, out) -> (total, filled)`：分页返回插件摘要，遵循 `proc_page` 模式
- `GraphRuntime` 新增 `plugin_count() -> usize`：返回已注册插件总数
- 新增公开包装函数 `gos_runtime::plugin_page<N>(offset, out)` 和 `gos_runtime::plugin_count()`

### `crates/k-shell/src/lib.rs`
- 新增 `pub fn dispatch_plugin_list(sink: &ConsoleSink)`：格式化输出类 `lsmod` 的插件表（名称/版本/状态/节点数），颜色编码：Loaded=绿/Faulted=红/Discovered=灰

### `crates/k-shell/src/proc.rs`
- 新增命令路由：`"plugins" | "lsmod" | "plugin list"` → `dispatch_plugin_list`，插入在 `nodes summary` 和 `boot` 之间

### `host-tests/gos-plugin-list-harness/` （新建）
- `Cargo.toml`、`.cargo/config.toml`（与其他 harness 一致）
- `tests/plugin_list.rs`：10 个测试，覆盖：
  1. `plugin_count_zero_on_empty_runtime`
  2. `plugin_count_one_after_single_discover`
  3. `plugin_count_two_after_two_discovers`
  4. `plugin_page_zero_on_empty_runtime`
  5. `plugin_page_correct_name_after_discover`
  6. `plugin_page_state_discovered_before_load`
  7. `plugin_page_state_loaded_after_mark_loaded`
  8. `plugin_page_node_count_zero_before_registration`
  9. `plugin_page_node_count_matches_registered_nodes`
  10. `plugin_page_respects_page_cap`

---

## 3. 测试结果

```
running 10 tests
test plugin_count_one_after_single_discover ... ok
test plugin_count_two_after_two_discovers ... ok
test plugin_count_zero_on_empty_runtime ... ok
test plugin_page_correct_name_after_discover ... ok
test plugin_page_node_count_matches_registered_nodes ... ok
test plugin_page_node_count_zero_before_registration ... ok
test plugin_page_respects_page_cap ... ok
test plugin_page_state_discovered_before_load ... ok
test plugin_page_state_loaded_after_mark_loaded ... ok
test plugin_page_zero_on_empty_runtime ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**回归验证（gos-runtime-harness 26 测试 / gos-graph-health-harness 10 测试 / gos-subscribe-harness 10 测试 / gos-theme-node-harness 10 测试）：全部通过，无回归。**

---

## 4. 架构意义

### Linux 运维基线对齐

| GOS Shell 命令 | Linux 等价命令 | 功能 |
|---|---|---|
| `plugins` / `lsmod` | `lsmod` | 列出已加载模块/插件 |
| `graph health` | `systemctl status` + `dmesg` | 系统健康快照 |
| `nodes` / `ps` | `ps aux` | 进程/节点列表 |
| `edges` | `ss -a` / `lsof` | 连接/边列表 |
| `proc` | `ps aux` | 信号活动表 |
| `stat <vec>` | `cat /proc/<pid>/status` | 单节点状态详情 |
| `graph diff` | `git log` | 结构变更日志 |
| `graph topo` | `ip route` / `lshw` | 拓扑视图 |
| `journal` | `journalctl --version` | 日志环信息 |
| `metrics export` | `vmstat` | 系统指标导出 |
| `boot verify` | `systemctl status --boot` | 启动清单验证 |

`plugins` / `lsmod` 填补了模块自省的空白，使 GOS 的运维工具面与 Linux 标准工具对齐。

### PluginState vs NodeLifecycle

- `NodeLifecycle` 有 11 个状态（细粒度节点生命周期），面向节点调度器。
- `PluginState` 只有 3 个状态（Discovered / Loaded / Faulted），面向插件加载器——这是合理的抽象粒度分离：插件是模块，不需要调度状态机。

---

## 5. 累计 host 测试数

| 套件 | 测试数 |
|---|---|
| V2.0~V2.7 各套件 | 43 |
| gos-node-inspect-harness (V2.8) | 8 |
| gos-boot-harness (V2.9) | 11 |
| gos-metrics-harness (V2.10) | 10 |
| gos-journal-harness (V2.11) | 14 |
| gos-edge-inspect-harness (V2.12) | 10 |
| gos-graph-diff-harness (V2.13) | 10 |
| gos-proc-harness (V2.14) | 10 |
| gos-stat-harness (V2.15) | 10 |
| gos-graph-diff-epoch-harness (V2.16) | 10 |
| gos-graph-topo-harness (V2.17) | 10 |
| gos-graph-health-harness (V2.18) | 10 |
| gos-theme-node-harness (V2.19) | 10 |
| **gos-plugin-list-harness (V2.20)** | **10** |
| gos-protocol-harness | 8 |
| gos-rewrite-harness | 12 |
| gos-rewrite-integration-harness | 6 |
| gos-runtime-harness | 26 |
| gos-supervisor-harness | 16 |
| **合计** | **234** |

---

## 6. 下一步

- `graph watch` / `graph diff --live` — 连续拓扑监控（类 `watch -n1 ip route`）
- `journal ring <N>` — 运行时可配置 JournalRing 容量
- `kill <vec>` / `node fault <vec>` — 主动将节点置为 Faulted 状态（类 kill -9）
- PAL_U32 → attribute node 重构（Demo A 先决条件）
