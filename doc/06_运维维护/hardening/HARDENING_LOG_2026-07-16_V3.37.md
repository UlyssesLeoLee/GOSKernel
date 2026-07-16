# HARDENING LOG — V3.37 (2026-07-16)

## 变更概要

**版本**: V3.37  
**日期**: 2026-07-16  
**分支**: feat/vk-auto-live-surface  
**主题**: NPC + NRM₂ + NRSO Neighborhood S-variant 拓扑指数 + gos-graph-topo26-harness (10 测试)

---

## 新增功能

### 三项 S-variant 拓扑指数（基于邻居度和 S(v)）

S(v) = Σ_{w∈N(v)} deg(w)（邻居度和，与 topo18/topo21–topo26 家族一致）

#### 1. NPC — Neighborhood Product Connectivity（邻域积连通性）
- **定义**: NPC(G) = Σ_{uv∈E} √(S_u·S_v)
- **类比**: S 版本的积连通性指数 R_{1/2}（Bollobás & Erdős 1998）
- **实现**: isqrt128(S_u·S_v·10^12)，ppm 精度，u128 中间值防溢出
- **不变量**: S-正则图: NPC = |E|·S·10^6（精确整数）；仅 K₂(S=1) 时 NPC = |E|×10^6
- **溢出安全**: S_u·S_v ≤ 16129² ≈ 2.6×10^8，× 10^12 ≈ 2.6×10^20 ≤ u128::MAX ✓

#### 2. NRM₂ — Neighborhood Reduced Second Zagreb（邻域简化第二 Zagreb）
- **定义**: NRM₂(G) = Σ_{uv∈E} (S_u-1)·(S_v-1)
- **类比**: S 版本的 RM₂（Furtula, Gutman & Ediz 2014）
- **实现**: (S_u-1)·(S_v-1)，精确 u64，saturating_sub 防下溢
- **不变量**: S-正则图: NRM₂ = |E|·(S-1)²；NRM₂ = 0 当且仅当所有边满足 S_u=1 或 S_v=1（K₂型）
- **溢出安全**: 每边 ≤ 16128² ≈ 2.6×10^8，总计 ≤ 512 × 2.6×10^8 ≈ 1.33×10^11 < u64::MAX ✓

#### 3. NRSO — Neighborhood Reciprocal Sombor（邻域互易 Sombor）
- **定义**: NRSO(G) = Σ_{uv∈E} 1/√(S_u²+S_v²)
- **类比**: S 版本的 RSO（Gutman 2022，度版本已在 topo20 实现）
- **实现**: isqrt64(10^12/(S_u²+S_v²))，ppm 精度
- **巧合**: K₂、P₃、K_{2,3} 均给出 NRSO = 707_106（不同分母下的取整巧合）；K_{1,4} 给出 707_104（4 × floor(1/√32×10^6) = 4 × 176_776）
- **溢出安全**: 分母 S_u²+S_v² ≥ 2（边端点非孤立），≤ 2×16129² ≈ 5.2×10^8；10^12/分母 ≤ 5×10^11 < u64::MAX ✓

---

## 解析验证表

| 图          | NPC(ppm)   | NRM₂ | NRSO(ppm) | 边数 | 点数 |
|-------------|-----------|------|-----------|------|------|
| 空图        | 0         | 0    | 0         | 0    | 0    |
| 单点        | 0         | 0    | 0         | 0    | 1    |
| K₂ 边      | 1_000_000 | 0    | 707_106   | 1    | 2    |
| P₃ 路径    | 4_000_000 | 2    | 707_106   | 2    | 3    |
| K₃ 三角    | 12_000_000 | 27  | 530_328   | 3    | 3    |
| K_{1,4} 星 | 16_000_000 | 36  | 707_104   | 4    | 5    |
| P₄ 路径    | 7_898_978 | 8    | 790_402   | 3    | 4    |
| K₄ 完全    | 54_000_000 | 384 | 471_402   | 6    | 4    |
| 两孤立点   | 0         | 0    | 0         | 0    | 2    |
| K_{2,3} 二部 | 36_000_000 | 150 | 707_106  | 6    | 5    |

