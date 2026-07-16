# 硬化日志 V2.84 — 图链接预测（CN / Jaccard / Adamic-Adar / 资源分配）

**日期：** 2026-07-04
**分支：** feat/vk-auto-live-surface
**提交：** f7f9fc7
**宿主测试总计：** 813（此前 803，+10）

---

## 功能：`graph predict <u> <v>` / `gpredict <u> <v>`

### 动机

生产级图分析平台（NetworkX、igraph、Neo4j GDS）都提供**链接预测**指标，
用于基于两个节点邻域的共享结构，量化二者之间缺失边形成的可能性。
GOSKernel 此前已具备丰富的拓扑分析能力（中心性、效率、社区检测），
但缺乏任何机制来推理内核依赖图中*潜在*的未来连接。

V2.84 新增了源自 Liben-Nowell & Kleinberg（2003）以及 Adamic & Adar（2003）
文献的四个经典链接预测分数：

| 指标 | 公式 | 含义 |
|---|---|---|
| 共同邻居数（CN） | \|N(u) ∩ N(v)\| | 共享邻居的数量 |
| Jaccard 系数 | CN / \|N(u) ∪ N(v)\| | 归一化重叠度 |
| Adamic-Adar（AA） | Σ_{w∈CN} 1/ln(deg(w)) | 按对数度数倒数加权 |
| 资源分配（RA） | Σ_{w∈CN} 1/deg(w) | 按容量加权 |

所有分数都是数值越高 → 预测缺失边 u→v 会形成的可能性越强。

操作系统类比：LLDP / CDP 邻居表预测——哪些内核子系统在结构上已经
准备好形成新的依赖边？

---

## 实现

### crates/gos-runtime/src/lib.rs

**新增方法**（位于 `GraphRuntime` 内，即 `impl GraphRuntime` 中）：
```rust
pub fn graph_link_predict_inner(
    &self,
    u: VectorAddress,
    v: VectorAddress,
) -> (usize, u32, u32, u32, usize)
// 返回 (cn, jaccard_ppm, aa_ppm, ra_ppm, node_count)
```

**算法：**
1. 将 u 与 v 解析为节点槽位；若任一未知或 u == v，则返回全零。
2. 一次扫描所有边（O(E)）以构建两个 128 位邻居位向量（`[u64; 2]`），
   并累积每个槽位的总无向度（`deg[slot]`）。
3. 从各自的位向量中清除 u 和 v（互斥处理）。
4. 交集位计数 → CN；并集位计数 → |N(u) ∪ N(v)|，用于 Jaccard。
5. 逐字扫描交集以累积 AA 与 RA：
   - AA：使用内嵌的 LN_TABLE[k]（与 V2.77/V2.80 使用的是同一张表）；
     项 = 1e12/LN_TABLE[deg]（单位 ppm）。
   - RA：项 = 1_000_000 / deg(w)。
6. 累加器饱和至 u32::MAX，防止溢出回绕。

**新增公开函数：**
```rust
pub fn graph_link_predict(u: VectorAddress, v: VectorAddress) -> (usize, u32, u32, u32, usize)
```

**关键不变量：**
- 邻域视为无向：边 u→w 或 w→u 都会让 w 计入 N(u)。
- u 与 v 相互排斥：`nbr_u[v_slot / 64] &= !(1 << (v_slot % 64))` 等。
- 退化保护：`if u_slot == v_slot { return (0,0,0,0,node_count); }`
- AA 通过 `if ln_d > 0` 跳过 deg ≤ 1 的情形（LN_TABLE[0]=LN_TABLE[1]=0）。
- RA 通过 `if d > 0` 跳过 deg = 0 的情形（孤立自环节点）。
- 自环在度数中只计一次：`if fs == ts { deg[fs] += 1 }`。
- 复杂度：每次调用 O(V + E)（一次边扫描 + 一次位扫描）。

### crates/k-shell/src/lib.rs

**新增函数** `dispatch_graph_predict(sink, u, v)`：
- 标题：`graph predict u → v`
- 包含 4 个指标行的表格：共同邻居数（原始计数）、jaccard、adamic-adar、资源分配。
- 每个指标都有颜色编码：灰色=0，黄色=弱，绿色=强。
- 脚注：`N node(s)  prediction: likely / weak / none`（N 个节点，预测：可能/较弱/无）。
- 通过内联的 `print_predict_ppm` 辅助函数以 6 位小数显示 ppm 值。

### crates/k-shell/src/proc.rs

**新增路由**（放置在 `graph compare` / `gcompare` 分发之后）：
```
graph predict <u> <v>   →  dispatch_graph_predict(u, v)
gpredict <u> <v>        →  别名
link predict <u> <v>    →  别名
predict <u> <v>         →  别名
```

在 `graph snapshot` / `graph compare` 条目旁新增了**帮助文本**。

---

## 测试装置：`host-tests/gos-graph-link-predict-harness`

**VectorAddress L4=60** 标识本装置的命名空间。

| 测试 | 图 | 预测 | 期望结果 |
|---|---|---|---|
| 1 | 空图 | 任意节点对 | CN=0，全零，nc=0 |
| 2 | 单节点 A | (A, B) | CN=0，nc=1 |
| 3 | A、B（无边） | (A, B) | CN=0，全零 |
| 4 | A→B | (A, B) | CN=0（互斥处理移除了 N(A) 中的 B） |
| 5 | A→B→C | (A, C) | CN=1，J=1M，AA≈1.443M，RA=500K |
| 6 | 任意图 | (A, A) 退化情形 | 全零 |
| 7 | 星形 A→{B,C,D} | (B, C) | CN=1，J=1M，AA≈910K，RA=333K |
| 8 | 菱形 A→{B,C}→D | (A, D) | CN=2，J=1M，AA≈2.885M，RA=1M |
| 9 | {A→B} ∥ {C→D} | (A, D) | CN=0，全零 |
| 10 | A→B | (A, unknown) | CN=0，nc=2 |

**结果：** 10/10 通过。

---

## 指标数值推导

对于测试 5（路径 A→B→C，预测 A、C）：`deg(B)=2`：
- AA = 1_000_000_000_000 / LN_TABLE[2] = 1e12 / 693_147 = 1_442_695（≈ 1/ln(2) × 10^6）
- RA = 1_000_000 / 2 = 500_000

对于测试 7（星形，预测叶子对 B、C）：`deg(A)=3`：
- AA = 1e12 / LN_TABLE[3] = 1e12 / 1_098_612 ≈ 910_239（≈ 1/ln(3) × 10^6）
- RA = 1_000_000 / 3 = 333_333（整数除法）

对于测试 8（菱形，预测 A、D）：`deg(B)=deg(C)=2`：
- AA = 2 × 1_442_695 = 2_885_390
- RA = 2 × 500_000 = 1_000_000

---

## VectorAddress L4 命名空间更新

| L4 | 装置 |
|---|---|
| 59 | gos-graph-snapshot-harness (V2.83) |
| **60** | **gos-graph-link-predict-harness (V2.84)** |

---

## 文献参考

- D. Liben-Nowell & J. Kleinberg，《社交网络中的链接预测问题》，CIKM 2003。
- L. Adamic & E. Adar，《Web 上的朋友与邻居》，Social Networks 25(3)，2003。
- T. Zhou, L. Lü & Y.-C. Zhang，《基于局部信息预测缺失链接》，EPJB 71，2009。

Adamic-Adar 是标准基准指标；资源分配（Zhou 2009）在稀疏图上往往表现更优。
两者均予以纳入以求完整性。
