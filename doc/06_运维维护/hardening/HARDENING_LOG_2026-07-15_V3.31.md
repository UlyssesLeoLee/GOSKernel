# 硬化日志 V3.31 — SO\* + RSO + rSO Sombor族变体拓扑指数

| 字段 | 内容 |
|------|------|
| 文档编号 | HARDENING-V3.31 |
| 版本 | 1.0 |
| 状态 | 已完成 |
| 作成 | Claude（自动硬化任务，2026-07-15） |
| 审核 | — |
| 批准 | — |

## 变更摘要

新增图拓扑指数命令 `graph topo20`（别名 `gtopo20`），实现 **Sombor 族三个变体指数**：

| 指数 | 定义 | 类型 | 文献 |
|------|------|------|------|
| **SO\*(G)** | Σ_{uv∈E} d_u·d_v / √(d_u²+d_v²) | 改进 Sombor 指数（floor ppm） | Ghanbari & Rajabi-Parsa 2021 |
| **RSO(G)** | Σ_{uv∈E} 1 / √(d_u²+d_v²) | 倒数 Sombor 指数（floor ppm） | Gutman 2022 / He et al. 2022 |
| **rSO(G)** | Σ_{uv∈E} √((d_u−1)²+(d_v−1)²) | 约化 Sombor 指数（floor ppm） | Doslic et al. 2022 |

三者均为 **O(V+E) 度数扫描算法**，无需 BFS，栈占用约 3KB（adj[128] + deg[128]）。

## 关联图论意义（图论操作系统类比）

| 指数 | OS 类比 |
|------|---------|
| SO\* | 加权 Euclidean 范数通道强度（权重为 d_u·d_v；高 = 枢纽间耦合紧密） |
| RSO | 倒数 Euclidean 范数通道广度（高 = 均匀低度网格；低 = 枢纽辐射拓扑） |
| rSO | 约化 Sombor（使用 d-1 替换；= 0 当所有边均为悬垂-悬垂边） |

**SO\*=rSO 特殊情况：** 对于 d_u=d_v=2 的正则图（如 K₃），两者均等于 m√2·10^6，因为 d_a·d_b/√(d_a²+d_b²) = 2/√2 = √((d_a-1)²+(d_b-1)²) = √2。

## 实现细节

### 运行时（`crates/gos-runtime/src/lib.rs`）

新增方法 `GosRuntime::graph_topo_indices20_inner()` 及公开函数 `graph_topo_indices20()`：

```
返回类型：(so_star_ppm: u64, rso_ppm: u64, rso_red_ppm: u64, edge_count: usize, node_count: usize)
```

**实现公式（无浮点，no_std 安全）：**

```
floor(A/√B) = floor(√(A²/B))  [同一性：isqrt(A²/B)]

SO* per edge  = isqrt128((d_a·d_b)²·10^12 / (d_a²+d_b²))
RSO per edge  = isqrt64(10^12 / (d_a²+d_b²))
rSO per edge  = isqrt64(((d_a-1)²+(d_b-1)²)·10^12)
```

**溢出安全分析：**
- SO\*：`num = (d_a·d_b)²·10^12 ≤ (127²)²·10^12 ≈ 2.6×10^20 < u128::MAX` ✓
- RSO：`10^12/(d_a²+d_b²) ≤ 10^12/2 = 5×10^11 < u64::MAX` ✓
- rSO：`2·126²·10^12 ≈ 3.2×10^16 < u64::MAX` ✓

**关键不变量：**
- `rSO=0` 当且仅当所有边均为悬垂-悬垂边（d_u=d_v=1）
- 悬垂-枢纽边（d_leaf=1, d_hub=k）：rSO 贡献 = (k-1)·10^6（精确整数，无取整误差）
- Δ-正则图：`SO*=m·Δ/√2·10^6`；`RSO=m·10^6/(Δ√2)`；`rSO=m·(Δ-1)·√2·10^6`

### Shell（`crates/k-shell/src/`）

- `proc.rs`：新增路由条件（`graph topo20` / `gtopo20` / `modified sombor` / `gsostar` / `reciprocal sombor` / `grso` / `reduced sombor` / `grsom` / `gsostarsombrsom`）
- `lib.rs`：新增 `dispatch_graph_topo_indices20()` — bright-yellow 标题；SO\* bright-cyan；RSO bright-green；rSO bright-magenta（`rSO=0: all pendant-pendant` 注解）

## 验证数据（分析交叉校验）

| 图 | SO\*(ppm) | RSO(ppm) | rSO(ppm) | 边数 | 节点数 |
|----|-----------|----------|----------|------|--------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| 边 A-B (d=1,1) | 707_106 | 707_106 | **0** | 1 | 2 |
| 路径 P₃ | 1_788_854 | 894_426 | 2_000_000 | 2 | 3 |
| 三角 K₃ | **4_242_639** | 1_060_659 | **4_242_639** | 3 | 3 |
| 星图 K_{1,4} | 3_880_568 | 970_140 | **12_000_000** | 4 | 5 |
| 路径 P₄ | 3_203_067 | 1_247_979 | 3_414_213 | 3 | 4 |
| 完全图 K₄ | 12_727_920 | 1_414_212 | 16_970_562 | 6 | 4 |
| 两孤立节点 | 0 | 0 | 0 | 0 | 2 |
| 完全二部图 K_{2,3} | 9_984_600 | 1_664_100 | 13_416_402 | 6 | 5 |

**注：** K₃ 的 SO\*=rSO（均为 3√2·10^6）；K_{1,4} 的 rSO=12_000_000 精确无取整误差（(3)²+0²=9，√9=3）。

## 测试

新增测试套件 `host-tests/gos-graph-topo20-harness`（VectorAddress L4=107）：

- **10 项测试全部通过**（空图、单节点、单边、P₃、K₃、K_{1,4}、P₄、K₄、两孤立节点、K_{2,3}）
- 累计宿主测试总数：**1283**（V3.30 为 1273）

```
running 10 tests
test test_01_empty ... ok
test test_02_single_node ... ok
test test_03_single_edge ... ok
test test_04_path_p3 ... ok
test test_05_triangle_k3 ... ok
test test_06_star_k14 ... ok
test test_07_path_p4 ... ok
test test_08_complete_k4 ... ok
test test_09_two_isolated ... ok
test test_10_k23_bipartite ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Shell 命令

```
graph topo20    — 标准命令
gtopo20         — 快速别名
modified sombor — 按指数名
gsostar         — SO* 别名
reciprocal sombor — RSO 别名
grso            — RSO 快速别名
reduced sombor  — rSO 别名
grsom           — rSO 快速别名
gsostarsombrsom — 三合一别名
```

## 文件变更清单

| 文件 | 变更类型 | 说明 |
|------|----------|------|
| `crates/gos-runtime/src/lib.rs` | 新增 | `graph_topo_indices20_inner()` + 公开函数 |
| `crates/k-shell/src/proc.rs` | 新增 | `graph topo20` 路由 |
| `crates/k-shell/src/lib.rs` | 新增 | `dispatch_graph_topo_indices20()` |
| `host-tests/gos-graph-topo20-harness/` | 新增 | Cargo.toml + .cargo/config.toml + 10 项测试 |
| `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-15_V3.31.md` | 新增 | 本文档 |

## VectorAddress L4 命名空间更新

```
106 = graph-topo19   (V3.30, Λ + RCW + TW)
107 = graph-topo20   (V3.31, SO* + RSO + rSO)  ← 本轮新增
```
