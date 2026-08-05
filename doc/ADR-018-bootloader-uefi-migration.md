# ADR-018：bootloader 0.9→0.11+ 迁移——UEFI 真机启动的第一步

> 状态：**已落地（UEFI-only，2026-08-04）**——`crates/gos-kernel` 正式迁移到 `bootloader_api = "=0.11.9"`，`xtask qemu` 端到端验证通过（23/23 builtin 模块、steady-state、framebuffer 全部工作），过程中发现并修复 4 个真实 bug，完整记录见 §七 · 提案日期：2026-08-04 · 配套：[ADR-013](./ADR-013-real-hardware-display-mvp.md)（virtio-gpu MVP 已选向 A 落地；本 ADR 是它明确留给"独立 ADR"的另一半）、[tools/build-installer.ps1](../tools/build-installer.ps1) + [tools/write-usb-image.ps1](../tools/write-usb-image.ps1) + [doc/06_运维维护/INSTALL_BARE_METAL_zh.md](../doc/06_运维维护/INSTALL_BARE_METAL_zh.md)（现有 U 盘安装链）、[.github/workflows/installer-artifact.yml](../.github/workflows/installer-artifact.yml)（CI 产物工作流）、V2 计划 line 104（"真机显示"exit criterion，已被 ADR-013 拆分）
>
> 触发：用户在开发队列（ADR-006…017 + 拓扑指数族重构）完成后提供了具体目标——"我希望把它安装在 2014 年 macmini 上，用 U 盘引导"，并明确指示"你完成开发后再做安装包"。开发已完成（本 ADR 之前的全部提案待选向 ADR 均已按选项 A 落地），现在轮到安装包这一步——但落地前先摸清 ADR-013 §一 1.1 已经指出的"UEFI GOP 隐藏前提"到底有多大。

## 一、问题陈述

### 1.1 现有安装链产出的是 BIOS/MBR 镜像，不是 UEFI 镜像

`tools/build-installer.ps1` 调 `cargo bootimage -p gos-kernel`，产出 `target/x86_64-gos-kernel/release/bootimage-gos-kernel.bin`——这是 `bootloader = "0.9.23"`（[crates/gos-kernel/Cargo.toml:59](../crates/gos-kernel/Cargo.toml)）+ `bootimage` cargo 子命令的产物，本质是一个**传统 BIOS 启动扇区 + MBR 分区表**的原始磁盘镜像（`dd` 语义写入即可用，[doc/06_运维维护/INSTALL_BARE_METAL_zh.md](../doc/06_运维维护/INSTALL_BARE_METAL_zh.md) 已有完整"生成→写入→引导"流程）。

[doc/06_运维维护/INSTALL_BARE_METAL_zh.md:104](../doc/06_运维维护/INSTALL_BARE_METAL_zh.md) 写"进入 BIOS/UEFI 启动菜单，选择该 U 盘启动"——这句话把两种完全不同的固件启动模型并列，但**从未在真实 UEFI-only 硬件上验证过**（任务 #45 一直 blocked on hardware，此前无真机可测）。这是本 session 反复出现的"文档假设了尚未验证的路径"模式（同类型：ADR-007/012/015 的"文档落后于现实"，方向相反——这里是文档超前于验证）。

2014 Mac mini（Late 2014，Haswell）是 Apple 自 2012 年前后逐步移除传统 BIOS 兼容支持（Boot Camp 的 CSM 仿真）之后的机型世代——**具体某一固件版本是否还残留 legacy/CSM 回退，本 ADR 不假设，需要真机验证**，但即使残留，legacy BIOS 启动依赖的 MBR 引导扇区链与 Mac 固件的"从 U 盘识别可启动项"逻辑历来兼容性差、不可移植（这正是 rEFIt/rEFInd 类项目存在的原因——它们本身依赖能被固件识别的正确格式）。唯一在**任何** Mac（2006 年至今、任何固件版本）上都保证可用的路径是**原生 UEFI 启动**：U 盘的 EFI 系统分区（FAT32，`/EFI/BOOT/BOOTX64.EFI`），按住 Option 键选择"EFI Boot"或直接设为启动盘。

### 1.2 UEFI 启动的真实第一步：`BootInfo` 没有 framebuffer，构建模型也不同

[ADR-013 §1.1](./ADR-013-real-hardware-display-mvp.md) 已指出 `bootloader 0.9.23` 的 `BootInfo`（`memory_map`/`physical_memory_offset`/`recursive_index`）没有 framebuffer 字段——这只是**症状**。查证 `rust-osdev/bootloader` 上游文档（Context7，2026-08-04）后，真正的差异是整个构建模型换了：

**0.9.x（当前）**：
- 内核 crate 自身就是最终产物；`.cargo/config.toml` 指定自定义 target `x86_64-gos-kernel.json` + `build-std`；
- `bootimage` cargo 子命令（独立安装的工具）读取内核二进制 + `bootloader` crate 的预编译引导代码，拼出磁盘镜像；
- `runner = "bootimage runner"` 让 `cargo run`/`cargo test` 直接起 QEMU；
- 产出**只有** BIOS 格式的 `.bin`。

**0.11.x（上游现状）**：
- 入口签名变为 `bootloader_api::entry_point!(kernel_main)`，`kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> !`；`BootInfo` 新增 `framebuffer: Optional<FrameBuffer>`（连同 `memory_regions`/`rsdp_addr`/`ramdisk_addr` 等一整套新字段——**不是加一个字段，是整个类型换代**）；
- 内核不再是"最终产物"，而是被一个新的顶层 `os`/构建 crate 通过 **artifact-dependency**（`[build-dependencies] kernel = { path = "...", artifact = "bin", target = "..." }`，需要 `.cargo/config.toml` 的 `[unstable] bindeps = true`）引用；
- 该顶层 crate 的 `build.rs` 调用 `bootloader::BiosBoot::new(&kernel_path).create_disk_image(&bios_out)` **和/或** `bootloader::UefiBoot::new(&kernel_path).create_disk_image(&uefi_out)`——两种镜像可以**同时产出**，互不排斥；
- 不再需要自定义 `x86_64-gos-kernel.json` target + 全量 `build-std`（0.11 的内核可以编译为标准 `x86_64-unknown-none`，引导代码由 bootloader crate 自己按其需要构建）；
- `cargo run`/`cargo run uefi` 的 QEMU runner 集成方式也变了（不再是 `bootimage runner` 这个外部子命令）。

