# HARDENING LOG — V3.79（2026-07-20）

## 概述

V3.79 为图论操作系统内核 runtime 新增三项 Neighborhood S-variant 拓扑指数（topo68），补齐此前遗漏的 topo66、topo67 的 k-shell 派发函数，并交付完整的 10 项验证测试 harness。

---

## 新增指数：NDOTETRAACTC + NHDOTETRAACTC + NAKSO

### 数学定义

**S(v) = Σ_{w∈N(v)} deg(w)** — 邻域度数和（S-variant，与 topo18 系列一致，未变化）

| 指数 | 公式 | 名称 | α |
|------|------|------|---|
| NDOTETRAACTC | Σ_v S(v)^42 | S-Dotetracontic 顶点和 | — |
| NHDOTETRAACTC | Σ_{uv∈E} (S_u+S_v)^41 | S-Hentetracontic 边和 | — |
| NAKSO | Σ_{uv∈E} (S_u²+S_v²)^36 | S-Dotetracontyl Sombor | 72 |

### 系列定位

- **NDOTETRAACTC** 由 NHENTETRAACTC=ΣS^41（topo67）扩展到第 42 次幂
- **NHDOTETRAACTC** 由 NHHENTETRAACTC=Σ(S+S)^40（topo67）扩展到第 41 次幂
- **NAKSO** 为 S-variant 广义 Sombor SO^α，α=72（第三轮双字母 "AK"）：
  NAISO(α=68)→NAJSO(α=70)→NAKSO(α=72)

### 标准图解析值

| 图 | NDOTETRAACTC | NHDOTETRAACTC | NAKSO | 边数 | 点数 |
|----|---------------|-----------------|-------|------|------|
| 空图 | 0 | 0 | 0 | 0 | 0 |
| 单点 | 0 | 0 | 0 | 0 | 1 |
| K₂ | 2 | 2_199_023_255_552 | 68_719_476_736 | 1 | 2 |
| P₃ | 13_194_139_533_312 | u64::MAX（饱和） | u64::MAX（饱和） | 2 | 3 |
| K₃ | u64::MAX（饱和） | u64::MAX（饱和） | u64::MAX（饱和） | 3 | 3 |
| K_{1,4} | u64::MAX（饱和） | u64::MAX（饱和） | u64::MAX（饱和） | 4 | 5 |
| P₄ | u64::MAX（饱和） | u64::MAX（饱和） | u64::MAX（饱和） | 3 | 4 |
| K₄ | u64::MAX（饱和） | u64::MAX（饱和） | u64::MAX（饱和） | 6 | 4 |
| K_{2,3} | u64::MAX（饱和） | u64::MAX（饱和） | u64::MAX（饱和） | 6 | 5 |

**关键推导：**
- K₂（S=1）：NDOTETRAACTC=2×1^42=2；NHDOTETRAACTC=2^41=2_199_023_255_552；NAKSO=2^36=68_719_476_736
- P₃（S=2）：NDOTETRAACTC=3×2^42=13_194_139_533_312；NHDOTETRAACTC：4^41=2^82≫u64::MAX → 饱和
- P₄（S=2,3,3,2）：3^42=109_418_989_131_512_359_209>u64::MAX → NDOTETRAACTC 饱和

### S-regular 正则图公式

- NDOTETRAACTC = n·S^42
- NHDOTETRAACTC = |E|·(2S)^41 = 2_199_023_255_552·|E|·S^41
- NAKSO = |E|·(2S²)^36 = 68_719_476_736·|E|·S^72

### 幂次分解实现

| 指数 | 分解 | 乘法次数 |
|------|------|----------|
| s^42 | s32×s8×s2（42=32+8+2） | 3 |
| ss^41 | ss32×ss8×ss（41=32+8+1） | 3 |
| s2s^36 | s2s32×s2s4（36=32+4） | **2**（效率很高！） |

注：s2s^36 在建立平方梯度后仅需 2 次乘法——36=32+4 恰为两个 2 的幂之和。

---

## 缺陷修复：topo66、topo67 缺失的 k-shell 派发

此前的自动化强化运行已将 topo66、topo67 纳入 runtime 并建立了对应的 harness，但遗漏了 k-shell 派发函数与 proc.rs 路由。V3.79 修复了这两处缺口：

### 新增 dispatch_graph_topo_indices66

- 展示 NTETRAACTC（S^40）、NHTETRAACTC（(S+S)^39）、NAISO（(S_u²+S_v²)^34，α=68）
- Shell 触发词：`graph topo66`、`gtopo66`、`gntetraactc`、`gnhtetraactc`、`gnnaiso`、`gntetraactcnhtetraactcnaiso`

### 新增 dispatch_graph_topo_indices67

- 展示 NHENTETRAACTC（S^41）、NHHENTETRAACTC（(S+S)^40）、NAJSO（(S_u²+S_v²)^35，α=70）
- Shell 触发词：`graph topo67`、`gtopo67`、`gnhentetraactc`、`gnhhentetraactc`、`gnnajso`、`gnhentetraactcnhhentetraactcnajso`

### 新增 dispatch_graph_topo_indices68

- 展示 NDOTETRAACTC（S^42）、NHDOTETRAACTC（(S+S)^41）、NAKSO（(S_u²+S_v²)^36，α=72）
- Shell 触发词：`graph topo68`、`gtopo68`、`gndotetraactc`、`gnhdotetraactc`、`gnnakso`、`gndotetraactcnhdotetraactcnakso`

---

## 修改文件清单

| 文件 | 变更 |
|------|------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices68_inner()` + `graph_topo_indices68()` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices66/67/68()` |
| `crates/k-shell/src/proc.rs` | 新增 topo66、topo67、topo68 路由 |
| `host-tests/gos-graph-topo68-harness/Cargo.toml` | 新建 harness 包 |
| `host-tests/gos-graph-topo68-harness/.cargo/config.toml` | 宿主目标覆盖 |
| `host-tests/gos-graph-topo68-harness/tests/graph_topo68.rs` | 10 项测试（全绿） |

---

## 测试结果

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

宿主测试套件累计：**1763 tests**（此前 1753 + 本次新增 10）

---

## VectorAddress L4 命名空间

88=graph-topo 起始，至 154=graph-topo67，**155=graph-topo68**

---

## 插件与执行器 ID

- 插件：`TOPIX_68`
- 执行器：`t68.exec`
- VectorAddress L4：155

---

*自动硬化运行 — 每2小时执行一次产品级强化*
