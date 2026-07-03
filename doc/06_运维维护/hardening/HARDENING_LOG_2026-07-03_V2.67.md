# HARDENING LOG — V2.67: graph modularity + gos-graph-modularity-harness

**Date:** 2026-07-03  
**Version:** V2.67  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated hardening run (scheduled task)

---

## 摘要 / Summary

V2.67 新增 `graph_modularity()` API，计算 Newman–Girvan 模块度 Q，
衡量 LPA 社区划分的质量。模块度越高，说明社区内部连接越密集、社区间连接越稀疏，
是评估社区划分质量的标准黄金指标。结果以 ppm 表示（0=单一社区，500_000=两等分离子团基准）。

V2.67 adds `graph_modularity()` to gos_runtime, computing the Newman–Girvan modularity Q
over the LPA community partition (the same partition returned by `graph_community`).
Modularity measures how well-separated the communities are: high Q means dense intra-community
edges and sparse inter-community edges. Directed edges are treated as undirected (consistent
with LPA). Result is in parts-per-million: 0 = single community or no edges, 500_000 = two
equal disconnected cliques (theoretical benchmark).

Shell: `graph modularity` / `modularity` / `gmodq`

---

## 背景 / Background

Newman–Girvan 模块度是社区检测质量的标准评估指标：

- **Q = Σ_c [ L_c/m − (d_c/(2m))² ]**
  - m：无向边数（有向对计为一条）
  - L_c：社区 c 内的无向边数
  - d_c：社区 c 内所有节点的度之和
- **Q = 0**：单一社区，或所有节点在同一社区中
- **Q = 0.5**：两个等大小孤立完全图——社区划分的理论基准
- **Q → 1**：理想划分（自然图中难以达到）

在 GOS 内核图中，模块度揭示内核子系统的聚集质量：
- Q=0：所有子系统紧密互联（单一超级社区，无明显功能分组）
- Q>0.3：内核存在明显的功能分组（如网络层、调度层、图运行时各自成簇）

模块度是 V2.45 `graph_community` 的配套指标：
社区检测给出划分，模块度量化该划分的优劣。

---

## 实现细节 / Implementation

### 核心算法

```
1. 运行 LPA（与 graph_community 完全相同的 20 轮异步算法）
2. 去重：将有向边对 (u,v)/(v,u) 折叠为一条无向边；计算 m
3. 计算每个节点的无向度 deg[v]
4. 对每条无向边检查两端是否同属一社区 → 累计 ΣL_c
5. 对每个社区计算 d_c = Σ deg[v]，再累计 Σd_c²
6. Q_ppm = (4m·ΣL_c − Σd_c²) × 1_000_000 / (4m²)
```

纯整数运算，无浮点，no_std 安全。溢出分析：
- m ≤ MAX_EDGES = 512
- Σd_c² ≤ (2m)² = 4·512² = 1_048_576
- (4m·ΣL_c) ≤ 4·512² = 1_048_576
- 最大分子：1_048_576 × 1_000_000 ≈ 10¹² ≪ i64::MAX（~9.2×10¹⁸）

### 返回值 / Return values

```rust
pub fn graph_modularity() -> (i32, usize, usize, usize)
// (modularity_ppm, community_count, undirected_edge_count, node_count)
```

- `modularity_ppm`：Q × 1_000_000，i32（负值理论可能但 LPA 输出不会出现）
- `community_count`：LPA 检测到的社区数量
- `undirected_edge_count`：去重后的无向边数（m）
- `node_count`：活跃节点数

### 边界情况 / Edge cases

| 场景 | 返回值 |
|------|--------|
| 空图 | (0, 0, 0, 0) |
| 无边（孤立节点） | (0, n, 0, n) |
| 单条边 | (0, 1, 1, 2) |
| 连通图 | (0, 1, m, n) |
| 互惠对 A↔B + 孤立 C | (0, 2, 1, 3) |
| 两等大孤立完全图 | (500_000, 2, ·, ·) |