这意味着"给真机加 UEFI 支持"的真实工作量不是"多写一个 GOP 驱动"，而是：
1. 引入一个新的顶层构建 crate（`os` 或类似），把 `gos-kernel` 变成它的 workspace 内 artifact-dependency——**crate 拓扑本身要变**；
2. 重写 `.cargo/config.toml`（`bindeps`/`build-std` 配置整段替换）、删除或保留 `x86_64-gos-kernel.json`（0.11 下内核可能不再需要它，但需要验证 `no_std` 全链接在标准 `x86_64-unknown-none` target 下是否还成立——现有 `crates/k-pmm`/`k-vmm`/`k-idt` 等直接碰物理内存布局的 crate 有没有对旧 target JSON 里某些字段的隐式假设，需要逐一核实，不能假设"零风险"）；
3. `main.rs` 的 `kernel_main` 签名、GDT/IDT 早期初始化对 boot-info 布局的任何隐式假设，都要在新 `BootInfo` 形状下重新验证；
4. `Makefile`、`.github/workflows/installer-artifact.yml`（`cargo install bootimage` 这一步整个作废）、`tools/build-installer.ps1`（`Get-BootImagePath`/`cargo bootimage` 调用全部要换成新 builder API 或 `cargo run uefi` 等价物）、`tools/write-usb-image.ps1`（如果产出变成"EFI 分区 + 数据"而非单一裸镜像，写盘脚本的分区处理逻辑也可能要跟着变——取决于 `UefiBoot::create_disk_image` 的产物形态，需要先跑一次实验验证)。

这是一次 boot-protocol 大版本迁移，量级与 ADR-011（3 文件 diff）或 ADR-012（新枚举变体）完全不是一个数量级——但比 ADR-013 §1.1 写下时能看到的还要再深一层：不只是"BootInfo 缺字段"，是**内核在构建拓扑里从"最终产物"降格为"另一个 crate 的输入"**，这件事本身会牵动一切假设"这个 crate 就是 `cargo build` 的直接目标"的现有脚本/CI 步骤。

### 1.3 好消息：BIOS 与 UEFI 可以共存产出，不是"二选一迁移"

`BiosBoot`/`UefiBoot` 是同一个 `build.rs` 里两条独立调用——迁移完成后，现有 QEMU 开发流（`cargo run` 起 BIOS 镜像）**不必须**被 UEFI 路径取代，两种镜像可以并存产出，一个继续喂 QEMU 日常开发循环，另一个喂真机安装包。这降低了"迁移=推倒重来"的顾虑，但不改变 §1.2 列的构建拓扑变化本身——两条产线共存，不等于只做一条产线的工作量。

## 二、选项

### 选项 A——全量迁移到 0.11.x，`build.rs` 同时产出 BiosBoot（QEMU 开发用）+ UefiBoot（真机安装用）镜像（倾向）

按 §1.2 完整重构：新顶层构建 crate，`gos-kernel` 变 artifact-dependency，两种镜像并存。`tools/build-installer.ps1` 改产出 `gos-installer-uefi.img`（EFI 分区镜像）而不是当前的 BIOS `.bin`；`doc/06_运维维护/INSTALL_BARE_METAL_zh.md` 改写"在目标机器上启动"一节，明确区分"legacy BIOS 目标机"与"UEFI-only 目标机（含所有 Mac）"两条路径。

- **优点**：唯一在 2014 Mac mini（以及任何未来的 UEFI-only 真机目标）上有把握工作的路径；QEMU 开发流不受影响（BiosBoot 继续产出）；`gos-hal::display` 的 UEFI GOP backend（ADR-007 范围内、V2.6"真机显示"的另一半，ADR-013 已选向 A 只做了 virtio-gpu 那一半）从此才有 `BootInfo.framebuffer` 可读，不再是空中楼阁。
- **代价**：§1.2 列的构建拓扑重构量不小，且部分风险点（`no_std` 内存管理 crate 对旧 target JSON 字段的隐式依赖、`UefiBoot::create_disk_image` 的实际产物形态、CI runner 是否需要 OVMF 固件跑 UEFI QEMU 验证）目前**没有实验数据**，只有上游文档描述——本 ADR 建议的"落地"应该先是一个小型 spike（新建一个最小 `no_std` 玩具内核 + 0.11 迁移，验证 QEMU + OVMF 能跑通)，再决定是否／如何把 `gos-kernel` 本体迁过去，而不是直接对生产内核动手。

### 选项 B——只做 UEFI，放弃 BIOS 产线

承认"两条产线"本身也是维护成本，既然真机目标是 UEFI-only 硬件，QEMU 开发流也切到 `-bios OVMF.fd` 起 UefiBoot 镜像，BiosBoot 完全不产出。

- **优点**：迁移 diff 更小（只做一半的 builder 调用），长期只维护一条产线。
- **代价**：QEMU 开发流的行为变化（需要本机装 OVMF 固件、`cargo run` 的默认体验改变），且**立即**牺牲了"BIOS 继续可用"这个当前唯一已验证工作的启动路径——如果 UEFI 迁移过程中出现意外问题，没有已知能跑的后备产线。§1.3 已指出 A/B 共存不额外增加构建拓扑变化的量级,只是"调用 builder 一次还是两次"的区别,放弃这个几乎零成本的后备选项没有清楚的收益。

### 选项 C——不迁移，先用 rEFInd 等第三方工具尝试链导现有 BIOS 镜像

不改 bootloader 版本,先验证 2014 Mac mini 这台具体机器的固件是否仍支持某种 legacy/CSM 回退,或能否通过 rEFInd 从 UEFI 侧链导现有 MBR 镜像。

