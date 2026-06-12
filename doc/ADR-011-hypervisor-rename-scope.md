# ADR-011：`crates/hypervisor` 改名范围——目标名字与 diff 大小

> 状态：**提案待选向** · 提案日期：2026-06-12 · 配套：[V2 计划 line 25/104](../plan/V2_DEVELOPMENT_PLAN.md)（"`hypervisor` → `gos-graph-engine` 改名（不只是改名，是 identity shift——它已缩成 rewrite engine 本身）"）、[crates/gos-rewrite](../crates/gos-rewrite)（既有的图重写算法库）、V3 计划（Linux 兼容/VM guest = 明确 V4+ 非目标）
>
> 口径：V2 line 104 的抱怨是真实的——"hypervisor"这个目录名暗示 VM/guest 虚拟化，与 V3 计划明确写下的非目标矛盾。但调查发现这个 crate 的 **Cargo 包名早就不是 "hypervisor" 了**——是 `gos-kernel`，且 `gos-kernel`/`x86_64-gos-kernel` 已是构建系统（target-spec、CI、Makefile、`.cargo/config.toml`）的事实标准（21 处引用）。真正过时的只是**目录名**这一处。而 V2 line 104 提议的目标名 `gos-graph-engine`，会与早已存在、且确实是"rewrite engine 本身"的 `crates/gos-rewrite` 产生概念重叠。

## 一、问题陈述

### 1.1 现状：三个名字，三种身份

| 身份 | 当前值 | 出现位置 |
|---|---|---|
| 目录名 | `crates/hypervisor/` | 唯一一处仍叫"hypervisor"的地方 |
| Cargo 包名 | `gos-kernel`（[Cargo.toml:2](../crates/hypervisor/Cargo.toml)） | `-p gos-kernel`、`cargo bootimage --package gos-kernel`（[Makefile](../Makefile)）、CI `cargo check -p gos-kernel`（[.github/workflows/graph-governance.yml:26](../.github/workflows/graph-governance.yml)） |
| target-spec 文件名 | `x86_64-gos-kernel.json`（仓库根目录，[.cargo/config.toml](../.cargo/config.toml)） | build-std target，`target/x86_64-gos-kernel/...` 输出路径 |

`doc/GITHUB_DESCRIPTION_020.md` 的"Building & Running"代码块在三行内同时出现两个名字：

```bash
cd crates/hypervisor
cargo bootimage
qemu-system-x86_64 \
  -drive format=raw,file=target/x86_64-gos-kernel/debug/bootimage-gos-kernel.bin \
```

这正是 V2 line 104 抱怨的症状的最浓缩展示：克隆仓库的人第一眼看到 `cd crates/hypervisor`（"这是个 hypervisor"），下一行就看到 `gos-kernel`（"不对，是个 kernel"）。**目录名是落后的那一方**——Cargo 包名、target-spec、CI、Makefile 早已统一用 `gos-kernel`，只有目录名还停在 V0 时代的命名。

### 1.2 真正的结构性引用点：只有 3 处

逐 crate grep `crates/hypervisor`（路径字符串）后，**结构性引用**（构建/治理脚本读取该路径，rename 后若不同步会失败）只有：

1. [`Cargo.toml:4`](../Cargo.toml) —— workspace member `"crates/hypervisor"`
2. [`tools/verify-graph-architecture.ps1:74,83`](../tools/verify-graph-architecture.ps1) —— 治理脚本硬编码 `Read-RepoFile "crates\hypervisor\src\main.rs"` / `"crates\hypervisor\src\builtin_bundle.rs"`（CI `graph-governance.yml` 跑这个脚本）
3. [`scratch_cypher.py:5`](../scratch_cypher.py) —— 一次性 scratch 脚本里的硬编码绝对路径

其余命中（V2 计划、ADR-002/005/009/010、`GITHUB_DESCRIPTION_020.md`）都是**文档 prose 里的相对链接**——指向"写这份文档那一刻"的代码位置，本身就是历史快照（类似 git blame，不强求随代码演进同步更新）。

### 1.3 `gos-kernel`（21 处引用）已经是事实标准，与目录名脱节

`x86_64-gos-kernel`/`gos-kernel` 出现在：`.cargo/config.toml`（target 名、runner 配置）、根目录 `x86_64-gos-kernel.json`（target-spec 文件，**已经在仓库根目录，不在 `crates/hypervisor/` 里**）、`.github/workflows/graph-governance.yml`（CI 检查）、`Makefile`（构建/运行命令）、`xtask/src/main.rs`、`run.ps1`、`host-tests/run-gos-supervisor-validation.ps1`、`tools/build-installer.ps1`，以及 V2.3b 起每个 V2.x 阶段总结里反复出现的 `cargo build -p gos-kernel --release`。**这 21 处全部已经在用 `gos-kernel`，与目录名 `hypervisor` 完全脱钩——它们不关心目录叫什么，只关心 `[package] name`。**

## 二、选项

### 选项 A —— 仅目录改名：`crates/hypervisor/` → `crates/gos-kernel/`（与既有包名对齐）

把目录名追上**已经是事实标准**的包名 `gos-kernel`，不新造任何名字。

