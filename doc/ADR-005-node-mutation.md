# ADR-005：节点创建/销毁 vs claim/quota 稳定性

> 状态：**提案（问题陈述 + 选项，待你选向）** · 日期：2026-06-08 · 配套：[ADR-004 §三](./ADR-004-mutation-visibility.md)（node-create 推迟到此）· [V2 计划](../plan/V2_DEVELOPMENT_PLAN.md)
>
> 口径：V2.1 把 mutation 锁在 edge-only。本 ADR 处理被推迟的硬问题——**Cypher 能否创建/销毁 node**。这是 soul demo（`MATCH...CREATE` 出新 3D node）和涌现愿景的前提，但与 Phase B 的实例模型冲突。本 ADR 只陈述问题与选项，**不替你拍板**。

## 一、冲突

`gos-cypher-mut` 现在硬拒 node create/delete，理由写在代码里（[lib.rs:18-21](../crates/gos-cypher-mut/src/lib.rs)）：

> "允许 Cypher 凭空创建或销毁 node 会让下游每一个 claim 和 restart_generation 计数失效"——Phase B 的 instance binding / HeapQuota / fault attribution 全挂在**稳定 `NodeId`** 上。

但涌现愿景要求 `CREATE (n)` 能真造出 node（否则 graph 不能生长）。矛盾在于：**Phase B 的 node 是"有 claim、有 quota、有 instance 生命周期"的重实体；而涌现需要的是"能随手创建的轻图元"。**

## 二、选项

### 选项 A —— Provisional nodes（临时节点，我倾向的方向）
Cypher 创建的 node 是 **provisional** 的：可见、可连边、可渲染，但**不能持有 claim / quota / instance**，直到被显式 `promote` 成正式模块 node。
- **优点**：图能自由生长（soul demo 通），Phase B 不变式不被破坏（provisional node 进不了 claim/quota 表）。两类 node 共存，按能力分层。
- **代价**：需要 node 生命周期的二级状态（provisional → promoted）；runtime 要区分两类 node 的能力门。

### 选项 B —— 双命名空间
正式模块 node（Phase B 拥有，NodeId 由 plugin manifest 派生、稳定）与用户/Cypher node（独立 NodeId 空间，无 claim 资格）物理隔离。
- **优点**：隔离最干净，互不污染。
- **代价**：两套 NodeId 空间增加复杂度；跨空间连边的语义要额外定义。

### 选项 C —— 永不允许 node mutation（维持现状）
一切表达为 edge mutation；node 集合在 boot 时固定。
- **优点**：零风险，Phase B 完全不受影响。
- **代价**：图不能生长 → 涌现愿景和 soul demo 落空。**与 GOS 长期方向冲突**。

## 三、建议与门禁

倾向 **A（provisional nodes）**——它在"图自由生长"与"Phase B 不变式"之间取得平衡，且与 ADR-001 的 node/edge 一等公民模型自洽。但这需要先确认：
1. provisional node 的渲染策略（接 ADR-002 §六 的渲染模型决定）。
2. promote 的触发者与权限（capability = Grant 路径，接 ADR-001 §五）。

**本 ADR 不在 V2.1/V2.2 范畴**——它在 soul demo（V2.5）前必须定。列为待你选向的 backlog 决定，不阻塞当前主线。
