# GOS 绳索物理化开发计划 — 宇宙环境真实绳索 + 物料解算引擎

> 状态：提案 · 日期：2026-07-08 · 配套：[V2 开发计划](V2_DEVELOPMENT_PLAN.md) · [优化计划](OPTIMIZATION_PLAN.md)
>
> 本计划把 fbtest.rs 中"名为 rope 实为直线"的图边渲染（[fbtest.rs:1703-1714](../crates/hypervisor/src/fbtest.rs)，两段 `draw_seg` A→中点→B）升级为**真实物理模拟的宇宙绳索**：Verlet 积分 + XPBD 约束求解 + 物料参数化解算。图的边不再是几何示意，而是有质量、有刚度、有阻尼、会摆动的物理对象。
>
> **产品级铁律（继承自 V2 计划）**：每个 phase 必须随附 harness 一同合入。**无 harness = 不合入**。物理核心必须与渲染解耦成独立 no_std crate，在 host 上确定性可测。

## 〇、现状盘点（代码事实，非假设）

| 事实 | 位置 | 对本计划的约束 |
|---|---|---|
| "rope" = 两段直线段，无任何动力学 | `fbtest.rs:1703-1714` | 这是被替换的对象 |
| 节点 3D 布局静态，init 时算好并归一化到 FIT_R=3.8 | `fbtest.rs:540-554` | 绳索锚点 = 节点球面 |
| MAXN=128 节点，MAXE=512 边 | `fbtest.rs:64-65` | 物理数组静态上限 |
| f32 + `sqrtf` 可用（软件渲染已全程 f32） | 全文件 | 无需定点数，直接 f32 |
| 1920×1080 软光栅 ~8-13 FPS，PERF 分段计时已存在 | `fbtest.rs:53-59, 1759-1772` | 物理预算必须挤进现有帧预算；用同一 PERF 机制举证 |
| **render_frame 绝不锁 RUNTIME**（项目不变式） | `fbtest.rs:27` + skill `gos-fbtest-render-lockfree-invariant` | 物理状态只能存 Desktop/独立 static，拓扑变化走 epoch 缓存刷新 |
| `_rdtsc` 已用于计时，`wrapping_sub` 防 WHPX 跳变 | `fbtest.rs:1760` | dt 测量沿用同一模式 |
| 场景 = 深空背景 (0x0a0a12) + 悬浮金属球 | `fbtest.rs:63` | **宇宙零重力是场景本义**，不是附加设定 |
| 边拾取 = 最近绳段阈值测试 | `fbtest.rs:1200-1201` | 拾取需升级为折线段测试 |
| host-tests 文化：89+ harness，`gos-harness-new-creation` 模板 | `host-tests/` | 物理核心照此模式测试 |

## 一、物理模型选型（决策已做，理由如下）

### 1.1 求解器：Verlet 积分 + XPBD 约束（非力学弹簧）

**选 XPBD（Extended Position-Based Dynamics），不选显式弹簧-质点力学**：

- **无条件稳定**：位置级投影不会像刚性弹簧那样在低帧率（本机 8-13 FPS）下爆炸。帧率波动只影响收敛质量，不影响稳定性。
- **刚度物理正确**：XPBD 的 compliance（柔度 α）是物理量纲参数，不随迭代次数/时间步漂移——这正是"物料解算"要求的：同一物料在任何帧率下表现一致。
- **零分配、定长数组友好**：整个求解器是对 `[f32; N]` 的纯函数迭代，完美契合 no_std 内核环境与本仓库治理（无堆、`SyncUnsafe` 单写者模式）。

**积分格式**（每子步 h）：

```text
v_implicit = (x - x_prev) * damp        // 阻尼吸收（宇宙近无损：damp ≈ 0.996-0.999）
x_prev     = x
x          = x + v_implicit + a_ext·h²  // 零重力：a_ext=0，仅交互冲量/漂移注入
∀ 约束 c:  XPBD 投影  Δλ = (−C − α̃λ) / (Σwᵢ|∇C|² + α̃),  α̃ = α/h²
```

