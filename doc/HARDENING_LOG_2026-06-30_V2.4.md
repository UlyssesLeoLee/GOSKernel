# GOS 自动硬化日志 — 2026-06-30（第5次，V2.2 Supervisor 接入 + Quiescence 闭环）

> 类型：定期自动硬化（每2小时）  
> 目标：V2.2 剩余工作——Supervisor 接入 RewriteEngine、RuntimeGraphView 桥接、Quiescence 集成测试套件  
> 提交：`feat(v2.2): supervisor rewrite pass — RuntimeGraphView + quiescence harness (6 tests)`

---

## 执行摘要

本次硬化交付 V2.2 的**最后两项**工作：

1. **Supervisor 接入** — `service_system_cycle` 末尾新增 Rewrite Pass：每帧调用 `REWRITE_ENGINE.lock().apply_rules(RuntimeGraphView, &mut out)`，对每条命中规则调用 `apply_cypher_mutation`（走完整审计路径）并发射 `RuleApplied` 控制平面遥测。
2. **Quiescence 集成测试** — 新建 `host-tests/gos-rewrite-integration-harness`，6 项测试验证"端到端从规则到 `RuleApplied` envelope"合约，重点覆盖 `EdgeAbsent` 规则的静默语义（第1帧触发→建边→第2帧不再触发）。

全部测试绿灯：runtime 26 + supervisor 16 + rewrite 12 + integration 6 = **60 项**，workspace clippy 零警告。

V2.2 所有退出判据已满足，本阶段正式关闭。

---

## 变更详情

### 1. `crates/gos-runtime/src/lib.rs`（+55 行）

#### `GraphRuntime::node_exists_by_id`

```rust
pub fn node_exists_by_id(&self, id: NodeId) -> bool {
    self.node_slot_by_id(id).is_some()
}
```

#### `GraphRuntime::edge_exists_by_kind`

```rust
pub fn edge_exists_by_kind(
    &self,
    from: NodeId,
    to: NodeId,
    kind: gos_cypher_mut::ReceptiveEdgeKind,
) -> bool {
    let edge_type = match kind {
        ReceptiveEdgeKind::Mount => RuntimeEdgeType::Mount,
        ReceptiveEdgeKind::Use   => RuntimeEdgeType::Use,
    };
    self.edges.iter().flatten().any(|rec| {
        rec.spec.from_node == from
            && rec.spec.to_node == to
            && rec.spec.edge_type == edge_type
    })
}
```

#### 新增模块级函数

```rust
pub fn node_exists_by_id(id: NodeId) -> bool { ... }
pub fn edge_exists_by_kind(from, to, kind) -> bool { ... }
```

这两个函数是 `RuntimeGraphView` 在 supervisor 中评估 `NodePresent`、`EdgePresent`、`EdgeAbsent` 模式时的唯一入口，每次独立加锁—不在 `apply_rules` 期间持有锁。

### 2. `crates/gos-supervisor/Cargo.toml`

新增两个依赖：

```toml
gos-rewrite    = { path = "../gos-rewrite" }
gos-cypher-mut = { path = "../gos-cypher-mut" }
```

### 3. `crates/gos-supervisor/src/lib.rs`（+80 行）

#### `RuntimeGraphView` 适配器

```rust
struct RuntimeGraphView;

impl GraphView for RuntimeGraphView {
    fn node_exists(&self, id: NodeId) -> bool        { gos_runtime::node_exists_by_id(id) }
    fn edge_exists(&self, from, to, kind) -> bool    { gos_runtime::edge_exists_by_kind(from, to, kind) }
    fn epoch(&self) -> u64                           { gos_runtime::graph_epoch() }
    fn snapshot(&self) -> GraphSnapshot              { gos_runtime::snapshot() }
}
```

轻量桥接：不持有 runtime 锁跨帧，每次查询独立获取/释放，内核单线程无竞态。

#### `REWRITE_ENGINE` 全局静态

```rust
static REWRITE_ENGINE: Mutex<RewriteEngine> = Mutex::new(RewriteEngine::new());
```

与 `SUPERVISOR` 独立，两把锁可以互不干扰地各自加锁。

