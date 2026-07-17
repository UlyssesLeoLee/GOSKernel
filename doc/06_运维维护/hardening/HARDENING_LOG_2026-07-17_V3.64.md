# 强化日志 V3.64 — NHEPTATC + NHHEPTATC + NXSO Neighborhood S-variant 拓扑指标

**日期**: 2026-07-17  
**版本**: V3.64  
**分支**: feat/vk-auto-live-surface  
**提交**: feat(v3.64): NHEPTATC + NHHEPTATC + NXSO Neighborhood S-variant indices + gos-graph-topo53-harness (10 tests)

---

## 变更摘要

新增三个 Neighborhood S-variant 拓扑指标（topo53 族），实现函数 `gos_runtime::graph_topo_indices53()`，并配套完整 host-test harness（10 个测试全部通过）和 k-shell 路由与展示。

---

## 新增拓扑指标

### NHEPTATC — S-Heptacosic 顶点幂次和

```
NHEPTATC(G) = Σ_v S(v)^27
```

- **中文名**: S-廿七次顶点和
- **定义**: 各节点邻域度数和 S(v) 的 27 次方之和
- **S-regular 公式**: NHEPTATC = n·S^27
- **实现**: `s^27 = s^16 × s^8 × s^2 × s`（u128 饱和乘法）
- **前序**: NHEXATC=Σ S²⁶ (topo52) → NHEPTATC=Σ S²⁷ (topo53)

### NHHEPTATC — S-Hexacosic 边幂次和

```
NHHEPTATC(G) = Σ_{uv∈E} (S_u + S_v)^26
```

- **中文名**: S-廿六次边和
- **定义**: 各无向边两端点 S 值之和的 26 次方之和
- **S-regular 公式**: NHHEPTATC = 67108864·|E|·S^26（= 2^26·|E|·S^26）
- **实现**: `ss^26 = ss^16 × ss^8 × ss^2`
- **前序**: NHHEXATC=Σ(S+S)²⁵ (topo52) → NHHEPTATC=Σ(S+S)²⁶ (topo53)

### NXSO — S-Dotetracontyl Sombor（α=42）

```
NXSO(G) = Σ_{uv∈E} (S_u² + S_v²)^21
```

- **中文名**: S-广义 Sombor 指标 α=42
- **定义**: 广义 Sombor SO^α 的 S-变体，α=42（指数 21 = α/2），精确整数无需 isqrt
- **S-regular 公式**: NXSO = 2097152·|E|·S^42（= 2^21·|E|·S^42）
- **实现**: `s2s^21 = s2s^16 × s2s^4 × s2s`
- **命名注**: W 已被 NWSO (S-Weighted Sombor, topo32) 占用，故跳至 X
- **前序**: NVSO(α=40,topo52) → NXSO(α=42,topo53)

---

## 关键测试值

| 图        | NHEPTATC (精确)            | NHHEPTATC (精确)              | NXSO (精确)         | 边数 | 节点数 |
|-----------|---------------------------|-------------------------------|---------------------|------|--------|
| 空图      | 0                         | 0                             | 0                   | 0    | 0      |
| 单节点    | 0                         | 0                             | 0                   | 0    | 1      |
| K₂        | 2                         | 67_108_864                    | 2_097_152           | 1    | 2      |
| P₃        | 402_653_184               | 9_007_199_254_740_992         | u64::MAX（溢出）    | 2    | 3      |
| K₃        | 54_043_195_528_445_952    | u64::MAX（溢出）              | u64::MAX（溢出）    | 3    | 3      |
| K_{1,4}   | 90_071_992_547_409_920    | u64::MAX（溢出）              | u64::MAX（溢出）    | 4    | 5      |
| P₄        | 15_251_463_405_430        | u64::MAX（溢出）              | u64::MAX（溢出）    | 3    | 4      |
| K₄        | u64::MAX（溢出）          | u64::MAX（溢出）              | u64::MAX（溢出）    | 6    | 4      |
| K_{2,3}   | u64::MAX（溢出）          | u64::MAX（溢出）              | u64::MAX（溢出）    | 6    | 5      |

**P₃ NXSO 溢出说明**: 每边 (4+4)^21 = 8^21 = 2^63 = 9_223_372_036_854_775_808，单边适配 u64，但两边之和 2×2^63 = 2^64 > u64::MAX → 饱和。

---

## 解析推导

**K₂ (S=1)**:
- NHEPTATC: 1^27 + 1^27 = 2 ✓
- NHHEPTATC: (1+1)^26 = 2^26 = 67_108_864 ✓
- NXSO: (1+1)^21 = 2^21 = 2_097_152 ✓

