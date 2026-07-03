# HARDENING LOG — V2.63: graph transitivity — raw triangle/triplet counts + gos-graph-transitivity-harness

**Date:** 2026-07-03  
**Version:** V2.63  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.63 新增 `graph_transitivity()` API，返回 (transitivity_ppm, triangle_count, triplet_count, node_count)。
与 V2.61 的 `graph_clustering()` 使用相同的全局比值公式（total_triangles / total_triplets ppm），
但额外暴露了原始三角形数和三元组数，可用于结构审计和与其他指标的组合计算。

V2.63 adds `graph_transitivity()` to gos_runtime. Both `graph_clustering` and `graph_transitivity`
compute the same global ratio (total_triangles / total_triplets expressed in ppm), but
`graph_transitivity` additionally returns the raw triangle and triplet counts — making it
useful for structural diagnostics and as a building block for derived metrics.

Shell: `graph transitivity` / `transitivity` / `gtrans` — shows ppm + raw counts.

---

## 变更范围 / Change Scope

### 1. `crates/gos-runtime/src/lib.rs`

新增 `graph_transitivity_inner` 方法（在 `graph_clustering_inner` 之后）：

```rust
pub fn graph_transitivity_inner(&self) -> (u32, u64, u64, usize)
// Returns: (transitivity_ppm, triangle_count, triplet_count, node_count)
```

公共 API：

```rust
pub fn graph_transitivity() -> (u32, u64, u64, usize) {
    RUNTIME.lock().graph_transitivity_inner()
}
```

算法与 `graph_clustering_inner` 相同（对无向投影的全局三角形/三元组比值），
差异在于返回原始计数而非仅返回 ppm。

### 2. `crates/k-shell/src/lib.rs`

新增 `dispatch_graph_transitivity`:
- 调用 `gos_runtime::graph_transitivity()`
- 显示 ppm 百分比 + 原始 triangles/triplets 计数

### 3. `crates/k-shell/src/proc.rs`

新增 shell 路由：

```
"graph transitivity" | "transitivity" | "gtrans"  →  dispatch_graph_transitivity
```

紧随 "graph clustering" / "clustering" / "gcluster" 之后。

### 4. `host-tests/gos-graph-transitivity-harness/` (新建)

- `Cargo.toml` — `[workspace]` 隔离
- `.cargo/config.toml` — target = x86_64-pc-windows-msvc
- `tests/graph_transitivity.rs` — 10 个测试全绿

---

## API 对比 / API Comparison

| 函数 | 返回值 | 用途 |
|------|--------|------|
| `graph_clustering()` | `(ppm, node_count)` | 快速全局聚类系数 |
| `graph_transitivity()` | `(ppm, triangles, triplets, n)` | 聚类系数 + 原始结构统计 |

两者使用相同公式：`ppm = total_triangles * 1_000_000 / total_triplets`。

---

## 测试矩阵 / Test Matrix (gos-graph-transitivity-harness)

VectorAddress 命名空间：L4=39

| # | 测试名 | 验证点 | 结果 |
|---|--------|--------|------|
| 1 | `empty_graph_transitivity_is_zero` | 空图 ppm=0, triangles=0, triplets=0 | ✅ |
| 2 | `single_node_transitivity_is_zero` | 单节点 ppm=0, triplets=0 | ✅ |
| 3 | `two_isolated_nodes_transitivity_is_zero` | 两孤立节点 triplets=0 | ✅ |
| 4 | `k2_has_no_triplets_and_zero_transitivity` | K₂ 无三元组 ppm=0 | ✅ |
| 5 | `open_path_abc_has_one_triplet_zero_triangles` | 路径 A-B-C: 1 triplet, 0 triangles | ✅ |
| 6 | `k3_triangle_has_full_transitivity` | K₃: triangles=3, triplets=3, ppm=1_000_000 | ✅ |
| 7 | `k4_complete_graph_has_full_transitivity` | K₄: triangles=12, triplets=12, ppm=1_000_000 | ✅ |
| 8 | `diamond_has_partial_transitivity` | 菱形: triangles=3, triplets=5, ppm=600_000 | ✅ |
| 9 | `transitivity_ppm_matches_clustering_and_exposes_raw_counts` | ppm与clustering相同+暴露原始计数 | ✅ |
| 10 | `transitivity_does_not_bump_epoch` | 纯读操作不推进 graph_epoch | ✅ |

**全部通过 10/10**

---

## 不变量 / Invariants

- `graph_transitivity_inner` 是纯读操作（同 `graph_clustering_inner`），不修改图状态
- 两者使用相同公式：ppm = total_triangles × 1_000_000 / total_triplets
- `graph_transitivity` 的附加值在于暴露原始计数（triangles, triplets）
- VectorAddress 命名空间：L4=39 保留给 gos-graph-transitivity-harness 测试节点

---

## 累计测试数 / Cumulative Test Count

- 本次新增：10 tests (gos-graph-transitivity-harness)
- 累计：**603 host tests** (593 + 10)
