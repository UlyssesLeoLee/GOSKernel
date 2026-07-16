# 硬化日志 —— V2.96 最大独立集
**日期：** 2026-07-05
**分支：** feat/vk-auto-live-surface
**Commit：** af1fa8e

## 摘要

实现了 `graph_independent_set<N>()`——通过对补图 G̅ 应用带 Tomita 主元的
Bron-Kerbosch 算法求得最大独立集（MIS）。

**关键洞察：** G̅ 中的最大团 = G 中的最大独立集。
补图邻接关系 `comp[i] = all_nodes & !adj[i] & !(1 << i)` 通过 O(n) 次
位运算即可计算得出；BK 算法在 `comp[]` 上运行，使用与 V2.95 的
`graph_clique` 相同的迭代栈。

## 新增产物

### gos-runtime
- `graph_independent_set_inner<N>` —— 在补图上运行 BK 算法，返回 (is_vecs, α, is_count, n)
- `graph_independent_set<N>` —— 公开包装函数

### k-shell
- `dispatch_graph_independent_set` —— 亮洋红色标题，亮蓝色显示独立集成员
- Shell：`graph independent set` / `graph indep` / `gindep` / `independent set` / `indep`

### gos-graph-indep-harness（L4=72）
10 项测试——全部通过：
1. 空图：α=0
2. 单个节点：α=1
3. 两个孤立节点：α=2（唯一 MIS = 两者均含）
4. 单条边 A-B：α=1, is_count=2
5. 三角形 K3：α=1, is_count=3
6. 路径 P4：α=2, is_count=3
7. K4 完全图：α=1, is_count=4
8. 星形 K_{1,4}：α=4, is_count=1（所有叶子节点）
9. 二分图 K_{3,3}：α=3, is_count=2；König 交叉验证 α=n-ν
10. K4 交叉验证：α·ω≥n, α+ω=n+1（完美图）

## 算法不变量

| 性质 | 公式 |
|---|---|
| 独立数 | α(G) = ω(G̅) |
| König（二分图） | α(G) = n − ν(G) |
| 补图 | comp[i] = all_nodes & !adj[i] & !self |
| 完美图界 | α(G) + ω(G) = n+1（对于 K_n） |
| 一般界 | α(G) · ω(G) ≥ n（点传递图） |

## 操作系统类比

α(G) 表示内核子系统中**彼此之间没有直接依赖关系**的最大集合的规模——
即最大可并行的启动或热补丁前沿。相当于在 `make -jN` 依赖图中找到最宽的
"独立工作批次"（互相之间没有边的那些节点可以无需同步地并行调度）。

## 测试计数：宿主测试总计 933 个
- 此前：V2.95 之前累计 923 个
- gos-graph-indep-harness：新增 +10
