# GOS 优化计划

> 起草：2026-04-25
> 范围：在 Phase B.4（domain CR3 隔离，详见 [`doc/PHASE_B4_DOMAIN_ISOLATION.md`](../doc/PHASE_B4_DOMAIN_ISOLATION.md)）之外，把 GOS 从"graph runtime"补成真正"操作系统"的并行工作流。
> 当前 main 已完成：Phase A、Phase B.1–B.3、Phase B.5。

---

## 总体路线

```
Phase D — 工程基线        ← 不动语义，先把开发回路立起来
Phase E — 抢占 + Ring 3   ← 让 OS 真正具备隔离与公平性
Phase F — 持久化           ← 解锁 "关机不丢"
Phase G — 真正的可扩展性    ← 外部模块 + 网络 + GPU 一等公民
Phase H — 自描述 OS         ← graph mutation / 分布式 / 形式化
```

D 和 E 可以并行启动；F 必须排在 D 之后；G 依赖 B.4 + E；H 是长期方向。

---

## Phase D — 工程基线（2–3 周）

**目标**：把 build / test / verify 做成 `cargo xtask <verb>` 一条命令。

### D.1 host-side 测试可跑通
- `host-tests/gos-supervisor-harness` 健康度验证（必须从该目录而非 workspace root 调用）
- 新增 `host-tests/gos-runtime-harness`：mock NodeExecutorVTable + signal 流量测试
- 在 supervisor harness 补 B.1（fault bridge）/ B.3（quota）/ B.5（degrade）回归
- 完成判据：`cargo xtask test` 全绿

### D.2 cargo xtask
- `xtask` crate 提供 `build / test / verify / qemu / lint`
- `verify` Rust 重写 `tools/verify-graph-architecture.ps1`
- `qemu` 装 ISO 跑 5 秒，串口看到 `boot: supervisor owns system cycle` 才算通过
- 完成判据：单条命令、CI 通过

### D.3 静态质量门槛
- `cargo clippy --workspace --all-targets -D warnings` 进 CI
- `rust-toolchain.toml` 锁定；bump `x86_64` / `linked_list_allocator`
- 修剪 dead_code（k-nim / k-serial / k-panic）

### D.4 结构化日志
- `gos-log` crate：`log!(level, module, instance, fmt, ...)`
- 双轨：串口（开发）+ 控制面 envelope（shell 订阅）
- 替换 hypervisor 的 `raw_serial_println!`

### D.5 ABI 版本三元组
- `GOS_ABI_VERSION` → `(major, minor, patch)`
- manifest 加载：major 必须一致、minor 兼容
- minor mismatch 的 mock manifest 被 `ModuleRejected`

---

## Phase E — 抢占式调度 + Ring 3（4–6 周）

**前置**：B.4.3（IST stack）必须先于 E 任何 Ring 切换。

### E.1 Tick-driven preemption ✅ 已完成（2026-04-26）
- PIT post stage 调用 `gos_runtime::tick_pulse()` → supervisor `on_tick`
- per-instance 时间片预算（`TIME_SLICE_DEFAULT_TICKS = 12`，约 100 ms）
- 用尽 → 设置 `preempt_requested` 标志 → 当前 native callback 返回后 runtime 检测 → re-enqueue + 状态升级为 `CellResult::Yield`
- 通过 `Scheduler { on_tick, should_preempt, clear_preempt }` hook 表解耦（runtime → supervisor 不会形成依赖环）
- 回归：`preempt_flag_re_enqueues_instance_and_reports_yield`（runtime harness）
- 限制：当前是软抢占（plugin 当前 callback 跑完才让出）；硬抢占（中断 plugin 代码）属 E.2/E.4 范围

### E.2 Ring 3 切换基础（结构性 ✅ 已完成 2026-04-26；端到端待 E.3.x）
- k-gdt：Selectors 加 `user_code_selector / user_data_selector`，user code/data descriptor 在 GDT 中按 IA32_STAR sysret 要求的连续布局注册
- hypervisor 新增 `ring3` 模块：`init()` 写 `IA32_STAR / IA32_LSTAR / IA32_FMASK`，并启用 `EFER.SCE`
- `syscall_entry` naked 汇编 trampoline：保存 user RIP（RCX）/ user RFLAGS（R11），调度到 `rust_syscall_handler`，sysretq 返回
- `rust_syscall_handler` 当前是骨架：识别 4 个 syscall number（AllocPages / FreePages / EmitSignal / ResolveCapability），暂返回 0；实际转发到 gos-runtime ABI 的实现是 E.3 后续
- kernel_main 在第一个 `service_system_cycle()` 后（GDT 已经被 k-gdt 加载）调用 `unsafe { ring3::init(); }`
- 限制：今天没有 plugin 在 Ring 3 跑（B.4.6.x ELF loader 没有产出 user-mode .gosmod），所以 syscall trampoline 不会被实际执行；MSR 已就位

### E.3 Native plugin 迁到 Ring 3（结构性 ✅ 已完成 2026-04-26；plugin 迁移待 B.4.6.x）
- gos-protocol 新增 `Privilege::Kernel | Privilege::User` enum 和 `MODULE_FLAG_USER = 1 << 0` 常量
- `ModuleDescriptor.flags` bit 0 编码 privilege；`ModuleDescriptor::privilege()` 解码
- 选择 flags 字段而不是新加字段：所有现存 21 个 builtin 都写 `flags: 0`，零源码改动；priv 默认 Kernel
- supervisor `start_module` 拒绝启动 `Privilege::User` 模块（返回 `ModuleRejected`）—— 直到 Ring 3 dispatch trampoline 闭环之前，强制保留隔离不变式
- 回归：`user_level_module_is_rejected_at_start`（supervisor harness）
- 后续切片（依赖 B.4.6.x）：把 1–2 个 leaf plugin（k-cypher / k-ai）打成 ELF + 标 `MODULE_FLAG_USER`，验证 Ring 3 dispatch + syscall round trip

