# ADR-009：F.5（FAT32 write + journal fsync）与 V2.5"截图依赖落盘"的范围——gfx golden-frame/gfx-fuzz 是否真的需要 F.5

> 状态：**已选向：选项 A（F.5 整体移入 V2.6）** · 提案日期：2026-06-11 · 选向日期：2026-06-12 · 配套：[V2 计划 V2.5](../plan/V2_DEVELOPMENT_PLAN.md)（line 98/104/113/146 已据此措辞，V2.5b 提交时同步更新）、[OPTIMIZATION_PLAN.md](../plan/OPTIMIZATION_PLAN.md)（F.2-F.5 现状）、[ADR-007](./ADR-007-display-hal-scope.md)（"V2.4/V2.5 行与实际架构脱节，重新归类"同型先例）、[ADR-005 §四](./ADR-005-node-mutation.md)（V2.5a，本 ADR 是 V2.5 第二项遗留 V2.5b 的产出）
>
> 口径：V2 计划三处分别写下"F.5（FAT32 write + journal fsync）并入（截图功能依赖落盘）"（line 98）、"F.5 持久化轨...V2.5 前必须并入（截图依赖）"（line 113）、"F.5 不阻塞主线 UI，但 V2.5 前必须并入"（§六规则 5，line 146）——三处共享同一个隐含前提：**"gfx golden-frame/gfx-fuzz 的截图要落盘，落盘＝内核 FAT32 write＝F.5"**。但 F.5 在 `OPTIMIZATION_PLAN.md` 中是全计划中量级最大、完全未启动的"后续切片"（`gos-vfs::FileSystem` 无 write/create、`gos-journal` 纯内存零磁盘接线、`k-fat32` 只读单 cluster、`crates/hypervisor` 对 `BlockDeviceVTable::write_sector`/`flush` 零接线）。本 ADR 检验这一隐含前提，并给出 V2.5b 已经验证可行的替代路径。**不替你拍板**——但与 V2.5b 一并提交，因为 V2.5b 的产物就是本 ADR 建议方案的具体实现，已经跑通。

## 一、冲突

### 1.1 V2.5 真正的退出判据是 harness 绿，不是"用户截图"

V2 计划 line 100 的退出判据原文：

> **退出判据 / Soul demo**：`MATCH ... CREATE (n)` → 下一帧 3D 显示新 node ... Harness：**gfx golden frame、gfx-fuzz**。

line 112（并行轨）：

> **测试基建轨**（贯穿全程）：**lavapipe golden frame**、gfx-harness、**gfx-fuzz**、quiescence harness。每阶强制随附，不单列阶段。

"gfx golden frame (lavapipe)" 和 "gfx-fuzz" 都是**host 侧 CI 测试基建**的标准词汇：

- **lavapipe golden frame**：lavapipe 是 Mesa 的纯软件 Vulkan 实现，CI 里常用来在无 GPU 环境跑 Vulkan 渲染并把输出帧与一张参考（golden）图按字节/像素比较，判定渲染管线有没有跑偏。
- **gfx-fuzz**：对显示协议的 parser/rasteriser 喂随机/畸形输入，断言不 panic、不越界——经典的 host 侧 property/fuzz harness 形态（类比 `gos-rewrite-harness` 里已有的各 property test）。

**两者字面定义都是"host 进程读写 host 文件系统"，与"内核态 FAT32 write"无关。** "F.5 并入" 把它们绑定在一起，是 line 98/113/146 三处共享的额外推断，不是 line 100/112 本身的要求。

### 1.2 ADR-007 已确认：`k-vk-host` 是 host-bridged，帧数据本来就在 host 侧

[ADR-007 §1.2](./ADR-007-display-hal-scope.md) 确认 `k-vk-host` "mirrors `k-cuda-host`"——通过 COM3 把 `@gos.vk` display list 发到 host，`tools/gfx-bridge.py`（已演进出 `tools/gos-vk-viewer`）在 host 侧消费并渲染。B3b（`a4cbd8d`/`c03b433`/`9f77984`）已经把这条路径做到双向输入端到端验证。

