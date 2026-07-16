# 硬化日志 V3.52 — NPTC + NHQTC + NIOSO 邻域 S-变体指标 + gos-graph-topo41-harness（10 项测试）

**日期：** 2026-07-16
**分支：** feat/vk-auto-live-surface
**宿主测试总计：** 1493（此前 1483，+10）

---

## 新增功能：NPTC + NHQTC + NIOSO 邻域 S-变体拓扑指标（VectorAddress L4=128）

### 背景

本版本延续 V3.51（NQTC + NHTC + NGSO，topo40）构建的 S-变体指标体系，引入三个新的高次幂图论拓扑指标。这三个指标与 Windows 性能计数器、Linux perf 计数器同属"可计算、可比较、可归档"的图系统健康度量量，是图论操作系统区别于传统 OS 的核心特色之一。

### 新增拓扑指标

**定义**：S(v) = Σ_{w∈N(v)} deg(w)（邻域度之和，又称"S-变体度"）

#### NPTC — S-十五次幂顶点和（S-Pentadecic Vertex Sum）

```
NPTC(G) = Σ_v S(v)^15
```

- 类型：精确 u128→u64（饱和截断）
- 延伸 S-幂次顶点序列：NM₁(S²) → NF(S³) → ... → NQTC(S¹⁴) → **NPTC(S¹⁵)**
- S-正则图：NPTC = n·S^15
- 溢出：S^15 ≤ 16129^15，使用饱和 u128 累加器

#### NHQTC — S-十四次幂边和（S-Tetradecic Edge Sum）

```
NHQTC(G) = Σ_{uv∈E} (S_u + S_v)^14
```

- 类型：精确 u128→u64（饱和截断）
- 延伸 S-幂次边序列：NHM1(Σ(S+S)²) → ... → NHTC(Σ(S+S)¹³) → **NHQTC(Σ(S+S)¹⁴)**
- S-正则图：NHQTC = |E|·(2S)^14 = 16384|E|·S^14
- 溢出：(2×16129)^14，使用饱和 u128 累加器

#### NIOSO — S-十八次 Sombor 指标（S-Octadecic Generalised Sombor, α=18）

```
NIOSO(G) = Σ_{uv∈E} (S_u² + S_v²)^9
```

- 类型：精确 u128→u64（无 isqrt，不需开方）
- S-广义 Sombor SO^α 序列（偶数 α，精确整数）：
  NSO(α=1) → NCSO(α=3) → NFSO(α=4) → NHSO(α=6) → NOSO(α=8) →
  NTSO(α=10) → NDSO(α=12) → NESO(α=14) → NGSO(α=16) → **NIOSO(α=18)**
- S-正则图：NIOSO = |E|·(2S²)^9 = 512|E|·S^18
- 注意：K₄（S=9）单边值 162^9 > u64::MAX，饱和截断为 u64::MAX
- 溢出：(2×16129²)^9，使用饱和 u128 累加器

### 实现细节

**计算流程（O(V+E)）**：
1. 紧凑节点索引（slot → compact index）
2. 无向邻接位掩码 + 边计数
3. 度数数组
4. S(v) = Σ_{w∈N(v)} deg(w)
5. 顶点扫描：NPTC（S^15 = S^8 × S^4 × S^2 × S）
6. 边扫描（a < b）：NHQTC（(S+S)^14 = ss^8 × ss^4 × ss^2）+ NIOSO（(S²+S²)^9 = s2s^8 × s2s）

**关键代码位置**：
- 内部方法：`crates/gos-runtime/src/lib.rs` — `graph_topo_indices41_inner()`
- 公开函数：`crates/gos-runtime/src/lib.rs` — `graph_topo_indices41()`
- Shell 分发：`crates/k-shell/src/lib.rs` — `dispatch_graph_topo_indices41()`
- 路由：`crates/k-shell/src/proc.rs` — "graph topo41" 等命令

**Shell 命令**（k-shell）：
```
graph topo41        gtopo41
gnptc               neighborhood pentadecic
gnhqtc              neighborhood tetradecic edge
gnioso              neighborhood octadecic sombor
gnptcnhqtcnioso
```

**VectorAddress 命名空间**：L4=128（topo41-harness）

**插件**：TOPIX_41，执行器：t41.exec

### 解析验证表

| 图       | NPTC（精确）          | NHQTC（精确）               | NIOSO（精确）              | 边 | 点 |
|----------|----------------------|----------------------------|---------------------------|----|----|
| Empty    | 0                    | 0                          | 0                         | 0  | 0  |
| 1 node   | 0                    | 0                          | 0                         | 0  | 1  |
| K₂       | 2                    | 16_384                     | 512                       | 1  | 2  |
| P₃       | 98_304               | 536_870_912                | 268_435_456               | 2  | 3  |
| K₃       | 3_221_225_472        | 13_194_139_533_312         | 105_553_116_266_496       | 3  | 3  |
| K_{1,4}  | 5_368_709_120        | 17_592_186_044_416         | 140_737_488_355_328       | 4  | 5  |
| P₄       | 28_763_350           | 90_571_195_346             | 219_568_289_114           | 3  | 4  |
| K₄       | 823_564_528_378_596  | 2_248_880_205_492_486_144  | u64::MAX（饱和）           | 6  | 4  |
| 2 iso.   | 0                    | 0                          | 0                         | 0  | 2  |
| K_{2,3}  | 2_350_924_922_880    | 7_703_510_787_293_184      | 311_992_186_885_373_952   | 6  | 5  |

**K₄ NIOSO 饱和推导**：
- S=9，S²=81，S_u²+S_v²=162
- 162^9 = 474_373_168_346_071_296 × 162 = 76_848_453_272_063_549_952 > u64::MAX
- 每条边已饱和，6 条边 × 饱和值 → 钳制为 u64::MAX ✓

**S-正则公式验证**：
- NPTC  = n·S^15                          ✓（K₂, P₃, K₃, K_{1,4}, K₄, K_{2,3}）
- NHQTC = |E|·(2S)^14 = 16384|E|·S^14   ✓
- NIOSO = |E|·(2S²)^9 = 512|E|·S^18     ✓（K₄ 除外，因饱和）

### 测试结果

**gos-graph-topo41-harness**：10/10 通过

```
test test_01_empty         ... ok
test test_02_single_node   ... ok
test test_03_single_edge   ... ok  (K₂)
test test_04_path_p3       ... ok
test test_05_triangle_k3   ... ok
test test_06_star_k14      ... ok
test test_07_path_p4       ... ok
test test_08_complete_k4   ... ok  (NIOSO=u64::MAX 饱和测试)
test test_09_two_isolated  ... ok
test test_10_k23_bipartite ... ok
```

**gos-kernel** 编译验证：`cargo check -p gos-kernel` 清洁通过。

### 与前版对比

| 版本  | 新增指标            | harness     | 宿主测试总计 |
|-------|---------------------|-------------|------------|
| V3.51 | NQTC + NHTC + NGSO  | topo40 (10) | 1483       |
| V3.52 | NPTC + NHQTC + NIOSO| topo41 (10) | **1493**   |

### 操作系统类比

- **NPTC**：S-十五次幂顶点压力（图论等价于 `perf stat cycles^15` 的邻域加权聚合）
- **NHQTC**：S-十四次幂边耦合强度（IPC 通道的 S-Tetradecic 协同总量）
- **NIOSO**：S-十八次 Sombor 欧几里得范数（邻域度不对称性的极高次放大，α=18 时精确无需开方）

---

*本日志由 GOSKernel 每2小时自动强化任务生成（2026-07-16）*
