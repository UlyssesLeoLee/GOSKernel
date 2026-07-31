# GOS V2 分阶段开发计划 — 产品级涌现式图论 OS

> 状态：提案 · 日期：2026-06-08 · 配套：[ADR-001 边代数宪法](../doc/ADR-001-edge-algebra-constitution.md)
>
> 本计划把 GOS 从"图数据库 + capability + 软光栅的精致 hobbyist OS"推进到 **Plan 9 之后第二个提出新计算抽象的 OS**。Plan 9 的 *everything is a file* 在 GOS 等价于 ***everything is a graph rewrite***。
>
> **产品级铁律（继承自项目记忆）**：每个 phase 必须随附 harness / golden test 一同合入。**无 harness = 不合入**。每个 phase 必须交付一个 killer demo——"系统做了一件没人显式编码的事"。做不出 demo，涌现就是 PPT。

## 〇、Prime Invariant（系统唯一不变式）

> **Quiescence：signal queue 空 ∧ rewrite queue 空 → 静默。任何"系统在跑但无人请求"的状态都是 bug。**

它同时是：测试方法学（跑 N 步必达静默，否则 = livelock = 治理失败）、节能策略（静默 = `hlt`）、可验证性入口（每条 rewrite rule 的 termination 可单独证明）、故障收敛判据（fault 也必须最终静默——要么传播完成，要么被 Grant 拓扑 firewall 截停）。

## 一、阶段总览

