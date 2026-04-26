#![no_std]

//! Phase D.4 — structured logging.
//!
//! Replaces the proliferation of `raw_serial_println!` and ad-hoc
//! `serial_println!` call sites with a single `log!(level, module,
//! "msg", args...)` macro that fans out to two installable backends:
//!
//!   * **Serial backend**  — the legacy development view; receives
//!     pre-formatted bytes.  k-serial installs this at boot.
//!   * **Control-plane backend** — emits a fixed-layout `LogRecord`
//!     into the supervisor's control-plane envelope queue so the
//!     shell can subscribe to per-module log streams without going
//!     through serial.
//!
//! Both are optional — when no backend is installed (boot, host
//! tests), the macro is a no-op.  When only the serial one is up,
//! control-plane records are silently dropped — same deal in reverse.
//!
//! The module identifier is a 16-byte ASCII tag (`PluginId`-shaped),
//! so log subscription can filter by source without parsing strings.

use core::fmt::{self, Write};
use spin::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

/// Maximum bytes captured per log record.  Anything longer is
/// truncated; the record carries `truncated: bool` so the shell can
/// flag it.
pub const LOG_PAYLOAD_BYTES: usize = 192;

#[derive(Clone, Copy)]
pub struct LogRecord {
    pub level: LogLevel,
    /// 16-byte tag — matches `PluginId` layout for cheap filtering.
    pub source: [u8; 16],
    pub payload: [u8; LOG_PAYLOAD_BYTES],
    pub payload_len: u16,
    pub truncated: bool,
}

impl LogRecord {
    pub const fn empty() -> Self {
        Self {
            level: LogLevel::Trace,
            source: [0; 16],
            payload: [0; LOG_PAYLOAD_BYTES],
            payload_len: 0,
            truncated: false,
        }
    }

    pub fn payload_str(&self) -> &[u8] {
        &self.payload[..self.payload_len as usize]
    }
}

/// Backend that consumes pre-formatted bytes (typically prints them
/// over a serial port).
#[derive(Clone, Copy)]
pub struct SerialBackend {
    pub write: unsafe extern "C" fn(level: u8, source: *const u8, msg: *const u8, len: u32),
}

/// Backend that consumes structured records (typically pushes them
/// into a runtime control-plane queue or a per-module ring buffer).
#[derive(Clone, Copy)]
pub struct StructuredBackend {
    pub publish: unsafe extern "C" fn(record: *const LogRecord),
}

static SERIAL_BACKEND: Mutex<Option<SerialBackend>> = Mutex::new(None);
static STRUCTURED_BACKEND: Mutex<Option<StructuredBackend>> = Mutex::new(None);
static MIN_LEVEL: Mutex<LogLevel> = Mutex::new(LogLevel::Info);

pub fn install_serial_backend(backend: SerialBackend) {
    *SERIAL_BACKEND.lock() = Some(backend);
}

pub fn install_structured_backend(backend: StructuredBackend) {
    *STRUCTURED_BACKEND.lock() = Some(backend);
}

pub fn set_min_level(level: LogLevel) {
    *MIN_LEVEL.lock() = level;
}

pub fn min_level() -> LogLevel {
    *MIN_LEVEL.lock()
}

#[doc(hidden)]
pub fn dispatch(level: LogLevel, source: [u8; 16], args: fmt::Arguments<'_>) {
    if level < min_level() {
        return;
    }
    let mut buf = LogBuf::new();
    let _ = buf.write_fmt(args);

    if let Some(backend) = *SERIAL_BACKEND.lock() {
        unsafe {
            (backend.write)(
                level as u8,
                source.as_ptr(),
                buf.bytes.as_ptr(),
                buf.len as u32,
            );
        }
    }

    if let Some(backend) = *STRUCTURED_BACKEND.lock() {
        let mut record = LogRecord::empty();
        record.level = level;
        record.source = source;
        let n = (buf.len).min(LOG_PAYLOAD_BYTES);
        record.payload[..n].copy_from_slice(&buf.bytes[..n]);
        record.payload_len = n as u16;
        record.truncated = buf.truncated;
        unsafe {
            (backend.publish)(&record as *const _);
        }
    }
}

/// Formatting buffer — bounded so `gos-log` can run pre-allocator (the
/// kernel logs from inside k-heap's own `on_init`, before any global
/// allocator exists).
struct LogBuf {
    bytes: [u8; LOG_PAYLOAD_BYTES],
    len: usize,
    truncated: bool,
}

impl LogBuf {
    fn new() -> Self {
        Self {
            bytes: [0; LOG_PAYLOAD_BYTES],
            len: 0,
            truncated: false,
        }
    }
}

impl Write for LogBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let space = LOG_PAYLOAD_BYTES.saturating_sub(self.len);
        let n = bytes.len().min(space);
        self.bytes[self.len..self.len + n].copy_from_slice(&bytes[..n]);
        self.len += n;
        if n < bytes.len() {
            self.truncated = true;
        }
        Ok(())
    }
}

/// `log!(level, source: [u8;16], "format string", args...)`
///
/// Source is typically a `PluginId` (`pub struct PluginId(pub [u8; 16])`)
/// or a 16-byte ASCII literal (`*b"K_HEAP\0\0\0\0\0\0\0\0\0\0"`).
#[macro_export]
macro_rules! log {
    ($level:expr, $source:expr, $($arg:tt)*) => {
        $crate::dispatch($level, $source, ::core::format_args!($($arg)*))
    };
}

