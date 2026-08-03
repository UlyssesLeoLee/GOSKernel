# ADR-015：ABI 版本治理——三条轴线，一条已闭环，两条只半闭环

> 状态：**已选向：选项 A（审计 + 补齐轴线②）· 已落地** · 提案日期：2026-06-12 · 选向/落地日期：2026-08-03 · 配套：[V3 计划](../plan/V3_DEVELOPMENT_PLAN.md)（"#47 ADR-015，gates SDK publication"）、`gos-protocol`（Phase D.5 packed-semver 基建）、[gos-loader](../crates/gos-loader/src/lib.rs)、[gos-supervisor](../crates/gos-supervisor/src/lib.rs)、[gos-journal](../crates/gos-journal/src/lib.rs)、[ADR-012](./ADR-012-fast-path-node-tagging.md)（提议的 `PermissionKind::FastPathSnapshot=0x0A`——本 ADR minor-bump 判定规则的第一个真实测试用例）
>
> 口径：V3 计划把 ADR-015 列为"ABI 稳定性与版本策略"待写——读起来像一张白纸。但 `gos-protocol` 早在"Phase D.5"就已经实现了一套 packed-semver 机制（`encode_abi(major,minor,patch)`、`abi_compatible(plugin_v, host_v)`、三个独立版本常量），其中一条轴线**已经在生产路径上强制执行**。本 ADR 不是设计新机制，是审计三条轴线各自的"设而不查"/"设而查之"状态、补齐缺口，并第一次把"什么算 minor bump"从隐含约定写成规则——为 V3.1 gos-sdk 的外部 `.gosmod` 模块准备一份它能依赖的契约。
>
> **落地状态回填**（本 ADR 撰写时三条轴线是"一闭环两半闭环"，现状已是**三条全闭环**——落地过程比本 ADR 记录的更早、分散在两次独立的后续 hardening 工作里，此处回填而非重新执行）：
> - **轴线①**（`GOS_ABI_VERSION`）：本 ADR 撰写时即已闭环（Phase D.5，`gos-loader::validate_manifest`）。
> - **轴线②**（`MODULE_ABI_VERSION`）：`gos_supervisor::validate_module` 已加 `abi_compatible(descriptor.abi_version, MODULE_ABI_VERSION)` 检查（[lib.rs:1287](../crates/gos-supervisor/src/lib.rs)，`SupervisorError::AbiVersionMismatch`），配 harness `incompatible_module_abi_version_is_rejected_at_bring_up`（`gos-supervisor-harness`）——由已合并的 `claude/module-abi-version-gate` 分支落地，早于本次回填。
> - **轴线③**（`CONTROL_PLANE_PROTOCOL_VERSION`）：`gos_journal::deserialize_envelope` 已加版本校验（`JournalError::UnsupportedProtocolVersion`），配 harness `journal_rejects_envelope_with_stale_protocol_version`——详见 [OPS_JOURNAL_PROTOCOL_VERSION_GATE.md](./OPS_JOURNAL_PROTOCOL_VERSION_GATE.md)，该文档记录时即引用了本（当时未合并的）ADR。本 ADR §二选项 A 原判断"轴线③留作记录、不强制修"已被这次独立 hardening 超额完成，不是本 ADR 自身要求的范围。
> - **本次真正新落地的**：minor-bump checklist（`gos-protocol/src/lib.rs`，`abi_compatible` 文档注释）——三条轴线里唯一在本 ADR 选向前仍是空白的部分。`gos-verify/src/lib.rs` 的参数顺序注释核查后发现已是正确顺序（`abi_compatible(plugin, host)`），无需改动。

## 一、问题陈述

### 1.1 三条轴线，现状对照表

`gos-protocol/src/lib.rs` 定义了三个独立的版本常量，分别贴在三种不同的数据结构上：

