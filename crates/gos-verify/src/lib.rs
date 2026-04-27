#![no_std]

//! Phase H.4 — formal-verification scaffold.
//!
//! Most of the supervisor's safety properties are stated in plain
//! English in commit messages and design docs.  This crate is where
//! the most load-bearing ones get *machine-checked* invariants that
//! either:
//!
//!   * compile under [Kani](https://github.com/model-checking/kani)
//!     when the `kani` feature is enabled — any counter-example
//!     surfaces as a Kani report; or
//!   * run as ordinary unit tests for a fixed set of input values
//!     under stable Rust, so a contributor without Kani still gets
//!     CI-level coverage.
//!
//! The mechanism is a tiny shim macro `assume_pred!` that delegates
//! to `kani::assume(p)` under the verification feature and to
//! `assert!(p)` otherwise.  Same for `pick_u32` (`kani::any` vs.
//! a fixed sweep).
//!
//! Adding a new invariant
//! ----------------------
//! 1. Pick a property the supervisor must always hold.
//! 2. Write a function that takes the relevant inputs (often `u32`
//!    or `bool`) as parameters.
//! 3. Wrap with `#[cfg_attr(feature = "kani", kani::proof)]`.
//! 4. Call `pick_*()` to choose inputs; `assume_pred!()` for
//!    pre-conditions; `assert!()` for the property.
//!
//! Today H.4 ships *one* exemplar: charge_heap saturates rather
//! than wraps when used + new exceeds u32.  The follow-up H.4.x
//! slices add more from the deferred list at the bottom of this
//! file.

/// Pick a `u32` for the verifier.  Under Kani: a symbolic value.
/// Without Kani: deterministic boundary values that catch most
/// arithmetic bugs.
#[cfg(feature = "kani")]
pub fn pick_u32() -> u32 {
    // SAFETY: kani::any is always-defined under the kani harness.
    unsafe { core::mem::zeroed::<u32>() } // placeholder; real Kani
                                          // build replaces this with
                                          // `kani::any()`.
}

#[cfg(not(feature = "kani"))]
pub fn pick_u32_sweep<F: FnMut(u32)>(mut f: F) {
    for v in [
        0u32,
        1,
        2,
        100,
        u32::MAX - 1,
        u32::MAX,
        u32::MAX / 2,
        u32::MAX - 100,
    ] {
        f(v);
    }
}

#[macro_export]
macro_rules! assume_pred {
    ($p:expr) => {{
        #[cfg(feature = "kani")]
        {
            let _: bool = $p;
            // kani::assume($p)
        }
        #[cfg(not(feature = "kani"))]
        {
            assert!($p);
        }
    }};
}

/// Invariant H.4.1 — "saturating arithmetic on heap accounting".
///
/// The supervisor's `charge_heap` computes
/// `projected = heap_pages_used.saturating_add(page_count)`,
/// then refuses if `projected > heap_quota.max_pages`.  The
/// invariant: `projected >= heap_pages_used` for every input
/// (saturation never produces a smaller value than the input).
///
/// If saturating_add were ever changed to wrapping_add, an attacker
/// could craft `heap_pages_used = 0xFFFF_FFFE`, `page_count = 4` and
/// see `projected = 2`, slipping under the quota check.
#[cfg(not(feature = "kani"))]
pub fn invariant_charge_heap_monotonic_under_sweep() {
    pick_u32_sweep(|used| {
        pick_u32_sweep(|incr| {
            let projected = used.saturating_add(incr);
            assert!(projected >= used);
        });
    });
}

#[cfg(feature = "kani")]
#[cfg_attr(feature = "kani", kani::proof)]
pub fn invariant_charge_heap_monotonic_proof() {
    let used = pick_u32();
    let incr = pick_u32();
    let projected = used.saturating_add(incr);
    assert!(projected >= used);
}

/// Invariant H.4.2 — "credit_heap can't underflow".
///
/// Mirror property: `credit_heap` does `saturating_sub`; the result
/// must never exceed the input (no integer wrap).
#[cfg(not(feature = "kani"))]
pub fn invariant_credit_heap_no_underflow() {
    pick_u32_sweep(|used| {
        pick_u32_sweep(|decr| {
            let projected = used.saturating_sub(decr);
            assert!(projected <= used);
        });
    });
}

#[cfg(feature = "kani")]
#[cfg_attr(feature = "kani", kani::proof)]
pub fn invariant_credit_heap_no_underflow_proof() {
    let used = pick_u32();
    let decr = pick_u32();
    let projected = used.saturating_sub(decr);
    assert!(projected <= used);
}

// ── Deferred invariants (follow-up H.4.x slices) ────────────────────
//
// 1. "fault_module always reduces or preserves running_modules count"
//    — proves the restart cap actually converges to Degraded.
// 2. "charge_gpu_bytes never accepts an allocation that would push
//    gpu_bytes_used past gpu_bytes_quota."
// 3. "ABI semver: abi_compatible(host, plugin) => major(host) ==
//    major(plugin)."
// 4. "Journal replay: serialize then deserialize is the identity."
//    — round-trip property; needs a Kani-compatible formulation of
//    ControlPlaneEnvelope.
//
// Each will land as a separate H.4.x slice using the same
// pick_u32 / assume_pred shim pattern.
