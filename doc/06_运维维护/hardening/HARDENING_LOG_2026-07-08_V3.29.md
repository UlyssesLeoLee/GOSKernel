# 硬化日志 — V3.29（2026-07-08）

## 摘要

为 GOS 运行时新增**邻域 Zagreb NM₁ + NM₂ + GA₂** 拓扑指数。
这些是基于度数的指数，使用**邻居度数和** S(v) = Σ_{u∈N(v)} deg(u) 作为二阶度数度量，相比一阶度数能提供更丰富的局部拓扑视角。

## 新功能：`graph topo18`

**Shell 命令**：`graph topo18` / `gtopo18` / `neighborhood zagreb` / `gnm1nm2` /
`nm1 index` / `gnm1` / `nm2 index` / `gnm2` / `neighborhood ga` / `gga2` / `gnm1nm2ga2`

**函数**：`gos_runtime::graph_topo_indices18() -> (nm1: u64, nm2: u64, ga2_ppm: u64, edge_count: usize, node_count: usize)`

### 指数定义

设 S(v) = Σ_{u∈N(v)} deg(u)（v 的所有邻居的度数之和——"二阶度数"）。

| 指数 | 公式 | 类型 | 参考文献 |
|-------|---------|------|-----------|
| NM₁(G) | Σ_v S(v)² | 精确 u64 | Mondal et al. 2019 |
| NM₂(G) | Σ_{uv∈E} S(u)·S(v) | 精确 u64 | Mondal et al. 2019 |
| GA₂(G) | Σ_{uv∈E} 2√(S_u·S_v)/(S_u+S_v) | 下取整 ppm（isqrt128） | — |

### 关键不变量

- **S 值均匀不变量**：当所有 S(v) 相等时，GA₂ = |E| × 10⁶。适用于：
  K_n（完全图）、K_{r,s}（完全二部图）、K_{1,k}（星图）、P₃、正则图。
- **孤立节点**：S(v) = 0 → 对三个指数的贡献均为 0。
- 对空图或全孤立节点图，**NM₁ = NM₂ = GA₂ = 0**。

### 交叉核对表

| 图 | NM₁ | NM₂ | GA₂（ppm） | S 值 |
|-------|-----|-----|-----------|----------|
| 空图 | 0 | 0 | 0 | — |
| 单节点 | 0 | 0 | 0 | S(A)=0 |
| 单边 A-B | 2 | 1 | 1_000_000 | 全部 S=1 |
| 路径 P₃ | 12 | 8 | 2_000_000 | 全部 S=2 |
| 三角形 K₃ | 48 | 48 | 3_000_000 | 全部 S=4 |
| 星图 K_{1,4} | 80 | 64 | 4_000_000 | 全部 S=4 |
| 路径 P₄ | 26 | 21 | 2_959_590 | S=(2,3,3,2) |
| 完全图 K₄ | 324 | 486 | 6_000_000 | 全部 S=9 |
| 两个孤立节点 | 0 | 0 | 0 | 全部 S=0 |
| K_{2,3} | 180 | 216 | 6_000_000 | 全部 S=6 |

### P₄ 的 GA₂ 推导

对 P₄ = A-B-C-D：S(A)=2, S(B)=3, S(C)=3, S(D)=2。

- {A,B}：isqrt128(4·2·3·10¹²)/5 = isqrt128(24·10¹²)/5 = 4_898_979/5 = **979_795**
- {B,C}：isqrt128(4·3·3·10¹²)/6 = 6·10⁶/6 = **1_000_000**
- {C,D}：与 {A,B} 相同 = **979_795**
- GA₂ = 979_795 + 1_000_000 + 979_795 = **2_959_590** ✓

### 算法

1. 紧凑节点索引：O(V)
2. 构建无向邻接位掩码：O(E)
3. 度数数组：deg[ci] = adj[ci].count_ones()
4. S(v) = Σ_{u∈N(v)} deg(u)：逐节点遍历邻接位
5. NM₁ = Σ_v S(v)²：节点扫描
6. NM₂、GA₂：无向边扫描（a < b）
   - 每条边 GA₂ = isqrt128(4·S_a·S_b·10¹²) / (S_a+S_b)
   - 溢出：S(v) ≤ 127·127 = 16129；4·S²·10¹² ≤ 约 10²¹，在 u128 范围内 ✓

**总复杂度**：O(V+E)，无需 BFS。
**栈空间**：adj[128]（2KB）+ deg[128]（1KB）+ sv[128]（1KB）≈ 共 4KB。

### 操作系统类比

| 指标 | 在图操作系统语境下的含义 |
|--------|-----------------------------|
| NM₁ | 二跳路由压力的平方（放大高连接度邻域） |
| NM₂ | 二跳边共负载乘积（边两端均处于密集邻域中） |
| GA₂ | 邻域均衡比（S 值均匀时 =|E|，如 K_n、K_{r,s}；非对称时 <|E|） |

### VectorAddress

`gos-graph-topo18-harness` 对应 L4 = 105。

## 变更文件

| 文件 | 变更内容 |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices18_inner()` 方法（O(V+E) 计算 NM₁+NM₂+GA₂）+ 公开 API `graph_topo_indices18()` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices18()` 显示函数 |
| `crates/k-shell/src/proc.rs` | 新增 `graph topo18` / `gtopo18` 及 9 个别名的路由 |
| `host-tests/gos-graph-topo18-harness/` | 新建 harness（10 项测试，全部通过） |

## 测试结果

```
test test_01_empty        ... ok
test test_02_single_node  ... ok
test test_03_single_edge  ... ok
test test_04_path_p3      ... ok
test test_05_triangle_k3  ... ok
test test_06_star_k14     ... ok
test test_07_path_p4      ... ok
test test_08_complete_k4  ... ok
test test_09_two_isolated ... ok
test test_10_k23_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**宿主测试套件总计：1263 个测试**（此前 1253 个 + 新增 10 个）。

---

*本文件于 2026-07-15 按文档管理规范就地中文化，仅译语言，不改动版本号 / 文件路径 / 函数名 / 测试名 / 测试计数等既成事实。*
