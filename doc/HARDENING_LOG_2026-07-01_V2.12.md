# GOS 自动硬化日志 — 2026-07-01（第13次，V2.12 edges 命令 + edge-inspect-harness）

> 类型：定期自动硬化（每2小时）
> 目标：V2.12 可观测性 — `edges` Shell 命令族 + gos-edge-inspect-harness（10项测试）
> 提交：`feat(v2.12): edges shell command + gos-edge-inspect-harness (10 tests)`

---

## 执行摘要

本次硬化围绕 **图论操作系统的边（Edge）可观测性**，填补了自 V2.8 的 `nodes` 命令以来的对称性缺口：

| 命令层 | V2.8（节点） | V2.12（边） |
|--------|-------------|-------------|
| 全量列表 | `nodes` | **`edges`** |
| 过滤视图 | `nodes faulted` | **`edges <type>`** |
| 轻量统计 | `nodes summary` | **`edges count`** |

在图论操作系统中，边（edge）是第一等公民——进程间通信、能力授予、资源挂载全部通过边表达。没有边的可观测性，就无法完整诊断图拓扑状态。本次添加的命令与 Linux 中 `ss -a`（socket 列表）和 `lsof`（文件描述符列表）等价，将 GOS 的运行时可见性延伸到连接层。

新增内容：
1. **3 个 Shell 命令**（`edges`、`edges count`、`edges <type>`）
2. **`gos-edge-inspect-harness`** — 10 项 edge 枚举 API 测试
3. 总测试数由 **113** 增至 **123**

---

## 新增功能

### 1. `crates/k-shell/src/lib.rs` — 3 个新函数（+118 行）

#### `parse_edge_type_filter(s: &str) -> Option<RuntimeEdgeType>`

将 `edges <type>` 命令参数解析为 `RuntimeEdgeType` 枚举。支持 9 种边类型关键词：

| 关键词 | 类型 |
|--------|------|
| `call` | `RuntimeEdgeType::Call` |
| `spawn` | `RuntimeEdgeType::Spawn` |
| `depend` | `RuntimeEdgeType::Depend` |
| `signal` | `RuntimeEdgeType::Signal` |
| `return` | `RuntimeEdgeType::Return` |
| `mount` | `RuntimeEdgeType::Mount` |
| `sync` | `RuntimeEdgeType::Sync` |
| `stream` | `RuntimeEdgeType::Stream` |
| `use` | `RuntimeEdgeType::Use` |

#### `dispatch_edges_list(sink: &ConsoleSink, filter_type: Option<RuntimeEdgeType>)`

文本模式边列表——GOS 中 `ss -a` / `lsof` 的等价命令。

- 通过 `gos_runtime::edge_page::<8>()` 分页枚举全部活跃边
- 当 `filter_type` 为 `Some(t)` 时，仅显示该类型的边（客户端过滤）
- 颜色编码边类型：
  - 绿色（`10`）= Call / Spawn（执行类）
  - 黄色（`14`）= Mount（挂载类）
  - 青色（`11`）= Use（使用类）
  - 蓝色（`9`）= Depend（依赖类）
  - 品红（`13`）= Sync（同步类）
  - 红色（`12`）= Signal（信号类）
- 格式：`  from_vec -[type]-> to_vec  key`

**示例输出**（`edges` 命令）：

```
 live edges
  1.1.0.0 -[depend]-> 2.1.0.0  k.boot.dep
  2.1.0.0 -[mount]-> 6.1.4.0   clipboard.mount
  6.1.3.0 -[use]-> 6.1.1.0     theme.use
  (共 3 条边)
```

**示例输出**（`edges mount` 命令）：

```
 mount edges
  2.1.0.0 -[mount]-> 6.1.4.0   clipboard.mount
```

#### `dispatch_edge_count(sink: &ConsoleSink)`

轻量统计命令，仅通过 `edge_page::<1>(0, _)` 读取 total，不枚举摘要。类似 Linux `ss --summary`。

```
 edge count
  total: 47
  status: edges active
```

---

### 2. `crates/k-shell/src/proc.rs` — 命令分发 + help 文本（+15 行）

**新增 dispatch 分支**（在 `journal` 之后）：

