# ADR-001：边代数宪法（Edge Algebra Constitution）

> 状态：**提案（待批准）** · 日期：2026-06-08 · 决策层级：宪法级（批准后不可向后兼容地修改）
>
> 口径：本文档定义 GOS V2 的**最小正交边代数**。一旦批准，整个系统的可表达性由它封顶。新增边类型只能是这套 primitive 的**组合命名点**，不得引入新 primitive。修改本文档等价于重新定义"GOS 是不是 GOS"。

## 一、为什么需要一部宪法

当前 runtime 暴露 9 条语义边：`Depend / Call / Spawn / Signal / Return / Mount / Sync / Stream / Use`（见 [GOS_ARCH_v2.md §3.2](./GOS_ARCH_v2.md)）。

问题不是"够不够用"，而是**哪些是 primitive、哪些是 derived 从未被回答**。9 条边是平铺的枚举，彼此语义重叠（`Mount` 与 `Use` 都含生命周期耦合；`Stream` 是 `Signal` 的时间展开；`Spawn` 含一次创建信号 + 一条父子边）。平铺枚举有三个致命后果：

1. **不封闭**：第 10、11、12 条边会按需求不断长出来，每加一条都要改 runtime、改 supervisor、改治理脚本。系统永远在"加边类型"。
2. **不可证**：无法对"边的组合"做形式化推理，因为边之间没有代数关系。
3. **不涌现**：如果每个能力都要显式编码一条边，系统就是"图数据库 + 调度器"，不是涌现 OS。

涌现式 OS 的成立前提是：**不设计组件，设计代数和重写规则；其余一切是这个代数在 boot manifest 上求得的不动点。** 边代数就是那个代数。Conway 生命游戏用 4 条规则涌现出图灵完备；GOS 需要同等克制的 primitive 集。

## 二、决策：4 个 primitive 关系 + 4 个正交属性

### 2.1 Primitive 关系

一条边的本质是两个 node 之间的关系，它在四个**正交维度**上各携带一个 bit。`Refer` 是强制基底，其余三个是可选位。

| Primitive | 维度 | 语义 | 蕴含 |
|---|---|---|---|
| **`Refer`** | 可见性 | A 能命名 / 寻址 B。这是一切的地基——没有 Refer 就不能 Send / Bind / Grant。 | 无（基底） |
| **`Send`** | 因果性 | A 的 fire 可向 B 投递一个信号，可能激活 B。纯数据流 / 控制流。 | Refer |
| **`Bind`** | 生命周期 | B 的生命周期与 A 耦合。A 消亡时 B 受影响（级联）。纯所有权 / 容纳。 | Refer |
| **`Grant`** | 权限 | 该边授予"调用 B 导出能力"的权利。纯安全。 | Refer |

> **一条边 = `Refer` + {`Send`, `Bind`, `Grant`} 的任意子集 + 属性。** 即 2³ = 8 种基础组合 × 属性空间。

### 2.2 正交属性（任何边都可携带的修饰位）

属性不是 primitive——它们调制 primitive，不增加新维度的关系语义。

| 属性 | 取值 | 含义 |
|---|---|---|
| `persistent` | bool | 边是否在 power-cycle 后存活（false = 易失，true = 落盘，由 `gos-vfs` / journal 承载）。 |
| `exclusive` | bool | 同一目标 node 上是否最多只能终结一条此种边（`theme.current` 的排他 `Use` 就靠这个）。 |
| `cardinality` | `one` \| `many` | 边触发一次还是重复触发（`Stream` = `Send` + `many`）。 |
| `reactive` | `None` \| `region predicate` | 是否让 engine 在**反向**自动传播信号：B 被 mutate 时，engine 沿反向 reactive-`Refer` 边向 A 发 `Send(changed)`。这是 Subscribe / 响应式渲染 / theme 扩散的**唯一机制**。 |

### 2.3 命名点（derived edges）

现有 9 条边降级为这套代数里的**人体工学别名**——是命名点，不是新 primitive。新增边类型 = 新命名点，**零 engine 改动**。

| 现有边 | primitive 分解 | 属性 | 备注 |
|---|---|---|---|
| `Depend` | `Refer` | — | 依赖语义不在边上，而在 A 的 **fire-guard**（"B 必须 ready"）。engine 维护反向 readiness 索引。 |
| `Signal` | `Send` | `one` | 最弱因果边，fire-and-forget。 |
| `Stream` | `Send` | `many` | 时间维度展开的 Signal。 |
| `Call` | `Send` + `Grant` | `one` | Grant 给被调方一次性 reply-cap，`Return` 走它回来。 |
| `Return` | `Send` | `one` | 在 Call 的 reply-cap 上反向 Send，不是独立 primitive。 |
| `Spawn` | `Send` + `Bind` | `one` | Send(create-intent) + Bind(parent→child)。 |
| `Sync` | `Send` 双向 | `one` | 屏障语义在双方 fire-guard（rendezvous predicate），不在边上。 |
| `Mount` | `Refer` + `Bind` | `exclusive=false` | 非排他生命周期挂载。clipboard.mount 即此。 |
| `Use` | `Refer` + `Bind` + `Grant` | `exclusive=true` | 排他 + 授权 + 生命周期。`theme.current -[Use]-> theme.wabi` 即此。 |
| `Subscribe`（V2 新增） | `Refer` | `reactive=region` | 不需要新 primitive——只是带 reactive 属性的 Refer。 |

## 三、封闭性与完备性（为什么这是对的）

**完备性**：上表证明现有 9 条边 + 新增 Subscribe 全部可由 `{Refer, Send, Bind, Grant}` × 属性表达。没有一条边需要第 5 个 primitive。

