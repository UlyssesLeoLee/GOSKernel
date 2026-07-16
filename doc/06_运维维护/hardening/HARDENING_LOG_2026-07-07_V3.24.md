# GOSKernel 硬化日志 — V3.24
**日期:** 2026-07-07
**分支:** feat/vk-auto-live-surface
**宿主测试套件总计:** 1213 个测试（全部通过）

---

## 摘要

V3.24 引入了**传输 Zagreb 指数（Transmission Zagreb indices）** —— 三个由顶点传输量 T(v)（每个节点到所有可达节点的 BFS 距离和）推导而来的指标。它们扩展了 V3.22 建立的基于传输量的指数家族（Balaban J、TI、PI_v），新增了平方传输量和乘积传输量变体。新的几何-算术传输指数 GA_t 使用 `isqrt128` 牛顿-拉夫逊实现，以处理会导致 u64 溢出的大型 u128 中间乘积。

---

## 新功能: `graph topo13` — TM₁ + TM₂ + GA_t 传输 Zagreb 指数

### API

```rust
pub fn graph_topo_indices13() -> (u64, u64, u64, usize, usize)
// 返回: (tm1, tm2, ga_t_ppm, edge_count, node_count)
```

### 指数

| 符号 | 公式 | 类型 | 文献 |
|--------|---------|------|-----------|
| TM₁ | Σ_v T_v² | 精确 u64 | Xing & Gutman 2012 |
| TM₂ | Σ_{uv∈E} T_u·T_v | 精确 u64 | Xing & Gutman 2012 |
| GA_t | Σ_{uv∈E} 2√(T_u·T_v)/(T_u+T_v) | 向下取整 ppm (×10⁶) | Alizadeh et al. 2013 |

其中 **T_v = Σ_{w reachable, w≠v} d(v,w)** 是 v 所在连通分量内的顶点传输量。

### 关键不变量

- 当图为**传输正则图**（所有 T_v 相等）时，`GA_t = |E| × 10⁶`
  - 例如：K_n（全部 T=n-1）、K₃（全部 T=2）、K₄（全部 T=3）、偶数环
- 对于非传输正则图（例如 K_{2,3}、星图、路径），`GA_t < |E| × 10⁶`
- 孤立节点：T_v=0，对 TM₁ 贡献为 0；对 TM₂ 或 GA_t 无边贡献

### 算法

1. **BFS O(n·(n+m))**：为所有节点计算 T_v
2. **O(n) 节点扫描**：TM₁ = Σ T_v²
3. **O(m) 无向边扫描（a < b）**：
   - TM₂ += T_a × T_b
   - GA_t: `isqrt128(4·T_a·T_b·10¹²) / (T_a + T_b)`（u128 运算）

### isqrt128 实现

每条边的 GA_t = `floor(2√(T_u·T_v) / (T_u+T_v) × 10⁶) = isqrt128(4·T_u·T_v·10¹²) / (T_u+T_v)`

由于 MAX_NODES=128 节点时最大 T_v ≈ 8128，`4·T_u·T_v·10¹² ≤ 2.64×10²⁰` 会导致 u64 溢出（最大值 1.84×10¹⁹）。因此需要 u128 版本的牛顿-拉夫逊 isqrt：

```rust
fn isqrt128(n: u128) -> u128 {
    if n == 0 { return 0; }
    let bits = 128u32 - n.leading_zeros();
    let mut x: u128 = 1u128 << ((bits + 1) / 2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x { return x; }
        x = y;
    }
}
```

无浮点运算，no_std 安全，以 O(log log n) 步牛顿-拉夫逊迭代收敛。

### 栈内存占用

- `adj[128]`（u128 × 128 = 2KB）
- `trans[128]`（u64 × 128 = 1KB）
- `dist[128]` + `queue[128]` = 256B
- **总计：约 3.5KB**

### 交叉验证表

| 图 | TM₁ | TM₂ | GA_t | 边数 | 节点数 |
|-------|-----|-----|------|-------|-------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| 边 A-B | 2 | 1 | 1_000_000 | 1 | 2 |
| 路径 P₃ | 22 | 12 | 1_959_590 | 2 | 3 |
| 三角形 K₃ | 12 | 12 | 3_000_000 | 3 | 3 |
| 星图 K_{1,4} | 212 | 112 | 3_848_364 | 4 | 5 |
| 路径 P₄ | 104 | 64 | 2_959_590 | 3 | 4 |
| 完全图 K₄ | 36 | 54 | 6_000_000 | 6 | 4 |
| 两个孤立点 | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | 158 | 180 | 5_975_154 | 6 | 5 |

### 推导示例

**K₃**：T_A=T_B=T_C=2。TM₁=3×4=12。TM₂=3×4=12。
GA_t：每条边 isqrt128(4×2×2×10¹²)/4 = 4_000_000/4 = 1_000_000 → 3_000_000（传输正则 ✓）

**K_{2,3}**：T_左侧=5, T_右侧=6。
GA_t：isqrt128(4×5×6×10¹²)/11 = isqrt128(120×10¹²)/11 = 10_954_451/11 = 995_859（每条边） → 5_975_154

**P₄**：T_A=T_D=6, T_B=T_C=4。
边 {B,C}：isqrt128(4×4×4×10¹²)/8 = 8_000_000/8 = 1_000_000（精确，传输量相同）。

### Shell 别名

```
graph topo13 | gtopo13 | transmission zagreb | gtm1tm2
tm1 index    | gtm1    | tm2 index           | gtm2
geometric arithmetic transmission | ggat | gtm1tm2gat
```

---

## OS 类比

| 指数 | OS 层面的解读 |
|-------|------------------|
| TM₁ | 平方路由负载压力 —— 放大那些距离加权可达范围较大的节点（枢纽放大器） |
| TM₂ | 边共同负载乘积 —— 衡量通道对的负载；TM₂ 越高表示端点对负载越重 |
| GA_t | 几何-算术通道负载均衡度 —— 均衡路由下等于 \|E\|×10⁶；枢纽-辐条式不对称时小于 \|E\|×10⁶ |

---

## 变更文件

| 文件 | 变更 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices13_inner()` + `graph_topo_indices13()`（含 isqrt128） |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices13()`（含 ppm 显示） |
| `crates/k-shell/src/proc.rs` | 新增 "graph topo13" / "gtopo13" / 别名的路由 |
| `host-tests/gos-graph-topo13-harness/` | 新增 10 项测试套件（VectorAddress L4=100） |

**提交（Commit）：** `feat(v3.24): Transmission Zagreb TM1 + TM2 + GA_t transmission-based indices + gos-graph-topo13-harness (10 tests)`