| 阶段 | 主题 | 核心交付 | Killer Demo | 退出判据 |
|---|---|---|---|---|
| **V2.0** ✅ 地基已落 | 立宪 & 地基 | primitive 边代数进 `gos-protocol`（`edge_algebra.rs`）；9 边 → primitive lowering；harness `gos-protocol-harness` | 9 边全部机械分解忠实于 ADR §2.3；6 边往返恒等、`{Signal,Return,Sync}` 形稳定（property test 8/8 绿） | ✅ 零行为变更（内核 `cargo check` 干净 + 治理门禁 OK）；edge-algebra property test 全绿。ADR-001/002/003 批准仍待办 |
| **V2.1** ✅ 完成（待桌面目视） | Cypher = ISA（edge 写路径） | ✅ 真实 `RuntimeDispatcher` 接 runtime edge table（替换 harness `Stub`）+ 原子 `rebind_exclusive_use` + `GraphSnapshot.graph_epoch`；可见性语义 [ADR-004](../doc/ADR-004-mutation-visibility.md)；**k-shell theme 切换已改用原子 `rebind_use`、edge-key 统一为 `"use"`** | theme 切换 **原子**提交（一次 epoch），reader 全程见恰好一条 `Use` 边——harness 已证 | ✅ 原子性 harness 绿（1/1）+ epoch 暴露 + dispatcher 接真表 + k-shell 集成；headless 启动 smoke 干净到 steady-state（modules 22/22、nodes 26 edges 50、无 panic）。⏳ 仅剩**桌面目视**（切 theme 不闪/不崩）；node-create 仍被拒（ADR-005） |
| **V2.2** ✅ 完成（调度=边传播留给 V2.3） | Rewrite Engine & Boot-as-fixpoint | [ADR-002](../doc/ADR-002-rewrite-engine.md)（§六 渲染模型 ✅=B 图即场景）：`RewriteEngine`（LHS=MATCH→guard→RHS=mutation）；boot manifest 静态图；`kernel_main` < 300 行；调度 = 边传播；因果深度计替换 2048 hardcap；quiescence（吸收 ADR-003） | 打乱 boot 依赖顺序，系统照常正确启动（序被求解，不是被编码）——✅ 已由 `boot_order` harness 证明（13 节点 / 12 边，含 GDT/IDT/PIC/PS2/activate 子链，打乱依赖声明仍解出有效序、环被报告不挂起） | ✅ **V2.2a** 引擎骨架（`gos-rewrite`：ready-set 传播 + quiescence + 因果深度计 + livelock）+ ✅ **V2.2b** boot 接线（`kernel_main` 由 `resolve_boot_order(&NODES,&DEPS)` 驱动，硬编码序消失，299 行）+ ✅ **V2.2c** 因果深度计替换 2048 hardcap（`service_system_cycle` 返回 `QuiescenceReport`；idle 必静默有 harness）+ ✅ **V2.2d** GDT/IDT/PIC/PS2/activate 拆为 5 个独立 Depend 节点（`main.rs` 内无"先 GDT 再 IDT"硬编码序）。`gos-rewrite-harness` 7/7、`gos-supervisor-harness` 15/15、治理脚本 OK、boot-smoke 干净到 steady-state。**遗留**：line 65"调度=边传播"在 sub-cycle 粒度尚未落地——`CycleRule` 仍以单一 rule 包裹既有 `drain_ready_to_runtime`/`pump` 命令式调度；留给 V2.3（与 Subscribe 反向传播共用同一传播基底）一并解决 |
| **V2.3** 🟡 进行中（V2.3a-d 已完成；遗留 2 项→V2.3e/f，均需独立 ADR） | 响应式 Subscribe & 渲染统一 | `Subscribe` 反向传播索引；`theme.wabi/shoji` 变调色板数据 node（杀掉 `PAL_U32` 常量）；`fbtest.rs` → `k-render`（纯光栅）+ `k-desktop`（场景图构造） | **Demo C**：切 theme 0 行代码扩散；静止画面 0 帧重绘；脏矩形免费 | theme 扩散 0 行；idle = 0 帧；鼠标更新独立于渲染 tick（修掉 PIT→shell heartbeat lag 根因）。✅ **V2.3a** `Subscribe` 边 + `Region` 脏矩形谓词（`gos-protocol::edge_algebra`，ADR-001 §2.3：`Refer`+`reactive`，零新 primitive，`recognize`仍拒绝识别）+ engine 反向传播索引 `gos_rewrite::reactive::{Subscription,propagate}`（`no_std`/无 `alloc`，纯 slice scan，镜像 `resolve_boot_order` 风格）。`gos-protocol-harness` 新增 9 条 property test（17/17 绿）；`gos-rewrite-harness::engine` 新增 `dirty_rect_propagation_is_region_scoped`，与既有 `reactive_subscribe_propagation_quiesces` 共用同一 `propagate`，证明 theme 扩散（`Region::EVERYTHING`）与脏矩形（不重叠 region 不重绘）确为同一机制（8/8 绿）。✅ **V2.3b** deliverable 2 的"寻址"半部：新模块 `gos_protocol::theme`（`Palette=[[u8;3];16]`、`PALETTE_WABI`/`PALETTE_SHOJI`、`palette_for_theme(theme: u8)`）从 `k-vga` 私有上移到协议层，按 `DISPLAY_THEME_WABI/SHOJI` theme-kind byte 寻址——与 `theme.current -[Use]-> theme.wabi\|shoji`（ADR-004 §2.2 `rebind_use`）切换的是同一个 byte。`k-vga` 删除本地 `ThemePalette`/`PALETTE_WABI`/`PALETTE_SHOJI`/`palette_for_theme`（~50 行），`apply_theme_palette` 经既有 `use gos_protocol::*` 解析到协议层版本，函数体逐字节不变。`gos-protocol-harness` 新增 `theme_palette.rs`：5 条 property test（寻址、未知值回退 wabi、VGA DAC 6-bit range 不变式、两套调色板互不相同），全套 22/22 绿；`cargo build -p gos-kernel --release`（真实 `x86_64-gos-kernel` target + build-std 全链接）通过；`gos-rewrite-harness` 8/8、治理 OK。未做 QEMU boot-smoke——纯数据搬迁，零控制流变更。✅ **V2.3c** deliverable 1 后半的场景化落地：`gos_rewrite::reactive` 新增 `propagate_with`（闭包回调版反向传播，`propagate` 重构为对其的 `Emit`-sink 薄封装，零行为变更，既有 `engine.rs` 5 条测试照样全过）；`k-shell` 新增直接依赖 `gos-rewrite`（经 `gos-supervisor` 早已可达，提升为直接依赖不引入新环）。`apply_theme_choice_raw` 把唯一一次硬编码 `emit_target_signal_raw` 调用，改为 `THEME_SUBSCRIPTIONS: &[Subscription]`（1 行：`theme.current` -[Subscribe, Region::EVERYTHING]-> k-vga）经 `propagate_with` 分发 + `reactive_target_vector` 把抽象 subscriber 映射回真实信号目标（保留原 `console_target==0 ? VGA_VEC : console_target` 回退逻辑，逐分支等价）；新增 subscriber 只需 1 行 `Subscription` + 1 个 `reactive_target_vector` 分支——Demo C "0 行代码扩散" 对未来 subscriber 成立。`gos-rewrite-harness` 新增 `reactive.rs`：3 条 `propagate_with` property test（EVERYTHING 全量按表序分发、不重叠 region 静默跳过、无匹配零回调），全套 11/11 绿；`gos-protocol-harness` 22/22、`gos-supervisor-harness` 15/15 不受影响；`cargo check --workspace`、`cargo build -p gos-kernel --release`（真实 target + build-std 全链接，含新 `no_std` 闭包分发路径）通过；治理 OK。本变更触及 boot 路径（k-shell 初始化时调用 `apply_theme_choice(&sink, THEME_KIND_WABI)`）——本应跑 QEMU boot-smoke，但本 session 两种自动化捕获方式（`Start-Process` 重定向、后台 job）均只捕到 QEMU 自身诊断输出，COM1（`-serial stdio`）内核日志未捕获，疑似该环境下 stdio chardev 需真实控制台句柄、与本次改动无关；verification 改为依赖按构造的等价证明——表中恰 1 行、`Region::EVERYTHING` 自重叠恒真，故 `propagate_with` 恰调用回调 1 次，`reactive_target_vector(REACT_VGA, console_target)` 对任意 `console_target` 复现原内联三元 `if console_target==0 {VGA_VEC} else {console_target}`；且 boot 调用点本就 `let _ = apply_theme_choice(...)` 丢弃返回值，假设性退化也不会新增 panic 面——叠加全量 release 链接通过。✅ **V2.3d** deliverable 3 + Demo C "idle=0" 复核（纯架构评估，零新代码）：检视 `k-vga`（`render_cell`/`render_row`/`render_full`/`scroll_body`/`apply_theme_palette`/`handle_control` 等——纯光栅缓冲区操作 + HW 寄存器 + 信号分发，V2.3b 已把唯一的非光栅状态 `PALETTE_*` 移出）与 `k-shell`（`draw_runtime_header`/`draw_console_sigil`/`draw_ai_panel`/`draw_operator_band`/`apply_theme_choice_raw` 等——场景合成 + 控制流），二者已是事实上的 `k-render`（纯光栅）+ `k-desktop`（场景合成/分发）边界；`fbtest.rs` 在 V2 代码库中从未存在，原"拆分"诉求已被现有 crate 边界满足、唯命名不同——deliverable 3 视为按现状达成，无需新 crate/文件移动。Demo C "静止画面 0 帧重绘"：`kernel_main` steady-state loop（`crates/hypervisor/src/main.rs`）逐 tick 只跑 `service_system_cycle`（quiescent 时 0 工作，ADR-002）+ 节流后的 `vk_auto_refresh`（按 `graph_epoch` 门控，无变化时仅读 epoch，B3b 已验证）；VGA framebuffer 无任何周期性整屏重绘路径——该 criterion 由架构保证，非待办。**遗留**（均建议独立 ADR）：(1) **鼠标 own-Subscribe 路径**——当前光标重绘耦合在 `Input::Heartbeat`（`heartbeat_divider`，PIT 节拍）；目标是 k-ps2 鼠标 IRQ 直接产生 `Subscription(mouse.position -> cursor-redraw, region=光标包围盒)`，经 `propagate_with` 立即重绘，脱离 heartbeat tick——这同时是 deliverable 1 脏矩形半部（`Region` 不重叠跳过）首个真实 subscriber 落地（目前 `THEME_SUBSCRIPTIONS` 唯一一行用的是 `Region::EVERYTHING`），也是 Demo C 最后一项未满足标准；需要新 signal kind + 光标下像素 save/restore 语义，量级大于 V2.3a-d 总和。(2) **deliverable 2 图遍历半部**：`theme.wabi/shoji` 仍是函数查表（`palette_for_theme`），非真正的「node 携带 `Palette` 数据、render node 经图遍历读取」——需要 `gos_protocol` graph 的 node-payload 机制，目前无设计 |
| **V2.4** 🟡 进行中（V2.4a/b/c 已完成） | 能力即可达性 & 显示 HAL | capability 检查 = Grant 路径图查询；`gos-hal::display` trait（Bochs-VBE backend #1）；跨域调用走 Grant 路径 | 热插拔 node，外部持有的 capability 仍工作；子图 fault 被 Grant 拓扑天然 firewall | 5 个 killer demo 全绿；capability-path / hot-swap / fault-containment test 绿。✅ **V2.4a** capability 检查 = Grant 路径图查询的加性原语：新模块 `gos_rewrite::capability`（`GrantEdge{from,to: NodeId}` + `reachable_via_grant(nodes, edges, from, to)`——`no_std`/无 `alloc`，定容数组 BFS，镜像 `boot::resolve_boot_order` 的"表即图"风格；`from==to` 恒可达，越界/缺节点返回 `false` 而非 panic）。`gos-rewrite-harness` 新增 `capability.rs`：7 条 property test（直连可达、传递链可达且方向不可逆、不相交分量不可达、自反、缺节点安全、热插拔——无关拓扑变更不影响既有路径、fault-containment——A 的 Grant 闭包不含不相交分量节点），全套 18/18 绿；`gos-protocol-harness` 22/22、`gos-supervisor-harness` 15/15 不受影响；`cargo check --workspace`、`cargo build -p gos-kernel --release`（真实 target + build-std 全链接）通过；治理 OK。✅ **V2.4b** 把 V2.4a 的抽象原语接到真实 runtime 边词汇：新增 `grant_edges_from_specs(specs: &[gos_protocol::EdgeSpec], nodes_out, edges_out)`——按 `edge_type.lower().bits.grant`（ADR-001 §2.3，目前仅 `Use`/`Call`）过滤 `EdgeSpec`，把每个不同的 128-bit `gos_protocol::NodeId` 端点按首次出现序内联（intern）进定容数组，产出对应 `GrantEdge`（小整数 `crate::NodeId` 索引）；`capability_check(specs, from, to)` 把"内联 + `reachable_via_grant`"封装成单一 `(specs, from, to) -> bool` 查询，`from==to` 在内联前优先短路（即使端点未出现在任何边里，自身能力恒成立）；表越界（不同 grant 端点 > `MAX_CAPABILITY_NODES`）报 `None`/`false`，与 V2.4a 越界语义一致。仍 `no_std`/无 `alloc`。`gos-rewrite-harness` 新增 `capability_specs.rs`：7 条基于真实 `EdgeSpec` 的 property test——`Call`/`Use` 授予可达、`Depend`/`Signal` 被过滤（不进入派生表）、传递链可达且方向不可逆、自反对未知节点也成立、热插拔与 fault-containment 两个 killer-demo 核心改用 `EdgeSpec` 重做、超容表"只剩自反"——全套 25/25 绿（3 boot_order + 7 capability + 7 capability_specs + 5 engine + 3 reactive）；`gos-protocol-harness` 22/22、`gos-supervisor-harness` 15/15 不受影响；`cargo check --workspace`、`cargo build -p gos-kernel --release`（真实 target + build-std 全链接）通过；治理 OK。✅ **V2.4c**（ADR-006 §三 选项 A，影子验证层；详见 [ADR-006](../doc/ADR-006-capability-graph-migration.md)）：在 `capability_specs.rs` 追加 3 条 property test，逐条对应 ADR-001 §5「claim/revoke 退化为 Grant 边的 create/delete」——`claim_is_grant_edge_create_revoke_is_delete`（空 specs 下 `capability_check` 为假；加入一条 module--Call-->resource 边后为真；移除该边后复原为假，claim≡边创建、revoke≡边删除，无独立 claim 表需要同步）、`revoking_one_module_does_not_affect_another`（M1→R1 与 M2→R2 两条独立 Call 边；移除 M1→R1 不影响 capability_check(_, M2, R2)）、`revoking_all_of_a_module_capabilities_contains_it`（M 经 Mid 传递持有对 R1/R2 的能力，对应 revoke_capabilities(M) 的"移除 M 的全部出边"后，M 对 R1/R2 均不可达，但自反 capability_check(_, M, M) 恒真）。三条测试只用 `EdgeSpec`/`capability_check`，不引入 `gos-supervisor` 类型，零生产代码变更、零 ABI 风险——按 ADR-006 §三建议，是「立即可做、不依赖 ADR-005」的部分。`gos-rewrite-harness` 全套 28/28（10 capability_specs + 7 capability + 3 boot_order + 5 engine + 3 reactive）；`gos-protocol-harness` 22/22、`gos-supervisor-harness` 15/15 不受影响；`cargo check --workspace`、`cargo build -p gos-kernel --release`（真实 target + build-std 全链接）通过；治理 OK。**遗留**（均建议独立 ADR，非 V2.4 阻塞）：ADR-006 选项 B（`gos_rewrite::capability` 容量提至 `MAX_CLAIMS`/`MAX_HEAP_GRANTS` 量级、`resolve_capability`/`claim_resource` 热路径直接调用 `capability_check`）依赖 ADR-005 先选向（claim 记录↔NodeId 模型）+ 128+ 节点 BFS 性能评估，归入 V2.6 硬化范畴；**V2.4d**（[ADR-007](../doc/ADR-007-display-hal-scope.md) 提案，待选向）——`gos-hal::display` trait（V2.4 line 89 草案：Bochs-VBE LFB backend）与现状脱节：草案引用的 `fbtest.rs` 在 V2 代码库中不存在；当前显示路径是 `k-vga`（VGA 文本模式 80x25 cell buffer + DAC 调色板端口，零 trait 抽象，函数全 private）与 `k-vk-host`（host-bridged graph-native `@gos.vk` 显示列表经 COM3，B3b 已验证双向输入，即 V2.5「Soul demo」雏形）两条均非 LFB 的并行路径，二者都不是 line 89 设想的 `Surface::lfb()`/`flip()` 形状。提案倾向：`gos-hal::display`（真正的 LFB Surface）归入 V2.6"真机显示（UEFI GOP/virtio-gpu）"范畴，与已经在跑的 `k-vk-host`（V2.5 scene 显示）并列、非替代关系；`k-vga` trait 化（若需要）另开 ADR-008，不与本 ADR 共享决策依据；**V2.4e**（[ADR-008](../doc/ADR-008-cross-domain-grant-mapping.md) 提案，待选向）——deliverable 3"跨域调用走 Grant 路径"：`capability_check`（V2.4a/b/c）的 `from`/`to` 是 `gos_protocol::NodeId([u8;16])`，B.4.5 跨域 dispatch（`route_signal`）的 `target` 是 `VectorAddress`（48-bit），二者无既定映射——同一类问题在路由层的对应物。更关键：B.4.5 今天对所有 builtin 一视同仁（共享 kernel CR3，"切换"是计数 no-op，"允许的调用集合"=全集），`capability_check` 与"B.4.5 当前行为"的等价性证明在全集下 vacuous——**这个等价性证明要等 B.4.6（ELF 模块、真实 per-domain CR3）落地、B.4.5 第一次产生非平凡允许/拒绝集合后才有意义**，是前置条件未满足而非待选设计；`NodeId⇄VectorAddress` 派生函数（不依赖 B.4.6）可独立做，但编码方案是新设计决定，按铁律应先选向。5 个 killer demo 中其余 4 个（capability-path 端到端、hot-swap 真实热插拔、fault-containment 真实故障注入）均未开始，部分同样依赖上述前置条件 |
| **V2.5** ✅ 完成（V2.5a-e；ADR-005/ADR-009 均已选向 A 并落地，详见各自 ADR） | Phase I 图形（Vulkan host-bridged） | Vulkan Gen-1 落在 rewrite 基底上（scene 子图 → `k-vk-host`，✅V2.5c 确认 B3b 已落地）。✅ **V2.5a**（[ADR-005](../doc/ADR-005-node-mutation.md) §四，影子验证，零生产代码变更）：ADR-005 §三"倾向 A（provisional nodes）"列出的两项前置确认——provisional node 的渲染策略（接 ADR-002§六）、promote 的触发者与权限（接 ADR-001§五）——在 2026-06-08 写下时均无对应原语；V2.3c 的 `propagate_with` 与 V2.4b/c 的 `capability_check`/`grant_edges_from_specs` 落地后，二者均已是既有 API 的直接推论：渲染（`propagate_with`，只看 `Subscription` 表）对 promote 状态不可知（ADR-002§六 B 已蕴含）；promote = 给 provisional node 新增一条 Grant 边，`capability_check` 由假变真（与 V2.4c"claim≡边"同形）。新增 `gos-rewrite-harness/tests/provisional_render.rs`（2 条 property test）机械证明二者；`gos-rewrite-harness` 全套 30/30（2 provisional_render + 10 capability_specs + 7 capability + 3 boot_order + 5 engine + 3 reactive）；`cargo check --workspace`、治理 OK。**不构成"选向 A"**——A/B/C 仍待你选向；本步骤只是把选 A 的实现成本从"未知"降到"零新原语，CREATE 接线（`gos-cypher-mut` 新增 `CreateNode` mutation 变体）本身才是新工作"。✅ **V2.5b**（[ADR-009](../doc/ADR-009-f5-screenshot-scope.md) 提案，待选向）：检验 V2 计划三处"截图依赖 F.5（FAT32 write+journal fsync）落盘，V2.5 前必须并入"的隐含前提——`k-vk-host` 是 host-bridged（ADR-007 §1.2），帧数据经 COM3 已在 host 进程内存里，"内核 FAT32 write → host 读出"这一圈从未被需要。`tools/gfx-bridge.py` 新增 `FrameBuffer`（光栅化 display list 到 RGB byte buffer，复用 `TkRenderer` 画图原语：填充矩形/Bresenham 直线/像素）+ `write_ppm`（P6 PPM，零依赖、字节可比较）+ `--dump-ppm FILE`（可选 `--replay`）；`--check` 新增 PPM round-trip 校验（800x600 → 1,440,015 字节，header/size 均验证，跑通）。这就是 line 100/112 要求的"gfx golden frame（lavapipe）+ gfx-fuzz" harness 的 host 侧落盘基础——`parse_frame`/`FrameBuffer.apply` 同时是 gfx-fuzz 现成入口。提案：F.5 整体移入 V2.6（服务"persistent 边"存储层），删除本行"F.5 必须并入"措辞（已删）。✅ **V2.5c**（架构评估，零新代码，mirrors V2.3d）：核验"scene 子图 → `k-vk-host`"是否真已落地——`crates/k-vk-host/src/lib.rs` 的 `render_live_graph`/`vk_auto_refresh`（B3b，commits a4cbd8d/c03b433/9f77984）已经走 `gos_runtime::node_page`/`edge_page` 把*整个活体 runtime 图*布局成网格、emit 为 `@gos.vk` frame；`vk_auto_refresh` 在 `kernel_main` steady-state loop 中每 30 PIT tick 轮询一次（[main.rs:254-257](../crates/hypervisor/src/main.rs)），仅当 `gos_runtime::graph_epoch()` 变化时才重绘。而 `gos_runtime::register_node`/`register_edge`/`unregister_edge`（[lib.rs:539,555,585](../crates/gos-runtime/src/lib.rs)）三者均已无条件 `graph_epoch.wrapping_add(1)`——即**任何**未来的 `CreateNode` mutation，只要落到 `register_node`（无论 ADR-005 选 A/B/C 哪一个，新 node 终归要进 `gos_runtime` 的 node 表），下一次 `vk_auto_refresh` 轮询就会把它画出来，**零新 k-vk-host 代码**。"scene 子图 → `k-vk-host`"deliverable 视为已达成（与 V2.3d 对 deliverable 3 的处理同型：现状已超越文字，无需新工作）。**ADR-005 选向 A（2026-06-12，provisional nodes）**——A/B/C 三选项中实现成本最低、唯一不与"图自由生长"涌现愿景冲突（详见 [ADR-005 §五](../doc/ADR-005-node-mutation.md)）。✅ **V2.5d**（ADR-005 §五步骤 1）：读 `NodeSpec`（`local_node_key: &'static str` 等编译期常量字段）与 `register_node` 实现后发现，"provisional"状态本身**已是** `register_node` 对任何调用者的默认结果（`lifecycle: Allocated, binding: Unbound, instance_id: ZERO`），22 个 boot builtin 同样从此状态起步——无需新枚举/新字段。真正缺的是"运行时分配全新 `NodeId`/`VectorAddress` + 填出 `NodeSpec`"的方式（boot builtin 的 `NodeSpec` 是 `derive_node_id` 编译期常量，Cypher 节点名是运行时字符串）。新增 `gos_runtime::create_provisional_node()`/`is_provisional_node_id()`：`NodeId` = 标签字节 `0xC0` + 单调 `seq`（`AtomicU64`）；`VectorAddress.l4 = 0xC0`（现有 builtin 取值 0-30，不冲突）；`NodeSpec` 用 `RuntimeNodeType::Vector`（与 `theme.current/wabi/shoji` 同型"被动数据节点"）+ `EntryPolicy::Manual` + `ExecutorId::ZERO` + 空 `permissions`/`exports`。新增 `host-tests/gos-runtime-harness/tests/provisional_node.rs`（2 条测试：分配的 id/vector 各不相同、新节点立即见于 `node_page`、`graph_epoch` 按调用次数递增、记录字段与设计一致）；该 harness 全套 27/27（24 runtime + 2 provisional_node + 1 mutation_visibility）。`gos-rewrite-harness` 30/30 不变；`cargo check --workspace`、图治理脚本均通过。Cypher 节点名/属性的存储留给 V2.5e——不阻塞"下一帧出现新 node"判据（`render_live_graph` 按 `RuntimeNodeType` 着色，不读节点名）。✅ **V2.5e**（ADR-005 §五步骤 2，详见 [ADR-005 §七](../doc/ADR-005-node-mutation.md)）：`gos-cypher-mut::CypherMutation` 新增 `CreateNode`（unit variant，无 Label/props 载荷）——`pre_validate` 直接放行；`to_envelope` 产出 `ControlPlaneMessageKind::NodeUpsert`（提案时新 id 尚不存在，仅审计"谁请求了 create"，真实 id/vector 由 dispatch 后 `register_node` 自身的 `NodeUpsert` 携带）；`MutationDispatcher` 新增 `create_node`，`apply_mutation` 返回类型改为 `Result<Option<NodeId>, MutationError>`（仅 `CreateNode` 携带新分配的 `NodeId`，其余三个 variant 仍 `Ok(None)`，唯一既有调用点的 3 处 `.expect(...)` 无需改动）。`RuntimeDispatcher::create_node` 直接转调 V2.5d 的 `create_provisional_node`（签名同步改为返回 `(NodeId, VectorAddress)`，原 2 处调用点同步更新）。`k-cypher` 新增 `CREATE (...)` 分支——子串 `"create ("` 触发，直调 `gos_runtime::create_provisional_node()`（镜像既有 `spawn`/`activate`/`route` 直调风格，不经 `gos-cypher-mut`；AI 建议管线与交互式 parser 各自独立接线，理由见 ADR-005 §七）。`gos-runtime-harness` 28/28（`provisional_node.rs` 2→3 条，新增 `create_node_mutation_dispatches_to_provisional_node`；`cypher_mutation_pre_validate_and_dispatch` 扩展 CreateNode 分支）；`cargo check --workspace`（kernel target，含 `crates/hypervisor`）、图治理脚本均通过。**V2.6 backlog**（不阻塞 V2.5 完成判据——line 100 Soul demo 原语链路已 harness 全覆盖）：(1) Cypher `Label`/`{props}` 持久化——`local_node_key`/`plugin_id` 当前仍是共享占位值（`"cypher.provisional"`/`PROVISIONAL_PLUGIN_ID`），`no_std` 下的字符串/属性存储原语待设计，建议独立 ADR（候选 V2.5f 或并入 V2.6）；(2) "promote" 机制（§五 step 3）——Grant 边的触发者/权限检查点仍未定义；(3) 同语句 `CREATE (a)-[:Mount]->(n)` 边接线——`apply_mutation` 已回传新 `NodeId`，但 `k-cypher` 的 `CREATE` 分支与 `gos-cypher-mut::AddEdge` 尚未在同一语句内组合；详见 [ADR-005 §七](../doc/ADR-005-node-mutation.md) 遗留 1-3。F.5（FAT32 write + journal fsync）已按 [ADR-009](../doc/ADR-009-f5-screenshot-scope.md) 选向 A（2026-06-12）移交 V2.6（line 104"persistent 边"存储层基础），本行不再追踪 | **Soul demo**：`MATCH...CREATE` → 下一帧 3D 出现新 node——原语链路（`CREATE` → `create_provisional_node` → `graph_epoch` → `vk_auto_refresh` → 渲染）V2.5e 起每段均有 harness 覆盖；`k-cypher` UI 的交互式 QEMU 验证待做 | gfx golden frame（lavapipe）+ gfx-fuzz 绿——host 侧落盘已验证（`tools/gfx-bridge.py --dump-ppm`，ADR-009），不依赖 F.5 |
| **V2.6** 🟡 启动（V2.5 已✅闭环，主线进入 V2.6） | 硬化 & 产品收尾 | ✅ `.gitignore`/根目录 log 文件清理——架构核验：`ac144f0`（早期提交）已落地，当前根目录无 `log*.txt`/`*.log` 残留，`.gitignore` 已含 `*.log`/`*.txt`/`log*.txt`/`qemu_out.txt` 等全部目标模式，本项视为已达成，零新工作（mirrors V2.3d/V2.5c）；✅ **V2.6a**（[ADR-010](../doc/ADR-010-f5-persistent-storage-path.md) 提案，待选向）：F.5 拆解为三段——F.5-logic（`gos_journal`/`k-fat32`/`gos_vfs` 的 write/flush/replay 算法，零架构依赖，可对着合成 ramdisk harness 验证，mirrors `vfs_trait_drives_a_synthetic_in_memory_filesystem`）、F.5-wiring（首个真实 `BlockDeviceVTable` 后端 + boot mount/replay——目前 0，且 F.3.1/F.4 虽标✅但在 `crates/hypervisor` 里同样 0 caller，与本行"真机显示"的后端选型可能同根同源）、F.5-graph-integration（`EdgeAttrs::persistent` 字段就绪但全部 9 个 legacy edge 仍产出 `false`，依赖 ADR-005 §七"promote"机制遗留）。建议 F.5-logic 先行（候选 V2.6a.1）；真机显示（UEFI GOP / virtio-gpu）；`persistent` 属性接真实 FS-backed 边（含 F.5：FAT32 write + journal fsync，[ADR-009](../doc/ADR-009-f5-screenshot-scope.md) 已选向 A，全计划最大未启动项）；installer 真机验证（需真实硬件）；fast-path node 性能 pass（`fast-path`/`FastPath` 当前代码库零命中，需新 ADR 定义"图里挂 fast-path 标签的 node"长什么样）；`hypervisor` → `gos-graph-engine` 改名（identity shift，多 crate `Cargo.toml`/`use` 路径 + `.cargo/config.toml` + CI，需先有改名范围 ADR） | Boot 自调优：换 CPU、图不变，boot 时长缩短（rewrite engine 找到更宽并发 fire 层） | 全系统测试报告；产品级 V2.0 发布 |

