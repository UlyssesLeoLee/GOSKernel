# HARDENING LOG — V2.59: graph density — E / (N*(N-1)) sparsity metric

**Date:** 2026-07-03  
**Version:** V2.59  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.59 为运行图添加 `graph_density`（图密度）指标——有向图的边数除以最大可能边数。
这是诊断图拓扑健康度的基础指标（类比 Linux `netstat -s` 的连接密度统计），
完成了从单指标（中心度/度数/最短路）向多维拓扑健康视图的演进。

V2.59 adds `graph_density` — the directed graph density E / (N*(N-1)), expressed
in parts-per-million for precision without floating-point. This is the foundational
sparsity metric for the live runtime graph, completing the set of "health at a glance"
metrics alongside `graph_eccentricity` (diameter/radius) and `graph_health`.

---

## 变更范围 / Change Scope

### 1. `crates/gos-runtime/src/lib.rs`

**新内部方法** `GraphRuntime::graph_density_inner() -> (u32, usize, usize)`:

```rust
// V2.59: Graph density = E / (N*(N-1)) for a directed graph.
pub fn graph_density_inner(&self) -> (u32, usize, usize) {
    let n = self.nodes.iter().filter(|s| s.is_some()).count();
    let e = self.edges.iter().filter(|s| s.is_some()).count();
    if n < 2 { return (0, n, e); }
    let max_edges = (n as u64) * (n as u64 - 1);
    let density_ppm = ((e as u64 * 1_000_000) / max_edges).min(1_000_000) as u32;
    (density_ppm, n, e)
}
```

**新公开函数** `graph_density() -> (density_ppm: u32, node_count: usize, edge_count: usize)`

### 2. `crates/k-shell/src/lib.rs`

**新函数** `dispatch_graph_density(sink)` — 输出格式：

```
 graph density
  density: 33.33%  (333333 ppm)
  nodes=4  edges=4  max=12
```

### 3. `crates/k-shell/src/proc.rs`

- **新路由** `cmd == "graph density"` / `"density"` / `"gdensity"`
- **帮助文本** 新增 `graph density` / `gdensity` 条目

### 4. `host-tests/gos-graph-density-harness/` (新建)

- `Cargo.toml` — `[workspace]` 隔离
- `.cargo/config.toml` — target = x86_64-pc-windows-msvc, build-std
- `tests/graph_density.rs` — 10 个测试全绿

---

## 算法设计 / Algorithm Design

```
graph_density_inner():
  n = count(nodes where slot is_some())   // active node count
  e = count(edges where slot is_some())   // active edge count
  if n < 2: return (0, n, e)             // undefined: single-node or empty
  max_edges = n * (n-1)                  // max directed edges (no self-loops)
  density_ppm = (e * 1_000_000) / max_edges  clamped to [0, 1_000_000]
  return (density_ppm, n, e)
```

**density_ppm 解读：**

| 值 | 含义 |
|---|------|
| `0` | 空图 / 单节点（定义未确定）或零连边 |
| `333_333` | ≈33.3%，典型的稀疏图（路径/树） |
| `500_000` | 50%，中等密度 |
| `1_000_000` | 100%，完全有向图 K_n（每对节点互联） |

### 为何选 ppm 而非浮点

- `no_std` 内核代码避免 soft-float 依赖
- u32 ppm 的精度为 0.0001%，足够诊断使用
- 与 `graph_closeness` / `graph_katz` 等现有 u32 指标一致

---

## 测试矩阵 / Test Matrix

| # | 测试名 | 验证点 | 结果 |
|---|--------|--------|------|
| 1 | `empty_graph_density_is_zero` | 空图 ppm=0, n=0, e=0 | ✅ |
| 2 | `single_node_density_undefined_returns_zero` | 单节点 ppm=0 | ✅ |
| 3 | `two_nodes_no_edges_density_zero` | 2节点无边 ppm=0 | ✅ |
| 4 | `two_nodes_one_edge_density_fifty_percent` | A→B: ppm=500_000 (50%) | ✅ |
| 5 | `two_nodes_two_edges_complete_graph_density_100` | K2: ppm=1_000_000 (100%) | ✅ |
| 6 | `four_nodes_four_edges_density_33pct` | 4节点4边: ppm=333_333 | ✅ |
| 7 | `complete_k4_density_100_pct` | K4完全图: ppm=1_000_000 | ✅ |
| 8 | `reset_clears_density` | reset 后 ppm=0 | ✅ |
| 9 | `three_node_path_density_33pct` | A→B→C: ppm=333_333 | ✅ |
| 10 | `graph_density_does_not_bump_epoch` | 不推进 graph_epoch | ✅ |

**全部通过 10/10**

---

## 不变量 / Invariants (never break)

- `graph_density` 是纯读操作，不修改任何图状态，不推进 epoch
- 当 N < 2 时返回 `(0, n, e)`（密度未定义，非错误）
- ppm 值上限 1_000_000（完全图），下限 0（无边）
- VectorAddress L4=35 保留给 gos-graph-density-harness 测试节点

---

## 累计测试数 / Cumulative Test Count

- 本次新增：10 tests (gos-graph-density-harness)
- 累计：**563 host tests** (553 + 10)
