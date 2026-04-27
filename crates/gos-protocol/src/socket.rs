//! Phase G.3 — socket ABI for plugin networking.
//!
//! `SocketDeviceVTable` is a kernel-installable C-ABI handle that a
//! networking driver (smoltcp wrapper, future user-mode stack) fills
//! in.  Plugins reach it through capability resolution after claiming
//! `RESOURCE_SOCKET` from the supervisor.
//!
//! The shape is intentionally tiny and protocol-agnostic so the same
//! driver surface fits TCP, UDP, and (later) raw sockets.  Callers
//! that need protocol-specific knobs (TCP keepalive, UDP multicast)
//! grow them into `SocketOptions` rather than the vtable itself.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SocketStatus {
    Ok = 0,
    /// Endpoint is closed / never opened.
    NotConnected = -1,
    /// Underlying transport reported a failure (DNS resolve, TCP
    /// reset, link down).
    TransportError = -2,
    /// Caller's buffer was the wrong size or null.
    BadBuffer = -3,
    /// No driver registered for sockets — plugin claimed
    /// RESOURCE_SOCKET before the kernel installed a backend.
    Unbound = -4,
    /// Endpoint table full.
    OutOfHandles = -5,
    /// Operation would block; caller asked for non-blocking.
    WouldBlock = -6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SocketKind {
    Tcp = 1,
    Udp = 2,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SocketAddress {
    /// IPv4 address in host byte order — bytes [a, b, c, d] for
    /// `a.b.c.d`.  IPv6 expansion is a follow-up; until then `flags`
    /// bit 0 == 0 means "the four octets in `addr` are an IPv4
    /// address; ignore the rest".
    pub addr: [u8; 16],
    pub port: u16,
    pub flags: u16,
}

impl SocketAddress {
    pub const fn ipv4(a: u8, b: u8, c: u8, d: u8, port: u16) -> Self {
        let mut addr = [0u8; 16];
        addr[0] = a;
        addr[1] = b;
        addr[2] = c;
        addr[3] = d;
        Self {
            addr,
            port,
            flags: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SocketOptions {
    /// 0 = blocking, 1 = non-blocking.
    pub nonblocking: u8,
    /// Reserved for protocol-specific bits (TCP_NODELAY, SO_REUSEADDR,
    /// IP_MULTICAST_TTL, ...).  Each flag is documented per kind.
    pub flags: u32,
}

impl SocketOptions {
    pub const fn default_blocking() -> Self {
        Self {
            nonblocking: 0,
            flags: 0,
        }
    }
}

/// V-table a socket-driver fills in and registers with the runtime.
/// `handle` is opaque driver state (smoltcp's interface struct in the
/// production wiring; nothing in tests).  Sockets returned from
/// `open` are referenced by `SocketHandle = u64`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SocketDeviceVTable {
    pub handle: u64,
    pub open: unsafe extern "C" fn(
        handle: u64,
        kind: SocketKind,
        addr: *const SocketAddress,
        opts: *const SocketOptions,
        out_socket: *mut u64,
    ) -> i32,
    pub send: unsafe extern "C" fn(
        handle: u64,
        socket: u64,
        buf: *const u8,
        len: u32,
        out_sent: *mut u32,
    ) -> i32,
    pub recv: unsafe extern "C" fn(
        handle: u64,
        socket: u64,
        buf: *mut u8,
        len: u32,
        out_received: *mut u32,
    ) -> i32,
    pub close: unsafe extern "C" fn(handle: u64, socket: u64) -> i32,
}

impl SocketStatus {
    pub const fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::Ok,
            -1 => Self::NotConnected,
            -2 => Self::TransportError,
            -3 => Self::BadBuffer,
            -4 => Self::Unbound,
            -5 => Self::OutOfHandles,
            -6 => Self::WouldBlock,
            _ => Self::TransportError,
        }
    }
}
