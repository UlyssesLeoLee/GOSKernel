# GOSKernel Hardening Log — V2.74
**Date:** 2026-07-03  
**Branch:** feat/vk-auto-live-surface  
**Author:** Automated Hardening Pass

---

## 变更摘要 (Change Summary)

**V2.74 — 全局图效率 (Global Graph Efficiency)**

新增 `graph_efficiency` 指令：计算图的全局效率 E(G)——所有节点对逆距离的归一化均值。全局效率量化了图在信息传播方面的平均效率，是网络科学中衡量网络鲁棒性与连通性的核心指标之一。

---

## 数学定义 (Mathematical Definition)

对于有向图 G = (V, E)，n = |V|：

$$E(G) = \frac{1}{n(n-1)} \sum_{\substack{i \neq j \\ d(i,j) < \infty}} \frac{1}{d(i,j)}$$

其中 d(i,j) 为从节点 i 到节点 j 的最短路径长度（BFS，无权图）。不可达节点对贡献 0（而非无穷大），因此该指标对非连通图自然有效。

**与相关指标的对比：**

| 指标 | 公式 | 特点 |
|------|------|------|
| Wiener 指数 (V2.70) | Σ d(i,j) | 总路径代价，断图不计入 |
| 平均路径长度 | W / reachable_pairs | 仅统计可达对 |
| 调和中心性 HC[v] (V2.71) | Σ 1/d(v,u) | 单节点视角 |
| **全局效率 E(G)** | Σ 1/d(i,j) / (n*(n-1)) | 全图归一化，含断图 |

**边界情况：**

| 情形 | 结果 |
|------|------|
| 空图 (n=0) | E=0.0 (ppm=0) |
| 单节点 (n=1) | E=0.0, pairs_max=0 |
| 无边图 (n≥2) | E=0.0（无可达对） |
| 单向边 A→B | E=0.5 (ppm=500_000) |
| 完全双向图 K_n | E=1.0 (ppm=1_000_000) |
| 非连通图 | 0 < E < 1，仅统计可达对 |

**OS 类比：** 类似于网络诊断中的 `traceroute` 延迟倒数均值——E(G) 越高，图中各节点间的信息交换越高效，OS 的任务调度拓扑越优化。

---

## 新增 API (New API)

### gos-runtime

```rust
/// V2.74: 返回 (efficiency_ppm, pairs_max, node_count)
/// efficiency_ppm = E(G) × 1_000_000  (0..=1_000_000)
/// pairs_max      = n*(n-1)  (n < 2 时为 0)
/// node_count     = 存活节点数
pub fn graph_global_efficiency() -> (u64, usize, usize)
```

精度说明：ppm 精度为 1e-6，整数运算无浮点误差。

### k-shell

```
graph efficiency     — 计算并显示全局图效率 E(G)
graph eff            — graph efficiency 的简写
geff                 — 最短别名
global efficiency    — 全名别名
```

输出格式：绿色高亮 `E(G) = X.XXXXXX`（6位小数），页脚显示 `pairs_max=X nodes=X`。

---

## 算法 (Algorithm)

BFS 框架与 V2.70 (Wiener) 完全相同：

1. 枚举所有存活节点，n < 2 时提前返回 0
2. 每个节点 s 为源：执行 BFS，记录 dist[t] for all t
3. 对每个 t ≠ s：若 dist[t] < ∞ 且 dist[t] > 0，则 sum_recip += 1_000_000 / dist[t]
4. efficiency_ppm = sum_recip / (n*(n-1))

**复杂度：** O(V × (V+E))，与 Wiener/harmonic 相同。  
**精度：** 1_000_000/d 整数运算，截断误差 < 1 ppm。

---

## VectorAddress 命名空间

L4=50 用于本版本 host-test harness：`VectorAddress::new(50, 1, x, 0)`

---

## 新增文件 (New Files)

| 文件 | 说明 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | +`graph_global_efficiency_inner()` + `pub fn graph_global_efficiency()` |
| `crates/k-shell/src/lib.rs` | +`dispatch_graph_global_efficiency()` |
| `crates/k-shell/src/proc.rs` | 路由 "graph efficiency"/"graph eff"/"geff"/"global efficiency" + 帮助文本 |
| `host-tests/gos-graph-global-eff-harness/Cargo.toml` | 新 harness |
| `host-tests/gos-graph-global-eff-harness/.cargo/config.toml` | x86_64-pc-windows-msvc 目标覆盖 |
| `host-tests/gos-graph-global-eff-harness/tests/graph_global_efficiency.rs` | 10 个测试用例 |

---

## 测试用例 (Test Cases — 10/10 PASS)

| # | 图结构 | 期望 ppm | 期望结果 |
|---|--------|---------|----------|
| 1 | 空图 | 0 | node_count=0, pairs_max=0 |
| 2 | 单孤立节点 | 0 | node_count=1, pairs_max=0 |
| 3 | 2 节点无边 | 0 | pairs_max=2 |
| 4 | A→B（单向边） | 500_000 | E=0.5（仅一方向可达） |
| 5 | A↔B（双向） | 1_000_000 | E=1.0（完全连通 K2） |
| 6 | A→B→C（路径） | 416_666 | sum=2.5M/6 ≈ 0.4167 |
| 7 | A→B→C→A（有向3环） | 750_000 | sum=4.5M/6 = 0.75 |
| 8 | K3 全双向 | 1_000_000 | E=1.0（完全 K3） |
| 9 | {A→B}∥{C→D}（断图） | 166_666 | sum=2M/12 ≈ 0.167 |
| 10 | A→{B,C,D}（出星图） | 250_000 | sum=3M/12 = 0.25 |

---

## 不变量确认 (Invariant Checks)

- [x] render_frame in fbtest.rs 不锁 RUNTIME（本 PR 不修改 fbtest.rs）
- [x] VectorAddress 无 ::ZERO 常量（均使用 `VectorAddress::new(0,0,0,0)`）
- [x] graph_global_efficiency 为纯读操作，不 bump epoch
- [x] 无 non-ASCII hex escape（shell 字符串均使用 `\u{xxxx}` Unicode 转义）
- [x] L4=50 命名空间未被占用
- [x] 治理验证：`tools/verify-graph-architecture.ps1` PASS
- [x] 10/10 harness 测试通过

---

## 累计指标 (Cumulative Metrics)

- **Host-test total: 713 tests** (V2.74 增加 10 个，V2.73 前为 703)
- **核心图论指标集：**

| 类别 | 指标 | 版本 |
|------|------|------|
| 全局效率 | 全局图效率 E(G) = Σ 1/d(i,j)/(n*(n-1)) | **V2.74 ★** |
| 边界识别 | 外围节点 (ecc==diameter) | V2.72 |
| 中心识别 | 中心节点 (ecc==radius) | V2.73 |
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