**正交性**：给定 Refer，Send / Bind / Grant 三两独立可证——
- 仅 `Send`：向不拥有、无特权的 node 发 fire-and-forget = `Signal`。
- 仅 `Bind`：拥有 B 生命周期但从不信号、无能力 = 纯容纳边（父持有子供 GC）。
- 仅 `Grant`：可调用 B 的能力但不拥有、不直接信号 = 纯 capability 授予。

三者无法互相推导，故正交。

**封闭性**：两条同向边 A→B 的组合 = 两者 primitive 位的并集 + 属性归并，结果仍是 `Refer` + {Send,Bind,Grant} 子集 + 属性，仍是合法边。代数在组合下封闭。

**这意味着**：edge algebra 是一个有限维布尔代数（3 个可选 primitive 位）叠加一个属性向量空间。可对它写 property test（往返分解、组合封闭、命名点一致性），可对每条 rewrite rule 单独证 termination。这是"可证 OS"的入口。

**V2.0 实现发现（2026-06-08，lowering 是满射不是双射）**：把 §2.3 落成代码（`crates/gos-protocol/src/edge_algebra.rs`，property test `host-tests/gos-protocol-harness`）后确认——`Signal` / `Return` / `Sync` 三者 lower 到**同一个** `EdgeForm`（`Refer+Send`，cardinality `One`）。这不是缺陷，恰恰是 §2.3 自身论断的兑现：`Return` 的应答配对住在 `Call` 的 reply-cap、`Sync` 的屏障住在两端 fire-guard——两者的区别都是 **node 级（role）而非 edge 级**。因此严格的"往返恒等"只对 6 条具唯一 form 的边（`Call/Spawn/Depend/Mount/Stream/Use`）成立；对 `{Signal,Return,Sync}` 这个三元等价类，`recognize()` 返回**规范代表** `Signal`，往返性精确表述为**形稳定**：`lower(recognize(lower(e))) == lower(e)`。8 条 property test 全绿验证了这一点。结论：**代数无需第 5 个 primitive；这 3 条边在 edge 层是同一条边，差异应在 V2.2 的 node rewrite rule 里承载。**

## 四、禁止事项（宪法红线）

批准后，治理脚本 `tools/verify-graph-architecture.ps1` 将机械强制：

1. **禁止新增 primitive**。任何 PR 引入第 5 个不可分解的边语义 = 宪法违规，必须先修宪（改本 ADR + 全员评审）。
2. **禁止把语义藏进 node 私有状态**。跨 node 协作必须通过边表达（延续 [GOVERNANCE §2.3](./GOS_GOVERNANCE_v0_2.md)）。
3. **禁止绕过 Grant 做跨域调用**。capability 即可达性（见 §五），授权必须是图上一条真实的 Grant 路径，不得硬编码函数指针跨域。
4. **命名点必须可机械分解**。每个 derived edge 在代码里必须能 lower 成 primitive 组合，由 property test 验证往返。

## 五、推论：capability 即图可达性

宪法的直接推论，写明以正式化 Phase V2.4：

> "node A 能否调用 node B 的能力 C？" ≡ "在允许的边类型上是否存在 A→B 的 **Grant 路径**，且终点暴露 C？"

授权检查从"表格查询"还原为"图可达性查询"，与 `k-cypher` 的 `MATCH` 完美统一——**授权 IS Cypher MATCH**。claim / revoke 退化为 Grant 边的 create / delete。

## 六、考虑过但否决的方案

| 方案 | 否决理由 |
|---|---|
| **保留 9 条平铺枚举** | 不封闭、不可证、不涌现（见 §一）。 |
| **边 = N 维能力向量（无命名点）** | 纯向量丢失人体工学——开发者要记"`{Refer,Bind,Grant,exclusive}`"而非 `Use`。命名点保留可读性同时保持代数纯度，是更优解。 |
| **把 `persistent` 做成 primitive** | 持久性是边的**属性**不是**关系维度**——一条 Mount 边持久与否，关系语义不变。强行做 primitive 会破坏正交性。 |
| **把 `Subscribe` 做成第 5 primitive** | Subscribe = `Refer` + `reactive` 属性即可。把它做 primitive 会让"响应式"和"可见性"耦合，破坏正交。更重要：reactive 属性让 theme 扩散与脏矩形渲染共用**同一机制**——这正是涌现的证据，不能拆散。 |

## 七、后果

**正面**：
- `kernel_main` 可缩到 < 300 行（boot = 在 manifest 图上求重写不动点）。
- 新能力 = 新命名点 + manifest 边，零 engine / 零治理脚本改动。
- 渲染、theme 扩散、热插拔、故障隔离共用少数几条 primitive 的传播——可做出"0 行代码"killer demo。
- 可对 rewrite rule 做形式化 termination 证明。

**代价**：
- 现有 runtime edge 枚举需引入 lowering 层（命名点 → primitive），含兼容期（Phase V2.0 的迁移映射）。
- 团队需先吃透代数才能贡献——这是 hiring filter，不是纯负担。
- primitive 选错的代价是宪法级的——所以本 ADR 必须经全员对抗性评审，附 §三的完备性 / 正交性证明逐条复核后方可批准。

## 八、批准检查单

- [ ] §2.3 分解表逐条复核，确认无遗漏边、无需第 5 primitive
- [ ] §三正交性三例逐一确认无法互推
- [x] property test 已实现并全绿（`host-tests/gos-protocol-harness`，8/8）：判别值稳定 / 分解忠实 / 6 边往返恒等 / 三元等价类形稳定 / 组合封闭 / refer-floor —— Phase V2.0 退出条件已达成
- [ ] `tools/verify-graph-architecture.ps1` 红线规则 §四已转成机械检查
- [ ] 全员签字：批准后修改本文档需走修宪流程
