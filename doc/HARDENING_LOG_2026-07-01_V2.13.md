# GOS 自动硬化日志 — 2026-07-01（第14次，V2.13 graph diff + diff-ring changelog）

> 类型：定期自动硬化（每2小时）
> 目标：V2.13 可观测性 — `graph diff` Shell 命令族 + 结构变更日志环 + gos-graph-diff-harness（10项测试）
> 提交：`feat(v2.13): graph diff shell command + structural mutation diff ring + 10-test harness`

---

## 执行摘要

本次硬化实现了 **图论操作系统的结构变更日志**（Structural Mutation Changelog），一个类比 `git log` / `git diff` 的内核级拓扑审计机制：

> 在图论 OS 中，内核图的每次结构变化（节点注册、边注册、边注销）都是一个可审计的事件。V2.13 起，这些事件被记录在一个 128 条容量的环形缓冲区（diff ring）中，可通过 `graph diff` Shell 命令、`since_epoch` API、以及 host-side 测试框架查询。

此前 GOS 的可观测性链路：

```
serial log → runtime atomics → TUI panel → text export → journal format → edges list
```

V2.13 后扩展为：

```
serial log → runtime atomics → TUI panel → text export → journal format → edges list → graph diff ring
```

新增内容：
1. **`gos-protocol`** — 2 个新类型（`GraphDiffKind`、`GraphDiffEntry`）
2. **`gos-runtime`** — diff ring 存储 + `graph_diff_since()` API + 3 个 mutation hook
3. **`k-shell`** — `dispatch_graph_diff()` 函数 + `GRAPH_DIFF_PIN_EPOCH` 原子
4. **`k-shell/proc.rs`** — 3 个新 Shell 命令分发分支
5. **`gos-graph-diff-harness`** — 10 项测试
6. 总测试数由 **123** 增至 **133**

---

## 新增功能

### 1. `crates/gos-protocol/src/lib.rs` — 2 个新类型（+55 行）

#### `GraphDiffKind` 枚举

```rust
pub enum GraphDiffKind {
    NodeAdded   = 0,
    NodeRemoved = 1,
    EdgeAdded   = 2,
    EdgeRemoved = 3,
}
```

带两个常量方法：
- `is_addition()` — true for NodeAdded / EdgeAdded
- `is_node()` — true for NodeAdded / NodeRemoved

#### `GraphDiffEntry` 结构体

```rust
pub struct GraphDiffEntry {
    pub epoch: u64,              // graph_epoch immediately after this mutation
    pub kind: GraphDiffKind,
    pub from_vector: VectorAddress,  // node vector (node events) or from-node vector (edge events)
    pub to_vector: VectorAddress,    // to-node vector (edge events) or zero (node events)
    pub label: [u8; 16],            // local_node_key / capability_binding, zero-padded
}
```

附 `label_str()` 方法（从 `[u8; 16]` 解析 UTF-8，安全截断）。

---

### 2. `crates/gos-runtime/src/lib.rs` — diff ring + API（+75 行）

#### 新常量

```rust
pub const MAX_DIFF_RING: usize = 128;
```

128 条容量（与 `MAX_NODES` 对齐），足以覆盖一次完整 boot 序列。环满时最旧条目被覆盖（circular overwrite）。

#### `GraphRuntime` 新字段

```rust
diff_ring: [GraphDiffEntry; MAX_DIFF_RING],
diff_ring_head: usize,      // next write slot
diff_total: u64,            // monotonic mutation count (never wraps to 0)
```

#### `push_diff()` 私有方法

```rust
fn push_diff(&mut self, kind, from_vec, to_vec, label: &[u8])
```

- 截断 label 至 16 字节
- 写入当前 `graph_epoch` 到 `epoch` 字段
- 递增 `diff_total`

#### Hook 点（3 处）

| 位置 | Hook 类型 |
|------|----------|
| `register_node()` — epoch bump 后 | `push_diff(NodeAdded, vector, zero, spec.local_node_key)` |
| `register_edge()` — epoch bump 后 | `push_diff(EdgeAdded, from_vec, to_vec, capability_binding)` |
| `unregister_edge()` — epoch bump 后 | `push_diff(EdgeRemoved, from_vec, to_vec, capability_binding)` |

#### 新公开 API

```rust
// 返回 since_epoch 之后的所有 diff 条目（按时间顺序，填入 out）
pub fn graph_diff_since<const N: usize>(
    since_epoch: u64,
    out: &mut [GraphDiffEntry; N],
) -> (usize, usize)  // (total_matching, filled)

// 自 boot 以来推入的 diff 条目总数（单调递增）
pub fn diff_total() -> u64
```

**`graph_diff_since` 算法**：从最旧存活槽开始遍历环，仅返回 `entry.epoch > since_epoch` 的条目——等价于 git 的 `--after` 过滤。

---

### 3. `crates/k-shell/src/lib.rs` — diff 命令实现（+90 行）

#### 新原子

```rust
pub(crate) static GRAPH_DIFF_PIN_EPOCH: AtomicU64 = AtomicU64::new(0);
```

0 表示"自 boot 以来"，任意值 N 表示"自 epoch N 被 pin 以后"。

#### `dispatch_graph_diff(sink, since_epoch)` 函数

类 `git diff` 的彩色输出：

```
 graph diff (epoch 0 -> 7)
 + [node+] 0xD1.1.0.0  gd.alpha  @epoch 1
 + [edge+] 0xD1.1.0.0 -[call.ab]-> 0xD1.2.0.0  @epoch 3
 - [edge-] 0xD1.1.0.0 -[call.ab]-> 0xD1.2.0.0  @epoch 5
   total: 3 change(s)  |  use 'graph diff pin' to update baseline
```

