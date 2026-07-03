# HARDENING LOG — V2.56: pal boot — theme nodes carry palette color u32 attrs at boot

**Date:** 2026-07-03  
**Version:** V2.56  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.56 将调色板颜色写入到引导时的图节点属性中，使主题节点成为"自描述"的。
这是 PAL_U32 → 图原生重构的第二步（V2.55 建立属性存储原语，V2.56 在引导时填充）。

V2.56 wires the PAL_U32 palette colors into theme node attributes at boot, making theme
nodes self-describing: `theme.wabi` carries `0x00DB_1C21` (RED) and `theme.shoji` carries
`0x00ED_EDF2` (WHITE) as u32 node attributes. This is step 2 of the PAL_U32 → graph-native
refactor — the renderer can eventually call `node_attr_get(theme_vec)` instead of indexing
the hardcoded constant array.

---

## 变更范围 / Change Scope

### 1. `crates/k-shell/src/lib.rs`

In `shell_on_init` (after the existing `register_node_prop_u8` calls at V2.15):

```rust
// V2.56: bind each theme node's primary palette color so the renderer can
// call node_attr_get(theme_vec) instead of indexing the hardcoded PAL_U32 array.
// PAL_U32[DISPLAY_THEME_WABI=0]=0x00DB_1C21 (RED), [1]=0x00ED_EDF2 (WHITE).
let _ = gos_runtime::register_node_prop_u32(THEME_WABI_NODE_ID, 0x00DB_1C21);
let _ = gos_runtime::register_node_prop_u32(THEME_SHOJI_NODE_ID, 0x00ED_EDF2);
```

Palette color mapping (matches `const PAL_U32: [u32; 4]` in `crates/hypervisor/src/fbtest.rs`):

| 节点 / Node    | PAL_U32 index      | 颜色 / Color | u32 值         |
|---------------|--------------------|--------------|---------------|
| `theme.wabi`  | `[DISPLAY_THEME_WABI=0]` | RED   | `0x00DB_1C21` |
| `theme.shoji` | `[DISPLAY_THEME_SHOJI=1]` | WHITE | `0x00ED_EDF2` |

### 2. `host-tests/gos-pal-boot-harness/` (新建)

- `Cargo.toml` — `[workspace]` 隔离，依赖 gos-runtime/gos-protocol/gos-cypher-mut/gos-supervisor
- `.cargo/config.toml` — target = x86_64-pc-windows-msvc, build-std
- `tests/pal_boot.rs` — 10 个测试全绿

---

## 设计说明 / Design Notes

### 调色板映射 / Palette mapping

`fbtest.rs` 中的 `PAL_U32` 数组以 `DISPLAY_THEME_*` 常量（`u8`）为下标。
V2.56 利用这个自然映射：

```
DISPLAY_THEME_WABI  = 0  →  PAL_U32[0] = 0x00DB_1C21  → register_node_prop_u32(WABI_ID, RED)
DISPLAY_THEME_SHOJI = 1  →  PAL_U32[1] = 0x00ED_EDF2  → register_node_prop_u32(SHOJI_ID, WHITE)
```

剩余两个颜色（CYAN=PAL_U32[2], GOLD=PAL_U32[3]）将在 V2.57+ 视需要绑定到额外节点。

### 幂等性 / Idempotency

`register_node_prop_u32` 与 `node_attr_set` 均幂等：
- 对同一 NodeId 重复调用 → 覆盖旧值，不扩展属性槽
- `reset()` 清空整张 `node_props_u32` 表

### Epoch 不变量 / Epoch invariant

属性写入不推进 `graph_epoch`（纯元数据写，不改图拓扑）——与 V2.55 的 `node_attr_set` 保持一致。

---

## 测试矩阵 / Test Matrix

| # | 测试名 | 验证点 | 结果 |
|---|--------|--------|------|
| 1 | `wabi_node_gets_red_palette_color` | wabi 节点携带 RED 颜色属性 | ✅ |
| 2 | `shoji_node_gets_white_palette_color` | shoji 节点携带 WHITE 颜色属性 | ✅ |
| 3 | `theme_palette_colors_are_independent` | 两色同时存在且互不干扰 | ✅ |
| 4 | `palette_registration_does_not_bump_epoch` | 属性注册不推进 graph_epoch | ✅ |
| 5 | `attr_set_overwrites_boot_palette_color` | node_attr_set 可覆盖引导时注册的颜色 | ✅ |
| 6 | `attr_get_before_palette_registration_returns_none` | 注册前 get → None | ✅ |
| 7 | `wabi_red_exact_hex_value` | 精确值检查 0x00DB_1C21 (RED) | ✅ |
| 8 | `shoji_white_exact_hex_value` | 精确值检查 0x00ED_EDF2 (WHITE) | ✅ |
| 9 | `additional_nodes_do_not_corrupt_palette_attrs` | 添加第三节点不破坏调色板属性 | ✅ |
| 10 | `reset_clears_palette_attrs` | reset() 清空两个调色板属性 | ✅ |

**全部通过 10/10**

---

## 与路线图的关联 / Roadmap Alignment

```
V2.55  建立属性原语   const PAL_U32 → node_props_u32 存储层
V2.56  引导时填充     theme 节点在 shell_on_init 中写入 register_node_prop_u32  ← 本次
V2.57  渲染端消费     fbtest.rs 通过 node_attr_get(theme_vec) 读色，消灭 PAL_U32 常量
```

---

## 不变量 / Invariants (never break)

- `register_node_prop_u32` 不推进 `graph_epoch`
- `node_props_u32` 在 `reset()` 时随 `GraphRuntime::new()` 归零
- VectorAddress L4=32 保留给 gos-pal-boot-harness 测试节点

---

## 累计测试数 / Cumulative Test Count

- 本次新增：10 tests (gos-pal-boot-harness)
- 累计：**533 host tests** (523 + 10)
