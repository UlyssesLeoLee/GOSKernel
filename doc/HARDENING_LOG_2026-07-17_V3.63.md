# GOS 强化日志 V3.63 — 2026-07-17

> **归位说明（2026-07-19）**：本文件此前仅存在于 `doc/` 根目录，从未归位至 `06_运维维护/hardening/`。本轮已复制归档至 [06_运维维护/hardening/HARDENING_LOG_2026-07-17_V3.63.md](06_运维维护/hardening/HARDENING_LOG_2026-07-17_V3.63.md)（内容一致，原为纯中文，未做改写）。根目录本文件按文档管理规范保留，作为硬化当时的原始存档快照。

## 版本概要

**V3.63** — NHEXATC + NHHEXATC + NVSO Neighborhood S-variant 拓扑指数

**分支**: `feat/vk-auto-live-surface`
**提交**: `3b1199b`
**宿主测试总数**: 1603（原 1593 + 新增 10）

---

## 新增内容

### 三个新的 S-变体拓扑指数 (topo52)

**gos_runtime::graph_topo_indices52()** → `(nhexatc: u64, nhhexatc: u64, nvso: u64, edge_count: usize, node_count: usize)`

#### NHEXATC — S-Hexacosic 顶点幂次和

```
NHEXATC(G) = Σ_v S(v)^26
```

- S-Hexacosic 顶点和；顶点幂次序列延伸：
  `NM₁=ΣS²(topo18) → ... → NPENTTC=ΣS²⁵(topo51) → NHEXATC=ΣS²⁶(topo52)`
- S-正则图: `NHEXATC = n·S^26`
- 实现: `s^26 = s^16 × s^8 × s^2`（重复平方法）
- 溢出安全: u128 累加器 + saturating 操作，截断至 u64::MAX

#### NHHEXATC — S-Pentacosic 边幂次和

```
NHHEXATC(G) = Σ_{uv∈E} (S_u+S_v)^25
```

- S-Pentacosic 边和；边幂次序列延伸：
  `NHM1=Σ(S+S)²(topo23) → ... → NHPENTTC=Σ(S+S)²⁴(topo51) → NHHEXATC=Σ(S+S)²⁵(topo52)`
- S-正则图: `NHHEXATC = |E|·(2S)^25 = 33554432|E|·S^25`
- 实现: `ss^25 = ss^16 × ss^8 × ss`
- 溢出安全: u128 累加器 + saturating 操作

#### NVSO — S-Tetracontyl Sombor（α=40）

```
NVSO(G) = Σ_{uv∈E} (S_u²+S_v²)^20
```

- 广义 Sombor 指数 SO^α，α=40，S-变体，精确整数（无 isqrt）
- Sombor 字母序列: NSO(α=1)→NCSO(α=3)→NFSO(α=4)→NHSO(α=6)→NOSO(α=8)→
  NTSO(α=10)→NDSO(α=12)→NESO(α=14)→NGSO(α=16)→NIOSO(α=18)→NJSO(α=20)→
  NKSO(α=22)→NLSO(α=24)→NMSO(α=26)→NNSO(α=28)→NPSO(α=30)→NQSO(α=32)→
  NRSO(α=34)→NSSO(α=36)→NUSO(α=38)→**NVSO(α=40)**
- S-正则图: `NVSO = |E|·(2S²)^20 = 1048576|E|·S^40`
- 实现: `s2s^20 = s2s^16 × s2s^4`

---

## 测试向量（分析交叉验证）

| 图         | NHEXATC (精确)          | NHHEXATC (精确)              | NVSO (精确)                  | 边数 | 点数 |
|------------|------------------------|------------------------------|------------------------------|------|------|
| 空图       | 0                      | 0                            | 0                            | 0    | 0    |
| 单孤立点   | 0                      | 0                            | 0                            | 0    | 1    |
| K₂         | 2                      | 33_554_432                   | 1_048_576                    | 1    | 2    |
| P₃         | 201_326_592            | 2_251_799_813_685_248        | 2_305_843_009_213_693_952    | 2    | 3    |
| K₃         | 13_510_798_882_111_488 | u64::MAX(饱和)               | u64::MAX(饱和)               | 3    | 3    |
| K_{1,4}    | 22_517_998_136_852_480 | u64::MAX(饱和)               | u64::MAX(饱和)               | 4    | 5    |
| P₄         | 5_083_865_874_386      | u64::MAX(饱和)               | u64::MAX(饱和)               | 3    | 4    |
| K₄         | u64::MAX(饱和)         | u64::MAX(饱和)               | u64::MAX(饱和)               | 6    | 4    |
| 双孤立点   | 0                      | 0                            | 0                            | 0    | 2    |
| K_{2,3}    | u64::MAX(饱和)         | u64::MAX(饱和)               | u64::MAX(饱和)               | 6    | 5    |

### S-正则公式验证

- `NHEXATC  = n·S^26`                         ✓
- `NHHEXATC = |E|·(2S)^25 = 33554432|E|·S^25` ✓
- `NVSO     = |E|·(2S²)^20 = 1048576|E|·S^40` ✓

### 关键推导

**K₂ (S=1, 1条边, 2个点)**:
- NHEXATC:  1^26 + 1^26 = 2 ✓
- NHHEXATC: (1+1)^25 = 2^25 = 33_554_432 ✓
- NVSO:     (1²+1²)^20 = 2^20 = 1_048_576 ✓

**P₃ (S=2 均匀, 2条边, 3个点)**:
- NHEXATC:  3×2^26 = 3×67_108_864 = 201_326_592 ✓
- NHHEXATC: 2×4^25 = 2×2^50 = 2^51 = 2_251_799_813_685_248 ✓（适合 u64）
- NVSO:     2×8^20 = 2×2^60 = 2^61 = 2_305_843_009_213_693_952 ✓（适合 u64）

**K₃ (S=4 均匀, 3条边)**:
- NHEXATC:  3×4^26 = 3×2^52 = 13_510_798_882_111_488 ✓（适合 u64）
- NHHEXATC: 3×8^25 = 3×2^75 → 饱和 ✓
- NVSO:     3×32^20 = 3×2^100 → 饱和 ✓

**P₄ (S=2,3,3,2; 3条边)**:
- NHEXATC:  2×67_108_864 + 2×2_541_865_828_329 = 5_083_865_874_386 ✓
  （3^26 = 3^16×3^8×3^2 = 43_046_721×6_561×9 = 2_541_865_828_329）
- NHHEXATC: 5^25+6^25+5^25；5^25 >> u64::MAX 每边 → 饱和 ✓
- NVSO:     13^20 每边 >> u64::MAX → 饱和 ✓

---

## Shell 命令

```
graph topo52          / gtopo52
neighborhood hexacosic / gnhexatc
neighborhood pentacosic edge / gnhhexatc
neighborhood tetracontyl sombor / gnvso
gnhexatcnhhexatcnvso
```

## VectorAddress L4 命名空间（更新后）

`88=graph-topo` 至 `138=graph-topo51`, **`139=graph-topo52`**

- 插件: `TOPIX_52`
- 执行器: `t52.exec`

---

## 变更文件

| 文件 | 变更内容 |
|------|---------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices52_inner()` + 公共函数 `graph_topo_indices52()` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices52()` + 帮助文本更新 |
| `crates/k-shell/src/proc.rs` | 新增 topo52 路由 |
| `host-tests/gos-graph-topo52-harness/` | 新建独立 workspace（10 个测试，全部通过）|

---

## 测试结果

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
