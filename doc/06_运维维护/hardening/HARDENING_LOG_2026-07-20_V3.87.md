# 强化日志 — V3.87（2026-07-20）

## 摘要

V3.87 新增 **NPENTAACTC + NHPENTAACTC + NASSO** —— 三项新的 Neighborhood
S-variant 拓扑指数，实现为 `gos_runtime::graph_topo_indices76()`，开启
pentacontic（50–59）次幂系列。新增 `gos-graph-topo76-harness`（10 项新测试）。

**宿主测试套件总数：1843 项**（新增 10 项，全部通过）。

---

## 新增拓扑指数（topo76）

三项指数均使用 `S(v) = Σ_{w∈N(v)} deg(w)`（邻域度数和），
与 topo18/topo21–topo76 系列保持一致。

### NPENTAACTC —— S-第50次幂顶点和

```
NPENTAACTC(G) = Σ_v S(v)^50    (u128→u64 饱和)
```

- 将 NNONATETRAACTC = ΣS⁴⁹（topo75）延伸至第50次幂
- pentacontic（50–59）系列首个指数
- S-正则图公式：`NPENTAACTC = n·S^50`
- 实现：`s^50 = s32 × s16 × s2`（50=32+16+2；3 次乘法 —— 效率高）

### NHPENTAACTC —— S-第49次幂边和

```
NHPENTAACTC(G) = Σ_{uv∈E} (S_u + S_v)^49    (u128→u64 饱和)
```

- 将 NHNONATETRAACTC = Σ(S+S)⁴⁸（topo75）延伸至第49次幂
- S-正则图公式：`NHPENTAACTC = 562_949_953_421_312 · |E| · S^49`
  （系数 = 2^49）
- 实现：`ss^49 = ss32 × ss16 × ss`（49=32+16+1；3 次乘法）

### NASSO —— S-变体 Sombor 指数（α=88）

```
NASSO(G) = Σ_{uv∈E} (S_u² + S_v²)^44    (u128→u64 饱和)
```

- S-变体广义 Sombor 指数 SO^α，α=88；第3轮双字母 "AS"
- 序列：… → NARSO(α=86, topo75) → NASSO(α=88, topo76)
- S-正则图公式：`NASSO = 17_592_186_044_416 · |E| · S^88`（系数 = 2^44）
- 实现：`s2s^44 = s2s32 × s2s8 × s2s4`（44=32+8+4；3 次乘法 —— 效率高！）

---

## VectorAddress L4 命名空间

| L4 值 | Harness |
|----------|---------|
| 88–162   | graph-topo 至 graph-topo75 |
| **163**  | **graph-topo76**（本版本） |

---

## 关键测试值

| 图 | NPENTAACTC | NHPENTAACTC | NASSO | 边数 | 节点数 |
|----------|-----------------------|-----------------------|---------------------|-------|-------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| K₂ | 2 | 562_949_953_421_312 | 17_592_186_044_416 | 1 | 2 |
| P₃ | 3_377_699_720_527_872 | u64::MAX（饱和）| u64::MAX（饱和）| 2 | 3 |
| K₃ | u64::MAX（饱和）| u64::MAX（饱和）| u64::MAX（饱和）| 3 | 3 |
| K_{1,4} | u64::MAX（饱和）| u64::MAX（饱和）| u64::MAX（饱和）| 4 | 5 |
| P₄ | u64::MAX（饱和）| u64::MAX（饱和）| u64::MAX（饱和）| 3 | 4 |
| K₄ | u64::MAX（饱和）| u64::MAX（饱和）| u64::MAX（饱和）| 6 | 4 |
| K_{2,3} | u64::MAX（饱和）| u64::MAX（饱和）| u64::MAX（饱和）| 6 | 5 |

**P₃ 的 NPENTAACTC 推导**：3 × 2^50 = 3 × 1_125_899_906_842_624 = 3_377_699_720_527_872（精确容纳于 u64）。

---

## 变更文件

| 文件 | 变更 |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices76_inner()`、`graph_topo_indices76()` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices76()` |
| `crates/k-shell/src/proc.rs` | 新增 topo76 的 k-shell 调度条目 |
| `host-tests/gos-graph-topo76-harness/` | 新建 harness（10 项测试，全部通过） |

---

## k-shell 命令

```
graph topo76           gtopo76
neighborhood pentacontic              gnpentaactc
neighborhood nonapentacontic edge     gnhpentaactc
neighborhood octaocontyl sombor       gnnasso
gnpentaactcnhpentaactcnasso
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
test test_09_two_isolated  ... ok
test test_10_k23_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured
```

`cargo check --manifest-path crates/gos-runtime/Cargo.toml`：构建通过，无警告。
