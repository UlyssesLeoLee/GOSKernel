# HARDENING LOG — V3.124 (2026-07-29)

## 摘要

在 V3.123 (NOCTAHEXACTC, S^86，八旬系列第 7 个) 基础上，新增三个 S-变体邻域拓扑指数
**NOCTAHEPTACTC**(S^87) + **NHOCTAHEPTACTC**((S_u+S_v)^86) + **NBDDSO**(SO^α, α=162)，
新建 `gos-graph-topo113-harness`（10 个测试，全部通过），宿主测试套件累计达 **2211 个测试**。

本次更新为 **八旬（octacontic，80-89）系列第 8 个**（NOCTAHEPTACTC），
NB 系列推进至第 30 个（字母 DD，α=162）。

---

## 新增拓扑指数定义

| 指数 | 公式 | 说明 |
|------|------|------|
| NOCTAHEPTACTC | Σ_v S(v)^87 | S-八十七次方顶点和；**八旬系列第 8 个 (80-89)** |
| NHOCTAHEPTACTC | Σ_{uv∈E} (S_u+S_v)^86 | S-八十六次方边和 |
| NBDDSO | Σ_{uv∈E} (S_u²+S_v²)^81 | S-变体 Sombor SO^α，α=162；NB 系列第 30 个 (字母 DD) |

其中 S(v) = Σ_{w∈N(v)} deg(w) 为顶点 v 的邻居度之和（S-变体）。

---

## 实现细节

### `crates/gos-runtime/src/lib.rs`

新增 `graph_topo_indices113_inner()` 及公共函数 `graph_topo_indices113()`：

- **NOCTAHEPTACTC**: s^87 = s64 × s16 × s4 × s2 × s（87=64+16+4+2+1，10 次乘法）
- **NHOCTAHEPTACTC**: ss^86 = ss64 × ss16 × ss4 × ss2（86=64+16+4+2，9 次乘法）
- **NBDDSO**: s2s^81 = s2s64 × s2s16 × s2s（81=64+16+1，8 次乘法）
- 所有累加器使用 u128 饱和算术，最终截断到 u64::MAX

### `crates/k-shell/src/lib.rs`

新增 `dispatch_graph_topo_indices113()`，支持：
- 命令：`graph topo113` / `gtopo113` / `neighborhood octaheptic` / `gnnoctaheptactc` 等（9 个别名）
- 输出三列：NOCTAHEPTACTC（亮青色）、NHOCTAHEPTACTC（亮绿色）、NBDDSO（亮洋红色）

### `crates/k-shell/src/proc.rs`

在 topo112 路由前插入 topo113 分支。

### `host-tests/gos-graph-topo113-harness/`

新建独立 Cargo workspace 宿主测试套件：
- VectorAddress L4=200 命名空间
- Plugin: `TOPIX113`，Executor: `t113.exec`
- 10 个测试覆盖：空图、单节点、K₂、P₃、K₃、K_{1,4}、P₄、K₄、双孤立节点、K_{2,3}

---

## K₂ 解析验证

```
S(A)=S(B)=1（均匀，1条边，2个节点）
NOCTAHEPTACTC  : 1^87 + 1^87 = 2                          ✓（不饱和，S=1是唯一不饱和情形）
NHOCTAHEPTACTC : (1+1)^86 = 2^86 ≈ 7.73×10²⁵ > u64::MAX → 饱和至 u64::MAX ✓
NBDDSO         : (1²+1²)^81 = 2^81 ≈ 2.42×10²⁴ > u64::MAX → 饱和至 u64::MAX ✓
```

---

## 测试结果

```
running 10 tests
test test_01_empty        ... ok
test test_02_single_node  ... ok
test test_03_k2_edge      ... ok
test test_04_path_p3      ... ok
test test_05_triangle_k3  ... ok
test test_06_star_k14     ... ok
test test_07_path_p4      ... ok
test test_08_complete_k4  ... ok
test test_09_two_isolated ... ok
test test_10_bipartite_k23 ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## 系列进度

| 系列 | 进度 |
|------|------|
| 八旬（octacontic，80-89） | 8/10（第 1-8 个已完成：NOCTAACTC S^80 … NOCTAHEPTACTC S^87） |
| NB 系列（SO^α on S-variant） | 第 30 个（DD，α=162）；下一个：EE，α=164 |

下一个里程碑：八旬系列第 9 个 NOCTAOCTACTC (S^88) + NHOCTAOCTACTC + NBEESO (α=164)，即 topo114。
