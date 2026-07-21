# GOSKernel 强化日志 V3.49 · 2026-07-16

## 摘要

新增 **NDoC + NHUC + NDSO** Neighborhood S-variant 拓扑指数
（`graph_topo_indices38`），附完整 harness 覆盖（10 项测试，0 失败）。

## 新增拓扑指数

### NDoC(G) = Σ_v S(v)¹²  — S-十二次方顶点和
- 精确 u64（u128 累加器，饱和运算，截断）
- 扩展顶点幂次序列：NM₁=Σ S² … NUC=Σ S¹¹（topo37） → NDoC=Σ S¹²（topo38）
- S-正则：NDoC = n·S¹²

### NHUC(G) = Σ_{uv∈E} (S_u+S_v)¹¹  — S-十一次方边和
- 精确 u64（u128 累加器，饱和运算，截断）
- 扩展边幂次序列：NHM1=Σ(S+S)² … NHDC=Σ(S+S)¹⁰（topo37） → NHUC=Σ(S+S)¹¹
- S-正则：NHUC = 2048·|E|·S¹¹

### NDSO(G) = Σ_{uv∈E} (S_u²+S_v²)⁶  — S-十二次 Sombor（α=12）
- 精确 u64，无需 isqrt（偶数次幂）
- 广义 Sombor SO^α 序列：NSO(α=1) … NTSO(α=10,topo37) → NDSO(α=12,topo38)
- S-正则：NDSO = 64·|E|·S¹²

## 交叉验证表

| 图     | NDoC              | NHUC                | NDSO               | 边数 | 点数 |
|-----------|-------------------|---------------------|--------------------|-------|-------|
| 空图     | 0                 | 0                   | 0                  | 0     | 0     |
| K₂        | 2                 | 2_048               | 64                 | 1     | 2     |
| P₃        | 12_288            | 8_388_608           | 524_288            | 2     | 3     |
| K₃        | 50_331_648        | 25_769_803_776      | 3_221_225_472      | 3     | 3     |
| K_{1,4}   | 83_886_080        | 34_359_738_368      | 4_294_967_296      | 4     | 5     |
| P₄        | 1_071_074         | 460_453_306         | 43_665_842         | 3     | 4     |
| K₄        | 1_129_718_145_924 | 385_610_460_475_392 | 108_452_942_008_704| 6     | 4     |
| K_{2,3}   | 10_883_911_680    | 4_458_050_224_128   | 835_884_417_024    | 6     | 5     |

## 变更文件

| 文件 | 变更内容 |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | `graph_topo_indices38_inner` + `graph_topo_indices38` |
| `crates/k-shell/src/lib.rs` | `dispatch_graph_topo_indices38` |
| `crates/k-shell/src/proc.rs` | 路由：`graph topo38 / gtopo38 / gndoc / gnhuc / gndso / gndocnhucndso` |
| `host-tests/gos-graph-topo38-harness/` | 新建 harness：Cargo.toml + .cargo/config.toml + tests/graph_topo38.rs |

## 测试结果

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed
```

## 指标

- 宿主测试套件总计：**1463 个测试**（topo38-harness 新增 10 个）
- VectorAddress L4 命名空间：125 = graph-topo38
- 插件：TOPIX_38 / 执行器：t38.exec
- Shell 别名：`graph topo38`, `gtopo38`, `gndoc`, `gnhuc`, `gndso`, `gndocnhucndso`