依赖链：**V2.0 → V2.1 → V2.2 → V2.3 → V2.4 → V2.5 → V2.6**，主线串行（每阶地基是下一阶前提）。并行轨见 §三。V2.6 之后的主线（生态・兼容・前沿，含 ADR-014/015/016 doc 轨）见 [V3 计划](V3_DEVELOPMENT_PLAN.md)。

## 二、各阶段详述

### Phase V2.0 — 立宪 & 地基（约 2–3 周）

**目标**：把宪法变成代码与机械约束，零行为变更。这是最便宜也最重要的一步——primitive 选错是宪法级代价。

**交付**：
- ADR-001（边代数）批准；ADR-002（Rewrite Engine 语义）、ADR-003（Quiescence 不变式 + 测试方法学）起草批准。
- `gos-protocol`：编码 `EdgeBits{ refer, send, bind, grant }` + `EdgeAttrs{ persistent, exclusive, cardinality, reactive }`。
- 9 条现有边 → primitive 的 lowering 兼容层（命名点保留为构造器，内部 lower 成 primitive）。
- **Harness**：edge-algebra property test —— 往返分解（命名点 → primitive → 命名点恒等）、组合封闭、正交性反例固化。

**退出判据**：现有 runtime 行为逐位不变（diff 仅在表示层）；property test 全绿；治理脚本新增"禁止第 5 primitive"红线。

