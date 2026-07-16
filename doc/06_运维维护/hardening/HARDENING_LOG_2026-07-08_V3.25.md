# GOS 硬化日志 — V3.25

**日期**: 2026-07-08
**分支**: feat/vk-auto-live-surface
**提交**: 7da2fb2
**会话**: 自动化定时硬化任务（每 2 小时一次）

---

## 摘要

V3.25 新增三个基于离心率的拓扑指数 —— 总离心率（Total Eccentricity, TE）、离心距离和（Eccentric Distance Sum, EDS）、几何-算术离心率（Geometric-Arithmetic Eccentricity, GEA） —— 以及一个新的 10 项测试宿主套件（`gos-graph-topo14-harness`）。

这些指数与现有的离心率系列（V3.19: ECI+D+R+avg_ecc；V3.23: M1\*+M2\*+M3\*）相互补充，新增了最简单的聚合值（TE）、距离-离心率乘积（EDS），以及几何-算术指数在离心率上的类比（GEA）。

**宿主测试套件总计：1223 个测试**（V3.24 累计的 1213 个 + 新增 10 个）。

---

## 新功能: `graph_topo_indices14()` — TE + EDS + GEA

### API

```rust
pub fn graph_topo_indices14() -> (u64, u64, u64, usize, usize)
//                                 te  eds  gea   edges  nodes
```

### 定义

- **TE(G) = Σ_v ecc(v)** —— 总离心率指数（精确 u64；Dankelmann et al. 2004）
- **EDS(G) = Σ_v ecc(v)·T_v** —— 离心距离和（精确 u64；Gupta et al. 2008）
- **GEA(G) × 10^6 = Σ_{uv∈E} 2√(ecc(u)·ecc(v))/(ecc(u)+ecc(v))** —— 几何-算术离心率指数（向下取整 ppm）

其中：
- `ecc(v)` = 从 v 到任意可达节点的最大 BFS 距离（对孤立/单节点为 0）
- `T_v` = 顶点传输量 = Σ_{w reachable, w≠v} d(v,w)

### 关键不变量

- 当图**自中心**（所有 ecc 相等；离心率上的算术平均=几何平均）时，**GEA = |E|×10^6**
  - K_n（全部 ecc=1）、K_{r,s}（全部 ecc=2）、偶数环 C_{2k}（全部 ecc=k）：GEA = |E|×10^6
- **TE(K_n) = n**（全部 ecc=1）；**EDS(K_n) = n(n-1)**（ecc=1，T=n-1）
- **孤立节点**（ecc=0，T=0）：对全部三个指数贡献为 0；GEA 无边贡献

### 算法

1. 构建无向邻接位掩码：O(E)
2. 从每个源节点做 BFS —— 同时计算 ecc(v) 和 T_v：O(n·(n+m))
3. 节点扫描：TE = Σ ecc(v)；EDS = Σ ecc(v)·T_v
4. 边扫描（a < b）：GEA = Σ isqrt64(4·ea·eb·10^12) / (ea+eb)

**isqrt64** —— 牛顿-拉夫逊整数平方根（无浮点，no_std 安全）。
**溢出安全性**：4·127²·10^12 ≈ 6.5×10^16 < u64::MAX = 1.84×10^19。不可能溢出。

### 栈内存占用

- adj[128]（u128 × 128 = 2 KB）
- ecc[128]（u8 × 128 = 128 B）
- trans[128]（u64 × 128 = 1 KB）
- dist[128] + queue[128]（u8 × 256 = 256 B）
- **总计 ≈ 3.5 KB**（与 V3.23/V3.24 同一量级）

### 交叉验证表

| 图       | TE | EDS | GEA (ppm)  | 边数 | 节点数 |
|-------------|----|-----|------------|-------|-------|
| 空图       | 0  | 0   | 0          | 0     | 0     |
| 单节点     | 0  | 0   | 0          | 0     | 1     |
| 边 A-B    | 2  | 2   | 1_000_000  | 1     | 2     |
| 路径 P₃     | 5  | 14  | 1_885_618  | 2     | 3     |
| 三角形 K₃ | 3  | 6   | 3_000_000  | 3     | 3     |
| 星图 K₁,₄  | 9  | 60  | 3_771_236  | 4     | 5     |
| 路径 P₄     | 10 | 52  | 2_959_590  | 3     | 4     |
| 完全图 K₄ | 4  | 12  | 6_000_000  | 6     | 4     |
| 2 个孤立点  | 0  | 0   | 0          | 0     | 2     |
| K₂,₃        | 10 | 56  | 6_000_000  | 6     | 5     |

