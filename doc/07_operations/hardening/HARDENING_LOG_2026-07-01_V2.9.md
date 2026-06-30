# GOS 自动硬化日志 — 2026-07-01（第10次，V2.9 boot verify 命令）

> 类型：定期自动硬化（每2小时）  
> 目标：V2.9 可观测性 — `boot verify` / `boot status` Shell 命令（systemctl-status 类比）  
> 提交：`feat(v2.9): boot verify shell command + boot manifest report API`

---

## 执行摘要

本次硬化围绕 **启动完整性可观测性**，为 Shell 层新增 `boot verify` 命令，使操作员可以在运行时查询自启动时的依赖图自愈状态。同时在 `gos-runtime` 中引入轻量级的 boot 报告 API，遵循现有 `BOOT_FALLBACK_ALLOC_COUNT` 模式。

1. **`boot` / `boot verify` / `boot status`** — 显示启动清单边验证报告（类比 `systemctl status`）
2. **`gos_runtime::record_boot_manifest_report()`** — 启动时写入，shell 读取（无循环依赖）
3. **`gos_runtime::boot_manifest_rules_checked()`** / **`boot_manifest_edges_healed()`** — 两个读取器
4. **gos-boot-harness +3 测试**（tests 9-11）— 覆盖 round-trip、pending sentinel、healthy invariant

全部测试绿灯：runtime 26 + supervisor 16 + rewrite 12 + integration 6 + subscribe 10 + metrics 7 + boot **11** + node-inspect 8 = **96 项**。

---

## 架构动机

V2.6 已有 `verify_boot_manifest_graph()`（位于 `hypervisor/builtin_bundle.rs`），它在启动时自愈所有缺失的 Depend 边，并将结果写入串口日志。但此信息在启动后无法从 Shell 查询。

**问题**：操作员重启后无法知道启动清单是否健康运行，只能翻阅串口日志。

**方案**：遵循 `FaultDispatch`、`BOOT_FALLBACK_ALLOC_COUNT` 等现有模式，在 `gos-runtime` 中存储两个原子值（`AtomicU64`），由 `hypervisor` 在启动时写入，k-shell 在需要时读取。

**关键约束**：`k-shell` 在依赖层次上低于 `hypervisor`，不能直接调用 `builtin_bundle::verify_boot_manifest_graph()`。通过 gos-runtime 中间层完全解决了这个循环依赖问题。

---

## 变更详情

### 1. `crates/gos-runtime/src/lib.rs`（+20 行）

在 `BOOT_FALLBACK_ALLOC_COUNT` 块之后新增：

```rust
static BOOT_MANIFEST_RULES_CHECKED: AtomicU64 = AtomicU64::new(0);
static BOOT_MANIFEST_EDGES_HEALED:  AtomicU64 = AtomicU64::new(0);

pub fn record_boot_manifest_report(rules_checked: usize, edges_healed: usize);
pub fn boot_manifest_rules_checked() -> usize;
pub fn boot_manifest_edges_healed() -> usize;
```

复用已有的 `AtomicU64` import（无需新增 import）。

---

### 2. `crates/hypervisor/src/builtin_bundle.rs`（+4 行）

在 `verify_boot_manifest_graph()` 调用之后立即写入报告：

```rust
let manifest_report = verify_boot_manifest_graph();
gos_runtime::record_boot_manifest_report(
    manifest_report.rules_checked,
    manifest_report.edges_healed,
);
```

串口日志输出不变，仅追加了对原子值的写操作。

---

### 3. `crates/k-shell/src/lib.rs`（+33 行）

新增 `pub fn dispatch_boot_verify(sink: &ConsoleSink)`:

```
 boot manifest
  rules checked: 27
  edges healed:  0
  status:        OK — all 27 depend edges present
```

颜色编码：
| 颜色 | 状态 |
|------|------|
| 绿(10) | OK — 所有 edges 均存在（healed == 0, rules > 0） |
| 黄(14) | pending — 启动尚未完成（rules == 0） |
| 红(12) | WARNING — N 条 edges 在启动时被自愈（imperative pass 有漏洞） |

---

### 4. `crates/k-shell/src/proc.rs`（+3 行）

在 `nodes summary` 分支之后：

```rust
} else if cmd == "boot" || cmd == "boot verify" || cmd == "boot status" {
    super::dispatch_boot_verify(sink);
```

help 文本同步更新：
```
  boot verify        boot manifest edge verification report
```

---

### 5. `host-tests/gos-boot-harness/tests/boot.rs`（+44 行，3 项新测试）

| # | 测试名 | 验证点 |
|---|--------|--------|
| 9 | `record_boot_manifest_report_stores_and_reads_back` | round-trip 正确；覆盖 rules=27/healed=0 和 healed=3 两种写入 |
| 10 | `boot_manifest_report_zero_rules_indicates_pending` | rules=0 表示 pending（启动未完成） |
| 11 | `boot_manifest_healthy_when_zero_healed_and_nonzero_rules` | rules>0 && healed==0 = 健康启动 |

---

## 质量指标

| 指标 | 本次 | 前次（V2.8） |
|------|------|--------------|
| 测试总数 | **96** | 93 |
| Clippy 警告 | **0** | 0 |
| 新增测试 | **+3**（boot harness） | +8 |
| 新增 Shell 命令 | **+1**（`boot verify`） | +3 |
| 受影响 crate | 3（runtime/hypervisor/k-shell） | 2 |

---

## 图论 OS 特性维护

- **无循环依赖**：`gos-runtime` 作为中间存储层，保持 k-shell → runtime ← hypervisor 的单向依赖结构
- **原子存储**：`AtomicU64` 对 bare-metal 单核内核零开销，与 `BOOT_FALLBACK_ALLOC_COUNT` 完全一致
- **write-once 语义**：`record_boot_manifest_report` 在 `boot_builtin_graph` 中仅调用一次，之后只读——无并发写争用
- **可观测性链路**：serial log → runtime atomics → shell display，三层冗余覆盖（串口审计、运行时查询、交互式 Shell）

---

## 下一步（V2.9 后续）

- [ ] VGA 层脏行追踪（epoch-diff hardware-level skip）
- [ ] PAL_U32 → 属性节点重构（Demo A 前置）
- [ ] `metrics export` 命令（将 telemetry 写入 FAT32 日志节点）
- [ ] V3 生态层规划（见 `plan/V3_DEVELOPMENT_PLAN.md`）

---

## 测试结果

```
host-tests/gos-runtime-harness:              26 passed, 0 failed
host-tests/gos-supervisor-harness:          16 passed, 0 failed
host-tests/gos-rewrite-harness:             12 passed, 0 failed
host-tests/gos-rewrite-integration-harness:  6 passed, 0 failed
host-tests/gos-subscribe-harness:           10 passed, 0 failed
host-tests/gos-metrics-harness:              7 passed, 0 failed
host-tests/gos-boot-harness:               11 passed, 0 failed  (+3 新增)
host-tests/gos-node-inspect-harness:         8 passed, 0 failed

总计：96 项测试全绿
```

---

*自动生成于 2026-07-01 定期硬化任务（第10次）*
