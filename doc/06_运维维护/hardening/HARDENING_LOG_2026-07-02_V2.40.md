# GOS 硬化日志 — V2.40（2026-07-02）

## 特性：`graph closeness` —— 出向接近中心性

**分支：** feat/vk-auto-live-surface
**提交范围：** feat(v2.40): graph closeness / closeness — outgoing closeness centrality
**新增测试套件：** gos-graph-closeness-harness（10 个测试）
**本次切片后累计 host 测试数：** 403

---

## 本次构建内容

### Shell 命令一览

| 命令 | 别名 | 说明 |
|---------|---------|-------------|
| `graph closeness` | `closeness`, `graph close`, `close centrality`, `cc` | 每个节点的出向接近中心性，按降序排列 |

### 算法：出向接近中心性（每个源节点执行一次 BFS）

**定义：**

对每个存活节点 v，其出向接近中心性为：

```text
CC[v] = r_v × SCALE / Σ_{u reachable from v, u≠v} d(v,u)
```

其中：
- `r_v` = 通过有向边可从 `v` 到达的节点数量（不含 `v` 自身）
- `d(v,u)` = 从 `v` 到 `u` 的 BFS 最短路径距离
- `SCALE` = 1,000,000（定点数缩放，避免在 `no_std` 中使用浮点数）
- 孤立节点（`r_v = 0`）：`CC[v] = 0`

**复杂度：** O(V × (V + E)) —— 每个源节点执行一次 BFS。

**输出：** 按 CC 分值降序排列。角色标注：
- `central` —— CC 分值最高：能以最高效率向所有其他节点广播的节点
- `relay` —— CC 处于中等水平：能到达其他节点，但效率并非最高
- `peripheral` —— CC = 0：孤立节点、纯汇点，或与任何可达子图都不连通

**定点数说明：** CC 分值以整数 × 10⁻⁶ 的形式报告。例如：
- CC = 1,000,000 → 精确接近度 = 1.0（恰好 1 跳即可到达所有可达节点）
- CC = 666,666 → 精确接近度 ≈ 0.6667（到可达节点的平均跳数为 1.5）
- CC = 500,000 → 精确接近度 = 0.5（平均 2 跳）

**不连通图的处理：** 该公式在分子中使用 `r_v`（可达节点数），即使图被分割成多个部分，也能自然地给予能到达许多节点的节点相应的分值。无法到达任何其他节点的节点 CC = 0。

### 与介数中心性（V2.39）的比较

| 维度 | 介数中心性（V2.39） | 接近中心性（V2.40） |
|-----------|--------------------|--------------------|
| 回答的问题 | "哪些节点处于最多最短路径之上？" | "哪些节点能最快地到达所有其他节点？" |
| 算法 | Brandes 2001（O(V×E)） | 每源节点 BFS（O(V×(V+E))） |
| 高分含义 | 关键路由瓶颈 | 高效的广播者 / 广播枢纽 |
| 零分含义 | 从不作为中介（叶子节点/孤立节点） | 无法到达任何其他节点（汇点/孤立节点） |
| 操作系统类比 | `traceroute` 跳数频率 | `ping` 平均往返时延 |

介数中心性与接近中心性共同构成"结构性瓶颈 + 到达效率"这一分析对：
- 介数中心性回答**依赖风险**问题：移除一个高 BC 值节点会破坏许多路径。
- 接近中心性回答**延迟到达**问题：高 CC 值节点能最快地传播信号。

---

## 修改文件

### `crates/gos-runtime/src/lib.rs`
- 在 `GosRuntime` 结构体上新增 `graph_closeness_inner<const N>(&self)` 方法
- 新增 `pub fn graph_closeness<const N>()` 公开包装函数（获取 `RUNTIME` 锁）

### `crates/k-shell/src/lib.rs`
- 新增 `pub fn dispatch_graph_closeness(sink: &ConsoleSink)`，包含完整的彩色表格输出