#### `service_system_cycle` Rewrite Pass

```rust
// V2.2: 每帧 dispatch loop 结束后执行一次规则求值
{
    let mut out = [(0usize, sentinel_action); MAX_REWRITE_RULES];
    let fired = REWRITE_ENGINE.lock().apply_rules(&RuntimeGraphView, &mut out);
    for (rule_idx, action) in out[..fired].iter().copied() {
        let _ = gos_runtime::apply_cypher_mutation(action.mutation, action.source);
        let epoch_after = gos_runtime::graph_epoch();
        gos_runtime::emit_rule_applied(action.source, rule_idx as u32, epoch_after);
    }
}
```

#### 公开 API

```rust
pub fn add_rewrite_rule(rule: RewriteRule) -> Result<usize, RewriteError>
pub fn clear_rewrite_rules()
```

供 hypervisor 启动序列和内核模块注册规则，不需要直接接触 `REWRITE_ENGINE`。

### 4. `host-tests/gos-rewrite-integration-harness/`（新建）

6 项集成测试，验证端到端合约：

| 测试 | 验证点 |
|---|---|
| `add_and_clear_rules_roundtrip` | add / clear 往返，slot 复用 |
| `service_cycle_always_rule_emits_rule_applied` | Always 规则命中 → `RuleApplied` envelope |
| `edge_absent_rule_quiesces_after_one_cycle` | **Quiescence 核心**：第1帧触发 → 第2帧静默 |
| `always_rule_fires_every_cycle` | Always 规则每帧都触发 |
| `no_rules_no_rule_applied_envelope` | 无规则 → 无 envelope |
| `epoch_gt_rule_fires_when_epoch_exceeds_threshold` | EpochGt 语义与运行时 epoch 联动正确 |

`edge_absent_rule_quiesces_after_one_cycle` 是本次硬化最关键的测试：它验证了"图重写引擎 + 运行时写入 + 再次查询"的完整闭环——规则自行消除了触发它的条件，从而达到 quiescence。

---

## V2.2 退出判据核查

| 判据 | 状态 |
|---|---|
| `RewriteEngine` 骨架（match → guard → emit） | ✅ 第4次硬化 |
| **Supervisor 接入（service_system_cycle 内）** | ✅ **本次完成** |
| `RuleApplied` 控制平面遥测 | ✅ 第4次硬化（协议）+ 本次（端到端验证） |
| **Quiescence 测试** | ✅ **本次完成** |

V2.2 全部退出判据满足，正式关闭 V2.2。

---

## V2.x 路线图

| 阶段 | 状态 |
|---|---|
| V2.0 边代数地基 | ✅ |
| V2.1 Cypher = ISA | ✅ |
| **V2.2 Rewrite Engine** | ✅ **本次完成** |
| **V2.3 Subscribe & 渲染** | 🔲 → 可开始 |
| V2.4 能力即可达性 | 🔲 |

### V2.3 下一步（可立即启动）

1. **Subscribe 触发语义**：`graph_epoch` 变化时 → 推送 `SubscribeTriggered` 控制平面事件，让订阅方无需轮询
2. **Epoch diff renderer**：shell `where` 命令比较当前 epoch 与上次渲染 epoch，跳过空帧，仅在拓扑变化时重绘
3. **Boot manifest 静态图**：从 `hypervisor/src/main.rs` 启动序列提取为 `BootManifest`（声明式节点/边期望），用 `EdgeAbsent` 规则在启动帧自动补全缺失边
4. **因果深度计**：用深度计替换 `MAX_CYCLE_ITERATIONS = 2048` 硬上限（已有 `notify_causal_overflow` 桩）

---

## 测试结果

```
host-tests/gos-runtime-harness:              26 passed, 0 failed
host-tests/gos-supervisor-harness:          16 passed, 0 failed
host-tests/gos-rewrite-harness:             12 passed, 0 failed
host-tests/gos-rewrite-integration-harness:  6 passed, 0 failed  (新增)

cargo clippy --workspace:  0 warnings
cargo check --workspace:   Finished (0 errors)

总计：60 项测试全绿
```

---

*自动生成于 2026-06-30 定期硬化任务（第5次）*