**P₃ 的 GEA 推导**：边 {A,B}：isqrt64(4×2×1×10^12)/3 = isqrt64(8e12)/3 = 2_828_427/3 = 942_809。GEA = 2×942_809 = 1_885_618。
**K₂,₃ 的 GEA = 6_000_000**：确认 K₂,₃ 是自中心的（全部 ecc=2）。交叉验证：GEA/|E| = 1_000_000。✓

### Shell 派发

```
"graph topo14" | "gtopo14" | "total eccentricity" | "gte"
| "eccentric distance sum" | "geds"
| "geometric arithmetic eccentricity" | "ggea"
| "gteedsge" | "gteedsegea"
```

### VectorAddress

`gos-graph-topo14-harness` 的 **L4=101**

### 显示

- 标题：亮黄色
- TE：亮青色 `[Σ_v ecc(v)] (exact)`
- EDS：亮绿色 `[Σ_v ecc(v)·T_v] (exact)`
- GEA：亮品红色 `[Σ 2√(ea·eb)/(ea+eb)] (self-centered | ppm)`
- 页脚：`N node(s) M edge(s) Dankelmann et al. 2004 Gupta et al. 2008`

### OS 类比

- **TE**：聚合路由到达半径预算 —— 全部节点的总离心率负载；TE 低表示拓扑紧凑（枢纽节点占主导），TE 高表示链条被拉长
- **EDS**：离心率加权的距离压力 —— 放大那些既远（高 ecc）又负载重（高 T_v）的边缘枢纽；有助于在深层依赖链中识别 IPC 瓶颈
- **GEA**：离心率通道均衡比 —— 自中心拓扑（路由到达范围均匀）时 =|E|；到达范围不对称（部分端点比其他端点距图中心远得多）时 <|E|

### 参考文献

- Dankelmann, Goddard & Swart 2004（总离心率）
- Gupta, Singh & Madan 2008（离心距离和 ξ^d）
- 几何-算术离心率指数：GA 指数（Vukičević & Furtula 2009）在离心率上的类比应用

---

## VectorAddress L4 命名空间（更新）

```
88=graph-topo, 89=graph-topo2, 90=graph-topo3, 91=graph-topo4, 92=graph-topo5,
93=graph-topo6, 94=graph-topo7, 95=graph-topo8, 96=graph-topo9, 97=graph-topo10,
98=graph-topo11, 99=graph-topo12, 100=graph-topo13, 101=graph-topo14
```

---

## 变更文件

| 文件 | 变更 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | +133 行：`graph_topo_indices14_inner()` + `graph_topo_indices14()` |
| `crates/k-shell/src/lib.rs` | +76 行：`dispatch_graph_topo_indices14()` |
| `crates/k-shell/src/proc.rs` | +2 行：10 个别名的 shell 路由 |
| `host-tests/gos-graph-topo14-harness/` | 新增：Cargo.toml, .cargo/config.toml, tests/graph_topo14.rs |

---

## 测试结果

```
running 10 tests
test test_01_empty         ... ok
test test_02_single_node   ... ok
test test_03_single_edge   ... ok
test test_04_path_p3       ... ok
test test_05_triangle_k3   ... ok
test test_06_star_k14      ... ok
test test_07_path_p4       ... ok
test test_08_complete_k4   ... ok
test test_09_two_isolated  ... ok
test test_10_k23_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored
```

---

## 硬化质量评估

- **No_std 安全**：仅使用 `core::` 基础设施、牛顿-拉夫逊 isqrt，无堆分配，无浮点
- **溢出安全**：4·127²·10^12 < u64::MAX 已通过解析方式验证
- **自中心不变量**：GEA=|E|×10^6 已在 K₃、K₄、K_{2,3} 上验证（测试 5、8、10）
- **孤立节点不变量**：ecc=0，T=0 → 贡献为零（测试 2、9）
- **BFS 正确性**：ecc 和 T 在单次 O(n·(n+m)) 遍历中计算完成（与 V3.19/V3.23 相同）
- **精度**：每条边 GEA = isqrt64(4·ea·eb·10^12)/(ea+eb)；取整误差 ≤ 每边 1 ppm
