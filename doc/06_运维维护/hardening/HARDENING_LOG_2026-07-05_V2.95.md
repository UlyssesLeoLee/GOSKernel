# GOSKernel 硬化日志 —— V2.95（2026-07-05）

## 摘要

**V2.95：最大团检测** —— 使用 Tomita 主元优化的迭代 Bron-Kerbosch 算法

本轮迭代增加了 `graph_clique`，完善了运行时中的图密度层级体系：

```
密度层级（由细到粗）：
  max-clique (ω)  ⊇  k-truss  ⊇  k-core
  V2.95              V2.94       V2.64
```

---

## 算法：带 Tomita 主元的 Bron-Kerbosch

**参考文献**：Bron & Kerbosch 1973（算法 457）；Tomita, Tanaka & Takahashi 2006（主元选择可最大化分支缩减效果）

**复杂度**：最坏情况 O(3^{n/3})；实践中每个分量约为 O(d · 2^{d/2})（d 为退化度）

**函数签名**：
```rust
pub fn graph_clique<N: usize>() -> ([VectorAddress; N], usize, usize, usize)
//                                   clique_vecs      ω(G)  count   nodes
```

### BkFrame 设计（迭代栈）

```rust
#[derive(Copy, Clone)]
struct BkFrame {
    r:           u128,  // 当前部分团（位掩码）
    p:           u128,  // 剩余候选节点
    x:           u128,  // 已排除（本层已处理过）
    to_try:      u128,  // P \ N(pivot) —— 本层待分支尝试的节点
    came_from_v: u8,    // 压入该帧的节点索引；0xFF 表示根节点
}
```

### 关键实现不变量

1. **在压入子帧之前从 `to_try` 中移除 v**——这样子帧弹出返回后不会重复处理 v。
2. **在弹出子帧时（而非压入时）更新父帧的 p/x**——这与 BK 算法的语义一致，即
   P←P\{v} 是在递归调用返回之后才发生的。
3. **两种弹出情形都需更新父帧**：`p==0`（P 为空）和 `to_try==0`（本层已穷尽）
   两种情况都会弹出并更新父帧。
4. **根节点哨兵值**：`came_from_v = 0xFF = u8::MAX`；节点索引范围为 [0..127]，
   所以 0xFF 始终越界，可安全用作哨兵。
5. **`all_p` 溢出保护**：`if nc >= 128 { u128::MAX } else { (1u128 << nc) - 1 }`——
   防止当全部 128 个节点槽位都被占满时发生 128 位移位溢出。

### Tomita 主元选择（每层 O(|P∪X|)）

```rust
fn choose_pivot(p_x: u128, p: u128, adj: &[u128; MAX_NODES]) -> usize {
    // 在 P∪X 中寻找使 |P ∩ N(u)| 最大的节点 u
    // 将分支因子从 |P| 降低到 |P \ N(u)|
}
```

---

## Shell 集成

| 命令 | 别名 |
|---------|-------|
| `graph clique` | `gclique`, `clique`, `max clique`, `maxclique` |

显示：以亮绿色列出代表性最大团的成员，页脚显示 `ω(G)=N  distinct-max-cliques=M`。

---

## 测试套件：gos-graph-clique-harness（10 项测试）

| # | 图 | 预期 ω | 预期计数 |
|---|-------|-----------|----------------|
| 1 | 空图 | 0 | 0 |
| 2 | 单个孤立节点 | 1 | 1 |
| 3 | 两个孤立节点 | 1 | 2 |
| 4 | 单条边 A-B | 2 | 1 |
| 5 | 三角形（K3） | 3 | 1 |
| 6 | K4（完全图） | 4 | 1 |
| 7 | 两个不相交的三角形 | 3 | 2 |
| 8 | 菱形（K4 减去 C-D 边） | 3 | 2 |
| 9 | 4 节点路径 A-B-C-D | 2 | 3 |
| 10 | K4 与 k-core/truss 交叉验证 | 4 | 1 |

**测试 10 验证的不变量**：
- ω(K4) = 4 = max_truss(K4) = max_kcore(K4) + 1
- ω(G) ≥ max_kcore（一般不变量：团规模 ≥ 退化度）
- ω(G) ≥ max_truss − 1（一般不变量）

---

## VectorAddress 命名空间

- **L4=71**：gos-graph-clique-harness

---

## 操作系统类比

内核依赖图中的最大团是联系最紧密的全互联子系统集群——团中每个模块都
直接依赖其他所有模块。这是热补丁或故障隔离时最难解耦的集群。从最大团中
移除任意一个节点，都会同时破坏团内的全部互连关系。

对比：
- **k-core**（V2.64）：子图中每个节点的邻居数 ≥ k——粗粒度耦合度量
- **k-truss**（V2.94）：每条边参与 ≥ k-2 个三角形——三角形内聚度
- **max-clique**（V2.95）：每对节点都直接相连——绝对最大密度

---

## 新增技能

- `gos-bk-clique-iterative-pattern` —— 在迭代 BK 算法中何时以及如何更新父帧
- 更新了 `gos-rust-format-curly-brace` —— 在现有表格中新增了 `{A}` 具名参数变体（E0425）

## 累计宿主测试计数

| 版本 | 测试数 |
|---------|-------|
| V2.94   | 913   |
| **V2.95** | **923** |
