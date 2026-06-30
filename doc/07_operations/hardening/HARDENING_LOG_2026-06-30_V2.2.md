# GOS 自动硬化日志 — 2026-06-30（第3次，V2.1 收尾 + V2.2 地基）

> 类型：定期自动硬化（每2小时）  
> 目标：完成 V2.1 剩余工作——接通 fault attribution audit 路径；决定 ADR-004 可见性语义；V2.2 causal overflow telemetry 前置  
> 提交：`32b10ae` `feat(v2.1): audit path live — MutationAudit envelope + ADR-004 epoch semantics`

---

## 执行摘要

本次硬化完成了 V2.1 最后两项未决工作：**fault attribution / audit 路径接通**，以及 **ADR-004 mutation 可见性语义批准**。同时为 V2.2 "Rewrite Engine"埋下 causal overflow telemetry 基础。

全部 26 项 runtime harness 测试（含1项新增）和 16 项 supervisor harness 测试通过，workspace clippy 零警告。

---

## 变更详情

### 1. `crates/gos-protocol/src/lib.rs`

`ControlPlaneMessageKind` 新增两个变体：

| 枚举值 | 编码 | 语义 |
|---|---|---|
| `MutationAudit` | `0x09` | 每次成功的 Cypher write 产生一个，subject = source 标签，arg0/arg1 编码 mutation 操作数 |
| `CausalOverflow` | `0x0A` | `service_system_cycle` 触发每帧迭代上限时产生，arg0 = 触发时的迭代深度 |

同步更新 `control_plane_kind_from_u8` 解码函数。

### 2. `crates/gos-cypher-mut/src/lib.rs`

`AuditedMutation::to_envelope()` 改用 `ControlPlaneMessageKind::MutationAudit`（此前错误地用了 `EdgeUpsert`）。

### 3. `crates/gos-runtime/src/lib.rs`（+40 行）

#### `apply_cypher_mutation(mutation, source) -> Result<AuditedMutation, MutationError>`

**接口变更**（前后对比）：

```rust
// Before (V2.1 初始实现)
pub fn apply_cypher_mutation(
    mutation: CypherMutation,
) -> Result<(), MutationError>

// After (V2.1 收尾)
pub fn apply_cypher_mutation(
    mutation: CypherMutation,
    source: [u8; 16],             // 新增：调用方身份标签
) -> Result<AuditedMutation, MutationError>  // 新增：返回审计记录
```

实现流程：
1. `apply_mutation(&mut rt, mutation)?` — 同前，dispatch 前 validation
2. `AuditedMutation { mutation, source, tick: rt.tick }` — 打时间戳
3. `rt.emit_control_plane(MutationAudit, ...)` — 推入控制平面队列
4. 返回 `Ok(audited)` — 调用方可写入 journal ring

**两种 envelope 的关系**（已在新测试中验证）：
- `register_edge` 内部产生 `EdgeUpsert`（结构拓扑变化，原有行为）
- `apply_cypher_mutation` 额外产生 `MutationAudit`（谁在什么时刻触发，用于 audit）

两者互补，而非重复。

#### `pub fn runtime_tick() -> u64`

暴露当前 dispatch cycle 计数器，供无壁钟源的裸机调用方（如 shell 命令）使用。

#### `pub fn notify_causal_overflow(depth: u32)`

发布 `CausalOverflow` 控制平面事件，供 `service_system_cycle` 在触发迭代上限时调用。

### 4. `crates/gos-supervisor/src/lib.rs`

`service_system_cycle` 达到 `MAX_CYCLE_ITERATIONS = 2048` 时，改为：

```rust
// Before: 静默 break
break;

// After: 发 telemetry 后 break
gos_runtime::notify_causal_overflow(iter);
break;
```

这是 V2.2 causal depth 可观测性的第一步，后续 V2.2 会用因果深度计替换硬上限。

### 5. `host-tests/gos-runtime-harness/tests/runtime.rs`（+80 行）

#### 更新 4 个 `apply_cypher_mutation` 调用

所有调用新增 `source: *b"TEST\0\0\0\0\0\0\0\0\0\0\0\0"` 参数。

#### 新增 `v2_1_apply_cypher_mutation_emits_mutation_audit_envelope`

验证：
- `apply_cypher_mutation` 成功后，`AuditedMutation.source` 与传入的 source 标签一致
- 控制平面队列中先出 `EdgeUpsert`（结构 telemetry），再出 `MutationAudit`（audit trail）
- `MutationAudit.subject` 正确携带 source 标签
- 单次 mutation 之后队列无额外 envelope

### 6. `doc/ADR-004-mutation-visibility.md`（新建）

批准 **epoch-published** 为 GOS Cypher mutation 可见性语义：

**核心约定**：
- mutation 在单次 `RUNTIME.lock()` 持有期内完成全部 runtime 修改
- `graph_epoch` 仅由结构 mutation 递增（add/remove node/edge）
- V2.3 renderer 比较 epoch 跳过空帧（Demo #2 基础）
- Subscribe 触发条件 = epoch 变化（V2.3 的精确定义）
- capability 路径查询基于最新 epoch 快照（V2.4 无竞态）

`graph_epoch`（图拓扑版本）与 `runtime.tick`（dispatch cycle 计数）分工明确，ADR 在此钉死分工，消除后续 V2.3/V2.4 的歧义。

---

## 与 V2.x 路线图的关联

| 阶段 | 状态 | 本次推进 |
|---|---|---|
| V2.0 边代数地基 | ✅ | — |
| **V2.1 Cypher = ISA** | ✅ **完成** | audit path 接通；ADR-004 批准；所有退出判据已满足 |
| **V2.2 Rewrite Engine** | 🔲 → **可开始** | CausalOverflow telemetry 埋点；graph_epoch/tick 分工 ADR 固化 |
| V2.3 Subscribe & 渲染 | 🔲 | — |
| V2.4 能力即可达性 | 🔲 | — |

### V2.1 退出判据核查

| 判据 | 状态 |
|---|---|
| 1 NodeKind + 1 EdgeKind 端到端（AddEdge → 可见） | ✅ 第2次硬化 |
| mutation 下一 cycle 可见 | ✅ immediate（ADR-004 已确认 epoch-published 不影响此行为） |
| fault 注入测试绿（UnknownEndpoint） | ✅ 既有 |
| **audit log 有记录** | ✅ **本次完成** |

V2.1 全部退出判据满足，正式关闭 V2.1。

### V2.2 下一步（可立即启动）

1. `RewriteEngine` 骨架（`match → guard → emit`）— 新 crate `gos-rewrite`
2. boot manifest 静态图 — 从 `hypervisor/src/main.rs` 提取启动序列
3. 因果深度计替换 2048 hardcap（已有 `notify_causal_overflow` 桩）
4. quiescence test harness（`N 步必达静默`）

---

## 测试结果

```
host-tests/gos-runtime-harness:   26 passed, 0 failed  (+1 新增)
host-tests/gos-supervisor-harness: 16 passed, 0 failed
cargo clippy --workspace:          0 warnings
cargo check --workspace:           Finished (0 errors)
```

---

*自动生成于 2026-06-30 定期硬化任务（第3次）*
