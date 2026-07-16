# 硬化日志 V2.83 — 图指标快照保存与比较

**日期：** 2026-07-04
**分支：** feat/vk-auto-live-surface
**提交：** 1c2d26a
**宿主测试总计：** 803（此前 793，+10）

---

## 功能：`graph snapshot` / `graph compare`

### 动机

生产级操作系统监控系统（Linux `sysstat`、Windows 性能监视器、iOS MetricKit）
都提供捕获系统指标基线并与当前状态比较以检测漂移的能力。GOSKernel 此前缺少
这一能力——运维人员能够查看当前拓扑指标，但无法确定图相对于某个已知良好状态
发生了怎样的变化。

V2.83 新增了一套**指标快照基线**系统，类比 Linux sysstat 中的
`sar -o snap.bin`（保存）与 `sar -s <start> -e <end>`（比较）。

---

## 实现

### gos-runtime/src/lib.rs

**新增公开类型：**
```rust
#[derive(Copy, Clone)]
pub struct MetricSnapshot {
    pub valid: bool,       // 在首次调用 graph_snapshot_save() 之前为 false
    pub epoch: u64,        // 保存时刻的 graph_epoch
    pub node_count: usize,
    pub edge_count: usize,
    pub density_ppm: u32,  // 图密度 × 1_000_000
    pub trans_ppm: u32,    // 全局传递性 × 1_000_000
    pub avgcc_ppm: u32,    // 平均聚类系数（WS）× 1_000_000
    pub geff_ppm: u64,     // 全局效率 × 1_000_000
    pub leff_ppm: u32,     // 局部效率 × 1_000_000
    pub sigma_ppm: u32,    // 小世界 σ × 1_000_000（0=未定义）
    pub kappa_ppm: u32,    // 无标度 κ × 1_000_000（0=未定义）
    pub gamma_ppm: u32,    // 幂律 γ̂ × 1_000_000（0=未定义）
}
```

**新增静态变量：**
```rust
static METRIC_SNAPSHOT: Mutex<MetricSnapshot> = Mutex::new(MetricSnapshot { valid: false, ... });
```

**新增私有方法**（`impl GraphRuntime`）：
- `graph_snapshot_inner(&self) -> MetricSnapshot`
  在同一次 `RUNTIME.lock()` 持有期间调用全部 8 个 `*_inner()` 方法，以保证 epoch 一致性。

**新增公开函数：**
- `pub fn graph_snapshot_save() -> u64`
  捕获当前指标；存入 `METRIC_SNAPSHOT`；返回捕获时刻的 graph_epoch。
- `pub fn graph_snapshot_compare() -> (MetricSnapshot, MetricSnapshot)`
  返回 `(saved, current)`——current 始终是实时计算得到的。

### crates/k-shell/src/lib.rs

**`dispatch_graph_snapshot(sink)`** —— 运行 `graph_snapshot_save()`，打印已保存的
基线（epoch、节点数、边数、密度、聚类系数、效率、σ、κ、γ̂），并附带确认脚注。

**`dispatch_graph_compare(sink)`** —— 运行 `graph_snapshot_compare()`，渲染一张
三列表格：`saved | current | delta`，并对差值做颜色编码：
- 绿色（+）：指标增长
- 红色（-）：指标缩小
- 灰色（±0）：无变化

脚注显示：`epoch: N → M（epoch advanced by K）` 或 `（自快照以来无结构性变更）`。

若尚无快照，则显示：`no baseline — run 'graph snapshot' first`（尚无基线——请先运行 'graph snapshot'）。

### crates/k-shell/src/proc.rs

新增路由：
```
"graph snapshot" | "gsnapshot"  → dispatch_graph_snapshot
"graph compare"  | "gcompare"   → dispatch_graph_compare
```

为两个命令及其别名添加了帮助文本条目。

---

## 测试装置：gos-graph-snapshot-harness（L4=59）

10 个集成测试，覆盖：

