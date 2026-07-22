# 强化日志 — V3.99（2026-07-21）

## 摘要

V3.99 新增 **NHEXADYACTC + NHHEXADYACTC + NBESO** 三项 Neighborhood S-variant 拓扑指数
（topo88），为 hexacontic（60–69）系列第3个。新增 10 项宿主测试 → 累计 **1963 项**。

## 变更内容

### 新增运行时函数

`gos_runtime::graph_topo_indices88() -> (nhexadyactc: u64, nhhexadyactc: u64, nbeso: u64, edge_count: usize, node_count: usize)`

- **NHEXADYACTC(G)** = Σ_v S(v)^62 —— S-第62次幂顶点和（u128→u64，精确）
- **NHHEXADYACTC(G)** = Σ_{uv∈E} (S_u+S_v)^61 —— S-第61次幂边和（u128→u64，精确）
- **NBESO(G)** = Σ_{uv∈E} (S_u²+S_v²)^56 —— S-变体 Sombor 指数 α=112（u128→u64，精确，无需 isqrt）

其中 S(v) = Σ_{w∈N(v)} deg(w) 为邻域度数和。

### 系列定位

- NHEXADYACTC 将 NHEXAENACTC=Σ S^61（topo87）延伸至第62次幂；**hexacontic（60-69）系列第3个**
- NHHEXADYACTC 将 NHHEXAENACTC=Σ(S+S)^60（topo87）延伸至第61次幂
- NBESO = S-变体广义 Sombor 指数 SO^α，α=112：NBDSO(α=110,topo87)→NBESO(α=112,topo88)；**NB 系列第5个（字母 E）**

### 实现细节

幂次链（二进制分解）：
- s^62 = s32 × s16 × s8 × s4 × s2（62=32+16+8+4+2；5 次乘法）
- ss^61 = ss32 × ss16 × ss8 × ss4 × ss（61=32+16+8+4+1；5 次乘法）
- s2s^56 = s2s32 × s2s16 × s2s8（56=32+16+8；**3 次乘法 —— 效率高！**三者均为2的幂次）

### 解析测试值

| 图 | NHEXADYACTC | NHHEXADYACTC | NBESO |
|-----------|---------------------------------|------------------------------|---------------------------|
| 空图 | 0 | 0 | 0 |
| K₂ | 2 | 2_305_843_009_213_693_952 | 72_057_594_037_927_936 |
| P₃ | 13_835_058_055_282_163_712 | u64::MAX（饱和）| u64::MAX（饱和）|
| K₃ 及以上 | u64::MAX（饱和）| u64::MAX（饱和）| u64::MAX（饱和）|

S-正则图公式：
- NHEXADYACTC = n·S^62
- NHHEXADYACTC = |E|·(2S)^61 = 2305843009213693952·|E|·S^61
- NBESO = |E|·(2S²)^56 = 72057594037927936·|E|·S^112

### VectorAddress

L4=175 分配给 gos-graph-topo88-harness；插件 `TOPIX_88`；执行器 `t88.exec`

### Shell 命令

```
graph topo88  /  gtopo88  /  neighborhood hexadycontic  /  gnhexadyactc
neighborhood hexaencontic edge  /  gnnhhexadyactc
neighborhood dohectyl sombor be  /  gnnbeso
gnhexadyactnhhexadyactnbeso
```

## 测试覆盖

**gos-graph-topo88-harness**（10 项测试）：
1. 空图 → (0, 0, 0, 0, 0)
2. 单个孤立节点 → (0, 0, 0, 0, 1)
3. K₂ 单边 → (2, 2_305_843_009_213_693_952, 72_057_594_037_927_936, 1, 2)
4. 路径 P₃ → (13_835_058_055_282_163_712, u64::MAX, u64::MAX, 2, 3)
5. 三角形 K₃ → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
6. 星图 K_{1,4} → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
7. 路径 P₄ → (u64::MAX, u64::MAX, u64::MAX, 3, 4)
8. 完全图 K₄ → (u64::MAX, u64::MAX, u64::MAX, 6, 4)
9. 两个孤立节点 → (0, 0, 0, 0, 2)
10. K_{2,3} 二部图 → (u64::MAX, u64::MAX, u64::MAX, 6, 5)

## 累计状态

- 宿主测试套件总数：**1963 项**（全部通过）
- 此前：至 V3.98 累计 1953 项
- gos-graph-topo88-harness：新增 10 项（V3.99）
- VectorAddress L4 命名空间：88=graph-topo 至 174=graph-topo87，**175=graph-topo88**
- NB 系列：NBASO(α=104)→NBBSO(α=106)→NBCSO(α=108)→NBDSO(α=110)→**NBESO(α=112)**（字母 E，第5个）
- hexacontic 系列：NHEXAACTC(S^60,topo86)→NHEXAENACTC(S^61,topo87)→**NHEXADYACTC(S^62,topo88)**（10个中的第3个）
