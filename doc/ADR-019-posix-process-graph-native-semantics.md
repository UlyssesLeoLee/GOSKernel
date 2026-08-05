# ADR-019：POSIX 原生用户态进程——fork/exec/信号的图论原生语义

> 状态：**§五第 1 项已落地（2026-08-05）；fork/exec/信号本体仍待你在 §二/§三选向** · 提案日期：2026-08-05 · 配套：[ADR-014](./ADR-014-process-as-subgraph-compat-strategy.md)（§二进程=子图映射已落地；§六记录选向 B，本 ADR 是 B 自己要求的"各自独立 ADR"里第一个动笔的）、[ADR-018](./ADR-018-bootloader-uefi-migration.md)（真实 ELF header 解析证实内核当前加载在低半区 `0x200000`，是本 ADR §一发现的关键阻塞项的直接证据）
>
> 口径：用户在选定 ADR-014 选项 B 时明确追加约束（原话）："并且要做成适合图论构造的形式"——本 ADR 的方法论因此倒过来：先问"这件事在'一切皆图重写'模型里最自然的图操作是什么"，再检验它是否满足 POSIX 语义期望，不允许"POSIX 语义 + 图数据库外壳"这种退化写法。

## 一、问题陈述——三层真实地基，一个精确的单点阻塞

调研（读代码，不是猜)发现的现状,比 ADR-014 §三写下时能看到的更具体：

### 1.1 Ring3 syscall trampoline——真实存在，0 调用方

`crates/gos-kernel/src/ring3.rs`：`IA32_STAR`/`LSTAR`/`FMASK` 真实编程,`syscall_entry` 是真实的 naked-asm trampoline（保存/恢复调用者寄存器、`sysretq`)，`rust_syscall_handler` 分发到 4 个已存在的图原生 syscall（`AllocPages`/`FreePages`/`EmitSignal`/`ResolveCapability`)。代码自己的文档注释承认："Until an ELF-loaded plugin actually runs in Ring 3 ... no code path issues syscall"——这条路径**从未被真实执行过一次**。

### 1.2 ELF 解析器——真实存在，但服务于错误的隔离级别

`crates/gos-loader/src/elf.rs`（Phase B.4.6）：真实的 ET_DYN 解析 + `R_X86_64_RELATIVE` 重定位处理，`dynamic_segment`/`lookup_dynamic_symbol` 都是可用的真实代码。但它加载的目标是**内核态插件**（`module_init`/`event`/`stop` dynsym 入口，与 `k-net`/`k-vga` 同级，共享内核地址空间与 Ring 0 特权)——不是"独立地址空间、Ring 3 隔离的用户进程"。两者的**信任边界完全不同**：前者信任被加载的代码（它能看见整个内核地址空间)，后者恰恰是要否定这种信任。

### 1.3 每个 builtin 模块已经有真实的隔离域根——但 CR3 切换是文档化的 no-op

这是本次调研里最重要的发现，之前的会话（包括本 ADR 之前的 ADR-013/018 工作)都没有注意到：

- `k_vmm::create_isolated_address_space`（`crates/k-vmm/src/lib.rs:148`）是**真实实现**：分配一个全零页帧作为新 PML4 根，把当前活跃 PML4 的**索引 256..512（高半区，内核空间）**逐项克隆过去，再为 image/stack/ipc 三个窗口在**低半区（用户空间）**建立匿名映射。这不是占位符，是可以真正跑的分页代码。
- `gos-supervisor::create_domain_root`（`gos-supervisor/src/lib.rs:337`）在 `kernel-vmm` feature 下**为每一个注册的 builtin 模块调用上面这个函数**——即今天启动时，23 个 builtin 模块**每一个都已经拥有一个真实分配、真实映射好的隔离域根**（代码注释自称"Phase B.4.1"）。
- 但 `gos_supervisor::trampoline_enter`/`cr3_switch_into`（`gos-supervisor/src/lib.rs:2805-2865`，自称"Phase B.4.4"）——真正把 CR3 切换到这个域根的那一步——**是一个文档化的 no-op**，原话："no-op on real boot until the kernel image lives in the high half"。

**为什么**：`create_isolated_address_space` 克隆 PML4 索引 256..512 时，隐含假设"内核自己的映射都落在高半区（索引 ≥256，即规范高地址 `0xFFFF800000000000` 以上)"——这样克隆才有意义（子域和内核共享同一份高半区映射，用户区各自独立)。但 [ADR-018](./ADR-018-bootloader-uefi-migration.md) 的工作里，**直接解析编译出的 `gos-kernel` 二进制 ELF header 证实**：`PT_LOAD` 首段的 `virtual_addr = 0x200000`——内核实际加载在**低半区**（索引 0 附近)，不是索引 256+ 假设的高半区。`e_type = ET_EXEC` 也确认是固定地址、非 PIE。真的切换 CR3 到一个"只克隆了索引 256..512"的新根，会让内核自己的代码/数据（在索引 0 附近)瞬间从新页表里消失——这正是 no-op 注释想avoid 的那个坑。

