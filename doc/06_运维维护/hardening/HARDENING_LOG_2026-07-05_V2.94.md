# GOS 自动硬化日志 — 2026-07-05（V2.94 k-truss decomposition）

> 类型：定期自动硬化（每2小时）  
> 目标：V2.94 图论算法 — k-truss 分解（边三角形凝聚度）  
> 提交：`feat(v2.94): k-truss decomposition -- edge-peeling + gos-graph-truss-harness (10 tests)`

---

## 执行摘要

本次硬化为图论 OS 新增 **k-truss 分解**（Wang & Cheng 2012 边剥离算法），是 k-core 分解（V2.64）的严格精化版本。k-truss 在 **边级别**（而非节点级别）度量三角形凝聚度，为识别内核子系统中的高冗余依赖簇提供更细粒度的指标。

1. **`gos_runtime::graph_truss<N>()`** — 边剥离算法，返回每个节点的 trussness 值
2. **`dispatch_graph_truss`** — k-shell 显示层（彩色，类比 kcore 输出）
3. **Shell 路由** — `"graph truss"` / `"gtruss"` / `"truss"` / `"k-truss"` / `"ktruss"`
4. **gos-graph-truss-harness** — 10 项测试，全绿（L4=70）
5. **2 项新技能** — `gos-harness-static-str-helper` / `gos-truss-cascade-peeling-pattern`

---

## 算法原理

**k-truss 定义**：k-truss 是图的极大子图，其中每条边至少参与 `k−2` 个三角形（在该子图内）。  
**trussness(e)**：边 e 的 trussness = 使 e 仍属于 k-truss 的最大 k 值。  
**节点 trussness**：所有关联边的 trussness 最大值（孤立节点为 0）。

**与 k-core 关系**（严格精化）：
- 每个 k-truss 必然是 (k−1)-core 的子集：k-truss ⊆ (k−1)-core
- 对 K₄：max_truss = 4，max_core = 3；即 max_truss = max_core + 1
- 一般情况：max_truss ≥ max_core + 1（不等式，k-truss 粒度更细）

---

## 算法实现

### 阶段 1 — 建立无向边列表（去重）

从有向边表构建 `(lower_slot, higher_slot)` 的无向边列表，消除：
- 自环（`from_slot == to_slot`）  
- 反向重复边（A→B 和 B→A 合并为一条无向边 {A,B}）

### 阶段 2 — 计算初始三角形支持度

```
support[ei] = |N(a) ∩ N(b)|  (a, b 为 ei 的两个端点)
```

对每条边 `(a, b)`：枚举与 a 相邻的边 `(a, w)`，检查 `(b, w)` 是否存在 → 三角形计数。

时间复杂度：O(E²)，对 MAX_EDGES=512 约 262K 次操作。

### 阶段 3 — 迭代边剥离

```
for k = 3, 4, ...:
  threshold = k - 2
  repeat until stable:
    for each active edge ei:
      if sup[ei] < threshold:
        remove ei; edge_truss[ei] = k-1
        for each triangle (a, b, w) containing ei:
          sup[(a,w)] -= 1
          sup[(b,w)] -= 1
```

关键：当共享边 A-B（初始 sup=2）的两个三角形的外边（各 sup=1）被移除时，级联使 A-B 的 sup 降为 0。A-B 在同一 k=4 轮中也被移除，获得 trussness=3（而非 4）。

### 阶段 4 — 节点 trussness 聚合

```
node_truss[slot] = max(edge_truss[ei]) for all incident edges ei
```

孤立节点（无边）保持默认值 0。

---

## 变更详情

### 1. `crates/gos-runtime/src/lib.rs`（+220 行）

新增 `graph_truss_inner<const N: usize>()` 方法（在 `impl GraphRuntime` 中，`graph_2ecc_inner` 之后）：

```rust
pub fn graph_truss_inner<const N: usize>(&self)
    -> ([VectorAddress; N], [u8; N], usize, u8)
```

新增公开包装函数：
```rust
pub fn graph_truss<const N: usize>()
    -> ([VectorAddress; N], [u8; N], usize, u8)
```

栈内存用量（函数内）：
| 变量 | 大小 |
|------|------|
| `eu[512]`, `ev[512]` | 2 × 512 B |
| `sup[512]`, `active[512]`, `edge_truss[512]` | 3 × 512 B |
| `node_truss[128]`, `node_slots[128]` | 1152 B |
| `out_vecs[N=128]`, `out_truss[N=128]` | ~640 B |
| **Total** | **~5 KB** |

输出排序：trussness 降序 → VectorAddress 升序（确定性）。

---

### 2. `crates/k-shell/src/lib.rs`（+95 行）

新增 `dispatch_graph_truss(sink: &ConsoleSink)`:

