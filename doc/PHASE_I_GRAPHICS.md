# Phase I — 图形管线 (Host-Bridged Vulkan, 产品级 Gen-1)

> 起草：2026-05-17
> 范围：在 **H.1.x (Cypher 写真接通 runtime)** 之后启动，引入 Vulkan host-bridge，使 GOS 拥有"次世代图论 3D 交互界面"的初代产品形态。
> 前置：H.1.x 全绿（含 control-plane envelope 订阅）；D.1/D.2 xtask qemu verb 可用。
> 并行：F.5 写路径（关机持久化）。F.5 不阻塞 Gen-1 单 session 可用性，但要在 Gen-1 release 前并入。
> 不前置：B.4 / E.3。host-bridge 在 Ring 0 plugin 内即可工作；virtio-gpu 路径(Phase I.2) 才需要 PCIe + DMA + CR3 隔离。

---

## ⚠️ 实施现状（2026-05-17 之后的实际走向）

本文档原文是 Gen-1 host-bridged Vulkan 的规划稿。**真实落地走了两条并行线**，请对照阅读，不要把原文档当作"已发生"的事实。

### 线 A — host 端 wgpu 可视化（I.0 → I.2.2 实际完成）

- **后端选择改为 wgpu，不是 `ash`/Vulkan**。原因：开发-期更友好的跨后端、单一 crate 解决所有平台；后续如有需要可换回 `ash`。
- ✅ **I.0** `gos-gfx-protocol` crate（无 std）：RenderCommand 枚举 + handle 类型 + Bridge wire format（16-byte 帧头 + CRC32-IEEE） + `RenderBackend` trait
- ✅ **I.1.0** Wire transport：encode/decode round-trip + 25k 随机字节 fuzz test（gfx-harness）
- ✅ **I.1.1** `gos-gfx-bridge-host` crate（standalone workspace，stable toolchain）：`WgpuBackend` headless → 单三角形 渲染测试通过
- ✅ **I.1.2** Quota + handle 回收 + decoder fuzz
- ✅ **I.2.0** `k-scene` crate：SceneSnapshot → RenderCommand 流，100-node 网格 trace 测试
- ✅ **I.2.2** `gos-visualize` 可执行二进制（winit + wgpu surface + push-constant view-proj）：宿主机上启动可旋转 3D demo，64 个 cube + 键盘控制相机

**未完成 / 推迟（原文档列为 release blocker，实际未交付）：**

- ❌ **I.4.1–I.4.8 测试基础设施全部 deferred**：
  - lavapipe golden frame diff（I.4.3）—— **没建**
  - `gfx-smoke` / `gfx-interact` / `gfx-update-goldens` xtask verb —— **没建**
  - `gos-gfx-bench` 性能回归 —— **没建**
  - `gos-gfx-fuzz` cargo fuzz target（only the in-line property test in gfx-harness exists）—— **没建**
  - manual QA checklist `doc/PHASE_I_QA.md` —— **没建**
- ❌ **`k-vk-host` 内核插件**：原文档说"内核侧 plugin，把命令推到 hypervisor bridge"——**这个 crate 不存在**。Bridge transport 只在 host 端单元测试里跑过 round-trip
- ❌ **真实 carrier**（共享内存 ring / hypervisor escape）：原文档说 I.1.0 ABI 之后开始接 carrier ——**未启动**。今天 wgpu host backend 只接受 in-process 调用，没有跨 process / 跨 VM 边界
- ❌ **I.1.1 golden PNG diff**：原计划 lavapipe + 像素 SSIM，**实际只跑了"≥50 colored pixels"启发式**

### 线 B — 内核内软光栅 UI（I.3.0 → I.3.11 实际完成，原文档完全没规划）

这是一条与原文档**正交的线**，2026-05-17 之后用户要求"3D 界面要在虚拟机里启动"才开的支线。所有内容均在 mode 13h（320×200×256 色板）实现，零 GPU 依赖：