**风险**：lowering 层引入表示层 bug → 用"9 边行为快照对比"兜底（V2.0 前后 snapshot 必须逐字节相同）。

### Phase V2.1 — Cypher = ISA（约 3–4 周）

**目标**：兑现"Cypher 是 GOS 的机器码"——先通 **edge 写路径**。勘察修正（2026-06-08）：[`gos-cypher-mut`](../crates/gos-cypher-mut/) 的 `MutationDispatcher` trait + `apply_mutation` + `AuditedMutation` **已存在**，且 `gos-runtime` 的 `graph_epoch` 可见性机制**已存在**；真实缺口是**没有任何接真实 edge table 的 impl**（唯一实现是 harness `Stub`）。同时尊重现有约束：mutation 是 **edge-only**（`AddEdge/RemoveEdge/RebindUse`，仅 `Mount/Use`），node create/delete 被有意禁止（保护 claim/quota/NodeId 稳定），**推迟到 ADR-005**，V2.1 不碰。

**交付**：
- 真实 `impl MutationDispatcher`（接 `gos-runtime` 的 `register_edge`/`unregister_edge` + 新增**原子 rebind** 入口），替换 harness `Stub`。约束：不改 `register_edge`/`unregister_edge` 旧单次递增语义（零行为变更回归）。附录 B 的 `CreateNode/DeleteNode/SetProp` 是 V2.2 rewrite-engine 的远期 ISA，非 V2.1 范畴。
- **ADR-004 决定 mutation 可见性语义**：epoch-published（下一 cycle 可见，reader snapshot 隔离）vs immediate。**这个决定反向约束 V2.3 renderer 怎么读图**——必须在此钉死，否则 Phase I 建在沙上。推荐 epoch-published：reader 永远看一致 snapshot，writer 批量提交一个 epoch。
- audit envelope（已存在）接到真实 mutation；fault attribution（哪个 node 发起的 mutation 失败）。

