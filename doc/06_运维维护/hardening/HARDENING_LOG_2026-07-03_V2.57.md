# HARDENING LOG — V2.57: pal render — Desktop reads palette from graph node attrs

**Date:** 2026-07-03  
**Version:** V2.57  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.57 将 `fbtest.rs` 中的渲染路径从硬编码 `PAL_U32[ci]` 迁移为读取
`Desktop.pal_u32[ci]`——一个在 `init()` 中从图节点属性填充的缓存数组。
这完成了 PAL_U32 → 图原生重构链的前两步（V2.55 原语 → V2.56 引导写入 → V2.57 渲染消费）。

V2.57 migrates the fbtest.rs render path from hardcoded `PAL_U32[ci]` to
`Desktop.pal_u32[ci]` — a 4-entry cache populated in `init()` by querying
`node_attr_get(theme_vec)` for each theme node. Entries [0] (RED/wabi) and
[1] (WHITE/shoji) are now sourced from the live graph; entries [2] (CYAN) and
[3] (GOLD) remain at compile-time defaults until graph-backing is added in a
future slice. The render path remains entirely lockless (palette is cached once
at init, reads use `d.pal_u32[ci]` which is in Desktop state).

---

## 变更范围 / Change Scope

### 1. `crates/hypervisor/src/fbtest.rs`

**Desktop struct — 新增字段:**

```rust
// V2.57: live palette colors read from graph node attrs at init
pal_u32: [u32; 4],
```

静态初始化：`pal_u32: PAL_U32` (fallback default).

**`init()` — 新增图属性读取（在 `layout_force` 前）:**

```rust
// V2.57: populate pal_u32[0..1] from graph node attrs
if let Some(c) = without_interrupts(|| gos_runtime::node_attr_get(k_shell::THEME_WABI_NODE_VEC)) {
    d.pal_u32[0] = c;
}
if let Some(c) = without_interrupts(|| gos_runtime::node_attr_get(k_shell::THEME_SHOJI_NODE_VEC)) {
    d.pal_u32[1] = c;
}
```

**渲染路径 — 3 处 `PAL_U32[ci]` → `d.pal_u32[ci]`:**

| 位置 | 旧代码 | 新代码 |
|------|--------|--------|
| `draw_popup` 调色板色块 | `PAL_U32[ci]` | `d.pal_u32[ci]` |
| `render_frame` 绳索半段 A | `PAL_U32[PAL_CONTRAST[...a...]]` | `d.pal_u32[PAL_CONTRAST[...a...]]` |
| `render_frame` 绳索半段 B | `PAL_U32[PAL_CONTRAST[...b...]]` | `d.pal_u32[PAL_CONTRAST[...b...]]` |

`PAL_U32` 常量保留（用作 Desktop 静态初始化 fallback，`PAL_RGB` 及球体渲染另路径不变）。

### 2. `host-tests/gos-pal-render-harness/` (新建)

- `Cargo.toml` — `[workspace]` 隔离
- `.cargo/config.toml` — target = x86_64-pc-windows-msvc, build-std
- `tests/pal_render.rs` — 10 个测试全绿

---

## 设计说明 / Design Notes

### 渲染路径的 Lock-Free 不变量

`render_frame` 不锁 RUNTIME — 调色板缓存在 `init()` 中通过
`without_interrupts(|| gos_runtime::node_attr_get(...))` 一次性读取，
后续渲染帧仅读 `d.pal_u32[ci]`（Desktop 本地状态）。

### Fallback 语义

若 node_attr_get 返回 None（节点不存在或未设属性），`pal_u32[i]` 保持初始化值
`PAL_U32[i]`——与 V2.55 前行为完全一致，零回归风险。

### 进度 / Progress

```
V2.55  建立 node_props_u32 存储层
V2.56  shell_on_init 写入 wabi/shoji 节点属性
V2.57  fbtest init() 读属性 → Desktop.pal_u32 缓存，渲染路径消费  ← 本次
V2.5x  PAL_U32 → 消灭常量（CYAN/GOLD 绑定到额外节点后可删 PAL_U32 的渲染引用）
```

---

## 测试矩阵 / Test Matrix

| # | 测试名 | 验证点 | 结果 |
|---|--------|--------|------|
| 1 | `wabi_override_replaces_pal_index_0` | wabi attr set → pal[0] 覆盖 RED | ✅ |
| 2 | `shoji_override_replaces_pal_index_1` | shoji attr set → pal[1] 覆盖 WHITE | ✅ |
| 3 | `no_wabi_attr_falls_back_to_default_red` | 无 attr → pal[0] = PAL_U32[0] | ✅ |
| 4 | `no_shoji_attr_falls_back_to_default_white` | 无 attr → pal[1] = PAL_U32[1] | ✅ |
| 5 | `pal_index_2_stays_at_cyan_regardless_of_theme_attrs` | CYAN 不受 theme attrs 影响 | ✅ |
| 6 | `pal_index_3_stays_at_gold_regardless_of_theme_attrs` | GOLD 不受 theme attrs 影响 | ✅ |
| 7 | `wabi_override_changes_rope_contrast_color` | wabi 新色 → 绳索对比色正确 | ✅ |
| 8 | `shoji_override_changes_rope_contrast_color` | shoji 新色 → 绳索对比色正确 | ✅ |
| 9 | `reset_and_re_register_restores_palette_attr` | reset 后重注册可恢复 attr | ✅ |
| 10 | `partial_init_only_wabi_leaves_shoji_at_default` | 部分初始化：shoji 回退到 WHITE | ✅ |

**全部通过 10/10**

---

## 不变量 / Invariants (never break)

- `render_frame` 不锁 RUNTIME（调色板在 `init()` 中一次性缓存）
- `PAL_U32` 常量作为 Desktop.pal_u32 的静态 fallback 保留
- `pal_u32[2]` (CYAN) 和 `pal_u32[3]` (GOLD) 目前仅从常量初始化
- VectorAddress L4=33 保留给 gos-pal-render-harness 测试节点

---

## 累计测试数 / Cumulative Test Count

- 本次新增：10 tests (gos-pal-render-harness)
- 累计：**543 host tests** (533 + 10)
