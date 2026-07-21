# GOSKernel 强化日志 V3.40 — NZ₀ + NEM₂ + NSe S-variant 拓扑指数 + 修复 topo28 k-shell 缺口

**日期：** 2026-07-16
**分支：** feat/vk-auto-live-surface
**提交：**（见 git log）
**宿主测试总计：** 1373（此前 1363 + 新增 10）

---

## 摘要

本次改动包含两部分：

1. **修复（V3.39 遗留缺口）**：`dispatch_graph_topo_indices28` 及 topo28 的 k-shell 路由在 V3.39 提交中缺失。现已补齐（`crates/k-shell/src/lib.rs` + `crates/k-shell/src/proc.rs`）。

2. **新功能（V3.40）**：三个新的 S-variant 拓扑指数 —— NZ₀、NEM₂、NSe，在 `gos_runtime` 中暴露为 `graph_topo_indices29()`，在 k-shell 中对应 `dispatch_graph_topo_indices29`，并由 `gos-graph-topo29-harness` 验证（10 项测试，全部通过）。

---

## 新功能：`graph topo29` — NZ₀ + NEM₂ + NSe

### 定义

其中 S(v) = Σ_{w∈N(v)} deg(w)（邻居度数和，属于 Mondal et al. 2019 定义的 "S-variant" 系列）：

| 指数 | 公式 | 类型 | 参考文献 |
|-------|---------|------|-----------|
| **NZ₀** | Σ_{v: S(v)>0} 1/√S(v) | ppm（向下取整） | 零阶 Randić 指数 χ₀ 的 S-模拟量（Randić 1975） |
| **NEM₂** | Σ_{uv∈E} S_u·S_v·(S_u+S_v−2) | 精确 u64 | 重构第二 Zagreb 指数 EM₂ 的 S-模拟量（Miličević et al. 2004） |
| **NSe** | Σ_v √S(v) | ppm（向下取整） | S-平方根顶点和（topo22 中 NF=Σ_v S³ 的对偶指标） |

### 实现

- **算法**：O(V+E) — 度数遍历 → S(v) 计算 → 节点扫描（NZ₀、NSe） → 边扫描（NEM₂）
- **无需 BFS**（与 topo18–topo28 属于同一 O(V+E) 复杂度类）
- **溢出安全性**：
  - NZ₀：`isqrt64(10^12/S(v))` — 最大入参 10^12 < u64::MAX ✓；孤立节点（S=0）跳过
  - NEM₂：每边最大约 8.39×10^12；总和 ≤ 6.82×10^16 < u64::MAX ✓
  - NSe：`isqrt64(S(v)×10^12)` — 最大入参 16129×10^12 = 1.61×10^16 < u64::MAX ✓
- **返回值**：`(nz0_ppm: u64, nem2: u64, nse_ppm: u64, edge_count: usize, node_count: usize)`

### Shell 命令

```
graph topo29 / gtopo29
neighborhood zero randic / gnz0
neighborhood em2 / gnem2
neighborhood sqrt vertex / gnse
gnz0nem2nse
```

### 关键不变量

- 对 S-正则图：NZ₀ = n × isqrt64(10^12/S)
- 当且仅当所有边满足 S_u+S_v=2 时 NEM₂ = 0（仅 K₂ 类型；标注 "NEM2=0: all S=1 edges"）
- 对 S-正则图：NEM₂ = |E|·S²·(2S−2)
- 对 S-正则图：NSe = n × isqrt64(S×10^12)
- K₃ 与 K_{1,4}：均为 S-均匀 S=4 → 每顶点 NZ₀、NSe 相同；NEM₂ 按边数比例不同
- K₄ 与 K_{2,3}：两者的 S 均为正则（分别为 9 与 6）→ NEM₂ 精确公式均适用

### 交叉验证表

| 图 | NZ₀ (ppm) | NEM₂ | NSe (ppm) | 边数 | 点数 |
|-------|-----------|------|-----------|-------|-------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| K₂ | 2_000_000 | 0 | 2_000_000 | 1 | 2 |
| P₃ | 2_121_318 | 16 | 4_242_639 | 2 | 3 |
| K₃ | 1_500_000 | 288 | 6_000_000 | 3 | 3 |
| K_{1,4} | 2_500_000 | 384 | 10_000_000 | 4 | 5 |
| P₄ | 2_568_912 | 72 | 6_292_526 | 3 | 4 |
| K₄ | 1_333_332 | 7_776 | 12_000_000 | 6 | 4 |
| K_{2,3} | 2_041_240 | 2_160 | 12_247_445 | 6 | 5 |

### 操作系统类比

- **NZ₀** = 平方根倒数邻域负载（高值表示存在大量低负载路由节点；K₂=星中星型时取最大）
- **NEM₂** = S-重构第二 Zagreb 压力（K₂ 型边为 0；对高 S 边按 sum−2 平方级放大）
- **NSe** = 平方根邻域负载和（与三次方 NF 互补；相对 S 呈中等增长）

---

## 修复：topo28 k-shell 缺口（V3.39 追溯修复）

V3.39 新增了 `gos_runtime::graph_topo_indices28()` 与 `gos-graph-topo28-harness`，但遗漏了 k-shell 显示分发与 proc.rs 路由。本次提交补齐：

- `crates/k-shell/src/lib.rs`：`dispatch_graph_topo_indices28()` —— NNI/NNMI/NSM1 显示，亮黄色标题，NNI 亮青色（ppm），NNMI 亮绿色（ppm），NSM1 亮品红（精确值）
- `crates/k-shell/src/proc.rs`：新增 `graph topo28 / gtopo28 / neighborhood nirmala / gnni / neighborhood modified nirmala / gnnmi / gnsm1 / gnnigsm1 / gnninnminsm1` 路由

---

## VectorAddress L4 命名空间（更新后）

……, 114=graph-topo27, 115=graph-topo28, **116=graph-topo29**

---

## 变更文件

- `crates/gos-runtime/src/lib.rs` — `graph_topo_indices29_inner` + `graph_topo_indices29()`
- `crates/k-shell/src/lib.rs` — `dispatch_graph_topo_indices28`（修复）+ `dispatch_graph_topo_indices29`（新增）
- `crates/k-shell/src/proc.rs` — topo28 路由（修复）+ topo29 路由（新增）
- `host-tests/gos-graph-topo29-harness/` — 新建 harness（10 项测试，全部通过）
- `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.40.md` — 本篇日志