| 轴线 | 常量 | 当前值 | 贴在哪个结构体 | SET（构造时写入） | CHECK（消费时校验） |
|---|---|---|---|---|---|
| ① 插件/执行器 ABI | `GOS_ABI_VERSION`（`encode_abi(2,0,0)`，line 37-39,66） | 2.0.0 | `PluginManifest.abi_version`、`KernelAbi.abi_version` | ✅ 全部 ~13 个内建 `k-*` crate 的 manifest + [`gos-runtime:1675`](../crates/gos-runtime/src/lib.rs) 的 `KERNEL_ABI` 静态实例（`abi_version: GOS_ABI_VERSION`） | ✅ [`gos-loader/src/lib.rs:181`](../crates/gos-loader/src/lib.rs) `validate_manifest` 调 `abi_compatible(manifest.abi_version, GOS_ABI_VERSION)`，不兼容 → `LoaderError::AbiVersionMismatch`；[`builtin_bundle.rs:1535`](../crates/hypervisor/src/builtin_bundle.rs) 静态路径同款检查 |
| ② 模块 ABI | `MODULE_ABI_VERSION`（`encode_abi(1,0,0)`，line 846-850） | 1.0.0 | `ModuleDescriptor.abi_version`、`ModuleAbiV1.abi_version` | ✅ [`gos-supervisor:2257`](../crates/gos-supervisor/src/lib.rs)（`MODULE_ABI_V1` 静态实例，`abi_version: MODULE_ABI_VERSION`）+ `2785`/`2807`（`ModuleDescriptor` 实例同款） | ❌ 全 crate 搜索 `MODULE_ABI_VERSION`/`abi_compatible`/`AbiVersionMismatch`，gos-supervisor 里只有这 3 处命中，全是 SET，**零处 CHECK** |
| ③ 控制平面协议 | `CONTROL_PLANE_PROTOCOL_VERSION: u16`（line 67） | 1 | `ControlPlaneEnvelope.version` | ✅ `gos-runtime:353`、`gos-cypher-mut::to_envelope():125` 构造时写入；[`gos-journal::serialize_envelope`](../crates/gos-journal/src/lib.rs:108) 落盘 | ❌ [`deserialize_envelope`](../crates/gos-journal/src/lib.rs:118-140) 把 `version` 读回 `ControlPlaneEnvelope.version` 字段，但**不与 `CONTROL_PLANE_PROTOCOL_VERSION` 比较、不拒绝**——`decode_kind` 对未知 `kind` 会返回 `JournalError::UnknownKind`，`version` 没有对应的 `JournalError::VersionMismatch` |

**三条轴线的"形状"完全一致**（常量 + `#[repr(C)]`/manifest 字段 + 构造时写入），但只有①走完了"写入 → 比较 → 拒绝"的完整闭环。②③只走了一半——版本号被忠实地搬运，但没有人读它做决定。

### 1.2 为什么这不是"现在不需要，以后再说"

①（`GOS_ABI`）覆盖 `PluginManifest`/`KernelAbi`——**所有内建 `k-*` crate 与内核同一个 cargo workspace、同一次编译**，`abi_compatible(2.0.0, 2.0.0)` 今天永远是 `true`，这条检查目前更像"宪法已经写好，只是还没人违宪"。

②（`MODULE_ABI`）覆盖 `ModuleDescriptor`/`ModuleAbiV1`——[`gos-supervisor`](../crates/gos-supervisor/src/lib.rs) 的 `entry_fn(abi: *const ModuleAbiV1, handle, domain) -> ModuleCallStatus`（line 1199/2750/2759/2768）**正是 V3.1 gos-sdk 产出的外部 `.gosmod` ELF 模块会调用的入口**——这是 GOS 当前唯一一个"内核与被加载代码分别编译、通过 `#[repr(C)]` vtable 交互"的真实跨边界场景（gos-loader 的 ELF 路径目前加载的仍是与内核同 workspace 编译的内建 bundle，①的"同编译"前提仍成立；②理论上已经为"不同编译"准备好了字段，但闭环缺失）。**如果 V3.1 的第一个外部 `.gosmod` 今天就写出来、声明一个过期的 `abi_version`，gos-supervisor 不会拒绝它**——它会拿到 `MODULE_ABI_V1` vtable，正常被调用，直到某个它假设存在、但新版 vtable 已移除/改签名的函数指针上崩溃。这是一个**潜伏的、此刻就成立**的缺口，只是还没有外部模块去触发它。

③（`CONTROL_PLANE_PROTOCOL_VERSION`）覆盖 [`gos-journal`](../crates/gos-journal/src/lib.rs) 的 40 字节定长 on-disk 记录——`replay()` 是**跨进程/跨重启**读旧数据的路径，如果未来 `ControlPlaneEnvelope` 的字段语义变化（哪怕字节布局不变，`arg0`/`arg1` 的含义变了），`version` 字段是**唯一**能让 replay 代码区分"这条记录是旧格式"的信号——目前这个信号被写入但从未被读取判断。