### 1.2 宇宙环境模型（"处于宇宙中"的物理含义）

| 现象 | 模型 | 参数 |
|---|---|---|
| 零重力 | `a_ext = 0`，绳索不下垂成悬链线，形状由动量历史决定 | — |
| 真空无空气阻力 | 阻尼仅来自绳索内耗（材料阻尼），极低 | damp ∈ [0.996, 0.999] |
| 惯性主导 | 节点移动（布局变化/拖拽）注入动量后绳索长时间摆动、缓慢衰减 | 由 damp 决定衰减时标 |
| 绳索松弛悬浮 | rest 长度 = κ ×端点直线距离（κ=1.15~1.35 松弛系数），多余长度让绳在零重力中呈自由弧形漂浮 | κ 是物料参数 |
| 宇宙微扰（可选，防画面死寂） | 每粒子极小 xorshift 伪随机漂移冲量（遵循 skill `gos-xorshift-seed-zero`：种子非零） | 幅度 ≤ 1e-4 世界单位/帧 |

**关键视觉收益**：图拓扑变化（`CALL create_edge` / kill node / 布局重排）时，绳索**滞后、甩动、缓慢稳定**——图的演化第一次有了"质感"。

### 1.3 离散化与预算

- 每绳 **K=9 粒子（8 段）**；512 绳 × 9 = 4608 粒子。
- 状态数组（pos ×3 + prev ×3 + invmass ×1，f32）≈ **129 KB 静态内存**——与现有 FB(8MB)/ZB(8MB) 相比可忽略。
- 每帧：4 子步 × 1 轮约束迭代（XPBD 论文结论：**子步优于迭代**）。每子步约束数 ≈ 512×(8 拉伸 + 7 弯曲 + 2 锚点) ≈ 8700 个标量投影 → 现代 CPU **< 0.5 ms**，远低于光栅化成本（rope 绘制现测已是 PERF 单独段）。
- dt 来源：`_rdtsc` 帧间差（`wrapping_sub`），钳制到 [8 ms, 125 ms]，子步 h = dt/4。**测试模式走固定 dt = 1/60 保证确定性**。

## 二、物料解算系统（Material Solver）

### 2.1 物料参数表

每种物料（`RopeMaterial`）是一组物理参数，**决定同一求解器下完全不同的行为**：

```rust
pub struct RopeMaterial {
    pub linear_density:   f32, // kg/m → 粒子质量 = ρ·L0/K（质量影响碰撞/冲量响应）
    pub stretch_alpha:    f32, // 拉伸柔度 α（0=完全刚性钢缆，大=橡皮筋）
    pub bend_alpha:       f32, // 弯曲柔度（小=硬管/电缆，大=软绳/锁链）
    pub damping:          f32, // 材料内耗 ∈ [0.99, 0.9999]
    pub slack_kappa:      f32, // 松弛系数 κ（rest 总长 / 端点距离）
    pub max_strain:       f32, // 应变上限（strain limiting 硬钳制，如 0.05 = 5%）
    pub radius:           f32, // 视觉/碰撞半径（映射 ROPE_RAD）
}
```

### 2.2 GOS 原生映射：边语义 = 物理物料

这是本计划最 GOS 的一笔：**图边的类型决定绳索物料**——语义直接物化。

| RuntimeEdgeType（示例） | 物料直觉 | 参数倾向 |
|---|---|---|
| Grant / Bind 类（强约束边） | 钢缆：短、紧、几乎不摆 | κ=1.05, stretch_alpha≈0, bend 硬 |
| Signal / Send 类（通信边） | 软数据线：明显松弛、活跃摆动 | κ=1.3, bend 软, damp 低 |
| Refer 类（弱引用边） | 细丝：极软、大幅漂浮 | κ=1.35, 全软, radius 细 |

映射表放 `k-rope` crate 内（默认表）+ 允许 fbtest 覆盖。**边一眼看出类型**——不靠颜色靠动态。

### 2.3 约束集（物料解算的"解算"部分）

按每子步执行顺序：

