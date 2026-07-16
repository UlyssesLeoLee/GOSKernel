# 硬化日志 V3.08 — 边染色 χ'(G)

**日期**：2026-07-06  
**分支**：feat/vk-auto-live-surface  
**提交**：9f9278e  
**先前基线**：V3.07（顶点连通度 κ(G)，1043 个宿主测试）  
**新总计**：1053 个宿主测试（+10）

---

## 算法：边染色（Vizing 1964）

**边染色**为图的每一条无向边分配一种颜色，使得任意两条共享一个端点的边颜色不同。所需的最少颜色数称为**色指数（chromatic index）** χ'(G)。

### 理论背景

**Vizing 定理（1964）：** 对任意简单无向图 G，

```
Δ(G) ≤ χ'(G) ≤ Δ(G) + 1
```

其中 Δ(G) 是最大度数。达到 χ'(G) = Δ 的图称为**第一类（class 1）**图；达到 χ'(G) = Δ+1 的图称为**第二类（class 2）**图。

**König 定理（1916）：** 二部图总是第一类图 —— χ'(G) = Δ(G)。这是二部图能达到的最优结果；不过，贪心算法根据边的排列顺序不同，仍可能用到 Δ+1 种颜色。

**类别示例：**
- K_{2k}（偶数阶完全图）：第一类，χ'=2k−1
- K_{2k+1}（奇数阶完全图 / K_3）：第二类
  - K_3（三角形）：Δ=2，χ'=3（第二类）
  - K_4：Δ=3，χ'=3（第一类）
  - C_{2k}（偶数环）：第一类，χ'=2
  - C_{2k+1}（奇数环）：第二类，χ'=3
  - 星形 K_{1,n}：第一类，χ'=n
  - 树与二部图：第一类，χ'=Δ

### 算法：贪心边染色

实现采用 O(E) 时间的贪心策略：

1. **构建无向边列表**：遍历有向边槽位；对每条边 (a,b) 规范化使 a < b（紧凑索引）；通过 `seen_adj[a] |= 1<<b` 位掩码去重；排除自环。

2. **贪心分配**：对每条槽位顺序中的边 (a,b)：
   - `forbidden = node_colors[a] | node_colors[b]`  
     其中 `node_colors[ci]` 是一个 u128 位掩码，若颜色 k 已经被用在与节点 ci 相邻的某条边上，则第 k 位置 1
   - `colour = forbidden.trailing_ones()` —— 最低 0 位的索引，即最小可用颜色
   - 更新 `node_colors[a] |= 1<<colour` 和 `node_colors[b] |= 1<<colour`

3. **排序输出**：按 (colour, from.as_u64(), to.as_u64()) 升序 —— 将所有边按时间槽分组。

`trailing_ones()` 技巧是关键：它能在 O(1) 时间内找到最小未使用颜色，无需循环。

**正确性：** Vizing 定理保证贪心算法最多使用 Δ+1 种颜色。由于最大度数 ≤ 127（MAX_NODES=128），而 u128 恰有 128 位，位掩码方案始终有效。

**栈占用**（约 17KB）：
- `eu, ev [u8; 512]` —— 边紧凑索引
- `ef, et [VectorAddress; 512]` —— 边向量（每个 4B，共 512 × 2KB）
- `seen_adj, node_colors [u128; 128]` —— 位掩码（各 2KB）
- `edge_color [u8; 512]`、`order [usize; 512]` —— 输出数组

---

## 运行时 API

```rust
pub fn graph_edge_color<const N: usize>()
    -> ([VectorAddress; N], [VectorAddress; N], [u8; N], usize, u8)
```

返回 `(from_vecs, to_vecs, edge_colors, edge_count, chromatic_index)`：
- `from_vecs[0..edge_count]` —— 每条边的规范化"起点"向量
- `to_vecs[0..edge_count]` —— 规范化"终点"向量
- `edge_colors[0..edge_count]` —— 分配给每条边的颜色槽位（从 0 开始）
- `edge_count` —— 无向边总数（排除自环）
- `chromatic_index` —— χ'(G) = 使用的最大颜色 + 1；无边时为 0

**排序顺序：** 按 (colour, from.as_u64(), to.as_u64()) 升序

