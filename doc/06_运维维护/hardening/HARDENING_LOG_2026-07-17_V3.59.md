# GOS 强化日志 V3.59 — NDOCTC + NHDOCTC + NQSO 邻域 S-变体拓扑指数

**日期**: 2026-07-17  
**版本**: V3.59  
**分支**: feat/vk-auto-live-surface  
**自动化任务**: 每2小时强化运行

---

## 一、本次强化内容

新增三个邻域 S-变体拓扑指数（S-variant topological indices），均基于 S(v) = Σ_{w∈N(v)} deg(w)（邻居度数和）。

### 新增指数

| 指数 | 定义 | 类型 | 数学描述 |
|------|------|------|---------|
| **NDOCTC** | Σ_v S(v)^22 | 精确 u64 | S-二十二次幂顶点和（S-Docosic vertex sum） |
| **NHDOCTC** | Σ_{uv∈E} (S_u+S_v)^21 | 精确 u64 | S-二十一次幂边和（S-Heneicosic edge-sum） |
| **NQSO** | Σ_{uv∈E} (S_u²+S_v²)^16 | 精确 u64 | S-三十二次广义 Sombor α=32（S-Dotriacontyl Sombor） |

### 数学性质

- **NDOCTC** 延续 S-幂次顶点序列：...→NHENTC=ΣS²¹(topo47)→**NDOCTC=ΣS²²(topo48)**
- **NHDOCTC** 延续 S-幂次边序列：...→NHHENTC=Σ(S+S)²⁰(topo47)→**NHDOCTC=Σ(S+S)²¹(topo48)**
- **NQSO** 延续广义 Sombor SO^α 序列：...→NPSO(α=30,topo47)→**NQSO(α=32,topo48)**
- NQSO 为精确整数（不需要 isqrt）：(S_u²+S_v²)^16 无分数幂
- 命名注记：Q 接续 P（P=α=30 已被 topo47 使用）→ NQSO

### S-正则图公式验证

对 S-正则图（所有顶点 S 值相同）：
- NDOCTC = n·S^22
- NHDOCTC = |E|·(2S)^21 = 2,097,152·|E|·S^21
- NQSO = |E|·(2S²)^16 = 65,536·|E|·S^32

---

## 二、解析验证表

| 图 | NDOCTC | NHDOCTC | NQSO | 边数 | 节点数 |
|----|--------|---------|------|------|--------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| K₂ (S=1) | 2 | 2,097,152 | 65,536 | 1 | 2 |
| P₃ (S=2) | 12,582,912 | 8,796,093,022,208 | 562,949,953,421,312 | 2 | 3 |
| K₃ (S=4) | 52,776,558,133,248 | u64::MAX(饱和) | u64::MAX(饱和) | 3 | 3 |
| K_{1,4} (S=4) | 87,960,930,222,080 | u64::MAX(饱和) | u64::MAX(饱和) | 4 | 5 |
| P₄ (混合S) | 62,770,507,826 | 22,890,624,956,784,106 | u64::MAX(饱和) | 3 | 4 |
| K₄ (S=9) | u64::MAX(饱和) | u64::MAX(饱和) | u64::MAX(饱和) | 6 | 4 |
| 两孤立节点 | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} (S=6) | 658,108,519,211,335,680 | u64::MAX(饱和) | u64::MAX(饱和) | 6 | 5 |

### 关键推导

**K₂ (S=1)**:
- NDOCTC: 1^22 + 1^22 = 2 ✓
- NHDOCTC: (1+1)^21 = 2^21 = 2,097,152 ✓
- NQSO: (1²+1²)^16 = 2^16 = 65,536 ✓

**P₃ (S=2)**:
- NDOCTC: 3×2^22 = 3×4,194,304 = 12,582,912 ✓
- NHDOCTC: 2×(2+2)^21 = 2×4^21 = 2×4,398,046,511,104 = 8,796,093,022,208 ✓
- NQSO: 2×(4+4)^16 = 2×8^16 = 2×281,474,976,710,656 = 562,949,953,421,312 ✓

**P₄ (S(A)=S(D)=2, S(B)=S(C)=3)**:
- NDOCTC: 2×2^22 + 2×3^22 = 8,388,608 + 62,762,119,218 = 62,770,507,826
  - 3^22 = 3×3^21 = 3×10,460,353,203 = 31,381,059,609
- NHDOCTC: 5^21 + 6^21 + 5^21
  - 5^21 = 5×5^20 = 5×95,367,431,640,625 = 476,837,158,203,125
  - 6^21 = 6×6^20 = 6×3,656,158,440,062,976 = 21,936,950,640,377,856
  - 合计: 953,674,316,406,250 + 21,936,950,640,377,856 = 22,890,624,956,784,106 < u64::MAX ✓
