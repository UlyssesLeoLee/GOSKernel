# Phase B.4 — Domain CR3 / 模块镜像隔离设计

> 状态：设计阶段
> 起草：2026-04-25
> 前置：Phase B.1–B.3、B.5 已落地。supervisor 已具备 module / instance / claim / heap / fault 完整控制面，唯独 **address-space 隔离尚未启用**——builtin 模块仍与 hypervisor 共享同一 CR3。

---

## 一、目标

让每个 supervisor module 拥有独立的虚拟地址空间根（PML4），通过 `mov cr3` 在模块入口与出口切换；模块间的资源调用必须经过 supervisor trampoline，不能直接读写对方的私有窗口。

完成本阶段后：

- 模块越界访问会触发 page fault → 由 supervisor 归因 → 走 B.5 degrade 流程
- 共享内核高半区（kernel high half）在所有 domain 间一致，但模块私有窗口（image / stack / heap / ipc）互相不可见
- builtin native 节点继续可用，但每次 dispatch 进入 native callback 前会切到对应模块的 CR3，回到 runtime 后再切回 kernel CR3
- 跨模块的资源调用（包括 capability invocation、信号路由）通过 supervisor 提供的 trampoline 进入目标 domain

---

## 二、当前状态

### 2.1 已具备

- `gos_protocol::ModuleDomain` 数据结构：`{ id, root_table_phys, image_base, image_len, stack_base, stack_len, ipc_base, ipc_len, heap_base, heap_len }`
- `gos_supervisor` 在 `map_module` 阶段已分配 domain id 与窗口范围
- `request_pages` 已根据 `domain.root_table_phys` 映射堆页（前提：root_table_phys 非零）
- `k-vmm` 拥有 `map_page` / 当前 PML4 操控原语
- B.1 fault attribution 已能把异常归因到 module
- 所有 builtin 都是静态链接进 kernel，不需要装载

### 2.2 关键缺口

1. `domain.root_table_phys` 始终为 0：从未真正分配过私有 PML4
2. 没有 ELF loader / relocator
3. 没有 CR3 切换 trampoline
4. 没有专用 IST stack：模块内 page fault 会复用模块的栈，可能踩到 fault 起因本身
5. 没有跨 domain 调用规约（caller 与 callee 谁切 CR3、何时切）

---

## 三、设计原则

| 原则 | 含义 |
|---|---|
| **共享高半，隔离低半** | 所有 domain 共用同一份 kernel high-half（含 hypervisor 代码、runtime mutex、supervisor 控制面、IST 栈）；只有 user-half 上的模块私有窗口（image/stack/heap/ipc）按 domain 切换 |
| **Trampoline-only crossing** | 跨 domain 边界（包括 native callback 进入、信号路由、capability invocation）只能经过 supervisor 提供的 trampoline；trampoline 是高半区代码，对所有 domain 可见 |
| **Fault 不递归** | page fault / double fault 处理路径走 IST stack（一组在内核高半区的固定栈），与故障模块的私有栈完全独立 |
| **CR3 切换由 supervisor 拥有** | 模块永远不应自己执行 `mov cr3`；supervisor 在 trampoline 中切换，并在切之前/之后写 fence |
| **builtin 继续可用** | builtin native crate 静态链接进 kernel binary，不走 ELF 装载；它们仍要进入对应模块的 CR3 才能访问私有窗口（heap），但代码执行体 `EXECUTOR_VTABLE.on_event` 本身位于高半区 |

---

## 四、总体架构

```
┌─────────────────────────────────────────────────────────────────┐
│  Kernel high half  (shared across all domains)                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐         │
│  │ Hypervisor│  │ Runtime  │  │Supervisor│  │ IST stacks│        │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘         │
│  ┌────────────────────────────────────────────────────┐         │
│  │ trampoline: enter_domain / leave_domain / fault    │         │
│  └────────────────────────────────────────────────────┘         │
└─────────────────────────────────────────────────────────────────┘
                       ▲                    ▲
       per-domain      │ enter / leave       │ fault → kernel high half
       low half        │ (cr3 switch)        │ via IST stack
                       │                    │
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│  Domain A user   │  │  Domain B user   │  │  Domain C user   │
│  ┌────────────┐  │  │  ┌────────────┐  │  │  ┌────────────┐  │
│  │ image      │  │  │  │ image      │  │  │  │ image      │  │
│  │ stack      │  │  │  │ stack      │  │  │  │ stack      │  │
│  │ heap       │  │  │  │ heap       │  │  │  │ heap       │  │
│  │ ipc window │  │  │  │ ipc window │  │  │  │ ipc window │  │
│  └────────────┘  │  │  └────────────┘  │  │  └────────────┘  │
└──────────────────┘  └──────────────────┘  └──────────────────┘
```

