# GOS 强化日志 — V3.94

**日期**：2026-07-20
**分支**：feat/vk-auto-live-surface
**提交**：feat(v3.94): NHEPTPENTAACTC + NHHEPTPENTAACTC + NAZSO Neighborhood S-variant 指数 + gos-graph-topo83-harness（10 项测试）

## 摘要

为 GOS 图论内核新增三项 Neighborhood S-variant 拓扑指数（topo83），延伸 pentacontic 系列与 S-变体 Sombor 家族。

## 新增指数

### NHEPTPENTAACTC —— S-第57次幂顶点和
- **公式**：NHEPTPENTAACTC(G) = Σ_v S(v)^57
- **类型**：S-幂次顶点和；精确 u128→u64 饱和运算
- **系列**：pentacontic（50-59）系列第8个
- **延伸自**：NHEXPENTAACTC = Σ S^56（topo82）→ NHEPTPENTAACTC = Σ S^57（topo83）
- **S-正则图公式**：NHEPTPENTAACTC = n·S^57
- **实现**：s^57 = s32 × s16 × s8 × s（57=32+16+8+1；4 次乘法）

### NHHEPTPENTAACTC —— S-第56次幂边和
- **公式**：NHHEPTPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^56
- **类型**：S-幂次边和；精确 u128→u64 饱和运算
- **延伸自**：NHHEXPENTAACTC = Σ(S+S)^55（topo82）→ NHHEPTPENTAACTC = Σ(S+S)^56（topo83）
- **S-正则图公式**：NHHEPTPENTAACTC = 72057594037927936·|E|·S^56
- **实现**：ss^56 = ss32 × ss16 × ss8（56=32+16+8；3 次乘法 —— 效率高！）

### NAZSO —— S-变体 Sombor 指数 α=102
- **公式**：NAZSO(G) = Σ_{uv∈E} (S_u²+S_v²)^51
- **类型**：S-变体广义 Sombor 指数 SO^α，α=102；精确（无需 isqrt）
- **系列**：第3轮 "AZ" —— NA... 系列中字母表的最后一个字母
- **延伸自**：NAYSO(α=100, topo82) → NAZSO(α=102, topo83)
- **S-正则图公式**：NAZSO = 2251799813685248·|E|·S^102
- **实现**：s2s^51 = s2s32 × s2s16 × s2s2 × s2s（51=32+16+2+1；4 次乘法）

## 关键测试值（K₂，S=1 均匀）

| 指数 | K₂ 值 | 公式 |
|----------------|---------------------------|---------------|
| NHEPTPENTAACTC | 2 | 1^57+1^57 |
| NHHEPTPENTAACTC| 72_057_594_037_927_936 | 2^56 |
| NAZSO | 2_251_799_813_685_248 | 2^51 |

P₃ 未饱和：NHEPTPENTAACTC = 432_345_564_227_567_616 = 3×2^57

## 变更文件

- `crates/gos-runtime/src/lib.rs` —— `graph_topo_indices83_inner()` + 公开 API `graph_topo_indices83()`
- `crates/k-shell/src/lib.rs` —— `dispatch_graph_topo_indices83()` 显示函数
- `crates/k-shell/src/proc.rs` —— `"graph topo83"`、`"gtopo83"` 及别名路由
- `host-tests/gos-graph-topo83-harness/` —— 新建 harness（10 项测试，全部通过）

## VectorAddress 命名空间

- L4=170 分配给 gos-graph-topo83-harness
- 插件：TOPIX_83；执行器：t83.exec

## Shell 别名

- `graph topo83` / `gtopo83`
- `neighborhood heptapentacontic` / `gnheptpentaactc`
- `neighborhood hexapentacontic edge` / `gnnhheptpentaactc`
- `neighborhood dohectyl sombor` / `gnnazso`
- `gnheptpentaactcnhheptpentaactcnazso`

## 测试结果

- **宿主测试套件**：共 1913 项（此前 1903 项 + 本次新增 10 项）
- **新增 harness**：gos-graph-topo83-harness —— 10/10 通过
- **运行时检查**：`cargo check -p gos-runtime` —— 通过，无警告

## 备注

- NHHEPTPENTAACTC 的 ss^56 实现尤为高效：56=32+16+8（三个2的幂次，仅需3次组合乘法）
- NAZSO 标志着第3轮单字母 "NA..." Sombor 系列的终结（A 至 Z 已全部用尽）
- 下一步：topo84 将开启 NBASO 系列，或以 NOCTPENTAACTC（Σ S^58）延伸 pentacontic 系列
