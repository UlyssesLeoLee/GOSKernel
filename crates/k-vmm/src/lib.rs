#![no_std]

mod pre;
mod proc;
mod post;

// ============================================================
// GOS KERNEL TOPOLOGY — k-vmm
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_VMM", name: "k-vmm"})
// SET p.executor = "k_vmm::EXECUTOR_ID", p.node_type = "Service", p.state_schema = "0x200B"
//
// -- Dependencies
// MERGE (dep_K_PMM:Plugin {id: "K_PMM"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_PMM)
//
// -- Exported Capabilities (APIs)
// MERGE (cap_memory_map_page:Capability {namespace: "memory", name: "map_page"})
// MERGE (p)-[:EXPORTS]->(cap_memory_map_page)
// ============================================================


use core::ptr;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags,
    PhysFrame, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};
use gos_hal::{meta, vaddr};
use gos_protocol::*;

pub const NODE_VEC: VectorAddress = VectorAddress::new(2, 2, 0, 0);

pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.vmm");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(vmm_on_init),
    on_event: Some(vmm_on_event),
    on_suspend: Some(vmm_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

static mut BOOT_INFO_PTR: u64 = 0;

pub fn node_ptr() -> *mut u8 { vaddr::resolve_hal_node(NODE_VEC) }

#[repr(C)]
pub struct VmmState {
    pub phys_offset: u64,
}

/// # Safety
/// K_VMM node must have been initialised via `vmm_on_init` before calling.
pub unsafe fn state() -> &'static VmmState {
    let p = node_ptr();
    if p.is_null() { panic!("K_VMM not initialized"); }
    &*(p.add(1024) as *const VmmState)
}

/// # Safety
/// K_VMM must be initialised and `phys_offset` must match the identity
/// offset established at boot; CR3 must contain the kernel's PML4.
pub unsafe fn mapper() -> OffsetPageTable<'static> {
    let phys_offset = VirtAddr::new(state().phys_offset);
    let (level_4_frame, _) = Cr3::read();
    let phys = level_4_frame.start_address();
    let virt = phys_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
    OffsetPageTable::new(&mut *page_table_ptr, phys_offset)
}

pub fn register_hook(ctx: &mut BootContext) {
    unsafe { BOOT_INFO_PTR = ctx.payload; }
}

unsafe extern "C" fn vmm_on_init(_ctx: *mut ExecutorContext) -> ExecStatus {
    let p = node_ptr();
    meta::burn_node_metadata(p, "MEM", "VMM");
    
    let boot_info_payload = BOOT_INFO_PTR;
    if boot_info_payload != 0 {
        let boot_info_ptr = boot_info_payload as *const bootloader::BootInfo;
        let offset: Option<u64> = (*boot_info_ptr).physical_memory_offset.into();
        if let Some(o) = offset {
            let state_ptr = p.add(1024) as *mut VmmState;
            core::ptr::write(state_ptr, VmmState { phys_offset: o });
        }
    }
    
    ExecStatus::Done
}

unsafe extern "C" fn vmm_on_event(_ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    let Some(input) = pre::prepare(event) else { return ExecStatus::Done; };
    let Some(output) = proc::process(input) else { return ExecStatus::Done; };
    post::emit(output)
}

unsafe extern "C" fn vmm_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

struct PmmFrameAllocatorEdge;

unsafe impl FrameAllocator<Size4KiB> for PmmFrameAllocatorEdge {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        k_pmm::allocator().lock().allocate_frame()
    }
}

unsafe fn mapper_for_root(root_table_phys: u64) -> OffsetPageTable<'static> {
    let phys_offset = VirtAddr::new(state().phys_offset);
    let virt = phys_offset + root_table_phys;
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
    OffsetPageTable::new(&mut *page_table_ptr, phys_offset)
}

unsafe fn page_table_for_phys(root_table_phys: u64) -> &'static mut PageTable {
    let phys_offset = VirtAddr::new(state().phys_offset);
    let virt = phys_offset + root_table_phys;
    &mut *(virt.as_mut_ptr::<PageTable>())
}

unsafe fn frame_to_ptr(frame: PhysFrame<Size4KiB>) -> *mut u8 {
    let phys_offset = VirtAddr::new(state().phys_offset);
    let virt = phys_offset + frame.start_address().as_u64();
    virt.as_mut_ptr()
}

