# ADR-010：F.5（FAT32 write + journal fsync）落地路径——`persistent` 边存储层怎么分阶段建

> 状态：**已选向：选项 A（F.5-logic 先行）· 已落地** · 日期：2026-06-12 · 选向/落地日期：2026-08-03 · 配套：[V2 计划 V2.6](../plan/V2_DEVELOPMENT_PLAN.md)（line 104，"`persistent` 属性接真实 FS-backed 边"）、[ADR-009](./ADR-009-f5-screenshot-scope.md)（已选向 A：F.5 整体移入 V2.6，本 ADR 是其落地路径的具体设计，不重复其结论）、[ADR-007](./ADR-007-display-hal-scope.md)（host-bridged-first 先例，本 ADR 选项 B 同型）、[ADR-005 §七](./ADR-005-node-mutation.md)（"promote"机制遗留，F.5-graph-integration 依赖它）、[OPTIMIZATION_PLAN.md](../plan/OPTIMIZATION_PLAN.md)（F.1-F.5 现状，line 95-127）

## 一、现状盘点：F.5 不是一个任务，是三段不同性质的工作叠在一起

ADR-009 称 F.5"全计划中量级最大、完全未启动"。逐段拆开看，"未启动"的程度并不均匀——且三段彼此依赖关系清晰。

### 1.1 F.5-logic：FAT32 write + journal fsync 的*算法/格式*本身——零架构依赖、可立即 harness 化

- `gos_journal::JournalRing`（[lib.rs:188-243](../crates/gos-journal/src/lib.rs)）：`flush_into(out: &mut [u8])`（225-238）已把 header+records 序列化进调用方缓冲；`replay(blob, sink)`（159-179）已能从内存 blob 重放。**缺**：把 `flush_into` 的输出写到 `BlockDeviceVTable::write_sector` 的 "to-device" 版本，以及对应的 `replay_from_device`。
- `k-fat32::Fat32`（[lib.rs:128-415](../crates/k-fat32/src/lib.rs)）：F.3.1 已实现 BPB 解析 + FAT chain walk + `lookup`/`read`/`read_dir`（只读，230-415）。**缺**：FAT 表项写入（标记 cluster 已用/EOF）、空闲 cluster 分配、目录项写入/创建、`gos_vfs::FileSystem::write`。
- `gos_vfs::FileSystem` trait（[lib.rs:90-115](../crates/gos-vfs/src/lib.rs)）：只有 `lookup`/`read`/`read_dir`。**缺**：`write`/`create`/`fsync` 方法签名本身。

这三者的*正确性*（FAT 表更新算法对不对、journal 序列化格式对不对）与"接到哪个真实磁盘"完全正交——[OPTIMIZATION_PLAN.md:99](../plan/OPTIMIZATION_PLAN.md) 已经示范过这条路："`vfs_trait_drives_a_synthetic_in_memory_filesystem`（runtime harness）通过合成 ramdisk 验证 ABI round-trip"。F.3.1/F.4 当初就是这样验证"完成"的。

### 1.2 F.5-wiring：第一个真实 `BlockDeviceVTable` 后端 + boot-time mount/replay——目前是 0，且不是 F.5 独有的缺口

逐 crate grep `BlockDeviceVTable` 只命中三处定义/字段（[gos-protocol/src/block.rs:54](../crates/gos-protocol/src/block.rs)、[gos-vfs/src/lib.rs:17,121,129](../crates/gos-vfs/src/lib.rs)、[k-fat32/src/lib.rs:24,130,137](../crates/k-fat32/src/lib.rs)）——**`crates/hypervisor` 里一次都没有出现**。也就是说：