1. **拉伸约束**（相邻粒子距离 = L0/K）：XPBD，compliance = `stretch_alpha`。
2. **弯曲约束**（隔一粒子 i↔i+2 距离 = 2·L0/K·cos(θ/2) 近似）：XPBD，compliance = `bend_alpha`。廉价弯曲，避免真二面角。
3. **锚点约束**：端粒子 invmass=0，位置 = 节点中心 + 指向对端方向 × NODE_R（**球面附着**，不是球心，绳从球面长出）。
4. **应变限制**（post-pass 硬钳制）：每段长度 clamp 到 L0/K × (1+max_strain)，防高速冲量下过拉。
5. **节点球碰撞**（Phase R4）：粒子推出所有节点球（128 球 × 4608 粒子暴力 = 59 万次距离测试/子步 → 需网格加速或仅测端点邻域球）。

## 三、阶段总览

| 阶段 | 主题 | 核心交付 | Killer Demo | 退出判据 |
|---|---|---|---|---|
| **R0** | 物理核心 crate | `crates/k-rope`：Verlet + XPBD 拉伸/锚点约束，纯 no_std 零分配；`gos-rope-harness`（≥10 测试） | host 上跑 1000 步：松弛绳在零重力中保持总长不变、动量守恒、无 NaN | harness 全绿；内核零改动（`cargo check` 干净） |
| **R1** | 物料解算 | `RopeMaterial` 表 + 弯曲约束 + 应变限制 + 边类型→物料映射；`gos-rope-material-harness` | 同一扰动下钢缆/软绳/细丝三种物料行为可测量地不同（弯曲能量排序断言） | 物料参数确定性测试绿；刚度序数性质（α 小 ⇒ 残余应变小）property test 绿 |
| **R2** | fbtest 集成 | Desktop 增加绳索状态数组；epoch 感知重建（保留幸存边的粒子状态）；渲染折线化（K-1 段 `draw_seg`，双色调保留）；PERF 新增 `phys=` 段 | QEMU 内：k-shell 建一条新边 → 绳索以卷曲态生成、零重力中缓缓展开抖动稳定 | phys 段 < 2ms/帧（PERF 串口日志举证）；render_frame 仍零 RUNTIME 锁（治理 grep）；`test-frames/` 留金帧 |
| **R3** | 交互与应力可视化 | 折线拾取；点击"拨弦"冲量；张力→颜色热力（应力可视化）；popup 显示物料名+当前应变 | 鼠标点绳 → 绳被拨动横波传播到两端反射回来 | 拾取命中折线任意段；拨弦后 N 帧内可测振荡衰减；popup 物料面板截图 |
| **R4** | 高级物理（可选） | 节点球碰撞（推出）；LOD（屏占小 → 段数减半）；sleeping（静止绳跳过解算）；断裂（应变 > break_strain → 视觉绷断，联动 remove_edge 事件） | kill 节点 → 该节点所有绳索**释放漂走淡出**，而非瞬间消失 | 碰撞后无穿透（harness 几何断言）；sleeping 使静止场景 phys 段 < 0.2ms |

依赖链：**R0 → R1 → R2 → R3 → R4**。R0/R1 纯 host 侧，不碰内核，可与其他轨并行。

## 四、各阶段详述

### Phase R0 — 物理核心 crate（约 1 周）

**目标**：可证明正确的最小求解器，先于任何像素。

**交付**：
- `crates/k-rope`：`#![no_std]`，零分配。核心 API（纯函数，caller 提供数组切片——与 gos-runtime 风格一致）：
  ```rust
  pub struct RopeState { /* pos/prev 各 [f32; MAX_PARTICLES×3]，invmass，per-rope 元数据 */ }
  pub fn rope_seed(state, rope_id, a: [f32;3], b: [f32;3], material, seed_shape);
  pub fn rope_step(state, h: f32, materials: &[RopeMaterial]);  // 1 子步：积分+全约束
  pub fn rope_anchor(state, rope_id, end: u8, pos: [f32;3]);    // 每帧更新锚点
  ```