也就是说：**"截图"这个动作发生时，frame 数据已经在 host 进程的内存里**——根本不需要"内核先把帧写进 GOS 的 FAT32 文件系统，host 再读出来"这一圈。"截图依赖落盘"暗含的"从内核 FS 读出帧"路径，在 `k-vk-host` 的实际架构下不存在，也从未被需要过。

### 1.3 V2.5b：host 侧落盘截图，今天已经跑通，零 F.5 依赖

本轮（V2.5b）给 `tools/gfx-bridge.py` 加了 `FrameBuffer`（光栅化 display list 到 RGB byte buffer，复用 `TkRenderer` 的画图原语：填充矩形、Bresenham 直线、单像素）+ `write_ppm`（P6 PPM，零依赖、可字节比较）+ `--dump-ppm FILE` CLI（可选 `--replay` 接已捕获的帧）。验证：

```
$ python tools/gfx-bridge.py --check
...
[CHECK]   ppm dump: 1440015 bytes (expected 1440015)  [ok]
[CHECK] PASS - 8 frames parsed, all op counts match

$ python tools/gfx-bridge.py --dump-ppm out.ppm
[DUMP] wrote 800x600 PPM (8 ops) to out.ppm
# header: b'P6\n800 600\n255\n', size 1440015 = 15-byte header + 800*600*3
```

这就是 line 100/112 要求的"gfx golden frame"产物的雏形：`--dump-ppm` 的输出可以直接与一张 checked-in 的 golden `.ppm` 字节比较；`parse_frame`/`FrameBuffer.apply` 是 `gfx-fuzz` 现成的 fuzz 入口（纯 Python、无需 QEMU）。**全部发生在 host 文件系统上，与内核 FAT32/F.5 无关。**

### 1.4 矛盾的核心

V2 计划三处"F.5 V2.5 前必须并入"的措辞，建立在"截图＝内核落盘"这一在 `k-vk-host` host-bridged 架构下不成立的假设上——这与 ADR-007 发现"`gos-hal::display`（line 89）引用的 `fbtest.rs` 不存在、设想的 LFB 路径与实际架构脱节"是**同一类型**的"V2.4/V2.5 行文字写于架构成形之前，现状已超越文字"问题。F.5 本身（FAT32 write + journal fsync）是真实、有价值的工作——但它服务的是 V2.6 line 104 的"`persistent` 属性接真实 FS-backed 边"，不是 V2.5 的 gfx harness。

## 二、选项

### 选项 A —— F.5 整体重新归类为 V2.6 范畴，V2.5 退出判据删除"截图依赖 F.5"（mirrors [ADR-007](./ADR-007-display-hal-scope.md) 选项 A / [ADR-006](./ADR-006-capability-graph-migration.md) 选项 C）

承认"截图依赖落盘"的前提不成立：gfx golden-frame/gfx-fuzz 的 host 侧落盘需求已由 V2.5b 的 `--dump-ppm`/`FrameBuffer` 满足。F.5（FAT32 write + journal fsync）整体移入 **V2.6**，作为 line 104"`persistent` 属性接真实 FS-backed 边"的前置基础设施——这本来就是它的自然归宿（journal/FAT32 write 是"persistent 边"得以落地的存储层）。

- **优点**：V2.5 卸下全计划中量级最大、完全未启动的依赖项；gfx golden-frame/gfx-fuzz harness 现在就能在 `gfx-bridge.py` 上继续建（checked-in golden `.ppm` + 字节比较 + `parse_frame` fuzz harness），不必等 F.5。F.5 work 在 V2.6 有更贴切的落点（直接服务 `persistent` 边），而不是被"截图"这个其实不需要它的理由捆绑提前。
- **代价**：V2 计划 line 98/113/146 三处措辞需要修改（与 ADR-007/ADR-006 同型代价）；若未来确实出现"GOS 内部需要把截图存进自己的文件系统"的真实需求（例如 `k-shell` 的"保存截图"命令面向最终用户），那将是一个独立、新的 F.5 消费场景，需要重新评估时机——但这不是 V2.5 gfx harness 的需求。

### 选项 B —— F.5 保留在 V2.5 时间窗，但解除其与 gfx 退出判据的绑定

