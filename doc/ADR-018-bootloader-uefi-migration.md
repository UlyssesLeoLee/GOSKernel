# ADR-018：bootloader 0.9→0.11+ 迁移——UEFI 真机启动的第一步

> 状态：**选项 A 已选向**（2026-08-04，用户选择"迁移，BIOS+UEFI 并存"）；**spike 已完成，发现真实阻塞项——见 §四** · 提案日期：2026-08-04 · 配套：[ADR-013](./ADR-013-real-hardware-display-mvp.md)（virtio-gpu MVP 已选向 A 落地；本 ADR 是它明确留给"独立 ADR"的另一半）、[tools/build-installer.ps1](../tools/build-installer.ps1) + [tools/write-usb-image.ps1](../tools/write-usb-image.ps1) + [doc/06_运维维护/INSTALL_BARE_METAL_zh.md](../doc/06_运维维护/INSTALL_BARE_METAL_zh.md)（现有 U 盘安装链）、[.github/workflows/installer-artifact.yml](../.github/workflows/installer-artifact.yml)（CI 产物工作流）、V2 计划 line 104（"真机显示"exit criterion，已被 ADR-013 拆分）
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

**未完成的下一步**（供接手者或下一次会话参考，本 ADR 不在这次 spike 里继续搜索)：`bootloader` 0.11.8–0.11.16 之间某个点版本的 `x86_64` 依赖锁定版本,可能恰好落在 v0.14.11–v0.14.13 兼容窗口内——用 `cargo tree -p x86_64` 对每个点版本做一次**依赖解析检查**（不需要全量构建,比本次两次多分钟构建快得多)可以二分定位。若窗口内确实没有任何 `bootloader` 0.11.x 点版本命中,备选项是：(a) 提升 nightly 版本直到 `x86_64 v0.15.x` 兼容,同时验证 `crates/gos-kernel` 自己的 `x86_64 = "0.14"` 依赖是否也要同步升级到 0.15 系列（这本身是新的兼容性核实,不能假设"顺带就好"),或 (b) fork/patch `x86_64` crate 的 `Step` impl 到 `Step` trait 的本 nightly 实际形状,较重,不推荐。

Spike 目录 [`spike/bootloader-011-toy/`](../spike/bootloader-011-toy/) 保留在仓库中作为可复现的失败证据与继续排查的起点（其 `README.md` 已注明：一旦真正解决了兼容窗口问题，应在折入 gos-kernel 本体迁移的 PR 里连带删除，不该长期与生产代码并存)。
