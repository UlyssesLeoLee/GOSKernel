# GOS 自动硬化日志 — 2026-07-01（第12次，V2.11 journal 命令 + decode_kind 修复）

> 类型：定期自动硬化（每2小时）
> 目标：V2.11 可观测性 — `journal` Shell 命令 + gos-journal `decode_kind` Bug 修复 + 14 项测试覆盖
> 提交：`feat(v2.11): journal shell command + decode_kind bug fix + 14-test harness`

---

## 执行摘要

本次硬化围绕 **gos-journal 控制平面日志层**，聚焦两点：

1. **Bug 修复**：`gos-journal::decode_kind` 仅处理了 `ControlPlaneMessageKind` 的前 8 个变体（0x01–0x08），而协议已扩展至 12 个变体（新增 `MutationAudit`、`CausalOverflow`、`RuleApplied`、`SubscribeTriggered`）。任何包含这 4 种较新事件的日志文件在 `replay()` 时将返回 `JournalError::UnknownKind` 错误，导致静默丢失所有后续记录。
2. **`journal` Shell 命令**：新增 `dispatch_journal_info`，以人类可读格式报告日志/快照格式常量（magic、版本、记录字节数、状态行），等价于 Linux 的 `journalctl --version`。

测试总计：runtime 26 + supervisor 16 + rewrite 12 + integration 6 + subscribe 10 + metrics **10** + boot 11 + node-inspect 8 + journal **14** = **113 项**。

---

## Bug 修复详情

### `crates/gos-journal/src/lib.rs` — `decode_kind` (+4 个 match 臂)

**问题**：`gos-protocol::ControlPlaneMessageKind` 拥有 12 个变体，但 `decode_kind` 只处理了前 8 个（V2.3 引入 Subscribe 机制时协议已扩展，但 journal 层未同步）。

| 原始码值 | 变体 | 修复前 | 修复后 |
|----------|------|--------|--------|
| 0x09 | `MutationAudit` | `Err(UnknownKind(9))` | `Ok(MutationAudit)` |
| 0x0A | `CausalOverflow` | `Err(UnknownKind(10))` | `Ok(CausalOverflow)` |
| 0x0B | `RuleApplied` | `Err(UnknownKind(11))` | `Ok(RuleApplied)` |
| 0x0C | `SubscribeTriggered` | `Err(UnknownKind(12))` | `Ok(SubscribeTriggered)` |

**影响**：若日志文件中存在 Cypher mutation audit 事件、因果深度溢出事件、rewrite rule 触发事件或 Subscribe 触发事件，则 `replay()` 从第一个此类事件起就会提前终止并报错，之后所有事件均丢失。这是一个**静默数据丢失 bug**，在日志文件较长或有 V2.3 Subscribe 活动时尤为严重。

---

## 新增功能

### 1. `crates/k-shell/src/lib.rs` — `dispatch_journal_info` (+32 行)

在 `dispatch_metrics_export` 之后插入：

```rust
pub fn dispatch_journal_info(sink: &ConsoleSink) {
    // 报告 gos_journal 的全部格式常量
    // 包括：envelope magic/version/record_size
    //       snapshot magic/version/hdr_size/node_record/edge_record
    //       kinds 数量 + 状态行
}
```

**示例输出**（`journal` 命令）：

```
 journal format
  envelope magic:      GOSJ
  envelope version:    1
  header_bytes:        8
  envelope_record:     40 bytes (fixed)
  snapshot magic:      GOSS
  snapshot version:    1
  snapshot_hdr:        24 bytes
  node_record:         40 bytes
  edge_record:         40 bytes
  kinds:               12 (Hello..SubscribeTriggered)
  status:              F.4 control-plane journal -- replay-ready
```

### 2. `crates/k-shell/Cargo.toml` (+1 行)

```toml
gos-journal = { path = "../gos-journal" }
```

### 3. `crates/k-shell/src/proc.rs` (+3 行)

- help 文本新增：`  journal            journal format info and replay status`
- dispatch 分支新增：

```rust
} else if cmd == "journal" || cmd == "journal status" || cmd == "journal info" {
    super::dispatch_journal_info(sink);
```

---

## 新增测试 Harness

### `host-tests/gos-journal-harness/tests/journal.rs` — 14 项测试

| # | 测试名 | 验证点 |
|---|--------|--------|
| 1 | `journal_constants_are_correct` | ENVELOPE_RECORD_BYTES=40, HEADER_BYTES=8, 等 |
| 2 | `journal_header_roundtrip` | `write_into` → `parse` 得到相同值 |
| 3 | `all_twelve_kinds_survive_roundtrip` | **修复验证**：全部 12 种 kind 序列化 + 反序列化 |
| 4 | `replay_empty_journal` | 空 body → 0 条事件，无错误 |
| 5 | `replay_three_envelopes_in_order` | 3 条 envelope 按插入顺序重放 |
| 6 | `ring_append_flush_replay` | JournalRing<8> 追加 → flush → replay 全部还原 |
| 7 | `ring_full_returns_error` | 超出容量追加返回 Err |
| 8 | `ring_reset_and_reuse` | reset 后容量恢复，可重新追加 |
| 9 | `snapshot_header_roundtrip` | SnapshotHeader write + parse 一致 |
| 10 | `snapshot_node_roundtrip` | SnapshotNode write + parse 一致 |
| 11 | `snapshot_edge_roundtrip` | SnapshotEdge write + parse 一致 |
| 12 | `replay_snapshot_full_roundtrip` | 3节点2边快照完整重放 |
| 13 | `replay_bad_magic_returns_bad_header` | 错误 magic → `BadHeader` 错误 |
| 14 | `replay_trailing_bytes_returns_error` | 末尾残留字节 → `TrailingBytes` 错误 |

---

## 质量指标

| 指标 | 本次 | 前次（V2.10） |
|------|------|--------------|
| 测试总数 | **113** | 99 |
| Clippy 警告（新增） | **0** | 0 |
| 新增测试 | **+14**（journal harness 1-14） | +3 |
| 新增 Shell 命令 | **+1**（`journal`） | +1 |
| Bug 修复 | **+1**（decode_kind 缺失4个变体） | 0 |
| 受影响 crate | 3（gos-journal、k-shell、新 harness） | 2 |

---

## 图论 OS 特性维护

- **日志作为图的变更历史**：`ControlPlaneEnvelope` 记录每一次图拓扑变更（NodeUpsert / EdgeUpsert / StateDelta / RuleApplied / SubscribeTriggered）。修复后，replay 能完整重建图的完整历史，包括 Subscribe 和 Rewrite 产生的因果链事件，符合图论 OS 的 "everything is a graph event" 原则。
- **纯读取原则**：`dispatch_journal_info` 仅读取编译期常量，不访问运行时图状态，零 epoch 影响，符合 ADR-001 "read must be pure" 约束。
- **可观测性链路扩展**：serial log → runtime atomics → TUI panel → text export（`metrics export`）→ **journal format report（`journal`）**，五层覆盖。

---

## 下一步（V2.11 后续）

- [ ] VGA 层脏行追踪（epoch-diff hardware-level skip）
- [ ] `journal ring <N>` — 在运行时动态配置 JournalRing 容量
- [ ] PAL_U32 → 属性节点重构（Demo A 前置）
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
host-tests/gos-journal-harness:            14 passed, 0 failed  (+14 新增)

总计：113 项测试全绿
```

---

*自动生成于 2026-07-01 定期硬化任务（第12次）*
