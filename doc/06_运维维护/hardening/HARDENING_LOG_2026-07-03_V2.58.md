# HARDENING LOG — V2.58: node attr list — diagnostic enumeration of u32 attributes

**Date:** 2026-07-03  
**Version:** V2.58  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.58 实现 `node attr list` / `nattr list` — 完成节点 u32 属性的 CRUD 操作集：
V2.55 建立了 Set/Get 原语，V2.58 补全 List（枚举全部已设属性的节点），
形成可供 k-shell 诊断和 AI 工具调用的完整属性审计接口。

V2.58 adds `node_attr_list` — a diagnostic function that enumerates all nodes
with a u32 attribute set, returning (VectorAddress, u32) pairs in table order.
This completes the node-attr CRUD set (Set/Get from V2.55, List from V2.58)
and adds a `node attr list` / `nattr list` shell command for live auditing of
palette colors, flags, and counters across the running graph.

---

## 变更范围 / Change Scope

### 1. `crates/gos-runtime/src/lib.rs`

**新内部方法** `GraphRuntime::node_attr_list_inner<const N>(out_vec, out_val) -> usize`:

```rust
// V2.58: List all nodes that have a u32 attribute set.
// Fills out_vec/out_val in table order, skipping free (ZERO) slots.
pub fn node_attr_list_inner<const N: usize>(
    &self,
    out_vec: &mut [VectorAddress; N],
    out_val: &mut [u32; N],
) -> usize
```

**新公开函数** `node_attr_list<const N>(out_vec, out_val) -> usize`:

```rust
pub fn node_attr_list<const N: usize>(
    out_vec: &mut [VectorAddress; N],
    out_val: &mut [u32; N],
) -> usize {
    RUNTIME.lock().node_attr_list_inner(out_vec, out_val)
}
```

实现：线性扫描 `node_props_u32` 表，跳过 NodeId::ZERO（空槽），
对每个非空槽调用 `node_vector(node_id)` 解析为 VectorAddress。

### 2. `crates/k-shell/src/lib.rs`

**新函数** `dispatch_node_attr_list(sink)` — 输出诊断表格：

```
 node attr list
  6.1.1.0  0x00db1c21
  6.1.2.0  0x00edeef2
  2 / 32 slots used
```

### 3. `crates/k-shell/src/proc.rs`

- **新路由** `cmd == "node attr list"` / `cmd == "nattr list"`
- **帮助文本** 新增 `node attr list` 条目，更新 nattr 别名说明

### 4. `host-tests/gos-node-attr-list-harness/` (新建)

- `Cargo.toml` — `[workspace]` 隔离
- `.cargo/config.toml` — target = x86_64-pc-windows-msvc, build-std
- `tests/node_attr_list.rs` — 10 个测试全绿

---

## 算法设计 / Algorithm Design

```
node_attr_list_inner<N>():
  count = 0
  for (node_id, val) in node_props_u32:
    if node_id == NodeId::ZERO: skip   (free slot)
    if count >= N: break               (caller capacity)
    out_vec[count] = node_vector(node_id)  // NodeId → VectorAddress
    out_val[count] = val
    count += 1
  return count
```

**关键设计决策：**
- 结果顺序 = 属性表的插入顺序（线性 FIFO，可重现）
- N=0 安全：直接返回 0，不写入任何内存
- `node_vector` 解析失败时 fallback 到 `VectorAddress::new(0,0,0,0)`（仅在 NodeId 表损坏时触发，正常运行不可能）
- 不分配内存，不修改状态，不推进 epoch

---

## Shell 接口 / Shell Interface

```
node attr list         # 列出所有有 u32 属性的节点
nattr list             # 简写别名
```

输出格式：
```
 node attr list
  6.1.1.0  0x00db1c21     ← theme.wabi  → RED
  6.1.2.0  0x00edeef2     ← theme.shoji → WHITE
  2 / 32 slots used
```

---

## 测试矩阵 / Test Matrix

| # | 测试名 | 验证点 | 结果 |
|---|--------|--------|------|
| 1 | `empty_graph_attr_list_returns_zero` | 空图返回 0 | ✅ |
| 2 | `one_node_with_attr_appears_in_list` | 1 节点，正确 vec + val | ✅ |
| 3 | `two_nodes_both_appear_in_list` | 2 节点均出现 | ✅ |
| 4 | `node_without_attr_not_in_list` | 无 attr 节点不出现 | ✅ |
| 5 | `list_preserves_insertion_order` | 结果顺序 = 插入顺序 | ✅ |
| 6 | `zero_capacity_returns_zero_entries` | N=0 返回 0 | ✅ |
| 7 | `small_n_caps_returned_entries` | N<count 只返回 N 条 | ✅ |
| 8 | `list_shows_overwritten_attr_value` | 覆盖后显示新值 | ✅ |
| 9 | `reset_clears_list` | reset 后返回 0 | ✅ |
| 10 | `full_table_all_entries_appear_in_list` | 32 槽满全部出现 | ✅ |

**全部通过 10/10**

---

## 不变量 / Invariants (never break)

- `node_attr_list` 是纯读操作，不修改 node_props_u32 或 graph_epoch
- N=0 是合法调用（空 out 数组），返回 0，无内存写入
- 枚举顺序由 node_props_u32 表的物理槽位决定（FIFO 插入顺序）
- VectorAddress L4=34 保留给 gos-node-attr-list-harness 测试节点

---

## 累计测试数 / Cumulative Test Count

- 本次新增：10 tests (gos-node-attr-list-harness)
- 累计：**553 host tests** (543 + 10)
