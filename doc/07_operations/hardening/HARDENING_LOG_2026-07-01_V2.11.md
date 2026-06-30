# Hardening Log — 2026-07-01 — V2.11

## 产品级文档体系重组（doc/ restructure）

**交付摘要**：把 `doc/` 从一层平铺的 26 个文件，重组为按软件工程生命周期分层的
8 个分类目录（`00_project` → `01_spec` → `02_architecture` → `03_adr` →
`04_implementation` → `05_reference` → `06_testing` → `07_operations`），并新增
`doc/README.md` 作为分层索引。这是"做到和 Windows/iOS/Linux 一样的产品级水准"
的文档基础设施部分——成熟操作系统项目（Linux kernel `Documentation/`、
Windows WDK 文档树）都按生命周期阶段分层文档，而不是平铺堆放。

### 发现与完成的工作

本次硬化周期开始时，`main` 工作区已存在一份**未提交**的 doc 重组（文件已物理
移动到新目录，但从未 `git commit`）。核实内容：

- 26 个旧文件 1:1 映射到新目录，**零内容丢失**（逐文件 diff 确认，包括
  `GOS_GOVERNANCE_v0_2.md` → `00_project/GOVERNANCE.md` 的重命名，内容字节级
  相同）。
- `README.md`、`plan/V2_DEVELOPMENT_PLAN.md` 的内部链接已同步更新到新路径。
- `doc/README.md` 索引文件已写好，含每个分类的中日双语说明 + 07 节硬化日志
  按版本号的索引表。

补完的部分（本次新增）：

- `crates/gos-protocol/src/edge_algebra.rs` 文档注释中 `doc/ADR-001` 的相对链接
  未同步，指向已不存在的 `doc/ADR-001-edge-algebra-constitution.md`——已更新为
  `doc/03_adr/ADR-001-edge-algebra-constitution.md`。
- `plan/OPTIMIZATION_PLAN.md` 中 `doc/PHASE_B4_DOMAIN_ISOLATION.md` 链接同样
  未同步——已更新为 `doc/02_architecture/PHASE_B4_DOMAIN_ISOLATION.md`。
- 全仓库 grep 确认无其余悬空的旧 `doc/<FLAT>.md` 链接残留（`.claude/worktrees/`
  下的引用属于其他未合并分支，不在本次范围内）。

### 验证

- `cd host-tests/gos-protocol-harness && cargo test`：8/8 通过（确认
  doc-comment 路径修复未影响编译）。
- `cargo check -p gos-kernel`（workspace root）：通过，仅预存在、与本次改动
  无关的 warning。
- 本次改动为纯文档/链接层面，未触及任何运行时代码路径，无需新增 harness。

### 分类目录结构

| 目录 | 内容 |
|------|------|
| `00_project` | Graph Prime 规则、治理、对外项目描述 |
| `01_spec` | Runtime 规格、图原生调度语义 |
| `02_architecture` | 主线架构、设计总览、Phase B.4 隔离设计 |
| `03_adr` | 架构决策记录（ADR-001、ADR-004；ADR-006...017 仍在 `feat/v2-mutation-dispatcher` 分支未合并，见 [[gos-roadmap-direction]]） |
| `04_implementation` | 实施路线图、执行 backlog |
| `05_reference` | CLI 手册、Cypher 节点手册、网络节点、裸机安装指南 |
| `06_testing` | 系统测试报告 |
| `07_operations` | 硬化日志（按版本号索引） |

### 已知后续

- `feat/v2-mutation-dispatcher` 分支上的 ADR-006...017（12 份提案待选向）合并
  到 `main` 时，需落入 `doc/03_adr/`，索引表需相应扩展——不在本次范围。
- `.claude/worktrees/{determined-hamilton,intelligent-clarke,v2-advance}` 下各
  自有独立的 `doc/`、`plan/` 副本，指向各自分支历史时点的旧路径；这些是独立
  worktree 的内部一致性问题，不影响 `main` 的文档体系，留待对应分支合并时一并
  处理。