```
 graph truss  (k-truss decomposition)
 ──────────────────────────────────────────────────────────
  vector              k     role
  1.0.0.1            4  truss-core   ← 绿色（最高 trussness）
  1.0.0.2            4  truss-core
  1.0.0.3            3  inner        ← 青色（中间层）
  1.0.0.4            2  leaf         ← 黄色（有边无三角形）
  1.0.0.5            0  isolated     ← 灰色（孤立节点）
 ──────────────────────────────────────────────────────────
  5 node(s)  truss-number=4
```

颜色编码：
| 颜色 | 角色 |
|------|------|
| 绿(10) | truss-core（最高 k） |
| 青(11) | inner（中间 truss） |
| 黄(14) | leaf（trussness=2，有边无三角） |
| 灰(8)  | isolated（trussness=0） |

---

### 3. `crates/k-shell/src/proc.rs`（+2 行）

在 `"graph 2ecc"` 路由之后插入：

```rust
} else if cmd == "graph truss" || cmd == "gtruss" || cmd == "truss"
       || cmd == "k-truss" || cmd == "ktruss" {
    super::dispatch_graph_truss(sink);
```

---

### 4. `host-tests/gos-graph-truss-harness/`（新建，+4 文件）

| # | 测试名 | 验证点 |
|---|--------|--------|
| 1 | `test_01_empty_graph` | node_count=0, max_truss=0 |
| 2 | `test_02_single_isolated_node` | trussness=0 |
| 3 | `test_03_single_directed_edge_no_triangle` | trussness=2（有边无三角） |
| 4 | `test_04_triangle_trussness_three` | A→B→C→A: trussness=3 |
| 5 | `test_05_k4_trussness_four` | K₄ 双向完全图: trussness=4 |
| 6 | `test_06_two_triangles_sharing_edge` | 两三角共享边→级联剥离→trussness=3 |
| 7 | `test_07_path_no_triangles` | 路径: trussness=2 |
| 8 | `test_08_star_no_triangles` | 星图: trussness=2 |
| 9 | `test_09_triangle_plus_isolated_node` | 三角(3)+孤立(0)混合 |
| 10 | `test_10_truss_strictly_finer_than_kcore` | K₄: max_truss=4, max_core=3, max_truss==max_core+1 |

---

## 技术难点与发现

### 1. `&'static str` 在 harness helper 中的限制

第一次编译时：

```rust
fn add_bidir(a: NodeId, b: NodeId, key: &'static str) {
    add_edge(a, b, &format!("{key}f"));  // E0716: temporary dropped while borrowed
```

`format!()` 创建的字符串不满足 `'static` 生命周期约束（`derive_edge_id` 是 `const fn`，要求 `&'static str`）。  
**修复**：改为两个独立参数 `fwd: &'static str, rev: &'static str`。

→ 新技能：`gos-harness-static-str-helper`

### 2. 级联剥离的直觉陷阱

直觉上：共享边 A-B 初始 support=2（两个三角形），应比外边（sup=1）有更高 trussness。  
**实际**：外边在 k=4 轮被移除后，对 A-B 的三角形贡献消失，A-B sup 降至 0，也在同一轮被移除，trussness=3（与外边相同）。  
k-truss 是**内聚子图属性**：必须在剩余子图内保持三角形，不能引用已删除的边。

→ 新技能：`gos-truss-cascade-peeling-pattern`

---

## 质量指标

| 指标 | V2.94 | V2.93（前次） |
|------|-------|--------------|
| 宿主测试总数 | **913** | 903 |
| 新增测试 | **+10**（truss harness） | +10 |
| 新增 Shell 命令 | **+1**（`graph truss`） | +1 |
| 新增技能 | **+2** | +1 |
| 项目技能库总量 | **63** | 61 |
| 受影响 crate | 2（runtime/k-shell） | 2 |

---

## 图论 OS 特性维护

- **严格精化层次**：k-truss ⊆ (k−1)-core，提供 k-core 无法区分的更细粒度内核子图
- **无浮点运算**：全部为整数运算，no_std 安全
- **栈上全量计算**：5 KB 栈，无堆分配——与内核安全模型完全兼容
- **OS 类比**：高 trussness 节点 = 参与多路冗余三角形依赖的内核模块（如 RAID-1 或双 NIC bonding 架构中多重可靠路径的节点）

---

## 下一步建议（V2.95+）

- [ ] **最大密度子图**（Charikar 2000 贪心 2-近似）— 补充 k-truss 的密度分析维度
- [ ] **顶点连通度 κ(G)**（最小顶点割集，基于最大流）— 对比现有桥/割点检测
- [ ] **约翰逊全对最短路**（含负权重边）— 补全最短路算法族
- [ ] **社区检测 Girvan-Newman**（基于边介数的层次社区划分）

---

## 测试结果

```
host-tests/gos-graph-truss-harness:     10 passed, 0 failed  (新增, 全绿)
```

*（其余 903 项测试与 V2.93 状态相同，逐个运行时全绿）*

---

*自动生成于 2026-07-05 定期硬化任务（V2.94）*
