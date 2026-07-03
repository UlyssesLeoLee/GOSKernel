# GOS 硬化日志 — V2.16 — 2026-07-01

## 概述

V2.16 新增了按 epoch 寻址的 `graph diff <N>` shell 命令，使操作者能够查询自任意指定
epoch 编号以来的拓扑变更，而不再局限于仅能查询已保存的 pin epoch。这在 graph-OS
shell 中对应了 `git log --since-commit=<sha>` 的语义。

---

## 修改清单

### 1. `parse_epoch_decimal` — k-shell（`crates/k-shell/src/lib.rs`）

新增 `pub(crate)` 辅助函数：

```rust
pub(crate) fn parse_epoch_decimal(s: &str) -> Option<u64> {
    if s.is_empty() { return None; }
    let mut val: u64 = 0;
    for b in s.bytes() {
        if b < b'0' || b > b'9' { return None; }
        val = val.saturating_mul(10).saturating_add((b - b'0') as u64);
    }
    Some(val)
}
```

- 不需要 `std`/`alloc` —— 纯粹对输入切片进行逐字节迭代。
- `saturating_mul` + `saturating_add` 可防止溢出时 panic；非常大的 epoch 字符串会
  饱和到 `u64::MAX`（这会正确地返回 0 条 diff 记录）。
- 遇到任何非数字字符时返回 `None`，从而产生面向用户的错误提示信息。

### 2. `graph diff <N>` shell 命令 — k-shell（`crates/k-shell/src/proc.rs`）

在 `dispatch_text_command()` 中的 `graph diff reset` 之后插入新分支：

```
graph diff <N>   →   dispatch_graph_diff(sink, N)
diff <N>         →   同上（短别名同样有效）
```

实现模式：

```rust
} else if let Some(epoch_str) = cmd
    .strip_prefix("graph diff ")
    .or_else(|| cmd.strip_prefix("diff "))
    .filter(|s| *s != "pin" && *s != "reset")
{
    let trimmed = epoch_str.trim();
    if let Some(epoch) = super::parse_epoch_decimal(trimmed) {
        super::dispatch_graph_diff(sink, epoch);
    } else {
        // 打印错误："graph diff <epoch>: epoch must be a decimal number"
    }
```

`filter(|s| *s != "pin" && *s != "reset")` 这一层保护是多余的（精确匹配分支在
`else if` 链中已经排在前面），但它使意图更加明确，也更利于将来维护。

帮助文本更新为：
```
  graph diff <N>     show topology changes since epoch N (e.g. graph diff 42)
```

### 3. 测试套件 —— `host-tests/gos-graph-diff-epoch-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `diff_since_zero_returns_all_mutations` | epoch 0 → 3 条节点注册全部可见 |
| 2 | `diff_since_current_epoch_returns_nothing` | diff_since(current) → 0 条记录 |
| 3 | `diff_since_mid_epoch_shows_only_later_mutations` | 中途 pin → 只显示 pin 之后的记录 |
| 4 | `diff_since_epoch_boundary_is_exclusive` | epoch 边界具有排他性：epoch E 处的节点不出现在 diff_since(E) 中 |
| 5 | `diff_since_max_epoch_returns_nothing` | diff_since(u64::MAX) → 0 |
| 6 | `diff_since_zero_shows_mixed_node_and_edge_events` | NodeAdded 与 EdgeAdded 均可见 |
| 7 | `diff_since_epoch_before_edge_shows_edge_added` | 在 edge 之前 pin → EdgeAdded 可见 |
| 8 | `diff_since_after_node_shows_edge_not_node` | 在节点之后 pin → 只有 EdgeAdded 可见 |
| 9 | `diff_since_fills_capped_at_page_size` | 总数 > PAGE → filled == PAGE，total 正确 |
|10 | `diff_since_pin_shows_edge_removed` | pin → unregister_edge → EdgeRemoved 可见 |

---

## 验证

```
cd host-tests/gos-graph-diff-epoch-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cd host-tests/gos-graph-diff-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed  (regression: unchanged)

cargo build --release
# Finished `release` profile
```

---

## 生产质量说明

| 能力 | Linux/macOS 对应物 | GOS V2.16 |
|---|---|---|
| 从已知检查点进行的时间点 diff | `git log <commit>..HEAD` | `graph diff <epoch>` |
| 全量拓扑历史 | `git log` | `graph diff 0` |
| 自上次操作以来的 diff | `git diff HEAD~1` | `graph diff <epoch_before>` |
| 仅从未来某个时间点开始的 diff | 无对应物 | `graph diff <future_epoch>` → 空 |

在 V2.16 之前，操作者只能针对已保存的 pin epoch（上一次 `graph diff pin`）或
epoch 0 进行 diff。V2.16 允许直接寻址任意 epoch，从而支持诸如
"自我注册节点 X 以来发生了什么变化？" 这类一次性查询，而无需预先 pin。

---

## Graph-OS 特性的保留

epoch 系统是 graph-OS 中单调逻辑时钟的对应物：每一次结构性变更（节点注册/
注销、边注册/注销）都会使 epoch 前进一。`graph diff <N>` 将这一时钟直接暴露给
操作者 shell，忠实体现了 graph-OS 的原则——拓扑是一等公民、可检视、可审计的
结构，而不是隐藏的内核实现细节。

---

*自动化硬化流程 — GOS V2.16 — 2026-07-01*
