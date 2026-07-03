# HARDENING LOG — V2.62: pal full — all-4 palette entries graph-native

**Date:** 2026-07-03  
**Version:** V2.62  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.62 完成调色板图原生化重构：V2.57 已将 pal[0]（RED/wabi）和 pal[1]（WHITE/shoji）
绑定至图节点，V2.62 将 pal[2]（CYAN）和 pal[3]（GOLD）补全，所有 4 个调色板条目
现在均从图节点属性读取，PAL_U32 常量仅作后备（节点属性缺失时静默 fallback）。

V2.62 completes the palette graph-native refactor started in V2.56/V2.57. Two new
graph nodes — `palette.cyan` and `palette.gold` — are registered at boot and hold
their respective color values as u32 node attributes. At Desktop init, all four
`pal_u32` entries are now populated from the graph; PAL_U32 compile-time constants
serve only as silent fallbacks when attrs are absent.

**图理意义：** 调色板是 GOS 视觉身份的核心。将其存入图中意味着颜色配置可以像
其他图属性一样被 Cypher 查询、订阅、权限控制——符合"一切皆图"的图论 OS 哲学。

---

## 变更范围 / Change Scope

### 1. `crates/gos-protocol/src/vectors.rs`

新增两个 VectorAddress 常量：

```rust
pub const SVC_SHELL_PALETTE_CYAN: VectorAddress = VectorAddress::new(6, 1, 5, 0); // V2.62
pub const SVC_SHELL_PALETTE_GOLD: VectorAddress = VectorAddress::new(6, 1, 6, 0); // V2.62
```

填充 (6,1,5,0) 和 (6,1,6,0)，紧随现有 `SVC_SHELL_CLIPBOARD (6,1,4,0)`。

### 2. `crates/k-shell/src/lib.rs`

**新 VectorAddress 常量**（pub，供 fbtest.rs 使用）：

```rust
pub const PALETTE_CYAN_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 5, 0);
pub const PALETTE_GOLD_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 6, 0);
```

**新 ExecutorId 和 VTABLE**（被动数据节点，无事件处理器）：

```rust
pub const PALETTE_EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.pal");
pub const PALETTE_EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: PALETTE_EXECUTOR_ID,
    on_init: None, on_event: None, on_suspend: None,
    on_resume: None, on_teardown: None, on_telemetry: None,
};
```

**新 NodeId 常量**（在 shell_on_init 中用于注册 u32 属性）：

```rust
const PALETTE_CYAN_NODE_ID: gos_protocol::NodeId = derive_node_id(SHELL_PLUGIN_ID, "palette.cyan");
const PALETTE_GOLD_NODE_ID: gos_protocol::NodeId = derive_node_id(SHELL_PLUGIN_ID, "palette.gold");
```

**`shell_on_init` 新增注册**：

```rust
// V2.62: bind CYAN and GOLD to dedicated palette nodes.
// PAL_U32[2]=0x0000_CCFF (CYAN), [3]=0x00FF_CC44 (GOLD).
let _ = gos_runtime::register_node_prop_u32(PALETTE_CYAN_NODE_ID, 0x0000_CCFF);
let _ = gos_runtime::register_node_prop_u32(PALETTE_GOLD_NODE_ID, 0x00FF_CC44);
```

### 3. `crates/hypervisor/src/builtin_bundle.rs`

`SHELL_NATIVE_NODES` 新增两个 `NativeNodeBinding`：

```rust
NativeNodeBinding {
    vector: k_shell::PALETTE_CYAN_NODE_VEC,
    local_node_key: "palette.cyan",
    executor: k_shell::PALETTE_EXECUTOR_VTABLE,
}, NativeNodeBinding {
    vector: k_shell::PALETTE_GOLD_NODE_VEC,
    local_node_key: "palette.gold",
    executor: k_shell::PALETTE_EXECUTOR_VTABLE,
}
```

这使 `node_id_for_vec(PALETTE_CYAN/GOLD_NODE_VEC)` 在 Desktop init 时能解析出
NodeId，进而通过 `node_attr_get` 读取颜色值。

### 4. `crates/hypervisor/src/fbtest.rs`

Desktop `init()` 中 pal_u32 填充扩展至全 4 项：

```rust
// V2.57/V2.62: populate all 4 pal_u32 entries from graph node attrs.
if let Some(c) = without_interrupts(|| gos_runtime::node_attr_get(k_shell::THEME_WABI_NODE_VEC))  { d.pal_u32[0] = c; }
if let Some(c) = without_interrupts(|| gos_runtime::node_attr_get(k_shell::THEME_SHOJI_NODE_VEC)) { d.pal_u32[1] = c; }
if let Some(c) = without_interrupts(|| gos_runtime::node_attr_get(k_shell::PALETTE_CYAN_NODE_VEC)){ d.pal_u32[2] = c; }  // V2.62
if let Some(c) = without_interrupts(|| gos_runtime::node_attr_get(k_shell::PALETTE_GOLD_NODE_VEC)){ d.pal_u32[3] = c; }  // V2.62
```

