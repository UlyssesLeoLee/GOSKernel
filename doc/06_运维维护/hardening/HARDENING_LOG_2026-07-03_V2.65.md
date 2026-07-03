# HARDENING LOG — V2.65: graph assortativity coefficient (Newman 2002) + gos-graph-assortativity-harness

**Date:** 2026-07-03  
**Version:** V2.65  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.65 新增 `graph_assortativity()` API，实现 Newman (2002) 度同配系数。
该指标衡量高度节点是否倾向于连接其他高度节点（同配/assortative, r>0），
还是倾向于连接低度节点（异配/disassortative, r<0）。
计算使用纯整数算术，无浮点运算，完全兼容 no_std 内核环境。

V2.65 adds `graph_assortativity()` to gos_runtime implementing the Newman (2002)
degree assortativity coefficient r ∈ [−1, +1]. Measures structural mixing patterns:
positive r indicates hub-to-hub bias (assortative, common in social networks);
negative r indicates hub-to-leaf bias (disassortative, common in infrastructure/internet).
The kernel's directed signal graph is expected to be mildly disassortative (high-degree
subsystem hubs connecting to lower-degree peripheral modules), making this a useful
structural health indicator.

Shell: `graph assortativity` / `assortativity` / `gassort`

---

## 背景 / Background

度同配系数（degree assortativity coefficient）是 M.E.J. Newman 2002 年提出的
网络混合模式分析工具：

- **r > 0（同配）**：高度节点倾向连接高度节点（如社交网络）
- **r < 0（异配）**：高度节点倾向连接低度节点（如互联网、生物网络、内核信号图）
- **r = 0（不相关）**：随机混合，或正则图（分母为零，返回 0）

在 GOS 内核图中：
- 核心子系统节点（高度）通过信号边连接大量外围模块（低度）→ 预期异配
- 若 r 趋近 −1，说明图呈极端星形拓扑（单中心架构）
- r 的变化趋势可作为拓扑演化的健康指标

---

## 算法 / Algorithm

**Newman 2002 整数公式（基于有向边集）：**

```
M  = 存储的有向边总数
对每条边 (u→v)：j = 无向度(u), k = 无向度(v)
    S1 += j·k
    T  += j+k
    Q  += j²+k²
Numerator   = 4·M·S1 − T²
Denominator = 2·M·Q  − T²
r_ppm = Numerator·1_000_000 / Denominator    (clamp 到 [−1e6, +1e6])
```

其中"无向度"= 节点的去重无向邻居数（与 graph_clustering / graph_transitivity 保持一致）。

**整数溢出分析（保证 i64 不溢出）：**
- M ≤ 512, 度 ≤ 128
- S1 ≤ 512 × 128² = 8,388,608
- 4·M·S1 ≤ 17,179,869,184 (< i64 max 9.2×10¹⁸) ✓
- T ≤ 512 × 256 = 131,072;  T² ≤ 17,179,869,184 ✓
- Q ≤ 512 × 2 × 128² = 16,777,216;  2·M·Q ≤ 17,179,869,184 ✓
- r_ppm·1_000_000 ≤ 17×10¹⁵ (< i64 max) ✓

**边界条件：**
- 无边 (M=0)：直接返回 (0, 0, n)
- 分母=0（正则图，所有节点同度）：返回 0（未定义 → 不相关）
- 自环：跳过

**三个精确验证点（单元测试覆盖）：**

| 图结构 | 计算过程 | r |
|--------|----------|---|
| 路径 A→B→C (deg: 1,2,1) | S1=4, T=6, Q=10, M=2; numer=−4, denom=4 | −1.0 |
| 星形 hub→B,C,D (deg: 3,1,1,1) | S1=9, T=12, Q=30, M=3; numer=−36, denom=36 | −1.0 |
| K3环 + K2对 (deg: 2,2,2,1,1) | S1=13, T=14, Q=26, M=4; numer=12, denom=12 | +1.0 |

---

## 变更范围 / Change Scope

### 1. `crates/gos-runtime/src/lib.rs`

新增 `graph_assortativity_inner` 方法（在 `graph_kcore_inner` 之后）：

```rust
pub fn graph_assortativity_inner(&self) -> (i32, usize, usize)
// Returns: (assortativity_ppm, edge_count, node_count)
//   +1_000_000 → 完全同配
//   −1_000_000 → 完全异配
//        0     → 不相关 / 未定义（正则图或无边）
```