- ✅ **I.3.0** bootloader `vga_320x200` feature 接通，拿到线性 framebuffer
- ✅ **I.3.1** `k-fb` crate：palette + clear / fill_rect / hline / vline / stroke_rect / put_pixel
- ✅ **I.3.2** Boot 末尾画 tile-grid kernel 节点列表
- ✅ **I.3.4** `font8x8` 接入 + `draw_glyph` / `draw_text`
- ✅ **I.3.5** `graph_generation` tick 触发实时重绘
- ✅ **I.3.8** `k-rast` crate（无 std + libm）：Mat4 (perspective/look_at/mul) + Bresenham line + 半空间扫描三角形填充 + painter's algorithm 深度排序；`paint_3d_view` 替换 tile-grid 为 3D cube + edge line 场景，自动 yaw 旋转
- ✅ **I.3.9** F1-F8 键盘 → camera（k-ps2 IRQ post stage 写 k-fb 公共原子）
- ✅ **I.3.10** Cube 顶部标注 plugin name（projected screen pos + shadow outline）
- ✅ **I.3.11** Mouse hit-test + 选中环 + 右下角详情面板（plugin/vector/type）

**线 B 的已知债务**（参见审核报告 2026-05-17）：

- `k-fb::LOCK` 是 per-pixel `spin::Mutex`，单 CPU 下隐性，SMP 上线后 cache-line ping-pong。今天已加 `force_clear` / `force_fill_rect` 无锁变体专给 panic_handler 用，避开 panic 死锁
- `SELECTED_NODE_SLOT: AtomicI8` 上限 127 节点；MAX_NODES = 64 当下安全
- `k-rast::tests` 内联在 no_std crate 里、workspace target 是 kernel，`cargo xtask test` **不会执行这些 assert**。需要后续把它们搬到 `host-tests/k-rast-harness`
- `bridge-host` 的 `.cargo/config.toml target-dir` 是相对路径，从 workspace 根跑 `cargo -p` 会选错 target dir；xtask 用 `current_dir` 规避了

### 线 A vs 线 B 的关系

两条线**几乎独立**：
- 线 A 的产物（wgpu / winit visualizer）跑在宿主机上，**永远不在 QEMU 里**
- 线 B 的产物（mode 13h + k-rast）跑在 QEMU 内核里，**永远不调用 wgpu**
- 共用的只有 `k-scene` 的几何 helper（cube vertex 布局），且共享程度有限

**何时合流？** 在原文档的 I.2 (virtio-gpu) 真正落地、内核能跨 hypervisor 边界把 RenderCommand marshall 给 host 的那一天。目前没有切片正在做这件事；这是 Phase I.x 长期方向。

### Reading guide

下面的原文档（"## 设计立场"开始）**仍是 Gen-1 的正式规范**，描述目标态。实际进度按以上"现状"反映。新切片若要按原文档继续推（比如真正补 I.4.3 lavapipe goldens），应同时更新本节 ✅ / ❌ 状态。

---

## 设计立场

GOS 是图论 OS，UI 也必须是**图原生**的：节点 = 渲染单元，边 = 视觉/数据通路，相机/主题/布局都是图节点。Vulkan 只是底层管线，不应泄漏到 shell/cypher 语义。

裸机直驱 Vulkan 是 6~18 月级别的 GPU driver 工程，**不在 Gen-1 范围**。Gen-1 走 host-bridge：和 `k-cuda-host` 同构，hypervisor 把 Vulkan 命令 marshall 到宿主进程。QEMU 环境下产品可用；真机部署 = Phase I.2 (virtio-gpu) / I.3 (real driver)。

**核心闭环（Gen-1 灵魂演示）：**

```
shell> MATCH (a:net.uplink) CREATE (a)-[:Mount]->(clipboard.mount)
       │
       ├─> k-cypher.parse → CypherMutation::AddEdge
       ├─> gos-cypher-mut::pre_validate          (H.1.x.3)
       ├─> supervisor::MutationDispatcher::apply (H.1.x.2)
       ├─> gos-runtime::apply_edge_mutation      (H.1.x.1)
       │     └─> 边表更新 + restart_generation++
       ├─> ControlPlaneEnvelope(MutationApplied) → shell + k-scene 订阅
       └─> journal ring append
                          │
       ┌──────────────────┘
       ▼
k-scene 收到 MutationApplied → 增量更新 instance buffer →
   下一帧 (≤16ms 后) 3D 视图出现新边的 bezier line
```

---

## I.0 ABI 与边界（先于代码）

### 新 crate

