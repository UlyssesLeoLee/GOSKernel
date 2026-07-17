# GOS 强化日志 V3.57 — NEICTC + NHEICTC + NNSO 邻域 S-变体拓扑指数

**日期**: 2026-07-16  
**版本**: V3.57  
**分支**: feat/vk-auto-live-surface  
**自动化任务**: 每2小时强化运行

---

## 一、本次强化内容

新增三个邻域 S-变体拓扑指数（S-variant topological indices），均基于 S(v) = Σ_{w∈N(v)} deg(w)（邻居度数和）。

### 新增指数

| 指数 | 定义 | 类型 | 数学描述 |
|------|------|------|---------|
| **NEICTC** | Σ_v S(v)^20 | 精确 u64 | S-二十次幂顶点和（S-Eicosic vertex sum） |
| **NHEICTC** | Σ_{uv∈E} (S_u+S_v)^19 | 精确 u64 | S-十九次幂边和（S-Nonadecic edge-sum） |
| **NNSO** | Σ_{uv∈E} (S_u²+S_v²)^14 | 精确 u64 | S-二十八次广义 Sombor α=28（S-Octacosic Sombor） |

### 数学性质

- **NEICTC** 延续 S-幂次顶点序列：NM₁=ΣS²(topo18) → ... → NNONTC=ΣS¹⁹(topo45) → **NEICTC=ΣS²⁰(topo46)**
- **NHEICTC** 延续 S-幂次边序列：NHM1=Σ(S+S)²(topo23) → ... → NHNONTC=Σ(S+S)¹⁸(topo45) → **NHEICTC=Σ(S+S)¹⁹(topo46)**
- **NNSO** 延续广义 Sombor SO^α 序列：NSO(α=1) → NCSO(α=3) → ... → NMSO(α=26,topo45) → **NNSO(α=28,topo46)**
- NNSO 为精确整数（不需要 isqrt）：(S_u²+S_v²)^14 无分数幂

### S-正则图公式验证

对 S-正则图（所有顶点 S 值相同）：
- NEICTC = n·S^20
- NHEICTC = |E|·(2S)^19 = 524288·|E|·S^19
- NNSO = |E|·(2S²)^14 = 16384·|E|·S^28

---

## 二、解析验证表

| 图 | NEICTC | NHEICTC | NNSO | 边数 | 节点数 |
|----|--------|---------|------|------|--------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| K₂ (S=1) | 2 | 524,288 | 16,384 | 1 | 2 |
| P₃ (S=2) | 3,145,728 | 549,755,813,888 | 8,796,093,022,208 | 2 | 3 |
| K₃ (S=4) | 3,298,534,883,328 | 432,345,564,227,567,616 | u64::MAX(饱和) | 3 | 3 |
| K_{1,4} (S=4) | 5,497,558,138,880 | 576,460,752,303,423,488 | u64::MAX(饱和) | 4 | 5 |
| P₄ (混合S) | 6,975,665,954 | 647,506,712,666,746 | 382,688,120,353,479,602 | 3 | 4 |
| K₄ (S=9) | u64::MAX(饱和) | u64::MAX(饱和) | u64::MAX(饱和) | 6 | 4 |
| 两孤立节点 | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} (S=6) | 18,280,792,200,314,880 | u64::MAX(饱和) | u64::MAX(饱和) | 6 | 5 |

### 关键推导（P₄验证）

P₄ 节点 S 值：S(A)=S(D)=2, S(B)=S(C)=3

- **NEICTC**: 2^20 + 3^20 + 3^20 + 2^20 = 1,048,576 + 3,486,784,401 + 3,486,784,401 + 1,048,576 = **6,975,665,954**
- **NHEICTC**: 5^19 + 6^19 + 5^19 = 19,073,486,328,125 + 609,359,740,010,496 + 19,073,486,328,125 = **647,506,712,666,746**
- **NNSO**: 13^14 + 18^14 + 13^14 = 3,937,376,385,699,289 + 374,813,367,582,081,024 + 3,937,376,385,699,289 = **382,688,120,353,479,602**

### 溢出特性

- K₃/K_{1,4} (S=4): NNSO 每边 32^14 ≈ 1.18×10²¹ > u64::MAX → 饱和
- K₄ (S=9): NEICTC 4×9^20 ≈ 4.86×10²⁰ > u64::MAX → 饱和；NHEICTC、NNSO 亦饱和
- K_{2,3} (S=6): NHEICTC、NNSO 饱和；NEICTC 精确适入 u64

---

## 三、实现细节

### 幂次计算方案

```rust
// NEICTC: S^20 = S^16 × S^4
let s4  = s2 * s2;
let s8  = s4.saturating_mul(s4);
let s16 = s8.saturating_mul(s8);
let s20 = s16.saturating_mul(s4);

// NHEICTC: (S_u+S_v)^19 = ss^16 × ss^2 × ss
let ss19 = ss16.saturating_mul(ss2).saturating_mul(ss);

// NNSO: (S_u²+S_v²)^14 = s2s^8 × s2s^4 × s2s^2
let s2s14 = s2s8.saturating_mul(s2s4).saturating_mul(s2s2);
```

### 算法复杂度

O(V+E)：度数遍历 → S(v) 计算 → 顶点扫描（NEICTC）+ 边扫描（NHEICTC, NNSO）

全程使用 u128 饱和累加器，最终截断到 u64::MAX，无 isqrt 运算。

---

## 四、修改的文件

| 文件 | 修改内容 |
|------|---------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices46_inner()` 方法 + `graph_topo_indices46()` 公共函数 |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices46()` 显示函数 |
| `crates/k-shell/src/proc.rs` | 新增 shell 命令路由（graph topo46/gtopo46/gneictc/gnheictc/gnnso 等） |
| `host-tests/gos-graph-topo46-harness/` | 新建测试套件（Cargo.toml + .cargo/config.toml + tests/graph_topo46.rs） |

---

## 五、Shell 命令

```
graph topo46  |  gtopo46
neighborhood eicosic         |  gneictc
neighborhood nonadecic edge  |  gnheictc
neighborhood octacosic sombor|  gnnso
gneictcnheictcnnso
```

---

## 六、VectorAddress 命名空间更新

```
88=graph-topo ... 132=graph-topo45, 133=graph-topo46
```

插件：TOPIX_46 | 执行器：t46.exec | L4=133

---

## 七、测试结果

**新增测试**: 10 个（gos-graph-topo46-harness）  
**测试结果**: ✅ 10/10 通过  
**累计宿主测试总数**: 1543（原 1533 + 新增 10）

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## 八、S-variant 指数族系总览（截至 V3.57）

### S-幂次顶点和序列
NM₁(S²)→NF(S³)→NVQ(S⁴)→NPS(S⁵)→NSH(S⁶)→NSHP(S⁷)→NOC(S⁸)→NNC(S⁹)→NDC(S¹⁰)→NUC(S¹¹)→NDoC(S¹²)→NTC(S¹³)→NQTC(S¹⁴)→NPTC(S¹⁵)→NSTC(S¹⁶)→NHEPTC(S¹⁷)→NOCTC(S¹⁸)→NNONTC(S¹⁹)→**NEICTC(S²⁰)**

### S-广义 Sombor SO^α 序列
NSO(α=1)→NCSO(α=3)→NFSO(α=4)→NHSO(α=6)→NOSO(α=8)→NTSO(α=10)→NDSO(α=12)→NESO(α=14)→NGSO(α=16)→NIOSO(α=18)→NJSO(α=20)→NKSO(α=22)→NLSO(α=24)→NMSO(α=26)→**NNSO(α=28)**
