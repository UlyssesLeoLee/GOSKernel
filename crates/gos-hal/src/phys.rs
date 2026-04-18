// ============================================================
// gos-hal :: phys — physical address utilities
//
// Stores the bootloader-provided physical_memory_offset so that
// any kernel crate can convert virtual kernel addresses into the
// physical addresses required for DMA setup and page-table walks.
// ============================================================

use core::sync::atomic::{AtomicU64, Ordering};

static PHYS_OFFSET: AtomicU64 = AtomicU64::new(0);

/// Called once from `kernel_main`, immediately after HAL init, with the
/// value from `BootInfo::physical_memory_offset`.
pub fn set_phys_offset(offset: u64) {
    PHYS_OFFSET.store(offset, Ordering::SeqCst);
}

/// Returns the physical_memory_offset stored at boot.
/// Zero if `set_phys_offset` was never called.
pub fn phys_offset() -> u64 {
    PHYS_OFFSET.load(Ordering::SeqCst)
}

/// Walk the active 4-level page table to translate a kernel virtual address
/// to its guest-physical address.  Handles 4 KiB, 2 MiB, and 1 GiB pages.
///
/// Returns `None` if the mapping is absent (page not present at any level).
///
/// # Safety
/// Reads directly from the active page table via CR3.  Must only be called
/// after `set_phys_offset` has been set with the correct value.
pub unsafe fn virt_to_phys(virt: u64) -> Option<u64> {
    let offset = phys_offset();

    // ── Level 4 (PML4) ─────────────────────────────────────────────────────
    let cr3_phys = x86_64::registers::control::Cr3::read()
        .0
        .start_address()
        .as_u64();
    let pml4 = (offset + cr3_phys) as *const u64;
    let pml4e = unsafe { *pml4.add(((virt >> 39) & 0x1FF) as usize) };
    if pml4e & 1 == 0 {
        return None;
    }

    // ── Level 3 (PDPT) ─────────────────────────────────────────────────────
    let pdpt_phys = pml4e & 0x000F_FFFF_FFFF_F000;
    let pdpt = (offset + pdpt_phys) as *const u64;
    let pdpte = unsafe { *pdpt.add(((virt >> 30) & 0x1FF) as usize) };
    if pdpte & 1 == 0 {
        return None;
    }
    if pdpte & (1 << 7) != 0 {
        // 1 GiB huge page
        return Some((pdpte & 0x000F_FFC0_0000_0000) | (virt & 0x3FFF_FFFF));
    }

    // ── Level 2 (PD) ───────────────────────────────────────────────────────
    let pd_phys = pdpte & 0x000F_FFFF_FFFF_F000;
    let pd = (offset + pd_phys) as *const u64;
    let pde = unsafe { *pd.add(((virt >> 21) & 0x1FF) as usize) };
    if pde & 1 == 0 {
        return None;
    }
    if pde & (1 << 7) != 0 {
        // 2 MiB huge page
        return Some((pde & 0x000F_FFFF_FFE0_0000) | (virt & 0x001F_FFFF));
    }

    // ── Level 1 (PT) ───────────────────────────────────────────────────────
    let pt_phys = pde & 0x000F_FFFF_FFFF_F000;
    let pt = (offset + pt_phys) as *const u64;
    let pte = unsafe { *pt.add(((virt >> 12) & 0x1FF) as usize) };
    if pte & 1 == 0 {
        return None;
    }

    Some((pte & 0x000F_FFFF_FFFF_F000) | (virt & 0x0FFF))
}
