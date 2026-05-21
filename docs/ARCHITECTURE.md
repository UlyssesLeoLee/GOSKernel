# GOS Kernel Architecture

> 一个基于图论的下一代操作系统内核。Nodes + Edges 是原生 substrate，
> Cypher 是控制面。所有功能都建立在内核的 native primitives 上。

## 1. 核心思想 (Thesis)

经典 OS 把 process / file / socket 作为基础抽象。GOS 把 **graph node**
和 **graph edge** 作为唯一的基础抽象：

- **Node** = 一个执行单元（可以是 hardware driver, service, compute, router）
- **Edge** = 节点之间的关系（mount, use, depend, signal, call, link, …）
- 所有 system call → Cypher 查询/变更
- 所有 IPC → 沿着 edge 路由的 signal 或 RPC
- 所有 audit → 自动写入 journal envelope

这意味着：**任何高层功能都可以用图变更表达，不需要绕开内核架构**。

## 2. Crate 拓扑

```
gos-protocol  ← 数据结构、enum 定义、ABI 版本号（被所有人依赖）
       │
       ├── gos-runtime    ← node/edge 表，调度，capability 解析，journal ring
       ├── gos-supervisor ← 模块生命周期、ACL gate、Cypher mutation dispatcher
       ├── gos-journal    ← 序列化/反序列化、replay
       ├── gos-cypher-mut ← Cypher 写操作的数据结构
       ├── gos-loader     ← ELF / manifest 验证（K.x 待用）
       └── gos-vfs        ← 文件系统抽象（K.x 待用）

k-*  (plugins)            ← 每个 k-* crate 实现一个内核插件
       │
       ├── k-fb     framebuffer 驱动 (mode 13h + Bochs DispI HD path)
       ├── k-ps2    键盘
       ├── k-mouse  鼠标
       ├── k-shell  命令行
       ├── k-cypher Cypher 解析 + 调度
       ├── k-cuda-host  CUDA bridge
       ├── …       (~21 个插件)
       │
hypervisor (gos-kernel)   ← 主二进制，bootloader 入口，UI 渲染，命令栏
```

每个 plugin 通过 `BuiltinPluginDescriptor` 注册：manifest + native node bindings + register_hook。

## 3. 数据流

### 3.1 控制平面 (Control Plane)
```
[plugin action] → emit_control_plane(envelope)
                    │
                    ├── push to control_plane: RingQueue<Envelope>
                    │   (旧消费者：visualizer, 外部 telemetry pipe)
                    │
                    └── push to journal: JournalRing<512>  [J.2]
                        (新消费者：SHOW JOURNAL, save/restore)
```

`ControlPlaneEnvelope` = `{ version, kind, subject:[u8;16], arg0, arg1 }`。
8 种 kind:Hello, PluginDiscovered, NodeUpsert, EdgeUpsert, StateDelta,
SnapshotChunk, Fault, Metric, CypherMutationAudited。

### 3.2 数据平面 (Data Plane)
```
node A.executor → ctx.route_signal(target)     (异步 fire-and-forget)
              │
              └─→ runtime.signal_queue push
                  │
                  pump() drains → route_signal(target, sig)
                                    │
                                    └─→ target.executor.on_event
```

或同步：
```
node A → rpc_invoke(target, request: u64)      (J.3, 同步)
              │
              ├── save RPC_SLOT
              ├── route_signal(target, Signal::Call { from: request })
              │       │
              │       └─→ target reads rpc_request(), calls rpc_reply(value)
              │
              └── return slot.response
```

## 4. 调度 (J.7)

Ready queue 是优先级感知的 ring：
```
node_priority: u8  (0=lowest, 128=default, 255=highest)
                   NODE_PRIORITY_HIGH = 192
                   NODE_PRIORITY_BACKGROUND = 64
```

每次 `pump` 扫描 ready ring，pop 最高优先级条目；同优先级按 FIFO。

通过 Cypher：`SET PRIORITY 'V' = N` (K.2)。

## 5. 安全 (J.6)

子域 ACL：
```rust
const fn sub_domain_allows_edge(
    source: NodeSubDomain,
    target: NodeSubDomain,
    edge_kind: u8,
) -> bool;
```

当前规则集：
- **J.6.A** — Hardware 子域的节点只能被 KernelDriver 子域 Mount

`gos_supervisor::apply_cypher_mutation` 会在 mutation 落地前调用此函数；
违反会被 reject 为 `MUTATION_GATE_ACL_VIOLATION`。

## 6. Capability 版本协商 (J.4)

```rust
CapabilitySpec { namespace, name, version: u32 }
ImportSpec     { ..., min_version: u32, max_version: u32 }
```

在 `validate_imports` 中检查：`provider.version ∈ [import.min_version, import.max_version]`。
不兼容的 provider 被视为未导出。

## 7. 持久化 (J.2)

`JournalRing<512>` 自动接收每个 `emit_control_plane`：
- `push(env)` ring-style overwrite when full
- `flush_into(buf)` serializes header + records, oldest-first
- `envelope_at(i)` 读出第 i 个旧条目

