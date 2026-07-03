# HARDENING LOG — V2.60: node attr list u8 — u8 property table enumeration

**Date:** 2026-07-03  
**Version:** V2.60  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.60 为 u8 属性表添加 `node_attr_list_u8` 枚举函数，完成 u8 与 u32 两张属性表的对称覆盖。

V2.58 为 u32 表提供了 `node_attr_list`；V2.60 对 u8 表做了完全平行的实现：
`GraphRuntime::node_attr_list_u8_inner` → `gos_runtime::node_attr_list_u8` → shell 命令
`node attr list u8` / `nattr list u8`。

u8 属性表（`node_props_u8`，16 槽）存储主题节点的反应信号 val（WABI=0, SHOJI=1），
是 V2.15 reactive-signal 机制的核心存储。`list u8` 命令使运维人员可以实时审计所有
已注册的主题/信号 val，配合 `node attr list`（u32）形成完整的属性诊断工具集。

V2.60 completes the property-table symmetry: `node_attr_list_u8` mirrors V2.58's
`node_attr_list` for the u8 property table (`node_props_u8`, 16 slots). This table
drives the reactive signal-val encoding for theme nodes — WABI=0, SHOJI=1.
The new shell command provides real-time diagnostic visibility into all registered
u8 props, complementing the u32 palette audit from V2.58.

---

## 变更范围 / Change Scope

### 1. `crates/gos-runtime/src/lib.rs`

**新内部方法** `GraphRuntime::node_attr_list_u8_inner<const N>`:

```rust
// V2.60: List all nodes that have a u8 attribute set.
pub fn node_attr_list_u8_inner<const N: usize>(
    &self,
    out_vec: &mut [VectorAddress; N],
    out_val: &mut [u8; N],
) -> usize {
    let mut count = 0usize;
    for &(node_id, val) in self.node_props_u8.iter() {
        if node_id == NodeId::ZERO { continue; }
        if count >= N { break; }
        out_vec[count] = self.node_vector(node_id).unwrap_or(VectorAddress::new(0, 0, 0, 0));
        out_val[count] = val;
        count += 1;
    }
    count
}
```

**新公开函数** `pub fn node_attr_list_u8<const N>(out_vec, out_val) -> usize`

### 2. `crates/k-shell/src/lib.rs`

**新函数** `dispatch_node_attr_list_u8(sink)` — 输出格式：

```
 node attr list u8
  1.1.1.0  val=0
  1.1.2.0  val=1
  2 / 16 slots used
```

### 3. `crates/k-shell/src/proc.rs`

- **新路由** `cmd == "node attr list u8"` / `"nattr list u8"`（在 `node attr list` 之前，优先匹配）
- **帮助文本** 新增 `node attr list u8` 条目

### 4. `host-tests/gos-node-attr-list-u8-harness/` (新建)

- `Cargo.toml` — `[workspace]` 隔离
- `.cargo/config.toml` — target = x86_64-pc-windows-msvc, build-std
- `tests/node_attr_list_u8.rs` — 10 个测试全绿

---

## 对称性设计 / Symmetry Design

V2.60 完成了两张属性表的全覆盖：

| 操作   | u8 (node_props_u8, 16 slots) | u32 (node_props_u32, 32 slots) |
|--------|-------------------------------|--------------------------------|
| Set    | register_node_prop_u8 (V2.15) | node_attr_set / register_node_prop_u32 (V2.55) |
| Get    | node_prop_u8 [internal] (V2.15) | node_attr_get (V2.55) |
| List   | **node_attr_list_u8 (V2.60)** | node_attr_list (V2.58) |

Shell 命令对比：

| 命令               | 功能              |
|--------------------|-------------------|
| `node attr list`   | u32 表枚举（V2.58） |
| `node attr list u8`| u8 表枚举（V2.60） |
| `nattr list`       | u32 别名          |
| `nattr list u8`    | u8 别名           |

---

## 测试矩阵 / Test Matrix

| # | 测试名 | 验证点 | 结果 |
|---|--------|--------|------|
| 1 | `empty_table_list_returns_zero` | 空表返回 0 | ✅ |
| 2 | `single_entry_appears_in_list` | 单条目：vector + val 正确 | ✅ |
| 3 | `two_entries_appear_in_list` | 两条目均可查到 | ✅ |
| 4 | `update_does_not_grow_list` | 幂等更新不增长计数 | ✅ |
| 5 | `entries_appear_in_table_order` | 输出按注册顺序（表顺序） | ✅ |
| 6 | `reset_clears_u8_list` | reset 后列表清空 | ✅ |
| 7 | `full_table_all_entries_appear_in_list` | 16 槽全满时全部可见 | ✅ |
| 8 | `overflow_returns_false_list_capped` | 第 17 次注册返回 false，列表保持 16 | ✅ |
| 9 | `boundary_vals_zero_and_max_appear` | val=0 和 val=255 均正确显示 | ✅ |
| 10 | `node_attr_list_u8_does_not_bump_epoch` | 不推进 graph_epoch | ✅ |

**全部通过 10/10**

---

## 不变量 / Invariants (never break)

- `node_attr_list_u8` 是纯读操作，不修改任何图状态，不推进 epoch
- VectorAddress 无 ::ZERO 常量：使用 `VectorAddress::new(0,0,0,0)`
- u8 表上限 MAX_NODE_PROPS_U8 = 16（注意：比 u32 表的 32 槽少一半）
- 路由顺序：`node attr list u8` 必须在 `node attr list` 之前匹配（前缀歧义）
- VectorAddress L4=36 保留给 gos-node-attr-list-u8-harness 测试节点

---

## 累计测试数 / Cumulative Test Count

- 本次新增：10 tests (gos-node-attr-list-u8-harness)
- 累计：**573 host tests** (563 + 10)
