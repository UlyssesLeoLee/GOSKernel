# GOSKernel 强化日志 V3.42 — 2026-07-16

## 版本
**V3.42** — NSig + NHQS + NPS Neighborhood S-variant 拓扑指数 + gos-graph-topo31-harness（10 项测试）

## 分支
`feat/vk-auto-live-surface`（自动强化排程任务）

## 动机
延续自 V3.29（NM₁/NM₂/GA₂）至 topo30（NVQ/NRGS/NHCS）建立的 S-variant 拓扑指数家族。本版本将两条正在进行中的顶点幂次序列与边幂次序列各扩展一项，并引入 S-Sigma 作为标准 Sigma 不规则度指数的 S-模拟量。

## 新增指数

### NSig — S-Sigma 不规则度
```
NSig(G) = Σ_{uv∈E} (S_u − S_v)²
```
- 经典 Sigma 不规则度指数 σ(G) = Σ(d_u−d_v)² 的 S-模拟量（Gutman, Togan, Yurttas 等）
- **当且仅当 S-正则时 NSig = 0**（每条边两端的邻居度数和相等）
- 精确 u64；对现实图不会发生整数溢出（每边最大 ≤ 16129² ≈ 2.60×10⁸；总和 < u64::MAX）
- 与 NM3 = Σ|S_u−S_v|（topo23）互补：NM3 使用绝对值，NSig 使用平方

### NHQS — Neighborhood Hyper Quartic Sum（邻域超四次方和）
```
NHQS(G) = Σ_{uv∈E} (S_u + S_v)^4
```
- 扩展边和幂次序列：NHM1 = Σ(S+S)²（topo23），NHCS = Σ(S+S)³（topo30） → NHQS = Σ(S+S)⁴
- **对 S-正则图，NHQS = 16|E|S⁴**（因为 (2S)⁴ = 16S⁴）
- K₃ 与 K_{1,4}：均为 S-均匀 S=4 → 每边 NHQS 相同（8⁴=4096）；总值因 |E| 不同而不同
- u128 累加器 → u64 输出（每边值可容纳于 u64；大图的边求和可能超出）

### NPS — Neighborhood Penta Sum（邻域五次方和）
```
NPS(G) = Σ_v S(v)^5
```
- 扩展顶点幂次序列：NM₁=Σ S²（topo18），NF=Σ S³（topo22），NVQ=Σ S⁴（topo30） → NPS=Σ S⁵
- **对 S-正则图，NPS = n·S⁵**
- u128 累加器 → u64 输出（对大图中最大度数节点，S⁵ 可能超过 u64::MAX）

## 交叉验证表

| 图    | NSig | NHQS    | NPS     | 边数 | 点数 |
|----------|------|---------|---------|-------|-------|
| 空图    | 0    | 0       | 0       | 0     | 0     |
| 单节点   | 0    | 0       | 0       | 0     | 1     |
| K₂       | 0    | 16      | 2       | 1     | 2     |
| P₃       | 0    | 512     | 96      | 2     | 3     |
| K₃       | 0    | 12,288  | 3,072   | 3     | 3     |
| K_{1,4}  | 0    | 16,384  | 5,120   | 4     | 5     |
| P₄       | 2    | 2,546   | 550     | 3     | 4     |
| K₄       | 0    | 629,856 | 236,196 | 6     | 4     |
| 两孤立点| 0   | 0       | 0       | 0     | 2     |
| K_{2,3}  | 0    | 124,416 | 38,880  | 6     | 5     |

值得注意：P₄ 是唯一 NSig > 0 的测试用例（S-不规则：S 值为 2,3,3,2 → 两条边的 S_u≠S_v）。

## 算法
O(V+E) — 度数遍历 → S(v) 计算 → 顶点扫描（NPS） + 边扫描（NSig、NHQS）；无需 BFS。

## 实现

### gos-runtime/src/lib.rs
- 在 `GosRuntime` 上新增 `graph_topo_indices31_inner()`
- 新增公共函数 `graph_topo_indices31() -> (u64, u64, u64, usize, usize)`
- 返回顺序：(nsig, nhqs, nps, edge_count, node_count)

### k-shell/src/lib.rs
- 新增 `dispatch_graph_topo_indices31()`，附带彩色输出：
  - NSig：亮青色（精确值；适用时附 "NSig=0: S-regular" 注记）
  - NHQS：亮绿色（精确值）
  - NPS：亮品红（精确值）

### k-shell/src/proc.rs
- 路由：`"graph topo31"` | `"gtopo31"` | `"neighborhood sigma"` | `"gnsig"` | `"neighborhood quartic edge"` | `"gnhqs"` | `"neighborhood penta"` | `"gnps"` | `"gnsignhqsnps"`

### host-tests/gos-graph-topo31-harness/
- 新建独立测试 harness，VectorAddress L4=118
- 10 项测试：全部通过 ✓

## 测试计数
- 此前总计：1383（V3.41）
- 新增测试：10（gos-graph-topo31-harness）
- **新总计：1393 个宿主测试**

## VectorAddress L4 命名空间（更新后）
88=graph-topo 至 117=graph-topo30，**118=graph-topo31**（新增）

## 系列脉络
本次提交延续了 "S-variant family" 的一贯模式，即在经典拓扑指数公式中将度数 d(v) 替换为邻居度数和 S(v) = Σ_{w∈N(v)} deg(w)：
- **顶点幂次序列**：NM₁(Σ S²) → NF(Σ S³) → NVQ(Σ S⁴) → **NPS(Σ S⁵)**
- **边和幂次序列**：NHM1(Σ(S+S)²) → NHCS(Σ(S+S)³) → **NHQS(Σ(S+S)⁴)**
- **不规则度**：NM3(Σ|S−S|) → **NSig(Σ(S−S)²)**
