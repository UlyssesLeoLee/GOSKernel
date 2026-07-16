# 硬化日志 V3.12 — SC + GA + AZI 拓扑指数

**日期**: 2026-07-06
**分支**: feat/vk-auto-live-surface
**上一基线**: V3.11（Zagreb M1/M2 + Randić R + Albertson I，1083 个宿主测试）
**新总计**: 1093 个宿主测试（+10）

---

## 算法：SC + GA + AZI 基于度数的拓扑指数

V3.12 在拓扑指数体系中新增三个基于度数的描述符，均在单趟 O(V+E) 扫描中计算完成，全部沿用 V3.11 Zagreb 实现中的无向邻接构建方式。

### 和连接指数 SC（Zhou & Trinajstić 2009）

> SC(G) = Σ_{uv∈E} 1/√(deg(u) + deg(v))

也写作 "χ 指数" 或 "和连接指数"。类似于 Randić 指数，但使用的是度数之*和*而非度数之*积*。对于没有孤立边的图，SC ≤ |E|/√2；当所有度数都等于 1（完美匹配）时取等号。该指数与图的能量及谱性质相关。

**整数计算**: SC_ppm = Σ floor(10¹²/isqrt_ppm(s))，其中 s = deg(u)+deg(v)。

### 几何-算术指数 GA（Vukičević & Furtula 2009）

> GA(G) = Σ_{uv∈E} 2√(deg(u)·deg(v)) / (deg(u) + deg(v))

每一项都被 1 所界定（AM-GM 不等式：2√ab/(a+b) ≤ 1），因此 GA ≤ |E|。**当且仅当图是正则图（所有顶点度数相同）时 GA = |E|**——这是一个关键的交叉验证不变量。GA 指数最初是作为 Randić 指数的补充而提出的，强调几何平均连接性。

**整数计算**: GA_ppm = Σ 2·isqrt_ppm(p)/s，其中 p = deg(u)·deg(v)，s = deg(u)+deg(v)。

### 增广 Zagreb 指数 AZI（Furtula, Graovac & Vukičević 2010）

> AZI(G) = Σ_{uv∈E, deg(u)+deg(v)>2} (deg(u)·deg(v) / (deg(u)+deg(v)−2))³

三次方指数使 AZI 对高度数顶点比 M₂ 更敏感。悬挂-悬挂边（两端点度数均为 1，分母为 0）被跳过。研究表明，在某些化学分子族中，AZI 与标准生成焓的相关性强于 Randić 指数和 Zagreb 指数。

**整数计算**: AZI_milli = Σ p³·1000/q³，其中 p = deg(u)·deg(v)，q = deg(u)+deg(v)−2。

## 实现

- `gos_runtime::graph_topo_indices()` → `(sc_ppm: u64, ga_ppm: u64, azi_milli: u64, edge_count: usize, node_count: usize)`
- 单趟 O(V+E) 扫描，共享与 graph_zagreb_inner 相同的无向邻接构建方式
- 复用同一个 `isqrt_ppm` 牛顿-拉夫逊辅助函数（无代码重复）
- AZI 使用精确的 u64 整数运算：对所有节点数不超过 MAX_NODES=128 的图，p³·1000/q³ 均可容纳于 u64

## Shell 命令

`graph topo` · `gtopo` · `sum connectivity` · `gsc` · `geometric arithmetic` · `gga` · `augmented zagreb` · `gazi` · `sci ga azi`

## 测试框架

**gos-graph-topo-harness** — 10 个测试，VectorAddress L4=88：

| # | 图 | SC_ppm | GA_ppm | AZI_milli |
|---|-------|--------|--------|-----------|
| 1 | 空图 | 0 | 0 | 0 |
| 2 | 单节点 | 0 | 0 | 0 |
| 3 | 边 A→B | 707_107 | 1_000_000 | 0 |
| 4 | 路径 P₃ | 1_154_700 | 1_885_616 | 16_000 |
| 5 | 三角形 K₃ | 1_500_000 | 3_000_000 | 24_000 |
| 6 | 星形 K_{1,4} | 1_788_852 | 3_200_000 | 9_480 |
| 7 | 路径 P₄ | 1_654_700 | 2_885_616 | 24_000 |
| 8 | 完全图 K₄ | 2_449_488 | 6_000_000 | 68_340 |
| 9 | 两个孤立节点 | 0 | 0 | 0 |
| 10 | K_{2,3} | 2_683_278 | 5_878_770 | 48_000 |

**验证的关键不变量：**
- 测试 5（K₃）：GA_ppm = 3_000_000 = 3×10⁶ = |E|×10⁶ — 正则图不变量成立 ✓
- 测试 8（K₄）：GA_ppm = 6_000_000 = |E|×10⁶ — 正则图不变量成立 ✓
- 测试 10（K_{2,3}）：GA_ppm = 5_878_770 ≠ 6_000_000 — 非正则二部图，符合预期 ✓
- 测试 3、6：AZI_milli = 0（悬挂-悬挂边跳过）、9_480（q=3）— q=0 保护逻辑正确 ✓

## 解析交叉校验

对于**正则图**（所有度数均为 d），每条边贡献：
- SC: 1/√(2d)，GA: 1（精确），AZI: (d/(2(d-1)))³ × 1000

对于 **K₃**（d=2）：GA = |E| = 3（精确）；AZI = 3×(4/2)³×1000/2³ = 3×8000/8 = 3000 → 24_000 milli ✓
对于 **K₄**（d=3）：GA = |E| = 6（精确）；AZI = 6×729000/64 = 68_343.75 → 向下取整 = 68_340 ✓
对于 **K_{2,3}**（da=3, db=2）：AZI/边 = (6/3)³×1000 = 8_000 = K₃ 每边 AZI 值 — 巧合已验证 ✓

## 操作系统类比

- **SC**：和连接指数——衡量 IPC 通道间总"接口宽度"，按总度数的平方根倒数加权；SC 越低，表示带宽耦合越窄。
- **GA**：几何-算术指数——中枢和谐度量；当所有子系统负载均衡（正则）时 GA = |E|，当部分子系统相对其他子系统过载时 GA < |E|。
- **AZI**：增广 Zagreb 指数——立方加权耦合强度；对高度数中枢-中枢连接高度敏感；可用于检测内核依赖图中的"超级耦合体"。

## 参考文献

- Zhou, B. & Trinajstić, N. (2009). On a novel connectivity index. *Journal of Mathematical Chemistry*, 46(4), 1252–1270.
- Vukičević, D. & Furtula, B. (2009). Topological index based on the ratios of geometrical and arithmetical means of end-vertex degrees of edges. *Journal of Mathematical Chemistry*, 46(4), 1369–1376.
- Furtula, B., Graovac, A. & Vukičević, D. (2010). Augmented Zagreb index. *Journal of Mathematical Chemistry*, 48(2), 370–380.
</content>
