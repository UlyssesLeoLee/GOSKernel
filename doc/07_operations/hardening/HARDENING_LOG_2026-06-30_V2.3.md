# GOS 自动硬化日志 — 2026-06-30（第4次，V2.2 Rewrite Engine 骨架）

> 类型：定期自动硬化（每2小时）  
> 目标：V2.2 Rewrite Engine 核心——`match → guard → emit` 骨架落地；协议层扩展 `RuleApplied` 遥测；全测试套件绿灯  
> 提交：`feat(v2.2): gos-rewrite crate — RewriteEngine match→guard→emit skeleton`

---

## 执行摘要

本次硬化交付 V2.2 的核心机制：**图重写引擎（RewriteEngine）**。新建 `crates/gos-rewrite`，实现"图模式匹配 → 守卫谓词 → 变更发射"三段式架构。

同步扩展了控制平面协议（`RuleApplied = 0x0B`），并在 `gos-runtime` 暴露了对应的遥测发射接口。新增测试覆盖 12 个核心断言，全套回归（runtime 26 + supervisor 16 + rewrite 12 = 54 项）零失败，workspace clippy 零警告。

---

## 变更详情

### 1. `crates/gos-protocol/src/lib.rs`

`ControlPlaneMessageKind` 新增一个变体：

| 枚举值 | 编码 | 语义 |
|---|---|---|
| `RuleApplied` | `0x0B` | 重写规则命中并成功施加 mutation；`subject` = 规则标签；`arg0` = 规则下标；`arg1` = mutation 后的 graph epoch |

### 2. `crates/gos-runtime/src/lib.rs`

两处改动：

#### `control_plane_kind_from_u8` 解码器

新增 `RuleApplied` 分支，保持解码器与协议枚举同步。

#### `pub fn emit_rule_applied(label: [u8; 16], rule_idx: u32, epoch_after: u64)`

```rust
// Before: 不存在
// After:
pub fn emit_rule_applied(label: [u8; 16], rule_idx: u32, epoch_after: u64) {
    RUNTIME.lock().emit_control_plane(
        ControlPlaneMessageKind::RuleApplied, label, rule_idx as u64, epoch_after
    );
}
```

供 supervisor 在调用 `apply_cypher_mutation` 成功后发射遥测，让 shell `where` 和串口日志可以追踪重写规则触发历史。

### 3. `crates/gos-rewrite/` (**新建 crate**)

**Cargo.toml** 依赖：`gos-protocol`, `gos-cypher-mut`（无 `std`，无 `spin`，完全 `no_std`）。

**src/lib.rs** — 核心类型体系：

#### `RewritePattern`

```rust
pub enum RewritePattern {
    NodePresent(NodeId),                               // 节点存在
    EdgeAbsent  { from, to, kind: ReceptiveEdgeKind }, // 边缺失 → 触发建边
    EdgePresent { from, to, kind: ReceptiveEdgeKind }, // 边存在 → 触发操作
    EpochGt(u64),                                      // epoch 超阈值
    EpochEq(u64),                                      // epoch 精确匹配
    Always,                                            // 无条件触发
}
```

#### `GuardFn` + `RewriteRule`

```rust
pub type GuardFn = fn(&GraphSnapshot) -> bool;

pub struct RewriteRule {
    pub pattern: RewritePattern,
    pub guard:   Option<GuardFn>,   // None = 只靠 pattern 决定
    pub action:  RewriteAction,
    pub label:   [u8; 16],          // 遥测 subject 字段
}
```

守卫是普通函数指针（非 closure），保证 `Copy` + const 可构造，可以静态初始化到 `Mutex<RewriteEngine>` 中。

#### `GraphView` 特征

```rust
pub trait GraphView {
    fn node_exists(&self, id: NodeId) -> bool;
    fn edge_exists(&self, from: NodeId, to: NodeId, kind: ReceptiveEdgeKind) -> bool;
    fn epoch(&self) -> u64;
    fn snapshot(&self) -> GraphSnapshot;
}
```