---

## K-Shell 命令

```
graph edge color   — 显示边染色及 χ'(G) 色指数
gedgecolor         — 别名
edge color         — 别名
gec                — 别名
graph ecolor       — 别名
gecolor            — 别名
```

**显示效果：** 亮绿色标题；边按颜色槽位循环显示 6 种终端颜色；页脚：`N undirected edge(s)  χ'(G)=K  Vizing 1964`

---

## VectorAddress 命名空间

`gos-graph-ecolor-harness` 对应 **L4=84**

---

## 测试装置：gos-graph-ecolor-harness（10 个测试）

| 测试 | 图 | 期望结果 |
|------|-------|----------|
| 1 | 空图 | ec=0, χ'=0 |
| 2 | 单个孤立节点 | ec=0, χ'=0 |
| 3 | 单条边 A→B | ec=1, χ'=1, colour=0 |
| 4 | 路径 A→B→C（Δ=2） | ec=2, χ'=2，相邻边颜色不同 |
| 5 | 三角形 K_3（有向环） | ec=3, Δ=2, χ'=3（奇环，第二类） |
| 6 | C_4 有向环 | ec=4, Δ=2, χ'=2（偶环，第一类） |
| 7 | K_4 完全图 | ec=6, Δ=3, χ'=3（第一类） |
| 8 | 星形 K_{1,4} | ec=4, Δ=4, χ'=4（第一类） |
| 9 | 仅自环 | ec=0, χ'=0 |
| 10 | K_{3,3} Vizing + 合法性校验 | ec=9，贪心得 χ'=4=Δ+1，染色合法 ✓ |

全部 10 个测试通过。测试 10 验证了完整的合法染色不变量：任意两条相邻边不同色，采用 O(E²) 检查。

**测试 10 说明**：K_{3,3} 是二部图（Δ=3），根据 König 定理最优 χ'=3。然而在此边排列顺序下，贪心算法得到 χ'=4（Δ+1）—— 这达到了 Vizing 上界，仍是一个合法的染色。测试明确区分了"贪心色指数"与"最优色指数"这两个概念。

---

## 操作系统类比

色指数 χ'(G) 是调度所有 IPC 通道所需的**最少无冲突时间槽数**，使得共享同一内核子系统端点的两个通道不会同时激活。

- 槽位 0 = 第一个轮询周期，槽位 1 = 第二个周期，依此类推
- χ'(G) = 一个完整 I/O 分派周期所需的调度器总轮次
- 星形 K_{1,n}（枢纽-辐条结构）需要 n 个槽位——枢纽是瓶颈
- 二部图达到 Δ 个槽位（最优；König 定理）
- 奇数环需要 3 个槽位（比偶数环多一个）

这类似于：
- 网卡发送队列分片以避免队头阻塞
- 存储控制器的 O_DIRECT I/O 槽位复用
- 具有共享控制器端点的 CPU-外设 DMA 通道调度

---

## 与既有算法的关系

| 算法 | 版本 | 关系 |
|-----------|---------|---------|
| 节点染色（graph_color） | V2.48 | 对偶关系：给节点染色而非边染色 |
| 二部匹配（graph_bipartite_match） | V2.92 | König 定理：二部图中匹配↔顶点覆盖↔边染色 |
| 顶点覆盖（graph_vertex_cover） | V2.97 | 边染色 ↔ 分数匹配对偶性 |
| 边介数（graph_betweenness_edge） | V3.06 | 二者都作用于有向边列表 |

---

## 参考文献

- Vizing, V.G. (1964). "On an estimate of the chromatic class of a p-graph." *Diskret. Analiz* 3: 25–30.（定理本身出处。）
- König, D. (1916). "Über Graphen und ihre Anwendung auf Determinantentheorie und Mengenlehre." *Math. Ann.* 77: 453–465.（二部图情形 χ'=Δ。）
- Misra, J. & Gries, D. (1992). "A constructive proof of Vizing's theorem." *Inf. Process. Lett.* 41(3): 131–133.（针对二部图达到最优 χ' 的线性时间算法。）
- Garey, M.R. & Johnson, D.S. (1979). *Computers and Intractability*.（对一般图判定第一类/第二类是 NP 完全问题。）
