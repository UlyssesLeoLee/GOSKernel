#![no_std]

//! GOS Physical Memory Manager — Two-Level Bitmap Frame Allocator
//!
//! Every physical 4 KiB frame maps to a single bit in a static bitmap.
//! A second "summary" level tracks which L0 words still have capacity,
//! giving O(1) amortised alloc and O(1) deterministic free.
//!
//! RAM ceiling is compile-time configurable via Cargo features:
//!   default  → 1 GiB  (bitmap ≈ 32 KiB)
//!   max-ram-2g → 2 GiB  (bitmap ≈ 64 KiB)
//!   max-ram-4g → 4 GiB  (bitmap ≈ 128 KiB)
//!
//! The allocator lives as a graph-native plugin node at vector [1.11.0.0].

mod pre;
mod proc;
mod post;

// ============================================================
// GOS KERNEL TOPOLOGY — k-pmm
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_PMM", name: "k-pmm"})
// SET p.executor = "k_pmm::EXECUTOR_ID", p.node_type = "Service", p.state_schema = "0x200A"
//
// -- Exported Capabilities (APIs)
// MERGE (cap_memory_frame_alloc:Capability {namespace: "memory", name: "frame_alloc"})
// MERGE (p)-[:EXPORTS]->(cap_memory_frame_alloc)
// ============================================================

use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use gos_hal::{meta, vaddr};
use gos_protocol::*;
use spin::Mutex;
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};
use x86_64::PhysAddr;

// ---------------------------------------------------------------------------
// Compile-time RAM ceiling selection
// ---------------------------------------------------------------------------

#[cfg(feature = "max-ram-4g")]
const MAX_PHYS_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[cfg(all(feature = "max-ram-2g", not(feature = "max-ram-4g")))]
const MAX_PHYS_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[cfg(not(any(feature = "max-ram-2g", feature = "max-ram-4g")))]
const MAX_PHYS_BYTES: u64 = 1024 * 1024 * 1024; // 1 GiB default

const PAGE_SIZE: u64 = 4096;
const MAX_PHYS_FRAMES: usize = (MAX_PHYS_BYTES / PAGE_SIZE) as usize;
const BITMAP_WORDS: usize = MAX_PHYS_FRAMES / 64;
const SUMMARY_WORDS: usize = BITMAP_WORDS / 64;

// ---------------------------------------------------------------------------
// Static storage — bitmap starts all-ones (every frame marked USED).
// bit = 1 → used/reserved, bit = 0 → free.
// ---------------------------------------------------------------------------

static mut BITMAP: [u64; BITMAP_WORDS] = [u64::MAX; BITMAP_WORDS];
static mut SUMMARY: [u64; SUMMARY_WORDS] = [u64::MAX; SUMMARY_WORDS];

// ---------------------------------------------------------------------------
// Graph-native plugin identity
// ---------------------------------------------------------------------------

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 11, 0, 0);

pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.pmm");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(pmm_on_init),
    on_event: Some(pmm_on_event),
    on_suspend: Some(pmm_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: Some(pmm_telemetry),
};

/// PMM telemetry constants — Signal::Control cmd values for querying stats.
pub const PMM_CONTROL_QUERY_STATS: u8 = 0x10;

/// Telemetry callback — reports frame allocator state to the graph.
#[allow(improper_ctypes_definitions)]
unsafe extern "C" fn pmm_telemetry() -> NodeTelemetry {
    let p = node_ptr();
    if p.is_null() {
        return NodeTelemetry::EMPTY;
    }
    let lock = &*(p.add(1024) as *const Mutex<BitmapFrameAllocator>);
    let alloc = lock.lock();

    let mut t = NodeTelemetry::EMPTY;
    t.count = 5;
    t.entries[0] = TelemetryEntry { key: "total",    value: alloc.total_frames() as u64,                       unit: TelemetryUnit::Count };
    t.entries[1] = TelemetryEntry { key: "used",     value: alloc.used_frames() as u64,                        unit: TelemetryUnit::Count };
    t.entries[2] = TelemetryEntry { key: "free",     value: alloc.free_frames() as u64,                        unit: TelemetryUnit::Count };
    t.entries[3] = TelemetryEntry { key: "ceiling",  value: alloc.ceiling_bytes() / (1024 * 1024),              unit: TelemetryUnit::MiB };
    t.entries[4] = TelemetryEntry { key: "max",      value: alloc.max_supported_bytes() / (1024 * 1024),        unit: TelemetryUnit::MiB };
    t
}

