# ADR-007：`gos-hal::display` trait 的范围——与 `k-vga`（文本模式）/ `k-vk-host`（宿主桥接图形）的关系

> 状态：**提案（问题陈述 + 选项，待你选向）** · 日期：2026-06-11 · 配套：[V2 计划 V2.4](../plan/V2_DEVELOPMENT_PLAN.md)（line 89，`gos-hal::display` 原始草案）、[ADR-006](./ADR-006-capability-graph-migration.md)（同一遗留项的"需要独立 ADR"标注）、[ADR-002 §六](./ADR-002-rewrite-engine.md)（渲染模型）
>
> 口径：V2 计划 line 89 描述的 `gos-hal::display` trait——`init(prefer: ResolutionHint) -> Surface` / `surface.lfb()` / `surface.flip()`，Bochs-VBE 为 backend #1，封装"`fbtest.rs:296` 的 DISPI 直写"——与当前 V2 代码库的实际显示架构有两处脱节：(1) `fbtest.rs` 在 V2 代码库中从未存在（V2.3d 已确认，参见 [V2 计划 V2.3 行](../plan/V2_DEVELOPMENT_PLAN.md)）；(2) 当前真实显示路径是两条平行、均非线性帧缓冲（LFB）的机制——`k-vga`（VGA 文本模式）与 `k-vk-host`（宿主桥接 graph-native 显示列表）。本 ADR 处理"`gos-hal::display` 该长成什么样、该对接哪条路径"，**不替你拍板**。

## 一、冲突

### 1.1 `k-vga`：VGA 文本模式，零 trait 抽象

[`crates/k-vga/src/lib.rs`](../crates/k-vga/src/lib.rs)（492 行）是一个单体模块，直接操作两类硬件资源：

- **文本缓冲区**（`0xB8000`，`SCREEN_WIDTH=80` × `SCREEN_HEIGHT=25` 的 `ScreenChar` cell）：`render_cell`/`render_row`/`render_full`（均为 `fn(state: &VgaState, ...)`，经 `text_buffer().add(...).write_volatile(...)` 直写）。
- **VGA DAC 调色板端口**（`0x3C8`/`0x3C9`）：`apply_theme_palette(theme: u8)`（V2.3b 把调色板*数据*上移到 `gos_protocol::theme`，但*写端口*的代码仍在 `k-vga`，且仍是 private `fn`）。

这些函数全部是 crate-private、签名形如 `fn(&VgaState, ...)` / `fn(&mut VgaState, ...)`，没有任何 `trait`/`Surface`/`Backend` 边界——仓库内对 `Surface`/`ResolutionHint`/`lfb`/`flip` 的搜索零命中。

### 1.2 `k-vk-host`：宿主桥接 graph-native 显示列表，已大幅完成

[`crates/k-vk-host/src/lib.rs`](../crates/k-vk-host/src/lib.rs) 的模块文档明确写着："mirrors `k-cuda-host`：emits a stream of structured frames to a host-side helper"——通过专用 COM3 UART 发送 `@gos.vk` *display list*（node=渲染单元，edge=conduit），QEMU 把 COM3 暴露成 TCP，host watcher（`tools/gfx-bridge.py`，现已演进为 `tools/gos-vk-viewer`）渲染成窗口。模块文档原话："The display list is the architectural seam: a later Vulkan host backend can consume the same frames with no kernel-side change."

最近三个提交（`a4cbd8d`/`c03b433`/`9f77984`）已经把这条路径做到：B3b host-bridged GPU viewer 自动刷新/keepalive + 双向输入，并端到端验证了 COM3 输入回环。这**已经是** [V2 计划 V2.5](../plan/V2_DEVELOPMENT_PLAN.md)"Phase I 图形（Vulkan host-bridged）"描述的"host-bridged（像 `k-cuda-host`），scene 子图 → `k-vk-host` 桥接 host Vulkan"的雏形——而 V2.5 的 exit criteria 是"Soul demo: `MATCH...CREATE` → 下一帧 3D 出现新 node"，与 V2.4 line 89 设想的"`gos-hal::display` trait + Bochs-VBE LFB"是**不同的机制**（前者是 graph-native 显示列表协议，后者是像素级线性帧缓冲）。

### 1.3 矛盾的核心

V2.4 line 89 把"显示 HAL"设想成**一种** LFB 抽象（`init`/`lfb`/`flip`，Bochs-VBE 是 backend #1），但：

- 它引用的 `fbtest.rs:296` 不存在；
- 当前没有任何代码路径产生或消费 LFB；
- V2.5 的图形目标已经被 `k-vk-host`（非 LFB、graph-native、host-bridged）实质性推进；
- `k-vga`（V2.3 系列的渲染主体）是文本模式 cell buffer，不是 LFB，trait 化它得到的不会是"`Surface::lfb()`"形状的东西。

"`gos-hal::display` trait（Bochs-VBE backend #1）"这一行 V2.4 交付物，落不到现有任何一条路径上。