| 编号 | 用例 | 断言 |
|---|------|-----------|
| 1 | 尚未保存前 | 空图上 `cur.valid=true`，`node_count=0` |
| 2 | 保存空图 | `saved.valid=true`，所有指标为 0 |
| 3 | 保存非空图 | `node_count=2`，`edge_count=1`，`density_ppm>0` |
| 4 | 比较未变化 | `saved.epoch == cur.epoch`，指标完全相同 |
| 5 | 节点数差值 | 新增节点后：`cur.node_count = saved+1` |
| 6 | 密度差值 | 新增边后：`saved.density=0`，`cur.density>0` |
| 7 | 三角形传递性 | 完全双向连通的 K3 时 `trans_ppm = 1_000_000` |
| 8 | 二次保存覆盖 | 第二次保存得到 `node_count=2`，第一次（=1）被丢弃 |
| 9 | 孤立时 geff=0，连通后 >0 | 双向连接的一对节点：连接后 `geff_ppm > 0` |
| 10 | current.valid 不变量 | 在空图/孤立图/连通图之间 `cur.valid=true` 恒成立 |

**结果：** 10/10 通过，退出码 0。

---

## 设计决策

1. **单次 RUNTIME 锁持有**用于 `graph_snapshot_inner()`——确保全部 8 个指标
   反映同一个图 epoch，防止因交错的变更而产生不一致。

2. **独立的 `METRIC_SNAPSHOT` 静态变量**而非嵌入到 `GraphRuntime` 中——使快照
   能够在 `reset()` 调用之间持久存在（有利于跨测试周期的监控）。

3. 使用 **`MetricSnapshot.valid`** 标志而非 `Option<MetricSnapshot>`——在
   no_std 环境下更易使用（避免判别式开销；结构体始终保持 `Copy`）。

4. `geff_ppm` 使用 **u64** —— 与 `graph_global_efficiency()` 的返回类型一致，
   避免在效率接近 1_000_000 的稠密图上出现精度损失。

---

## VectorAddress L4 命名空间（已更新）

```
L4=59  gos-graph-snapshot-harness (V2.83, 新增)
L4=58  gos-graph-diameter-harness (V2.82)
L4=57  gos-graph-summary2-harness (V2.81)
L4=56  gos-graph-power-law-harness (V2.80)
```

---

## 指标覆盖矩阵（V2.83 之后）

| 类别             | 指标                          | 版本 |
|----------------------|---------------------------------|---------|
| 监控基线  | 快照保存与比较（差值） | V2.83   |
| 综合视图        | 中心 + 外围面板       | V2.82   |
| 幂律拟合        | 指数 MLE γ̂                  | V2.80   |
| 拓扑仪表盘   | 一次性汇总                | V2.79   |
| 无标度检测 | 度异质性 κ              | V2.78   |
| 小世界检测| σ = (CC/CC_rand)/(L/L_rand)     | V2.77   |
| 局部容错| E_loc = (1/n)ΣE(G_v)           | V2.76   |
| 平均聚类系数       | WS 逐节点 (1/n)ΣCC(v)        | V2.75   |
| 全局效率    | E(G) = Σ1/d(i,j)/(n(n-1))      | V2.74   |
| 中心节点         | ecc == radius                   | V2.73   |
| 外围节点     | ecc == diameter                 | V2.72   |
| 调和中心性  | HC[v] = Σ1/d(v,u)              | V2.71   |
| Wiener 指数         | W(G) = Σ 成对距离之和     | V2.70   |
| 围长                | 最短有向环长度  | V2.69   |
| 富人俱乐部          | ρ(k) = 枢纽间密度       | V2.68   |
| 模块度          | Newman-Girvan Q                 | V2.67   |
| 互惠性          | 互相连边所占比例            | V2.66   |
| 同配性        | Newman 度混合系数 r          | V2.65   |
| k-核分解 | Batagelj-Zaversnik 剥离法             | V2.64   |

---

## 后续建议

- `graph snapshot list` —— 未来方向：命名快照（需要动态存储）
- `graph watch compare` —— 在实时监视面板中叠加显示差值
- 正确性说明：`dispatch_graph_summary` 对来自 `graph_density()` 的
  `edge_count`/`node_count` 使用了颠倒的变量名，但位置对应的取值仍然正确——
  未来重构时可考虑清理。