- `host-tests/gos-rope-harness`（按 `gos-harness-new-creation`：Cargo.toml `[workspace]` + `.cargo/config.toml` MSVC override）。**≥10 测试**：
  1. 确定性：同 seed 同步数 → 位相同结果
  2. rest 态不变：已静止的绳 step 1000 次位置漂移 < 1e-5
  3. 拉伸收敛：拉长 2× 释放 → N 步内回到 L0±ε
  4. 总长守恒：任意扰动下 Σ段长 ∈ [L0, L0×(1+max_strain)]
  5. 零重力动量守恒：无锚自由绳质心速度恒定（阻尼=1 时）
  6. 阻尼单调：动能序列单调不增（damp<1）
  7. 锚点精确：invmass=0 端永不移动
  8. 应变限制：暴力冲量后无段超 (1+max_strain)
  9. NaN 模糊测试：xorshift 随机冲量 10⁴ 步无 NaN/Inf（种子非零）
  10. 双绳独立：绳 A 扰动不影响绳 B（内存越界哨兵）

**退出判据**：harness 全绿；内核构建零变化。

**风险**：f32 累积误差 → 测试用 ε 容差 + 定期重归一化 rest 长（不做定点数——现有渲染管线已全 f32）。

### Phase R1 — 物料解算（约 1 周）

**目标**：同一求解器，参数化出可测量不同的材料行为。

**交付**：
- `RopeMaterial` + 内置物料表（钢缆/软绳/细丝 3 档起步）+ `RuntimeEdgeType → 物料` 默认映射。
- 弯曲约束（i↔i+2 XPBD）+ 应变限制 post-pass。
- `gos-rope-material-harness`：刚度序数性（stretch_alpha 小 ⇒ 稳态残余应变小，property test 3 物料排序）、弯曲能量排序（bend_alpha 小 ⇒ 弯曲后回弹快）、κ 语义（seed 后总 rest 长 = κ×端距 ± ε）、物料确定性。

**退出判据**：harness 全绿；三物料在标准扰动脚本下行为指标（稳态应变/振荡周期/衰减时标）可区分且可复现。

**风险**：弯曲+拉伸约束互相打架致抖动 → 遵循"子步优先"（4 子步×1 迭代），弯曲 compliance 下限保护。

### Phase R2 — fbtest 集成（约 1-2 周）

**目标**：像素落地，且不破坏两条铁律（lock-free、帧预算）。

**交付**：
- Desktop（或独立 `SyncUnsafe<RopePhys>` static，遵循治理：不用 `static mut`）新增绳索状态；init() 内 `without_interrupts` 读图 seed——数据流与现有 `px/py/pz` 缓存完全同构。
- **epoch 感知重建**：帧首查 `graph_epoch`（原子读，无锁），变化时 diff 边表——幸存边 (a,b) 保留粒子状态（动量连续），新边 seed 卷曲初始形态，死边移除。遵循 skill `gos-diff-ring-epoch-invariant` 对 epoch 语义的界定。
- 渲染：`draw_seg` ×2 → ×(K-1) 折线；双色调按"距哪端粒子数"分半，保留 `PAL_CONTRAST` 逻辑。
- PERF 行新增 `phys={}us` 段。
- dt：`_rdtsc` 差 + 钳制；标定沿用现有"除以 3000 ≈ 3GHz µs"约定。

**退出判据**：
- PERF 串口日志证明 phys < 2ms/帧（60 帧采样）。
- 治理检查：render_frame 路径无 RUNTIME 锁（现有 grep 红线扩展到 k-rope 调用点）。
- `test-frames/` 存金帧：新边生成后第 1/30/120 帧三张，人工确认展开动画。
- rope 绘制段耗时增幅 < 4×（段数 2→8，但 draw_seg 内部 steps 与像素长度成正比，总像素数近似不变，理论增幅仅来自调用开销）。

**风险**：8-13 FPS 下 dt 大（~77-125ms）→ 子步 h 仍 ≤ 31ms，XPBD 稳定但动画偏"快进"——用 dt 钳上限 100ms 换取慢镜头感（宇宙场景反而合理）。

### Phase R3 — 交互与应力可视化（约 1 周）