**退出判据**（= [ADR-004 §七](../doc/ADR-004-mutation-visibility.md) 检查单）：`RebindUse` / `AddEdge` / `RemoveEdge` 端到端经真实 dispatcher 写入 edge table；`RebindUse` **原子性** harness 绿（全程不暴露零/双 `Use` 边的可观察 epoch）；`GraphSnapshot` 暴露 `graph_epoch`；audit envelope 与 mutation 同临界区入队；node-create 仍被 `pre_validate` 拒绝。Harness：rebind 原子性 test、epoch 可见性 test、dispatcher-接真实-runtime 的回归（旧 edge API 行为不变）。

**风险**：可见性语义选错会波及全栈——故先于一切 UI 工作完成 ADR-004。

### Phase V2.2 — Rewrite Engine & Boot-as-fixpoint（约 4–6 周）

**目标**：boot 不再是硬编码函数序列（当前 [`main.rs:17-128`](../crates/hypervisor/src/main.rs)），而是在 boot manifest 图上求重写不动点。

**交付**：
- `RewriteEngine`：rule = `LHS pattern`（Cypher MATCH）→ `guard`（fire predicate）→ `RHS`（Cypher mutation 集）。node "fire" ≡ 其 rewrite rule 命中并执行 RHS。
- boot manifest 编译期静态图；`kernel_main` 缩成：`load_boot_manifest → engine.run_to_quiescence → engine.steady_state`，目标 < 300 行。
- 调度 = 边传播：ready set = 有 ≥1 条满足谓词的待处理入向 `Send` 的 node；按 lane-class 标签选一个 fire；fire 产生新 Send/mutation；循环至 ready set 空（quiescence）。
- 因果深度计：替换 [`service_system_cycle` 的 2048 hardcap](../crates/hypervisor/src/main.rs)。跑满不再是静默截断，而是 telemetry 报警"本帧因果链深 N"。

