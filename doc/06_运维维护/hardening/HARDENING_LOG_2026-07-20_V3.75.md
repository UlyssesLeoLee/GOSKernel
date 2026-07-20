# HARDENING LOG — V3.75 (2026-07-20)

## 摘要

新增三个 Neighborhood S-variant 拓扑指数族 **NOCTATRIACTC + NHOCTATRIACTC + NAGSO**
（topo64），覆盖 S-幂次顶点和至第38次方、S-幂次边和至第37次方、
以及广义 Sombor 指数 SO^α（α=64，第3轮双字母序列AG）。
新建 `gos-graph-topo64-harness`（10 个测试），宿主测试总数升至 **1723**。

---

## 变更内容

### 1. `crates/gos-runtime/src/lib.rs`

新增 `RuntimeState::graph_topo_indices64_inner()` 及公开函数 `graph_topo_indices64()`：

```
gos_runtime::graph_topo_indices64() -> (noctatriactc: u64, nhoctatriactc: u64, nagso: u64, edge_count: usize, node_count: usize)
```

**NOCTATRIACTC(G) = Σ_v S(v)^38**
- S(v) = Σ_{w∈N(v)} deg(w)（邻居度数之和，与 topo18–topo64 族一致）
- S-Octatriacontic 顶点和；延伸自 NHEPTATRIACTC=Σ S^37（topo63）
- S 正则图：NOCTATRIACTC = n·S^38
- 实现：s^38 = s32 × s4 × s2（s32=s16²，s4=s2²；38=32+4+2）

**NHOCTATRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^37**
- S-Heptatriacontic 边和；延伸自 NHHEPTATRIACTC=Σ(S+S)^36（topo63）
- S 正则图：NHOCTATRIACTC = |E|·(2S)^37 = 137_438_953_472·|E|·S^37
- 实现：ss^37 = ss32 × ss4 × ss（37=32+4+1）

**NAGSO(G) = Σ_{uv∈E} (S_u²+S_v²)^32**
- S-Tetrahexacontyl Sombor SO^α，α=64；无需开方，精确整数
- 3rd-pass 双字母序 AG（接续 NAFSO α=62，topo63）
- S 正则图：NAGSO = |E|·(2S²)^32 = 4_294_967_296·|E|·S^64
- 实现：s2s^32 = s2s16 × s2s16（完全平方，32=16+16）

所有运算使用 u128 饱和累加器，截断至 u64::MAX。

### 2. `crates/k-shell/src/lib.rs`

新增 `dispatch_graph_topo_indices64()`，输出：
- `S-octatriacontic-vtx  NOCTATRIACTC= <值>  [Σ_v S(v)³⁸]`
- `S-heptatriacontic-edge NHOCTATRIACTC= <值>  [Σ_{uv∈E} (S_u+S_v)³⁷]`
- `S-tetrahexacontyl-sb   NAGSO=        <值>  [Σ_{uv∈E} (S_u²+S_v²)³²]`

### 3. `crates/k-shell/src/proc.rs`

新增 Shell 命令路由：
```
"graph topo64" | "gtopo64" | "neighborhood octatriacontic" | "gnoctatriactc"
| "neighborhood heptatriacontic edge" | "gnhoctatriactc"
| "neighborhood tetrahexacontyl sombor" | "gnnagso"
| "gnoctatriactcnhoctatriactcnagso"
```

### 4. `host-tests/gos-graph-topo64-harness/`

新建独立测试 crate，含 10 个测试（VectorAddress L4=151，plugin=TOPIX_64，executor=t64.exec）：

| # | 图         | NOCTATRIACTC                | NHOCTATRIACTC     | NAGSO             |
|---|------------|-----------------------------|-------------------|-------------------|
| 1 | 空图       | 0                           | 0                 | 0                 |
| 2 | 单孤立点   | 0                           | 0                 | 0                 |
| 3 | K₂         | **2**                       | **137_438_953_472** | **4_294_967_296** |
| 4 | P₃         | **824_633_720_832**         | u64::MAX          | u64::MAX          |
| 5 | K₃         | u64::MAX                    | u64::MAX          | u64::MAX          |
| 6 | K_{1,4}   | u64::MAX                    | u64::MAX          | u64::MAX          |
| 7 | P₄         | **2_701_703_985_101_798_066** | u64::MAX        | u64::MAX          |
| 8 | K₄         | u64::MAX                    | u64::MAX          | u64::MAX          |
| 9 | 两孤立点   | 0                           | 0                 | 0                 |
|10 | K_{2,3}   | u64::MAX                    | u64::MAX          | u64::MAX          |

---

## 关键推导

**K₂**（S=1，1条边，2节点）：
- NOCTATRIACTC = 1^38 + 1^38 = 2 ✓
- NHOCTATRIACTC = (1+1)^37 = 2^37 = 137_438_953_472 ✓
- NAGSO = (1²+1²)^32 = 2^32 = 4_294_967_296 ✓

**P₃**（S=2 均匀，2条边，3节点）：
- NOCTATRIACTC = 3×2^38 = 824_633_720_832 ✓
- NHOCTATRIACTC = 2×4^37 = 2×2^74 → 饱和 ✓
- NAGSO = 2×8^32 = 2×2^96 → 饱和 ✓

**P₄**（S(A)=S(D)=2，S(B)=S(C)=3；3条边，4节点）：
- 3^32 = 1_853_020_188_851_841
- 3^36 = 3^32×81 = 150_094_635_296_999_121
- 3^38 = 3^36×9 = 1_350_851_717_672_992_089
- 2×3^38 = 2_701_703_435_345_984_178
- 2×2^38 = 549_755_813_888
- **Total = 2_701_703_985_101_798_066** ✓

**NAGSO 完全平方优化**：
- s2s^32 = (s2s16)^2（仅需 1 次乘法，相比 s2s^31 减少 1 步）
- 这是第5个完全平方指数：ss^32（topo59），s^32（topo58），s2s^32（topo64）等

---

## S-Sombor 双字母序列进展

| Sombor α | 指数名  | Topo   |
|----------|---------|--------|
| 60       | NAESO   | topo62 |
| 62       | NAFSO   | topo63 |
| **64**   | **NAGSO** | **topo64** |

---

## VectorAddress L4 命名空间

88=graph-topo … 150=graph-topo63，**151=graph-topo64**

---

## 测试结果

```
running 10 tests
test test_01_empty ... ok
test test_02_single_node ... ok
test test_03_k2_edge ... ok
test test_04_path_p3 ... ok
test test_05_triangle_k3 ... ok
test test_06_star_k14 ... ok
test test_07_path_p4 ... ok
test test_08_complete_k4 ... ok
test test_09_two_isolated ... ok
test test_10_k23_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored
```

`cargo check -p gos-kernel` 通过（无新增错误，预存警告不变）。

---

## 宿主测试计数历史

| 版本  | 新增测试 crate              | 总计  |
|-------|----------------------------|-------|
| V3.73 | gos-graph-topo62-harness   | 1703  |
| V3.74 | gos-graph-topo63-harness   | 1713  |
| **V3.75** | **gos-graph-topo64-harness** | **1723** |

---

## Commit

`feat(v3.75): NOCTATRIACTC + NHOCTATRIACTC + NAGSO Neighborhood S-variant indices + gos-graph-topo64-harness (10 tests)`