- NQSO: 13^16 + 18^16 + 13^16 → 18^16 ≈ 1.21×10²⁰ >> u64::MAX → 饱和 ✓

**K_{2,3} (S=6)**:
- NDOCTC: 5×6^22 = 5×131,621,703,842,267,136 = 658,108,519,211,335,680 < u64::MAX → **精确** ✓
- NHDOCTC: 6×12^21 >> u64::MAX → 饱和 ✓
- NQSO: 6×72^16 >> u64::MAX → 饱和 ✓

### 溢出特性

- K₃/K_{1,4} (S=4): NHDOCTC 每边 8^21 = 9.22×10¹⁸ > u64::MAX → 饱和
- K₃/K_{1,4} (S=4): NQSO 每边 32^16 = 2^80 >> u64::MAX → 饱和
- K₄ (S=9): 全三指数均饱和
- K_{2,3} (S=6): NDOCTC = 658,108,519,211,335,680 < u64::MAX → **精确**

---

## 三、实现细节

### 幂次计算方案

```rust
// NDOCTC: S^22 = S^16 × S^4 × S^2
let s2  = s * s;
let s4  = s2 * s2;
let s8  = s4.saturating_mul(s4);
let s16 = s8.saturating_mul(s8);
let s22 = s16.saturating_mul(s4).saturating_mul(s2);

// NHDOCTC: (S_u+S_v)^21 = ss^16 × ss^4 × ss
let ss21 = ss16.saturating_mul(ss4).saturating_mul(ss);

// NQSO: (S_u²+S_v²)^16 = s2s^8 × s2s^8
let s2s8  = s2s4.saturating_mul(s2s4);
let s2s16 = s2s8.saturating_mul(s2s8);
```

### 算法复杂度

O(V+E)：度数遍历 → S(v) 计算 → 顶点扫描（NDOCTC）+ 边扫描（NHDOCTC, NQSO）

全程使用 u128 饱和累加器，最终截断到 u64::MAX，无 isqrt 运算。

---

## 四、修改的文件

| 文件 | 修改内容 |
|------|---------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices48_inner()` 方法 + `graph_topo_indices48()` 公共函数 |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices48()` 显示函数 |
| `crates/k-shell/src/proc.rs` | 新增 shell 命令路由（graph topo48/gtopo48/gndoctc/gnhdoctc/gnqso 等） |
| `host-tests/gos-graph-topo48-harness/` | 新建测试套件（Cargo.toml + .cargo/config.toml + tests/graph_topo48.rs） |

---

## 五、Shell 命令

```
graph topo48   |  gtopo48
neighborhood docosic             |  gndoctc
neighborhood heneicosic edge     |  gnhdoctc
neighborhood dotriacontyl sombor |  gnqso
gndoctcnhdoctcnqso
```

---

## 六、VectorAddress 命名空间更新

```
88=graph-topo ... 134=graph-topo47, 135=graph-topo48
```

插件：TOPIX_48 | 执行器：t48.exec | L4=135

---

## 七、测试结果

**新增测试**: 10 个（gos-graph-topo48-harness）  
**测试结果**: ✅ 10/10 通过  
**累计宿主测试总数**: 1563（原 1553 + 新增 10）

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## 八、S-variant 指数族系总览（截至 V3.59）

### S-幂次顶点和序列
NM₁(S²)→NF(S³)→NVQ(S⁴)→NPS(S⁵)→NSH(S⁶)→NSHP(S⁷)→NOC(S⁸)→NNC(S⁹)→NDC(S¹⁰)→NUC(S¹¹)→NDoC(S¹²)→NTC(S¹³)→NQTC(S¹⁴)→NPTC(S¹⁵)→NSTC(S¹⁶)→NHEPTC(S¹⁷)→NOCTC(S¹⁸)→NNONTC(S¹⁹)→NEICTC(S²⁰)→NHENTC(S²¹)→**NDOCTC(S²²)**

### S-广义 Sombor SO^α 序列
NSO(α=1)→NCSO(α=3)→NFSO(α=4)→NHSO(α=6)→NOSO(α=8)→NTSO(α=10)→NDSO(α=12)→NESO(α=14)→NGSO(α=16)→NIOSO(α=18)→NJSO(α=20)→NKSO(α=22)→NLSO(α=24)→NMSO(α=26)→NNSO(α=28)→NPSO(α=30)→**NQSO(α=32)**
