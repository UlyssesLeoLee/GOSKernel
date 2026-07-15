# HARDENING LOG — V3.32
**Date**: 2026-07-15  
**Session**: Automated 2h hardening run  
**Branch**: feat/vk-auto-live-surface  
**Commit**: 0b43798

---

## 变更摘要

实现 V3.32：`graph topo21` 命令——ABC₄（4代原子键连接性指数）+ NH（邻域调和指数）+ NSO（邻域 Sombor 指数）三个基于邻域度和 S(v) 的 S-变体拓扑指数，并创建 gos-graph-topo21-harness 10 个测试全部通过。

---

## 新增功能

### V3.32 — ABC₄ + NH + NSO S-变体拓扑指数

**核心定义**（S(v) = Σ_{w∈N(v)} deg(w)，即顶点 v 的邻域度和）：

| 指数 | 公式 | 文献 | 实现精度 |
|------|------|------|----------|
| ABC₄ | Σ_{uv∈E} √((S_u+S_v−2)/(S_u·S_v)) | Ghorbani & Hosseinzadeh 2010 | floor ppm (isqrt64) |
| NH   | Σ_{uv∈E} 2/(S_u+S_v)              | S-analogue of Harmonic H     | floor ppm (整除) |
| NSO  | Σ_{uv∈E} √(S_u²+S_v²)             | S-analogue of Sombor SO      | floor ppm (isqrt128) |

**实现公式**（无浮点，no_std 安全）：
- ABC₄ per edge = `isqrt64((ssum−2)×10^12 / (S_u·S_v))`，当 ssum≤2 时贡献 0
- NH per edge = `floor(2_000_000 / ssum)`
- NSO per edge = `isqrt128((S_u²+S_v²)×10^12) as u64`（用 u128 中间值防止 S²×10^12 溢出）

**关键不变量**：
- ABC₄=0 当 S_u+S_v=2（仅 K₂：S(A)=S(B)=1）
- S-均匀性：K₃ 和 K_{1,4} 的每条边 S 值均为 4，三个指数的逐边贡献完全相同
- K_{2,3} 尽管左部 d=3、右部 d=2，但 S(left)=S(right)=6（S-均匀！）

**算法**：O(V+E) 度扫描——第一遍计算 deg[]，第二遍计算 S(v)，第三遍边扫描；无需 BFS。

### 分析验证表

| 图         | ABC₄(ppm)  | NH(ppm)   | NSO(ppm)    | 边数 | 节点数 |
|------------|------------|-----------|-------------|------|--------|
| 空图       | 0          | 0         | 0           | 0    | 0      |
| 孤立点     | 0          | 0         | 0           | 0    | 1      |
| K₂         | 0          | 1_000_000 | 1_414_213   | 1    | 2      |
| P₃         | 1_414_212  | 1_000_000 | 5_656_854   | 2    | 3      |
| K₃         | 1_837_116  | 750_000   | 16_970_562  | 3    | 3      |
| K_{1,4}    | 2_449_488  | 1_000_000 | 22_627_416  | 4    | 5      |
| P₄         | 2_080_878  | 1_133_333 | 11_453_742  | 3    | 4      |
| K₄         | 2_666_664  | 666_666   | 76_367_532  | 6    | 4      |
| 2 孤立点   | 0          | 0         | 0           | 0    | 2      |
| K_{2,3}    | 3_162_276  | 999_996   | 50_911_686  | 6    | 5      |

注：K_{2,3} NH=999_996 而非 1_000_000，因 floor(2_000_000/12)=166_666，累计误差 4 ppm。

---

## 代码变更

### `crates/gos-runtime/src/lib.rs`
- 新增 `graph_topo_indices21_inner()` 内函数（S(v) 两遍扫描 + ABC₄/NH/NSO 逐边计算）
- 新增 `graph_topo_indices21()` 公开接口并添加完整 doc 注释

### `crates/k-shell/src/lib.rs`
- 新增 `dispatch_graph_topo_indices21()` 显示函数
  - 亮黄色标题：`graph topo21 (ABC₄ + NH + NSO S-variant indices)`
  - ABC₄ 亮青色（ppm，ssum≤2 时标注 "=0: S_u+S_v=2 for all edges"）
  - NH 亮绿色（ppm）
  - NSO 亮洋红色（ppm）
  - 页脚：`Ghorbani & Hosseinzadeh 2010  (NH/NSO: S-variant family)`

### `crates/k-shell/src/proc.rs`
- 新增 shell 路由（之前版本已添加）：
  - `"graph topo21"` / `"gtopo21"` / `"abc4 index"` / `"gabc4"` / `"neighborhood harmonic"` / `"gnh"` / `"neighborhood sombor"` / `"gnso"` / `"gabc4nhnso"`

### `host-tests/gos-graph-topo21-harness/`（新建）
- `.cargo/config.toml`：host target `x86_64-pc-windows-msvc`
- `Cargo.toml`：独立 workspace，依赖 gos-protocol / gos-cypher-mut / gos-runtime / gos-supervisor
- `tests/graph_topo21.rs`：10 个测试全部通过（VectorAddress L4=108 命名空间）

---

## 测试结果

```
running 10 tests
test test_01_empty ... ok
test test_02_single_node ... ok
test test_03_single_edge ... ok
test test_04_path_p3 ... ok
test test_05_triangle_k3 ... ok
test test_06_star_k14 ... ok
test test_07_path_p4 ... ok
test test_08_complete_k4 ... ok
test test_09_two_isolated ... ok
test test_10_k23_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored
```

**累计 host 测试数：1293**（上版 1283 + 本次 10）

---

## OS 类比

| 指数 | OS 含义 |
|------|---------|
| ABC₄ | S-变体 ABC：二阶度加权连接脆弱性（S+S−2 越小→边越"内聚"；基于邻域度压力而非直接度） |
| NH   | 邻域调和吞吐率（高=所有边端点的邻域度和均匀且小；=0 无意义仅发生于 K₂ 边界） |
| NSO  | 邻域 Sombor 耦合范数（Euclidean S-向量模；高=端点邻域度压力不对称；S-均匀时 =√2·S·m·10^6） |

---

## VectorAddress L4 命名空间（更新）

…107=graph-topo20, **108=graph-topo21**

---

## 下一步建议

可继续实现 topo22：选题候选：
- **Degree-Sum index DS + Sum-Connectivity S** (Vukičević & Gašperov 2010 weighted variant)
- **F-index variant FI₂** (Second Forgotten index Σ_v d_v⁴)
- **Hyper-Sombor HS** (Gutman 2022: Σ_{uv∈E} (d_u²+d_v²)²/√(d_u²+d_v²))

所有候选均为 O(V+E) 度扫描，可直接跟进。