- F.3.1（FAT32 read，✅ 已完成 2026-04-26）和 F.4（journal serializer/replay，✅ 已完成 2026-04-27）都只在 **host-tests harness（合成 ramdisk）** 里验证过，**从未被 `kernel_main` 调用**——`Fat32::mount()`、`gos_vfs::FileSystem::read()` 在当前 boot 路径里是 0 callers。
- 这与 ADR-007 发现"`gos-hal::display` 引用的 `fbtest.rs` 不存在"、ADR-009 发现"截图依赖 F.5"是同一类型的"V2 计划文字写于实现之前，现状已超越/落后文字"问题——只是这次方向相反："落后"：F.3/F.4 在 OPTIMIZATION_PLAN 里标"✅ 已完成"，但在真实 boot 图里完全不存在。
- F.1（`BlockDeviceVTable` ABI 本身 + `RESOURCE_BLOCK_DEVICE` supervisor 注册，[OPTIMIZATION_PLAN.md:95-99](../plan/OPTIMIZATION_PLAN.md)）也标注"实际 AHCI/NVMe 驱动（F.1.1/F.1.2）后续切片"——即 ABI 形状定了，没有任何驱动实现它。

**这意味着**：即便 F.5-logic（1.1）全部写完，只要 F.5-wiring 的缺口不补，"persistent 边接真实 FS-backed 边"在实机/QEMU boot 路径上仍然是 0——F.5-wiring 是 F.3/F.4/F.5 共享的、唯一能让这整条 VFS 栈"活"起来的缺口。

### 1.3 F.5-graph-integration：`EdgeAttrs::persistent` 何时被设为 `true`、谁触发 journal append / FAT write、boot replay 如何变回 `register_node`/`register_edge`

- `EdgeAttrs::persistent: bool`（[edge_algebra.rs:155-186](../crates/gos-protocol/src/edge_algebra.rs)）：字段已就绪，但 `RuntimeEdgeType::lower()` 的全部 9 个 legacy edge（[edge_algebra.rs:251-297](../crates/gos-protocol/src/edge_algebra.rs)）都用 `EdgeAttrs::plain()`（`persistent=false`）——**当前没有任何代码路径产生 `persistent=true` 的边**。
- 这一段依赖 [ADR-005 §七](./ADR-005-node-mutation.md) 的 V2.6 backlog 项 (2)："promote" 机制（Grant 边的触发者/权限检查点仍未定义）——"persistent"很可能是"promote"之后的下一步状态（provisional → promoted → persistent），但这条状态机目前完全没写下来。

## 二、选项：先打哪一段？

### 选项 A —— F.5-logic 先行（候选 V2.6a.1），对着 F.1 既有的"合成 ramdisk harness"模式验证；F.5-wiring 与 F.5-graph-integration 各自成为独立后续切片/ADR

只做 1.1：`gos_journal` 加 `flush_to_device`/`replay_from_device`（参数是 `&BlockDeviceVTable` + 起始 LBA）；`k-fat32` 加 FAT 表项写入 + 空闲 cluster 分配 + 目录项创建；`gos_vfs::FileSystem` trait 加 `write`/`create`/`fsync`。全部对着 host-tests 里一个 RAM byte buffer 实现的 `BlockDeviceVTable`（mirrors `vfs_trait_drives_a_synthetic_in_memory_filesystem`）验证："写入 → flush → 用读路径重新解析 → 内容一致"端到端 round-trip + journal "写入 → 掉电模拟（drop）→ 从 buffer replay → 记录一致"。

- **优点**：零新架构决定（backend 选型、boot 集成时机都不在这一步内），可独立 harness 化（mirrors V2.5a/d 模式），产出"FAT32 write + journal fsync"在字面意义上成立（代码存在且测试证明正确），为 1.2/1.3 解耦打地基。
- **代价**：做完之后"persistent 边接真实 FS-backed 边"仍是 0——boot 路径上没有真实磁盘，graph 里也没人设置 `persistent=true`。需要明确告知这是"地基"而非"deliverable 达成"，避免重蹈 F.3/F.4"harness 绿但 0 caller"的歧义。

### 选项 B —— F.5-wiring 先行：选定首个真实/host-bridged block backend + boot mount，先让 F.3 的"已完成"代码真正跑起来

