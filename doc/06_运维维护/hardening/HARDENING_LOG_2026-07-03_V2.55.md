# HARDENING LOG — V2.55: node attr — per-node u32 attribute storage

**Date:** 2026-07-03  
**Version:** V2.55  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

实现 `node attr` — 每节点 u32 属性存储原语。这是 PAL_U32 → 图节点重构的地基：
将硬编码调色板常量（`const PAL_U32: [u32; 4]`）迁移到图原生节点属性的第一步。

V2.55 adds per-node u32 attribute storage to the graph runtime — the foundational
primitive for the PAL_U32 → attribute-node refactor described in the V2 development
plan (Demo C prerequisite). Theme palette colors, flags, and arbitrary scalars can now
be attached to graph nodes and retrieved by vector address, without touching graph epoch
or structural state.

---

## 变更范围 / Change Scope

### 1. `crates/gos-runtime/src/lib.rs`

- **新常量** `MAX_NODE_PROPS_U32 = 32` — 与 `MAX_NODE_PROPS_U8 = 16` 平行的 u32 属性槽数量
- **新错误变体** `RuntimeError::PropTableFull` — u32 属性表满时返回
- **新字段** `GraphRuntime::node_props_u32: [(NodeId, u32); MAX_NODE_PROPS_U32]`
- **新内部方法** `GraphRuntime::register_node_prop_u32(node_id, u32) -> bool`
- **新内部方法** `GraphRuntime::node_prop_u32(node_id) -> Option<u32>`
- **新内部方法** `GraphRuntime::node_attr_set_inner(vec, u32) -> Result<(), RuntimeError>`
- **新内部方法** `GraphRuntime::node_attr_get_inner(vec) -> Option<u32>`
- **新公开函数** `node_attr_set(vec, u32) -> Result<(), RuntimeError>`
- **新公开函数** `node_attr_get(vec) -> Option<u32>`
- **新公开函数** `register_node_prop_u32(node_id, u32) -> bool` (boot-time direct use)

### 2. `crates/k-shell/src/lib.rs`

- **新函数** `dispatch_node_attr_set(sink, vec, val: u32)` — `node attr set` shell display
- **新函数** `dispatch_node_attr_get(sink, vec)` — `node attr get` shell display
- **新辅助函数** `print_hex32_inline(sink, u32)` — prints 8 hex digits (e.g. `00db1c21`)

### 3. `crates/k-shell/src/proc.rs`

- **新路由** `node attr set <vec> <hex>` / `nattr set <vec> <hex>`
- **新路由** `node attr get <vec>` / `nattr get <vec>` / `node attr <vec>`
- **帮助文本** 新增 3 行

### 4. `host-tests/gos-node-attr-harness/` (新建)

- `Cargo.toml` — `[workspace]` 隔离, 依赖 gos-runtime/gos-protocol/gos-cypher-mut/gos-supervisor
- `.cargo/config.toml` — target = x86_64-pc-windows-msvc, build-std
- `tests/node_attr.rs` — 10 个测试全绿

---

## 算法设计 / Algorithm Design

### `node_attr_set(vec, val)`

```
1. vec → NodeId (NodeNotFound if absent)
2. Scan node_props_u32 for existing NodeId entry → overwrite val (idempotent)
3. Else claim first slot where id == NodeId::ZERO → write (node_id, val)
4. If no free slot → Err(PropTableFull)
5. Return Ok(())
```

**关键不变量：**
- 不修改 graph_epoch（纯写属性表，不改拓扑）
- 不修改节点生命周期（attr 与节点状态正交）
- Idempotent：对同一 NodeId 重复 set 覆盖旧值，不扩展槽数

### `node_attr_get(vec)`

```
1. vec → NodeId (None if absent)
2. Scan node_props_u32 for matching NodeId → return Some(val)
3. No match → None
```

---

## Shell 接口 / Shell Interface

```
node attr set 6.1.1.0 00db1c21   # 设置节点 6.1.1.0 的 u32 属性为 0x00db1c21
node attr get 6.1.1.0             # 读取节点 6.1.1.0 的 u32 属性
nattr set 6.1.1.0 eddf2           # 简写 alias
nattr get 6.1.1.0                 # 简写 alias
```

输出格式：
```
 node attr set  6.1.1.0  =  0x00db1c21    # 成功设置（青色高亮）
 node attr  6.1.1.0  =  0x00db1c21        # 读取到值（绿色高亮）
 node attr  6.1.1.0  none                  # 未设置（灰色）
 node not found: 9.9.9.9                   # 节点不存在（红色）
```

---

## 测试矩阵 / Test Matrix

| # | 测试名 | 验证点 | 结果 |
|---|--------|--------|------|
| 1 | `empty_graph_attr_get_returns_none` | 空图未知 vec → None | ✅ |
| 2 | `empty_graph_attr_set_returns_err_node_not_found` | 空图 set → NodeNotFound | ✅ |
| 3 | `known_node_attr_get_before_set_returns_none` | set 前 get → None | ✅ |
| 4 | `attr_set_then_get_returns_value` | set 后 get → Some(val) | ✅ |
| 5 | `attr_set_overwrites_existing_value` | 二次 set 覆盖第一次值 | ✅ |
| 6 | `attr_set_max_u32_roundtrips` | 0xFFFF_FFFF 精确往返 | ✅ |
| 7 | `attr_set_zero_roundtrips` | 0x0000_0000 精确往返 | ✅ |
| 8 | `multiple_nodes_have_independent_attrs` | 多节点属性互不干扰 | ✅ |
| 9 | `attr_set_does_not_bump_epoch` | set 不推进 graph_epoch | ✅ |
| 10 | `attr_table_full_returns_prop_table_full` | 32 槽满 → PropTableFull | ✅ |

**全部通过 10/10**

---

## 与路线图的关联 / Roadmap Alignment

V2.55 是 V2.3 Demo C（"切 theme 0 行代码扩散"）的前置工程：

```
当前:  const PAL_U32: [u32; 4] = [0x00db_1c21, ...]; // fbtest.rs 硬编码
目标:  theme.wabi node → node_attr_set(wabi_vec, 0x00db1c21)
       render node 通过 node_attr_get(theme_current_use_target) 读色
```

V2.55 提供了 "图原生标量属性" 的存储层，下一步（V2.56+）可以：
1. 在 `builtin_bundle.rs` 引导时用 `register_node_prop_u32` 为 theme 节点写入调色板色值
2. `fbtest.rs` 中的渲染路径改为 `node_attr_get(theme_vec)` 读色
3. 最终消灭 `PAL_U32` 常量

---

## 不变量 / Invariants (never break)

- `node_attr_set` / `node_attr_get` 是纯属性操作，不产生 GraphDiffKind 条目
- `node_props_u32` 在 `reset()` 时随 `GraphRuntime::new()` 归零
- `PropTableFull` 是专属错误，不与 `NodeArenaFull` / `NodeTableFull` 混淆
- u32 属性槽按 NodeId 索引；同一节点多次 set 不增加槽消耗
- VectorAddress L4=29 保留给 gos-node-attr-harness 测试节点（避免与其他 harness 冲突）

---

## 累计测试数 / Cumulative Test Count

- 本次新增：10 tests (gos-node-attr-harness)
- 累计：**523 host tests** (513 + 10)
