#![no_std]

//! Phase G.2 — module signature verification gate.
//!
//! Every `ModuleDescriptor` already carries an `Option<&[u8]>`
//! signature field and a 16-byte `policy_hash`.  Until G.2 those
//! fields had no enforcement: an unsigned (or arbitrarily signed)
//! module would install just like a trusted one.  This crate fills
//! the *gate* — the actual cryptographic primitive (Ed25519
//! verification) is plugged in via the `SignatureVerifier` hook
//! table, so the supervisor doesn't grow a hard dependency on a
//! specific crypto crate.
//!
//! Wiring sequence
//! ---------------
//! 1. Kernel boot installs a `SignatureVerifier`:
//!      gos_sign::install_verifier(SignatureVerifier { verify, ... });
//! 2. supervisor::install_module calls `gos_sign::verify_module(..)`
//!    against the descriptor's signature + policy_hash.
//! 3. Verifier returns Ok / Err per the active SecurityPolicy.
//!
//! Defaults
//! --------
//! No verifier installed → `verify_module` honours `SecurityPolicy::
//! current()`:
//!   * `Permissive` (default) — both unsigned and signed-but-unchecked
//!     modules succeed.  Useful in development; matches the current
//!     in-tree builtin set which has signature: None.
//!   * `RequireSigned`        — modules without a signature are
//!     rejected; signed modules succeed (no crypto check yet).
//!   * `Strict`               — modules without a verifier installed
//!     are *all* rejected, signed or not.  Production stance once
//!     G.2.1 ships real Ed25519.

use gos_protocol::ModuleDescriptor;
use spin::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureError {
    /// Module had no signature but the active policy requires one.
    Unsigned,
    /// Cryptographic verification failed (signature didn't match the
    /// descriptor's policy_hash under the trusted public key).
    BadSignature,
    /// `Strict` policy with no verifier installed — refuse everything.
    NoVerifierAvailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SecurityPolicy {
    Permissive = 0,
    RequireSigned = 1,
    Strict = 2,
}

#[derive(Clone, Copy)]
pub struct SignatureVerifier {
    /// Verify `signature` against `policy_hash` for this module.
    /// Implementation-defined: a real verifier (G.2.1) computes
    /// `Ed25519::verify(trust_root_pubkey, policy_hash, signature)`
    /// and returns Ok on match.
    pub verify: unsafe extern "C" fn(
        signature: *const u8,
        signature_len: usize,
        policy_hash: *const u8,
    ) -> i32,
}

static VERIFIER: Mutex<Option<SignatureVerifier>> = Mutex::new(None);
static POLICY: Mutex<SecurityPolicy> = Mutex::new(SecurityPolicy::Permissive);

pub fn install_verifier(verifier: SignatureVerifier) {
    *VERIFIER.lock() = Some(verifier);
}

pub fn set_policy(policy: SecurityPolicy) {
    *POLICY.lock() = policy;
}

pub fn current_policy() -> SecurityPolicy {
    *POLICY.lock()
}

/// Decide whether `module` is allowed to install under the active
/// policy + verifier.  Wired into `supervisor::install_module`.
pub fn verify_module(module: &ModuleDescriptor) -> Result<(), SignatureError> {
    let policy = current_policy();
    match (module.signature, policy) {
        // Unsigned + Permissive: allow.
        (None, SecurityPolicy::Permissive) => Ok(()),
        // Unsigned + RequireSigned/Strict: reject.
        (None, _) => Err(SignatureError::Unsigned),
        // Signed: defer to verifier if installed.
        (Some(sig), pol) => {
            let verifier = *VERIFIER.lock();
            match verifier {
                Some(v) => {
                    // Today the descriptor doesn't carry a separate
                    // policy_hash field — the module_id (16 bytes,
                    // already a stable identity hash) is what we
                    // attest to.  G.2.1 may add a richer digest field
                    // (hash of the loaded image) but the verifier
                    // signature is the same shape: (sig, sig_len,
                    // 16-byte payload_pointer).
                    let payload = &module.module_id.0;
                    let rc = unsafe {
                        (v.verify)(sig.as_ptr(), sig.len(), payload.as_ptr())
                    };
                    if rc == 0 {
                        Ok(())
                    } else {
                        Err(SignatureError::BadSignature)
                    }
                }
                None => match pol {
                    SecurityPolicy::Permissive | SecurityPolicy::RequireSigned => {
                        // No crypto wired yet — under Permissive we allow
                        // signed modules through (developer convenience),
                        // under RequireSigned we still allow because the
                        // signature is *present* (just unverified).  Real
                        // Ed25519 (G.2.1) replaces this branch.
                        Ok(())
                    }
                    SecurityPolicy::Strict => Err(SignatureError::NoVerifierAvailable),
                },
            }
        }
    }
}
