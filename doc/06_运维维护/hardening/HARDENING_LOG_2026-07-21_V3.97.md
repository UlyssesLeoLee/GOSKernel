# 强化日志 — V3.97（2026-07-21）

## 摘要

V3.97 新增 **NHEXAACTC + NHHEXAACTC + NBCSO** 三项 Neighborhood S-variant 拓扑指数
（topo86），开启 hexacontic（60–69）系列。新增 10 项宿主测试 → 累计 **1943 项**。

## 变更内容

### 新增运行时函数

`gos_runtime::graph_topo_indices86() -> (nhexaactc: u64, nhhexaactc: u64, nbcso: u64, edge_count: usize, node_count: usize)`

- **NHEXAACTC(G)** = Σ_v S(v)^60 —— S-第60次幂顶点和（u128→u64，精确）
- **NHHEXAACTC(G)** = Σ_{uv∈E} (S_u+S_v)^59 —— S-第59次幂边和（u128→u64，精确）
- **NBCSO(G)** = Σ_{uv∈E} (S_u²+S_v²)^54 —— S-变体 Sombor 指数 α=108（u128→u64，精确，无需 isqrt）

其中 S(v) = Σ_{w∈N(v)} deg(w) 为邻域度数和。

### 系列定位

- NHEXAACTC 将 NNONAPENTAACTC=Σ S^59（topo85）延伸至第60次幂；**hexacontic（60-69）系列首个**
- NHHEXAACTC 将 NHNONAPENTAACTC=Σ(S+S)^58（topo85）延伸至第59次幂
- NBCSO = S-变体广义 Sombor 指数 SO^α，α=108：NBBSO(α=106,topo85)→NBCSO(α=108,topo86)；**NB 系列第3个（字母 C）**

### 实现细节

幂次链（二进制分解）：
- s^60 = s32 × s16 × s8 × s4（60=32+16+8+4；**4 次乘法 —— 效率高！**四者均为2的幂次）
- ss^59 = ss32 × ss16 × ss8 × ss2 × ss（59=32+16+8+2+1；5 次乘法）
- s2s^54 = s2s32 × s2s16 × s2s4 × s2s2（54=32+16+4+2；4 次乘法）

### 解析测试值

| 图 | NHEXAACTC | NHHEXAACTC | NBCSO |
|-----------|--------------------------------|------------------------------|--------------------------|
| 空图 | 0 | 0 | 0 |
| K₂ | 2 | 576_460_752_303_423_488 | 18_014_398_509_481_984 |
| P₃ | 3_458_764_513_820_540_928 | u64::MAX（饱和）| u64::MAX（饱和）|
| K₃ 及以上 | u64::MAX（饱和）| u64::MAX（饱和）| u64::MAX（饱和）|

S-正则图公式：
- NHEXAACTC = n·S^60
- NHHEXAACTC = |E|·(2S)^59 = 576460752303423488·|E|·S^59
- NBCSO = |E|·(2S²)^54 = 18014398509481984·|E|·S^108

### VectorAddress

L4=173 分配给 gos-graph-topo86-harness；插件 `TOPIX_86`；执行器 `t86.exec`

### Shell 命令

`graph topo86` / `gtopo86` / `gnhexaactc` / `gnnhhexaactc` / `gnnbcso` / `gnhexaactcnhhexaactcnbcso`

## 变更文件

- `crates/gos-runtime/src/lib.rs` —— 新增 `graph_topo_indices86_inner()` + `graph_topo_indices86()`
- `crates/k-shell/src/lib.rs` —— 新增 `dispatch_graph_topo_indices86()`
- `crates/k-shell/src/proc.rs` —— 新增 `graph topo86` 路由
- `host-tests/gos-graph-topo86-harness/` —— 新建独立 harness（10 项测试，全部通过）

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

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**宿主测试套件总数：1943**（此前 1933 项 + 本次新增 10 项）
