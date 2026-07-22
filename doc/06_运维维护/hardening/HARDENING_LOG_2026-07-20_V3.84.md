# 强化日志 — V3.84（2026-07-20）

## 摘要

**feat(v3.84): NHEPTETRAACTC + NHHEPTETRAACTC + NAPSO Neighborhood S-variant 指数 + gos-graph-topo73-harness（10 项测试）**

提交：`be5c904`
分支：`feat/vk-auto-live-surface`
宿主测试套件：**1813 项**（此前 1803 项 + 本次新增 10 项）

---

## 新增拓扑指数 —— topo73

### `gos_runtime::graph_topo_indices73()`
返回 `(nheptetraactc, nhheptetraactc, napso, edge_count, node_count)`

### NHEPTETRAACTC —— S-第47次幂顶点和
- **公式**：NHEPTETRAACTC(G) = Σ_v S(v)^47
- **S(v)**：Σ_{w∈N(v)} deg(w)（邻域度数和，与 topo18/topo21–topo73 系列一致）
- **延伸自**：NHEXTETRAACTC=Σ S^46（topo72）→ NHEPTETRAACTC=Σ S^47（topo73）
- **S-正则图公式**：NHEPTETRAACTC = n·S^47
- **实现**：s^47 = s32 × s8 × s4 × s2 × s（47=32+8+4+2+1；5 次乘法）
- **溢出处理**：饱和 u128 累加器 → 截断至 u64::MAX

### NHHEPTETRAACTC —— S-第46次幂边和
- **公式**：NHHEPTETRAACTC(G) = Σ_{uv∈E} (S_u + S_v)^46
- **延伸自**：NHHEXTETRAACTC=Σ(S+S)^45（topo72）→ NHHEPTETRAACTC=Σ(S+S)^46（topo73）
- **S-正则图公式**：NHHEPTETRAACTC = 70_368_744_177_664 · |E| · S^46
- **实现**：ss^46 = ss32 × ss8 × ss4 × ss2（46=32+8+4+2；4 次乘法 —— 效率很高，4 个 2 的幂次之和！）
- **说明**：ss^46 十分高效：46 可分解为 4 个 2 的幂次（32+8+4+2），仅需 4 次乘法

### NAPSO —— S-第82次 Sombor 变体（α=82）
- **公式**：NAPSO(G) = Σ_{uv∈E} (S_u² + S_v²)^41
- **系列**：第3轮双字母 "AP" —— NAOSO(α=80,topo72) → NAPSO(α=82,topo73)
- **S-正则图公式**：NAPSO = 2_199_023_255_552 · |E| · S^82
- **实现**：s2s^41 = s2s32 × s2s8 × s2s（41=32+8+1；3 次乘法）

---

## 测试数据

| 图 | NHEPTETRAACTC | NHHEPTETRAACTC | NAPSO | 边数 | 节点数 |
|-----------|---------------------------|-----------------------------|---------------------|-------|-------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单节点 | 0 | 0 | 0 | 0 | 1 |
| K₂ | 2 | 70_368_744_177_664 | 2_199_023_255_552 | 1 | 2 |
| P₃ | 422_212_465_065_984 | u64::MAX（饱和）| u64::MAX（饱和）| 2 | 3 |
| K₃ | u64::MAX（饱和）| u64::MAX（饱和）| u64::MAX（饱和）| 3 | 3 |
| K_{1,4} | u64::MAX（饱和）| u64::MAX（饱和）| u64::MAX（饱和）| 4 | 5 |
| P₄ | u64::MAX（饱和）| u64::MAX（饱和）| u64::MAX（饱和）| 3 | 4 |
| K₄ | u64::MAX（饱和）| u64::MAX（饱和）| u64::MAX（饱和）| 6 | 4 |
| 两孤立节点 | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | u64::MAX（饱和）| u64::MAX（饱和）| u64::MAX（饱和）| 6 | 5 |

**K₂ 关键值**：
- 2^46 = 70_368_744_177_664（NHHEPTETRAACTC 系数）
- 2^41 = 2_199_023_255_552（NAPSO 系数）
- 3 × 2^47 = 3 × 140_737_488_355_328 = 422_212_465_065_984（P₃ 的 NHEPTETRAACTC）

---

## 变更文件

| 文件 | 变更 |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices73_inner()`（内部实现，约110行）+ `graph_topo_indices73()` 公开封装 |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices73()`，含彩色终端输出 |
| `crates/k-shell/src/proc.rs` | 新增路由："graph topo73"/"gtopo73"/"gnheptetraactc"/"gnhheptetraactc"/"gnnapso"/"gnheptetraactcnhheptetraactcnapso" |
| `host-tests/gos-graph-topo73-harness/` | 新建 harness（Cargo.toml、.cargo/config.toml、tests/graph_topo73.rs） |

---

## Shell 命令

```
graph topo73
gtopo73
neighborhood heptatetracontic
gnheptetraactc
neighborhood hexatetracontic edge
gnhheptetraactc
neighborhood docosacontyl sombor
gnnapso
gnheptetraactcnhheptetraactcnapso
```

---

## VectorAddress 命名空间

- L4=160 分配给 `gos-graph-topo73-harness`
- 插件：`TOPIX_73`；执行器：`t73.exec`
- 完整范围：88=graph-topo 至 159=graph-topo72，**160=graph-topo73**

---

## 测试结果

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

全部 10 项测试首次运行即通过，无需算术修正。
