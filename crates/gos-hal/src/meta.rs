//! K_META — Graph Metadata Node
//!
//! Manages the HAL matrix metadata layer. All struct fields are written
//! byte-by-byte via `ptr::write_volatile` to avoid compiler-emitted
//! SIMD/AVX instructions that fault before XSAVE/FXSAVE are enabled.
//!
//! Layout constants are derived from `#[repr(C)]` rules and verified at
//! compile time with `const` assertions.

use core::mem;
use core::ptr;
use gos_protocol::{EdgeHeader, GOS_EDGE_MAGIC, GOS_NODE_MAGIC, NodeHeader, VectorAddress};

// ── Compile-time layout verification ─────────────────────────────────────────

/// NodeHeader field offsets (verified against #[repr(C)] rules).
const NH_OFF_MAGIC:    usize = 0;   // u32  @ 0
const NH_OFF_UUID:     usize = 4;   // [u8;16] @ 4
const NH_OFF_LABEL:    usize = 20;  // [u8;16] @ 20
const NH_OFF_NAME:     usize = 36;  // [u8;16] @ 36
const NH_OFF_VERSION:  usize = 52;  // u32  @ 52
const NH_OFF_ACL:      usize = 56;  // u64  @ 56  (8-byte aligned ✓)
const NH_OFF_CELLPTR:  usize = 64;  // [u64;2] @ 64
const NH_OFF_RES:      usize = 80;  // [u8;176] @ 80  → total 256
const NH_SIZE:         usize = 256;

/// EdgeHeader field offsets with #[repr(C)] natural alignment.
/// After `weight: f32` ends at 28, `acl_mask: u64` needs 8-byte alignment
/// → 4 bytes of padding inserted at 28 → acl_mask starts at 32.
const EH_OFF_MAGIC:    usize = 0;   // u32  @ 0
const EH_OFF_TYPENAME: usize = 4;   // [u8;12] @ 4
const EH_OFF_TARGET:   usize = 16;  // u64  @ 16
const EH_OFF_WEIGHT:   usize = 24;  // f32  @ 24
//                                   padding 4 bytes @ 28-31
const EH_OFF_ACLMASK:  usize = 32;  // u64  @ 32
const EH_OFF_RES:      usize = 40;  // [u8;28] @ 40
//                                   trailing padding 4 bytes @ 68-71
const EH_STRIDE:       usize = 72;  // sizeof(EdgeHeader) with align(8)

const MAX_EDGES: usize = 12;

// Compile-time guards — if struct layout changes these will fail to compile.
const _: () = assert!(mem::size_of::<NodeHeader>() == NH_SIZE,
    "NodeHeader size mismatch – update NH_SIZE");
const _: () = assert!(mem::size_of::<EdgeHeader>() == EH_STRIDE,
    "EdgeHeader size mismatch – update EH_STRIDE");

// ── Public vector constant ────────────────────────────────────────────────────

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 10, 0, 0);

pub fn node_ptr() -> *mut u8 {
    crate::vaddr::resolve_hal_node(NODE_VEC)
}

// ── Core write helpers ────────────────────────────────────────────────────────

#[inline(always)]
unsafe fn wv_u32(p: *mut u8, off: usize, val: u32) {
    ptr::write_volatile(p.add(off) as *mut u32, val);
}

#[inline(always)]
unsafe fn wv_u64(p: *mut u8, off: usize, val: u64) {
    ptr::write_volatile(p.add(off) as *mut u64, val);
}

#[inline(always)]
unsafe fn wv_bytes(p: *mut u8, off: usize, src: &[u8], max: usize) {
    for i in 0..max {
        let b = if i < src.len() { src[i] } else { 0 };
        ptr::write_volatile(p.add(off + i), b);
    }
}

#[inline(always)]
unsafe fn wv_zero(p: *mut u8, off: usize, len: usize) {
    for i in 0..len {
        ptr::write_volatile(p.add(off + i), 0u8);
    }
}

// ── NodeHeader writer ─────────────────────────────────────────────────────────

/// Write a `NodeHeader` field-by-field directly to `p`.
///
/// # Safety
/// `p` must point to at least 1024 bytes of mapped, writable memory.
pub unsafe fn burn_node_metadata(p: *mut u8, label: &str, name: &str) {
    if p.is_null() { return; }

    wv_u32(p, NH_OFF_MAGIC,   GOS_NODE_MAGIC);  // magic
    wv_zero(p, NH_OFF_UUID,   16);               // uuid
    wv_bytes(p, NH_OFF_LABEL, label.as_bytes(), 16);
    wv_bytes(p, NH_OFF_NAME,  name.as_bytes(),  16);
    wv_u32(p, NH_OFF_VERSION, 1);                // version
    wv_u64(p, NH_OFF_ACL,     0xFFFF);           // acl
    wv_u64(p, NH_OFF_CELLPTR, 0);               // cell_ptr[0]
    wv_u64(p, NH_OFF_CELLPTR + 8, 0);           // cell_ptr[1]
    wv_zero(p, NH_OFF_RES,    176);              // _res

    // Edge slots
    for i in 0..MAX_EDGES {
        burn_edge_slot(p.add(NH_SIZE + i * EH_STRIDE), b"NONE", 0);
    }
}

/// Write a single EdgeHeader in-place at `slot_ptr`.
unsafe fn burn_edge_slot(e: *mut u8, type_name: &[u8], target: u64) {
    wv_u32(e, EH_OFF_MAGIC,    GOS_EDGE_MAGIC);
    wv_bytes(e, EH_OFF_TYPENAME, type_name, 12);
    wv_u64(e, EH_OFF_TARGET,   target);
    wv_u32(e, EH_OFF_WEIGHT,   0x3F800000);     // 1.0f32
    wv_zero(e, EH_OFF_WEIGHT + 4, 4);           // padding
    wv_u64(e, EH_OFF_ACLMASK, u64::MAX);
    wv_zero(e, EH_OFF_RES,     28);             // _res
    wv_zero(e, EH_OFF_RES + 28, 4);            // trailing pad
}

// ── Public edge API ───────────────────────────────────────────────────────────

/// Update a single edge slot inside an existing node block.
///
/// # Safety
/// `p` must point to a node block written by `burn_node_metadata`.
pub unsafe fn burn_edge_metadata(p: *mut u8, slot: usize, edge_type: &str, target: u64) {
    if p.is_null() || slot >= MAX_EDGES { return; }
    burn_edge_slot(p.add(NH_SIZE + slot * EH_STRIDE), edge_type.as_bytes(), target);
}

/// Restrict an edge slot to a specific caller/target vector.
///
/// # Safety
/// `p` must point to a valid node block.
pub unsafe fn restrict_edge(p: *mut u8, slot: usize, mask: u64, expected_vec: u64) {
    if p.is_null() || slot >= MAX_EDGES { return; }
    let e = p.add(NH_SIZE + slot * EH_STRIDE);
    wv_u64(e, EH_OFF_ACLMASK, mask);
    wv_u64(e, EH_OFF_TARGET,  expected_vec);
}

/// Store a cell pointer inside the node header.
///
/// # Safety
/// `p` must point to a valid node block.
pub unsafe fn mount_cell(p: *mut u8, cell_ptr: [u64; 2]) {
    if p.is_null() { return; }
    wv_u64(p, NH_OFF_CELLPTR,     cell_ptr[0]);
    wv_u64(p, NH_OFF_CELLPTR + 8, cell_ptr[1]);
}

// ── Init ──────────────────────────────────────────────────────────────────────

pub fn init() {
    unsafe {
        let p = node_ptr();
        burn_node_metadata(p, "SYS", "META");
    }
}