| crate | no_std | 职责 |
|---|---|---|
| `gos-gfx-protocol` | ✅ | 渲染命令枚举、句柄类型、错误码、ABI version 三元组（vk-bridge 专用） |
| `k-vk-host` | ✅ | 内核侧 plugin：暴露 `gfx/surface`、`gfx/pipeline`、`gfx/buffer` 节点，把命令推到 hypervisor bridge |
| `gos-gfx-bridge-host` | ❌ (std/host) | 宿主进程：解析命令、调 `ash` (Vulkan binding)、显示窗口、回传 frame stats |
| `k-scene` | ✅ | 场景图 plugin：订阅 `ControlPlaneEnvelope::MutationApplied`，把 runtime 节点/边转换为渲染命令流 |
| `k-camera` | ✅ | 相机 plugin：暴露 `gfx/camera` 节点，接 `k-mouse` orbit/pan/zoom、`k-ime` WASD |

### Bridge transport

复用 `k-cuda-host` 的 hypervisor escape 通道（`hypervisor::host_bridge`）；新增 `BridgeChannel::Gfx`。命令 payload：固定头 + 变长 SoA，参考 `gos-journal` envelope 格式约定（小端、固定 record_size、CRC32 trailer）。

### Resource 注册

延续 G.4：
- `RESOURCE_GFX_SURFACE`
- `RESOURCE_GFX_PIPELINE`
- `RESOURCE_GFX_BUFFER` (vertex / instance / index / uniform)
- `RESOURCE_GFX_TEXTURE`

全部接入 supervisor quota 框架（默认 quota=0，必须显式 grant；degraded module 拒绝）。

---

## I.1 渲染命令集（最小闭环）

Gen-1 不暴露完整 Vulkan，抽象到"够画 3D 图"的层级：

```
CreateSurface { width, height, present_mode } -> SurfaceId
CreatePipeline { kind: NodeInstance | EdgeLine | Text2D, shader_blob } -> PipelineId
UploadBuffer { kind, bytes, hint: Static | Dynamic } -> BufferId
UploadTexture { format, w, h, mips, bytes } -> TextureId
BeginFrame { surface }
BindPipeline { pipeline }
BindBuffers { vertex, instance, index, uniform }
DrawInstanced { index_count, instance_count }
EndFrame
DestroyXxx { id }
```

Shader：HLSL → DXC → SPIR-V，预编译产物嵌在 `k-scene` 的 `.gos.shaders` section，**运行期不编译**（避免 host 端 dxc 依赖泄漏到 OS 语义）。

---

## I.2 场景图 → 渲染映射

`k-scene` 订阅 H.1.x.4 的 control-plane envelope，增量维护：

- **节点视觉**：每个 graph node → 一个 instance（位置 / 颜色 / 形状索引 / scale）。形状库 Gen-1 内置 8 个：sphere、cube、octahedron、torus、plane、capsule、tetrahedron、glyph_quad
- **边视觉**：directed edge → cubic bezier line strip；Mount/Use 用不同颜色 + dash pattern
- **布局**：Gen-1 用 force-directed (Barnes-Hut θ=0.7) 在 CPU 上跑，结果写入 instance buffer。GPU 加速布局是 I.x 后续
- **相机**：`gfx/camera` 节点接 `k-mouse` orbit/pan/zoom + `k-ime` WASD（已有 plugin）
- **HUD / shell 集成**：shell 文字层用 Text2D pipeline 叠在 3D 上方，PgUp/PgDn 在 2D / 3D 视图间切换
- **Theme 集成**：`theme.current -[use]-> theme.wabi|theme.shoji` 切换时，`k-scene` 收到 `RebindUse` envelope → 更新形状库 + 配色 + shader uniform，不重启

### 不变量

- `k-scene` **不直接调** Vulkan，只产 `RenderCommand`；测试时换 mock dispatcher（保证 golden test 不需要真 GPU）
- 场景图状态必须能从 graph state + envelope 重放完全重建（无隐藏渲染状态），保证 H.5 snapshot/migration 后视觉一致
- `RenderCommand` 序列对任意 mutation 序列必须 well-formed（无 dangling handle）— 由 `gos-verify` property test 守护

---

## I.3 产品级特性清单（Gen-1 必须验收）

