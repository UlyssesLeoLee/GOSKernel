# 强化日志 — V3.95（2026-07-20）

## 摘要

新增三项 Neighborhood S-variant 拓扑指数（topo84）及配套的 10 项测试 harness。

## 变更内容

### crates/gos-runtime/src/lib.rs
- 新增 `graph_topo_indices84_inner()` —— 计算 NOCTOPENTAACTC + NHOCTOPENTAACTC + NBASO
- 新增 `pub fn graph_topo_indices84()` 公开 API 封装

### crates/k-shell/src/lib.rs
- 新增 `dispatch_graph_topo_indices84()` —— 三项指数的彩色终端输出

### crates/k-shell/src/proc.rs
- 新增 topo84 命令路由：
  - `"graph topo84"`、`"gtopo84"`
  - `"neighborhood octopentacontic"`、`"gnoctopentaactc"`
  - `"neighborhood heptapentacontic edge"`、`"gnnhoctopentaactc"`
  - `"neighborhood tetrahectyl sombor"`、`"gnnbaso"`
  - `"gnoctopentaactcnhoctopentaactcnbaso"`

### host-tests/gos-graph-topo84-harness/（新增）
- 10 项测试：空图、单节点、K₂、P₃、K₃、K_{1,4}、P₄、K₄、两孤立节点、K_{2,3}
- 全部 10 项通过（已验证）

## 已实现指数

### NOCTOPENTAACTC(G) = Σ_v S(v)^58
- S-第58次幂顶点和（pentacontic 50–59 系列第9个）
- 延伸自 NHEPTPENTAACTC=Σ S^57（topo83）
- K₂：2；P₃：864_691_128_455_135_232；更大的图均饱和
- 实现：s^58 = s32×s16×s8×s2（58=32+16+8+2；4 次乘法）

### NHOCTOPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^57
- S-第57次幂边和
- 延伸自 NHHEPTPENTAACTC=Σ(S+S)^56（topo83）
- K₂：144_115_188_075_855_872（=2^57）；P₃ 及更大的图均饱和
- 实现：ss^57 = ss32×ss16×ss8×ss（57=32+16+8+1；4 次乘法）

### NBASO(G) = Σ_{uv∈E} (S_u²+S_v²)^52
- S-变体 Sombor 指数 SO^α，α=104（NB 系列首个；第4轮 BA）
- NAZSO(α=102,topo83) → NBASO(α=104,topo84)
- K₂：4_503_599_627_370_496（=2^52）；P₃ 及更大的图均饱和
- 实现：s2s^52 = s2s32×s2s16×s2s4（52=32+16+4；3 次乘法 —— 效率高！）

## VectorAddress
- L4=171 分配给 gos-graph-topo84-harness
- 插件：TOPIX_84，执行器：t84.exec

## 测试结果
- 10/10 项测试通过
- 宿主测试套件总数：**1923 项**（此前 1913 项 + 本次新增 10 项）

## Commit
9a5f892 feat(v3.95): NOCTOPENTAACTC + NHOCTOPENTAACTC + NBASO Neighborhood S-variant indices + gos-graph-topo84-harness (10 tests)