unsafe fn allocate_zeroed_frame() -> Result<PhysFrame<Size4KiB>, &'static str> {
    let frame = k_pmm::allocator()
        .lock()
        .allocate_frame()
        .ok_or("out of physical frames")?;
    ptr::write_bytes(frame_to_ptr(frame), 0, Size4KiB::SIZE as usize);
    Ok(frame)
}

/// # Safety
/// All base/len pairs must be page-aligned; the PMM must have free frames;
/// the caller must eventually call `destroy_isolated_address_space` to
/// free the returned root frame.
#[allow(clippy::too_many_arguments)]
pub unsafe fn create_isolated_address_space(
    image_base: u64,
    image_len: u64,
    stack_base: u64,
    stack_len: u64,
    ipc_base: u64,
    ipc_len: u64,
) -> Result<u64, &'static str> {
    let frame = allocate_zeroed_frame()?;
    let new_root = page_table_for_phys(frame.start_address().as_u64());

    let active_root = {
        let (active_frame, _) = Cr3::read();
        page_table_for_phys(active_frame.start_address().as_u64())
    };

    for idx in 256..512 {
        new_root[idx] = active_root[idx].clone();
    }

    map_anonymous_window(
        frame.start_address().as_u64(),
        image_base,
        image_len,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
    )?;
    map_anonymous_window(
        frame.start_address().as_u64(),
        stack_base,
        stack_len,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
    )?;
    map_anonymous_window(
        frame.start_address().as_u64(),
        ipc_base,
        ipc_len,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
    )?;

    Ok(frame.start_address().as_u64())
}

/// # Safety
/// `root_table_phys` must point to a valid 4 KiB-aligned page table frame;
/// `virt_base` and `byte_len` must be page-aligned.
pub unsafe fn map_anonymous_window(
    root_table_phys: u64,
    virt_base: u64,
    byte_len: u64,
    flags: PageTableFlags,
) -> Result<(), &'static str> {
    if byte_len == 0 {
        return Ok(());
    }

    let mut mapper = mapper_for_root(root_table_phys);
    let mut allocator = PmmFrameAllocatorEdge;
    let page_count = byte_len.div_ceil(Size4KiB::SIZE) as usize;

    for page_idx in 0..page_count {
        let page = Page::containing_address(VirtAddr::new(
            virt_base + (page_idx as u64 * Size4KiB::SIZE),
        ));
        let frame = allocate_zeroed_frame()?;
        mapper
            .map_to(page, frame, flags, &mut allocator)
            .map_err(|_| "map_to failed")?
            .ignore();
    }

    Ok(())
}

/// Counterpart to `create_isolated_address_space`: unmaps every page in
/// the image/stack/ipc/heap windows (freeing their physical frames back
/// to the PMM) and then frees the domain's root page-table frame itself.
///
/// Used by the supervisor to roll an isolated domain back to nothing when
/// a module's bring-up pipeline fails partway through, so a faulting
/// plugin never leaks physical memory.  `heap_len` should be the number
/// of bytes actually mapped via `map_anonymous_window` (the heap window
/// is demand-mapped, unlike the other three), not the full reserved
/// window — pages never mapped are silently skipped either way.
///
/// # Safety
/// `root_table_phys` must be the frame returned by `create_isolated_address_space`.
/// All base/len arguments must match what was originally mapped.  The domain
/// must not be the active CR3 when this is called.
#[allow(clippy::too_many_arguments)]
pub unsafe fn destroy_isolated_address_space(
    root_table_phys: u64,
    image_base: u64,
    image_len: u64,
    stack_base: u64,
    stack_len: u64,
    ipc_base: u64,
    ipc_len: u64,
    heap_base: u64,
    heap_len: u64,
) -> Result<(), &'static str> {
    unmap_window(root_table_phys, image_base, image_len)?;
    unmap_window(root_table_phys, stack_base, stack_len)?;
    unmap_window(root_table_phys, ipc_base, ipc_len)?;
    unmap_window(root_table_phys, heap_base, heap_len)?;
    let root_frame = PhysFrame::from_start_address(PhysAddr::new(root_table_phys))
        .map_err(|_| "root table phys not frame-aligned")?;
    deallocate_frame(root_frame);
    Ok(())
}