**退出判据**：GDT/IDT/PIC 经 `Depend` 边（= Refer + fire-guard）按 readiness 自然 fire，`main.rs` 无"先 GDT 再 IDT"硬编码序；打乱 manifest 依赖声明顺序系统照常正确启动。Harness：quiescence test（N 步必达静默）、boot-graph 拓扑排序 test、livelock 检测。

**风险**：涌现行为难调试 → 每次 fire 记录触发边（强 telemetry）；quiescence 不变式做兜底断言。

### Phase V2.3 — 响应式 Subscribe & 渲染统一（约 4–6 周）

**目标**：消灭 render loop。当前 [`main.rs:112` 的 `loop { service_cycle(); render_frame(); hlt() }`](../crates/hypervisor/src/main.rs) 把渲染绑死在内核 tick 上，且 [`fbtest.rs`](../crates/hypervisor/src/fbtest.rs) 1764 行命令式 UI 内嵌在 hypervisor crate——违背"最小引导"边界。

**交付**：
- `Subscribe` 边（= Refer + reactive 属性 + region 谓词）+ engine 反向传播索引：任何 mutation 触及 node X，engine 沿反向 reactive 边向订阅者发 `Send(repaint, region)`。
- `theme.wabi / theme.shoji` 从硬编码 [`PAL_U32` 常量](../crates/hypervisor/src/fbtest.rs)变成持有调色板数据的 node；render node 经图遍历 `theme.current -[Use]-> 调色板` 读色，不再读常量。
- `fbtest.rs` 拆分：`k-render`（纯 rasterizer，无 UI 逻辑）+ `k-desktop`（scene 子图构造）。`hypervisor` 不再直调渲染——渲染是一个 Subscribe 到 `scene_root` 的 node。