颜色方案：
- 绿色（`10`）= 添加（+ 前缀）
- 红色（`12`）= 删除（- 前缀）
- 青色（`11`）= 标题行
- 灰色（`8`）= epoch 注释、标点

---

### 4. `crates/k-shell/src/proc.rs` — 3 个新 Shell 命令（+20 行）

| 命令 | 行为 |
|------|------|
| `graph diff` 或 `diff` | `dispatch_graph_diff(sink, GRAPH_DIFF_PIN_EPOCH)` |
| `graph diff pin` 或 `diff pin` | `GRAPH_DIFF_PIN_EPOCH.store(graph_epoch(), ...)` → 打印确认 |
| `graph diff reset` 或 `diff reset` | `GRAPH_DIFF_PIN_EPOCH.store(0, ...)` → 打印确认 |

help 文本新增 3 行：
```
  graph diff         show topology changes since pinned epoch (like git diff)
  graph diff pin     pin current epoch as diff baseline
  graph diff reset   reset baseline to epoch 0 (show all since boot)
```

---

### 5. `host-tests/gos-graph-diff-harness/` — 新 harness（10 项测试）

| # | 测试名 | 验证点 |
|---|--------|--------|
| 1 | `empty_diff_returns_zero` | 空 runtime → `graph_diff_since(0)` 返回 `(0, 0)` |
| 2 | `register_node_appears_as_node_added` | 注册节点 → diff 中出现 `NodeAdded` 条目，vector 正确 |
| 3 | `register_edge_appears_as_edge_added` | 注册边 → diff 中出现 `EdgeAdded` 条目 |
| 4 | `diff_since_epoch_before_shows_mutations` | `diff_since(E_before)` 能看到 E_before 后的变更 |
| 5 | `diff_since_epoch_after_hides_earlier_mutations` | `diff_since(E_after)` 看不到 E_after 之前的变更 |
| 6 | `unregister_edge_appears_as_edge_removed` | `unregister_edge` → diff 中出现 `EdgeRemoved` 条目 |
| 7 | `diff_kind_is_node_correctness` | `is_node()` 对全部四个枚举变体正确 |
| 8 | `diff_kind_is_addition_correctness` | `is_addition()` 对全部四个枚举变体正确 |
| 9 | `diff_ring_wraps_and_total_is_monotonic` | 注册 130 个节点（> MAX_DIFF_RING=128）后，`diff_total()` ≥ 注册数 |
| 10 | `diff_entry_label_str_roundtrips_node_key` | `label_str()` 返回正确的 node key 前缀 |

---

## 质量指标

| 指标 | 本次 | 前次（V2.12） |
|------|------|--------------|
| 测试总数 | **133** | 123 |
| Clippy 警告（新增） | **0** | 0 |
| 新增测试 | **+10**（graph-diff harness 1-10） | +10 |
| 新增 Shell 命令 | **+3**（`graph diff`/`graph diff pin`/`graph diff reset`） | +3 |
| 受影响 crate | 3（gos-protocol、gos-runtime、k-shell） | 2 |

---

## 图论 OS 特性维护

- **结构变更即一等公民事件**：在图论 OS 中，"节点上线"和"边建立"本质上是拓扑状态机的跳变。V2.13 将这些跳变记录为可审计的 changelog，让系统的演化历史可被查询——类比 `git log` 之于代码库。
- **epoch-diff 语义**：`graph_diff_since(E)` 精确返回 epoch E 之后的所有变更，与 GOS 的 `graph_epoch()` 单调时钟完全对齐，无需挂钟时间，无需序列号注册。
- **零开销读取**：`dispatch_graph_diff` 只读 `diff_ring`（RUNTIME 锁持有时间极短），不触发任何写操作，不产生 epoch bump，符合 ADR-001 "read must be pure" 约束。
- **pin/reset 工作流**：`graph diff pin` → 做一些操作 → `graph diff` 显示这段时间内的所有拓扑变更，等价于 Linux 中 `inotify_add_watch` → 做操作 → `read(inotify_fd)` 的模式，但语义更清晰（基于 epoch 而非 inode 事件）。
- **diff ring 不阻塞 boot**：`push_diff` 是 inline 写入，无分配，无锁等待（在已持有 RUNTIME Mutex 的上下文中调用）。

---

## 下一步（V2.13 后续）

- [ ] VGA 层脏行追踪（epoch-diff hardware-level skip）
- [ ] `journal ring <N>` — 运行时动态配置 JournalRing 容量
- [ ] PAL_U32 → 属性节点重构（Demo A 前置）
- [ ] `graph diff <epoch>` — 支持直接传入 epoch 数字（`graph diff 42`）
- [ ] V3 生态层规划（见 `plan/V3_DEVELOPMENT_PLAN.md`）

---

## 测试结果

```
host-tests/gos-runtime-harness:              26 passed, 0 failed
host-tests/gos-supervisor-harness:          16 passed, 0 failed
host-tests/gos-rewrite-harness:             12 passed, 0 failed
host-tests/gos-rewrite-integration-harness:  6 passed, 0 failed
host-tests/gos-subscribe-harness:           10 passed, 0 failed
host-tests/gos-metrics-harness:            10 passed, 0 failed
host-tests/gos-boot-harness:               11 passed, 0 failed
host-tests/gos-node-inspect-harness:         8 passed, 0 failed
host-tests/gos-journal-harness:            14 passed, 0 failed
host-tests/gos-edge-inspect-harness:       10 passed, 0 failed
host-tests/gos-graph-diff-harness:         10 passed, 0 failed  (+10 新增)

总计：133 项测试全绿
```

---

*自动生成于 2026-07-01 定期硬化任务（第14次）*