**P₃ (S=2 均匀)**:
- NHEPTATC: 3×2^27 = 3×134_217_728 = 402_653_184 ✓
- NHHEPTATC: 2×4^26 = 2×4_503_599_627_370_496 = 9_007_199_254_740_992 ✓
- NXSO: 2×8^21 = 2×2^63 = 2^64 → 饱和 ✓

**K₃ (S=4 均匀)**:
- NHEPTATC: 3×4^27 = 3×2^54 = 54_043_195_528_445_952（适配 u64）✓
- NHHEPTATC: 3×8^26 = 3×2^78 → 单边饱和 ✓
- NXSO: 3×32^21 = 3×2^105 → 饱和 ✓

**K_{1,4} (S=4 均匀)**:
- NHEPTATC: 5×4^27 = 5×2^54 = 90_071_992_547_409_920（适配 u64）✓

**P₄ (S(A)=S(D)=2, S(B)=S(C)=3)**:
- NHEPTATC: 2×2^27 + 2×3^27 = 268_435_456 + 2×7_625_597_484_987 = 15_251_463_405_430 ✓
  （3^27 = 3^16×3^8×3^3 = 43_046_721×6_561×27 = 7_625_597_484_987）

---

## VectorAddress 命名空间

- L4=140: gos-graph-topo53-harness（新增）
- L4 namespace 累计: 88=graph-topo 至 **140=graph-topo53**

---

## k-shell 路由

```
graph topo53 / gtopo53
neighborhood heptacosic / gnheptatc
neighborhood hexacosic edge / gnhheptatc
neighborhood dotetracontyl sombor / gnxso
gnheptatcnhheptatcnxso
```

---

## 测试结果

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**host-test 累计**: 1613 tests（V3.64 新增 10，前累计 1603 from V3.63）

---

## 文件变更列表

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `crates/gos-runtime/src/lib.rs` | 修改 | 新增 `graph_topo_indices53_inner()` + `graph_topo_indices53()` |
| `crates/k-shell/src/lib.rs` | 修改 | 新增 `dispatch_graph_topo_indices53()`，更新 topo52/topo50 头部 |
| `crates/k-shell/src/proc.rs` | 修改 | 新增 topo53 路由条目 |
| `host-tests/gos-graph-topo53-harness/Cargo.toml` | 新建 | harness 包配置 |
| `host-tests/gos-graph-topo53-harness/.cargo/config.toml` | 新建 | host 目标覆盖 |
| `host-tests/gos-graph-topo53-harness/tests/graph_topo53.rs` | 新建 | 10 个测试用例 |

---

## S-variant 拓扑指标家族完整序列（截至 V3.64）

### 顶点幂次和系列
NM₁(S²,t18) → NF(S³,t22) → NVQ(S⁴,t30) → NPS(S⁵,t31) → NSH(S⁶,t32) → NSHP(S⁷,t33) → NOC(S⁸,t34) → NNC(S⁹,t35) → NDC(S¹⁰,t36) → NUC(S¹¹,t37) → NDoC(S¹²,t38) → NTC(S¹³,t39) → NQTC(S¹⁴,t40) → NPTC(S¹⁵,t41) → NSTC(S¹⁶,t42) → NHEPTC(S¹⁷,t43) → NOCTC(S¹⁸,t44) → NNONTC(S¹⁹,t45) → NEICTC(S²⁰,t46) → NHENTC(S²¹,t47) → NDOCTC(S²²,t48) → NTRICTC(S²³,t49) → NTETRTC(S²⁴,t50) → NPENTTC(S²⁵,t51) → NHEXATC(S²⁶,t52) → **NHEPTATC(S²⁷,t53)**

### Sombor 广义系列（α=2k）
NSO(α=1,t21) → NCSO(α=3,t33) → NFSO(α=4,t34) → NHSO(α=6,t35) → NOSO(α=8,t36) → NTSO(α=10,t37) → NDSO(α=12,t38) → NESO(α=14,t39) → NGSO(α=16,t40) → NIOSO(α=18,t41) → NJSO(α=20,t42) → NKSO(α=22,t43) → NLSO(α=24,t44) → NMSO(α=26,t45) → NNSO(α=28,t46) → NPSO(α=30,t47) → NQSO(α=32,t48) → NRSO(α=34,t49) → NSSO(α=36,t50) → NUSO(α=38,t51) → NVSO(α=40,t52) → **NXSO(α=42,t53)**
