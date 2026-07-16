# GOSKernel 硬化日志 — V3.14

**日期：** 2026-07-06
**分支：** feat/vk-auto-live-surface
**宿主测试：** 1113（此前 1103，新增 10）

---

## V3.14 — SDD + ISI + Nirmala 基于度数的拓扑指数

### 新增运行时 API

```rust
pub fn graph_topo_indices3() -> (u64, u64, u64, usize, usize)
// 返回值：(sdd_ppm, isi_ppm, ni_ppm, edge_count, node_count)
```

### 新增指数

| 符号 | 名称 | 公式 | 参考文献 |
|--------|------|---------|-----------|
| SDD | 对称除法度数指数 | Σ_{uv∈E} (da²+db²)/(da·db) | Vasilyev 2014 / Gupta et al. 2000 |
| ISI | 逆和入度指数 | Σ_{uv∈E} da·db/(da+db) | Sedlar et al. 2011 |
| NI | Nirmala 指数 | Σ_{uv∈E} √(da+db) | Rather et al. 2021 |

### 整数精度

- **SDD**：`sdd_ppm = Σ floor((da²+db²) × 10^6 / (da·db))` — 当 `da=db`（正则图）时精确
- **ISI**：`isi_ppm = Σ floor(da·db × 10^6 / (da+db))` — 当 `(da+db)` 整除 `da·db × 10^6` 时精确
- **NI**：`ni_ppm = Σ isqrt64((da+db) × 10^12)` — 牛顿-拉夫逊向下取整平方根；当 `da+db` 为完全平方数时精确

### 关键不变量

**SDD：**
- SDD ≥ 2|E| 恒成立（AM-GM 不等式：(da²+db²)/(da·db) ≥ 2）
- **当且仅当图为正则图**（所有边 da=db）时 SDD = 2|E|
- 满足等式时 shell 会显示标注

**ISI：**
- 对任意 Δ-正则图，ISI = |E|·Δ/2（精确）
- K₃（Δ=2，3 条边）：ISI = 3（精确）
- K₄（Δ=3，6 条边）：ISI = 9（精确）

**NI：**
- 对 Δ-正则图，NI = |E|·√(2Δ)（当 2Δ 为完全平方数时精确）
- K₃（Δ=2）：NI = 3·√4 = 6 **精确**（da+db=4，isqrt64(4×10^12) = 2_000_000）
- K₄（Δ=3）：NI = 6·√6 ≈ 14.697（向下取整；2Δ=6 非完全平方数）

### isqrt64 关键取值

```
isqrt64(2_000_000_000_000) = 1_414_213  (√2 × 10^6；向下取整)
isqrt64(3_000_000_000_000) = 1_732_050  (√3 × 10^6；向下取整)
isqrt64(4_000_000_000_000) = 2_000_000  (√4 × 10^6；精确)
isqrt64(5_000_000_000_000) = 2_236_067  (√5 × 10^6；向下取整)
isqrt64(6_000_000_000_000) = 2_449_489  (√6 × 10^6；向下取整)
```

### 解析交叉校验表

| 图 | SDD_ppm | ISI_ppm | NI_ppm | 边数 | 备注 |
|-------|---------|---------|--------|-------|-------|
| 空图 | 0 | 0 | 0 | 0 | |
| 单节点 | 0 | 0 | 0 | 0 | |
| 边 A-B | 2_000_000 | 500_000 | 1_414_213 | 1 | da=db=1；AM-GM 等式成立 |
| P₃ | 5_000_000 | 1_333_332 | 3_464_100 | 2 | |
| K₃ | 6_000_000 | 3_000_000 | 6_000_000 | 3 | 三个不变量均精确 |
| K_{1,4} | 17_000_000 | 3_200_000 | 8_944_268 | 4 | SDD > 2|E| 严格成立 |
| P₄ | 7_000_000 | 2_333_332 | 5_464_100 | 3 | |
| K₄ | 12_000_000 | 9_000_000 | 14_696_934 | 6 | SDD=2|E|；ISI=|E|Δ/2 精确 |
| 2 个孤立节点 | 0 | 0 | 0 | 0 | |
| K_{2,3} | 12_999_996 | 7_200_000 | 13_416_402 | 6 | SDD > 2|E| 严格成立；ISI 精确 |

### 算法（O(V+E)）

与 V3.12/V3.13 相同的紧凑索引 + 无向邻接位掩码构建方式。
按 `a < b` 规范顺序进行边扫描：

```rust
// SDD: floor((da²+db²) × 10^6 / (da·db))
sdd_acc += (da * da + db * db) * 1_000_000 / p;

// ISI: floor(da·db × 10^6 / (da+db))
isi_acc += p * 1_000_000 / s;

// NI: isqrt64((da+db) × 10^12)
ni_acc += isqrt64(s * 1_000_000_000_000u64);
```

溢出安全性：最大 `da²+db² ≤ 2·128² = 32_768`；`(da²+db²)×10^6 ≤ 3.27×10^10` — 可容纳于 u64。
最大 `s×10^12 = 256×10^12 = 2.56×10^14` — 可容纳于 u64（上限约 1.8×10^19）。

### Shell 命令

```
graph topo3   gtopo3
symmetric division deg   gsdd
inverse sum indeg        gisi
nirmala index            gnirmala
gsddisini
```

### 操作系统类比

- **SDD** = IPC 通道间带宽不对称因子——SDD 越高表示中枢辐射型拓扑，耦合不均衡；SDD=2|E| 表示完全均衡（网状/环状拓扑）
- **ISI** = 每通道调和平均度数积——衡量有效耦合强度；当所有端点负载相等时 ISI=|E|·Δ/2
- **NI** = IPC 路径的总"宽度"——√(da+db) 对更宽的通道赋予更高权重；高 NI 表示以粗管道为主的拓扑

### VectorAddress 命名空间

```
L4=90: gos-graph-topo3-harness
```

（此前：L4=89 gos-graph-topo2-harness，L4=88 gos-graph-topo-harness，L4=87 gos-graph-zagreb-harness）

### 参考文献

- Vasilyev / Gupta et al. 2000（SDD）
- Sedlar, Stevanović & Vasilyev 2011（ISI）
- Rather, Imran & Degree 2021（Nirmala 指数）
- 遵循基于度数的指数系列：V3.11（M1/M2/R/I）→ V3.12（SC/GA/AZI）→ V3.13（H/ABC/F）→ V3.14（SDD/ISI/NI）
</content>
