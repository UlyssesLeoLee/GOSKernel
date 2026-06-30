# GOS 自动硬化日志 — 2026-06-30（第8次，V2.6 Boot Manifest Static Graph）

> 类型：定期自动硬化（每2小时）  
> 目标：V2.6 自愈 — Boot Manifest 静态图（EdgeAbsent 规则 × 27 + ReceptiveEdgeKind::Depend）  
> 提交：`feat(v2.6): boot manifest static graph — 27 EdgeAbsent self-heal rules + gos-boot-harness`

---

## 执行摘要

本次硬化将启动时的插件依赖图从**纯命令式构建**升级为**声明式 EdgeAbsent 重写规则集**，实现机器可读的依赖规范与自动自愈：

1. **`ReceptiveEdgeKind::Depend = 3`** — 扩展 cypher-mut 枚举，支持启动时 Depend 边
2. **`BOOT_MANIFEST_RULES` (27 条)** — 将所有插件依赖边编码为 EdgeAbsent 重写规则
3. **`verify_boot_manifest_graph()`** — 迭代规则集，自愈缺失 Depend 边，输出结构化报告
4. **`gos-boot-harness`** — 8 项测试验证 EdgeAbsent 自愈机制的完整语义
5. **零 clippy 警告** — `boot_manifest_rules()` 驱动 verify 循环，报告字段输出到串口

全部测试绿灯：runtime 26 + supervisor 16 + rewrite 12 + integration 6 + subscribe 10 + metrics 7 + **boot 8** = **85 项**。

---

## 架构动机

原有 `synchronize_manifest_graph()` 以命令式代码构建 Depend 边，无法被机器检查。  
图论操作系统的核心原则是**一切皆图变换** — 依赖关系本身应声明为图结构，
而非藏在过程代码里。

引入 `BOOT_MANIFEST_RULES` 后：

- 依赖图有了**规范的机器可读形式**（Rust `static` 数组）
- `RewritePattern::EdgeAbsent` 在边缺失时自动触发 `AddEdge(Depend)` 修复
- `BootManifestReport` 提供结构化审计信息（串口可见）
- 测试可以直接验证规则语义，而不必 mock 启动序列

---

## 变更详情

### 1. `crates/gos-cypher-mut/src/lib.rs`

#### 新增 `ReceptiveEdgeKind::Depend = 3`

```rust
pub enum ReceptiveEdgeKind {
    Mount = 1,
    Use = 2,
    /// Boot-manifest self-repair only. Maps to `RuntimeEdgeType::Depend`.
    Depend = 3,
}
```

`pre_validate` 更新为接受 `Depend` 在 `AddEdge` mutation 中合法：

```rust
ReceptiveEdgeKind::Mount | ReceptiveEdgeKind::Use | ReceptiveEdgeKind::Depend => Ok(()),
```

### 2. `crates/gos-runtime/src/lib.rs`

在两处 match 臂中补全 `Depend` 映射到 `RuntimeEdgeType::Depend`：

**`edge_exists_by_kind`：**
```rust
gos_cypher_mut::ReceptiveEdgeKind::Depend => RuntimeEdgeType::Depend,
```

**`MutationDispatcher::add_edge`：**
```rust
gos_cypher_mut::ReceptiveEdgeKind::Depend => (RuntimeEdgeType::Depend, "manifest.depend"),
```

### 3. `crates/hypervisor/Cargo.toml`

添加依赖：
```toml
gos-rewrite = { path = "../gos-rewrite" }
```

### 4. `crates/hypervisor/src/builtin_bundle.rs`（核心）

#### `const fn boot_dep_rule` — 编译期规则构造器

```rust
const fn boot_dep_rule(from: NodeId, to: NodeId, label: [u8; 16]) -> RewriteRule {
    RewriteRule {
        pattern: RewritePattern::EdgeAbsent { from, to, kind: ReceptiveEdgeKind::Depend },
        guard: None,
        action: RewriteAction {
            mutation: CypherMutation::AddEdge { from, to, edge_kind: ReceptiveEdgeKind::Depend },
            source: *b"boot.manifest\0\0\0",
        },
        label,
    }
}
```

#### `static BOOT_MANIFEST_RULES: [RewriteRule; 27]` — 全部插件依赖边

覆盖 13 个插件间的 27 条有向依赖边：