| # | 项 | 验收 |
|---|---|---|
| 1 | 60 fps @ 1080p、5000 节点 / 20000 边 | host bridge frame time p99 < 16ms |
| 2 | 启动 → 第一帧可交互 < 2 秒 | 串口 `boot` 时间戳到 `gfx: first frame submitted` 差 |
| 3 | Cypher mutation 反应延迟 ≤ 1 帧 | `CREATE` envelope 到 `DrawInstanced` 含新 instance 的时间差 |
| 4 | 主题切换不重启 | `RebindUse theme.current` 后下一帧风格切换，无 surface 重建 |
| 5 | Surface lost / device lost 优雅恢复 | host kill 渲染窗口 → guest 收 `GfxDeviceLost` → 自动重连不 panic |
| 6 | 截图 | shell `gfx capture <path>`，PNG 写盘走 F.5（F.5 未完成时走 host bridge 直存宿主磁盘） |
| 7 | 输入闭环 | mouse orbit / WASD pan / scroll zoom / Esc 回 2D shell 全可用 |
| 8 | 配额执行 | grant=0 时 CreateSurface 必须返回 `HeapQuotaExceeded`，不 OOM |

---

## I.4 测试工程化（与 Phase I **同步**建设，不是事后补丁）

> 你说"足够的自动测试工程确保健全"——这是 Gen-1 的核心承诺。下面每一项都是 release blocker。

### I.4.1 Protocol 单元测试 — `gos-gfx-protocol/tests/`

- envelope 编解码 round-trip
- CRC mismatch / version mismatch / 截断三类错误码独立
- 句柄回收：destroy 后 use → `InvalidHandle`，不 UB

### I.4.2 Host harness — `host-tests/gos-gfx-harness`

- `k-vk-host` plugin 在 stub `RenderBackend` 上跑端到端，验证命令序列正确
- Mutation→render：构造合成 `ControlPlaneEnvelope::MutationApplied` 流，断言 `k-scene` 产出的 `RenderCommand` 序列等于 golden 文件（文本形式，便于 diff review）
- 配额：grant=0 / grant=1 surface → 第二个 CreateSurface 拒绝

### I.4.3 Golden frame 测试 — `host-tests/gos-gfx-golden`

- 宿主用 **lavapipe**（Mesa 软件 Vulkan）跑实际渲染
- 固定 seed + 固定场景 → PNG 输出 → 与 `test-frames/gfx-golden/` 下 baseline 像素 diff（容差 < 0.5%，SSIM 阈值）
- CI 必须能跑（lavapipe 是无头 CPU 实现，GitHub Actions ubuntu-latest 可装）
- baseline 更新走 PR review：`cargo xtask gfx-update-goldens` 生成新 PNG，review 视觉差异后合入

### I.4.4 Property 测试 — 扩展 `gos-verify`

- `k-scene` 状态机不变量：节点数 == NodeInstance buffer 长度；边数 == EdgeLine draw call 数
- `RenderCommand` 序列对任意 H.1.x mutation 序列都 well-formed
- envelope 顺序保序：journal replay 出的渲染最终态 == 原始 envelope 流的最终态

### I.4.5 QEMU smoke — 扩展 xtask

- 新 verb `cargo xtask gfx-smoke`：启动 QEMU + host bridge → grep 串口 `gfx: first frame submitted` → 超时 30s 失败
- 新 verb `cargo xtask gfx-interact`：scripted input（虚拟 mouse 事件）→ 多帧后截图 → golden diff
- CI matrix：`{check, test, lint, qemu-smoke, gfx-smoke, gfx-golden}` 全绿才能合 PR

### I.4.6 性能回归

- `host-tests/gos-gfx-bench`：lavapipe + 固定场景，记录 frame time p50/p99 / mutation→frame 延迟
- CI 上 ±15% 阈值 fail（lavapipe 抖动较大，硬件 GPU 路径走另一个 nightly job）
- 历史曲线存 `test-frames/perf-history.jsonl`，可视化 dashboard 后续

### I.4.7 Fuzz — 新 `host-tests/gos-gfx-fuzz`

- `cargo fuzz` target：随机 byte 流喂 `gos-gfx-protocol` 解码器 → 不 panic
- 随机 `RenderCommand` 序列喂 `k-vk-host` stub backend → 不 panic、错误码合理
- CI nightly 跑 30 分钟

### I.4.8 Manual QA checklist — `doc/PHASE_I_QA.md`

- 8 项功能验收（I.3 表格）每个对应一个手动复现脚本
- release 前必须人工签字 + 录屏存档

---

## I.5 里程碑分解

