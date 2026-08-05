# ADR-013：真机显示 MVP——一个条目里藏着两种成本

> 状态：**选项 A 已选向；virtio-gpu 发现骨架已落地**（2026-08-03）；UEFI GOP 的独立 ADR 已写出——见 [ADR-018](./ADR-018-bootloader-uefi-migration.md)（提案待选向，2026-08-04）· 提案日期：2026-06-12 · 配套：[V2 计划 line 104](../plan/V2_DEVELOPMENT_PLAN.md)（"真机显示（UEFI GOP / virtio-gpu backend）"）、[ADR-007](./ADR-007-display-hal-scope.md)（`gos-hal::display` LFB `Surface` trait 范围已收窄至本 ADR）、`crates/k-net`（PCI 发现层 + virtio 检测，本 ADR 的可复用precedent）、任务 #45（installer 真机验证，blocked on hardware）
>
> 口径：V2 line 104 把"UEFI GOP"和"virtio-gpu"并列写在同一个括号里，读起来像"两个可互换的 backend 选项"。调查后发现二者的**依赖结构完全不同**——virtio-gpu 是 `k-net` 已经探测到、且有完整可复用 PCI 发现层的 PCI 设备，加一个驱动 crate 量级；UEFI GOP 需要从 `BootInfo` 拿到 framebuffer 句柄，而当前 `bootloader = "0.9.23"`（[main.rs:9](../crates/hypervisor/src/main.rs)）的 `BootInfo` **没有这个字段**——这是 bootloader 0.10/0.11 重写后才引入的，意味着"加 UEFI GOP backend"的真实第一步是**整条 boot pipeline 的大版本迁移**，量级与"显示 backend"完全不同。本 ADR 把这一个条目拆成两个，分别定价。

## 一、问题陈述

### 1.1 UEFI GOP 的隐藏前提：`BootInfo` 里没有 framebuffer

[`crates/hypervisor/src/main.rs:9,18,225`](../crates/hypervisor/src/main.rs)：

```rust
use bootloader::{entry_point, BootInfo};
...
entry_point!(kernel_main);
...
fn kernel_main(boot_info: &'static BootInfo) -> ! {
```

[`crates/hypervisor/Cargo.toml:42`](../crates/hypervisor/Cargo.toml)：`bootloader = { version = "0.9.23", features = ["map_physical_memory"] }`。

bootloader 0.9.x 的 `BootInfo`（`memory_map`/`physical_memory_offset`/`recursive_index` 等字段）是 **BIOS 时代**的产物，不携带任何 framebuffer/GOP 信息——UEFI GOP 的线性帧缓冲地址、分辨率、像素格式（`FrameBufferInfo{byte_len, width, height, stride, pixel_format}`）是 bootloader 0.10/0.11 大重写后才加入 `bootloader_api::BootInfo.framebuffer: Option<FrameBuffer>` 的。0.11 同时把构建方式从"`bootimage` cargo 子命令 + 自定义 `x86_64-gos-kernel.json` target + build-std"（当前 [`.cargo/config.toml`](../.cargo/config.toml) 的形态）换成了 `bootloader::BiosBoot`/`UefiBoot` builder API（build.rs 里调用，产出磁盘镜像）。

**这意味着**："加一个 UEFI GOP display backend" 的真实第一步不是写 GOP 相关代码，而是：
1. `bootloader` 0.9.23 → 0.11.x 升级——entry-point 宏、`BootInfo`/内存映射类型全部改变；
2. 构建管线重写——`Cargo.toml`、`.cargo/config.toml`、`x86_64-gos-kernel.json`、`Makefile`、`run.ps1`、`tools/build-installer.ps1`、CI（`graph-governance.yml`）全部涉及；
3. `main.rs` 的整个 `kernel_main` 入口签名与初始化早期路径（GDT/IDT 等在新 boot-info 布局下的地址假设）需要重新验证。

这是**一个 boot-protocol 迁移级别的工作**，与 ADR-011（hypervisor 改名，3 文件 diff）或 ADR-012（新枚举变体）完全不是一个量级——但 V2 line 104 把它写得和"virtio-gpu backend"并列，仿佛两者成本相近。

### 1.2 virtio-gpu：`k-net` 已经探测到的 PCI 设备，复用现成发现层

[`crates/k-net/src/lib.rs`](../crates/k-net/src/lib.rs) 已经有：

- 完整 PCI 配置空间访问层：`pci_config_read_dword`/`pci_config_write_dword`（端口 `0xCF8`/`0xCFC`，PCI 机制 #1——BIOS 与 UEFI 真机均支持，非 QEMU 专属），总线/插槽/功能扫描，BAR 读取（`STAGE_BAR_READY`，line 70）。
- **已经识别 virtio 厂商 ID**：`const VIRTIO_VENDOR_ID: u16 = 0x1AF4`（line 61），`DRIVER_VIRTIO`（line 65），扫描时 `vendor_id == VIRTIO_VENDOR_ID => DRIVER_VIRTIO`（line 487-488）。当前状态："virtio-net discovered; native datapath still pending"（line 1192）——即 PCI 层发现已完成，只缺协议层（virtqueue 等）。