不删除"F.5 可在 V2.5 期间推进"（line 113 前半"F.5 可在 V2.1 后任意时点推进"本就允许），但删除 line 98/146 的"必须/截图依赖"措辞——F.5 变成 V2.5 期间*可选*的并行工作，gfx golden-frame/gfx-fuzz 退出判据完全基于 `gfx-bridge.py` 的 host 侧路径。

- **优点**：改动更小——只删"必须"和"截图依赖"的因果声明，不动"F.5 在 V2.5 时间窗"这一时间安排。
- **代价**：V2.5 行的"交付"列表里仍挂着"F.5 并入"，但 V2.5 的退出判据已经不再需要它——容易造成"V2.5 算不算完成"的歧义（F.5 完成与否不影响判据，但字面仍是交付物）。语义上不如 A 干净。

### 选项 C —— 推迟记录依赖，不重新归类（mirrors [ADR-008](./ADR-008-cross-domain-grant-mapping.md) 选项 C）

只记录"F.5 与 V2.5 gfx 判据的绑定缺乏依据"这一发现，不提出具体重新归类方案，留给你在选 ADR-005（A/B/C）时一并评估对本 ADR 的影响。

- **优点**：最小承诺。
- **代价**：不解决任何问题——V2.5 的"F.5 并入"依赖原样挂着，gfx-golden-frame/gfx-fuzz harness 该不该等 F.5 仍不明确。给不出可执行结论。

## 三、建议与门禁

倾向 **A**：F.5（FAT32 write + journal fsync）整体移入 V2.6，作为 line 104"`persistent` 边"的存储层基础——这是它在全计划里唯一有"谁消费它"的位置。V2.5 line 98/113/146 的"截图依赖 F.5 / V2.5 前必须并入"措辞删除；V2.5 的 gfx golden-frame/gfx-fuzz harness 基于 V2.5b 已验证的 `tools/gfx-bridge.py --dump-ppm`（`FrameBuffer` 光栅化 + P6 PPM + `--check` 已纳入 round-trip 校验）继续建设：

- **gfx golden frame (lavapipe)**：`--dump-ppm` 的输出与 checked-in 的 golden `.ppm` 字节比较；"lavapipe"一词在此处的落点是——若未来 `tools/gos-vk-viewer`（或某个真实 Vulkan host backend）替换 `gfx-bridge.py` 的 tkinter/FrameBuffer 光栅化器，`--dump-ppm` 产出的 PPM 即是该 backend 的比较基准（golden），lavapipe 是该 backend 在 CI 里的无 GPU 实现选择——这是后续切片，不阻塞当下。
- **gfx-fuzz**：`parse_frame`/`FrameBuffer.apply` 是现成的纯 Python fuzz 入口（畸形 `VK*` 行、越界坐标、非法颜色），无需 QEMU，可独立成 host harness 步骤。

**本 ADR 范围不含**：(1) ADR-005 选向（A/B/C，CreateNode 接线）——V2.5 的另一项独立遗留，与本 ADR 正交；(2) "GOS 内部截图保存到自己文件系统"这一假设性最终用户功能——若未来提出，是 F.5（V2.6）的一个新消费场景，与本 ADR 处理的"V2.5 gfx harness 是否需要 F.5"是不同问题。

## 四、选向（2026-06-12）：选项 A

采纳选项 A：F.5（FAT32 write + journal fsync）整体移入 **V2.6**，作为 [V2 计划 line 104](../plan/V2_DEVELOPMENT_PLAN.md)"`persistent` 边接真实 FS-backed 边"的存储层基础——这是它在全计划里唯一有"谁消费它"的位置。

V2 计划 line 98/104/113/146 在 V2.5b 提交时已同步改写为"F.5 移至 V2.6"/"自 V2.5 移入"/"不再是 V2.5 前置"/"不是 V2.5 gfx harness 的依赖"等措辞，无需再改。V2.5 行（line 24）的遗留列表同步更新为"V2.5 已闭环，F.5 移交 V2.6 line 104 追踪"。

gfx golden-frame / gfx-fuzz harness 按 §三的建议，基于 V2.5b 已验证的 `tools/gfx-bridge.py --dump-ppm`（`FrameBuffer` 光栅化 + P6 PPM + `--check` round-trip）继续建设，零 F.5 依赖——V2.5 的退出判据（line 100）不受 F.5 进度影响。