### 5. `host-tests/gos-pal-full-harness/` (新建)

- `Cargo.toml` — `[workspace]` 隔离，依赖 gos-protocol / gos-runtime / gos-supervisor
- `.cargo/config.toml` — target = x86_64-pc-windows-msvc, build-std
- `tests/pal_full.rs` — 10 个测试全绿

---

## 节点图谱 / Node Topology (Shell group, L1=6, L2=1)

```
(6,1,0,0)  shell.entry       — k-shell 主节点    EXECUTOR_VTABLE
(6,1,1,0)  theme.wabi        — wabi 主题节点     THEME_EXECUTOR_VTABLE  (V2.56: u32=RED)
(6,1,2,0)  theme.shoji       — shoji 主题节点    THEME_EXECUTOR_VTABLE  (V2.56: u32=WHITE)
(6,1,3,0)  theme.current     — 当前主题指针       THEME_EXECUTOR_VTABLE
(6,1,4,0)  clipboard.mount   — 剪切板挂载点       CLIPBOARD_EXECUTOR_VTABLE
(6,1,5,0)  palette.cyan      — CYAN 调色板节点   PALETTE_EXECUTOR_VTABLE  (V2.62: u32=0x0000_CCFF)
(6,1,6,0)  palette.gold      — GOLD 调色板节点   PALETTE_EXECUTOR_VTABLE  (V2.62: u32=0x00FF_CC44)
```

---

## 调色板体系 / Palette System Status

V2.62 后调色板全部图原生化：

| 索引 | 颜色  | 图节点             | u32 默认值      | 版本  |
|------|-------|--------------------|-----------------|-------|
| 0    | RED   | theme.wabi         | 0x00DB_1C21     | V2.56 |
| 1    | WHITE | theme.shoji        | 0x00ED_EDF2     | V2.56 |
| 2    | CYAN  | palette.cyan ✅    | 0x0000_CCFF     | **V2.62** |
| 3    | GOLD  | palette.gold ✅    | 0x00FF_CC44     | **V2.62** |

**下一步（已记录）**：当所有 4 项均有稳定图节点后，可考虑将 `PAL_U32` 常量
标记为 deprecated 并最终删除（届时 fallback 路径将成为不可达代码）。

---

## 测试矩阵 / Test Matrix (gos-pal-full-harness)

| # | 测试名 | 验证点 | 结果 |
|---|--------|--------|------|
| 1 | `cyan_override_replaces_pal_index_2` | cyan attr → pal[2] 覆盖默认 CYAN | ✅ |
| 2 | `gold_override_replaces_pal_index_3` | gold attr → pal[3] 覆盖默认 GOLD | ✅ |
| 3 | `no_cyan_attr_falls_back_to_default_cyan` | 无属性时 pal[2] = PAL_CYAN fallback | ✅ |
| 4 | `no_gold_attr_falls_back_to_default_gold` | 无属性时 pal[3] = PAL_GOLD fallback | ✅ |
| 5 | `all_four_entries_set_populates_full_palette` | 4项全部图原生化，全覆盖 | ✅ |
| 6 | `cyan_override_changes_rope_contrast_color` | PAL_CONTRAST[2]=3，CYAN节点绳使用 pal[3] | ✅ |
| 7 | `gold_override_changes_rope_contrast_color` | PAL_CONTRAST[3]=2，GOLD节点绳使用 pal[2] | ✅ |
| 8 | `reset_and_re_register_restores_cyan_attr` | reset→重注册后 cyan attr 恢复 | ✅ |
| 9 | `reset_and_re_register_restores_gold_attr` | reset→重注册后 gold attr 恢复 | ✅ |
| 10 | `partial_init_only_cyan_gold_leaves_wabi_shoji_at_defaults` | 仅 cyan/gold 设置时 wabi/shoji 保持默认 | ✅ |

**全部通过 10/10**

---

## 不变量 / Invariants (never break)

- `PALETTE_EXECUTOR_VTABLE` 的所有钩子为 `None`——palette 节点是纯被动数据节点
- `register_node_prop_u32` 在 shell_on_init 中被调用；节点通过 `NativeNodeBinding`
  在 builtin_bundle.rs 中注册，保证 `node_id_for_vec()` 在 Desktop init 前已可用
- `pal_u32` 在 Desktop 静态初始化时设为 `PAL_U32`（fallback），`init()` 再覆盖
- `render_frame` 不锁 RUNTIME，所有 pal_u32 数据在 `init()` 时缓存（fbtest invariant）
- VectorAddress 命名空间：L4=38 保留给 gos-pal-full-harness 测试节点

---

## 累计测试数 / Cumulative Test Count

- 本次新增：10 tests (gos-pal-full-harness)
- 累计：**593 host tests** (583 + 10)