### 关键精度推导

**P₄ NPC** (sa=2,sb=3):
- isqrt128(6·10^12) = 2_449_489（√6·10^6 ≈ 2_449_489.74；取下整）
- Total = 2×2_449_489 + 3_000_000 = 7_898_978 ✓

**K₄ NRSO** (S=9, denom=162):
- 10^12/162 = 6_172_839_506.17...
- 78_567² = 6_172_773_489 ≤ 6_172_839_506; 78_568² = 6_172_930_624 > 6_172_839_506
- isqrt64 = 78_567; 6×78_567 = 471_402 ✓

**K_{2,3} NRSO** (S=6, denom=72):
- 10^12/72 = 13_888_888_888.88...
- 117_851² = 13_888_858_201 ≤ 13_888_888_888; 117_852² = 13_889_093_904 > 13_888_888_888
- isqrt64 = 117_851; 6×117_851 = 707_106 ✓（与 K₂/P₃ 同值，不同分母取整巧合）

---

## 实现细节

### gos_runtime::graph_topo_indices26_inner()
**返回值**: `(npc_ppm: u64, nrm2: u64, nrso_ppm: u64, edge_count: usize, node_count: usize)`

**算法**: O(V+E) — adj+deg 扫描 → S(v) 计算 → 边扫描（a < b），无 BFS，无 float

**栈占用**: adj[128](u128=2KB) + deg[128](u64=1KB) + sv[128](u64=1KB) ≈ 4KB

**辅助函数**: isqrt128（Babylonian法，无浮点，no_std安全）+ isqrt64（同类）

### Shell 命令
- `graph topo26` / `gtopo26`
- `neighborhood product conn` / `gnpc`
- `neighborhood reduced zagreb2` / `gnrm2`
- `neighborhood reciprocal sombor` / `gnrso`
- `gnpcnrm2nrso`

### VectorAddress L4 命名空间
L4=113 为 gos-graph-topo26-harness 使用

---

## 测试覆盖

**新增**: gos-graph-topo26-harness — 10 项测试
1. test_01_empty — 空图 → (0, 0, 0, 0, 0)
2. test_02_single_node — 单孤立点 → (0, 0, 0, 0, 1)
3. test_03_single_edge — K₂ → (1_000_000, 0, 707_106, 1, 2)
4. test_04_path_p3 — P₃ → (4_000_000, 2, 707_106, 2, 3)
5. test_05_triangle_k3 — K₃ → (12_000_000, 27, 530_328, 3, 3)
6. test_06_star_k14 — K_{1,4} → (16_000_000, 36, 707_104, 4, 5)
7. test_07_path_p4 — P₄ → (7_898_978, 8, 790_402, 3, 4)
8. test_08_complete_k4 — K₄ → (54_000_000, 384, 471_402, 6, 4)
9. test_09_two_isolated — 两孤立点 → (0, 0, 0, 0, 2)
10. test_10_k23_bipartite — K_{2,3} → (36_000_000, 150, 707_106, 6, 5)

**全部通过**: 10/10 ✓

**累计 host-test 总数**: 1343（新增 10，上版 V3.36 共 1333）

---

## OS 类比

- **NPC** = S-加权几何耦合度（>|E| 对 hub-spoke 拓扑；= |E|·S·10^6 对 S-正则图）
- **NRM₂** = 邻域简化键强度（对 S=1 所有边为 0；S-正则图 = |E|·(S-1)²，衡量内部连接密度）
- **NRSO** = S 欧几里得范数倒数（高值=低次图，均匀低负载网格；= 0 仅对孤立节点边）

---

## 参考文献

- Bollobás B. & Erdős P. (1998). Graphs of extremal weights.
- Furtula B., Gutman I. & Ediz S. (2014). On difference of Zagreb indices.
- Gutman I. (2022). Geometric approach to degree-based topological indices: Sombor indices.
- S-variant family (topo18–topo26): Mondal et al. 2019 延伸