**这意味着：本项目已经把"给每个模块/进程一个隔离地址空间"的地基铺了大半（B.4.1 的域根创建、B.4.4 的切换框架），只差一件事没做：把内核自己重定位到高半区**。这件事一旦完成，B.4.1/B.4.4 的现有代码几乎不需要改动就能真正激活——不是巧合，是这批代码原本就是照着"内核终将在高半区"这个假设写的，只是那个假设至今没有兑现。

### 1.4 `CreateNode` 无法铸造真正的"进程"形状

[ADR-014 §五](./ADR-014-process-as-subgraph-compat-strategy.md) 已经记录：`CypherMutation::CreateNode`/`create_provisional_node` 硬编码产出 `RuntimeNodeType::Vector`/`EntryPolicy::Manual`/`ExecutorId::ZERO`，无参数。§2.1 设想的进程节点形状（`RuntimeNodeType::Compute`/`EntryPolicy::OnDemand`/解释器自己的 `ExecutorId`）今天铸造不出来。这是四个缺口里最小、最独立的一个。

## 二、fork 的图论原生语义——核心分叉，需要用户选择

**先问图论问题，不先问 POSIX 问题**：既然"一个进程 = 一个 `CreateNode` 铸造的节点 + 它的 `Use`/`Call` 出边（fd，ADR-014 §2.2 已确立"不是类比，是同一个 Grant 位")"，那么"复制一个进程"在图上最自然的操作就是——**复制一个节点持有的能力边集合**，不是"复制一段内存"这种 POSIX 教科书式的、以内存为中心的视角。

**能力克隆本身语义清楚，直接可写**：`fork()` = 新 `CreateNode` 铸造子节点 + 遍历父节点的每条 `Use`/`Call` 出边，对每条边执行"以子节点为 `from`、同 `to`、同 `edge_kind`"的 `AddEdge`——逐条复制，走已经验证过的 `gos-cypher-mut` receptive 门禁，不发明批量克隆原语（"批量操作"在这个门禁模型里没有先例；逐条复制的每一步都是已经证明过安全的动作，组合起来的整体行为不需要新的信任假设）。

**真正没有答案的是内存**——POSIX `fork` 的语义要求父子内存内容相同但地址空间独立（通常用 CoW 优化）。今天的 `AllocPages`/`FreePages`（`ring3.rs` 已编程的两个 syscall）是**裸操作，完全不经过图**——内存目前不是图的一等公民，只有"通过 syscall 申请到的一段虚拟地址"，图上没有对应节点/边。这暴露一个本 ADR 无法替用户拍板的更深问题：

### 需要你选择的子分叉：内存要不要也进图？

- **子选项 fork-a（本 ADR 倾向）**：内存留在图外。`fork` 时对 `k_vmm` 的页表做**真正的写时复制**（标记只读 + 页错误时复制，x86 硬件原生支持，是 CoW 最"便宜"、最成熟的实现路径）；图层只克隆能力边（`Use`/`Call`），不表示内存本身。地址空间管理保持传统机制，只有"这个进程能碰哪些资源"这一层语义真正走图。
  - 优点：复用 x86 硬件 CoW，不需要设计"页面节点"的粒度问题；能力克隆（真正体现 Parity 不变式的部分）和内存管理（性能敏感、传统机制已经很成熟）解耦，互不拖累。
  - 代价：不是"彻底"的一切皆图——内存这一层游离在图之外，未来如果想对内存做图级别的审计/热插拔，这里会是个缝隙。
- **子选项 fork-b**：内存本身也成为图的一部分（每个已分配的映射/VMA 是一个节点，CoW 通过边的写时复制语义统一表达，和能力克隆是同一机制的两个实例）——更彻底地贯彻"一切皆图"，但需要先回答"页面节点的粒度"（单页太细，当前 `MAX_NODES=128` 撑不住任何真实进程的页表规模；按 VMA/mapping 粒度则需要设计一整套目前完全没有先例的图结构），且没有任何现成机制可以参考——是这两个子选项里工作量和风险都大得多的一个。

本 ADR 不在两者间拍板——这是"图论原生"这个约束第一次真正遇到"没有免费答案"的地方，值得你亲自决定。

## 三、exec 的图论原生语义

进程节点身份不变（`NodeId` 保留，对外"这仍是同一个进程"的语义不丢失）——`exec` = 替换该节点的 `executor_id`（从旧程序换成新程序自己的解释器/原生 executor）+ 释放旧镜像窗口、重新映射新镜像窗口（`k_vmm` 层面的操作，不是图操作）+ **清空所有未标记 CLOEXEC 的 `Use`/`Call` 出边，保留标记 CLOEXEC 的边**。

