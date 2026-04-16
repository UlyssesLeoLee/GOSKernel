#![no_std]


// ==============================================================
// GOS KERNEL TOPOLOGY — k-heap (native.heap)
// 以下 Cypher 脚本可直接导入 Neo4j，与其他模块共同还原内核完整图谱。
//
// MERGE (p:Plugin {id: "K_HEAP", name: "k-heap"})
// SET p.executor = "native.heap", p.node_type = "Service", p.state_schema = "0x200C"
//
// // ── 启动依赖 (DEPENDS_ON) ──────────────────────────────────
// MERGE (k_pmm:Plugin {id: "K_PMM"})
// MERGE (p)-[:DEPENDS_ON {required: true}]->(k_pmm)
// MERGE (k_vmm:Plugin {id: "K_VMM"})
// MERGE (p)-[:DEPENDS_ON {required: true}]->(k_vmm)
//
// // ── 能力导出 (EXPORTS Capability) ────────────────────────
// MERGE (cap_memory_alloc:Capability {namespace: "memory", name: "alloc"})
// MERGE (p)-[:EXPORTS]->(cap_memory_alloc)
// ==============================================================

extern crate alloc;

use x86_64::structures::paging::{Page, PageTableFlags, FrameAllocator};
use x86_64::VirtAddr;
use linked_list_allocator::LockedHeap;
use gos_protocol::*;
use gos_hal::{vaddr, meta};

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 200 * 1024; // 200 KiB

pub const NODE_VEC: VectorAddress = VectorAddress::new(2, 3, 0, 0);

pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.heap");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(heap_on_init),
    on_event: Some(heap_on_event),
    on_suspend: Some(heap_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

pub fn node_ptr() -> *mut u8 { vaddr::resolve_hal_node(NODE_VEC) }

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

unsafe extern "C" fn heap_on_init(_ctx: *mut ExecutorContext) -> ExecStatus {
    let p = node_ptr();
    meta::burn_node_metadata(p, "MEM", "HEAP");
    
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
    
    ExecStatus::Done
}

unsafe extern "C" fn heap_on_event(_ctx: *mut ExecutorContext, _event: *const NodeEvent) -> ExecStatus {
    ExecStatus::Done
}

unsafe extern "C" fn heap_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}
