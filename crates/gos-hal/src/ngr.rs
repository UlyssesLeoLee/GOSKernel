//! Legacy NGR compatibility shim.
//!
//! Existing plugins still call `gos_hal::ngr::*`. The v0.2 runtime owns the
//! actual queues and lifecycle tracking, while this module keeps the old API
//! surface alive during migration.

use core::mem::transmute;

use gos_protocol::{CellResult, NodeCell, Signal, VectorAddress};

pub fn route_signal(target: VectorAddress, signal: Signal) -> CellResult {
    match gos_runtime::route_signal(target, signal) {
        Ok(result) => result,
        Err(_) => CellResult::Fault("runtime route failed"),
    }
}

pub fn activate(target: VectorAddress) -> CellResult {
    match gos_runtime::activate(target) {
        Ok(result) => result,
        Err(_) => CellResult::Fault("runtime activate failed"),
    }
}

pub fn try_mount_cell(target: VectorAddress, cell: &'static spin::Mutex<dyn NodeCell>) {
    let raw: *const spin::Mutex<dyn NodeCell> = cell;
    let fat_ptr: [usize; 2] = unsafe { transmute(raw) };
    let _ = gos_runtime::bind_legacy_cell(target, fat_ptr);
}

/// Asynchronously enqueue a signal (safe to call from interrupt context).
pub fn post_signal(target: VectorAddress, signal: Signal) {
    let _ = gos_runtime::post_signal(target, signal);
}

/// Pump queued runtime signals and ready nodes.
pub fn pump_signals() {
    gos_runtime::pump();
}