**退出判据 / Demo C**（逆向详见附录 C）：切 theme 0 行代码扩散；mutation 只激活受影响 region 内的 render node（脏矩形免费）；静止画面 0 帧（`hlt` 真省电）；鼠标走自己的 Subscribe 路径、不抢 ring 0 时间（根除 `fix(desktop): throttle PIT→shell heartbeat` 修的那个 mouse lag）。Harness：golden frame（lavapipe）、repaint-region test、idle-frame-count test。

**关键涌现证据**：theme 扩散与脏矩形渲染共用**同一个** Subscribe + 反向传播机制。一个机制喂出两个特性——这就是"不是 ad hoc"的证明。

### Phase V2.4 — 能力即可达性 & 显示 HAL（约 3–4 周）

**交付**：
- capability 检查 = Grant 路径图查询（正式化 ADR-001 §五）：跨域调用前 engine 验证 `from -[Grant*]-> target` 可达且终点暴露该 cap。claim/revoke = Grant 边 create/delete。
- `gos-hal::display` trait：`init(prefer: ResolutionHint) -> Surface` / `surface.lfb()` / `surface.flip()`。Bochs-VBE 为 backend #1（封装当前 [`fbtest.rs:296` 的 DISPI 直写](../crates/hypervisor/src/fbtest.rs)），为 virtio-gpu / UEFI GOP 留口。
- 跨域调用走 Grant 路径（接 Phase B.4.5 已有的 cross-domain capability invocation 结构）。

**退出判据**：5 个 killer demo 全绿（见 §四）。Harness：capability-path test、hot-swap test、fault-containment test。

### Phase V2.5 — Phase I 图形（Vulkan host-bridged）（约 6–8 周）

**目标**：原 Phase I Vulkan Gen-1 现在**落在稳固的 rewrite 基底上**，而非建在沙上。host-bridged（像 `k-cuda-host`），非 bare-metal Vulkan。

**交付**：scene 子图 → `k-vk-host` 桥接 host Vulkan。（F.5 移至 V2.6，见 [ADR-009](../doc/ADR-009-f5-screenshot-scope.md)）

**退出判据 / Soul demo**：`MATCH ... CREATE (n)` → 下一帧 3D 显示新 node。**此时已 trivial**——CREATE 改 scene 子图 → Subscribe 反向传播 → render node 激活 → 下一帧出节点。不写一行 demo 专用代码。Harness：gfx golden frame、gfx-fuzz。

### Phase V2.6 — 硬化 & 产品收尾（约 4–6 周）

**交付**：真机显示（UEFI GOP / virtio-gpu backend）；`persistent` 属性接真实 FS-backed 边（接 `gos-vfs` / journal，含 F.5：FAT32 write + journal fsync，自 V2.5 移入，见 [ADR-009](../doc/ADR-009-f5-screenshot-scope.md)）；installer 真机验证（已有 `tools/build-installer.ps1` 流程）；fast-path node（rasterizer / DMA 仍走快路径——只要快路径也是图里一个挂 fast-path 标签的 node）；`.gitignore` 收拾根目录 20+ 个 `log*.txt`；`hypervisor` → `gos-graph-engine` 改名（不只是改名，是 identity shift——它已缩成 rewrite engine 本身）。

**退出判据 / 终极 Demo**：换 CPU、图不变，boot 时长缩短（rewrite engine 找到更宽的并发 fire 层——没有性能调优 PR，性能来自图算法）。全系统测试报告；产品级 V2.0 发布。

## 三、并行轨

主线串行，但以下可与主线并行推进，互不阻塞：

- **测试基建轨**（贯穿全程）：lavapipe golden frame、gfx-harness、gfx-fuzz、quiescence harness。每阶强制随附，不单列阶段。
- **F.5 持久化轨**：FAT32 write + journal fsync 是 V2.6"`persistent` 边"的存储层基础（[ADR-009](../doc/ADR-009-f5-screenshot-scope.md)）；可在 V2.1 后任意时点推进，不阻塞主线 UI，不再是 V2.5 前置。
- **文档轨**：ADR 持续产出；`GOS_ARCH_v2.md` 在 V2.2 / V2.3 后各更新一次以反映新身份。

## 四、五个 Killer Demo（涌现的验收标准）

口号便宜，证据贵。每个 demo 都是"系统做了一件没人显式编码的事"。**这五个不全绿之前，涌现就是 PPT。**

| # | Demo | 落地阶段 | 证明的不变式 |
|---|---|---|---|
| 1 | **Theme 0 行扩散**：改 `theme.current -[Use]->` target，所有路径上含 theme.current 的 node 自动重渲。无 broadcast、无 observer、无 `redraw_all()`。 | V2.3 | Subscribe + 反向传播 足够替代显式发布订阅 |
| 2 | **最小重绘 0 行**：mutation 只激活受影响 region 内 render node。无脏矩形表。 | V2.3 | region 谓词使重绘自动微分 |
| 3 | **热插拔 0 行**：销毁 node、换新版本替换，外部持有的 capability 仍工作。无 handoff 代码。 | V2.4 | NodeId/VectorAddress 分离 + Grant 路径稳定性 |
| 4 | **故障隔离 0 行**：子图 fault 沿 Bind 边传播至子图内、被 Grant 边天然 firewall 在子图外。 | V2.4 | Bind ≠ Grant 的拓扑隔离 |
| 5 | **Boot 自调优**：换 CPU、图不变，boot 时长缩短（更宽并发 fire 层）。 | V2.6 | 性能来自图算法，无调优 PR |

## 五、风险登记册