### E.4 SMP 启动（已延后到独立 phase）
- LAPIC + AP 唤醒 + per-CPU 数据 + TLB shootdown 是多周工作量
- 当前 `RUNTIME / SUPERVISOR / DOMAIN_SWITCH / FAULT_DISPATCH / SCHEDULER` 均为单 Mutex；SMP 需要至少分片 RUNTIME
- 完成判据：QEMU `-smp 4` 下 shell 显示 4 核都在跑
- 状态：作为 Phase H 之外的独立 future track 跟踪，不阻塞 Phase E 视为完成

---

## Phase F — 持久化（结构性 ✅ 已完成 2026-04-26；驱动/FS impl 后续切片）

### F.1 Block 驱动 — 结构性
- `gos_protocol::block::BlockDeviceVTable` C ABI：read_sector / write_sector / flush / geometry
- `BlockGeometry { sector_count, sector_size, flags }`、`BlockIoStatus` 错误分类
- `RESOURCE_BLOCK_DEVICE` ResourceId 注册到 supervisor 默认资源表
- 实际 AHCI / NVMe 驱动（F.1.1 / F.1.2）后续切片
- 回归：`vfs_trait_drives_a_synthetic_in_memory_filesystem`（runtime harness）通过合成 ramdisk 验证 ABI round-trip + out-of-bounds 错误码

### F.2 VFS 抽象 — 结构性
- 新 crate `gos-vfs`（no_std）：`Inode / DirEntry / InodeKind / MountId / InodeNum / VfsError / FileSystem trait / MountSource`
- `RESOURCE_FILE_HANDLE` 注册到 supervisor，plugin 可声明 file-handle claim
- 同上回归测试驱动一个 in-memory `TinyFs` impl，验证 lookup / read / read_dir 端到端

### F.3 FAT32 read-only — 后续切片
- 实现 `FileSystem` trait 的 FAT32 reader（可考虑外部 `fatfs` crate 或自研）
- shell `ls / cat`

### F.4 Graph state 持久化 — 后续切片
- 控制面 envelope 写 `/var/gos/journal.bin`
- 启动回放 journal 重建图
- 完成判据：手工建边 → reboot → 边仍在

### F.5 写路径 + 安全 — 后续切片
- FAT32 write、journal fsync
- 拔电后 journal 完整

---

## Phase G — 可扩展性（4–5 周，与 B.4.6 衔接）

### G.1 ELF loader
- ET_DYN + R_X86_64_RELATIVE
- manifest 嵌在 `.gos.manifest` section
- shell `module load /apps/foo.gosmod`

### G.2 模块签名
- ed25519（`ed25519-dalek` no_std）
- trust root 内嵌；自签 / 篡改 / 无签名 → 通过 / 拒绝 / 拒绝

### G.3 socket + smoltcp
- 替换 k-net 当前 ad-hoc ARP/ICMP
- `Socket` capability（TCP / UDP）

### G.4 GPU 资源化
- GPU memory 显式纳入 `RESOURCE_GPU_ACCEL` 配额

---

## Phase H — 自描述 OS（长期）

| 子项 | 价值 | 难度 |
|---|---|---|
| Cypher 写支持（受控 mutation） | 用图语言改图、不重启 | ★★★ |
| LLM-driven 控制面（k-ai 闭环） | OS 自述、自调优 | ★★★★ |
| 分布式图（远程节点用 VectorAddress） | 集群操作系统 | ★★★★ |
| 形式化不变量验证（Kani / Prusti） | "supervisor 永远不会 charge degraded module" | ★★ |
| Snapshot / migration | runtime 状态序列化迁移 | ★★★★ |

---

## 时间线汇总

```
month 1: D 全套                                     | E.1
month 2: E.2 + E.3                                  | F.1
month 3: B.4.1–B.4.5                                | F.2
month 4: F.3 + F.4                                  | E.4 或推迟
month 5: G.1 + G.2                                  | F.5
month 6: G.3                                        | H 子项试点
```

| 里程碑 | 完成判据 |
|---|---|
| **M1: 可调试 OS** | D 全部完成；CI 绿；shell 看得到结构化日志 |
| **M2: 真正的 OS** | E.1+E.2+E.3 + B.4 完成；至少一个 plugin 在 Ring 3 + 独立 CR3 |
| **M3: 持久 OS** | F.4 完成；reboot 后状态保留 |
| **M4: 可扩展 OS** | G.1+G.2 完成；外部签名模块装载运行 |
| **M5: 联网 OS** | G.3 完成；shell 能跑出站 TCP |
| **M6: 自描述 OS** | H 任一子项试点跑通 |

---

## 立刻可启动的两条线

- **主线**：B.4.1 — Domain PML4 构造（设计文档已就绪）
- **支线**：D.1 + D.2 — 工程回路立起来，所有后续阶段受益

本 plan 第一步：**D.1 — host harness 健康度验证 + 补 B.1/B.5 回归测试**。