公共 API：

```rust
pub fn graph_assortativity() -> (i32, usize, usize) {
    RUNTIME.lock().graph_assortativity_inner()
}
```

### 2. `crates/k-shell/src/lib.rs`

新增 `dispatch_graph_assortativity`:
- 调用 `gos_runtime::graph_assortativity()`
- 带符号百分比显示（`-` 前缀处理）
- 注释标签 "assortative" / "disassortative"
- 页脚显示 nodes= 和 edges=

### 3. `crates/k-shell/src/proc.rs`

新增 shell 路由（紧随 "graph kcore" 之后）：

```
"graph assortativity" | "assortativity" | "gassort"
→ dispatch_graph_assortativity
```

帮助文本新增：
```
graph assortativity  Newman degree assortativity r ∈ [−1,+1] — hubs-connect-to-hubs?
assortativity / gassort  aliases for graph assortativity
```

### 4. `host-tests/gos-graph-assortativity-harness/` (新建)

- `Cargo.toml` — `[workspace]` 隔离
- `.cargo/config.toml` — target = x86_64-pc-windows-msvc
- `tests/graph_assortativity.rs` — 10 个测试全绿

---

## 测试矩阵 / Test Matrix (gos-graph-assortativity-harness)

VectorAddress 命名空间：L4=41

| # | 测试名 | 验证点 | 结果 |
|---|--------|--------|------|
| 1 | `empty_graph_returns_zero` | 空图 ppm=0, edges=0, nodes=0 | ✅ |
| 2 | `nodes_no_edges_returns_zero` | 有节点无边 → ppm=0, edges=0 | ✅ |
| 3 | `single_edge_regular_undefined` | 单边 A→B (deg 1,1) → denom=0 → ppm=0 | ✅ |
| 4 | `path_graph_perfectly_disassortative` | 路径 A→B→C → ppm=−1_000_000 | ✅ |
| 5 | `triangle_cycle_regular_zero` | 有向三角环（全 deg=2）→ ppm=0 | ✅ |
| 6 | `star_graph_perfectly_disassortative` | 星形 hub→3叶 → ppm=−1_000_000 | ✅ |
| 7 | `two_disjoint_triangles_regular_zero` | 两独立三角环（全 deg=2）→ ppm=0 | ✅ |
| 8 | `disjoint_cliques_perfectly_assortative` | K3环 + K2对 → ppm=+1_000_000, edges=4, nodes=5 | ✅ |
| 9 | `edge_count_returned_correctly` | 星形轴+4叶 → edges=4 精确 | ✅ |
| 10 | `node_count_includes_isolated` | 三角形+2孤立节点 → nodes=5, edges=3, ppm=0 | ✅ |

**全部通过 10/10**

---

## VectorAddress L4 命名空间 / Namespace

```
29=node-attr,         32=pal-boot,            33=pal-render,
34=node-attr-list,    35=graph-density,        36=node-attr-list-u8,
37=graph-clustering,  38=pal-full,             39=graph-transitivity,
40=graph-kcore,       41=graph-assortativity
```

---

## 核心指标全景 / Core Metric Panorama

| 类别 | 指标 | 版本 |
|------|------|------|
| 连通性 | SCC 数量 | V2.34 |
| 中心性 | 度中心/PageRank | V2.38/V2.43 |
| 可达性 | 离心率/直径/半径 | V2.41 |
| 流量 | 最大流 (Edmonds-Karp) | V2.50 |
| 全局结构 | 图密度 | V2.59 |
| 局部结构 | 聚类系数 / 传递性 | V2.61/V2.63 |
| 核-外围结构 | k-核分解 / 退化度 | V2.64 |
| **混合模式** | **度同配系数** | **V2.65** |

---

## 不变量 / Invariants

- `graph_assortativity_inner` 是纯读操作，不修改图状态，不推进 epoch
- 无向度定义与 graph_clustering / graph_transitivity 完全一致（去重无向邻居）
- 返回 i32 PPM（有符号），范围 [−1_000_000, +1_000_000]
- 正则图（分母=0）：返回 0，不触发除零
- VectorAddress 命名空间：L4=41 保留给 gos-graph-assortativity-harness 测试节点

---

## 累计测试数 / Cumulative Test Count

- 本次新增：10 tests (gos-graph-assortativity-harness)
- 累计：**623 host tests** (613 + 10)