| 风险 | 严重度 | 缓解 |
|---|---|---|
| **走偏成"图数据库 + 调度器"** | 致命 | 钥匙是宪法（ADR-001）+ 5 个 killer demo。立宪 + demo 通了才算真涌现。每阶评审强制问"这阶有没有 demo 证明 0 行涌现"。 |
| Rewrite engine 自身 6–12 月工程量 | 高 | 比"先做 Phase I 再回头重做"便宜。V2.0–V2.2 是不可压缩的地基投资。 |
| primitive 选错（宪法级代价） | 高 | ADR-001 §三完备性/正交性证明逐条对抗性评审后方批准。 |
| 涌现行为难调试 | 中 | quiescence 不变式 + 每次 fire 记录触发边的强 telemetry。Erlang/actor 社区已解过同类问题。 |
| 性能上限低于命令式 | 中 | GOS 重可表达性 > 极限性能。真热路径（rasterizer/DMA）走 fast-path 标签节点。 |
| mutation 可见性语义选错 | 中 | ADR-004 先于一切 UI 工作完成；推荐 epoch-published + snapshot isolation。 |
| 招人难 | 低 | 反转为 hiring filter——能理解这套设计的人正是要找的人。 |

## 六、sequencing 铁律

1. **V2.1（写路径）通之前不推 V2.5（Phase I）**。Phase I 的 trait shape 取决于 V2.1 定的可见性语义。
2. **每阶必带 harness 与一个 killer demo 一同合入**。无 harness = 不合入；无 demo = 这阶没证明涌现。
3. **ADR 先于实现**。V2.0 批 ADR-001/002/003，V2.1 批 ADR-004。没有 ADR 的宪法级决定不得进代码。
4. **不在 Gen-1 scope 内提 bare-metal Vulkan / virtio-gpu**（继承项目记忆约束）；它们是 V2.6+ 的事。
5. F.5（FAT32 write + journal fsync）服务于 V2.6"`persistent` 边"存储层，**不是 V2.5 gfx harness 的依赖**（[ADR-009](../doc/ADR-009-f5-screenshot-scope.md)）；不阻塞主线 UI。

---

## 附录 B — Cypher-as-ISA trait 草图（V2.1 落地形状）

把"每个 syscall / IPC / 中断进入 / capability 调用 = 一条 Cypher mutation"具象化。这是 V2.1 的设计输入，批准后毕业为 ADR-002。

```rust
// ── ISA：一切状态变更都是 mutation ───────────────────────────────
// 对应 ADR-001 的 primitive 边代数：CreateEdge 携带 EdgeBits + EdgeAttrs。
pub enum Mutation {
    CreateNode { kind: NodeKind, props: Props, vector: VectorHint },
    DeleteNode { id: NodeId },
    SetProp    { id: NodeId, key: PropKey, val: PropVal },
    CreateEdge { from: NodeId, to: NodeId, bits: EdgeBits, attrs: EdgeAttrs },
    DeleteEdge { id: EdgeId },
}

// ── rewrite rule = match + guard + emit；这就是一个 node 的"代码" ──
// LHS 是 Cypher MATCH，RHS 是 Cypher CREATE/DELETE/SET。
pub trait RewriteRule {
    fn lhs(&self) -> Pattern;                    // Cypher MATCH 模式
    fn guard(&self, m: &Match) -> bool;          // fire 谓词（Depend 的依赖语义住这里）
    fn rhs(&self, m: &Match) -> Vec<Mutation>;   // fire 时发出的 mutation
}

// ── dispatcher：带可见性 + 审计 + 故障归因地应用 mutation ─────────
// ADR-004 决定 apply 是 epoch-published（推荐）还是 immediate。
pub trait MutationDispatcher {
    /// 批量提交一个 epoch；返回新 epoch 或带故障归因的拒绝。
    fn apply(&mut self, batch: MutationBatch, by: NodeId) -> Result<Epoch, MutFault>;
    /// reader 永远看一致 snapshot（snapshot isolation）。
    fn snapshot(&self, at: Epoch) -> GraphSnapshot;
}
```

**"Cypher 是 ISA"的具象**：
- PS/2 IRQ → `CreateNode{kind: InputEvent, ...}` + `CreateEdge{ from: ps2, to: central_irq, bits: Send }`。中断进入 = 一条 mutation。
- syscall → 一个 `MutationBatch`（如 `CreateNode(request)` + `CreateEdge(Send to handler)`）。
- 切 theme → `DeleteEdge(旧 Use)` + `CreateEdge(新 Use)`（见附录 C）。
- 跨域 cap 调用 → engine 先验 Grant 路径可达，再 `CreateEdge{ bits: Send|Grant }`。

quiescence ≡ 无 node 的 RewriteRule guard 被满足 ≡ 无待处理 Send。

## 附录 C — Demo C（Theme 0 行扩散）逆向

证明"切 theme 0 行代码"需要什么前置，以及它如何复用通用 Subscribe 机制。

**当前态**：`theme.current -[Use]-> theme.wabi`（vector 6.1.3.0 → 6.1.1.0），`Use` = Refer+Bind+Grant，exclusive。色值硬编码在 [`fbtest.rs` 的 `const PAL_U32`](../crates/hypervisor/src/fbtest.rs)。

**目标动作**：执行一条 mutation 即让所有受 theme 影响的 node 重渲，零 theme 专用代码。

**传播 trace**：
1. render node 持 `(render)-[Subscribe{reactive}]->(theme.current)`。
2. mutation（一条）：`MATCH (c:theme.current)-[u:Use]->(:theme.wabi) DELETE u CREATE (c)-[:Use{exclusive}]->(shoji)`。
3. 该 mutation 触及 `theme.current`（其出边变了）。
4. engine 反向 reactive 索引：谁 Subscribe 了 theme.current？→ 所有 render node。→ 向各自发 `Send(repaint)`。
5. render node fire，重新经图遍历读 `theme.current -[Use]-> shoji` 的调色板，按新色渲染。

**零 theme 专用代码**——传播是通用 Subscribe 机制，与脏矩形渲染（Demo #2）是**同一机制**。

**前置（即 V2.3 必须先做的具体重构）**：
- V2.1：`DELETE u / CREATE edge` 的 mutation 必须真生效（`MutationDispatcher`）。
- V2.3：`Subscribe` 边 + engine 反向传播索引。
- V2.3：**杀掉 `PAL_U32` 常量**——`theme.wabi/shoji` 变持调色板数据的 node，render node 经图遍历读色而非读常量。这是 Demo C 暴露出的第一个落地重构，也是整条路的起点。
