# 强化日志 — V3.98（2026-07-21）

## 摘要

V3.98 新增 **NHEXAENACTC + NHHEXAENACTC + NBDSO** 三项 Neighborhood S-variant 拓扑指数
（topo87），为 hexacontic（60–69）系列第2个。新增 10 项宿主测试 → 累计 **1953 项**。

## 变更内容

### 新增运行时函数

`gos_runtime::graph_topo_indices87() -> (nhexaenactc: u64, nhhexaenactc: u64, nbdso: u64, edge_count: usize, node_count: usize)`

- **NHEXAENACTC(G)** = Σ_v S(v)^61 —— S-第61次幂顶点和（u128→u64，精确）
- **NHHEXAENACTC(G)** = Σ_{uv∈E} (S_u+S_v)^60 —— S-第60次幂边和（u128→u64，精确）
- **NBDSO(G)** = Σ_{uv∈E} (S_u²+S_v²)^55 —— S-变体 Sombor 指数 α=110（u128→u64，精确，无需 isqrt）

其中 S(v) = Σ_{w∈N(v)} deg(w) 为邻域度数和。

### 系列定位

- NHEXAENACTC 将 NHEXAACTC=Σ S^60（topo86）延伸至第61次幂；**hexacontic（60-69）系列第2个**
- NHHEXAENACTC 将 NHHEXAACTC=Σ(S+S)^59（topo86）延伸至第60次幂
- NBDSO = S-变体广义 Sombor 指数 SO^α，α=110：NBCSO(α=108,topo86)→NBDSO(α=110,topo87)；**NB 系列第4个（字母 D）**

### 实现细节

幂次链（二进制分解）：
- s^61 = s32 × s16 × s8 × s4 × s（61=32+16+8+4+1；5 次乘法）
- ss^60 = ss32 × ss16 × ss8 × ss4（60=32+16+8+4；**4 次乘法 —— 效率高！**四者均为2的幂次）
- s2s^55 = s2s32 × s2s16 × s2s4 × s2s2 × s2s（55=32+16+4+2+1；5 次乘法）

### 解析测试值

| 图 | NHEXAENACTC | NHHEXAENACTC | NBDSO |
|-----------|--------------------------------|------------------------------|--------------------------|
| 空图 | 0 | 0 | 0 |
| K₂ | 2 | 1_152_921_504_606_846_976 | 36_028_797_018_963_968 |
| P₃ | 6_917_529_027_641_081_856 | u64::MAX（饱和）| u64::MAX（饱和）|
| K₃ 及以上 | u64::MAX（饱和）| u64::MAX（饱和）| u64::MAX（饱和）|

S-正则图公式：
- NHEXAENACTC = n·S^61
- NHHEXAENACTC = |E|·(2S)^60 = 1152921504606846976·|E|·S^60
- NBDSO = |E|·(2S²)^55 = 36028797018963968·|E|·S^110

### VectorAddress

L4=174 分配给 gos-graph-topo87-harness；插件 `TOPIX_87`；执行器 `t87.exec`

### Shell 命令

```
graph topo87  /  gtopo87  /  neighborhood hexaencontic  /  gnhexaenactc
neighborhood hexacontic edge  /  gnnhhexaenactc
neighborhood dohectyl sombor bd  /  gnnbdso
gnhexaenactcnhhexaenactcnbdso
```

## 测试覆盖

**gos-graph-topo87-harness**（10 项测试）：
1. 空图 → (0, 0, 0, 0, 0)
2. 单个孤立节点 → (0, 0, 0, 0, 1)
3. K₂ 单边 → (2, 1_152_921_504_606_846_976, 36_028_797_018_963_968, 1, 2)
4. 路径 P₃ → (6_917_529_027_641_081_856, u64::MAX, u64::MAX, 2, 3)
5. 三角形 K₃ → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
6. 星图 K_{1,4} → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
7. 路径 P₄ → (u64::MAX, u64::MAX, u64::MAX, 3, 4)
8. 完全图 K₄ → (u64::MAX, u64::MAX, u64::MAX, 6, 4)
9. 两个孤立节点 → (0, 0, 0, 0, 2)
10. K_{2,3} 二部图 → (u64::MAX, u64::MAX, u64::MAX, 6, 5)

## 累计状态

- 宿主测试套件总数：**1953 项**（全部通过）
- 此前：至 V3.97 累计 1943 项
- gos-graph-topo87-harness：新增 10 项（V3.98）
- VectorAddress L4 命名空间：88=graph-topo 至 173=graph-topo86，**174=graph-topo87**
- NB 系列：NBASO(α=104)→NBBSO(α=106)→NBCSO(α=108)→**NBDSO(α=110)**（字母 D，第4个）
- hexacontic 系列：NHEXAACTC(S^60,topo86) → **NHEXAENACTC(S^61,topo87)**（10个中的第2个）
