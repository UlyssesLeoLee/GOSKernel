//! K_VADDR — Legacy vector compatibility space.
//!
//! The v0.2 runtime no longer relies on direct `VectorAddress -> raw block`
//! arithmetic for global identity. This module remains as a compatibility
//! mapping for legacy HAL plugins that still burn metadata into fixed blocks.

use core::ptr;
use gos_protocol::VectorAddress;
use crate::meta::burn_node_metadata;

/// A single 4KB-aligned block in the HAL Matrix.
#[derive(Debug, Clone, Copy)]
#[repr(align(4096))]
pub struct NodeBlock(pub [u8; 4096]);

/// The legacy HAL matrix keeps a small pool of fixed blocks for compatibility.
/// Indexing is intentionally coarse: one 16-slot window per domain.
pub const HAL_MATRIX_LEN: usize = 128;
pub static mut HAL_MATRIX: [NodeBlock; HAL_MATRIX_LEN] =
    [NodeBlock([0; 4096]); HAL_MATRIX_LEN];

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 9, 0, 0);

pub fn node_ptr() -> *mut u8 {
    resolve_hal_node(NODE_VEC)
}

pub fn resolve_hal_node(vec: VectorAddress) -> *mut u8 {
    let domain_offset = (vec.l4 as usize) * 16;
    let idx = domain_offset + (vec.l3 as usize & 0x0F);
    if idx >= HAL_MATRIX_LEN {
        return ptr::null_mut();
    }
    // SAFETY: `HAL_MATRIX` is a static mut.  We avoid materializing a
    // shared reference (forbidden under Rust 2024) by going through
    // `&raw mut` and indexing the raw slice element.  Single-threaded
    // kernel: no concurrent access window exists.
    unsafe {
        let base: *mut NodeBlock = (&raw mut HAL_MATRIX) as *mut NodeBlock;
        (*base.add(idx)).0.as_mut_ptr()
    }
}

pub fn init() {
    unsafe {
        let p = node_ptr();
        burn_node_metadata(p, "SYS", "VADDR");
    }
}