不碰 write，先解决 1.2：参考 ADR-007 的"host-bridged-first"先例（`k-vk-host`/`k-cuda-host` 的模式）——是否给 `BlockDeviceVTable` 也做一个 COM3-bridged "`k-blk-host`"（host 侧用一个文件模拟磁盘），还是直接写最小 ATA PIO 真驱动（QEMU 和多数真机都支持，常见 OSDev 入门驱动）？无论哪种，目标是 `kernel_main` 里出现第一次 `Fat32::mount(...)` 调用，F.3.1 的 `lookup`/`read`/`read_dir` 在 boot 图里首次有 caller。write 逻辑（1.1）随后叠加在"已经活"的路径上。

- **优点**：直接关闭"F.3/F.4 ✅ 但 0 caller"的尴尬状态；为 V2.6 line 104 的"真机显示"（也需要"在 boot 路径里接入新硬件后端"这同一类问题）提供一个更小、风险更低的同型先例。
- **代价**：backend 选型本身是架构决定（host-bridged vs ATA PIO vs virtio-blk），按宪法第三条（ADR 先于实现）大概率需要**自己的一份 ADR**（类比 ADR-007 之于显示后端）——本 ADR 若选 B，等于把"选 F.5 第一步"变成"先去写另一份 ADR"，绕了一层。

### 选项 C —— 不拆分，按 V2 计划字面整体设计

一次性设计 FAT32 write + journal fsync + 真实设备 + boot replay + graph 集成。不建议——违反[六、sequencing 铁律](../plan/V2_DEVELOPMENT_PLAN.md)第 2 条"每阶必带 harness 与一个 killer demo"：单个切片过大，一个 PR 内无法 harness 验证全部三段，且 backend 选型（架构决定）与算法实现（机械工作）混在一起会让 review 困难。

## 三、建议与门禁

倾向 **A**：F.5-logic 先行，作为候选 **V2.6a.1**，理由——

1. **复用已验证模式**：F.1 的 `vfs_trait_drives_a_synthetic_in_memory_filesystem` 已经证明"合成 ramdisk + host-tests harness"这条路径可行且被信任，F.5-logic 只是在同一模式下扩展 write 方向，零新架构面。
2. **与本会话 V2.5 系列的 sequencing 风格一致**：V2.5a（harness-only shadow verify）→ V2.5d（host 端验证的新原语）→ V2.5e（接到交互层）。F.5-logic 对应"V2.5a/d 阶段"；F.5-wiring 对应"V2.5e 阶段"，但 wiring 还需要自己的 backend-选型 ADR，先于其实现。
3. **解耦 F.5-wiring 的架构决定**：backend 选型（选项 B 的内容）值得单独成 ADR——与 ADR-007 的显示后端选型同型，而且"第一个真实 block device"和"第一个真实显示后端"很可能共享同一个"host-bridged-first vs 真机驱动-first"的根问题。建议 V2.6d 的 UEFI GOP ADR 草拟时一并回顾本节，评估是否合并成一份"V2.6 真实硬件后端总 ADR"。

**候选 V2.6a.1（F.5-logic）建议形状**（供实现时参考，非最终签名）：

- `gos_journal`：
  - `JournalRing::flush_to_device(&self, vtable: &BlockDeviceVTable, start_lba: u64) -> Result<(), JournalError>` —— 复用 `flush_into` 的序列化，按 sector 大小切片调 `write_sector`，最后 `flush`。
  - `replay_from_device(vtable: &BlockDeviceVTable, start_lba: u64, sector_count: u32, sink: F) -> Result<usize, JournalError>` —— 读 sector 拼 buffer，调用既有 `replay`。