**交付**：
- `do_pick` 边测试升级为对 K-1 段折线逐段距离测试（阈值逻辑不变，`fbtest.rs:1200` 模式外推）。
- 左键点中绳身 → 对命中粒子注入垂直于绳向的冲量（"拨弦"）。
- 张力可视化：每段应变 → 颜色向红端插值（复用 PAL 思路，应力热力图）。
- 边 popup 增加：物料名、当前最大应变、振荡状态。遵循 skill `gos-combined-view-dedup-guard` 检查面板去重。

**退出判据**：拨弦后横波可见（金帧序列）；popup 数据与 harness 侧同参数计算一致（数值 pin 测试，参照 skill `gos-ppm-assertion-pin-from-runtime` 方法论）。

### Phase R4 — 高级物理（可选，按需启动）

节点球碰撞（仅测绳段 AABB 相交的球，避免 59 万暴力测试）、LOD、sleeping（每绳动能 < ε 连续 N 帧 → 冻结，任何锚点移动/冲量唤醒）、断裂视觉联动 `remove_edge`。每项独立 harness。

## 五、内存与性能预算总表

| 项 | 预算 | 依据 |
|---|---|---|
| 粒子状态静态内存 | ≤ 160 KB | 4608 粒子 × (pos+prev+invmass+per-rope 元数据) |
| 物理解算 CPU | < 2 ms/帧（目标 <0.5ms） | 4 子步 × ~8700 标量投影 |
| 渲染增量 | rope 段 PERF 增幅 < 4× | 总覆盖像素不变，仅折线调用数 ×4 |
| 帧率影响 | 净 FPS 下降 < 10% | 物理 ≪ 光栅成本（现瓶颈是 fill/blit） |

## 六、治理与测试红线（本计划新增）

1. **无 harness = 不合入**（继承铁律）：R0-R4 每阶段先 harness 后集成。
2. **render_frame 零锁**：k-rope 调用点纳入现有治理 grep；物理只消费 epoch 缓存拓扑。
3. **确定性优先**：k-rope 一切随机性走显式传入的 xorshift 状态（种子非零），host 测试可复现。
4. **物理正确性以不变量测试表达**（动量/能量/长度守恒），不以"看起来对"验收——金帧只作视觉回归兜底。
5. **新 harness 遵循** `gos-harness-new-creation` / `gos-harness-push-location` / `gos-windows-lnk1104-retry` 既有 skill。

## 七、风险登记

| 风险 | 概率 | 缓解 |
|---|---|---|
| 低帧率下大 dt 使动画不自然 | 中 | dt 钳制 + 子步；宇宙慢镜头美学兜底 |
| f32 累积漂移致绳缓慢"缩水/生长" | 中 | rest 长度为常量真值，每步重投影；harness #4 守恒测试 |
| epoch 重建丢动量致视觉跳变 | 中 | (a,b) 键控状态保留；harness 覆盖重建路径 |
| 弯曲约束抖动 | 低 | 子步优先策略；compliance 下限 |
| WHPX 下 TSC 跳变致 dt 异常 | 低 | 沿用 `wrapping_sub` + 钳制（已有先例） |
| 512 绳满载时预算超支 | 低 | sleeping + LOD 预留在 R4；R2 退出判据用满载压测 |

## 八、与 V2/V3 主线的关系

- 本计划位于 **V2.3（渲染统一）→ V2.5（Vulkan Phase I）** 之间的表现层轨道，不阻塞主线，不修改 gos-runtime/gos-protocol 语义（R1 的边类型映射是只读消费）。
- 若 V2.3 完成 `fbtest.rs → k-render + k-desktop` 拆分，k-rope 作为纯解算 crate **无需改动**——这正是 R0 把物理与渲染解耦的原因。
- 长期（V3 前沿轨）：绳索物理是"图即世界"的第一个物理化实验——后续可推广到节点质量（度数=质量）、力导向布局与物理统一（布局器退役，物理即布局）。

---

**执行入口**：从 R0 开始——`cargo new crates/k-rope --lib`，参照 `gos-harness-new-creation` 建 `host-tests/gos-rope-harness`，首个测试是确定性测试（#1）。
