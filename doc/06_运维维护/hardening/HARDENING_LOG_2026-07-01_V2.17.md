# GOS 硬化日志 — V2.17 — 2026-07-01

## 概述

V2.17 新增了一个 L4 域拓扑检视命令（`graph topo` / `graph topo <L4>`），
以及两个新的 gos-runtime API（`node_count_for_l4`、`node_page_l4`），
将 graph 命名空间的内省能力提升到了 Linux 的 `ip route show` / `lshw -short`
的水平。

---

## 修改清单

### 1. 两个新 API —— gos-runtime（`crates/gos-runtime/src/lib.rs`）

#### `RuntimeState::node_count_for_l4(l4: u8) -> usize`
私有 impl 方法 —— 单遍扫描统计 `vector.l4 == l4` 的节点数量。

#### `RuntimeState::node_page_l4<const N>(l4: u8, offset: usize, out: &mut [GraphNodeSummary; N]) -> (usize, usize)`
私有 impl 方法 —— 返回按指定 l4 域过滤后的一页 `GraphNodeSummary` 记录。
使用 `refresh_node_order()` 以确保结果按 vector 地址排序（与 `node_page` 的
不变式相同）。在填充该页之前会跳过 `offset` 个匹配项。返回值为
`(total_in_domain, filled)`。

#### 公开的模块级导出（位于 `proc_stat_for_vector` 之后）
```rust
pub fn node_count_for_l4(l4: u8) -> usize { RUNTIME.lock().node_count_for_l4(l4) }
pub fn node_page_l4<const N: usize>(l4: u8, offset: usize, out: ...) -> (usize, usize) { ... }
```

### 2. `graph topo` shell 命令 —— k-shell（`crates/k-shell/src/lib.rs`、`crates/k-shell/src/proc.rs`）

#### `dispatch_graph_topo(sink, l4_filter: Option<u8>)`

**总览模式**（`graph topo` / `topo`）：
- 通过 `node_page` 遍历所有存活节点，使用本地的 `[u8; 64]` / `[usize; 64]`
  数组对按 `l4` 分桶（不使用堆内存，也不使用 256 项的表）。
- 对域列表按 l4 值进行插入排序，以获得确定性的输出顺序。
- 每个非空域打印一行：`[l4=N]  K node(s)`。
- 页脚：域数量 + 总计 + 使用 `graph topo <l4>` 的提示。

**域详情模式**（`graph topo <L4>` / `topo <L4>`）：
- 以给定的 l4 值调用 `node_page_l4`，翻页直至耗尽。
- 打印每个节点：vector（补齐到 16 字符）、生命周期标签、plugin/key。
- 页脚：该域的节点总数。

`proc.rs` 中新增的 shell 分发逻辑：
- 精确匹配 `"graph topo"` 和 `"topo"` → `dispatch_graph_topo(sink, None)`。
- 前缀 `"graph topo <L4>"` / `"topo <L4>"` → 复用 `parse_epoch_decimal()`，
  校验其在 0–255 之间，然后调用 `dispatch_graph_topo(sink, Some(l4))`。
- 帮助文本新增两条条目。

### 3. 测试套件 —— `host-tests/gos-graph-topo-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `empty_graph_count_for_l4_returns_zero` | 空 runtime → count_for_l4(5) == 0 |
| 2 | `single_node_counted_in_correct_l4_domain` | l4=5 处有一个节点 → count_for_l4(5) == 1 |
| 3 | `node_not_counted_in_wrong_l4_domain` | l4=5 处有节点 → count_for_l4(6) == 0 |
| 4 | `two_nodes_same_l4_counted_correctly` | l4=3 处有 2 个节点 → count_for_l4(3) == 2 |
| 5 | `two_nodes_different_l4_each_counted_separately` | l4=5 和 l4=6 各有节点 → 各自计数均为 1 |
| 6 | `node_page_l4_returns_only_matching_domain` | l4=5 过滤器排除 l4=6 节点 |
| 7 | `node_page_l4_empty_domain_returns_zero` | 过滤 l4=99 → (0, 0) |
| 8 | `node_page_l4_total_matches_count_api` | page 的 total == node_count_for_l4 |
| 9 | `node_page_l4_offset_skips_correctly` | 3 个节点，offset=1 → filled==2 |
|10 | `node_page_l4_caps_filled_at_page_size` | 5 个节点，PAGE=2 → total=5，filled=2 |

---

## 验证

```
cd host-tests/gos-graph-topo-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

内核编译：
```
cargo build --release
# Finished `release` profile [optimized]
```

host-test 套件总计：**173 个测试**（163 个 V2.16 + 10 个新增）

---

## 生产质量说明

| 能力 | Linux/macOS 对应物 | GOS V2.17 |
|---|---|---|
| 网络拓扑视图 | `ip route show` / `ip link show` | `graph topo` / `graph topo <l4>` |
| 硬件设备树 | `lshw -short` | 带域细分的 `graph topo` |
| 按子系统列出节点 | `ip link show type veth` | `graph topo 6`（shell 域节点） |
| 域范围内的节点计数 | `ip -s link` 计数器 | `node_count_for_l4(l4)` |

`graph topo` 命令使 GOS vector 地址命名空间可以直接从操作者界面观察到，
补全了可观测性三件套：
- **运行的是什么** → `proc`（节点 × 信号 × 边）
- **它们如何连接** → `edges`（边拓扑）
- **它们存在于何处** → `graph topo`（域命名空间分布）

---

## Graph-OS 特性的保留

`graph topo` 是 GOS 独有的功能 —— 没有 POSIX 对应物，因为传统操作系统没有
分层 vector 地址命名空间的概念。l4 域字节是 GOS 的顶层命名空间分区
（类似于 BGP 的 AS 编号），`graph topo` 使这一分区边界成为理解系统结构的
主要视角。

---

*自动化硬化流程 — GOS V2.17 — 2026-07-01*