## 二、选项

### 选项 A —— 整体推迟到 V2.6（mirrors [ADR-006](./ADR-006-capability-graph-migration.md) 选项 C）

承认 V2.4 line 89 设想的 LFB `Surface`（`init`/`lfb`/`flip`，Bochs-VBE backend #1）只有在 V2.6"真机显示（UEFI GOP / virtio-gpu）"时才有真正的硬件 LFB 可抽象——`gos-hal::display` 整体重新归类为 V2.6 范畴，V2.4 退出判据中删除"显示 HAL"这一项（或重新措辞）。

- **优点**：诚实；不为不存在的 backend 设计抽象。
- **代价**：V2.4 的标题"能力即可达性 **& 显示 HAL**"名不副实，需要改措辞（与 ADR-006 选项 C 同样的代价类型）。

### 选项 B —— 现在提取最小 `gos-hal::display::TextSurface`，backend #0 = 现有 `k-vga`

把 §1.1 列出的私有函数收敛成一个 trait（形如 `trait TextSurface { fn write_cell(&mut self, row, col, ScreenChar); fn set_palette(&mut self, Palette); fn flush(&mut self, dirty: Region); }`），`k-vga` 的现有实现改为该 trait 在一个 `VgaTextSurface` 类型上的 impl，调用点保持逐字节行为不变——镜像 V2.3b"lift palette"（零行为变更）+ V2.4a→b"先纯原语后接线"的模式。

- **优点**：`gos-hal::display` 现在就有真实存在（backend #0），为 V2.6 新增 LFB backend（Bochs-VBE/virtio-gpu/UEFI GOP）预留 trait 接口；不依赖不存在的 `fbtest.rs`。
- **代价**：text-mode cell-buffer trait 的形状和 V2.6 真正需要的 LFB `Surface`（`lfb()`/`flip()`）形状大概率不同——backend #0 的 trait 到 V2.6 加 LFB backend 时可能要推倒重来；`k-vga` 是 `no_std` kernel crate、直接 MMIO/port I/O，**没有 host-harness 基础设施**——"host harness 等价证明"需要先解决"如何在 host 上 mock MMIO/port I/O"，这是一个新的、未解决的设计问题（V2.3b 的 palette 提取没遇到，因为调色板数据本身是纯数据、无 I/O）。

### 选项 C —— `k-vk-host` 的显示列表正式认定为 V2.5 的"Surface"，`gos-hal::display` 只留给 V2.6 真机 LFB，两者并列不互相依赖

把"display"明确分两层，不强行统一：

1. **scene/dev surface**：`k-vk-host` 的 `@gos.vk` 显示列表（已存在，B3b 已验证）——engine 的 `Subscribe`→repaint 反向传播（V2.3a/c）面向这一层，是 V2.5 主线。
2. **boot/真机 surface**：`gos-hal::display`（V2.6 起才需要，UEFI GOP/virtio-gpu，服务于"真机显示"）——`k-vga` 文本模式在 V2.4/V2.5 期间继续作为这一层的占位实现，不强行 trait 化。

- **优点**：承认两套机制目的不同（开发/演示 vs. 真机 boot console），互不牵制；V2.5 主线（已经在跑的 `k-vk-host`）不被"display HAL 该长什么样"卡住。
- **代价**：`gos-hal::display` 仍然"未开始"，只是更明确地推迟并解释了原因；V2.4 标题中的"显示 HAL"承诺基本落空，需重新措辞（与选项 A 代价类型相同，但理由不同：A 是"LFB 暂无落点"，C 是"功能已被 `k-vk-host` 满足，trait 化只是为了 V2.6 真机"）。

## 三、建议与门禁

倾向 **C 的现状澄清 + A 的措辞调整一起做**：`gos-hal::display`（真正的 LFB `Surface` trait）归入 **V2.6"真机显示"**范畴，与已经在跑的 `k-vk-host`（V2.5 scene 显示）并列、不是替代关系；V2.4 退出判据中"显示 HAL"一项重新措辞为"已确认 `k-vk-host` 实质性满足 V2.5 图形需求，`gos-hal::display`（LFB）范围收窄至 V2.6 真机 backend，不在 V2.4 阻塞范围内"。

**B（`k-vga` trait 提取）不是本 ADR 选向的必需项**——若选 C，`k-vga` 文本模式现在不必 trait 化；B 本身还引入"如何 host-harness 化 MMIO/port-I/O 代码"这一新的、未解决的设计问题，若未来需要，建议另开 ADR-008 单独处理，不与本 ADR 的"`gos-hal::display` 范围"问题共享决策依据。

本 ADR 范围**不含** V2.4 第三项遗留（跨域调用走 Grant 路径 + 剩余 4 个 killer demo——capability-path 端到端、hot-swap 真实热插拔、fault-containment 真实故障注入）——IPC 机制问题，性质不同，需独立 ADR。
