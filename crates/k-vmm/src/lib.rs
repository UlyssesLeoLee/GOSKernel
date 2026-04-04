#![no_std]

use x86_64::VirtAddr;
use x86_64::structures::paging::{
    OffsetPageTable, PageTable, PhysFrame, Mapper, Page, PageTableFlags,
    Size4KiB, FrameAllocator,
};
use x86_64::registers::control::Cr3;
use spin::Mutex;
use gos_protocol::*;
use gos_hal::{vaddr, meta};

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

pub unsafe fn map_page(
    page: Page<Size4KiB>,
    frame: PhysFrame<Size4KiB>,
    flags: PageTableFlags,
) -> Result<(), &'static str> {
    let mut mapper = mapper();
    struct PmmFrameAllocatorEdge;
    unsafe impl FrameAllocator<Size4KiB> for PmmFrameAllocatorEdge {
        fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
            // Unrestricted edge access for Phase 2
            unsafe {
                k_pmm::allocator().lock().allocate_frame()
            }
        }
    }
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
