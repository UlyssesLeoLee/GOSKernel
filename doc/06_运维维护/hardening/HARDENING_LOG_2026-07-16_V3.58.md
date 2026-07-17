# GOS 强化日志 V3.58 — NHENTC + NHHENTC + NPSO 邻域 S-变体拓扑指数

**日期**: 2026-07-16  
**版本**: V3.58  
**分支**: feat/vk-auto-live-surface  
**自动化任务**: 每2小时强化运行

---

## 一、本次强化内容

新增三个邻域 S-变体拓扑指数（S-variant topological indices），均基于 S(v) = Σ_{w∈N(v)} deg(w)（邻居度数和）。

### 新增指数

| 指数 | 定义 | 类型 | 数学描述 |
|------|------|------|---------|
| **NHENTC** | Σ_v S(v)^21 | 精确 u64 | S-二十一次幂顶点和（S-Heneicosic vertex sum） |
| **NHHENTC** | Σ_{uv∈E} (S_u+S_v)^20 | 精确 u64 | S-二十次幂边和（S-Eicosic edge-sum） |
| **NPSO** | Σ_{uv∈E} (S_u²+S_v²)^15 | 精确 u64 | S-三十次广义 Sombor α=30（S-Triacontyl Sombor） |

### 数学性质

- **NHENTC** 延续 S-幂次顶点序列：...→NEICTC=ΣS²⁰(topo46)→**NHENTC=ΣS²¹(topo47)**
- **NHHENTC** 延续 S-幂次边序列：...→NHEICTC=Σ(S+S)¹⁹(topo46)→**NHHENTC=Σ(S+S)²⁰(topo47)**
- **NPSO** 延续广义 Sombor SO^α 序列：...→NNSO(α=28,topo46)→**NPSO(α=30,topo47)**
- NPSO 为精确整数（不需要 isqrt）：(S_u²+S_v²)^15 无分数幂
- 命名注记：O 字母跳过（NOSO=α=8 已被 topo36 使用），改用 P → NPSO

### S-正则图公式验证

对 S-正则图（所有顶点 S 值相同）：
- NHENTC = n·S^21
- NHHENTC = |E|·(2S)^20 = 1,048,576·|E|·S^20
- NPSO = |E|·(2S²)^15 = 32,768·|E|·S^30

---

## 二、解析验证表

| 图 | NHENTC | NHHENTC | NPSO | 边数 | 节点数 |
|----|--------|---------|------|------|--------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| K₂ (S=1) | 2 | 1,048,576 | 32,768 | 1 | 2 |
| P₃ (S=2) | 6,291,456 | 2,199,023,255,552 | 70,368,744,177,664 | 2 | 3 |
| K₃ (S=4) | 13,194,139,533,312 | 3,458,764,513,820,540,928 | u64::MAX(饱和) | 3 | 3 |
| K_{1,4} (S=4) | 21,990,232,555,520 | 4,611,686,018,427,387,904 | u64::MAX(饱和) | 4 | 5 |
| P₄ (混合S) | 20,924,900,710 | 3,846,893,303,344,226 | 6,849,012,402,505,639,946 | 3 | 4 |
| K₄ (S=9) | u64::MAX(饱和) | u64::MAX(饱和) | u64::MAX(饱和) | 6 | 4 |
| 两孤立节点 | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} (S=6) | 109,684,753,201,889,280 | u64::MAX(饱和) | u64::MAX(饱和) | 6 | 5 |

### 关键推导（P₄验证）

P₄ 节点 S 值：S(A)=S(D)=2, S(B)=S(C)=3

- **NHENTC**: 2×2^21 + 2×3^21 = 4,194,304 + 20,920,706,406 = **20,924,900,710**
  - 3^21 = 3×3^20 = 3×3,486,784,401 = 10,460,353,203
- **NHHENTC**: 5^20 + 6^20 + 5^20 = 95,367,431,640,625 + 3,656,158,440,062,976 + 95,367,431,640,625 = **3,846,893,303,344,226**
- **NPSO**: 13^15 + 18^15 + 13^15 = 51,185,893,014,090,757 + 6,746,640,616,477,458,432 + 51,185,893,014,090,757 = **6,849,012,402,505,639,946**

