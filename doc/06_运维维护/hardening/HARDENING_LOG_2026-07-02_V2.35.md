# 硬化日志 — GOS V2.35

**日期**：2026-07-02
**提交**：feat(v2.35): graph condensation / condense
**分支**：main
**作者**：自动化硬化流程（Claude Sonnet 4.6）

---

## 摘要

实现了 `graph condensation`——活跃节点图的凝聚（condensation）DAG。
至此完成了**图算法四重奏**（cycles / toposort / scc / condensation）。

---

## 修改内容

### `crates/gos-runtime/src/lib.rs`

在 `RuntimeState` 上新增 `graph_condensation_inner<const N: usize>` 方法：

- 运行与 `graph_scc_inner` 相同的 Kosaraju 两遍 DFS，为每个槽位分配 SCC ID。
- 阶段 3（新增）：扫描所有活跃边；对每条跨越 SCC 边界的边
  （`scc_id[from] != scc_id[to]`），设置 `adj[from_scc] |= 1u128 << to_scc`。
  重复的跨 SCC 边通过该位掩码以 O(1) 去重。
- 阶段 4：按 SCC 顺序打包节点与标签（与 `graph_scc_inner` 输出一致）。
- 返回值：`([VectorAddress; N], [u8; N], usize, usize, [u128; 128], usize)`
  = `(nodes, labels, total, scc_count, adj, cond_edges)`。
- 复杂度：O(V + E)，no_std 安全，全部为固定栈数组（无堆分配）。

新增公开包装函数 `graph_condensation<const N: usize>()`——薄封装（加锁 + 调用）。

**栈开销预算**：约 7.5 KB（两个 DFS 栈 + scc_id 数组 + 凝聚邻接矩阵 +
输出数组）。在内核栈限制范围内（x86_64-gos 上每线程 ≥ 16 KB）。

### `crates/k-shell/src/lib.rs`

新增 `dispatch_graph_condensation(sink: &ConsoleSink)`：

- 标题：`GRAPH CONDENSATION`（黑底青色）。
- 摘要行：`N components / M condensation edges / K nodes`。
- 每个 SCC 分块（布局与 `graph scc` 相同）：
  - `C#i` 标签（对多节点 SCC 显示 "cycle" 菱形标记）。
  - 成员 vector 地址（每行 4 个）。
  - 节点 key + 插件名。
  - 出向凝聚边：`→ C#j, C#k, …`（黄色箭头，青色目标）。
- 页脚提示：`condensation is always a DAG | use 'graph scc' to see cycle details`。

### `crates/k-shell/src/proc.rs`

新增路由（位于 `graph scc` 之后）：
```
"graph condensation" | "condensation" | "condense" | "graph condense"
    → dispatch_graph_condensation(sink)
```

### `host-tests/gos-graph-condensation-harness/`

新增测试套件——10 个集成测试，全部通过：

| # | 场景 | 预期结果 |
|---|----------|----------|
| 1 | 空图 | 0 个分量，0 条凝聚边 |
| 2 | 单个孤立节点 | 1 个分量，0 条凝聚边 |
| 3 | 双节点互环 A↔B | 1 个分量，0 条凝聚边 |
| 4 | 线性链 A→B→C | 3 个分量，2 条凝聚边 |
| 5 | 三角环 A→B→C→A | 1 个分量，0 条凝聚边 |
| 6 | 三角环 + 出边到 D | 2 个分量，1 条凝聚边 |
| 7 | 菱形 DAG（A→B, A→C, B→D, C→D） | 4 个分量，4 条凝聚边 |
| 8 | 同一 SCC 对之间的多条平行边 | 1 条凝聚边（已去重） |
| 9 | 内嵌环的链：A↔B, B→C, C→D | 3 个分量，2 条凝聚边 |
| 10 | DAG 不变量：凝聚邻接中无自边 | 已在混合图上验证 |

---

## 图算法四重奏——已完成

| 命令 | 算法 | 起始版本 | POSIX 对应物 |
|---------|-----------|-------|----------------|
| `graph cycles` / `cycles` | DFS 三色法 | V2.32 | `tsort` / 环检测工具 |
| `graph toposort` / `tsort` | Kahn BFS | V2.33 | `tsort(1)` |
| `graph scc` / `scc` | Kosaraju 两遍法 | V2.34 | `scc(1)` / `sccmap` |
| `graph condensation` / `condense` | Kosaraju + 邻接扫描 | V2.35 | `sccmap -F` / cargo 包间依赖 |

---

## 测试结果

```
gos-graph-condensation-harness:
  10 passed; 0 failed  ✓

gos-graph-scc-harness (regression):
  10 passed; 0 failed  ✓
```

**host 测试总数：353 个**（全部通过）

---

## 保持的不变量

- 所有 dispatch 函数均为纯读取——不产生 epoch 递增，无写操作。
- `graph_condensation_inner` 使用与其他测试相同的 `TEST_LOCK + reset()`
  隔离模式。
- 该测试套件拥有独立的 `[workspace]` + `.cargo/config.toml`
  （`x86_64-pc-windows-msvc`）。
- 凝聚邻接采用位压缩存储（每个 SCC 行一个 `u128`），自动对平行的跨
  SCC 边去重。
- 源图中的自环：在阶段 3 中跳过 `from_slot == to_slot`（与 SCC 逻辑
  保持一致）。
- 同一 SCC 内部的边：在阶段 3 中跳过 `fs == ts`——仅跨 SCC 的边才计入。

---

## 后续计划（V2.36+）

- `journal ring <N>` —— runtime 可配置的 JournalRing 容量
- `node checkpoint <vec>` —— 将节点状态快照写入 diff ring
- `graph path --all <to>` —— 多源路径枚举
- PAL_U32 → 属性节点重构（Demo A 前置条件）

---

*自动化硬化流程 — GOS V2.35 — 2026-07-02*
