# Hardening Log — V3.92 (2026-07-20)

## 变更摘要

新增 **topo81** 三项 Neighborhood S-variant 拓扑指数：
- **NPENTAPENTAACTC** — S-Pentapentacontic 顶点和 = Σ_v S(v)^55
- **NHPENTAPENTAACTC** — S-Tetrapentacontic 边和 = Σ_{uv∈E} (S_u+S_v)^54
- **NAXSO** — S-变体广义 Sombor 指数 α=98，即 Σ_{uv∈E} (S_u²+S_v²)^49

其中 S(v) = Σ_{w∈N(v)} deg(w) 为邻居度数和（S-variant 定义）。

## 技术细节

### 数学定义

| 指数 | 公式 | 说明 |
|------|------|------|
| NPENTAPENTAACTC | Σ_v S(v)^55 | pentacontic(50-59) 系列第 6 个；延续自 topo80 S^54 |
| NHPENTAPENTAACTC | Σ_{uv∈E} (S_u+S_v)^54 | 边和版本，指数为 54 |
| NAXSO | Σ_{uv∈E} (S_u²+S_v²)^49 | 3rd-pass 双字母 AX，α=98；延续自 NAWSO α=96 |

### 幂次分解（快速整数幂）

| 计算 | 分解 | 乘法次数 |
|------|------|---------|
| s^55 | s32 × s16 × s4 × s2 × s | 5 次 |
| ss^54 | ss32 × ss16 × ss4 × ss2 | 4 次 |
| s2s^49 | s2s32 × s2s16 × s2s | **3 次（高效！49=32+16+1）** |

s2s^49 效率较高：49=32+16+1，三项二次幂之和，仅需 3 次最终乘法。

### S-正则图公式

- NPENTAPENTAACTC = n·S^55（对 S-正则图）
- NHPENTAPENTAACTC = 18_014_398_509_481_984 · |E| · S^54（= 2^54·|E|·S^54）
- NAXSO = 562_949_953_421_312 · |E| · S^98（= 2^49·|E|·S^98）

### K₂ 精确值

| 指数 | K₂ 精确值 |
|------|-----------|
| NPENTAPENTAACTC | 2 |
| NHPENTAPENTAACTC | 18_014_398_509_481_984（= 2^54）|
| NAXSO | 562_949_953_421_312（= 2^49）|

### P₃ 精确值

| 指数 | P₃ 精确值 |
|------|-----------|
| NPENTAPENTAACTC | 108_086_391_056_891_904（= 3×2^55，未饱和）|
| NHPENTAPENTAACTC | u64::MAX（饱和）|
| NAXSO | u64::MAX（饱和）|

## 变更文件

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `crates/gos-runtime/src/lib.rs` | 新增 | `graph_topo_indices81()` 公开函数（inner 函数已预先存在）|
| `crates/k-shell/src/lib.rs` | 新增 | `dispatch_graph_topo_indices81` + 帮助文本 |
| `crates/k-shell/src/proc.rs` | 新增 | topo81 路由（"graph topo81"/"gtopo81" 等）|
| `host-tests/gos-graph-topo81-harness/` | 新增 | 10 项测试，全部通过 |

## VectorAddress 命名空间

- L4=168 用于 gos-graph-topo81-harness
- 插件：TOPIX_81，执行器：t81.exec

## Shell 命令别名

```
graph topo81
gtopo81
neighborhood pentapentacontic  →  NPENTAPENTAACTC
gnpentapentaactc
neighborhood tetrapentacontic edge  →  NHPENTAPENTAACTC
gnhpentapentaactc
neighborhood octanonacontyl sombor  →  NAXSO
gnnaxso
gnpentapentaactcnhpentapentaactcnaxso
```

## 测试结果

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

宿主测试总计：**1893 tests**（V3.92 新增 10 项）

## Commit

`feat(v3.92)`: NPENTAPENTAACTC + NHPENTAPENTAACTC + NAXSO Neighborhood S-variant indices + gos-graph-topo81-harness (10 tests)
