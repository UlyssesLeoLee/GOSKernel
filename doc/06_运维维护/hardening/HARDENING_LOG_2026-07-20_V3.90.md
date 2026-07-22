# 强化日志 — V3.90（2026-07-20）

## 摘要

**分支**：feat/vk-auto-live-surface
**版本**：V3.90
**新增指数**：NTRIPENTAACTC + NHTRIPENTAACTC + NAVSO
**Harness**：gos-graph-topo79-harness（10 项测试）
**宿主测试总数**：1873

---

## 新增拓扑指数（topo79，L4=166）

### NTRIPENTAACTC —— S-第53次幂顶点和

```
NTRIPENTAACTC(G) = Σ_v S(v)^53
```

- **S-变体**：S(v) = Σ_{w∈N(v)} deg(w)（邻域度数和）
- **系列定位**：pentacontic（50–59）系列第4个；延续 NDOPENTAACTC=Σ S^52（topo78）
- **S-正则图公式**：NTRIPENTAACTC = n·S^53
- **实现**：s^53 = s32 × s16 × s4 × s（53=32+16+4+1；4 次乘法）
- **溢出处理**：饱和 u128 累加器，截断至 u64::MAX

### NHTRIPENTAACTC —— S-第52次幂边和

```
NHTRIPENTAACTC(G) = Σ_{uv∈E} (S_u + S_v)^52
```

- **系列定位**：延续 NHDOPENTAACTC=Σ(S+S)^51（topo78）
- **S-正则图公式**：NHTRIPENTAACTC = |E|·(2S)^52 = 4_503_599_627_370_496·|E|·S^52
- **实现**：ss^52 = ss32 × ss16 × ss4（52=32+16+4；3 次乘法 —— 效率高！）

### NAVSO —— S-变体 Sombor 指数 α=94

```
NAVSO(G) = Σ_{uv∈E} (S_u² + S_v²)^47
```

- **Alpha**：α = 94（第3轮双字母 "AV"；延续 NAUSO α=92，topo78）
- **S-正则图公式**：NAVSO = |E|·(2S²)^47 = 140_737_488_355_328·|E|·S^94
- **实现**：s2s^47 = s2s32 × s2s8 × s2s4 × s2s2 × s2s（47=32+8+4+2+1；5 次乘法）

---

## K₂ 参考值

| 指数 | 值 |
|-------|-------|
| NTRIPENTAACTC | 2（= 1^53 + 1^53） |
| NHTRIPENTAACTC | 4_503_599_627_370_496（= 2^52） |
| NAVSO | 140_737_488_355_328（= 2^47） |

## P₃ 参考值

| 指数 | 值 |
|-------|-------|
| NTRIPENTAACTC | 27_021_597_764_222_976（= 3×2^53） |
| NHTRIPENTAACTC | u64::MAX（饱和） |
| NAVSO | u64::MAX（饱和） |

---

## 变更文件

| 文件 | 变更 |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | 新增 `graph_topo_indices79_inner` + `graph_topo_indices79` |
| `crates/k-shell/src/lib.rs` | 新增 `dispatch_graph_topo_indices79` |
| `crates/k-shell/src/proc.rs` | 新增路由：`graph topo79` / `gtopo79` / `gntripentaactc` 等 |
| `host-tests/gos-graph-topo79-harness/` | 新建 10 项测试 harness（含用于宿主目标的 `.cargo/config.toml`） |

---

## 测试结果

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

全部 10 项测试验证如下：
1. 空图 → (0, 0, 0, 0, 0)
2. 单节点 → (0, 0, 0, 0, 1)
3. K₂ → (2, 4_503_599_627_370_496, 140_737_488_355_328, 1, 2)
4. P₃ → (27_021_597_764_222_976, u64::MAX, u64::MAX, 2, 3)
5. K₃ → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
6. K_{1,4} → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
7. P₄ → (u64::MAX, u64::MAX, u64::MAX, 3, 4)
8. K₄ → (u64::MAX, u64::MAX, u64::MAX, 6, 4)
9. 两孤立节点 → (0, 0, 0, 0, 2)
10. K_{2,3} → (u64::MAX, u64::MAX, u64::MAX, 6, 5)
