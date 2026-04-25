#![no_std]

mod pre;
mod proc;
mod post;

// ============================================================
// GOS KERNEL TOPOLOGY — k-heap
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_HEAP", name: "k-heap"})
// SET p.executor = "k_heap::EXECUTOR_ID", p.node_type = "Service", p.state_schema = "0x200C"
//
// -- Dependencies
// MERGE (dep_K_PMM:Plugin {id: "K_PMM"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_PMM)
// MERGE (dep_K_VMM:Plugin {id: "K_VMM"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_VMM)
//
// -- Exported Capabilities (APIs)
// MERGE (cap_memory_alloc:Capability {namespace: "memory", name: "alloc"})
// MERGE (p)-[:EXPORTS]->(cap_memory_alloc)
// ============================================================


extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use x86_64::structures::paging::{Page, PageTableFlags, FrameAllocator};
use x86_64::VirtAddr;
use linked_list_allocator::LockedHeap;
use gos_protocol::*;
use gos_hal::{vaddr, meta};

const PAGE_SIZE: usize = 4096;

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 1024 * 1024; // 1 MiB

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

    // Install ourselves as the runtime's heap backend so plugin
    // ctx.abi.alloc_pages calls land here (after supervisor quota
    // accounting in gos-runtime).
    gos_runtime::install_heap_backend(gos_runtime::HeapBackend {
        alloc: heap_backend_alloc,
        free: heap_backend_free,
    });

    ExecStatus::Done
}

unsafe extern "C" fn heap_backend_alloc(page_count: usize) -> *mut u8 {
    if page_count == 0 {
        return core::ptr::null_mut();
    }
    let Ok(layout) = Layout::from_size_align(page_count * PAGE_SIZE, PAGE_SIZE) else {
        return core::ptr::null_mut();
    };
    unsafe { ALLOCATOR.alloc(layout) }
}

unsafe extern "C" fn heap_backend_free(ptr: *mut u8, page_count: usize) {
    if ptr.is_null() || page_count == 0 {
        return;
    }
    let Ok(layout) = Layout::from_size_align(page_count * PAGE_SIZE, PAGE_SIZE) else {
        return;
    };
    unsafe { ALLOCATOR.dealloc(ptr, layout) };
}

unsafe extern "C" fn heap_on_event(_ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    // pre::prepare always returns None — heap has no runtime event processing.
    let Some(input) = pre::prepare(event) else { return ExecStatus::Done; };
    let Some(output) = proc::process(input) else { return ExecStatus::Done; };
    post::emit(output)
}

unsafe extern "C" fn heap_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

// ── Plugin Descriptor ────────────────────────────────────────────────────────

const HEAP_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PhysMap, arg0: u64::MAX, arg1: u64::MAX },
    PermissionSpec { kind: PermissionKind::GraphWrite, arg0: 0, arg1: 0 },
];
const HEAP_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "memory", name: "alloc" },
];

pub const PLUGIN_DESCRIPTOR: BuiltinPluginDescriptor = BuiltinPluginDescriptor {
    manifest: PluginManifest {
        abi_version: GOS_ABI_VERSION,
        plugin_id: PluginId::from_ascii("K_HEAP"),
        name: "K_HEAP",
        version: 1,
        depends_on: &[PluginId::from_ascii("K_PMM"), PluginId::from_ascii("K_VMM")],
        permissions: HEAP_PERMS,
        exports: HEAP_EXPORTS,
        imports: &[],
        nodes: &[NodeSpec {
            node_id: derive_node_id(PluginId::from_ascii("K_HEAP"), "heap.entry"),
            local_node_key: "heap.entry",
            node_type: RuntimeNodeType::Service,
            entry_policy: EntryPolicy::Bootstrap,
            executor_id: EXECUTOR_ID,
            state_schema_hash: 0x200C,
            permissions: HEAP_PERMS,
            exports: HEAP_EXPORTS,
            vector_ref: None,
        }],
        edges: &[],
        signature: None,
        policy_hash: [0; 16],
    },
    granted_permissions: HEAP_PERMS,
    nodes: &[NativeNodeBinding {
        vector: NODE_VEC,
        local_node_key: "heap.entry",
        executor: EXECUTOR_VTABLE,
    }],
    register_hook: None,
};