- **代价**：即使某台具体机器验证可行，这条路径**不通用**——依赖具体固件版本的历史遗留行为，未来任何其他 UEFI-only 目标机器（包括同一台 Mac mini 未来固件更新后）都可能失效,不是长期路径,只是"这一台机器凑合能跑"的偶然性证据,与本项目一贯"诚实标注成本、不假装临时方案是完成"的原则（mirrors ADR-009/010/013 的拆分处理)相悖。可以作为**并行的、不排他的快速验证手段**（如果 rEFInd 链导真的能在这台机器上跑通，说明"暂时不用等 A/B 落地也能跑通 demo"，对排期有参考价值)，但不能替代 A/B 中任何一个成为长期方案。

## 三、建议与门禁

倾向 **A**：唯一通用、唯一让 ADR-013 留空的"UEFI GOP 真机 LFB"从概念变为可实现的路径，且 BiosBoot/UefiBoot 共存意味着不必牺牲已验证的 QEMU 开发流。

**门禁**（本 ADR 不实现,只把 diff 摸清、把选项摆出来,按项目"ADR 先于实现"铁律,以及本人之前已明确记录的约束——UEFI GOP/bootloader 迁移不在用户签字前动手)：

- **spike 先行**：在动 `gos-kernel` 本体之前,先在一个最小 `no_std` 玩具内核上跑通 0.11 迁移的完整闭环（`BiosBoot`+`UefiBoot` 双产出、QEMU BIOS 验证、QEMU+OVMF UEFI 验证)——把 §1.2 列的"没有实验数据"的点（`no_std` target 依赖、`UefiBoot` 产物形态、CI OVMF 需求)转成实测结论,再决定要不要往生产内核迁。
- **内存管理 crate 逐一核实**：`k-pmm`/`k-vmm`/`k-idt`/`k-gdt`/`k-heap`（`tools/verify-graph-architecture.ps1` 里的 `legacyAllowlist`,本身就是这批对底层内存布局有强假设的 crate)对 `x86_64-gos-kernel.json` 里任何字段（`llvm-target`/`data-layout`/`linker-flavor`/`panic-strategy` 等)的隐式依赖,需要在 spike 里逐条对照 0.11 默认 target 的等价设置,不能假设"标准 target 天然兼容"。
- **不牺牲已验证路径**：迁移产出必须同时保留 BiosBoot 镜像（供 QEMU 开发流、CI `graph-governance.yml` 继续用),UefiBoot 是**新增**产线,不是替换——选项 A 与 B 的门禁分界点就在这里,本 ADR 选向前默认按 A 的"共存"要求设计 spike。
- **真机验证时间点对齐 #45**：即使 spike + 迁移都完成,"UEFI GOP 在 2014 Mac mini 上真正点亮画面"仍然是 ADR-013 §1.3 所说的"等真机解除阻塞才是验证的合适时机"——本 ADR 的门禁只到"迁移后 QEMU+OVMF 里能引导到 `kernel_main`,能读到非空 `BootInfo.framebuffer`",不包括"已经在真机上启动过"（那是 §45/installer 验证本身的判据,不是这个迁移 ADR 的)。
- **选项 A/B/C 最终选向留给用户**——本 ADR 只确认迁移的存在、大致形状与门禁,不替用户拍板选哪个,mirrors ADR-014 的 WASI/POSIX 分叉处理方式。

## 四、spike 结果（2026-08-04）——发现真实阻塞项，不是假设性风险

按 §三门禁"spike 先行"，在 [`spike/bootloader-011-toy/`](../spike/bootloader-011-toy/)（独立 `[workspace]`，未加入根 `Cargo.toml` members，不影响主仓库构建）里搭了一个最小 `no_std`/`no_main` 玩具内核（`bootloader_api::entry_point!` + 写 framebuffer 前 64×64 像素为绿色），验证 §一 1.2 描述的"新构建拓扑"能否在本仓库固定的 `nightly-2026-04-02` 上跑通。

**方法论修正（本身也是发现）**：第一次尝试直接用 `bootloader` 上游文档推荐的 artifact-dependency 方式（`kernel = { path = "kernel", artifact = "bin", target = "x86_64-unknown-none" }` + `.cargo/config.toml` 的 `[unstable] bindeps = true`）在仓库内跑，先后踩了两个环境坑：(1) `-Z bindeps` 对"artifact dependency 指定非默认 target"这个组合有一个至今未修的 cargo 自身 bug（[rust-lang/cargo#10444](https://github.com/rust-lang/cargo/issues/10444)、[#10647](https://github.com/rust-lang/cargo/issues/10647)，`unit_dependencies.rs:201` panic "no entry found for key"）——**放弃 artifact-dependency 方案本身**，改用 bootloader 自己文档里并列提到的替代形状："kernel 单独 `cargo build`，一个不含 build.rs 的普通 host 二进制工具读取产物路径调 `BiosBoot`/`UefiBoot`"，这与本仓库现有 `bootimage` 流程的"两步"结构同型，且规避了 bindeps。(2) 在 `E:\GOSKernel` 目录树内跑任何 cargo 命令都会向上找到 [`.cargo/config.toml`](../.cargo/config.toml) 并把**宿主侧工具自身**也强制交叉编译到 `x86_64-gos-kernel.json`——这解释了 `Makefile`（`cd /tmp && cargo +nightly test --manifest-path ...`）与全部 `host-tests/*` 为什么必须从仓库目录树**外**运行，不只是"避免 build-std 继承"这一句注释字面意思。spike 同样必须从 `/tmp` 之类的目录外跑,并显式 `cargo +nightly-2026-04-02`（`/tmp` 没有 `rust-toolchain.toml` 祖先,默认解析到 `stable`,而 bootloader 自身 build.rs 内部要跑 `-Z` 需要 nightly)。

**真实阻塞项（排除以上两个方法论干扰后,复现两次)**：`bootloader` 的 UEFI/BIOS stage 二进制自身依赖 `x86_64` crate（与本项目 `crates/gos-kernel` 依赖的 `x86_64 = "0.14"` 是**同一个上游 crate 家族,不同版本**),而这个 crate 的 `Step` trait 实现随版本经历了至少三个不兼容的不稳定 API 形状,`nightly-2026-04-02` 精确卡在中间那一档:

