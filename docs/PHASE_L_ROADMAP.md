# Phase L — Cypher 控制面增量扩展 + 工具链补全

继 Phase K (Cypher 作为完整控制面) 之后，Phase L 围绕**实用性、
可观察性、自动化**做精细化补完。每项独立小巧，但合起来把 Phase
J/K 的原语转化成日常可用的工具。

## 已完成 (10 项 + 2 项工具)

| # | 标题 | 受益 |
|---|---|---|
| **L.4** | Deadline-aware scheduling | J.7 priority 的延迟维度补充；RDTSC 测量 + Fault 计数 |
| **L.6** | Built-in RPC echo (0.0.0.0) | INVOKE 立即可用；BENCH 有 baseline |
| **L.7** | `BENCH RPC <N>` | RDTSC cycle 级延迟测量工具 |
| **L.8** | RPC counters in SHOW STATS | metric 链路打通 word + buf |
| **L.9** | `WATCH [filter]` | 实时按 envelope kind 过滤 tail |
| **L.12** | `SHOW CAPABILITIES` | 全系统 export capability 一览 |
| **J.3.B** | `rpc_invoke_buf` 指针 RPC | 任意字节流 + payload helpers |
| **K.8** | SHOW/RESET PRIORITY | J.7 子系统 Cypher 闭环 |

工具：

| # | 工具 | 用途 |
|---|---|---|
| **tools/rpa/** | 11 个 PowerShell 脚本 | Windows 上 QEMU 全套自动化 (launch/quit/monitor/sendkey/screenshot/serial-read/cypher/bench/smoke) |
| **interfaces/** | 4 个 YAML 文件 | 21 plugin 全清单 + edge types + KernelAbi 接口契约 |
| **xtask check-interfaces** | CI lint | 防 YAML/Rust manifest drift |

## 测试覆盖

| 测试套 | 之前 | 之后 |
|---|---|---|
| runtime harness | 29 (Phase J 起点) | **36** |
| supervisor harness | 16 | 16 |
| gfx harness | 5 | 5 |
| **总计** | 50 | **57** |

QEMU smoke 全 Phase L 期间无回归。

## Phase L 期间 Cypher 控制面增量

新加 7 个 verb：

```
读   SHOW CAPABILITIES
读   SHOW DEADLINE 'V'
写   SET DEADLINE 'V' = N
读   SHOW PRIORITY 'V'         (K.8)
写   RESET PRIORITY 'V'        (K.8)
工具 (内置命令)
     watch <filter>            (L.9 扩展 K.6)
     bench [rpc] [N]           (L.7)
```

加上 J/K 期间的：

```
读   SHOW STATS / NODES / EDGES / PLUGINS / JOURNAL
写   CREATE MOUNT/USE, LINK, REBIND USE, DELETE EDGE
动作 SET PRIORITY 'V' = N, INVOKE 'V' [WITH N]
```

→ Cypher 控制面: **15 个 verb**, 全部归一到 k-cypher 解析器。

## RDTSC-based scheduling 子系统完整图

```
NodeRecord.priority: u8           (J.7)  →  ready queue 排序
NodeRecord.deadline_cycles: u64   (L.4)  →  dispatch wrapper RDTSC 围测

dispatch:
  let deadline = lookup(node_id);
  let t0 = _rdtsc();
  <executor>
  let t1 = _rdtsc();
  if t1-t0 > deadline { Fault envelope + counter++ }

Cypher control:
  SET PRIORITY 'V' = N    →  J.7
  SET DEADLINE 'V' = N    →  L.4
  SHOW PRIORITY/DEADLINE  →  J.7 + L.4
  RESET PRIORITY 'V'      →  J.7

Observability:
  SHOW STATS (rpc / deadlines overruns)
  WATCH fault            →  实时看 deadline overrun
  BENCH RPC <N>          →  RDTSC delay 测量
```

## Phase M 候选

剩下的 K/L+ 候选未做的：

| # | 标题 | 状态 |
|---|---|---|
| **K.7** | NodeSpec.default_priority — 节点 manifest 声明默认优先级 | deferred (58 NodeSpec literal 大迁移) |
| **L.2** | Plugin hot-reload — 通过 J.4 版本号原子替换 implementation | deferred (大工程，需详细设计) |
| **L.3** | Schema enforcement — `state_schema_hash` 升级为完整 schema descriptor | deferred (中等，需要 vtable schema 字段) |
| **L.5** | Ring-3 ELF loader — 用户进程，capability 作为 protection model | deferred (最大工程) |
| **J.2.B** | VFS-backed journal | deferred (需 VFS write 链路) |

未来工作方向 (Phase M)：

| # | 标题 | 大致工作量 |
|---|---|---|
| **M.1** | tools/rpa 的 Linux/macOS 等价物 (bash/python) | 小 (移植 PowerShell 逻辑) |
| **M.2** | plugins.yaml schema 完整版 (含 nodes/edges/permissions 字段全覆盖) + 对应 lint 校验 | 中 |
| **M.3** | 内置 `htop` 风格 ASCII 仪表盘 (持续刷新 SHOW STATS) | 小 |
| **M.4** | INVOKE 异步版本 (`INVOKE_ASYNC` + 后续读结果) | 中 |
| **M.5** | 内置 cron-style 周期任务 (priority + deadline 配合) | 中 |

## 仓库形状

```
crates/                    21 plugin crates (k-*) + 8 service crates (gos-*)
docs/
   ARCHITECTURE.md
   PHASE_J_ROADMAP.md     Phase J 收尾 (J.1-J.8)
   PHASE_L_ROADMAP.md     <-- 本文档
interfaces/
   README.md
   plugins.yaml           21 plugin 清单, J.4 versioned
   runtime-edges.yaml     10 edge types + Cypher verb 索引
   kernel-abi.yaml        KernelAbi vtable 描述
tools/
   rpa/                   11 个 PowerShell 脚本 + README
xtask/                    build / test / lint / qemu / check-interfaces
host-tests/               runtime + supervisor + gfx harnesses
```