virtio-gpu（同一 `0x1AF4` 厂商 ID 段下的另一个 device ID）走的是**完全相同的发现路径**——不需要触碰 `bootloader`/`BootInfo`/构建管线，是"新增一个 `k-*` 驱动 crate，复用 k-net 已验证的 PCI 扫描+BAR 映射模式"量级的工作，与 e1000（k-net 已驱动）同级。

**但 virtio-gpu 严格意义上不是"真机"**——它是 QEMU/云虚拟化的 paravirtualized 设备，真实物理主板上不存在。它满足"图形路径不依赖 host-bridge 进程、由内核自身驱动"，但 §三选项里需要明确这不等于"在真机上能用"。

### 1.3 与 #45（installer 真机验证）的关系

#45 当前 blocked on hardware。即使本 ADR 现在选向并开始实现，"真机"那一半的验证（UEFI GOP 在真实主板上点亮）仍然等 #45 解除阻塞才能跑——本 ADR 现在能做的只是"为将来真机验证准备代码路径"，而 virtio-gpu 路径**在真机上预期就是不可用的**（设计上如此，非 bug），#45 解除阻塞后应预期验证的是 k-vga（文本模式，BIOS/真机皆可工作）+（如果选项 B 已完成）UEFI GOP LFB，不包括 virtio-gpu。

## 二、选项

### 选项 A —— virtio-gpu 优先作为 V2.6 MVP，UEFI GOP 拆出独立 ADR

"真机显示"拆成两个条目：

1. **V2.6 现在**：新 crate（如 `k-virtio-gpu`），复用 `k-net` 的 PCI 发现/BAR 映射模式，识别 virtio-gpu 设备，达到"k-net 式的 `STAGE_BAR_READY` 骨架"（设备发现 + BAR 映射 + 驱动状态机骨架）——完整 2D/3D 数据通路（virtqueue、command queue）不在 MVP 范围。QEMU 可直接验证（`-device virtio-gpu-pci`）。
2. **独立 ADR（V2.6 之后或与 #45 同批）**：UEFI GOP，明确框定为"bootloader 0.9→0.11+ 迁移"，其产出（`BootInfo.framebuffer`）是后续任何真机 LFB backend 的共同前提，与"显示"本身解耦——这是一个 boot-protocol ADR，不是显示 ADR。

V2 line 104 的"真机显示"exit criterion 重新措辞为两段：virtio-gpu MVP 证明"图形路径可脱离 host-bridge、由内核自身驱动"（V2.6 范围内可关闭）；UEFI GOP/真机 LFB 留给 bootloader 迁移 ADR + #45 解除阻塞后验证。

- **优点**：现在就能做、有现成模式（k-net `DRIVER_VIRTIO`）、QEMU 立即可验证；不在没有真机的情况下假装"真机显示"已完成。诚实拆分两种不同性质的工作（mirrors ADR-009/010 拆分"截图"与"持久化"、"F.5-logic"与"F.5-wiring"）。
- **代价**：V2.6 退出判据里"真机显示"这一项实质性内容变成"virtio-gpu MVP"，UEFI GOP 真正完成的时间点推到 V2.6 之外——需要在计划文档里说清楚这不是"放弃"，而是"原条目拆分后,其中一半被重新归类"（与 ADR-009 给"截图"deliverable 的处理同型)。

### 选项 B —— 接受 bootloader 迁移作为 V2.6 范围本身

承认"真机显示"=bootloader 0.9→0.11+ 迁移 + GOP framebuffer 接入 + 基本 LFB 绘制（如 blit k-vga 的 cell buffer 经字形表渲染到像素 framebuffer），这就是 V2.6 里最大的单项工作，可能需要类似 ADR-014 的"主线 + Plan-B escape hatch"结构。

- **优点**：直面 V2 line 104 字面意思的"UEFI GOP"，一次到位；完成后 `gos-hal::display`（ADR-007 选项 B/C 待选的 trait）有了第一个真正的 LFB backend 可以抽象。
- **代价**：构建管线重写波及面极广（§1.1 列举的 7+ 文件/脚本），且 bootloader 0.11 的 API 与当前 0.9 形态差异之大,本身就值得单独的"迁移 ADR"先把 diff 摸清——在那之前把它算作"V2.6 一项"，对 V2.6 整体时间线的影响被严重低估。

### 选项 C —— 两者都不在本轮做，"真机显示"整体移到 V2.6 之后

类似 ADR-009 把 F.5 从 V2.5 移到 V2.6 的处理：承认 #45 本身 blocked on hardware 的现实下，"真机显示"的"真机"验证半部分这一轮无论如何跑不完整,先把 V2.6 其余条目（installer 流程本身、`.gitignore` 清理等不依赖真机的部分）跑完。

- **代价**：V2 line 104 的承诺继续悬空；virtio-gpu MVP 本来是"现在就能做、QEMU 可验证"的，推迟没有技术理由,只是排期选择。

## 三、建议与门禁