- **diff**：`git mv crates/hypervisor crates/gos-kernel` + 更新 §1.2 的 3 处结构引用（`Cargo.toml:4` 一行、`verify-graph-architecture.ps1` 两行、`scratch_cypher.py` 一行）。
- **不需要改**：`.cargo/config.toml`、`x86_64-gos-kernel.json`（已在根目录）、CI、Makefile、`xtask`、`run.ps1`、`build-installer.ps1`、`host-tests/*.ps1`——这 21 处全部继续工作，因为它们引用的是包名/target 名，不是目录路径。
- **解决 V2 line 104 的真实抱怨**：目录不再叫"hypervisor"，不再暗示 VM/guest 虚拟化，与 V3 非目标对齐；同时修复 §1.1 的"`GITHUB_DESCRIPTION_020.md` 三行两个名字"症状——改完后 `cd crates/gos-kernel` 与 `bootimage-gos-kernel.bin` 是同一个词根。
- **不引入新概念冲突**：`gos-kernel` 这个名字本来就准确描述这个 crate 是什么——它是产出可启动内核二进制的 crate（`cargo bootimage --package gos-kernel`）。

### 选项 B —— 完整改名为 `gos-graph-engine`（V2 line 104 字面建议）

目录 + 包名 + target-spec 文件名（`x86_64-gos-kernel.json` → `x86_64-gos-graph-engine.json`，位于仓库根目录）全部改为 `gos-graph-engine`。

- **diff**：选项 A 的 3 处 + **全部 21 处 `gos-kernel`/`x86_64-gos-kernel` 引用**（CI workflow、Makefile ×2、`.cargo/config.toml` ×3、`xtask`、`run.ps1`、`host-tests` 校验脚本、`build-installer.ps1`、target-spec JSON 文件本身的重命名）。
- **代价：与 `crates/gos-rewrite` 概念重叠**。`gos-rewrite`（`no_std` 算法库：ready-set 传播、quiescence、因果深度计、`capability_check`/`reachable_via_grant`）**已经是**"rewrite engine 本身"——这正是 V2 line 104 用来描述改名目标的措辞。`crates/hypervisor`（现 `gos-kernel`）不是这个引擎本身，而是**托管**它的可启动二进制：链接 `gos-rewrite` + `gos-runtime` + 30+ 个 `k-*` 驱动 crate，跑 `kernel_main`。把后者命名为 `gos-graph-engine`，会让不熟悉代码库的人合理地猜测它和 `gos-rewrite` 是同一样东西，或存在某种显而易见的包装关系——但实际依赖方向是反过来的（`gos-kernel` 依赖 `gos-rewrite`，不是 `gos-rewrite` 依赖/是 `gos-kernel`）。
- target-spec 文件改名前建议先 `cargo clean`（`/target` 已在 `.gitignore`，风险低但路径假设会变）。

### 选项 C —— 不改，留给 V3 统一命名 pass

V3 还会引入 `gos-sdk`、gpm 包格式、`crates/k-wasm`（ADR-014）等新命名面；可以把"hypervisor"目录名问题与这些新名字放在同一次命名审视里处理。

- **代价**：V2 line 104 的抱怨（目录名暗示 VM，与 V3 非目标矛盾；`GITHUB_DESCRIPTION_020.md` 三行两个名字）继续悬空，且这是**纯文档/路径层面**的不一致，与 V3 的新命名决策（功能性，如"wasm 解释器叫什么 `ExecutorId`"）性质不同——没有理由绑在一起决策，只是绑在一起执行的话省一次"to 同一批改名"的协调成本。

## 三、建议与门禁

倾向 **A**：`crates/hypervisor/` → `crates/gos-kernel/`。这是全计划里少见的"三选项中一个选项的 diff 严格是另一个的子集，且子集本身就完整解决了被引用的抱怨"的情况——A 是 B 的前 3 行 diff，且 A 已经让目录名追上构建系统 21 处早已采用的身份（`gos-kernel`），不需要再造 `gos-graph-engine` 这个会与 `gos-rewrite` 冲突的新词。"GOS 的核心是图重写引擎"这个概念本身已经有名字了——`gos-rewrite`（算法）+ `gos-runtime`（活图状态）+ `gos-kernel`（托管前两者的可启动 binary）三者合起来即是，不需要第四个名字。

**门禁**：选项 A 的 4 处编辑（`git mv` + `Cargo.toml:4` + `verify-graph-architecture.ps1` 两处 + `scratch_cypher.py` 一处）是纯机械操作，rename 后跑 `cargo check --workspace` + 治理脚本 + boot-smoke 三件套即可验证零行为变更（mirrors ADR-007/009/010 之类"现状已大致到位、只缺一次对齐"的轻量执行）。历史 ADR/V2 计划文档里 `../crates/hypervisor/...` 形式的链接是否批量替换为 `../crates/gos-kernel/...`，还是保留作"撰写时刻快照"——两种做法都不影响功能，留给选向时一并决定；若选 B，额外门禁是 target-spec JSON 重命名前先 `cargo clean`，并确认 21 处引用逐一更新不遗漏。
