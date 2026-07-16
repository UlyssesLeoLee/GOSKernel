# 硬化日志 V3.11 — Zagreb 指数 M1/M2 + Randić R + Albertson I

**日期**：2026-07-06
**分支**：feat/vk-auto-live-surface
**上一基线**：V3.10（图熵 H(G)，1073 个宿主测试）
**新总计**：1083 个宿主测试（+10）

---

## 算法：Zagreb / Randić / Albertson 拓扑指数

V3.11 在单趟 O(V+E) 扫描中新增四个经典的基于度数的拓扑指数：

### 第一 Zagreb 指数 M₁（Gutman & Trinajstić 1972）

> M₁(G) = Σ_v deg(v)²

度数平方和。等价地，M₁ = Σ_{uv∈E} (deg(u) + deg(v))——两种表述给出相同结果。M₁ 刻画了图的"度数异质性压力"；对正则图，M₁ = n × d²，其中 d 为统一度数。

### 第二 Zagreb 指数 M₂（Gutman & Trinajstić 1972）

> M₂(G) = Σ_{uv∈E} deg(u) × deg(v)

无向边上度数乘积之和。M₂ 衡量枢纽间的相互依赖：M₂ 越高，说明高度数节点越倾向于直接相连。

### Randić 连通性指数 R（Randić 1975）

> R(G) = Σ_{uv∈E} 1/√(deg(u) × deg(v))

化学图论中研究最广泛的拓扑描述符之一，由 Randić 作为分子图的分支指数提出。通过牛顿-拉夫逊 isqrt 计算：每条边贡献 = floor(10¹²/isqrt_ppm(deg(u)×deg(v)))，误差 ≤ 1 ppm。

### Albertson 不规则指数 I（Albertson 1997）

> I(G) = Σ_{uv∈E} |deg(u) − deg(v)|

衡量各边度数不平衡的总和。当且仅当图为正则图时 I = 0。提供了一种简单、计算成本低的不规则性度量。

## 实现

- `gos_runtime::graph_zagreb()` → `(m1: u64, m2: u64, randic_ppm: u32, irregularity: u32, edge_count: usize, node_count: usize)`
- 对无向邻接位掩码单趟扫描（a < b 规范化以避免重复计数）
- 从有向边列表构建无向 `adj[]` 与 `deg[]` 数组
- M₁ 在独立的 O(V) 节点扫描中计算；M₂/R/I 在 O(E) 边扫描中计算
- isqrt_ppm(p) = 牛顿-拉夫逊 floor(√p × 10⁶)——与谱分析模块共用

## Shell 命令

`graph zagreb` · `gzagreb` · `zagreb` · `zagreb index` · `graph topo index` · `randic` · `graph randic`

## 测试 Harness

**gos-graph-zagreb-harness** —— 10 项测试，VectorAddress L4=87：

| # | 图 | M1 | M2 | R_ppm | I |
|---|-------|----|----|-------|---|
| 1 | 空图 | 0 | 0 | 0 | 0 |
| 2 | 单节点 | 0 | 0 | 0 | 0 |
| 3 | 单边 A→B | 2 | 1 | 1_000_000 | 0 |
| 4 | 路径 P₃ | 6 | 4 | 1_414_214 | 2 |
| 5 | 三角形 K₃ | 12 | 12 | 1_500_000 | 0 |
| 6 | 星图 K_{1,4} | 20 | 16 | 2_000_000 | 12 |
| 7 | 路径 P₄ | 10 | 8 | 1_914_214 | 2 |
| 8 | 完全图 K₄ | 36 | 54 | 1_999_998 | 0 |
| 9 | 两个孤立节点 | 0 | 0 | 0 | 0 |
| 10 | K_{2,3} | 30 | 36 | 2_449_488 | 6 |

## 操作系统类比

- **M₁**：度数平方耦合压力——所有内核子系统依赖扇出的平方和。
- **M₂**：枢纽间相互依赖——高扇出模块彼此直接耦合的紧密程度。
- **R**：Randić 连通性指数——内核依赖图的分支度量；R 低 = 星形拓扑（枢纽-辐射型 IPC），R 高 = 正则网格。
- **I**：IPC 通道负载不平衡——各边度数差异的总和；I = 0 意味着所有子系统耦合程度相等，是均衡调度的理想状态。

## 参考文献

- Gutman, I. & Trinajstić, N. (1972). Graph theory and molecular orbitals. *Chemical Physics Letters*, 17(4), 535–538.
- Randić, M. (1975). Characterization of molecular branching. *JACS*, 97(23), 6609–6615.
- Albertson, M.O. (1997). The irregularity of a graph. *Ars Combinatoria*, 46, 219–225.

---

*本文件于 2026-07-15 按文档管理规范就地中文化，仅译语言，不改动版本号 / 文件路径 / 函数名 / 测试名 / 测试计数等既成事实。*