/// Unmaps every page in `[virt_base, virt_base + byte_len)` within the
/// page table rooted at `root_table_phys`, returning each backing frame
/// to the PMM.  Pages that were never mapped (e.g. the unused tail of a
/// demand-mapped heap window) are skipped rather than treated as errors.
///
/// # Safety
/// `root_table_phys` must be a valid, frame-aligned page table root.
unsafe fn unmap_window(root_table_phys: u64, virt_base: u64, byte_len: u64) -> Result<(), &'static str> {
    if byte_len == 0 {
        return Ok(());
    }

    let mut mapper = mapper_for_root(root_table_phys);
    let page_count = byte_len.div_ceil(Size4KiB::SIZE) as usize;

    for page_idx in 0..page_count {
        let page = Page::containing_address(VirtAddr::new(
            virt_base + (page_idx as u64 * Size4KiB::SIZE),
        ));
        if let Ok((frame, flush)) = mapper.unmap(page) {
            flush.ignore();
            k_pmm::allocator().lock().dealloc_frame(frame);
        }
    }

    Ok(())
}

/// # Safety
/// `page` must not already be mapped; `frame` must be a freshly allocated
/// physical frame not aliased elsewhere.
pub unsafe fn map_page(
    page: Page<Size4KiB>,
    frame: PhysFrame<Size4KiB>,
    flags: PageTableFlags,
) -> Result<(), &'static str> {
    let mut mapper = mapper();
    let mut pmm_allocator = PmmFrameAllocatorEdge;
    mapper
        .map_to(page, frame, flags, &mut pmm_allocator)
        .map_err(|_| "map_to failed")?
        .flush();
    Ok(())
}

/// # Safety
/// `page` must be currently mapped in the active address space; after
/// unmapping the page is no longer accessible via the virtual address.
pub unsafe fn unmap_page(page: Page<Size4KiB>) -> Result<(), &'static str> {
    let mut mapper = mapper();
    let (frame, flush) = mapper.unmap(page).map_err(|_| "unmap failed")?;
    flush.flush();
    // Return the physical frame to the bitmap allocator.
    k_pmm::allocator().lock().dealloc_frame(frame);
    Ok(())
}

/// Return a physical frame to the PMM without unmapping.
/// Useful when the caller manages its own page table entries.
///
/// # Safety
/// `frame` must not be referenced by any live page table entry after this
/// call; double-freeing the same frame causes allocator corruption.
pub unsafe fn deallocate_frame(frame: PhysFrame<Size4KiB>) {
    k_pmm::allocator().lock().dealloc_frame(frame);
}

// ── Plugin Descriptor ────────────────────────────────────────────────────────

const VMM_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PhysMap, arg0: u64::MAX, arg1: u64::MAX },
    PermissionSpec { kind: PermissionKind::GraphWrite, arg0: 0, arg1: 0 },
];
const VMM_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "memory", name: "map_page", version: 1 },
];

pub const PLUGIN_DESCRIPTOR: BuiltinPluginDescriptor = BuiltinPluginDescriptor {
    manifest: PluginManifest {
        abi_version: GOS_ABI_VERSION,
        plugin_id: PluginId::from_ascii("K_VMM"),
        name: "K_VMM",
        version: 1,
        depends_on: &[PluginId::from_ascii("K_PMM")],
        permissions: VMM_PERMS,
        exports: VMM_EXPORTS,
        imports: &[],
        nodes: &[NodeSpec {
            node_id: derive_node_id(PluginId::from_ascii("K_VMM"), "vmm.entry"),
            local_node_key: "vmm.entry",
            node_type: RuntimeNodeType::Service,
            entry_policy: EntryPolicy::Bootstrap,
            executor_id: EXECUTOR_ID,
            state_schema_hash: 0x200B,
            permissions: VMM_PERMS,
            exports: VMM_EXPORTS,
            vector_ref: None,
        }],
        edges: &[],
        signature: None,
        policy_hash: [0; 16],
    },
    granted_permissions: VMM_PERMS,
    nodes: &[NativeNodeBinding {
        vector: NODE_VEC,
        local_node_key: "vmm.entry",
        executor: EXECUTOR_VTABLE,
    }],
    register_hook: Some(register_hook),
};