### 1.3 minor 位从未移动过

`abi_compatible(plugin_v, host_v)`（line 57-64）的判定是 `abi_major(plugin_v)==abi_major(host_v) && abi_minor(plugin_v)<=abi_minor(host_v)`——**minor 字段存在的意义是"host 可以比 plugin 新，但不能比 plugin 旧"**，这个判定只有在 minor 真的会变化时才有意义。但 `GOS_ABI_VERSION`/`MODULE_ABI_VERSION` 自 Phase D.5 写下 `(2,0,0)`/`(1,0,0)` 以来，minor 从未变化过——即使期间 V2.1-V2.6 增加了大量**按 D.5 注释的字面定义就该算"加法变更"**的内容（新 `RuntimeEdgeType`/`RoutePolicy` 变体、`CypherMutation::CreateNode`、以及 ADR-012 正在提议的 `PermissionKind::FastPathSnapshot = 0x0A`）。这不是 bug——内建 crate 间 `abi_compatible(2.0.0,2.0.0)` 恒真，minor 不动不影响任何东西——但意味着**"什么算 minor bump"这件事从来没有被任何一次真实决策检验过**，留下的只是 D.5 注释里的字面定义，没有先例。

## 二、选项

### 选项 A——审计现有三轴，补齐②的强制检查，把 minor-bump 规则写成 checklist（倾向）

1. **补齐②**：在 gos-supervisor 安装/启动模块的路径上（`entry_fn` 调用前）加一次 `abi_compatible(descriptor.abi_version, MODULE_ABI_VERSION)` 检查，不兼容则拒绝（mirrors `gos-loader::validate_manifest` 的 5 行模式，新增一个错误变体表示 ABI 不兼容）。这是**对称性修复**——①已经有的闭环，②照抄一遍。
2. **③留作记录，不强制 V2.6 内修**：journal 的 `version` 字段语义检查（"遇到旧版本记录该怎么办"——拒绝？尽力兼容解析？）本身是个需要单独设计的小决策（取决于"是否承诺 journal 向后兼容读取"——这本身可能是另一条策略），本 ADR 记录这是"已识别、未关闭"的缺口，留给 journal 自身未来的 format-versioning 子决策（可能不需要单独 ADR，是一行 `if version != CONTROL_PLANE_PROTOCOL_VERSION { ... }` 级别的修复）。
3. **写下 minor-bump checklist**：在 `gos-protocol`（`abi_compatible`/`GOS_ABI_VERSION` 注释旁）补一段"以下变更需要 bump `*_ABI_MINOR`"的具体规则——例如：
   - 给 `#[repr(u8)]` 的 `PermissionKind`/`RoutePolicy`/`RuntimeEdgeType` 等枚举新增变体（纯加法，不改变既有判别值）→ **minor bump**（旧 plugin 仍兼容新 host，新 plugin 在旧 host 上被拒绝——精确符合 `abi_compatible` 的 `<=` 语义）。
   - 给 `PluginManifest`/`NodeSpec`/`EdgeSpec`/`ModuleDescriptor` 新增**末尾字段**且有默认值 → **minor bump**。
   - 改变既有字段语义/判别值，或移除字段/变体 → **major bump**（`abi_compatible` 的 `==` 语义，双向不兼容）。
   - 把这条规则套到 ADR-012：若 `PermissionKind::FastPathSnapshot=0x0A` 选向落地，它是**第一个**符合"`GOS_ABI_VERSION` 2.0.0→2.1.0"定义的真实变更——本 ADR 不要求 ADR-012 落地时必须同步 bump（内建 crate 间无影响），但 ADR-012 的门禁可引用这条规则，作为"V3.1 gos-sdk 上线后，这类变更才会有外部观察者"的预演先例。
4. **顺手修正一处措辞偏差**：[`gos-verify/src/lib.rs:137`](../crates/gos-verify/src/lib.rs) 的 deferred-invariant 注释写的是 `abi_compatible(host, plugin)`，但真实签名/生产调用（如 `gos-loader:181` 的 `abi_compatible(manifest.abi_version, GOS_ABI_VERSION)`）是 `abi_compatible(plugin_v, host_v)`——参数顺序写反了。这是一条**尚未实现**的 Kani harness 占位注释（H.4.x 系列），不影响当前行为，但若按字面实现会验证一个与生产用法参数顺序相反的调用。本 ADR 顺手修正措辞，避免 H.4.x 落地时继承这个偏差。

