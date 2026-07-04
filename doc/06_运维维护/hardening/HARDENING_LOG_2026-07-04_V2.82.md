# HARDENING LOG — V2.82 | 2026-07-04

## 版本 / Version
**V2.82** — `graph diameter` 合并视图：单命令呈现图形的核心节点（center）与边界节点（peripheral）

## 变更摘要 / Change Summary

### 新增功能 / New Feature

**图论操作系统：新增 `graph diameter` / `gdiameter` 命令 — 结合 center + peripheral 的一站式结构边界视图**

`graph diameter` 命令在单个面板中同时展示：
- **图直径（diameter）**：最大有限离心率（max eccentricity）
- **图半径（radius）**：最小非零离心率（min nonzero eccentricity）
- **中心节点（center nodes，绿色）**：ecc == radius 的节点集合
- **边界节点（peripheral nodes，红色）**：ecc == diameter 的节点集合

当 radius == diameter 时（如完全图、有向环），所有节点同时出现在 center 和 peripheral 中，
此时面板仅显示一次（center 列）避免重复列出。

**命令输出格式示例（有向链 A→B→C→D→E）：**
```
 graph diameter view
 ───────────────────────────────────────────────────────────
  vector              ecc   role
  58.1.3.0              1   center         ← C (ecc=radius=1)
  58.1.1.0              4   peripheral     ← A (ecc=diameter=4)
 ───────────────────────────────────────────────────────────
  radius=1  diameter=4  center=1  periph=1  nodes=5
```

**颜色编码：**
| 颜色 | 含义 |
|------|------|
| 绿色（10） | center 节点：ecc == radius，图形"最中心"的位置 |
| 红色（12） | peripheral 节点：ecc == diameter，图形"最边缘"的位置 |
| 灰色（8）  | 分割线、空图/孤立图提示 |

**边界情况处理：**
- 空图 → "(empty graph)"
- 全孤立节点 → "all nodes isolated — diameter=0, radius=0"
- radius == diameter（完全对称图）→ 仅显示 center 列，不重复显示 peripheral

### 实现详情 / Implementation Details

**crates/k-shell/src/lib.rs — 新增 `dispatch_graph_diameter`（V2.82，约90行）**

```rust
pub fn dispatch_graph_diameter(sink: &ConsoleSink) {
    let (p_vecs, p_ecc, periph_count, p_node_count, diameter) =
        gos_runtime::graph_peripheral::<64>();
    let (c_vecs, c_ecc, center_count, _c_node_count, radius) =
        gos_runtime::graph_center::<64>();
    // ... 显示逻辑 ...
    // 当 radius != diameter 时追加 peripheral 节点
    if radius != diameter { /* 显示 periph 行 */ }
}
```

**核心设计决策：**
1. 调用容量限制为 `N=64`（center/peripheral 各最多64节点），比现有单独命令的128节省栈空间
2. 复用 `graph_peripheral` 和 `graph_center` 运行时函数，无新运行时代码，纯 k-shell 显示层
3. radius == diameter 判断：避免节点在同一面板中重复列出（如完全图所有节点ecc相等）
4. 列宽对齐：vector列16字符左对齐，ecc列6字符右对齐，role列追加文字标签

**crates/k-shell/src/proc.rs**

路由条目（在 `graph power law` 之后插入，遵循更具体命令在前的原则）：
```rust
} else if cmd == "graph diameter" || cmd == "gdiameter" {
    super::dispatch_graph_diameter(sink);
```

帮助文本（在 `graph center` / `gcenter` 行之后插入）：
```
  graph diameter     combined center+peripheral view: radius/diameter + core/boundary nodes
  gdiameter          alias for graph diameter
```

### 新增测试 / New Tests

**host-tests/gos-graph-diameter-harness/ (L4=58)**

10 个测试，覆盖 `graph_peripheral` 和 `graph_center` 的联合正确性：

| # | 图结构 | 关键验证 |
|---|--------|---------|
| 1 | 空图 | diameter=0, radius=0, 所有计数=0 |
| 2 | 单孤立节点 | diameter=0, radius=0, 所有计数=0 |
| 3 | 双向对 A↔B | radius=1, diameter=1, center=2, periph=2 |
| 4 | 有向链 A→B→C | radius=1, diameter=2, center={B}, periph={A} |
| 5 | 双向三角形 | radius=1, diameter=1, center=3, periph=3 |
| 6 | 有向环 A→B→C→A | radius=2, diameter=2, center=3, periph=3 |
| 7 | 双向星形 hub=A, spokes=B,C,D | radius=1, diameter=2, center={A}, periph=3 |
| 8 | 断开的双向对 A↔B + C↔D | radius=1, diameter=1, center=4, periph=4 |
| 9 | 有向链 A→B→C→D + 孤立 E | radius=1, diameter=3, center={C}, periph={A} |
| 10 | radius ≤ diameter 不变量（4种图形验证） | 路径/星形/环/K4 均满足 |

**精确值验证（测试4，有向链 A→B→C）：**
```
ecc[A] = 2 (A→B 1步, A→C 2步)
ecc[B] = 1 (B→C 1步)
ecc[C] = 0 (无出边，排除)
diameter = 2, radius = 1
peripheral = {A} (ecc==2), center = {B} (ecc==1)
```

**精确值验证（测试7，双向星形）：**
```
ecc[hub=A] = 1 (直接到达所有辐)
ecc[spoke=B] = 2 (B→A→C/D 两跳)
diameter = 2 (辐的离心率), radius = 1 (hub 的离心率)
center_count = 1 (仅 hub), periph_count = 3 (所有辐)
```

## 测试结果 / Test Results

```
gos-graph-diameter-harness (V2.82 新增):
running 10 tests
test bidirected_pair_radius_equals_diameter ... ok
test bidirected_star_hub_is_center ... ok
test disconnected_pairs_all_center_all_periph ... ok
test directed_cycle_all_symmetric ... ok
test directed_path_abc_center_and_periph ... ok
test bidirected_triangle_all_center_all_periph ... ok
test empty_graph_diameter_zero ... ok
test path_abcd_plus_isolated_e ... ok
test radius_leq_diameter_invariant ... ok
test single_isolated_diameter_zero ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

verify-graph-architecture: OK
```

## VectorAddress 命名空间 / VectorAddress Namespace

**L4=58** — gos-graph-diameter-harness

（L4=57 为 V2.81 gos-graph-summary2-harness）

## 关键不变量 / Key Invariants

- **radius == diameter 时不重复列出**：`if radius != diameter` 门控 peripheral 显示
- **N=64 容量上限**：合并视图各部分最多64节点（单独命令为128），节省栈空间
- **纯读取**：调用 `graph_peripheral` 和 `graph_center`，均为 pure read，不 bump epoch
- **无新运行时函数**：完全是 k-shell 显示层组合，零 ABI 增长
- **颜色语义一致**：green(10)=center 与 `dispatch_graph_center` 一致；red(12)=peripheral 与 `dispatch_graph_peripheral` 一致

## 累积测试套件 / Cumulative Test Suite

| 新增 Harness | 测试数 |
|-------------|--------|
| gos-graph-diameter-harness (V2.82) | 10 |
| 累积总数 | **793 tests** (783 + 10) |

## 下一步 / Next Steps

- Shell `graph compare <snapshot>` — 保存并对比两个时间点的拓扑指标快照
- 考虑将 `graph_clustering`（V2.61）重命名标注为 `graph_transitivity`，消除"Watts-Strogatz"命名误导
- 节点局部效率按 ecc 排序展示（结合 V2.74 + V2.82 的结构信息）