### 溢出特性

- K₃/K_{1,4} (S=4): NPSO 每边 32^15 ≈ 3.74×10²² > u64::MAX → 饱和
- K₃: NHHENTC = 3×8^20 = 3,458,764,513,820,540,928 < u64::MAX → **精确** ✓
- K_{1,4}: NHHENTC = 4×8^20 = 4,611,686,018,427,387,904 < u64::MAX → **精确** ✓
- K₄ (S=9): NHENTC 4×9^21 >> u64::MAX → 饱和（全三指数饱和）
- K_{2,3} (S=6): NHENTC = 5×6^21 = 109,684,753,201,889,280 < u64::MAX → **精确**；NHHENTC、NPSO 饱和
- 注：K_{2,3} 的 NHENTC 在 topo47 中**首次不饱和**（同等 topo46 中 NEICTC 也不饱和）

---

## 三、实现细节

### 幂次计算方案

```rust
// NHENTC: S^21 = S^16 × S^4 × S
let s4  = s2 * s2;
let s8  = s4.saturating_mul(s4);
let s16 = s8.saturating_mul(s8);
let s21 = s16.saturating_mul(s4).saturating_mul(s);

// NHHENTC: (S_u+S_v)^20 = ss^16 × ss^4
let ss20 = ss16.saturating_mul(ss4);

// NPSO: (S_u²+S_v²)^15 = s2s^8 × s2s^4 × s2s^2 × s2s
let s2s15 = s2s8.saturating_mul(s2s4).saturating_mul(s2s2).saturating_mul(s2s);
```

### 算法复杂度

O(V+E)：度数遍历 → S(v) 计算 → 顶点扫描（NHENTC）+ 边扫描（NHHENTC, NPSO）

全程使用 u128 饱和累加器，最终截断到 u64::MAX，无 isqrt 运算。

---

## 四、修改的文件

| 文件 | 修改内容 |
|------|---------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices47_inner()` 方法 + `graph_topo_indices47()` 公共函数 |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices47()` 显示函数 |
| `crates/k-shell/src/proc.rs` | 新增 shell 命令路由（graph topo47/gtopo47/gnhentc/gnhhentc/gnpso 等） |
| `host-tests/gos-graph-topo47-harness/` | 新建测试套件（Cargo.toml + .cargo/config.toml + tests/graph_topo47.rs） |

---

## 五、Shell 命令

```
graph topo47  |  gtopo47
neighborhood heneicosic       |  gnhentc
neighborhood eicosic edge     |  gnhhentc
neighborhood triacontyl sombor|  gnpso
gnhentcnhhentcnpso
```

---

## 六、VectorAddress 命名空间更新

```
88=graph-topo ... 133=graph-topo46, 134=graph-topo47
```

插件：TOPIX_47 | 执行器：t47.exec | L4=134

---

## 七、测试结果

**新增测试**: 10 个（gos-graph-topo47-harness）  
**测试结果**: ✅ 10/10 通过  
**累计宿主测试总数**: 1553（原 1543 + 新增 10）

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## 八、S-variant 指数族系总览（截至 V3.58）

### S-幂次顶点和序列
NM₁(S²)→NF(S³)→NVQ(S⁴)→NPS(S⁵)→NSH(S⁶)→NSHP(S⁷)→NOC(S⁸)→NNC(S⁹)→NDC(S¹⁰)→NUC(S¹¹)→NDoC(S¹²)→NTC(S¹³)→NQTC(S¹⁴)→NPTC(S¹⁵)→NSTC(S¹⁶)→NHEPTC(S¹⁷)→NOCTC(S¹⁸)→NNONTC(S¹⁹)→NEICTC(S²⁰)→**NHENTC(S²¹)**

### S-广义 Sombor SO^α 序列
NSO(α=1)→NCSO(α=3)→NFSO(α=4)→NHSO(α=6)→NOSO(α=8)→NTSO(α=10)→NDSO(α=12)→NESO(α=14)→NGSO(α=16)→NIOSO(α=18)→NJSO(α=20)→NKSO(α=22)→NLSO(α=24)→NMSO(α=26)→NNSO(α=28)→**NPSO(α=30)**
