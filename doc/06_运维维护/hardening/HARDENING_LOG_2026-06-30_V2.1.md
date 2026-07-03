# GOS 自动硬化日志 — 2026-06-30（第2次，V2.1）

> 类型：定期自动硬化（每2小时）  
> 目标：推进 V2.1 "Cypher = ISA" 阶段——把 `MutationDispatcher` 接到真实 runtime  
> 提交：`4ddbb97` `feat(v2.1): wire MutationDispatcher to GraphRuntime — Cypher write path live`

---

## 执行摘要

本次硬化实现了 V2.1 的核心交付：**Cypher 写路径接通真实 runtime**。

`gos_cypher_mut::MutationDispatcher` trait 现已为 `GraphRuntime` 实现，并通过全局函数
`gos_runtime::apply_cypher_mutation()` 对外暴露。新增的 golden 测试覆盖了完整的 V2.1
退出判据（AddEdge → 可见，RemoveEdge → 消失，RebindUse → 原子替换）。

全部 25 项 runtime harness 测试（含1项新增）和 16 项 supervisor harness 测试通过。
工作区 clippy 零警告。

---

## 变更详情

### 1. `crates/gos-runtime/Cargo.toml`

添加 `gos-cypher-mut` 为 runtime 依赖（无循环依赖：`gos-cypher-mut` 仅依赖 `gos-protocol`）。

### 2. `crates/gos-runtime/src/lib.rs`（+79 行）

#### `impl gos_cypher_mut::MutationDispatcher for GraphRuntime`

| 方法 | 实现策略 |
|---|---|
| `lookup_node(id)` | `node_slot_by_id(id).is_some()` — O(MAX_NODES) 线性扫描，128 个槽 |
| `add_edge(from, to, kind)` | `derive_edge_id(from, to, "cypher.Mount/Use")` 生成确定性 EdgeId，构造 EdgeSpec 调 `register_edge` |
| `remove_edge(id)` | 直接委托 `unregister_edge` |
| `rebind_use(from, new_target)` | 扫描 edges 表找到 `from_node==from && edge_type==Use` 的旧边，先 `unregister_edge`，再注册新 Use 边到 `new_target` |

**EdgeId 生成**：使用 `gos_protocol::derive_edge_id(from, to, key)` 确定性哈希，同一对端点+类型总能重新计算出相同 EdgeId，方便调用方按需查找。

#### `pub fn apply_cypher_mutation()`

全局函数，锁住 RUNTIME → 调 `gos_cypher_mut::apply_mutation`。调用方（Cypher shell、AI bridge、控制平面适配器）无需暴露 Mutex 类型或 GraphRuntime 内部结构。

### 3. `host-tests/gos-runtime-harness/tests/runtime.rs`（+100 行）

新增测试 `v2_1_cypher_mutation_writes_through_to_runtime_edge_table`：

```
AddEdge Mount (src→dst)
  → edge_page 返回 1 条边，edge_id 匹配
RemoveEdge (expected_mount_id)
  → edge_page 返回 0 条边，edge_vector_for_id 返回 None
AddEdge Use (src→dst) 后 RebindUse (src→alt)
  → 旧 Use 边 (src→dst) 消失
  → 新 Use 边 (src→alt) 出现
  → edge_page 返回恰好 1 条边（新 Use）
```

---

## 与 V2.x 路线图的关联

| 阶段 | 状态 | 本次推进 |
|---|---|---|
| V2.0 边代数地基 | ✅ | — |
| **V2.1 Cypher = ISA** | 🔄 **大幅推进** | MutationDispatcher 实装；golden test 覆盖退出判据 |
| V2.2 Rewrite Engine | 🔲 | — |
| V2.3 Subscribe & 渲染 | 🔲 | — |

### V2.1 剩余工作

- ADR-004：epoch-published vs immediate 可见性语义（推荐 epoch-published；影响 V2.3 renderer 读图方式）
- `graph_epoch` 已存在于 GraphRuntime——可直接作为 `snapshot_at` 键；需要文档化
- fault attribution 路径：`apply_cypher_mutation` 当前返回 `MutationError`，可接入 journal 审计（`AuditedMutation` 结构已存在于 `gos-cypher-mut`）

---

## 测试结果

```
host-tests/gos-runtime-harness:   25 passed, 0 failed  (+1 新增)
host-tests/gos-supervisor-harness: 16 passed, 0 failed
cargo clippy --workspace:          0 warnings
cargo check --workspace:           Finished (0 errors)
```

---

*自动生成于 2026-06-30 定期硬化任务（第2次）*
