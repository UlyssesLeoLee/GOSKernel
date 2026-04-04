#![no_std]

extern crate alloc;

use spin::Mutex;
use x86_64::structures::paging::{Page, PageTableFlags, Size4KiB, FrameAllocator};
use x86_64::VirtAddr;
use linked_list_allocator::LockedHeap;
use gos_protocol::*;
use gos_hal::{vaddr, meta};

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 200 * 1024; // 200 KiB

pub const NODE_VEC: VectorAddress = VectorAddress::new(2, 3, 0, 0);

pub fn node_ptr() -> *mut u8 { vaddr::resolve_hal_node(NODE_VEC) }

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub unsafe fn init_node_state() {
    let p = node_ptr();
    meta::burn_node_metadata(p, "MEM", "HEAP");
}

pub struct HeapCell { state: NodeState }

impl HeapCell {
    pub const fn new() -> Self { Self { state: NodeState::Unregistered } }
}

impl NodeCell for HeapCell {
    fn declare(&self) -> CellDeclaration {
        let mut edges = [CellEdge::NONE; MAX_CELL_EDGES];
        edges[0] = CellEdge::new("MALLOC", 0x01, 0);
        edges[1] = CellEdge::new("FREE", 0x01, 0);
        CellDeclaration {
            vec: NODE_VEC, domain_label: "MEM", name: "HEAP",
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

pub static HEAP_CELL: spin::Mutex<HeapCell> = spin::Mutex::new(HeapCell::new());

impl PluginEntry for HeapCell {
    const VEC: VectorAddress = NODE_VEC;
    const WAVEFRONT: u32 = 7; // Depends on VMM

    fn plugin_main(_ctx: &mut BootContext) {
        unsafe {
            init_node_state();
            
            // Allocate heap pages via VMM and PMM
            let heap_start = VirtAddr::new(HEAP_START as u64);
            let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
            let heap_start_page = Page::containing_address(heap_start);
            let heap_end_page = Page::containing_address(heap_end);

            for page in Page::range_inclusive(heap_start_page, heap_end_page) {
                let frame = k_pmm::allocator().lock().allocate_frame()
                    .expect("Heap memory exhausted (PMM allocate_frame failed)");
                let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
                k_vmm::map_page(page, frame, flags).expect("Heap mapping failed (VMM map_page failed)");
            }

            ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
        }
        
        gos_hal::ngr::try_mount_cell(Self::VEC, &HEAP_CELL);
    }
}