- **优点**：①②③现状盘点本身就是"文档落后于现实"的标准修复（同型 ADR-011/012/013）；②的修复是纯加法（新增一次检查 + 一个错误变体），低风险，直接关闭"V3.1 第一个外部 `.gosmod` 可能崩溃在过期 vtable 上"的潜伏问题；minor-bump checklist 第一次把"什么算加法变更"从隐含约定变成书面规则，ADR-012 待选向的提案可以直接引用。
- **代价**：②的修复需要在 gos-supervisor 的模块安装路径上找到合适的插入点并新增一个错误变体——比 ADR-011 的"纯改名"略重，但量级与 ADR-012 的"新枚举变体"相当，可作为 harness-provable 的小步骤独立落地。

### 选项 B——把②③也提升为"立即强制"，作为 V2.6 收尾的一部分

不仅补齐检查，还要求②③在 V2.6 结束前就有对应失败路径的 harness 测试（mirrors V2.4c `capability_specs.rs` 的 claim/revoke 等价性测试模式）——确保"声明不兼容的 manifest/descriptor/journal record 会被拒绝"这件事本身被一个测试断言锁住，不只是代码里加了 if。

- **优点**：比 A 更彻底——不仅"代码能拒绝"，还有"测试证明代码会拒绝"，避免未来重构悄悄删掉这个 if 而无人发现。
- **代价**：③的设计决策（journal 的向后兼容承诺是什么）本身还没有答案——为一个还没有答案的问题写 harness，可能锁住一个错误的答案。范围比 A 大，且把"journal format 版本策略"这个本该独立的小决策硬塞进本 ADR 的门禁。

### 选项 C——只写 minor-bump checklist，不碰②③的代码

承认②③的"设而不查"现状，只把 §1.3 的 minor-bump 规则写成文档；认为①已闭环、规则已写好后"政策"层面已经足够，②③的代码缺口留给各自未来需要时再补（"届时第一个真实的外部 `.gosmod`/journal 兼容性问题出现时，自然会暴露并修复"）。

- **代价**：与 §1.2 的论证矛盾——②的缺口是"此刻就成立"的潜伏缺口，不是"未来才会暴露"；V3.1 gos-sdk 的第一个外部模块就可能是触发者，而按"ADR before implementation"铁律，那时应该已经有政策可循，而不是现场发现现场修。选 C 等于让 ADR-015 的"门禁"形同虚设。

## 三、建议与门禁

倾向 **A**：①已经是一个完整、可工作的范本——补齐②就是把这个范本在仅有的一处真实"分别编译"边界（`ModuleAbiV1`/`ModuleDescriptor`，V3.1 gos-sdk 将真正使用的边界）上抄一遍，5 行级别的修改；③记录在案、留给 journal 自身的 format-versioning 子问题；minor-bump checklist 把"D.5 注释里隐含的加法/破坏性变更定义"第一次升级为可引用的书面规则，ADR-012 的 `PermissionKind::FastPathSnapshot` 选向落地时可直接套用这条规则作为先例。

**门禁**：
- ②的修复（gos-supervisor 模块安装路径新增 `abi_compatible(descriptor.abi_version, MODULE_ABI_VERSION)` 检查 + 新错误变体）是纯加法、独立可落地，可在选向后随时进行，建议配一个"声明不兼容 `abi_version` 的 `ModuleDescriptor` 必须被拒绝"的最小 harness 测试（mirrors gos-loader 现有路径若有同类测试的话，否则新增）。
- minor-bump checklist 是纯文档变更（写入 `gos-protocol` 注释或新建 `doc/` 下的版本策略说明），零代码风险，可立即落地。
- ③（journal version 字段的兼容性语义）不在本 ADR 门禁范围内——本 ADR 只要求记录该缺口的存在，具体决策（拒绝 vs 尽力解析 vs 其它）留给 journal 自身未来需要时的小决策，不阻塞 V3.1 gos-sdk 工作（②已覆盖 gos-sdk 最相关的边界）。
- `gos-verify/src/lib.rs:137` 的参数顺序措辞修正，作为本 ADR 的随附 drive-by 编辑直接落地（零风险，纯注释）。
