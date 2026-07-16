# 硬化日志 V3.28 — Zagreb 补图指数 M̄₁ + M̄₂ + F̄（2026-07-08）

## 摘要

为 `gos_runtime` 新增三个 **Zagreb 补图指数（coindex）** 拓扑图不变量：
- **M̄₁(G)** —— 第一 Zagreb 补图指数（Ashrafi, Došlić & Hamzeh 2010）
- **M̄₂(G)** —— 第二 Zagreb 补图指数（Ashrafi, Došlić & Hamzeh 2010）
- **F̄(G)**  —— 遗忘补图指数（De 2016）

这些指数是 Zagreb 指数（V3.11）在*补图空间*中的对应物，对图的**非边（non-edge）**而非边求和。它们通过闭式恒等式解析计算——**无需扫描补图**，与所有基于度数的指数一样都是 O(V+E) 复杂度。

---

## 数学定义

对无向图 G = (V, E)，设 d_v 为 v 的度数：

| 指数 | 定义 | 公式 |
|-------|-----------|---------|
| M̄₁(G) | Σ_{uv∉E, u≠v} (d_u + d_v) | = 2m(n−1) − M₁ |
| M̄₂(G) | Σ_{uv∉E, u≠v} d_u · d_v  | = 2m² − M₁/2 − M₂ |
| F̄(G)  | Σ_{uv∉E, u≠v} (d_u²+d_v²) | = (n−1)·M₁ − F |

其中 M₁ = Σ_v d_v²，M₂ = Σ_{uv∈E} d_u·d_v，F = Σ_v d_v³，m = |E|，n = |V|。

### M₁ 恒为偶数的证明

M₁ = Σ d_v² ≡ #{奇数度顶点数} (mod 2)。根据握手引理，奇数度顶点的数量恒为偶数。因此 M₁ 恒为偶数，M₁/2 恒为非负整数。

### 关键不变量

- 当且仅当 G 为完全图（不存在非边）时，M̄₁ = M̄₂ = F̄ = 0。
- 恒有 M̄₁ ≥ 0，M̄₂ ≥ 0，F̄ ≥ 0（每一项均非负）。
- 比较 Zagreb 指数与 Zagreb 补图指数，可以揭示图的度数压力有多少分布在边上、多少分布在非边上。

---

## 交叉核对表

| 图       | M̄₁ | M̄₂ | F̄  | 边数 | 节点数 |
|-------------|-----|-----|-----|-------|-------|
| 空图       | 0   | 0   | 0   | 0     | 0     |
| 1 个节点      | 0   | 0   | 0   | 0     | 1     |
| 单边 A-B    | 0   | 0   | 0   | 1     | 2     |
| 路径 P₃     | 2   | 1   | 2   | 2     | 3     |
| 三角形 K₃ | 0   | 0   | 0   | 3     | 3     |
| 星图 K_{1,4}| 12  | 6   | 12  | 4     | 5     |
| 路径 P₄     | 8   | 5   | 12  | 3     | 4     |
| 完全图 K₄ | 0   | 0   | 0   | 6     | 4     |
| 两个孤立节点| 0   | 0   | 0   | 0     | 2     |
| K_{2,3}     | 18  | 21  | 42  | 6     | 5     |

---

## 操作系统类比

| 指数 | 操作系统解释 |
|-------|------------------|
| M̄₁   | "潜在通道压力"——所有缺失 IPC 链路的度数和之和；数值高表示许多潜在的高度数通道尚未建立连接 |
| M̄₂   | "枢纽-枢纽补图共负载"——缺失链路间的度数乘积；数值高表示高度数节点彼此之间**没有**直接连接（枢纽隔离） |
| F̄    | "平方度数补图压力"——M̄₁ 的放大版本，更强调枢纽节点；对全连通网格为零 |

在图操作系统场景中：M̄₁=M̄₂=F̄=0 是理想的全网格状态（无缺失的关键链路）。F̄/F 比值较高则说明该图相对于完全连通而言在结构上较为稀疏。

---

## 算法

O(V+E) 度数扫描——与 V3.11（Zagreb 指数）复杂度类别相同：
1. 构建紧凑节点索引与无向邻接位掩码。
2. 由位掩码计算度数数组：d_v = popcount(adj[v])。
3. 分两趟累加 M₁=Σd²、M₂=Σ_{边}d_u·d_v、F=Σd³。
4. 应用恒等式：
   - M̄₁ = 2m(n−1) − M₁
   - M̄₂ = 2m² − M₁/2 − M₂
   - F̄  = (n−1)·M₁ − F

**无需 BFS，也无需枚举补图。**

栈空间：adj[128]（u128=2KB）+ deg[128]（u64=1KB）≈ 共 3KB。

---

## Shell 接口

```
graph topo17        # 全称
gtopo17             # 简称别名
zagreb coindex      # 语义别名
gcoindex            # 简称语义别名
complement zagreb   # 补图视角命名
gcozagreb           # 简称补图视角
forgotten coindex   # 遗忘补图指数专用
gfbar               # F̄ 专用
gm1barm2barfbar     # 三者合一
```

---

## 变更文件

| 文件 | 变更内容 |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices17_inner()` + 公开函数 `graph_topo_indices17()` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices17()`，带彩色显示 |
| `crates/k-shell/src/proc.rs` | 新增 "graph topo17" 等命令的分发分支 |
| `host-tests/gos-graph-topo17-harness/` | 新建 harness：Cargo.toml、.cargo/config.toml、tests/graph_topo17.rs（10 项测试） |

---

## 测试覆盖

`gos-graph-topo17-harness` **新增 10 项测试**：
1. 空图 → (0,0,0,0,0)
2. 单节点 → (0,0,0,0,1)
3. 单边 A→B → (0,0,0,1,2) —— 完全 2 节点对上不存在非边
4. 路径 P₃ → (2,1,2,2,3) —— 存在一条非边 {A,C}
5. 三角形 K₃ → (0,0,0,3,3) —— 完全图，无非边
6. 星图 K_{1,4} → (12,6,12,4,5) —— 6 条叶子-叶子非边
7. 路径 P₄ → (8,5,12,3,4) —— 3 条非边，度数混合
8. 完全图 K₄ → (0,0,0,6,4) —— 无非边
9. 两个孤立节点 → (0,0,0,0,2) —— 零度非边贡献为 0
10. K_{2,3} 二部图 → (18,21,42,6,5) —— 恒等式交叉核对 ✓

**结果：10/10 通过**
**累计宿主测试套件：1253 个测试**（截至 V3.27 为 1243 个）

---

## VectorAddress 命名空间

`gos-graph-topo17-harness` 对应 L4=104

```
...102=graph-topo15, 103=graph-topo16, 104=graph-topo17
```

---

## 参考文献

- Ashrafi, A.R., Došlić, T. & Hamzeh, A. (2010). *The Zagreb coindices of graph operations.* Discrete Applied Mathematics, 158(15), 1571–1578.
- De, N. (2016). *The forgotten topological coindex.* AKCE International Journal of Graphs and Combinatorics.
- Gutman, I. & Trinajstić, N. (1972). *Graph theory and molecular orbitals.*（Zagreb 指数，供对照）

---

*本文件于 2026-07-15 按文档管理规范就地中文化，仅译语言，不改动版本号 / 文件路径 / 函数名 / 测试名 / 测试计数等既成事实。*
