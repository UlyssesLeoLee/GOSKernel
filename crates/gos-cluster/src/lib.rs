#![no_std]

//! Phase H.3 — distributed graph: remote-vector addressing.
//!
//! `VectorAddress` (l4 / l3 / l2 / offset) is the kernel-local address
//! of a node in the runtime graph.  Once GOS speaks across nodes
//! (cluster operating system, the long-game in the roadmap), a single
//! `VectorAddress` is no longer enough — we need to disambiguate
//! "this node on host A" from "this node on host B".
//!
//! H.3 is the *type system + registry* slice.  Real network transport
//! lives behind `RemoteTransportVTable` and is plugged in once a real
//! cluster bus exists.  Until then `RemoteVector` resolves only to
//! self-hosted vectors (mostly useful in tests, but the addressing
//! invariants are real).

use gos_protocol::VectorAddress;
use spin::Mutex;

/// 64-bit identifier for a host in the cluster.  Reserved id `0`
/// means "the local node" — encoding it explicitly makes `LOCAL` an
/// ordinary `RemoteVector` rather than a special case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct HostId(pub u64);

impl HostId {
    pub const LOCAL: Self = Self(0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteVector {
    pub host: HostId,
    pub vector: VectorAddress,
}

impl RemoteVector {
    pub const fn local(vector: VectorAddress) -> Self {
        Self {
            host: HostId::LOCAL,
            vector,
        }
    }

    pub const fn remote(host: HostId, vector: VectorAddress) -> Self {
        Self { host, vector }
    }

    pub const fn is_local(&self) -> bool {
        self.host.0 == HostId::LOCAL.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterError {
    /// Host id not present in the registry — the cluster bus hasn't
    /// announced this peer.
    UnknownHost(HostId),
    /// Cluster transport not installed.
    NoTransport,
    /// Underlying transport reported a failure (timeout, reset).
    TransportError(i32),
    /// The registry can't fit another peer.
    RegistryFull,
    /// Vector address is malformed for cross-host routing (e.g. l4
    /// reserved bit).
    InvalidAddress,
}

pub const MAX_PEERS: usize = 16;

#[derive(Debug, Clone, Copy)]
pub struct PeerRecord {
    pub host: HostId,
    pub generation: u32,
    pub healthy: bool,
}

impl PeerRecord {
    pub const fn empty() -> Self {
        Self {
            host: HostId(0),
            generation: 0,
            healthy: false,
        }
    }
}

struct Registry {
    peers: [Option<PeerRecord>; MAX_PEERS],
}

impl Registry {
    const fn new() -> Self {
        Self {
            peers: [None; MAX_PEERS],
        }
    }
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry::new());

/// Announce a peer.  Idempotent: re-announcing an existing host bumps
/// its `generation` and resets `healthy = true`.  Returns the
/// effective generation, or `RegistryFull` if the table is full.
pub fn announce_peer(host: HostId) -> Result<u32, ClusterError> {
    if host == HostId::LOCAL {
        return Err(ClusterError::InvalidAddress);
    }
    let mut reg = REGISTRY.lock();
    for slot in reg.peers.iter_mut() {
        if let Some(record) = slot {
            if record.host == host {
                record.generation = record.generation.wrapping_add(1);
                record.healthy = true;
                return Ok(record.generation);
            }
        }
    }
    for slot in reg.peers.iter_mut() {
        if slot.is_none() {
            *slot = Some(PeerRecord {
                host,
                generation: 1,
                healthy: true,
            });
            return Ok(1);
        }
    }
    Err(ClusterError::RegistryFull)
}

pub fn forget_peer(host: HostId) -> bool {
    let mut reg = REGISTRY.lock();
    for slot in reg.peers.iter_mut() {
        if let Some(record) = slot {
            if record.host == host {
                *slot = None;
                return true;
            }
        }
    }
    false
}

pub fn peer(host: HostId) -> Option<PeerRecord> {
    let reg = REGISTRY.lock();
    reg.peers
        .iter()
        .flatten()
        .find(|p| p.host == host)
        .copied()
}

pub fn known_peer_count() -> usize {
    let reg = REGISTRY.lock();
    reg.peers.iter().filter(|s| s.is_some()).count()
}

/// Cluster transport vtable.  Real implementations (TCP-over-smoltcp,
/// shared-memory IPC for co-located guests, ...) plug in via
/// `install_transport`.
#[derive(Clone, Copy)]
pub struct RemoteTransportVTable {
    pub handle: u64,
    pub send_signal: unsafe extern "C" fn(
        handle: u64,
        target: u64,
        signal_lo: u64,
        signal_hi: u64,
    ) -> i32,
}

static TRANSPORT: Mutex<Option<RemoteTransportVTable>> = Mutex::new(None);

pub fn install_transport(transport: RemoteTransportVTable) {
    *TRANSPORT.lock() = Some(transport);
}

/// Forward a signal addressed to a non-local `RemoteVector`.  Local
/// addresses are rejected here — those should go through
/// `gos_runtime::post_signal` directly.
pub fn route_remote_signal(
    target: RemoteVector,
    signal_lo: u64,
    signal_hi: u64,
) -> Result<(), ClusterError> {
    if target.is_local() {
        return Err(ClusterError::InvalidAddress);
    }
    let _ = peer(target.host).ok_or(ClusterError::UnknownHost(target.host))?;
    let transport = *TRANSPORT.lock();
    let transport = transport.ok_or(ClusterError::NoTransport)?;
    let rc = unsafe {
        (transport.send_signal)(
            transport.handle,
            target.vector.as_u64(),
            signal_lo,
            signal_hi,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(ClusterError::TransportError(rc))
    }
}