### `crates/k-shell/src/proc.rs`
- 新增路由：`"graph closeness" || "closeness" || "graph close" || "close centrality" || "cc"`
- 为 `graph closeness` 和 `closeness / cc` 新增帮助文本条目

### `host-tests/gos-graph-closeness-harness/`（新建）
- `Cargo.toml` —— 与工作区隔离的 harness crate
- `.cargo/config.toml` —— `target = "x86_64-pc-windows-msvc"`，`build-std = ["std", "panic_abort"]`
- `tests/graph_closeness.rs` —— 10 个测试

---

## 测试覆盖（10 个测试）

| # | 测试 | 断言 |
|---|------|-----------|
| 1 | 空图 | `total=0`，不发生 panic |
| 2 | 单个孤立节点 | `CC[A]=0, total=1` |
| 3 | 两节点 A→B | `CC[A]=1_000_000, CC[B]=0` |
| 4 | 路径 A→B→C | `CC[B]=1_000_000, CC[A]=666_666, CC[C]=0`；B 排最前 |
| 5 | 星形 A→{B,C,D} | `CC[A]=1_000_000`，叶节点 `=0`；A 排最前 |
| 6 | 有向 3 元环 A→B→C→A | 全部相等，均为 `666_666`（旋转对称性） |
| 7 | 菱形 A→{B,C}→D | `CC[B]=CC[C]=1_000_000, CC[A]=750_000, CC[D]=0` |
| 8 | 线性 5 节点链 A→B→C→D→E | 顺序为 D>C>B>A>E；断言精确数值 |
| 9 | 不连通 {A→B} ∥ {C→D} | `CC[A]=CC[C]=1_000_000`，汇点 `=0` |
| 10 | 自环 A→A + B→C | `CC[A]=0`（自环 = 无外部可达性） |

全部 10 个测试：**通过**（已在本地通过 `cargo +nightly test` 验证）。

---

## 操作系统类比

**`graph closeness`** ↔ **`ping` 平均往返时延统计**

正如 `ping -c 100 <host>` 测量到某个远程端点的平均延迟一样，接近中心性测量一个内核服务节点通过有向信号边"到达"服务图中所有其他节点的速度。高 CC 值节点就像一个核心路由守护进程，与所有对等节点的往返时延都在亚毫秒级 —— 它能以最少的跳数将信号传播到最广泛的一批节点。

```bash
# 在 POSIX 操作系统中的等价概念操作：
for host in $(hosts); do
    avg_rtt=$(ping -c 10 $host | awk '/avg/{print $4}' | cut -d/ -f2)
    echo "$host: avg_rtt=$avg_rtt"
done | sort -t= -k2 -n

# GOS 中的等价命令：
graph closeness
```

---

## 图算法套件 —— V2.40 后的状态

| 版本 | 命令 | 算法 | 复杂度 |
|---------|---------|-----------|------------|
| V2.32 | `graph cycles` | DFS 三色标记 | O(V+E) |
| V2.33 | `graph toposort` | Kahn BFS | O(V+E) |
| V2.34 | `graph scc` | Kosaraju | O(V+E) |
| V2.35 | `graph condensation` | Kosaraju+邻接 | O(V+E+V²) |
| V2.36 | `graph reachable <V>` | 迭代式 DFS | O(V+E) |
| V2.37 | `graph bipartite` | BFS 二染色 | O(V+E) |
| V2.38 | `graph degree` | 边统计 | O(V×E) |
| V2.39 | `graph centrality` | Brandes BC | O(V×E) |
| **V2.40** | **`graph closeness`** | **每源节点 BFS** | **O(V×(V+E))** |

**下一步候选项：**
- `graph eccentricity` —— 每个节点的最大最短路径距离（图半径/直径）
- `node checkpoint <vec>` —— 将节点状态快照到 diff ring
- `journal ring <N>` —— 运行时可配置的 JournalRing 容量
- PAL_U32 → attribute node 重构（Demo A 前置条件）
