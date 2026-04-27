#![no_std]

//! Phase H.2 — typed bridge between `k-ai` and the rest of the
//! kernel.
//!
//! Today's `k-ai` is a host-backed bridge that returns free-form text.
//! H.2 adds *structure*: an `LlmRequest` carries graph context the
//! model needs; an `LlmResponse` carries free-form text **and** a
//! bounded list of `CypherMutation` candidates.  Mutations from the
//! AI never apply directly — they go through a `MutationGate` that
//! requires explicit operator approval (`AcceptanceMode::Confirmed`).
//!
//! This is the surface that lets a future LLM-driven control plane
//! ("self-describing OS" in the roadmap) propose changes without
//! granting the AI write access to the graph.  Confirmed mutations
//! flow through the H.1 receptive subset, so even an approval bug
//! can't escalate beyond what a human operator could do interactively.

use gos_cypher_mut::{pre_validate, CypherMutation, MutationError};

pub const MAX_PROMPT_BYTES: usize = 4096;
pub const MAX_RESPONSE_BYTES: usize = 8192;
pub const MAX_SUGGESTED_MUTATIONS: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct LlmRequest<'a> {
    pub prompt: &'a [u8],
    /// Model picks from this when answering — typically
    /// `[node_id_a, node_id_b, ...]` or `b"theme.current"` etc.
    /// Free-form bytes; H.2 doesn't constrain encoding.
    pub context: &'a [u8],
    /// Acceptance mode the user picked when dispatching the prompt.
    /// `AcceptanceMode::DryRun` always rejects every mutation —
    /// useful for AI-driven analysis where mutations are display-only.
    pub mode: AcceptanceMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AcceptanceMode {
    /// Mutations are surfaced but never applied; UI shows "would do".
    DryRun = 0,
    /// Mutations are queued; operator must run `MutationGate::confirm`
    /// to actually apply them.
    Confirmed = 1,
    /// Mutations apply automatically.  RESERVED — H.2 never returns
    /// this from the gate; future trust mechanism (signed AI runtime)
    /// would unlock.  Today it's accepted for completeness, but the
    /// gate maps it to Confirmed.
    Auto = 2,
}

#[derive(Debug, Clone, Copy)]
pub struct LlmResponse {
    /// Free-form prose payload returned from the model.  Bounded so
    /// the reply fits a single control-plane envelope chain.
    pub text_len: u16,
    pub text: [u8; MAX_RESPONSE_BYTES],
    /// Suggested mutations the model would like to apply.  All must
    /// pass `gos_cypher_mut::pre_validate`; anything outside the
    /// receptive subset is dropped server-side before the response
    /// crosses the host bridge.
    pub mutation_count: u8,
    pub mutations: [Option<CypherMutation>; MAX_SUGGESTED_MUTATIONS],
}

impl LlmResponse {
    pub const fn empty() -> Self {
        Self {
            text_len: 0,
            text: [0; MAX_RESPONSE_BYTES],
            mutation_count: 0,
            mutations: [None; MAX_SUGGESTED_MUTATIONS],
        }
    }

    pub fn text_bytes(&self) -> &[u8] {
        &self.text[..self.text_len as usize]
    }

    pub fn pending_mutations(&self) -> impl Iterator<Item = &CypherMutation> {
        self.mutations[..self.mutation_count as usize]
            .iter()
            .filter_map(|m| m.as_ref())
    }
}

/// Bridge implementations register a backend through this hook —
/// host-side k-ai installs one that proxies to a real LLM service;
/// host harnesses install a deterministic stub for tests.  Same
/// pattern as the gos-runtime hooks.
#[derive(Clone, Copy)]
pub struct LlmBackend {
    pub query: unsafe extern "C" fn(
        prompt: *const u8,
        prompt_len: u32,
        context: *const u8,
        context_len: u32,
        out_response: *mut LlmResponse,
    ) -> i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeError {
    /// No backend installed — k-ai's host process not connected.
    BackendUnavailable,
    /// Backend returned a non-zero status.
    BackendError(i32),
    /// One of the suggested mutations didn't pass pre_validate.
    InvalidSuggestion(MutationError),
    /// Prompt or context exceeded the bounded buffers.
    PayloadTooLarge,
}

use spin::Mutex;
static BACKEND: Mutex<Option<LlmBackend>> = Mutex::new(None);

pub fn install_backend(backend: LlmBackend) {
    *BACKEND.lock() = Some(backend);
}

/// Issue an `LlmRequest` synchronously.  Validates each suggested
/// mutation through `pre_validate`; the *first* invalid suggestion
/// surfaces as `InvalidSuggestion` and the entire response is
/// discarded — better to refuse the whole turn than mix valid and
/// invalid mutations into the gate.
pub fn ask(req: &LlmRequest<'_>) -> Result<LlmResponse, BridgeError> {
    if req.prompt.len() > MAX_PROMPT_BYTES || req.context.len() > MAX_PROMPT_BYTES {
        return Err(BridgeError::PayloadTooLarge);
    }
    let backend = *BACKEND.lock();
    let backend = backend.ok_or(BridgeError::BackendUnavailable)?;
    let mut response = LlmResponse::empty();
    let rc = unsafe {
        (backend.query)(
            req.prompt.as_ptr(),
            req.prompt.len() as u32,
            req.context.as_ptr(),
            req.context.len() as u32,
            &mut response,
        )
    };
    if rc != 0 {
        return Err(BridgeError::BackendError(rc));
    }
    for m in response.pending_mutations() {
        pre_validate(m).map_err(BridgeError::InvalidSuggestion)?;
    }
    Ok(response)
}

/// MutationGate: holds the AI's confirmed-but-not-yet-applied
/// suggestions.  Operator UI calls `accept_index(i)` to apply one,
/// `reject_index(i)` to drop one, `clear()` to flush all.
pub struct MutationGate {
    pending: [Option<CypherMutation>; MAX_SUGGESTED_MUTATIONS],
    len: usize,
}

impl MutationGate {
    pub const fn new() -> Self {
        Self {
            pending: [None; MAX_SUGGESTED_MUTATIONS],
            len: 0,
        }
    }

    pub fn enqueue(&mut self, response: &LlmResponse, mode: AcceptanceMode) -> usize {
        // DryRun never accepts anything; Confirmed/Auto stage for
        // operator approval.  See AcceptanceMode docs.
        if mode == AcceptanceMode::DryRun {
            return 0;
        }
        let mut staged = 0usize;
        for m in response.pending_mutations().copied() {
            if self.len >= MAX_SUGGESTED_MUTATIONS {
                break;
            }
            self.pending[self.len] = Some(m);
            self.len += 1;
            staged += 1;
        }
        staged
    }

    pub fn pending(&self) -> &[Option<CypherMutation>] {
        &self.pending[..self.len]
    }

    pub fn accept_index(&mut self, idx: usize) -> Option<CypherMutation> {
        if idx >= self.len {
            return None;
        }
        let m = self.pending[idx].take()?;
        self.compact();
        Some(m)
    }

    pub fn reject_index(&mut self, idx: usize) -> bool {
        if idx >= self.len {
            return false;
        }
        self.pending[idx] = None;
        self.compact();
        true
    }

    pub fn clear(&mut self) {
        for slot in self.pending.iter_mut() {
            *slot = None;
        }
        self.len = 0;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn compact(&mut self) {
        let mut write = 0usize;
        for read in 0..self.len {
            if let Some(m) = self.pending[read].take() {
                self.pending[write] = Some(m);
                write += 1;
            }
        }
        self.len = write;
    }
}
