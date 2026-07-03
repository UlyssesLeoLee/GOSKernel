# HARDENING LOG — V2.50 (2026-07-02)

## 概述

**版本**: V2.50  
**日期**: 2026-07-02  
**功能**: `graph flow <source> <sink>` — Edmonds-Karp 最大流算法  
**测试**: 10 个 host-test 全部通过（gos-graph-flow-harness）

---

## 变更内容

### 算法实现 (`crates/gos-runtime/src/lib.rs`)

新增内部函数 `GraphRuntime::graph_flow_inner<const N: usize>` 和公开包装函数 `graph_flow<const N: usize>`。

**算法**: Edmonds-Karp（BFS Ford-Fulkerson）

- 将 `edge_weight` 视为有向边容量
- 残差图：前向残差 = capacity − flow，后向残差 = flow（支持流量取消）
- BFS 寻找最短增广路径（保证多项式复杂度 O(V × E²)）
- 沿路径按瓶颈容量增广，直到无增广路径为止
- 精度阈值：`1e-9`（避免浮点误差导致无限循环）

**返回值**: `([VectorAddress; N], [u32; N], [u32; N], usize, u32)`
- `vecs[0..node_count]`     — 所有活跃节点（source 第一位，sink 第二位）
- `out_flow[0..node_count]` — 每节点总出流量 × 1000（u32）
- `in_flow[0..node_count]`  — 每节点总入流量 × 1000（u32）
- `node_count`              — 活跃节点总数
- `max_flow`                — 最大流量 × 1000（u32）

**退化情况**（返回 max_flow=0）：
- source/sink 未找到
- source == sink
- 无增广路径（不连通）

**栈内存使用**：
- `edge_flow: [f32; MAX_EDGES]` = 2048 bytes（512 条边）
- BFS 辅助数组（pred/pred_edge/pred_fwd/visited/queue）= ~5 × 1024 bytes
- 节点流量统计 = 2 × 512 bytes
- 总计约 10KB，与现有图算法相当

### Shell 分发 (`crates/k-shell/src/lib.rs`)

新增 `dispatch_graph_flow(sink: &ConsoleSink, source: VectorAddress, snk_vec: VectorAddress)`：

**输出格式**：
```
 graph flow 1.1.1.0 → 1.1.4.0
 ───────────────────────────────────────────────────────────
  role       out-flow  in-flow   vector
  source     3.000     0.000     1.1.1.0
  relay      3.000     3.000     1.1.2.0
  sink       0.000     3.000     1.1.4.0
  isolated   0.000     0.000     1.1.3.0
 ───────────────────────────────────────────────────────────
4 node(s)  max-flow: 3.000
```

颜色编码：
- 洋红（13）= source
- 绿（10）= sink  
- 黄（14）= relay（中继节点）
- 灰（8）= isolated（孤立节点）

### 命令路由 (`crates/k-shell/src/proc.rs`)

支持的命令格式（插入在 `graph mst` 之后、`graph shortest` 之前）：
- `graph flow <src> <snk>`
- `flow <src> <snk>`
- `max flow <src> <snk>`
- `maxflow <src> <snk>`

参数解析：以空格分隔两个 VectorAddress，无效格式返回错误提示。

### 测试套件 (`host-tests/gos-graph-flow-harness/`)

```
host-tests/gos-graph-flow-harness/
├── .cargo/config.toml        (target = x86_64-pc-windows-msvc, build-std)
├── Cargo.toml
├── Cargo.lock
└── tests/graph_flow.rs       (10 个测试)
```

**测试矩阵**：

| # | 场景 | 预期 max_flow |
|---|------|--------------|
| 1 | 空图 | 0 |
| 2 | 单节点，source==sink | 0 |
| 3 | source 未注册 | 0 |
| 4 | sink 未注册 | 0 |
| 5 | K₂ A→B(cap=3.0) | 3000 |
| 6 | K₂ A→B(cap=2.5) | 2500 |
| 7 | 瓶颈：A→B(5.0), B→C(2.0) | 2000 |
| 8 | 菱形：A→B(3)+A→C(2)+B→D(3)+C→D(2) | 5000 |
| 9 | 三路径：各 1.0+1.0+0.5 直达 | 2500 |
|10 | 两个不连通分量，跨分量求流 | 0 |

全部通过 ✅（10/10）

---

## 算法背景

最大流问题（Max-Flow Problem）是网络流理论的核心：
- **现实类比**：给定有向网络（管道、通信链路），每条边有容量上限，求从 S 到 T 的最大可行流量
- **图论OS意义**：在 GOS 中，edge_weight 代表信号/数据传输容量，max-flow 回答"这两个内核子系统之间的最大吞吐量是多少"
- **OS类比**：`tc -s qdisc show` 带宽统计 + 流量瓶颈分析

**Edmonds-Karp vs. 普通 Ford-Fulkerson**：
- Ford-Fulkerson 使用任意增广路，复杂度 O(E × max_flow)，伪多项式
- Edmonds-Karp 指定 BFS 找**最短**增广路，O(V × E²)，真正多项式
- 对浮点容量（GOS 使用 f32 weights）必须用 Edmonds-Karp，否则可能不收敛

---

## 不变式检查

- [x] `graph_flow_inner` 是纯读取函数（无 epoch bump，无写操作）
- [x] `graph_flow` 公开包装通过 `RUNTIME.lock().topology_snapshot()` 获取快照后释放锁
- [x] 测试使用 `TEST_LOCK: Mutex<()>` + `reset()` 隔离
- [x] Harness `.cargo/config.toml` 配置正确（target + build-std）
- [x] VectorAddress 选取 L4=27 系列（与其他 flow 测试不冲突）

---

## 下一步

- `node checkpoint <vec>` — 快照节点状态到 diff ring（观测性）
- `graph sim <N>` — 模拟 N 步随机游走，输出信号流量轨迹
- `graph between` — 基于全对 Dijkstra 的有向带权介数中心性
- PAL_U32 → attribute node 重构（Demo A 前置条件）