| 插件 | 依赖于 | 条数 |
|------|--------|------|
| PIT  | PIC    | 1    |
| PS2  | PIC    | 1    |
| IDT  | GDT, PIT, PS2 | 3 |
| VMM  | PMM    | 1    |
| HEAP | PMM, VMM | 2  |
| NET  | VGA    | 1    |
| MOUSE | VGA, PS2, IDT | 3 |
| CYPHER | VGA  | 1    |
| CUDA | VGA, SERIAL | 2 |
| SHELL | VGA, PS2, HEAP, IME, NET, CYPHER, CUDA | 7 |
| CHAT | VGA, NET | 2  |
| NIM  | VGA, NET | 2  |
| AI   | SHELL  | 1    |
| **合计** | | **27** |

#### `pub struct BootManifestReport` + `verify_boot_manifest_graph()`

```rust
pub struct BootManifestReport {
    pub rules_checked: usize,
    pub edges_healed: usize,
}

impl BootManifestReport {
    pub fn is_healthy(&self) -> bool { self.edges_healed == 0 }
}

pub fn verify_boot_manifest_graph() -> BootManifestReport {
    let mut healed = 0usize;
    for rule in boot_manifest_rules() {            // drives boot_manifest_rules() call
        if let RewritePattern::EdgeAbsent { from, to, kind } = rule.pattern {
            if !gos_runtime::edge_exists_by_kind(from, to, kind) {
                let _ = gos_runtime::apply_cypher_mutation(rule.action.mutation, rule.action.source);
                healed += 1;
            }
        }
    }
    BootManifestReport { rules_checked: BOOT_MANIFEST_RULE_COUNT, edges_healed: healed }
}
```

#### 串口报告集成（`boot_builtin_graph`）

```rust
let manifest_report = verify_boot_manifest_graph();
crate::raw_serial_println(format_args!(
    "boot.manifest: checked={} healed={}{}",
    manifest_report.rules_checked,
    manifest_report.edges_healed,
    if manifest_report.is_healthy() { "" } else { " [WARN: edges healed — imperative pass missed edges]" },
));
```

### 5. `host-tests/gos-boot-harness/`（新增）

**`Cargo.toml`：** 依赖 gos-protocol, gos-cypher-mut, gos-rewrite, gos-runtime, gos-sign, gos-supervisor(host-testing)

**`.cargo/config.toml`：** 覆盖目标为 `x86_64-pc-windows-msvc`（绕过内核交叉编译目标）

**`tests/boot.rs`：** 8 项测试：

| # | 测试名 | 验证点 |
|---|--------|--------|
| 1 | `boot_manifest_rule_count_is_27` | 规则数量等于声明依赖边数 |
| 2 | `all_boot_manifest_rules_use_depend_kind` | pattern 和 mutation 均使用 `Depend` |
| 3 | `depend_edge_absent_before_creation` | 创建前 `edge_exists_by_kind` 返回 false |
| 4 | `apply_mutation_creates_depend_edge` | mutation 成功创建 Depend 边 |
| 5 | `edge_absent_rule_quiesces_after_edge_created` | 边存在时规则不触发，epoch 不变 |
| 6 | `edge_absent_rule_heals_missing_depend_edge` | service_cycle 自愈缺失边 |
| 7 | `depend_edge_creation_is_idempotent` | 重复创建安全，epoch 不回退 |
| 8 | `mount_and_use_kinds_unaffected_by_depend_extension` | 向后兼容，Mount/Use 语义不变 |

---

## 质量指标

| 指标 | 本次 | 前次（V2.6） |
|------|------|--------------|
| 测试总数 | **85** | 77 |
| Clippy 警告 | **0** | 0 |
| 新增测试 | **+8** | +7 |
| 新增规则（静态图） | **+27** | — |
| 受影响 crate | 4 | 3 |

---

## 图论 OS 特性维护

- **EdgeAbsent 模式**：直接复用现有重写引擎语义，无新机制
- **声明式依赖图**：插件依赖从代码逻辑提升为图拓扑的一等公民
- **自愈不可变式**：`synchronize_manifest_graph` 构建 → `verify_boot_manifest_graph` 校验 → `service_system_cycle` 稳定
- `ReceptiveEdgeKind::Depend` 仅在启动清单修复路径使用，运行时 Cypher 通道继续受限

---

## 下一步（V2.6 剩余项）

- [ ] Epoch-diff 渲染跳过（VGA 层空帧优化，Demo A 前置）
- [ ] PAL_U32 → 属性节点重构（Demo A 前置）
- [ ] V3 生态层规划（见 `plan/V3_DEVELOPMENT_PLAN.md`）