| 切片 | 范围 | 验收 |
|---|---|---|
| **H.1.x 全套** | Cypher 写真接通 runtime + envelope + harness 回归 | `cargo xtask test` 全绿，含新 4 项 mutation harness |
| **I.0** | crates 骨架 + protocol ABI + harness 框架 | `cargo xtask test` 含 gfx-harness 空 case 全绿 |
| **I.1.0** | bridge transport + 单 `BeginFrame/EndFrame` round-trip | host bridge 收到 magic + 串口 trace |
| **I.1.1** | UploadBuffer + DrawInstanced；画一个三角形 | gfx-golden：单三角通过 |
| **I.1.2** | 配额执行 + handle 回收 | gfx-harness 配额测试 + fuzz 不 panic |
| **I.2.0** | `k-scene` 接 envelope；画 100 个节点 | gfx-golden：100 instance grid |
| **I.2.1** | 边渲染 (bezier line strip) | gfx-golden：节点+边二部图 |
| **I.2.2** | `k-camera` + mouse / WASD 集成 | gfx-interact：scripted orbit → golden |
| **I.2.3** | 主题切换 + Text2D HUD + shell 叠层 | gfx-golden：theme.wabi vs theme.shoji 两 baseline |
| **I.3.0** | 5000 节点性能达标 | bench p99 < 16ms (lavapipe 阈值会比硬件宽松，硬件 nightly job 单独 gate) |
| **I.3.1** | device lost 恢复 + 截图 + 8 项 manual QA | 全表过 + 录屏存档 |
| **Gen-1 release** | F.5 并入 + 文档完整 + version bump 0.4.0 | release notes + 安装镜像 + demo 视频 |

每个切片 = 一个 PR，必须带对应 harness/golden 回归才能合并。**没有 harness 的 PR 不审。**

---

## I.6 显式 Non-Goals (Gen-1)

- ❌ 裸机 Vulkan / virtio-gpu / 真 GPU driver
- ❌ 远程 client 渲染（独立 Phase J 跟踪，与 H.3 cluster transport 共享 wire format）
- ❌ Ray tracing / mesh shader / compute pipeline（compute 留 `k-cuda-host`）
- ❌ 多 window / 多 surface
- ❌ Audio
- ❌ HiDPI / 多显示器
- ❌ 中文/复杂文本渲染（Text2D Gen-1 只支持 ASCII glyph atlas）
- ❌ VR / 立体显示
- ❌ Vulkan 资源跨 process 共享（host bridge 单进程）

---

## I.7 依赖与排序图

```
H.1.x (Cypher 写接通 runtime)
  │
  ├──> I.0 (crates 骨架) ──> I.1 (transport + 三角形) ──> I.2 (场景图) ──> I.3 (产品验收) ──> Gen-1
  │
  └──> I.4 (test infra, 与 I.0~I.3 并行；每个 slice 必带 harness)

D.1/D.2 (xtask qemu / CI) ── 必须先于 I.4.5 gfx-smoke

F.5 (FAT32 write + journal fsync) ── 并行；Gen-1 release 前并入（I.3.1 截图依赖）
```

---

## I.8 后续 phase（不在 Gen-1）

- **Phase I.2** — virtio-gpu + Venus：guest 标准 paravirt，依赖 `k-pci` + 安全 DMA
- **Phase I.3** — 真 bare-metal driver（Intel Gen11 候选）：博士论文级，开放给社区
- **Phase J** — 远程 WebGPU client：和 H.3 cluster transport 共享 wire format，"OS 在裸机、UI 在浏览器"
- **Phase I.x** — GPU 加速布局 (compute shader force-directed) / 中文文本 / 多 surface / VR

---

## I.9 风险与对策

| 风险 | 影响 | 对策 |
|---|---|---|
| lavapipe 在 CI 上不稳定 / 渲染结果与硬件差异大 | golden test 假阳/假阴 | 容差 + SSIM；硬件 nightly job 平行校准 |
| `ash` (Vulkan binding) 在 host 端是 std crate，污染依赖图 | crate 依赖混乱 | 严格隔离：只 `gos-gfx-bridge-host` 可依赖 `ash`，guest 侧零 Vulkan 依赖 |
| host bridge 跨进程延迟拖慢 mutation→frame | 验收项 #3 不达标 | 优先用共享内存 ring 而非串行 syscall；profile 阶段 I.3.0 |
| Cypher mutation 风暴打爆 envelope 队列 | UI 卡顿 | envelope 队列加 watermark + 合并 (coalescing)；超过阈值降级到 full reload |
| Gen-1 ABI 早期变动频繁导致 baseline 频繁刷 | golden 维护成本高 | I.0 锁定 ABI version 三元组；breaking change 走 major bump，baseline 整批重生成 |
