# GOSKernel 强化日志 V3.38 — 2026-07-16

## 摘要

为 GOSKernel runtime 与 k-shell 新增三个 Neighborhood S-variant 拓扑指数（topo27 族）：**NRR**（Neighborhood Reciprocal Randić）、**NSO\***（Neighborhood Modified Sombor）、**NrSO**（Neighborhood Reduced Sombor）。这是 S-variant 指数系列（V3.22–V3.38）的第 16 篇，将 VectorAddress 命名空间的索引数扩展至 L4=114。全部计算为纯整数运算（`no_std` 安全）。

---

## 数学背景

S-variant 指数将经典公式中的度数 `d(v)` 替换为**邻居度数和**：

```
S(v) = Σ_{w ∈ N(v)} deg(w)
```

### NRR — Neighborhood Reciprocal Randić（邻域倒数 Randić 指数）

```
NRR(G) = Σ_{uv ∈ E} 1 / (S_u · S_v)
```

倒数 Randić 指数 `R_{-1}`（Bollobás & Erdős 1998）的 S-模拟量。

运行时公式（无浮点）：每边 `floor(10^6 / (S_u · S_v))`（单位 ppm）。

### NSO\* — Neighborhood Modified Sombor（邻域修正 Sombor 指数）

```
NSO*(G) = Σ_{uv ∈ E} (S_u · S_v) / √(S_u² + S_v²)
```

修正 Sombor 指数 SO\*（Ghanbari & Rajabi-Parsa 2021）的 S-模拟量。

运行时公式：每边 `isqrt128(S_u² · S_v² · 10^12 / (S_u² + S_v²))`（单位 ppm）。

### NrSO — Neighborhood Reduced Sombor（邻域简化 Sombor 指数）

```
NrSO(G) = Σ_{uv ∈ E} √((S_u - 1)² + (S_v - 1)²)
```

简化 Sombor 指数 rSO（Doslic et al. 2022）的 S-模拟量。

运行时公式：每边 `isqrt128(((S_u-1)² + (S_v-1)²) · 10^12)`（单位 ppm）。

**溢出说明**：对高度数图，`(S_u-1)² + (S_v-1)²` × 10^12 可达 ~5.2×10^20，超过 u64::MAX（~1.84×10^19）。通过在 isqrt128 之前使用 u128 中间量处理。

---

## 关键不变量

| 不变量 | 条件 |
|---|---|
| NRR = \|E\| × 10^6 | 全部 S=1（仅 K₂） |
| 每边 NSO\* = NrSO | S-均匀 S=2（P₃ 的边：均等于 √2 × 10^6 = 1_414_213） |
| NrSO = 0 | 全部 S=1（K₂：两端点的 (S-1)²=0） |
| K₃ ≡ K_{1,4}（三个指数均一致） | S-均匀 S=4 的巧合 |
| K_{2,3} 每边 NSO\* = K₃ 每边 NrSO | 两者均等于 isqrt128(18×10^12) = 4_242_640 |

---

## 实现

### 变更文件

| 文件 | 变更内容 |
|---|---|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices27_inner()` + `graph_topo_indices27()` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices27()` |
| `crates/k-shell/src/proc.rs` | 新增 `graph topo27` / `gtopo27` / `gnrr` / `gnsos` / `gnrso2` 等命令路由 |

### 新建文件

| 文件 | 说明 |
|---|---|
| `host-tests/gos-graph-topo27-harness/Cargo.toml` | 独立 workspace 清单 |
| `host-tests/gos-graph-topo27-harness/.cargo/config.toml` | 宿主目标覆盖配置（x86_64-pc-windows-msvc） |
| `host-tests/gos-graph-topo27-harness/tests/graph_topo27.rs` | 10 项测试，附完整解析交叉验证 |

### Shell 命令

```
graph topo27
gtopo27
gnrr
gnsos
gnrso2
gnrrnsosnrso
neighborhood reciprocal randic
neighborhood modified sombor
neighborhood reduced sombor
```

### 返回签名

```rust
pub fn graph_topo_indices27() -> (u64, u64, u64, usize, usize)
//                                nrr  nsos nrso  edges  nodes
// 三个指数均为百万分率（ppm，向下取整）。
```

### VectorAddress

L4 = **114**（topo27 harness 命名空间）

---

## 测试结果

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### 测试矩阵

| 测试 | 图 | NRR (ppm) | NSO\* (ppm) | NrSO (ppm) | 边数 | 点数 |
|---|---|---|---|---|---|---|
| 01 | 空图 | 0 | 0 | 0 | 0 | 0 |
| 02 | 单孤立节点 | 0 | 0 | 0 | 0 | 1 |
| 03 | K₂（单边） | 1_000_000 | 707_106 | 0 | 1 | 2 |
| 04 | P₃（路径） | 500_000 | 2_828_426 | 2_828_426 | 2 | 3 |
| 05 | K₃（三角形） | 187_500 | 8_485_281 | 12_727_920 | 3 | 3 |
| 06 | K_{1,4}（星图） | 250_000 | 11_313_708 | 16_970_560 | 4 | 5 |
| 07 | P₄（路径） | 444_443 | 5_449_520 | 7_300_561 | 3 | 4 |
| 08 | K₄（完全图） | 74_070 | 38_183_766 | 67_882_248 | 6 | 4 |
| 09 | 两个孤立节点 | 0 | 0 | 0 | 0 | 2 |
| 10 | K_{2,3}（二部图） | 166_662 | 25_455_840 | 42_426_402 | 6 | 5 |

---

## 参考文献

- Bollobás, B. & Erdős, P. (1998). Graphs of extremal weights. *Ars Combinatoria*, 50, 225–233.（原始 Randić 型指数 R_{-1}）
- Ghanbari, N. & Rajabi-Parsa, S. (2021). A variant of the Sombor index. *MATCH Commun. Math. Comput. Chem.*, 86, 669–683.（SO\*）
- Doslic, T., Réti, T., & Ali, A. (2022). On the reduced Sombor index and its applications. *MATCH Commun. Math. Comput. Chem.*, 88, 529–543.（rSO）

---

## V3.38 之后的版本状态

- **分支**：`feat/vk-auto-live-surface`
- **宿主测试**：1353（1343 + topo27 harness 新增 10 个）
- **S-variant 指数**：topo22–topo27 已完成（V3.33–V3.38）
  - topo22: NR, NF, NSC
  - topo23: NHM1, NSDD, NM3
  - topo24: NISI, NAZI, NEM1
  - topo25: NHM2, NAG, NABS
  - topo26: NPC, NRM₂, NRSO
  - topo27: NRR, NSO\*, NrSO
- **VectorAddress L4**：114（topo27 harness）