通过 Cypher：`SHOW JOURNAL [LIMIT N]` (J.8)。

将来的 K.x 跨重启持久化：把 `flush_into` 的 blob 写到 VFS 文件，
boot 时 replay。

## 8. UI / Command Bar

底部命令栏 + chat HUD + 3D scene 是一个统一的 boot UI。3D scene 显示
runtime 图：metallic spheres (nodes) + ropes (edges) + Verlet 物理 +
PBR 着色 (I.14)。

命令栏接受三种语句：
1. **Cypher reads**: `SHOW STATS / NODES / EDGES / PLUGINS / JOURNAL / PRIORITY`
2. **Cypher mutations**: `CREATE MOUNT / USE`, `LINK`, `REBIND USE`, `DELETE EDGE`
3. **Cypher actions**: `SET PRIORITY 'V' = N`, `RESET PRIORITY 'V'`, `INVOKE 'V' WITH N`
4. **Built-in commands**: `kernel`, `os`, `help`, `clear`, `log`, `nodes`, `edges`, `ps`, `gen`, `uptime`, `journal`, `watch` / `unwatch`, `inspect <vec>`

每次输入产生 `you> ...` echo + `cypher> ...` 或 `gos> ...` reply，写
入 scrollback ring，前 4 条作为 chat HUD overlay 浮在 3D scene 上。

## 9. Audit Roadmap (closed)

- **P0 #1** — `resolve_capability` 自动创建 Use edge → 解决跨插件能力解析绕开图的问题
- **P0 #2** — `register_node_routes` 自动创建 Signal edge → 让条件路由变可见
- **P1 #3** — `FaultEvent { callee, caller, status }` 替换裸 VectorAddress → 故障可归因到调用链
- **P2 #4** — `NodeSubDomain` 类划分 → 安全策略基础
- **P2 #5** — `manifest_edges_well_formed` → 拒绝跨插件 spoof 边

## 10. Phase 进度

- **Phase A-H** — 早期 bootstrap，capability lookup，Cypher writes
- **Phase I** — 3D UI (octahedral → metal balls + ropes), Cypher in command bar, PBR shader, chat HUD
- **Phase J** — Kernel native mechanism completion (J.1-J.8)
- **Phase K** — Cypher as full control plane
   - K.1 `SHOW STATS` — 综合运行时状态
   - K.2 `SET PRIORITY 'V' = N` — Cypher 写 J.7 priority
   - K.3 `INVOKE 'V' [WITH N]` — Cypher 发起 J.3 RPC
   - K.4 `ARCHITECTURE.md` (本文档)
   - K.6 `watch` / `unwatch` — 实时 tail journal envelope 流
   - K.8 `SHOW PRIORITY 'V'` + `RESET PRIORITY 'V'` — priority 子系统闭环

## 11. Phase L+ candidates (待定)

| 候选 | 说明 |
|---|---|
| J.3.B | 指针 payload RPC（payload + length 编码到 u64） |
| J.2.B | VFS-backed journal — 真正跨重启持久化 |
| K.7   | NodeSpec.default_priority — 节点 manifest 声明默认优先级 (58 个 literal 需要迁移) |
| L.1   | `SET <node> <property> = <value>` — 通用属性变更框架 |
| L.2   | Plugin hot-reload — 通过 J.4 版本号原子替换 implementation |
| L.3   | Schema enforcement — `state_schema_hash` 升级为完整 schema descriptor |
| L.4   | Deadline-aware scheduling — 每节点微秒预算 + 超时 fault |
| L.5   | Ring-3 ELF loader — 用户进程，capability 作为 protection model |
| L.6   | RPC-capable echo plugin — 让 INVOKE 有真实 target，可做 ping/bench |

## 12. 测试覆盖

- **runtime harness** (host) — 32 tests，覆盖 audit, RPC, priority, journal, sub-domain mapping, capability resolution
- **supervisor harness** (host) — 16 tests，覆盖 module lifecycle, ACL, capability binding
- **gfx harness** (host) — 5 tests
- **xtask qemu smoke** — 验证完整 boot 路径到 `enabling interrupts; entering steady-state`

每个 Phase J/K commit 都通过 smoke + 完整 harness。

## 13. 如何扩展

要添加一个新功能：

1. **不是新机制** → 在现有 plugin 里实现，用 Cypher mutate 图，不需要碰内核
2. **新机制需要 RPC** → 用 `rpc_invoke` 调用 service node
3. **新机制需要订阅事件** → 注册一个 plugin，给它一个 conditional route，让上游节点 route 给它
4. **新机制需要存储** → 写入 journal envelope 或将来的 VFS（K.x）
5. **新机制需要安全边界** → 通过 `NodeSubDomain` 分类 + `sub_domain_allows_edge` rule

任何功能都能用以上这五条 primitives 表达——这就是 GOS 架构完备性的定义。
