# GOS 硬化日志 — V2.19

| 项目 | 内容 |
|---|---|
| 版本 | V2.19 |
| 日期 | 2026-07-01 |
| 主题 | Theme Palette Nodes + Subscribe 自动重绘 |
| 前置版本 | V2.18（graph health 命令 + faulted_node_count + diff_ring_fill） |
| 测试套件 | gos-theme-node-harness（10 个 host 测试，全部通过） |
| Demo 进度 | Demo C（Theme 0 行代码扩散）先决条件 ✓ |

---

## 1. 变更目标

V2.19 解决 **Demo C（切 theme 0 行代码扩散）** 的核心技术缺口：

**变更前（V2.14）**：
- `apply_theme_choice_raw` 在切主题时，`sync_theme_use_edges` 触发 `fire_subscribers` 发出控制面信封（仅限 supervisor 可见），同时**显式**调用 `emit_target_signal_raw(abi, VGA_VEC, Signal::Control { cmd: DISPLAY_CONTROL_THEME, val: theme })` 通知 k-vga 重绘。
- Subscribe 机制（V2.5 已实现）只通知 supervisor 控制面，不直接触达 k-vga 的运行时信号队列。

**变更后（V2.19）**：
- `fire_subscribers` 在发出控制面信封的同时，**也向每个订阅者的运行时信号队列**投递 `Signal::Control { cmd: DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED, val }` — 使节点无须轮询控制面即可同步响应图结构变更。
- `val` 字段通过 **node_prop_u8 存储**和 **Use 边目标查询**自动编码：k-shell 在初始化时注册 `THEME_WABI_NODE_ID → 0x00`、`THEME_SHOJI_NODE_ID → 0x01`，runtime 的 `fire_subscribers` 通过 `active_use_target(changed)` 找到当前活跃主题节点，再查 prop 得到 val。
- k-shell 的 `apply_theme_choice_raw` 删除显式 VGA 信号，主题切换完全由 Subscribe 机制驱动。

---

## 2. 修改清单

### `crates/gos-protocol/src/lib.rs`
- 新增 `pub const DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED: u8 = 0xC4`

### `crates/gos-runtime/src/lib.rs`
- 新增常量 `pub const MAX_NODE_PROPS_U8: usize = 16`
- `GraphRuntime` 新增字段 `node_props_u8: [(NodeId, u8); 16]`
- 新增方法 `register_node_prop_u8(node_id, val) -> bool`（幂等更新，满时返回 false）
- 新增私有方法 `node_prop_u8(node_id) -> Option<u8>`
- 新增方法 `active_use_target(source) -> Option<NodeId>`（返回 source 的第一条 Use 边目标）
- 修改 `fire_subscribers`：在发出控制面信封后，额外通过 `post_signal` 向每个订阅者的运行时队列投递 `Signal::Control { cmd: DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED, val: signal_val }`
- 新增公开包装函数 `register_node_prop_u8(node_id, val) -> bool`
- 新增公开包装函数 `active_use_target(source) -> Option<NodeId>`
- 新增公开包装函数 `drain_signal() -> Option<(VectorAddress, Signal)>`（供测试套件验证信号投递）

### `crates/k-vga/src/lib.rs`
- `handle_control` 新增 `DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED` 分支：接收到信号时调用 `apply_theme_palette(val.min(DISPLAY_THEME_SHOJI))`，与现有 `DISPLAY_CONTROL_THEME` 行为对称

### `crates/k-shell/src/lib.rs`
- `shell_on_init`：新增 `register_node_prop_u8(THEME_WABI_NODE_ID, DISPLAY_THEME_WABI)` 和 `register_node_prop_u8(THEME_SHOJI_NODE_ID, DISPLAY_THEME_SHOJI)` 注册，以及 `register_subscribe(THEME_CURRENT_NODE_ID, k_vga_node_id)` 订阅对注册
- `apply_theme_choice_raw`：删除对 k-vga 的显式 `emit_target_signal_raw`（DISPLAY_CONTROL_THEME），主题切换完全通过 Subscribe 信号驱动；参数重命名 `abi → _abi`、`console_target → _console_target`
- 删除已不使用的 `DISPLAY_CONTROL_THEME` 导入