### 无向边去重 / Undirected edge deduplication

有向边 (A→B) 与 (B→A) 代表同一条无向边，只计一次：
```
for each directed edge (u, v):
    if (v, u) already in seen → skip
    else → record (u, v), m += 1
```
这保证了与 LPA 使用相同的无向投影（directed edges as undirected）。

---

## VectorAddress 命名空间扩展

VectorAddress L4=43 分配给 gos-graph-modularity-harness（测试隔离用）

完整 L4 命名空间：
```
29=node-attr, 32=pal-boot, 33=pal-render, 34=node-attr-list, 35=graph-density,
36=node-attr-list-u8, 37=graph-clustering, 38=pal-full, 39=graph-transitivity,
40=graph-kcore, 41=graph-assortativity, 42=graph-reciprocity, 43=graph-modularity
```

---

## 修改文件 / Changed files

| 文件 | 修改内容 |
|------|----------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_modularity_inner()` 静态方法 + 公共 `graph_modularity()` 函数 |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_modularity()` shell 显示函数 |
| `crates/k-shell/src/proc.rs` | 新增路由：`graph modularity` / `modularity` / `gmodq` + help 文本 |
| `host-tests/gos-graph-modularity-harness/Cargo.toml` | 新建测试 harness |
| `host-tests/gos-graph-modularity-harness/.cargo/config.toml` | host 目标覆盖 |
| `host-tests/gos-graph-modularity-harness/tests/graph_modularity.rs` | 10 个测试 |

---

## 测试矩阵 / Test matrix (10 tests, all passing)

| # | 场景 | Q_ppm | comms | edges | nodes |
|---|------|-------|-------|-------|-------|
| 1 | 空图 | 0 | 0 | 0 | 0 |
| 2 | 3 个孤立节点（无边） | 0 | 3 | 0 | 3 |
| 3 | 单条边 A→B | 0 | 1 | 1 | 2 |
| 4 | 有向三角形 A→B→C→A | 0 | 1 | 3 | 3 |
| 5 | K4 完全图 | 0 | 1 | 6 | 4 |
| 6 | 两对孤立点 {A-B} {C-D} | 500_000 | 2 | 2 | 4 |
| 7 | 两个 K3 孤立完全图 | 500_000 | 2 | 6 | 6 |
| 8 | K3 + K2 孤立 | 375_000 | 2 | 4 | 5 |
| 9 | 互惠对 A↔B + 孤立节点 C | 0 | 2 | 1 | 3 |
| 10 | 星形 hub→B/C/D | 0 | 1 | 3 | 4 |

**关键测试用例数学验证：**
- 测试 6（两孤立对）：Q = (4·2·2−8)·10⁶/(4·4) = 8·10⁶/16 = **500_000** ✓
- 测试 7（两 K3）：Q = (4·6·6−72)·10⁶/(4·36) = 72·10⁶/144 = **500_000** ✓
- 测试 8（K3+K2）：Q = (4·4·4−40)·10⁶/(4·16) = 24·10⁶/64 = **375_000** ✓
- 测试 9（互惠对+孤立）：Q = (4·1·1−4)·10⁶/(4·1) = **0** ✓（C 的度=0 不影响模块度）

---

## 核心指标全貌 / Complete metric set

| 类别 | 指标 | 版本 |
|------|------|------|
| 连通性 | SCC 数 | V2.34 |
| 中心性 | Degree / PageRank | V2.38 / V2.43 |
| 可达性 | Eccentricity / diam / rad | V2.41 |
| 流量 | 最大流 (Edmonds-Karp) | V2.50 |
| 全局结构 | 图密度 | V2.59 |
| 局部结构 | 聚类系数 / 传递性 | V2.61 / V2.63 |
| 核-外围 | k-core 分解 / 退化度 | V2.64 |
| 混合模式 | 度同配系数 (Newman) | V2.65 |
| 方向对称 | 互惠性 (reciprocity) | V2.66 |
| 社区质量 | 模块度 (Newman–Girvan Q) | V2.67 |