提供测试隔离缝（测试用 `MockGraphView`）和内核侧真实实现的解耦接口。V2.3 的 supervisor 将在 `service_system_cycle` 中通过此特征把 `RUNTIME.lock()` 视图传给引擎。

#### `RewriteEngine::apply_rules`

```rust
pub fn apply_rules<V: GraphView>(
    &mut self,
    view: &V,
    out: &mut [(usize, RewriteAction)],
) -> usize
```

逐条评估规则（pattern → guard），命中的规则将 `(rule_index, action)` 写入 `out`，返回命中数量。不要求内部 alloc。调用方（supervisor）依次调用 `apply_cypher_mutation + emit_rule_applied` 完成应用闭环。

### 4. `Cargo.toml`（workspace 根）

`crates/gos-rewrite` 加入 workspace members。

### 5. `host-tests/gos-rewrite-harness/` (**新建测试套件**)

12 个测试，覆盖全部模式分支和守卫逻辑：

| 测试 | 验证点 |
|---|---|
| `new_engine_starts_empty` | 初始状态 rule_count=0, fire_count=0 |
| `add_rule_increments_rule_count` | 规则注册返回正确下标 |
| `rule_table_full_returns_error` | 超出 64 条限额触发 `RuleTableFull` |
| `always_pattern_fires_unconditionally` | Always 无视图状态直接触发 |
| `node_present_fires_only_when_node_exists` | NodePresent 双向验证 |
| `edge_absent_fires_when_edge_missing` | EdgeAbsent 双向验证 |
| `edge_present_fires_when_edge_exists` | EdgePresent 双向验证 |
| `epoch_gt_fires_only_above_threshold` | EpochGt 边界值（等于不触发） |
| `epoch_eq_fires_at_exact_epoch` | EpochEq 精确匹配三值测试 |
| `guard_fn_vetoes_matching_pattern` | 守卫 false 阻断 pattern 匹配 |
| `multiple_rules_fire_in_same_cycle` | 单次 apply_rules 多规则同时命中 |
| `clear_resets_engine_to_empty` | clear() 重置规则表和计数器 |

---

## 与 V2.x 路线图的关联

| 阶段 | 状态 | 本次推进 |
|---|---|---|
| V2.0 边代数地基 | ✅ | — |
| V2.1 Cypher = ISA | ✅ | — |
| **V2.2 Rewrite Engine** | 🔄 **进行中** | 引擎骨架 + 协议 + 测试套件落地 |
| V2.3 Subscribe & 渲染 | 🔲 | — |
| V2.4 能力即可达性 | 🔲 | — |

### V2.2 剩余工作（下次硬化可启动）

1. **Supervisor 接入**：在 `service_system_cycle` 结束后调用 `RewriteEngine::apply_rules`，把命中的 mutation 送入 `apply_cypher_mutation`，并调用 `emit_rule_applied` 发遥测
2. **Boot manifest 静态图**：从 `hypervisor/src/main.rs` 启动序列提取为 `BootManifest` 声明，作为内核期望的初始图断言
3. **因果深度计**：用深度计替换 `service_system_cycle` 中的 `MAX_CYCLE_ITERATIONS = 2048` 硬上限（已有 `notify_causal_overflow` 桩）
4. **Quiescence 测试**：harness 验证"N 步必达静默"语义（无任何规则触发 = 当前帧已稳定）

---

## 测试结果

```
host-tests/gos-rewrite-harness:   12 passed, 0 failed  (新增)
host-tests/gos-runtime-harness:   26 passed, 0 failed
host-tests/gos-supervisor-harness: 16 passed, 0 failed
cargo clippy --workspace:          0 warnings
cargo check --workspace:           Finished (0 errors)

总计：54 项测试全绿
```

---

*自动生成于 2026-06-30 定期硬化任务（第4次）*
