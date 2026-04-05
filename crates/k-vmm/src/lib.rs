#![no_std]

use core::ptr;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags,
    PhysFrame, Size4KiB,
};
use x86_64::VirtAddr;
use gos_hal::{meta, vaddr};
use gos_protocol::*;

pub const NODE_VEC: VectorAddress = VectorAddress::new(2, 2, 0, 0);

pub fn node_ptr() -> *mut u8 { vaddr::resolve_hal_node(NODE_VEC) }

#[repr(C)]
pub struct VmmState {
    pub phys_offset: u64,
}

pub unsafe fn state() -> &'static VmmState {
    let p = node_ptr();
    if p.is_null() { panic!("K_VMM not initialized"); }
    &*(p.add(1024) as *const VmmState)
}

pub unsafe fn mapper() -> OffsetPageTable<'static> {
    let phys_offset = VirtAddr::new(state().phys_offset);
    let (level_4_frame, _) = Cr3::read();
    let phys = level_4_frame.start_address();
    let virt = phys_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
    OffsetPageTable::new(&mut *page_table_ptr, phys_offset)
}

pub unsafe fn init_node_state(physical_memory_offset: u64) {
    let p = node_ptr();
    meta::burn_node_metadata(p, "MEM", "VMM");
    let state_ptr = p.add(1024) as *mut VmmState;
    core::ptr::write(state_ptr, VmmState { phys_offset: physical_memory_offset });
}

pub struct VmmCell { state: NodeState }

impl VmmCell {
    pub const fn new() -> Self { Self { state: NodeState::Unregistered } }
}

impl NodeCell for VmmCell {
    fn declare(&self) -> CellDeclaration {
        let mut edges = [CellEdge::NONE; MAX_CELL_EDGES];
        edges[0] = CellEdge::new("MAP",   0x01, 0);
        edges[1] = CellEdge::new("UNMAP", 0x01, 0);
        CellDeclaration {
            vec: NODE_VEC, domain_label: "MEM", name: "VMM",
            edges, edge_count: 2, depends_on: &[],
        }
    }

    unsafe fn init(&mut self) { self.state = NodeState::Ready; }

    fn on_activate(&mut self) -> CellResult { CellResult::Done }
    fn on_signal(&mut self, _: Signal) -> CellResult { CellResult::Done }
    fn on_suspend(&mut self) { self.state = NodeState::Suspended; }
    fn state(&self) -> NodeState { self.state }
    fn vec(&self) -> VectorAddress { NODE_VEC }
}

pub static VMM_CELL: spin::Mutex<VmmCell> = spin::Mutex::new(VmmCell::new());

impl PluginEntry for VmmCell {
    const VEC: VectorAddress = NODE_VEC;
    const WAVEFRONT: u32 = 6; // Depends on PMM

    fn plugin_main(ctx: &mut BootContext) {
        unsafe {
            let boot_info_ptr = ctx.payload as *const bootloader::BootInfo;
            let offset: Option<u64> = (*boot_info_ptr).physical_memory_offset.into();
            if let Some(o) = offset {
                init_node_state(o);
            }
        }
        gos_hal::ngr::try_mount_cell(Self::VEC, &VMM_CELL);
    }
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
    let page_count = ((byte_len + Size4KiB::SIZE - 1) / Size4KiB::SIZE) as usize;

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

pub unsafe fn unmap_page(page: Page<Size4KiB>) -> Result<(), &'static str> {
    let mut mapper = mapper();
    mapper.unmap(page).map_err(|_| "unmap failed")?.1.flush();
    Ok(())
}
