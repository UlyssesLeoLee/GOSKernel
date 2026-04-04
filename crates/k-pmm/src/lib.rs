#![no_std]

use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};
use x86_64::PhysAddr;
use spin::Mutex;
use gos_protocol::*;
use gos_hal::{vaddr, meta};

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 11, 0, 0);

pub fn node_ptr() -> *mut u8 { vaddr::resolve_hal_node(NODE_VEC) }

pub struct BootInfoFrameAllocator {
    pub memory_map: &'static MemoryMap,
    pub next: usize,
}

impl BootInfoFrameAllocator {
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| r.region_type == MemoryRegionType::Usable);
        let addr_ranges = usable_regions.map(|r| r.range.start_addr()..r.range.end_addr());
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

pub fn allocator() -> &'static Mutex<BootInfoFrameAllocator> {
    unsafe {
        let p = node_ptr();
        if p.is_null() { panic!("K_PMM Matrix not initialized"); }
        &*(p.add(1024) as *mut Mutex<BootInfoFrameAllocator>)
    }
}

pub unsafe fn init_node_state(boot_info_payload: u64) {
    let p = node_ptr();
    meta::burn_node_metadata(p, "SYS", "PMM");

    let boot_info_ptr = boot_info_payload as *const bootloader::BootInfo;
    let memory_map = &(*boot_info_ptr).memory_map;

    let alloc = BootInfoFrameAllocator::init(memory_map);

    let state_ptr = p.add(1024) as *mut Mutex<BootInfoFrameAllocator>;
    core::ptr::write(state_ptr, Mutex::new(alloc));
}

pub struct PmmCell { state: NodeState }

impl PmmCell {
    pub const fn new() -> Self { Self { state: NodeState::Unregistered } }
}

impl NodeCell for PmmCell {
    fn declare(&self) -> CellDeclaration {
        let mut edges = [CellEdge::NONE; MAX_CELL_EDGES];
        edges[0] = CellEdge::new("ALLOC", 0x01, 0);
        edges[1] = CellEdge::new("FREE", 0x01, 0);
        CellDeclaration {
            vec: NODE_VEC, domain_label: "SYS", name: "PMM",
            edges, edge_count: 2, depends_on: &[],
        }
    }

    unsafe fn init(&mut self) { 
        // Real init happens in plugin_main
        self.state = NodeState::Ready; 
    }

    fn on_activate(&mut self) -> CellResult { CellResult::Done }
    fn on_signal(&mut self, _: Signal) -> CellResult { CellResult::Done }
    fn on_suspend(&mut self) { self.state = NodeState::Suspended; }
    fn state(&self) -> NodeState { self.state }
    fn vec(&self) -> VectorAddress { NODE_VEC }
}

pub static PMM_CELL: spin::Mutex<PmmCell> = spin::Mutex::new(PmmCell::new());

impl PluginEntry for PmmCell {
    const VEC: VectorAddress = NODE_VEC;
    const WAVEFRONT: u32 = 5;

    fn plugin_main(ctx: &mut BootContext) {
        unsafe { init_node_state(ctx.payload); }
        gos_hal::ngr::try_mount_cell(Self::VEC, &PMM_CELL);
    }
}