static mut BOOT_INFO_PTR: u64 = 0;

pub fn node_ptr() -> *mut u8 {
    vaddr::resolve_hal_node(NODE_VEC)
}

// ---------------------------------------------------------------------------
// BitmapFrameAllocator
// ---------------------------------------------------------------------------

pub struct BitmapFrameAllocator {
    /// Total frames tracked (limited by both physical RAM and MAX_PHYS_FRAMES).
    total_frames: usize,
    /// Number of currently allocated (used) frames.
    used_frames: usize,
    /// Current ceiling in bytes (configurable at runtime, capped by static storage).
    ceiling_bytes: u64,
}

impl BitmapFrameAllocator {
    /// Bootstrap the allocator from the bootloader memory map.
    ///
    /// All frames start as USED. Only `MemoryRegionType::Usable` regions are
    /// freed into the bitmap.
    ///
    /// # Safety
    /// Must be called exactly once, before any allocation. Accesses
    /// `static mut BITMAP` and `static mut SUMMARY`.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        // Determine the highest physical address reported by the bootloader.
        let mut max_addr: u64 = 0;
        for region in memory_map.iter() {
            let end = region.range.end_addr();
            if end > max_addr {
                max_addr = end;
            }
        }

        // Cap at our static storage limit.
        let ceiling = MAX_PHYS_BYTES;
        let capped_addr = max_addr.min(ceiling);
        let total_frames = (capped_addr / PAGE_SIZE) as usize;
        let mut used_frames = total_frames;

        // Free frames that belong to usable regions.
        for region in memory_map.iter() {
            if region.region_type != MemoryRegionType::Usable {
                continue;
            }

            let start_frame = (region.range.start_addr() / PAGE_SIZE) as usize;
            let end_frame = ((region.range.end_addr() / PAGE_SIZE) as usize).min(total_frames);

            for frame_idx in start_frame..end_frame {
                if frame_idx >= MAX_PHYS_FRAMES {
                    break;
                }
                let word_idx = frame_idx / 64;
                let bit_idx = frame_idx % 64;
                let mask = 1u64 << bit_idx;

                if BITMAP[word_idx] & mask != 0 {
                    BITMAP[word_idx] &= !mask;
                    used_frames -= 1;
                }
            }
        }

        // Rebuild L1 summary: bit = 1 when corresponding L0 word is all-ones.
        Self::rebuild_summary(total_frames);

