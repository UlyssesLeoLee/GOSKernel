# 强化日志 — V3.93（2026-07-20）

## 摘要

为 GOS 图内核新增三项 Neighborhood S-variant 拓扑指数：

- **NHEXPENTAACTC** —— S-第56次幂顶点和：`Σ_v S(v)^56`
- **NHHEXPENTAACTC** —— S-第55次幂边和：`Σ_{uv∈E} (S_u+S_v)^55`
- **NAYSO** —— S-变体 Sombor 指数 SO^α，α=100（Centyl Sombor）：`Σ_{uv∈E} (S_u²+S_v²)^50`

新增 harness `gos-graph-topo82-harness`（10 项测试，全部通过）。

累计宿主测试数：**1903 项**。

---

## 数学定义

设 `S(v) = Σ_{w∈N(v)} deg(w)` 为顶点 `v` 的邻域度数和。

### NHEXPENTAACTC（S-第56次幂顶点和）

```
NHEXPENTAACTC(G) = Σ_v S(v)^56
```

- **系列定位**：pentacontic（50–59次幂）系列第7个指数
- **前驱**：NPENTAPENTAACTC = Σ S^55（V3.92，topo81）
- **S-正则图公式**：NHEXPENTAACTC = n · S^56
- **实现**：s^56 = s32 × s16 × s8（56 = 32+16+8；3 次乘法 —— 效率高！）
- **溢出处理**：饱和 u128 累加器，截断至 u64::MAX

### NHHEXPENTAACTC（S-第55次幂边和）

```
NHHEXPENTAACTC(G) = Σ_{uv∈E} (S_u + S_v)^55
```

- **系列定位**：将 NHPENTAPENTAACTC = Σ(S+S)^54（topo81）延伸至第55次幂
- **S-正则图公式**：NHHEXPENTAACTC = |E| · (2S)^55 = 36028797018963968 · |E| · S^55
- **实现**：ss^55 = ss32 × ss16 × ss4 × ss2 × ss（55 = 32+16+4+2+1；5 次乘法）

### NAYSO（S-Centyl Sombor，α=100）

```
NAYSO(G) = Σ_{uv∈E} (S_u² + S_v²)^50
```

- **系列定位**：广义 Sombor SO^α 家族中第3轮双字母 AY
- **前驱**：NAXSO（α=98，topo81）
- **S-正则图公式**：NAYSO = |E| · (2S²)^50 = 1125899906842624 · |E| · S^100
- **实现**：s2s^50 = s2s32 × s2s16 × s2s2（50 = 32+16+2；3 次乘法）
- **说明**：s^56 = s32×s16×s8 效率很高（56 = 32+16+8，三个2的幂次，仅需3次最终乘法）

---

## 解析测试值

| 图 | NHEXPENTAACTC | NHHEXPENTAACTC | NAYSO | 边数 | 节点数 |
|----------|-----------------------|-----------------------------|---------------------|-------|-------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| K₂ | 2 | 36_028_797_018_963_968 | 1_125_899_906_842_624 | 1 | 2 |
| P₃ | 216_172_782_113_783_808 | u64::MAX（饱和）| u64::MAX（饱和）| 2 | 3 |
| K₃ | u64::MAX（饱和）| u64::MAX（饱和）| u64::MAX（饱和）| 3 | 3 |
| K₄ | u64::MAX（饱和）| u64::MAX（饱和）| u64::MAX（饱和）| 6 | 4 |

**K₂ 推导**（S=1 均匀）：
- NHEXPENTAACTC：1^56 + 1^56 = 2
- NHHEXPENTAACTC：(1+1)^55 = 2^55 = 36_028_797_018_963_968
- NAYSO：(1²+1²)^50 = 2^50 = 1_125_899_906_842_624

**P₃ 推导**（S=2 均匀，3节点 × S^56）：
- NHEXPENTAACTC：3 × 2^56 = 3 × 72_057_594_037_927_936 = 216_172_782_113_783_808（可容纳于 u64）
- NHHEXPENTAACTC：2 × 4^55 = 2 × 2^110 → 饱和
- NAYSO：2 × 8^50 = 2 × 2^150 → 饱和

---

## 变更文件

| 文件 | 变更 |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices82_inner()` + 公开函数 `graph_topo_indices82()` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices82()` 显示函数 |
| `crates/k-shell/src/proc.rs` | 新增路由：`graph topo82`/`gtopo82`/`gnhexpentaactc`/`gnnhhexpentaactc`/`gnnayso` |
| `host-tests/gos-graph-topo82-harness/` | 新建 harness（10 项测试，全部通过） |

---

## Shell 命令

```
graph topo82
gtopo82
neighborhood hexapentacontic       (→ NHEXPENTAACTC)
gnhexpentaactc
neighborhood pentapentacontic edge (→ NHHEXPENTAACTC)
gnnhhexpentaactc
neighborhood centyl sombor         (→ NAYSO)
gnnayso
gnhexpentaactcnhhexpentaactcnayso
```

---

## VectorAddress 命名空间

- L4=168：gos-graph-topo81-harness（V3.92）
- **L4=169：gos-graph-topo82-harness（V3.93，本次变更）**

---

## 运行时 API

```rust
gos_runtime::graph_topo_indices82() -> (nhexpentaactc: u64, nhhexpentaactc: u64, nayso: u64, edge_count: usize, node_count: usize)
```

- 插件：`TOPIX_82`
- 执行器：`t82.exec`

---

## 背景说明

本次变更是持续自动化强化周期的一部分。S-变体 pentacontic 系列（topo76–topo85）实现了
建立在邻域度数和（S-变体）之上的一系列高次幂顶点/边拓扑指数。

每次迭代新增三项指数：
1. 顶点和 S^n（每版本递增 1 次幂）
2. 边和 (S_u+S_v)^(n-1)（次幂少 1）
3. 广义 S-变体 Sombor 指数 SO^α，α = 2×(n-6)（每版本 α 递增 2）

双字母 SO 命名（NAASO、NABSO、……、NAXSO、NAYSO）沿着扩展系列追踪相对于
初始 NSO（α=1，topo21）的 α/2 偏移量。
