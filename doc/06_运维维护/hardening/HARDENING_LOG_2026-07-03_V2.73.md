# GOSKernel Hardening Log — V2.73
**Date:** 2026-07-03  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated Hardening Pass

---

## 变更摘要 (Change Summary)

**V2.73 — 图中心节点 (Graph Center Nodes)**

新增 `graph_center` 指令：识别图中离心率等于半径的节点集合——即图的结构中心。中心节点是 V2.72 外围节点 (peripheral nodes, ecc==diameter) 的对称补集，代表从最坏情况角度出发，到所有可达节点距离最短的节点群。

---

## 数学定义 (Mathematical Definition)

对于有向图 G = (V, E)：

- 离心率 ecc[v] = max d(v, u)，对所有从 v 可达的 u ≠ v（若 v 为孤立节点则 ecc[v]=0）
- 半径 radius = min ecc[v]，对所有 ecc[v] > 0 的节点 v（若所有节点孤立则 radius=0）
- 中心节点集合 Center(G) = { v ∈ V : ecc[v] == radius，radius > 0 }

**与外围节点的关系：**
- 外围节点 (V2.72)：ecc[v] == diameter（距某节点最远）
- 中心节点 (V2.73)：ecc[v] == radius（到所有可达节点的最坏距离最小）
- 当 radius == diameter 时，所有节点同时既是中心节点又是外围节点（如完全双向图、均匀有向环）

**OS 类比：** 类似 NUMA 拓扑中的 `sched_setaffinity` 最优节点——内核服务调度时，中心节点保证到所有可达节点的最坏延迟最小。

---

## 新增 API (New API)

### gos-runtime

```rust
/// V2.73: 返回 (vecs, ecc, center_count, node_count, radius)
/// center_count 个中心节点，按 VectorAddress 升序排列
pub fn graph_center<const N: usize>() -> ([VectorAddress; N], [u32; N], usize, usize, u32)
```

- `vecs[0..center_count]`  — 中心节点向量地址（升序排列）
- `ecc[0..center_count]`   — 离心率（均等于 radius）
- `center_count`           — 中心节点数（上限 N）
- `node_count`             — 全部存活节点数
- `radius`                 — 最小非零离心率；若所有节点孤立则为 0

**边界情况：**
| 情形 | 结果 |
|------|------|
| 空图 | radius=0, center_count=0 |
| 单孤立节点 | radius=0, center_count=0 |
| 汇聚节点（sink，ecc=0）| 永不成为中心节点（0 < 任意正 radius）|
| radius==diameter | 所有节点同时为中心和外围 |

### k-shell

```
graph center          — 显示 ecc==radius 的中心节点集合
gcenter               — graph center 的缩写命令
```

输出格式：绿色显示中心节点 + ecc 值，页脚显示 `radius=X center=Y nodes=Z`。

---

## 算法 (Algorithm)

与 V2.72 (graph_peripheral_inner) 完全相同的 BFS 框架：
1. 枚举所有存活节点
2. 每个节点为源，执行 BFS 计算离心率
3. radius = min(非零 ecc)（u32::MAX 哨兵处理全孤立情形）
4. 筛选 ecc[v] == radius 的节点，按 VectorAddress as_u64() 插入排序
5. 打包输出数组（上限 N=128）

**复杂度：** O(V × (V+E))，与 peripheral 相同。

---

## VectorAddress 命名空间

L4=49 用于本版本 host-test harness：`VectorAddress::new(49, 1, x, 0)`

---

## 新增文件 (New Files)

| 文件 | 说明 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | +`graph_center_inner<N>()` + `pub fn graph_center<N>()` |
| `crates/k-shell/src/lib.rs` | +`dispatch_graph_center()` |
| `crates/k-shell/src/proc.rs` | 路由 "graph center"/"gcenter" + 帮助文本 |
| `host-tests/gos-graph-center-harness/Cargo.toml` | 新 harness |
| `host-tests/gos-graph-center-harness/.cargo/config.toml` | x86_64-pc-windows-msvc 目标覆盖 |
| `host-tests/gos-graph-center-harness/tests/graph_center.rs` | 10 个测试用例 |

---

## 测试用例 (Test Cases — 10/10 PASS)

| # | 图结构 | 期望结果 |
|---|--------|----------|
| 1 | 空图 | center_count=0, radius=0 |
| 2 | 单孤立节点 | center_count=0, radius=0 |
| 3 | A→B（两节点链） | 仅 A 为中心（ecc=1=radius） |
| 4 | A→B→C（路径） | 仅 B 为中心（ecc=1=radius），A 和 C 排除 |
| 5 | A→B→C→A（有向3环） | A、B、C 全为中心（radius==diameter==2） |
| 6 | A→{B,C,D}（出星图） | 仅 A 为中心（ecc=1=radius） |
| 7 | A→B→C→D→E（5节点链） | 仅 D 为中心（ecc=1=radius，倒数第二节点） |
| 8 | {A→B} ∥ {C→D→E}（断图） | A 和 D 为中心（均 ecc=1=radius） |
| 9 | K3 全双向 A↔B↔C，A↔C | A、B、C 全为中心（radius==diameter==1） |
| 10 | A→B→C→A + 孤立 D | A、B、C 为中心，D 排除（ecc=0 < radius=2） |

---

## 不变量确认 (Invariant Checks)

- [x] render_frame in fbtest.rs 不锁 RUNTIME（本 PR 不修改 fbtest.rs）
- [x] VectorAddress 无 ::ZERO 常量（均使用 `VectorAddress::new(0,0,0,0)`）
- [x] graph_center 为纯读操作，不 bump epoch
- [x] 无 non-ASCII hex escape（shell 字符串均使用 `\u{xxxx}` Unicode 转义）
- [x] L4=49 命名空间未被占用
- [x] 治理验证：`tools/verify-graph-architecture.ps1` PASS

---

## 累计指标 (Cumulative Metrics)

- **Host-test total: 703 tests** (V2.73 增加 10 个，V2.72 前为 693)
- **核心图论指标集：**

| 类别 | 指标 | 版本 |
|------|------|------|
| 边界识别 | 外围节点 (ecc==diameter) | V2.72 |
| 中心识别 | 中心节点 (ecc==radius) | **V2.73 ★** |
| 调和可达性 | 调和中心性 HC[v]=Σ 1/d(v,u) | V2.71 |
| 路径总代价 | Wiener 指数 | V2.70 |
| 环结构 | 围 (最短有向环长) | V2.69 |
| 精英连接 | Rich-club 系数 ρ(k) | V2.68 |
| 社区质量 | 模块度 (Newman-Girvan Q) | V2.67 |
| 方向对称性 | 互反性 | V2.66 |
| 混合模式 | 度同配系数 (Newman) | V2.65 |
| 核心-外围 | k-core 分解 | V2.64 |
| 局部结构 | 聚类系数/传递性 | V2.61/V2.63 |
| 全局结构 | 图密度 | V2.59 |

---

*自动化硬化任务 · 每 2 小时执行一次 · 保持图论操作系统的产品级水准*