倾向 **A**：把一个被并列书写、实则成本相差一个数量级的条目拆开——virtio-gpu MVP 复用 k-net 已经埋好的 `DRIVER_VIRTIO` 检测种子，是"现在就能做、QEMU 可验证、不碰构建管线"的 V2.6 范围；UEFI GOP 诚实标注为"bootloader 0.9→0.11+ 迁移"，拆给独立 ADR，与 #45 解除阻塞的时间线天然对齐（真机到位时，才是验证 UEFI GOP LFB 的合适时机；现在做这个迁移,验证手段仍只有 QEMU+OVMF，价值有限）。

**门禁**：virtio-gpu MVP 的"最小"边界=设备发现+BAR 映射+状态机骨架（mirrors `k-net` 当前对 virtio-net 的"discovered; datapath pending"状态)——完整 2D 命令队列、与 `k-vk-host`/`gos-hal::display`（ADR-007）的对接方式是否需要、长什么样,是后续步骤,不在本 ADR 门禁内,避免重犯 V2 line 104"一句话掩盖巨大工作量"的错误。bootloader 迁移 ADR 的门禁是"先摸清 0.9→0.11 的完整 diff 清单"（entry point、`BootInfo` 字段、构建命令、CI），再决定是否值得在 V2.6 内做或推到 V3——本 ADR 不替那个迁移 ADR 拍板,只确认它的存在与大致形状。

## 四、virtio-gpu MVP 落地状态（选项 A，2026-08-03）

新 crate [`k-virtio-gpu`](../crates/k-virtio-gpu/src/lib.rs)：PCI 扫描（class 0x03 显示控制器 + vendor 0x1AF4）、BAR 解析、`pci_enable_device`，状态机停在 `STAGE_BAR_READY`——**没有** `STAGE_DEVICE_READY`（那意味着 virtqueue/命令队列已协商，本 MVP 明确不做）。逐字节复用 `k-net::probe_network_device`/`parse_pci_bars`/`pci_enable_device` 的既有形状（未共享 crate——两边各自 40 行左右，不值得为此拆一个依赖边）,只换了匹配的 PCI class。

**接线**（`crates/gos-kernel/src/builtin_bundle.rs`）：完整 builtin module 注册（`K_VIRTIO_GPU_ID`、`GPU_PERMS`/`GPU_EXPORTS`/`GPU_IMPORTS`、`GPU_NODE_SPECS`、`GPU_MANIFEST`、`BuiltinModule::Native` 条目、`BUILTIN_SUPERVISOR_MODULES` 条目、`boot_dep_rule`（GPU 依赖 VGA）、`activate_kernel_tier_nodes` 里紧跟 `k_net::NODE_VEC` 之后），与 k-net 同一"tier"、同一形状——不是临时拼接。导出能力 `gpu`/`status`（`namespace: "gpu"`，与既有的 `cuda`/`bridge` 区分,因为 `gpu`/`gpu status` 这两个 shell 命令字符串**已经**被 k-cuda 的既有别名占用）。

**shell 命令**：`vgpu` / `vgpu status`（不是 ADR 原文可能暗示的 `gpu status`——落地时发现 k-shell 里 `gpu`/`gpu status` 已经是 `cuda`/`cuda status` 的既有别名，命名冲突,与 ADR-017 的 `ai`/`ask` 冲突同类,同样选择改名而不是覆盖既有行为）。走既有的"resolve capability 到模块级 `AtomicU64` target（mirror `NIM_TARGET`）+ `emit_target_signal` + `gos_runtime::pump()`"模式,驱动自身通过 `GPU_CONTROL_REPORT` 信号在自己的控制台 sink 上打印完整探测报告（pci 地址、vendor/device、stage、BAR 值）。

**验证**：`cargo check -p gos-kernel`、`cargo check --workspace`、`tools/verify-graph-architecture.ps1` 均绿。QEMU headless 实测两次（`qemu-system-x86_64 -display none`,`bootimage-gos-kernel.bin`）：带 `-device virtio-gpu-pci` 与不带,内核均稳定跑到"idle 稳态"(`vk-input` 轮询输出持续产生,无 panic、无卡死)——证明新 PCI 扫描代码在设备存在/不存在两种情况下都不会拖垮启动。未做的：没有自动化按键注入 + VGA framebuffer 截屏来逐字核对 `vgpu status` 的渲染文本——这需要一整套 QEMU 键盘/显示自动化,超出本切片验证预算;结构上与 `k-net` 的 `net status` 完全同型（同一批打印 helper、同一 `print_probe_report` 形状),置信度来自这一同构性,而非像素级截图比对。

**明确未做（门禁外）**：完整 2D 命令队列（virtqueue 协商、`VIRTIO_GPU_CMD_*`）、与 `k-vk-host`/`gos-hal::display` 的对接、UEFI GOP（独立、待 bootloader 迁移 ADR，等真机 #45 解除阻塞后才是验证的合适时机——见本文件 §一 1.3、2026-08-03 前收到的用户上下文：目标真机是 2014 Mac mini,USB 引导,已确认存在但仍按"先完成开发再做安装包"的顺序排在后面）。
