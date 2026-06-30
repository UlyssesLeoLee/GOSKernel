# GOS 自动硬化日志 — 2026-06-30（第6次，V2.3 Subscribe 反应式机制）

> 类型：定期自动硬化（每2小时）  
> 目标：V2.3 核心交付——Subscribe 触发语义 + Epoch-diff 空帧跳过基础  
> 提交：`feat(v2.3): Subscribe reactive mechanism — fire_subscribers + epoch-diff idle tracking`

---

## 执行摘要

本次硬化交付 V2.3 的**核心原语**：

1. **Subscribe 协议扩展** — 新增 `SubscribeTriggered = 0x0C` 控制平面事件，定义观察-订阅对的有线格式：`subject` = 被观察节点 ID；`arg0` = 订阅方节点 ID（低64位LE）；`arg1` = 新 `graph_epoch`。
2. **Runtime Subscribe 表** — `GraphRuntime` 内嵌 64 槽订阅对数组；`register_subscribe_pair`（幂等）；`fire_subscribers` 在每次 `graph_epoch` 递增后自动触发（`register_node`、`register_edge`、`unregister_edge` 三条结构变更路径全覆盖）。
3. **Supervisor Epoch-diff 跟踪** — `LAST_RENDER_EPOCH` + `IDLE_CYCLE_COUNT` 两个原子计数器，`service_system_cycle` 末尾比对 epoch；epoch 不变则累加 `IDLE_CYCLE_COUNT`（"零空帧"渲染基础设施）。
4. **Subscribe 测试套件** — 新建 `host-tests/gos-subscribe-harness`，6 项测试覆盖全部合约点。

全部测试绿灯：runtime 26 + supervisor 16 + rewrite 12 + integration 6 + **subscribe 6** = **66 项**，workspace clippy 零警告。

---

## 变更详情

### 1. `crates/gos-protocol/src/lib.rs`（+6 行）

```rust
/// A reactive Subscribe pair was triggered: the observed node was
/// structurally mutated (graph_epoch bumped by register_node,
/// register_edge, or unregister_edge).  `subject` = observed NodeId
/// (16 bytes); `arg0` = lower 8 bytes of subscriber NodeId (LE u64);
/// `arg1` = new graph_epoch after the mutation.
SubscribeTriggered = 0x0C,
```

控制平面消息类型 `0x0C` 预留为 Subscribe 触发遥测，与 V2.1 的 `MutationAudit` 和 V2.2 的 `RuleApplied` 同层级。

### 2. `crates/gos-runtime/src/lib.rs`（+72 行）

#### Subscribe 表结构

```rust
pub const MAX_SUBSCRIBE_PAIRS: usize = 64;

// RuntimeError 新增变体
SubscribeTableFull,

// GraphRuntime 新增字段
subscribe_pairs: [Option<(NodeId, NodeId)>; MAX_SUBSCRIBE_PAIRS],
```

#### `register_subscribe_pair`（幂等）

```rust
pub fn register_subscribe_pair(&mut self, observed: NodeId, subscriber: NodeId) -> Result<(), RuntimeError> {
    for pair in self.subscribe_pairs.iter().flatten() {
        if pair.0 == observed && pair.1 == subscriber { return Ok(()); }
    }
    let slot = self.subscribe_pairs.iter_mut().find(|s| s.is_none())
        .ok_or(RuntimeError::SubscribeTableFull)?;
    *slot = Some((observed, subscriber));
    Ok(())
}
```

重复注册同一对时直接 `Ok(())` 返回，避免上层重复注册时膨胀槽位。

#### `fire_subscribers`（两阶段，避免借用冲突）

```rust
fn fire_subscribers(&mut self, changed: NodeId, epoch: u64) {
    let mut subs = [NodeId::ZERO; MAX_SUBSCRIBE_PAIRS];
    let mut count = 0usize;
    for pair in self.subscribe_pairs.iter().flatten() {
        if pair.0 == changed && count < MAX_SUBSCRIBE_PAIRS {
            subs[count] = pair.1;
            count += 1;
        }
    }
    for sub in subs[..count].iter().copied() {
        let arg0 = u64::from_le_bytes([sub.0[0], sub.0[1], sub.0[2], sub.0[3],
            sub.0[4], sub.0[5], sub.0[6], sub.0[7]]);
        self.emit_control_plane(ControlPlaneMessageKind::SubscribeTriggered, changed.0, arg0, epoch);
    }
}
```

先收集再发射，避免在 `&mut self` 上同时持有不可变迭代引用和可变发射路径。

#### 自动触发点

| 方法 | 触发时机 |
|---|---|
| `register_node` | epoch 递增后 `fire_subscribers(spec.node_id, epoch)` |
| `register_edge` | epoch 递增后 `fire_subscribers(from_node)` + `fire_subscribers(to_node)` |
| `unregister_edge` | 先保存 from/to，移除后 epoch 递增，再双向 fire |