| `x86_64` 版本 | 来源 | `Step` 期望形状 | 本 nightly 实际结果 |
|---|---|---|---|
| v0.14.10 | `bootloader = "=0.11.7"` 拉的 | `steps_between() -> Option<usize>`（旧形状) | **E0053**：本 nightly 的 `Step` 已要求 `-> (usize, Option<usize>)`,旧形状不兼容 |
| v0.14.13 | `crates/gos-kernel` 自己钉的版本 | `steps_between() -> (usize, Option<usize>)` | ✅ 兼容（这正是 [`gos-nightly-toolchain-pin`](../.claude/skills/gos-nightly-toolchain-pin/SKILL.md) skill 记录的钉版本原因) |
| v0.15.5 | `bootloader = "0.11.17"`（最新)拉的 | 额外要求 `forward_overflowing`/`backward_overflowing` 成为 `Step` trait 方法（更新形状) | **E0407**：本 nightly 的 `Step` trait 还没有这两个方法,not a member of trait |

也就是说：**本仓库当前钉的 nightly 只有一个精确的兼容窗口，`x86_64 v0.14.13` 恰好落在窗口内——但 `bootloader` 0.11.7 与 0.11.17 分别落在窗口两侧**，都编译不过。没有对着两次全量构建失败去猜测；两次实验用的是同一 nightly、同一台机器、同一套 `edk2-x86_64-code.fd`（已确认存在于 `C:\Program Files\qemu\share\`,供后续 UEFI QEMU 验证用),只换了 `bootloader` 版本号,失败签名不同（E0053 vs E0407)且都精确指向 `Step` trait,足以确认这是版本窗口问题,不是环境噪声。

**结论**：QEMU/OVMF 引导验证本身**未能开始**——两次尝试都在 `bootloader` 自身构建阶段失败,从未产出 `toy-bios.img`/`toy-uefi.img`。这不是"迁移工作量比预期大"这类软性发现,是一个**具体的、可复现的阻塞项**,必须先解决才能继续本 ADR 门禁列的其余步骤（内存管理 crate 逐一核实、真机验证)。

**后续定位（同一 spike 会话内完成，未开新会话)**：没有盲目二分 0.11.8–0.11.16,而是直接用 GitHub API 读取每个 tag 的 `bios/stage-4/Cargo.toml`（`gh api repos/rust-osdev/bootloader/contents/bios/stage-4/Cargo.toml?ref=<tag>`,零构建成本)——发现 `x86_64` 依赖锁定值在 0.11.9→0.11.10 之间从 `"0.14.8"`（caret 语义,可解析到任何 0.14.x)跳到 `"0.15.2"`。`"0.14.8"` 在今天会解析到 **0.14.13**——与 `crates/gos-kernel` 自己钉的版本完全相同。

**验证结果**：`bootloader = "=0.11.9"` 确实解析出 `x86_64 v0.14.13` 并**编译通过**（无 E0053/E0407,`Compiling x86_64 v0.14.13` 后 `bootloader-x86_64-uefi v0.11.9` 编译+安装成功)——本 ADR §一 1.2 提出的核心兼容性问题**已有一个已验证可行的版本组合**。UEFI stage 干净通过；BIOS boot-sector 的构建额外触发了两层与本次 spike 特有的隔离手法相关的环境噪声（均已定位根因,但未继续消耗更多 spike 时间去彻底铺平,详见下段)，不影响"`bootloader 0.11.9` 与本项目 nightly 兼容"这个结论本身的可信度——UEFI 路径（真机启动真正需要的那一半，见 §1.1)已经完整跑通了依赖解析与编译两关。

**未彻底解决、但已诊断清楚根因的环境噪声**（判断为 spike 本身"钉在 `E:\GOSKernel` 目录树内"这个隔离手法特有的问题，**预期在真正迁移 `gos-kernel` 本体时不会重现**——那时不是从内部逃逸父目录配置，而是直接重写 `crates/gos-kernel` 自己的 `.cargo/config.toml`)：(1) BIOS boot-sector 的嵌套 cargo 调用需要 `-Z json-target-spec`,而 spike 为了逃逸 `E:\GOSKernel\.cargo\config.toml` 的 `[build] target` 强制交叉编译（宿主侧 `runner` 工具本身也被强制编译到裸机 target,导致 `serde_core` 等 host-only 依赖找不到 `Option`/`Some` 这类 prelude 项)而从 `/tmp` 跑,连带丢了这个必需的 unstable flag；(2) 在 spike 目录本地补一个只设 `json-target-spec` 的 `.cargo/config.toml` 后,Cargo 配置合并规则对未在近端文件重新声明的 key **不会**从祖先配置里"取消"——`[build] target` 与 `[unstable] build-std` 数组仍从 `E:\GOSKernel\.cargo\config.toml` 继承,即使本地文件显式覆盖 `target` 回宿主三元组,`build-std` 数组仍激活,导致宿主构建里 `core`/`alloc` 被重复从源码编译一份,与 rustup 自带的预编译版本产生 `E0152 duplicate lang item` 冲突。两层都已经用真实构建输出诊断到具体机制,不是"没查出原因就放弃"。

**结论与建议**：真正迁移时,`gos-kernel` 的 `Cargo.toml` 建议钉 `bootloader = "=0.11.9"`（而非 `"0.11"` 隐式解析到不兼容的 0.11.17）,并在设计新的 `.cargo/config.toml`/构建 crate 拓扑时,直接以"这个仓库以后可能有多套 target/build-std 需求（宿主侧镜像构建工具 vs 裸机内核本体）"为前提去写,不要重演 spike 里"事后补丁式覆盖继承配置"的弯路——这正是 §一 1.2 已经预见的"内核在构建拓扑里从最终产物降格为另一个 crate 的输入"那部分工作,现在多了一条具体的钉版本建议。

Spike 目录 [`spike/bootloader-011-toy/`](../spike/bootloader-011-toy/) 保留在仓库中作为可复现证据（`README.md` 已注明：一旦真正的 `gos-kernel` 迁移 PR 存在，应连带删除，不该长期与生产代码并存)。

## 五、门禁核实：内存管理 crate 的 target-JSON 依赖（部分完成）

按 §三门禁"内存管理 crate 逐一核实",对比了 [`x86_64-gos-kernel.json`](../x86_64-gos-kernel.json) 与 rustc 内建 `x86_64-unknown-none`（`rustc -Z unstable-options --print target-spec-json --target x86_64-unknown-none`)的完整字段：

- `data-layout`、`disable-redzone`、`panic-strategy` 三项**完全一致**——这三项是最容易踩坑的 ABI 相关字段,一致是好消息。
- **两项真实差异,足以否决"直接切到内建 target"这个选项**：
  1. 内建 target 显式 `"features": "...,+soft-float"` + `"rustc-abi": "softfloat"`（强制软件浮点,不用 SSE 寄存器)；本仓库的自定义 JSON 没有 `features` 字段,按 LLVM x86_64 默认值走,大概率是**硬件浮点**（SSE2 默认启用)。若 `crates/k-pmm`/`k-vmm`/`k-idt`/`k-heap` 或中断处理路径里有任何 `f32`/`f64` 运算,在早期中断/异常上下文（FPU/SSE 状态尚未 `CR0.EM`/`CR4.OSFXSR` 显式初始化前)触发,两种 target 的行为可能不同——软浮点走库调用不碰 XMM 寄存器,硬浮点会;没有查到本仓库是否已经显式做了 FPU 早期初始化,这是需要另外核实的点,不在本次 spike 预算内。
  2. 内建 target 显式 `"code-model": "kernel"`（假设代码/数据被链接在虚拟地址空间顶部 -2GB 窗口内,x86_64 内核的标准做法);自定义 JSON 没有这个字段,按默认 code-model 走。本仓库**没有独立的 linker script**（`.ld` 文件未找到)——当前链接地址由 `bootloader 0.9.23` 自身的构建流程决定,不在本仓库直接可见,核实"迁移后 code-model 是否仍与实际加载地址匹配"需要先弄清 `bootloader` 0.11.9 默认把内核链接到哪（大概率也是顶部 -2GB,这是 bootloader crate 系列的一贯做法,但"大概率"不等于"已核实")。

**结论（修正 §一 1.2 的一个假设）**：真正迁移时**不建议**切到内建 `x86_64-unknown-none`——应该**保留自定义 `x86_64-gos-kernel.json`**（或从它派生一份新版本),只换 `bootloader` 版本 + 构建方式（§四已验证的"两步构建,不用 artifact-dependency"形状),而不是把 target 本身也换成内建的。这比 §1.2 原文设想的"整套 target/build-std 配置整段替换"要小——`build-std`（`core`/`compiler_builtins`/`alloc`)与自定义 target JSON 都可以照旧保留,因为 §四的 spike 已经证明这套组合能在 `bootloader = "=0.11.9"` 下正常工作（`toy-kernel` 用的是内建 `x86_64-unknown-none` 只是为了让 spike 尽快出结果,不代表迁移必须切过去)。

**(a) FPU/SSE 已核实**：读 [`crates/gos-kernel/src/main.rs:46-58`](../crates/gos-kernel/src/main.rs) 确认——`kernel_main` 的第一段可执行代码（早于 `vaddr`/`meta`/`phys`/`k_fb` 等任何子系统初始化)就是显式 `Cr0::remove(EMULATE_COPROCESSOR)` + `Cr4::insert(OSFXSR | OSXMMEXCPT_ENABLE)`,把 FPU/SSE 状态准备好。这段代码本身的存在就是"当前自定义 target 用硬件浮点,不是软浮点"的独立佐证（若走软浮点,没有理由需要启用 SSE 异常/OSFXSR)。迁移到 `bootloader 0.11.9` 只换入口宏与 `BootInfo` 类型,不改 `kernel_main` 函数体内部这几行的执行顺序,风险已排除。

**(b) 加载地址部分核实,方法论上遇到本 spike 特有的限制**：直接解析当前已编译的真实二进制 `target/x86_64-gos-kernel/debug/gos-kernel` 的 ELF header——`e_type = ET_EXEC`（固定地址,非 PIE),`PT_LOAD` 首段 `vaddr = 0x200000`（2MB,经典低地址布局,不是"kernel"code-model假设的顶部 -2GB 高半区)。这与 §五第(2)条"自定义 JSON 没有 `code-model` 覆盖,默认值应该匹配这个低地址布局"的推测**一致,不是假设**。

为直接验证"bootloader 0.11.9 的 loader 能否接受这种固定地址、非 PIE 的内核"（bootloader changelog 提到 0.11.1 "support for higher half position independent kernels"，需要确认这是否意味着旧的固定地址内核不再受支持),在 spike 里用**真实的 `x86_64-gos-kernel.json`**（而非玩具内核原来用的内建 target)重新编译了 toy-kernel——产出的二进制同样是 `ET_EXEC`,与真实 `gos-kernel` 二进制形状一致,证实了 spike 玩具内核可以复现真实内核的关键 ELF 特征。但让 `bootloader::BiosBoot`/`UefiBoot` 消费这个二进制时,再次撞上 §四已经记录过的 `[unstable] build-std` 继承问题——这次额外确认了一个新的、更精确的子结论：**在 spike 本地 `.cargo/config.toml` 里显式设 `build-std = []` 并不能取消从 `E:\GOSKernel\.cargo\config.toml` 继承的 `build-std = ["core", ...]`**（两次独立复现,`E0152 duplicate lang item` 签名一致)——`[unstable] build-std` 这个 key 在 Cargo 配置层级合并里走的是"追加/合并"语义,不是"就近覆盖"语义,和 `[build] target` 这种标量 key 的合并规则不同。这本身是一个值得记录的 Cargo 配置合并细节,但意味着**这个具体子测试在 spike 现有的目录嵌套方式下无法干净地跑完**——不是 bootloader 0.11.9 或 gos-kernel 二进制形状本身有问题,是 spike 逃逸父目录配置的手法在这一层失效了。

**结论**：(a) 已用真实源码核实,风险排除。(b) 的"是否与真实二进制形状一致"这一半已核实（ET_EXEC、低地址、非 PIE),"bootloader 0.11.9 的 loader 是否接受这种形状"这一半**仍未验证**,原因是 spike 自身的环境隔离限制,不是新发现的兼容性问题。**建议**：真正开始迁移 `gos-kernel` 时,第一步就是从一个不嵌套在任何冲突 `.cargo/config.toml` 之下的全新目录（例如迁移分支自己的工作区,而不是继续在 `E:\GOSKernel` 内部的 spike 子目录里打补丁)重跑这个具体测试——那时 `gos-kernel` 自己的构建配置是被重新设计的,不是被继承/逃逸的,不会重演这个问题。

## 六、门禁收口——真实 UEFI+OVMF 端到端跑通（2026-08-04，同一 session 内完成）

按 §五的建议,把测试搬到完全在 `E:\GOSKernel` 目录树**之外**的位置（`%TEMP%\claude\...\scratchpad\bootloader-test\`),从根上避免与仓库自身 `.cargo/config.toml` 的继承冲突。这次是**从零**独立复现,不是继续修补 spike 子目录里的旧状态。

**中途发现两项新的、真实的问题,均已定位并解决或规避**：

1. **`bootloader` 0.11.9 自带的 BIOS 阶段 target JSON 用的是旧 schema**——`bios/stage-2/3/4` 各自内嵌 `i386-code16-boot-sector.json`/`i686-stage-3.json`/`x86_64-stage-4.json` 等文件,字段如 `target-c-int-width`/`target-pointer-width` 写成**带引号的字符串**（如 `"32"`),而本项目固定的 `nightly-2026-04-02` 的 target-spec-json 解析器要求这些字段是**裸整数**（`32`),报 `invalid type: string "32", expected u16`。这是 `bootloader` crate 自己内嵌文件的 schema 版本落后于本项目 nightly 的 target-spec 解析器——与本 ADR 反复出现的"unstable API 随 nightly 演进,依赖方版本卡在不同代际"是同一类问题,但这次不是 `x86_64` crate 的 `Step` trait,而是 bootloader 自己的构建资产。**这个问题只影响 BIOS 阶段,不影响 UEFI 阶段**（UEFI 用标准内建 target `x86_64-unknown-uefi`,不触碰这些自带 JSON 文件)。
   - **规避方式**：`bootloader` crate 把 `bios`/`uefi` 拆成独立 Cargo feature（`default = ["bios", "uefi"]`,查证自 `Cargo.toml` 原文),`bootloader = { version = "=0.11.9", default-features = false, features = ["uefi"] }` 让 `bios` 相关子 crate 完全不参与编译,问题消失。**这不是权宜之计**——2014 Mac mini（本 ADR 的真实目标)本身就需要原生 UEFI 启动（§一 1.1 已论证 Mac 固件的 legacy/CSM 支持不可靠),BIOS 镜像从来就不是必需产出物,只是 ADR 选项 A 最初"BIOS+UEFI 并存"图省事想两个都要。
2. **kernel 侧 `bootloader_api` 版本必须与 host 侧 `bootloader` 版本严格一致**——玩具内核最初写的是 `bootloader_api = "0.11"`（隐式最新 0.11.17),而 disk-image builder 钉的是 `bootloader = "=0.11.9"`,QEMU 里 UEFI 固件真的加载了 bootloader 阶段代码后,该代码自己校验版本戳,直接 panic："kernel was compiled with incompatible bootloader_api version: invalid len"——这是 bootloader crate 自己的设计特性（一次真实的、有意义的运行时校验,不是 bug),需要两侧钉同一个精确版本。改成 `bootloader_api = "=0.11.9"` 后问题消失。

**修完这两项后,完整端到端验证跑通**（`qemu-system-x86_64 -drive if=pflash,format=raw,readonly=on,file=<OVMF>.fd -drive format=raw,file=toy-uefi.img -serial stdio -display none`,OVMF 固件复制到本地可写目录后加 `readonly=on`——直接指向 `Program Files` 下的原始文件会因 Windows 权限报"拒绝访问"),完整日志证据链：

```
BdsDxe: loading Boot0001 "UEFI QEMU HARDDISK QM00001 " ...
BdsDxe: starting Boot0001 "UEFI QEMU HARDDISK QM00001 " ...
INFO : Framebuffer info: FrameBufferInfo { ... width: 1280, height: 800, ... }
INFO : UEFI bootloader started
...
INFO : Elf file loaded at Pointer { addr: 0x5f5e000, ... }
INFO : Handling Segment: Ph64(ProgramHeader64 { type_: Ok(Load), ..., virtual_addr: 200000, ... })
INFO : Handling Segment: Ph64(ProgramHeader64 { type_: Ok(Load), ..., virtual_addr: 203410, ... })
INFO : Handling Segment: Ph64(ProgramHeader64 { type_: Ok(Load), ..., virtual_addr: 210a40, ... })
INFO : Entry point at: 0x203760
INFO : Jumping to kernel entry point at VirtAddr(0x203760)
```

`virtual_addr: 200000`——**与真实 `gos-kernel` 二进制的 `PT_LOAD` 首段地址 `0x200000` 完全一致**（§五已记录的独立核实结果),不是凑巧接近,是同一个数字。loader 把三段 `PT_LOAD` 按 ELF header 里写的固定地址原样映射,**没有做任何 PIE 重定位或地址平移**,证实 §五"bootloader 0.11.x 仍支持传统固定地址内核,higher-half/PIE 只是新增能力不是强制替代"的判断。QEMU 干净退出（`timeout 15` 触发的正常终止,内核自身 `spin_loop()` 死循环,无 panic 输出)。

**门禁结论（全部关闭)**：
- 版本兼容窗口——`bootloader = "=0.11.9"` 编译通过（§四)。
- FPU/SSE 时序——`kernel_main` 首段代码即初始化,风险排除（§五)。
- ELF 形状——`ET_EXEC`/固定低地址/非 PIE,与真实 `gos-kernel` 完全一致（§五、本节复核)。
- **loader 接受度——本节新增,已用真实 OVMF+QEMU 端到端证实：接受,原样映射,成功跳转到入口点。**
- BIOS 路径——发现独立的、真实的不兼容项（bootloader 自带 target JSON schema 落后于本项目 nightly),**建议真正迁移时不做**,原因见上文第 1 点,不是"没空验证"而是"验证后判断不必要,且原设想的它'并存'的收益本来就低于诚实拆分成两个条目"（mirrors ADR-013 处理 virtio-gpu vs UEFI GOP 的同一原则:一个条目里两种不同成本的工作要拆开算账,不能因为选项 A 最初写了"并存"就不顾新证据继续两个都做)。

**给真正迁移 `crates/gos-kernel` 的具体建议**（本 ADR 仍不实现,这是记录经验证的构建形状供迁移分支直接采用)：
1. `crates/gos-kernel/Cargo.toml`：`bootloader = "0.9.23"` → `bootloader_api = "=0.11.9"`（钉精确版本,不用范围)。
2. 新增一个 host 侧 disk-image-builder（建议放进已有的 `xtask` crate,新增子命令,而不是新建顶层 crate)：`bootloader = { version = "=0.11.9", default-features = false, features = ["uefi"] }`,读取已构建的 `gos-kernel` 二进制路径,调 `bootloader::UefiBoot::new(path).create_disk_image(out)`。
3. `main.rs`：`use bootloader::{entry_point, BootInfo}` → `use bootloader_api::{entry_point, BootInfo}`；`kernel_main` 签名从 `&'static BootInfo` 改 `&'static mut BootInfo`；`boot_info.physical_memory_offset` 等字段现在是 `bootloader_api::info::Optional<u64>` 而非裸 `u64`,所有读取点需要跟着改（`gos_hal::phys::set_phys_offset` 等调用点)——这是本次 spike 没有覆盖的部分,真正迁移时需要单独过一遍。
4. `.cargo/config.toml`：继续保留 `[build] target = "x86_64-gos-kernel.json"` + `build-std`（§五已论证不建议换内建 target);`bootimage runner` 换成指向新 xtask 子命令的 runner,或迁移期间先用显式命令代替 `cargo run` 的隐式 runner。
5. `Makefile`/`.github/workflows/installer-artifact.yml`/`tools/build-installer.ps1`：`cargo install bootimage` + `cargo bootimage` 整条链路替换为新 xtask 命令;`tools/write-usb-image.ps1` 需要确认 `UefiBoot::create_disk_image` 的产物格式（本次验证用的是单一 raw 磁盘镜像,GPT/ESP 结构内嵌,与现有"整镜像 dd 到 U 盘"的写入方式兼容,不需要新脚本逻辑)。
6. `doc/06_运维维护/INSTALL_BARE_METAL_zh.md`：把"进入 BIOS/UEFI 启动菜单"改为明确的"UEFI 启动菜单/按住 Option 键选 EFI Boot"（含 Mac 特有步骤),不再暗示两者等价。

**7. 迁移前发现的关键澄清（`crates/k-fb` 的图形路径与本迁移的关系)**：核实了 `k_fb::init`（[`crates/k-fb/src/lib.rs:216`](../crates/k-fb/src/lib.rs)）——它不是单一路径,而是"HD"（自驱动 Bochs VBE/DispI,端口 I/O 直接探测+编程显卡,与 bootloader 的模式设置完全无关)优先,失败才退回"legacy mode 13h"（依赖 `bootloader 0.9` 的 `vga_320x200` feature 提前切好模式,读固定物理地址 `0xA0000`)。这意味着：
   - **QEMU 环境**（有 Bochs DispI/stdvga 设备,与固件是 BIOS 还是 UEFI 无关）下,`try_set_hd_mode` 探测会成功,`k_fb` 走自驱动路径,**完全不受本次 bootloader 迁移影响**——本 ADR 的迁移可以只改 boot 入口/`BootInfo` 字段访问,不用碰 `k_fb`,QEMU 验证依然有效。
   - **真机（2014 Mac mini,Intel 集成显卡,非 Bochs DispI 兼容)**下,`try_set_hd_mode` 会失败,退回 legacy mode 13h 路径——但这条路径依赖的 `vga_320x200` bootloader feature 在 0.11/UEFI 下**不存在**（UEFI 固件没有"BIOS INT 10h 模式 13h"这个概念),意味着**真机上 `k_fb` 目前两条路径都不可用,会黑屏**,直到有一条消费 `BootInfo.framebuffer`（真正的 UEFI GOP 线性帧缓冲)的新路径。
   - 这**不是本次迁移新引入的缺口**——正是 [ADR-013](./ADR-013-real-hardware-display-mvp.md) 已经明确拆出、标注为"独立、待 bootloader 迁移完成、等 #45 真机解除阻塞后才是验证合适时机"的那部分工作（UEFI GOP backend)。本 ADR 完成后,`BootInfo.framebuffer` 终于存在,那项工作的前置条件被满足,但**实现它本身不在本 ADR 门禁内**——迁移本身（内核能启动、串口/shell 能用、QEMU 下图形照常)与"真机上有画面"是两个不同的判据,前者是本 ADR 的范围,后者留给 ADR-013 那条独立线。

## 七、真正迁移已落地（2026-08-04，同一 session 内完成）——QEMU+OVMF 端到端验证通过

`crates/gos-kernel` 从 `bootloader = "0.9.23"` 迁移到 `bootloader_api = "=0.11.9"`（UEFI-only，`xtask` 新增 `image`/`run`/`qemu` 三个子命令承接原 `cargo bootimage`/`cargo run`/`Makefile` 工作流)。过程中发现并修复了**四个真实 bug**，全部是"旧 bootloader 的宽松内存布局掩盖了既有假设，新 bootloader 的精确布局把它们暴露成硬故障"这同一类问题的不同实例：

1. **物理内存映射不再默认开启**——`Mappings::physical_memory` 默认 `None`（0.9 的 `map_physical_memory` feature 总是开)。`main.rs` 新增 `BOOTLOADER_CONFIG` 显式设 `Mapping::Dynamic`。
2. **`k-vga`/`fbtest.rs` 硬编码物理地址 `0xB8000`**（VGA 文本模式诊断输出)当裸虚拟地址用，从未加 `phys_offset`——0.9 的映射方案下低地址物理页恰好可能落在可用虚拟范围内（未证实为设计保证，只是没炸)，0.11 的 `Dynamic` 映射下这是一个未映射地址，第一次写入即触发 page fault→双重错误→三重错误（QEMU `-no-reboot` 下表现为进程干净退出，容易误判成"卡住"而非崩溃)。两处都改成 `gos_hal::phys::phys_offset() + 0xB8000`，与 `k-fb` 自己的做法同型。
3. **默认内核栈 80 KiB 不够**（`bootloader_api` 文档:默认值 + guard page，0.9 无此机制，此前只是静默腐化相邻内存，从未表现为故障)——GOS 的启动调用深度（`gos_supervisor::bootstrap`/`gos_runtime::reset` 等)会踩进 guard page。改为显式 `config.kernel_stack_size = 1 MiB`（经验值，非量测下限)。
4. **`k-pmm`/`k-vmm`/`gos-hal` 仍直接依赖 `bootloader = "0.9.23"`**，且在 `on_init` 回调里把 `main.rs` 传下来的裸指针（现在指向新 `bootloader_api::BootInfo` 布局)强行转型成旧 `bootloader::BootInfo` 读取——一次类型混淆，读出的 `memory_map`/`physical_memory_offset` 是垃圾数据。表现为一条指向**旧 `bootloader-0.9.34` crate 自身源码**的 panic（`memory_map.rs:72`，"range end index ... out of range"）——panic 位置本身就是排查这个 bug 的关键线索。`k-pmm` 迁移到 `bootloader_api::info::{MemoryRegion, MemoryRegionKind}`（字段形状不同：`region.range.start_addr()`→`region.start` 等)；`k-vmm` 同理迁移 `physical_memory_offset` 读取（新类型 `Optional<u64>`，`.into_option()` 而非旧 `Option<u64>` 的恒等 `.into()`)；`gos-hal` 的 `bootloader` 依赖经核实**从未被使用**，直接删除，不是迁移目标。

**排障方法**：不是靠猜，是靠临时二分诊断日志（在 `boot_builtin_graph` 逐阶段插 `raw_serial_println`，定位到具体在哪一步炸)+ QEMU `-d int,cpu_reset` 读取 `CR2`/`RIP`/`RSP` 精确定位故障地址与触发指令，每次缩小范围一步——四个 bug 是循序浮现的（先解决 1 才能看到 2，先解决 3 才能看到 4)，不是一次性发现的。诊断日志已在修复确认后移除，不留存到生产代码。

**验证证据**（`xtask qemu`，OVMF+QEMU，非模拟)：`xtask: qemu smoke PASS — marker observed`。完整链路：UEFI 固件找到并启动磁盘的 UEFI 引导项 → `bootloader_api` 阶段初始化 1280×800 framebuffer、映射物理内存、创建 bootinfo → 跳转内核入口点 → `kernel_main entered` → FPU/SSE 使能 → `k_fb` 自驱动 Bochs VBE 上屏（1920×1200)→ 23/23 builtin 模块发现+加载 → manifest graph 同步 → supervisor 实现 23/23 模块、17 个已发布能力、0 失败 → graph ready（28 节点/66 边)→ GDT/IDT/PIC 就绪 → ring3 syscall 武装 → **`interrupts enabled — steady-state loop`** → desktop/framebuffer 初始化（1920×1080)→ `vk-input` 轮询持续产生（本 session 全程用来确认"内核跑到稳态"的同一个信号)。

顺带修了两处与本迁移相关、但独立发现的问题：(a) `xtask` 的 `QEMU_SMOKE_MARKER` 常量字符串与 `main.rs` 实际输出的日志文本从未匹配过（`"boot: enabling interrupts; entering steady-state"` vs 实际的 `"interrupts enabled — steady-state loop"`）——这是迁移前就存在的 bug，验证时顺带发现并修正；(b) `tools/build-installer.ps1` 把 `doc/INSTALL_BARE_METAL_zh.md`（单行 stub，指向真正的 `doc/06_运维维护/INSTALL_BARE_METAL_zh.md`）打进每一个安装包——同样是迁移前就存在、借这次改脚本的机会一并修正。

**改动范围**（全部本 session 完成，已验证 `cargo check --workspace`、`tools/verify-graph-architecture.ps1`、`xtask test`（14 个 test result 块全绿）、`xtask qemu` 全部通过）：`crates/gos-kernel`（`Cargo.toml` + `main.rs` + `fbtest.rs`）、`crates/k-pmm`、`crates/k-vmm`、`crates/k-vga`、`crates/gos-hal`（删除未用依赖）、`xtask`（新增 `image`/`run` 子命令，修正 `qemu` 子命令的 marker + 直接调用 QEMU/OVMF 而非 `bootimage runner`）、`.cargo/config.toml`、`Makefile`、`.github/workflows/installer-artifact.yml`、`tools/build-installer.ps1`、`doc/06_运维维护/INSTALL_BARE_METAL_zh.md`。

**未做（明确不在本次范围内）**：UEFI GOP 真机图形路径（ADR-013 的独立线，见 §一第 7 条)；BIOS 双启动（§六已论证放弃,原因是 `bootloader` 自带 target JSON schema 不兼容 + 真机目标本就需要 UEFI)；真机（2014 Mac mini)实测——本 ADR 的验证判据是 QEMU+OVMF,真机验证是 installer 工作本身的判据,不是这个迁移 ADR 的。