```rust
} else if cmd == "edges" || cmd == "edges all" {
    super::dispatch_edges_list(sink, None);
} else if cmd == "edges count" || cmd == "edge count" {
    super::dispatch_edge_count(sink);
} else if let Some(type_str) = cmd.strip_prefix("edges ") {
    if let Some(et) = super::parse_edge_type_filter(type_str) {
        super::dispatch_edges_list(sink, Some(et));
    } else {
        // error: unknown edge type
    }
```

**help 文本新增**（3 行）：

```
  edges              list all live graph edges (ss-style)
  edges count        total edge count
  edges <type>       filter by type: call spawn depend signal return mount sync stream use
```

---

### 3. `host-tests/gos-edge-inspect-harness/` — 新 harness（10 项测试）

完整结构：
- `Cargo.toml`（依赖：gos-protocol / gos-cypher-mut / gos-runtime / gos-supervisor）
- `.cargo/config.toml`（`x86_64-pc-windows-msvc` target，与现有 harness 一致）
- `tests/edge_inspect.rs`（10 项测试）

| # | 测试名 | 验证点 |
|---|--------|--------|
| 1 | `empty_runtime_edge_page_returns_zero` | 空 runtime → `edge_page` 返回 `(0, 0)` |
| 2 | `single_edge_returned_by_edge_page` | 注册1条边 → `(1, 1)`，from/to_vector 正确 |
| 3 | `edge_type_roundtrips_through_register_and_read` | `RuntimeEdgeType::Call` 序列化→反序列化一致 |
| 4 | `all_registered_edges_appear_in_edge_page` | 注册3条边 → 全部在 page 中出现 |
| 5 | `edge_page_offset_beyond_total_returns_zero` | offset ≥ total → returned = 0 |
| 6 | `unregister_edge_removes_from_edge_page` | 注销后 total 减1，剩余边正确 |
| 7 | `edge_page_for_node_filters_to_node` | `edge_page_for_node(VEC_C, ...)` 仅返回涉及 VEC_C 的边 |
| 8 | `edge_page_for_node_returns_outbound_from_source` | VEC_A 有2条出边 → 均被 `edge_page_for_node` 返回 |
| 9 | `register_edge_is_idempotent` | 同一 EdgeId 注册两次 → EdgeId 相同，无重复 |
| 10 | `mixed_edge_types_all_round_trip` | Call / Mount / Use 三种类型均出现在 `edge_page` 结果中 |

---

## 质量指标

| 指标 | 本次 | 前次（V2.11） |
|------|------|--------------|
| 测试总数 | **123** | 113 |
| Clippy 警告（新增） | **0** | 0 |
| 新增测试 | **+10**（edge-inspect harness 1-10） | +14 |
| 新增 Shell 命令 | **+3**（`edges`/`edges count`/`edges <type>`） | +1 |
| 受影响 crate | 2（k-shell、新 harness） | 3 |

---

## 图论 OS 特性维护

- **边是第一等公民**：在图论 OS 中，进程间通信 = 图边。`edges` 命令让运维人员实时看到完整的连接拓扑，与节点视图对等，符合 "everything is a graph" 原则。
- **纯读取原则**：`dispatch_edges_list` 和 `dispatch_edge_count` 均仅调用 `gos_runtime::edge_page`，不触发任何写操作，不产生 epoch bump，符合 ADR-001 "read must be pure" 约束。
- **类型过滤对齐图论语义**：`edges depend` 显示依赖图（相当于 `ldd`），`edges mount` 显示挂载关系（相当于 `mount`），`edges call` 显示调用图（相当于 `strace -e trace=all`），充分发挥图论 OS 语义丰富性。
- **可观测性链路扩展**：serial log → runtime atomics → TUI panel → text export → journal format → **边列表（`edges`）**，六层覆盖。

---

## 下一步（V2.12 后续）

- [ ] VGA 层脏行追踪（epoch-diff hardware-level skip）
- [ ] `journal ring <N>` — 运行时动态配置 JournalRing 容量
- [ ] PAL_U32 → 属性节点重构（Demo A 前置）
- [ ] `graph diff` — 两个 epoch 间的拓扑差量（类 `git diff`）
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
host-tests/gos-edge-inspect-harness:       10 passed, 0 failed  (+10 新增)

总计：123 项测试全绿
```

---

*自动生成于 2026-07-01 定期硬化任务（第13次）*