### `host-tests/gos-theme-node-harness/` （新建）
- `Cargo.toml`、`.cargo/config.toml`（与其他 harness 一致）
- `tests/theme_node.rs`：10 个测试，覆盖：
  1. `active_use_target_none_when_no_use_edge`
  2. `active_use_target_returns_correct_node_after_use_edge`
  3. `active_use_target_ignores_non_use_edges`
  4. `node_prop_u8_roundtrip_via_subscribe_signal_val`
  5. `subscribe_signal_val_zero_when_no_prop_registered`
  6. `subscribe_signal_val_updates_on_use_edge_switch`
  7. `node_prop_u8_overwrite_updates_existing_entry`
  8. `node_prop_u8_table_full_returns_false`
  9. `subscribe_triggered_not_delivered_without_subscribe_pair`
  10. `subscribe_signal_delivered_to_correct_subscriber_vector`

---

## 3. 测试结果

```
running 10 tests
test active_use_target_ignores_non_use_edges ... ok
test active_use_target_none_when_no_use_edge ... ok
test active_use_target_returns_correct_node_after_use_edge ... ok
test node_prop_u8_overwrite_updates_existing_entry ... ok
test subscribe_signal_delivered_to_correct_subscriber_vector ... ok
test node_prop_u8_table_full_returns_false ... ok
test node_prop_u8_roundtrip_via_subscribe_signal_val ... ok
test subscribe_signal_val_zero_when_no_prop_registered ... ok
test subscribe_signal_val_updates_on_use_edge_switch ... ok
test subscribe_triggered_not_delivered_without_subscribe_pair ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**回归验证（gos-subscribe-harness V2.5 原有 10 个测试）：全部通过，无回归。**

---

## 4. 架构意义

### Demo C 先决条件状态

| 条件 | 状态 |
|---|---|
| V2.5 Subscribe 机制（控制面信封） | ✅ V2.5 已完成 |
| fire_subscribers 向运行时队列投递 Signal | ✅ **V2.19 新增** |
| k-vga 响应 SUBSCRIBE_TRIGGERED | ✅ **V2.19 新增** |
| k-shell 删除显式 VGA 广播 | ✅ **V2.19 新增** |
| node_prop_u8 encode 活跃主题 val | ✅ **V2.19 新增** |

Demo C 定义：「切 theme → 所有渲染节点自动重绘，theme 切换代码 0 行扩散」  
V2.19 后，`apply_theme_choice_raw` 中不再有任何向 k-vga 的显式信号，主题切换完全通过 Subscribe 边代数驱动。**Demo C 先决条件已全部满足。**

### node_prop_u8 通用性

`node_prop_u8` 是通用图节点属性原语，并非 theme 专用：
- 任何节点都可以注册一个 u8 属性值，当该节点作为 Use 边目标时，该值自动编码进订阅者收到的信号 val 中。
- 容量 16 槽，幂等更新，不依赖 theme 语义；可扩展用于其他 Use 边驱动的反应式场景（如热插拔模块版本号传播）。

---

## 5. 累计 host 测试数

| 套件 | 测试数 |
|---|---|
| V2.0~V2.18 各套件（含 gos-theme-node-harness 以外） | 183 |
| gos-node-inspect-harness (V2.8) | 10 |
| gos-boot-harness (V2.9) | 14 |
| gos-metrics-harness (V2.10) | 10 |
| gos-journal-harness (V2.11) | 14 |
| gos-edge-inspect-harness (V2.12) | 10 |
| gos-graph-diff-harness (V2.13) | 10 |
| gos-proc-harness (V2.14) | 10 |
| **gos-theme-node-harness (V2.19)** | **10** |
| **合计** | **131** |