#### 公开模块级 API

```rust
pub fn register_subscribe(observed: NodeId, subscriber: NodeId) -> Result<(), RuntimeError> {
    RUNTIME.lock().register_subscribe_pair(observed, subscriber)
}
```

### 3. `crates/gos-supervisor/src/lib.rs`（+37 行）

#### Epoch-diff 原子计数器

```rust
static LAST_RENDER_EPOCH: AtomicU64 = AtomicU64::new(u64::MAX);
static IDLE_CYCLE_COUNT:  AtomicU64 = AtomicU64::new(0);
```

`LAST_RENDER_EPOCH` 初始为 `u64::MAX`，确保第一帧始终视为"有变化"而非空帧。

#### `service_system_cycle` 末尾 epoch 比对

```rust
let epoch_now = gos_runtime::graph_epoch();
let prev_epoch = LAST_RENDER_EPOCH.swap(epoch_now, Ordering::Relaxed);
if epoch_now == prev_epoch {
    IDLE_CYCLE_COUNT.fetch_add(1, Ordering::Relaxed);
}
```

#### 公开访问器

```rust
pub fn render_epoch() -> u64    { LAST_RENDER_EPOCH.load(Ordering::Relaxed) }
pub fn idle_cycle_count() -> u64 { IDLE_CYCLE_COUNT.load(Ordering::Relaxed) }
```

### 4. `host-tests/gos-subscribe-harness/`（新建）

6 项测试，覆盖 Subscribe 合约全部维度：

| 测试 | 验证点 |
|---|---|
| `subscribe_pair_registration_succeeds_and_is_idempotent` | 基础注册 + 幂等重注册 |
| `subscribe_triggered_fires_when_observed_node_gets_new_edge` | 观察节点结构变更 → `SubscribeTriggered` 发射 |
| `no_subscribe_triggered_for_mutation_to_non_observed_node` | 无关节点变更 → 不触发观察节点的订阅 |
| `multiple_subscribers_both_notified` | 同一观察节点多订阅方 → 全部收到通知 |
| `subscribe_triggered_arg1_is_new_graph_epoch` | `arg1` 字段精确等于变更后的 `graph_epoch` |
| `idle_cycle_count_increments_when_graph_stable` | 图拓扑稳定时 `idle_cycle_count` 持续递增 |

---

## V2.3 退出判据核查

| 判据 | 状态 |
|---|---|
| `SubscribeTriggered` 协议事件（`0x0C`） | ✅ **本次完成** |
| Subscribe 表注册（幂等，MAX 64 对） | ✅ **本次完成** |
| `fire_subscribers` 三条变更路径覆盖 | ✅ **本次完成** |
| Epoch-diff 空帧计数基础设施 | ✅ **本次完成** |
| Subscribe 测试套件（6 项） | ✅ **本次完成** |
| Boot manifest 静态图 | 🔲 → V2.3 后续 |
| 因果深度计（替换硬上限 2048） | 🔲 → V2.3 后续 |
| Demo A: theme 0-line 传播 | 🔲 → V2.3 后续 |

---

## V2.x 路线图

| 阶段 | 状态 |
|---|---|
| V2.0 边代数地基 | ✅ |
| V2.1 Cypher = ISA + MutationAudit | ✅ |
| V2.2 Rewrite Engine + Supervisor 接入 | ✅ |
| **V2.3 Subscribe & 渲染** | 🔶 核心原语完成，后续工作待下次 |
| V2.4 能力即可达性 | 🔲 |

### V2.3 下一步（可立即启动）

1. **Boot manifest** — 从 `hypervisor/src/main.rs` 启动序列提取声明式节点/边期望，用 `EdgeAbsent` 规则在启动帧自动补全缺失边
2. **因果深度计** — 用测量深度计替换 `MAX_CYCLE_ITERATIONS = 2048` 硬上限（`notify_causal_overflow` 桩已存在）
3. **Demo A: theme 0-line 传播** — 将 `theme.wabi` / `shoji` 常量提升为数据节点，Subscribe 触发时重绘
4. **Epoch-diff 渲染跳过** — 在 shell `where` 层比对 `render_epoch()` 实现真正零空帧渲染

---

## 测试结果

```
host-tests/gos-runtime-harness:              26 passed, 0 failed
host-tests/gos-supervisor-harness:          16 passed, 0 failed
host-tests/gos-rewrite-harness:             12 passed, 0 failed
host-tests/gos-rewrite-integration-harness:  6 passed, 0 failed
host-tests/gos-subscribe-harness:            6 passed, 0 failed  (新增)

cargo clippy --workspace:  0 warnings
cargo check --workspace:   Finished (0 errors)

总计：66 项测试全绿
```

---

*自动生成于 2026-06-30 定期硬化任务（第6次）*