        BitmapFrameAllocator {
            total_frames,
            used_frames,
            ceiling_bytes: ceiling,
        }
    }

    /// Rebuild the L1 summary bitmap from current L0 state.
    unsafe fn rebuild_summary(total_frames: usize) {
        let active_words = (total_frames + 63) / 64;

        for summary_idx in 0..SUMMARY_WORDS {
            let mut word: u64 = 0;
            let base = summary_idx * 64;

            for bit in 0..64 {
                let word_idx = base + bit;
                if word_idx < active_words {
                    if BITMAP[word_idx] == u64::MAX {
                        word |= 1u64 << bit;
                    }
                } else {
                    // Beyond tracked range — mark as full so allocator skips.
                    word |= 1u64 << bit;
                }
            }

            SUMMARY[summary_idx] = word;
        }
    }

    // -- Public query API --------------------------------------------------

    pub fn total_frames(&self) -> usize {
        self.total_frames
    }

    pub fn used_frames(&self) -> usize {
        self.used_frames
    }

    pub fn free_frames(&self) -> usize {
        self.total_frames.saturating_sub(self.used_frames)
    }

    pub fn ceiling_bytes(&self) -> u64 {
        self.ceiling_bytes
    }

    pub fn max_supported_bytes(&self) -> u64 {
        MAX_PHYS_BYTES
    }

    // -- Runtime ceiling adjustment ----------------------------------------

    /// Shrink the tracked ceiling at runtime. Cannot exceed the static
    /// storage limit (`MAX_PHYS_BYTES`). Frames beyond the new ceiling are
    /// marked as used and become unreachable.
    ///
    /// # Safety
    /// Caller must hold the allocator lock (this is called through `&mut self`).
    pub fn set_ceiling(&mut self, new_ceiling_bytes: u64) {
        let capped = new_ceiling_bytes.min(MAX_PHYS_BYTES);
        let new_total = (capped / PAGE_SIZE) as usize;

        if new_total >= self.total_frames {
            // Growing beyond what the bootloader reported is a no-op.
            self.ceiling_bytes = capped;
            return;
        }

        // Shrinking: mark frames beyond new_total as used.
        unsafe {
            for frame_idx in new_total..self.total_frames {
                if frame_idx >= MAX_PHYS_FRAMES {
                    break;
                }
                let word_idx = frame_idx / 64;
                let bit_idx = frame_idx % 64;
                let mask = 1u64 << bit_idx;

                if BITMAP[word_idx] & mask == 0 {
                    // Was free, now mark used.
                    BITMAP[word_idx] |= mask;
                    self.used_frames += 1;
                }
            }

            Self::rebuild_summary(new_total);
        }

        self.total_frames = new_total;
        self.ceiling_bytes = capped;
    }

    // -- Core alloc / dealloc ----------------------------------------------

    /// Allocate a single 4 KiB physical frame. O(1) amortised.
    pub fn alloc_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        unsafe {
            for summary_idx in 0..SUMMARY_WORDS {
                let summary_word = SUMMARY[summary_idx];
                if summary_word == u64::MAX {
                    // All 64 L0 words in this group are full.
                    continue;
                }

                // First non-full L0 word in this group.
                let summary_bit = (!summary_word).trailing_zeros() as usize;
                let word_idx = summary_idx * 64 + summary_bit;

                if word_idx >= BITMAP_WORDS {
                    return None;
                }

                let bitmap_word = BITMAP[word_idx];
                if bitmap_word == u64::MAX {
                    // Summary inconsistency guard — should not happen.
                    continue;
                }

                // First free bit in this L0 word.
                let bit_idx = (!bitmap_word).trailing_zeros() as usize;
                let frame_idx = word_idx * 64 + bit_idx;

                if frame_idx >= self.total_frames {
                    return None;
                }

                // Mark used.
                BITMAP[word_idx] |= 1u64 << bit_idx;
                self.used_frames += 1;

                // If L0 word is now full, set the summary bit.
                if BITMAP[word_idx] == u64::MAX {
                    SUMMARY[summary_idx] |= 1u64 << summary_bit;
                }

                return Some(PhysFrame::containing_address(PhysAddr::new(
                    frame_idx as u64 * PAGE_SIZE,
                )));
            }

            None
        }
    }

    /// Return a single 4 KiB physical frame to the free pool. O(1).
    ///
    /// Double-free is safely ignored.
    pub fn dealloc_frame(&mut self, frame: PhysFrame<Size4KiB>) {
        let frame_idx = (frame.start_address().as_u64() / PAGE_SIZE) as usize;

        if frame_idx >= self.total_frames || frame_idx >= MAX_PHYS_FRAMES {
            return;
        }

        let word_idx = frame_idx / 64;
        let bit_idx = frame_idx % 64;
        let mask = 1u64 << bit_idx;

        unsafe {
            if BITMAP[word_idx] & mask == 0 {
                // Already free — double-free guard.
                return;
            }

            // Clear bit = mark free.
            BITMAP[word_idx] &= !mask;
            self.used_frames = self.used_frames.saturating_sub(1);

            // L0 word is no longer full → clear summary bit.
            let summary_idx = word_idx / 64;
            let summary_bit = word_idx % 64;
            SUMMARY[summary_idx] &= !(1u64 << summary_bit);
        }
    }
}

// Implement the x86_64 FrameAllocator trait so k-vmm and k-heap
// continue working without any caller-side changes.
unsafe impl FrameAllocator<Size4KiB> for BitmapFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        self.alloc_frame()
    }
}

// ---------------------------------------------------------------------------
// Public accessor — backward-compatible with k-vmm and k-heap call sites.
// ---------------------------------------------------------------------------

pub fn allocator() -> &'static Mutex<BitmapFrameAllocator> {
    unsafe {
        let p = node_ptr();
        if p.is_null() {
            panic!("K_PMM node not initialized");
        }
        &*(p.add(1024) as *mut Mutex<BitmapFrameAllocator>)
    }
}

/// Registration hook — receives boot_info payload before on_init.
pub fn register_hook(ctx: &mut BootContext) {
    unsafe {
        BOOT_INFO_PTR = ctx.payload;
    }
}

// ---------------------------------------------------------------------------
// Graph-native executor callbacks
// ---------------------------------------------------------------------------