---

## 五、主要组件

### 5.1 Domain PML4 构造

新增 `k-vmm::clone_kernel_pml4() -> u64`：

- 分配一页物理内存做新 PML4 root
- 复制当前 kernel PML4 的 **高半部分**（≥ `0xFFFF_8000_0000_0000`）所有顶层条目；低半部分置零
- 返回 root 物理地址，写入 `ModuleDomain.root_table_phys`

调用时机：`Supervisor::map_module` 当前已经设置好 domain 窗口；插入 `clone_kernel_pml4` 调用，把返回值写入 record。

### 5.2 私有窗口映射

`request_pages` 当前用 `map_domain_heap_pages(domain.root_table_phys, base, page_count, writable)` 映射，但走的是当前 CR3 而非 domain CR3 的页表。需要：

- `k-vmm::map_into_pml4(root_phys, va, frame, flags)`：临时把目标 PML4 通过递归映射或线性 map 写入条目，写完不切换 CR3
- 把 image / stack / ipc 三个窗口在 `map_module` 阶段也一起映射好（heap 是按需）

### 5.3 CR3 切换 trampoline

新增 `gos-supervisor::trampoline` 模块：

```rust
pub unsafe fn enter_domain(target: DomainId) -> SavedDomain {
    let saved = read_cr3();                  // 当前 CR3
    let target_pml4 = lookup_domain_root(target);
    write_cr3(target_pml4);                  // mov cr3 — 隐式 TLB flush
    SavedDomain(saved)
}

pub unsafe fn leave_domain(saved: SavedDomain) {
    write_cr3(saved.0);
}
```

约束：

- `enter_domain` 之前必须保证目标 domain 的 root_table_phys 非零
- `enter_domain`/`leave_domain` 必须配对，禁止跨函数边界传递 `SavedDomain`（用 RAII guard 强制：`DomainGuard { _private: () }` 在 drop 中调用 `leave_domain`）
- 中断在 trampoline 内全程开启；进入中断时 CPU 切到 IST stack（在高半区，不依赖当前 CR3）

### 5.4 native dispatch 的 CR3 包装

`gos-runtime` 当前在 `route_signal` / `activate` 中直接调用 `vtable.on_event(&mut ctx, &event)`。修改为：

```rust
let _guard = unsafe { gos_supervisor::trampoline::enter_for_instance(dispatch.instance_id) };
unsafe { on_event(&mut ctx, &event) }
// _guard 在这里 drop，自动 leave_domain
```

`enter_for_instance` 在 supervisor 中实现：根据 instance → module → domain 取 root，切 CR3。`NodeInstanceId::ZERO`（boot 期）保持当前 CR3 不变。

### 5.5 IST stacks & fault handler