- `k-fat32::Fat32`：
  - `allocate_cluster(&mut self) -> Result<u32, VfsError>` —— 扫 FAT 表找第一个 free entry（`0x00000000`），标记 `0x0FFFFFFF`（EOF）。
  - `write(&mut self, inode: Inode, offset: u64, data: &[u8]) -> Result<usize, VfsError>` —— 第一版只支持"扩展既有文件最后一个 cluster 内"+"分配新 cluster 追加"，不支持 truncate / random-write-with-hole。
  - `create(&mut self, parent: Inode, name: &[u8], kind: InodeKind) -> Result<Inode, VfsError>` —— 在父目录 cluster 链里找 free 32-byte slot，写 8.3 目录项 + 分配首 cluster。
- `gos_vfs::FileSystem` trait 新增 `write`/`create`/`fsync`（`fsync` 默认实现可以是 no-op，由 `Fat32`/未来其他 impl 决定是否需要刷 FAT 表）。
- harness：新增 RAM `BlockDeviceVTable` 实现 + 5-8 条 round-trip 测试（写小文件读回、跨 cluster 写、FAT 表更新后重新 mount 仍可读、journal flush → drop → replay 记录一致）。

## 四、本 ADR 范围不含

(1) F.5-wiring 的 backend 选型（真实/host-bridged block device，见选项 B）——独立 ADR；(2) F.5-graph-integration 的"persistent 状态机"（provisional → promoted → persistent 的触发条件）——依赖 [ADR-005 §七](./ADR-005-node-mutation.md) 遗留 (2)"promote"机制，本身也可能需要独立 ADR；(3) `gos-vfs`/`k-fat32` 当前"0 caller"问题本身是否需要在 F.5 之外单独修——本 ADR 认为这正是 F.5-wiring（选项 B）要解决的问题，不重复立项。

## 五、落地状态（2026-08-03）

F.5-logic 按 §三"建议形状"基本原样落地，含义调整记录如下：

- `gos_journal::JournalRing::flush_to_device`/`gos_journal::replay_from_device` 已实现——`no_std`/无 `alloc`，流式经一个 sector 大小的栈缓冲写入/读回，而不是先在内存拼出整个 blob（`N` 较大时更省栈）。签名比草图更明确：`replay_from_device` 要求调用方传入精确的 `record_count`（而非仅 `sector_count`）——on-disk header 不带记录数字段，`sector_count` 无法唯一确定"最后一条记录在哪结束"（sector 512 字节不是 40 字节记录大小的整数倍）；调用方本就从对应的 `flush_to_device` 返回值得到这个数字，这不是新的负担。新增 `JournalError::Io(BlockIoStatus)` 变体。
- `k-fat32::Fat32`：`allocate_cluster`/`write`/`create` 均落地，签名与草图唯一差异是保持既有 `&self`（不是 `&mut self`）——`Fat32` 自身不持有可变缓存，所有状态活在块设备上，与既有 `read`/`lookup` 的 `&self` 风格一致。`write` 支持范围略宽于草图："`offset <= size_bytes` 的原地覆写或追加"（不仅是"最后一个 cluster 内追加"），复用度更高且实现量相近。已知 v1 边界（均已写入代码文档注释与 harness 断言，非事后发现）：不支持 truncate/hole；`write` 的目录项 size 回写只从根目录扫描——子目录内文件的字节写入正确落盘，但 `lookup` 报告的 size 不会更新，直到未来切片把 `patch_dir_entry_size` 泛化为可递归子目录。
- `gos_vfs::FileSystem` trait 新增 `write`/`create`/`fsync`，均带默认实现（返回 `NotImplemented`/`Ok(())`）——对任何未来的其他 `FileSystem` 实现是纯加法，不强制立即跟进。
- harness：`host-tests/gos-runtime-harness/tests/fat32_write.rs`（6 条：创建+写入+读回、跨 cluster 写入、原地覆写不增长、拒绝重名、子目录创建+写入+v1 size 限制的显式断言、卷满 `NoSpace`）+ `tests/journal_device.rs`（3 条：flush→replay 往返、drop 后重放模拟断电、跨 sector 边界）。`cargo check --workspace` 与 `gos-runtime-harness` 全套（58 测试）绿。