unsafe extern "C" fn pmm_on_init(_ctx: *mut ExecutorContext) -> ExecStatus {
    unsafe {
        let p = node_ptr();
        meta::burn_node_metadata(p, "SYS", "PMM");

        let boot_info_payload = BOOT_INFO_PTR;
        let boot_info_ptr = boot_info_payload as *const bootloader::BootInfo;
        let memory_map = &(*boot_info_ptr).memory_map;

        let alloc = BitmapFrameAllocator::init(memory_map);

        let state_ptr = p.add(1024) as *mut Mutex<BitmapFrameAllocator>;
        core::ptr::write(state_ptr, Mutex::new(alloc));
    }
    ExecStatus::Done
}

unsafe extern "C" fn pmm_on_event(
    _ctx: *mut ExecutorContext,
    event: *const NodeEvent,
) -> ExecStatus {
    // Future: handle Signal::Control { cmd: PMM_CONTROL_QUERY_STATS, .. }
    // via proc::process when pre::prepare returns Some.
    let Some(input) = pre::prepare(event) else { return ExecStatus::Done; };
    let Some(output) = proc::process(input) else { return ExecStatus::Done; };
    post::emit(output)
}

unsafe extern "C" fn pmm_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// ── Plugin Descriptor ────────────────────────────────────────────────────────

const PMM_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PhysMap, arg0: u64::MAX, arg1: u64::MAX },
    PermissionSpec { kind: PermissionKind::GraphWrite, arg0: 0, arg1: 0 },
];
const PMM_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "memory", name: "frame_alloc" },
];

pub const PLUGIN_DESCRIPTOR: BuiltinPluginDescriptor = BuiltinPluginDescriptor {
    manifest: PluginManifest {
        abi_version: GOS_ABI_VERSION,
        plugin_id: PluginId::from_ascii("K_PMM"),
        name: "K_PMM",
        version: 1,
        depends_on: &[],
        permissions: PMM_PERMS,
        exports: PMM_EXPORTS,
        imports: &[],
        nodes: &[NodeSpec {
            node_id: derive_node_id(PluginId::from_ascii("K_PMM"), "pmm.entry"),
            local_node_key: "pmm.entry",
            node_type: RuntimeNodeType::Service,
            entry_policy: EntryPolicy::Bootstrap,
            executor_id: EXECUTOR_ID,
            state_schema_hash: 0x200A,
            permissions: PMM_PERMS,
            exports: PMM_EXPORTS,
            vector_ref: None,
        }],
        edges: &[],
        signature: None,
        policy_hash: [0; 16],
    },
    granted_permissions: PMM_PERMS,
    nodes: &[NativeNodeBinding {
        vector: NODE_VEC,
        local_node_key: "pmm.entry",
        executor: EXECUTOR_VTABLE,
    }],
    register_hook: Some(register_hook),
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bootloader::bootinfo::{MemoryRegion, MemoryRegionType, MemoryMap};

    #[test]
    fn test_bitmap_alloc_dealloc() {
        unsafe {
            // Reset bitmaps (since they are static mut)
            BITMAP = [u64::MAX; BITMAP_WORDS];
            SUMMARY = [u64::MAX; SUMMARY_WORDS];

            // Create a mock memory map with one usable region [4MB, 8MB)
            // This contains (8-4)*1024/4 = 1024 frames.
            let mut mmap = MemoryMap::default();
            mmap.add_region(MemoryRegion {
                range: bootloader::bootinfo::MemoryRange::new(0, 0x400000),
                region_type: MemoryRegionType::Reserved,
            });
            mmap.add_region(MemoryRegion {
                range: bootloader::bootinfo::MemoryRange::new(0x400000, 0x800000),
                region_type: MemoryRegionType::Usable,
            });

            let mut alloc = BitmapFrameAllocator::init(&mmap);
            
            assert_eq!(alloc.total_frames(), (0x800000 / PAGE_SIZE) as usize);
            assert_eq!(alloc.free_frames(), 1024);

            // Alloc first frame (should be index 1024 since [0..1024] is reserved)
            let f1 = alloc.alloc_frame().expect("should have frames");
            assert_eq!(f1.start_address().as_u64(), 0x400000);
            assert_eq!(alloc.free_frames(), 1023);

            // Alloc second
            let f2 = alloc.alloc_frame().expect("should have frames");
            assert_eq!(f2.start_address().as_u64(), 0x400000 + 4096);

            // Dealloc first
            alloc.dealloc_frame(f1);
            assert_eq!(alloc.free_frames(), 1023);

            // Alloc again — should get f1 back (search finds the first hole)
            let f3 = alloc.alloc_frame().expect("should have frames");
            assert_eq!(f3.start_address().as_u64(), 0x400000);
        }
    }
}