- 在 `k-idt` 启动时分配 4 张 16 KiB 内核高半区栈，挂到 TSS 的 IST 槽 1–4
- page fault (#PF)、general protection (#GP)、double fault (#DF)、stack-segment (#SS) handler 全部走 IST
- handler 入口拿到 fault frame 后，读取当前 instance（通过 runtime 的 `current_dispatch_instance`），调用 `gos_supervisor::fault_instance(instance_id)`：内部把 instance 的 module 标记 Faulted，走 B.5 degrade 路径
- handler 返回时 supervisor 决定是否 restart（如果是 RestartAlways 且未达 cap）；否则不返回，直接 longjmp 到 trampoline 之前保存的 SavedDomain，即从 native 调用的 caller 处恢复

### 5.6 ELF loader（独立子目标）

builtin 静态模块不需要 ELF loader。但 Phase B.4 要支持外部模块装载（这是 B.4 的核心价值之一）。最小可行 loader：

- 输入：一段连续字节切片（来自 boot payload / disk / 内嵌资源）
- 解析 ET_DYN ELF header，仅支持 `R_X86_64_RELATIVE` 重定位
- 申请 image 窗口物理页 → map 到 domain 私有低半区 image_base → 拷贝 PT_LOAD segment → 应用重定位
- 解析 dynamic symbol table 找到 `module_init` / `module_event` / `module_stop` 的入口偏移，写入 `ModuleEntry`

第一阶段不要求支持外部 ELF；只要 builtin 路径在 domain CR3 下跑通即可。ELF loader 作为 B.4.6 单独子片。

---

## 六、实施分片

按依赖与风险递增排列：

### B.4.1 — Domain PML4 构造 ✅ 已完成（2026-04-26）

- `k-vmm::create_isolated_address_space` 已实现并被 `Supervisor::map_module → build_domain → create_domain_root` 调用
- 复制 kernel high half (entries 256..512) 到新 PML4
- image / stack / ipc 私有窗口在 map 阶段一次性映射
- 公开观察 API：`gos_supervisor::instance_domain_root(instance_id) -> Option<u64>`
- shell `where` 显示 `domain root_phys=0x...`，root=0 时附 `(UNMAPPED)` 标记
- 回归测试：`map_module_assigns_distinct_non_zero_domain_roots`（host-testing 桩 + kernel-vmm 真实路径都满足）

### B.4.2 — Domain-aware page mapping ✅ 已完成（2026-04-26）

- `k_vmm::map_anonymous_window(root_phys, va, byte_len, flags)` 已实现并被 `request_pages` 通过 `map_domain_heap_pages` 调用
- `create_isolated_address_space` 在 `map_module` 阶段把 image / stack / ipc 三个窗口一次性映射进新 PML4
- builtin 模块的私有窗口当前持有空白页（PT_LOAD segment 没替换）；ELF loader（B.4.6）落地后开始填充实代码 / 数据
- 验证：B.3 的 plugin alloc 仍跑通；shell `where` 的 quota 行无变化（已在 host harness 锁定）

### B.4.3 — IST stack & fault routing ✅ 已完成（2026-04-26）

- `k-gdt` 在 TSS 中分配 IST slot 0/1/2/3 → #DF / #PF / #GP / #SS（每张 16 KiB，slot 0 为旧的 20 KiB 双重故障栈保留）
- `k-idt` 在 `idt_on_init` 为 PF/GP/SS handler 调用 `set_stack_index(...)`，#DF 沿用既有 IST 0
- `gos_trap_normalizer` 区分 CPU fault（vector ∈ {8, 12, 13, 14}）与普通 IRQ：
  - CPU fault：读 `gos_runtime::dispatching_instance()`，调用 `gos_runtime::dispatch_fault(instance_id)`
  - 普通 IRQ：原有 `post_irq_signal` 路径不变
- `gos-runtime` 新增 hook 表 `FaultDispatch { fault: extern "C" fn(NodeInstanceId) }`，公开 `install_fault_dispatch` 与 `dispatch_fault`；避免 runtime → supervisor 反向依赖
- `gos-supervisor::bootstrap` 安装 hook：`instance → module → fault_module(handle)`，复用 B.5 的 ModuleFaultPolicy 路径
- 回归：`fault_dispatch_hook_attributes_cpu_fault_to_module_policy`（host harness）— 直接调用 `dispatch_fault`，断言 `restart_generation` ++（PROVIDER 是 `RestartAlways`）

剩余真实 #PF 验证（构造越界访问的测试 plugin → 观察 `where` 上 DEGRADED）属于真实 boot 验证范畴，可在 B.4.4 trampoline 接入后一并端到端跑。

### B.4.4 — CR3 trampoline & native dispatch wrapping ✅ 已完成（2026-04-26）

- `gos-runtime` 新增 hook 表 `DomainSwitch { enter, leave }`，公开 `install_domain_switch`、`domain_switch_count`
- 内部 `DomainGuard` RAII 包装：`route_signal` 与 `activate` 的 native callback 全部置于 guard 之内，drop 自动 `leave`
- supervisor `bootstrap` 安装真实实现 `trampoline_enter / trampoline_leave`：
  - kernel-vmm feature：读 instance → module → `domain.root_table_phys`，`Cr3::write` 切入；保存的 CR3 编码进 64 位 token 在 leave 中还原
  - host-testing feature：no-op 实现，仅做 enter/leave 平衡
- 短路逻辑：当 target root == live CR3 或 root == 0 时不切，所有 builtin 当前命中此分支（共享 kernel CR3）；ELF 模块落地后自动开始切换
- 回归：`domain_switch_hook_brackets_every_native_dispatch`（runtime harness）— 安装计数 hook，断言每次 route_signal 触发 enter+leave 完美平衡，token round-trip 正确

### B.4.5 — Cross-domain capability invocation ✅ 已完成（2026-04-26）

- 由 B.4.4 的 RAII 包装直接获得：每次 native 调用都进 / 出 trampoline
- 插件 A 的 `kernel_emit_signal` → `route_signal(target=B)` → B 的 `on_event` 被一个 enter(B 的 instance) 包裹；返回时 leave 还原回 A 的 CR3
- 嵌套调用通过 token 链式保存：A→B→C 的 enter 调用产生独立 token，drop 顺序保证 C→B→A 的 leave 正确回退
- 当前所有 builtin 共享 kernel CR3，所以"切换"是 no-op，但 enter/leave 计数仍每次发生（可通过 `domain_switch_count` 观察）
- 完整端到端验证（插件 A 读自己 heap → 调用 B → 回来仍能读 A heap）需要至少两个外部 ELF 模块；待 B.4.6 ELF loader 落地后跑

### B.4.6 — ELF loader（最小切片 ✅ 已完成 2026-04-26；relocation/symbol resolution 后续切片）

**已完成（首切片）：**
- `gos-loader::elf` 模块：纯 no_std ET_DYN ELF64 解析器
- 校验 magic、ELFCLASS64、ELFDATA2LSB、EI_OSABI（SYSV/GNU）、ET_DYN、EM_X86_64、program-header 表完整性
- `ParsedElf::for_each_load_segment` 遍历 PT_LOAD 段并提取 `(virt_addr, mem_len, file_offset, file_len, flags)`
- `highest_virt_end()` 用于 supervisor 在 map_module 阶段计算 image 窗口大小
- 公开 segment-flag 常量 `PF_R / PF_W / PF_X`、program-header type `PT_LOAD / PT_DYNAMIC / PT_GNU_RELRO`
- 6 项 ELF 错误分类（`TooSmall / BadMagic / NotElf64 / NotLittleEndian / NotEtDyn / NotX86_64 / BadProgramHeader / UnsupportedAbi`）
- 回归：`elf_parser_rejects_bad_inputs_and_walks_synthetic_etdyn`（runtime harness）— 4 类错误样本被拒，1 个手工拼装的合法 ET_DYN 通过解析且 PT_LOAD 字段精确

**后续切片（独立 B.4.6.x，不阻塞 Phase B 视为完成）：**
- B.4.6.1 — `R_X86_64_RELATIVE` 重定位 ✅ 已完成（2026-04-26）
  - `gos-loader::elf` 新增 `DynamicTable / parse_dynamic / apply_relative_relocations`
  - 仅识别 `R_X86_64_RELATIVE`（type 8）；其他类型返回 `UnsupportedRelocation(t)` 让 loader 拒绝模块
  - 检查 `r_offset + 8 ≤ image.len()` → `RelocationOutOfBounds`
  - 回归：手工拼装 ELF + PT_DYNAMIC + 一条 RELATIVE relocation，apply 后 8 字节 patch == `image_base + addend`；改为 R_X86_64_64 → 拒绝
- B.4.6.2 — dynamic symbol table 解析，定位 `module_init / module_event / module_stop` 入口偏移
- B.4.6.3 — 把外部 .gosmod 文件从 FAT32（Phase F）装载并启动
- B.4.6.4 — manifest 嵌入 `.gos.manifest` section + 完整性校验

最小切片已经把"格式错误的 payload 在 install 阶段被拒"这条门把住，是后续切片的前置基础。

---

## 七、风险与回退

| 风险 | 影响 | 回退策略 |
|---|---|---|
| IST 栈耗尽 / 嵌套 fault | triple fault 重启 | 在 fault handler 入口立即 disable interrupts + 提示串口；guard 用栈使用率监控 |
| TLB 不一致 | 跨 domain 数据可见性错位 | 单 CPU 下 `mov cr3` 自动 flush；多 CPU 留待 SMP 阶段 |
| ELF relocator bug | 模块装载失败 / 入口跳错 | 先只实现 R_X86_64_RELATIVE；其他类型直接拒绝；保留 builtin 路径作为对照 |
| trampoline 与中断交叠 | 中断处理读到错误 CR3 | 中断走 IST，handler 自己保存/恢复 CR3；保证 handler 不依赖低半区 |
| 启动顺序：CR3 切换发生在 IDT 加载前 | 无法处理 fault | B.4.3 IST 必须在 B.4.4 trampoline 启用之前完成 |

---

## 八、验证计划

| 检查 | 方法 | 通过判据 |
|---|---|---|
| Domain PML4 隔离 | 启动后 shell 显示每个 module 的 root_table_phys 非零且互不相同 | 所有 21 个 builtin 各一份 |
| 跨 domain 写入失败 | 测试 plugin A 试图 write 到 plugin B 的 heap_base | #PF 触发，A 进入 DEGRADED |
| Fault 归因正确 | 手动在 plugin 内 `unsafe { *(0xdead as *mut u8) = 0 }` | shell `where` 上对应模块 restarts++、最终 DEGRADED |
| heap quota 仍工作 | 维持 B.3 的所有验证项 | 无回归 |
| boot-fallback audit | 启动稳定后 `audit: boot-fallback allocs 0` | 不变 |

---

## 九、与 Phase C 的接口

完成 B.4 后，Phase C 的图原生控制面可以建立在以下保证之上：

- 每个图节点都能映射到一个 domain，任何节点级别的资源越权都可被 supervisor 归因
- shell / cypher / AI 这些 control-plane 客户端可以放心从 user space 域调用，因为越权会被硬件挡住
- ELF 装载路径意味着外部 plugin（非 builtin）成为可能，未来支持热重载与签名校验

B.4 不直接交付外部 plugin 装载（只在 B.4.6 探索），但它扫清了"再也没有共享地址空间"这一前置障碍。
