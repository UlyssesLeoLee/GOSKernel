# Hardening Log — V3.91 (2026-07-20)

## 变更摘要

新增 **topo80** 三项 Neighborhood S-variant 拓扑指数：
- **NTETRAPENTAACTC** — S-Tetrapentacontic 顶点和 = Σ_v S(v)^54
- **NHTETRAPENTAACTC** — S-Tripentacontic 边和 = Σ_{uv∈E} (S_u+S_v)^53
- **NAWSO** — S-变体广义 Sombor 指数 α=96，即 Σ_{uv∈E} (S_u²+S_v²)^48

其中 S(v) = Σ_{w∈N(v)} deg(w) 为邻居度数和（S-variant 定义）。

## 技术细节

### 数学定义

| 指数 | 公式 | 说明 |
|------|------|------|
| NTETRAPENTAACTC | Σ_v S(v)^54 | pentacontic(50-59) 系列第 5 个；延续自 topo79 S^53 |
| NHTETRAPENTAACTC | Σ_{uv∈E} (S_u+S_v)^53 | 边和版本，指数为 53 |
| NAWSO | Σ_{uv∈E} (S_u²+S_v²)^48 | 3rd-pass 双字母 AW，α=96；延续自 NAVSO α=94 |

### 幂次分解（快速整数幂）

| 计算 | 分解 | 乘法次数 |
|------|------|---------|
| s^54 | s32 × s16 × s4 × s2 | 4 次 |
| ss^53 | ss32 × ss16 × ss4 × ss | 4 次 |
| s2s^48 | s2s32 × s2s16 | **2 次（极高效！48=32+16）** |

s2s^48 的效率极高：48=32+16，两个 2 的幂次之和，仅需 1 次最终乘法。

### S-正则图公式

- NTETRAPENTAACTC = n·S^54（对 S-正则图）
- NHTETRAPENTAACTC = 9_007_199_254_740_992 · |E| · S^53（= 2^53·|E|·S^53）
- NAWSO = 281_474_976_710_656 · |E| · S^96（= 2^48·|E|·S^96）

### K₂ 精确值

| 指数 | K₂ 精确值 |
|------|-----------|
| NTETRAPENTAACTC | 2 |
| NHTETRAPENTAACTC | 9_007_199_254_740_992（= 2^53）|
| NAWSO | 281_474_976_710_656（= 2^48）|

### P₃ 精确值

| 指数 | P₃ 精确值 |
|------|-----------|
| NTETRAPENTAACTC | 54_043_195_528_445_952（= 3×2^54，未饱和）|
| NHTETRAPENTAACTC | u64::MAX（饱和）|
| NAWSO | u64::MAX（饱和）|

## 变更文件

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `crates/gos-runtime/src/lib.rs` | 新增 | `graph_topo_indices80_inner` + `graph_topo_indices80()` |
| `crates/k-shell/src/lib.rs` | 新增 | `dispatch_graph_topo_indices80` |
| `crates/k-shell/src/proc.rs` | 新增 | topo80 路由（"graph topo80"/"gtopo80" 等）|
| `host-tests/gos-graph-topo80-harness/` | 新增 | 10 项测试，全部通过 |

## VectorAddress 命名空间

- L4=167 用于 gos-graph-topo80-harness
- 插件：TOPIX_80，执行器：t80.exec

## Shell 命令别名

```
graph topo80
gtopo80
neighborhood tetrapentacontic  →  NTETRAPENTAACTC
gntetrapentaactc
neighborhood tripentacontic edge  →  NHTETRAPENTAACTC
gnhtetrapentaactc
neighborhood hexanonacontyl sombor  →  NAWSO
gnnawso
gntetrapentaactcnhtetrapentaactcnawso
```

## 测试结果

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

宿主测试总计：**1883 tests**（V3.91 新增 10 项）

## Commit

`157ee58` feat(v3.91): NTETRAPENTAACTC + NHTETRAPENTAACTC + NAWSO Neighborhood S-variant indices + gos-graph-topo80-harness (10 tests)
