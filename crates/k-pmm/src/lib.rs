#![no_std]

use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};
use x86_64::PhysAddr;
use spin::Mutex;
use gos_protocol::*;
use gos_hal::{vaddr, meta};

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 11, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.pmm");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(pmm_on_init),
    on_event: Some(pmm_on_event),
    on_suspend: Some(pmm_on_suspend),
    on_resume: None,
    on_teardown: None,
};

static mut BOOT_INFO_PTR: u64 = 0;

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

pub fn register_hook(ctx: &mut BootContext) {
    unsafe { BOOT_INFO_PTR = ctx.payload; }
}

unsafe extern "C" fn pmm_on_init(_ctx: *mut ExecutorContext) -> ExecStatus {
    unsafe {
        let p = node_ptr();
        meta::burn_node_metadata(p, "SYS", "PMM");

        let boot_info_payload = BOOT_INFO_PTR;
        let boot_info_ptr = boot_info_payload as *const bootloader::BootInfo;
        let memory_map = &(*boot_info_ptr).memory_map;

        let alloc = BootInfoFrameAllocator::init(memory_map);

        let state_ptr = p.add(1024) as *mut Mutex<BootInfoFrameAllocator>;
        core::ptr::write(state_ptr, Mutex::new(alloc));
    }
    ExecStatus::Done
}

unsafe extern "C" fn pmm_on_event(_ctx: *mut ExecutorContext, _event: *const NodeEvent) -> ExecStatus {
    ExecStatus::Done
}

unsafe extern "C" fn pmm_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}