最后这一条是本节唯一需要新发明的部分：`RuntimeEdgeType` 现有的四个可变位（`Refer`/`Bind`/`Send`/`Grant`，[ADR-001 §2.3](./ADR-001-edge-algebra-constitution.md)）里没有空位表示"exec 后是否存活"。需要在选向时一并决定：复用某个现有位的语义（有风险，可能与该位原有含义冲突），还是给 receptive edge 家族新增一个属性字段（更干净，但要过 [ADR-015](./ADR-015-abi-stability-versioning-policy.md) 的 minor-bump checklist）。倾向后者，但同样留给你确认。

## 四、信号的图论原生语义

不需要新原语——直接复用已存在的 `RuntimeEdgeType::Signal`（`0x04`，[ADR-001 §2.3](./ADR-001-edge-algebra-constitution.md) 早已定义）和真实、已测试的 `post_signal`/`drain_control_signal` 投递路径。

真正缺的两样东西都不是图结构问题，是**进程节点的本地状态**问题：`sigprocmask`（屏蔽表）和 `sigaction`（handler 注册表）——建议作为进程节点 `ExecutorContext` 的本地字段（与 fd 表——ADR-014 §2.2 的 `[EdgeId; N]` 投影——同一个"本地缓存，图是事实来源"的既有风格），不需要图层面的新设计，只需要新的进程 state schema 字段。这一节相对确定，不构成本 ADR 门禁的一部分。

## 五、建议的落地顺序（供参考，不是门禁内容）

1. **内核高半区重定位**——✅ **已落地（2026-08-05）**。§1.3 发现的单点阻塞，优先级最高：一旦完成，B.4.1（域根创建）+ B.4.4（CR3 切换框架）这两块已经写好的代码几乎不需要改动就能真正激活。实现：`x86_64-gos-kernel.json` 加 `code-model=kernel`，`.cargo/config.toml` 对该 target 加 `-C link-arg=--image-base=0xffffffff80000000`（PML4 index 511，落在 `create_isolated_address_space` 已经克隆的 256..512 范围内）。真机验证：ELF `PT_LOAD` 确实搬到高半区，QEMU+OVMF 端到端跑到 steady-state，一次成功。
   过程中发现并顺带修的一个更深的问题："B.4.4 只是等内核搬到高半区就能激活"这个假设本身不完整——boot-time PML4 index 诊断（临时加、验完即删）实测发现 bootloader_api 的动态映射（内核栈、`phys_offset` 窗口、`BootInfo`）落在低位索引（这次实测：栈=2、phys_offset=4、boot_info=6），根本不在 256..512 里；`k_vmm::create_isolated_address_space` 因此改为克隆**整个** PML4（而不只是 256..512），域私有窗口（`gos-supervisor::DOMAIN_BASE`）在克隆后显式清零以保持隔离性。修完之后把 `cr3_switch_into`/`cr3_restore`（B.4.4）从硬编码 no-op 换成真实的 `Cr3::read_raw`/`write_raw`，QEMU 实测：boot 期间 13 次 bracket 调用，13 次真实 CR3 写入，一一对应，内核仍然干净跑到 steady-state（23/23 模块、23/23 域、0 失败）；`gos-supervisor-harness` 25/25 host test 全过。这两次校验（switch count 对账）作为永久性 boot-time 健康检查保留在 `crates/gos-kernel/src/main.rs`。详见 commit `cf6a840`、`d8380cf`。
2. **`CreateNode` 参数化**——§1.4 的独立小缺口，无依赖，可以随时先做。**下一步。**
3. **验证 ring3 syscall trampoline 真的工作**——第一次真实触发一次 `syscall` 指令，不需要等前两项，可以用一个精简的、直接嵌入 boot 流程的测试路径验证（不必是真正的用户进程，先证明"trampoline 本身接得住"）。
4. **`gos-loader` 的 ET_DYN 解析器复用到新的隔离加载路径**——解析/重定位逻辑复用，加载目标从"内核空间"换成"isolated domain"，依赖第 1 项。
5. **`exec`**——依赖第 1、2、4 项，不依赖 `fork`，可以先做。
6. **`fork`**——依赖第 1、2 项 + 你在 §二 fork-a/fork-b 之间的选择。
7. **信号**——依赖第 2 项（进程节点要先能被参数化铸造），§四本身语义已经清楚，实现上相对独立。

## 六、门禁

`crates/k-libc` 或任何触及 `fork`/`exec`/信号的代码，在你对 §二（fork-a vs fork-b）、§三（CLOEXEC 位的表示方式）做出选择之前不得落地——延续 ADR-014 §六"门禁降低不是解除"的纪律。§五落地顺序里的第 1-3 项（高半区重定位、`CreateNode` 参数化、ring3 trampoline 验证）是选项无关的地基工作，可以先行——mirrors ADR-014 §二"进程=子图映射不管 A/B/C 都可以先接线"的同一处理方式。
