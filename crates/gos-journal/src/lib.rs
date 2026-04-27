#![no_std]

//! Phase F.4 — graph state journal.
//!
//! The supervisor's control-plane envelope queue already records every
//! significant state transition: PluginDiscovered, NodeUpsert,
//! EdgeUpsert, StateDelta, Fault, Metric.  A persisted append-only
//! log of those envelopes — replayed on boot — lets the kernel recover
//! the runtime graph without re-running every plugin's boot side
//! effects.
//!
//! This slice ships:
//!
//!   * A fixed 40-byte on-disk format for `ControlPlaneEnvelope`.
//!   * A 4-byte magic + 2-byte version + 2-byte entry-size header
//!     (`JournalHeader`) that bookends the log file.
//!   * `JournalWriter` — in-memory append buffer that emits one big
//!     blob ready to hand to a `gos_vfs::FileSystem` write path
//!     once F.5 lands.
//!   * `replay` — walks a blob and calls back into the supervisor.
//!
//! The on-disk format is *deliberately not* the in-memory layout:
//! `ControlPlaneEnvelope` is `#[derive(Debug, Clone, Copy)]` without
//! `#[repr(C)]`, so a future change to its in-memory padding wouldn't
//! silently corrupt journal files.

use gos_protocol::{ControlPlaneEnvelope, ControlPlaneMessageKind};

pub const JOURNAL_MAGIC: [u8; 4] = *b"GOSJ";
pub const JOURNAL_VERSION: u16 = 1;
/// Fixed on-disk size of one envelope record.  See `serialize_envelope`.
pub const ENVELOPE_RECORD_BYTES: usize = 40;
pub const HEADER_BYTES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalError {
    /// The blob is shorter than `HEADER_BYTES` or doesn't carry the
    /// `GOSJ` magic.
    BadHeader,
    /// Version mismatch — caller is too old/new for this journal.
    UnsupportedVersion(u16),
    /// Record size advertised in the header doesn't match
    /// `ENVELOPE_RECORD_BYTES`.
    UnsupportedRecordSize(u16),
    /// Trailing bytes after the last full record were left over.
    TrailingBytes,
    /// Unknown ControlPlaneMessageKind value during deserialize.
    UnknownKind(u8),
}

#[derive(Debug, Clone, Copy)]
pub struct JournalHeader {
    pub magic: [u8; 4],
    pub version: u16,
    pub record_size: u16,
}

impl JournalHeader {
    pub const fn current() -> Self {
        Self {
            magic: JOURNAL_MAGIC,
            version: JOURNAL_VERSION,
            record_size: ENVELOPE_RECORD_BYTES as u16,
        }
    }

    pub fn write_into(&self, out: &mut [u8; HEADER_BYTES]) {
        out[..4].copy_from_slice(&self.magic);
        out[4..6].copy_from_slice(&self.version.to_le_bytes());
        out[6..8].copy_from_slice(&self.record_size.to_le_bytes());
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, JournalError> {
        if bytes.len() < HEADER_BYTES {
            return Err(JournalError::BadHeader);
        }
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[..4]);
        if magic != JOURNAL_MAGIC {
            return Err(JournalError::BadHeader);
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != JOURNAL_VERSION {
            return Err(JournalError::UnsupportedVersion(version));
        }
        let record_size = u16::from_le_bytes([bytes[6], bytes[7]]);
        if record_size as usize != ENVELOPE_RECORD_BYTES {
            return Err(JournalError::UnsupportedRecordSize(record_size));
        }
        Ok(Self {
            magic,
            version,
            record_size,
        })
    }
}

/// Serialize an envelope into the fixed 40-byte on-disk record.
///
/// Layout (little-endian):
///   0..2   version
///   2..3   kind (u8 enum tag)
///   3..4   reserved padding
///   4..20  subject (16 bytes)
///   20..28 arg0 (u64)
///   28..36 arg1 (u64)
///   36..40 reserved padding (zero-filled)
pub fn serialize_envelope(env: &ControlPlaneEnvelope, out: &mut [u8; ENVELOPE_RECORD_BYTES]) {
    out[..2].copy_from_slice(&env.version.to_le_bytes());
    out[2] = env.kind as u8;
    out[3] = 0;
    out[4..20].copy_from_slice(&env.subject);
    out[20..28].copy_from_slice(&env.arg0.to_le_bytes());
    out[28..36].copy_from_slice(&env.arg1.to_le_bytes());
    out[36..40].copy_from_slice(&[0u8; 4]);
}

pub fn deserialize_envelope(
    record: &[u8; ENVELOPE_RECORD_BYTES],
) -> Result<ControlPlaneEnvelope, JournalError> {
    let version = u16::from_le_bytes([record[0], record[1]]);
    let kind = decode_kind(record[2])?;
    let mut subject = [0u8; 16];
    subject.copy_from_slice(&record[4..20]);
    let arg0 = u64::from_le_bytes([
        record[20], record[21], record[22], record[23], record[24], record[25], record[26],
        record[27],
    ]);
    let arg1 = u64::from_le_bytes([
        record[28], record[29], record[30], record[31], record[32], record[33], record[34],
        record[35],
    ]);
    Ok(ControlPlaneEnvelope {
        version,
        kind,
        subject,
        arg0,
        arg1,
    })
}

fn decode_kind(raw: u8) -> Result<ControlPlaneMessageKind, JournalError> {
    use ControlPlaneMessageKind::*;
    Ok(match raw {
        0x01 => Hello,
        0x02 => PluginDiscovered,
        0x03 => NodeUpsert,
        0x04 => EdgeUpsert,
        0x05 => StateDelta,
        0x06 => SnapshotChunk,
        0x07 => Fault,
        0x08 => Metric,
        other => return Err(JournalError::UnknownKind(other)),
    })
}

/// Walk a journal blob (header + N records) and call `sink` for each
/// envelope encountered.  Returns the number of envelopes replayed.
pub fn replay<F>(blob: &[u8], mut sink: F) -> Result<usize, JournalError>
where
    F: FnMut(ControlPlaneEnvelope),
{
    let _header = JournalHeader::parse(blob)?;
    let body = &blob[HEADER_BYTES..];
    if !body.len().is_multiple_of(ENVELOPE_RECORD_BYTES) {
        return Err(JournalError::TrailingBytes);
    }
    let mut count = 0usize;
    let mut cur = 0usize;
    while cur + ENVELOPE_RECORD_BYTES <= body.len() {
        let mut record = [0u8; ENVELOPE_RECORD_BYTES];
        record.copy_from_slice(&body[cur..cur + ENVELOPE_RECORD_BYTES]);
        let env = deserialize_envelope(&record)?;
        sink(env);
        count += 1;
        cur += ENVELOPE_RECORD_BYTES;
    }
    Ok(count)
}

/// Bounded in-memory append buffer.  Once full, `append` returns
/// `Err(JournalError::BadHeader)` until the caller `flush_into`s the
/// buffer to disk and calls `reset`.
///
/// Generic over capacity so callers (kernel boot, host harness) can
/// pick a size appropriate to their working set.  N counts envelopes,
/// not bytes — total buffer size is N * 40.
pub struct JournalRing<const N: usize> {
    records: [[u8; ENVELOPE_RECORD_BYTES]; N],
    len: usize,
}

impl<const N: usize> JournalRing<N> {
    pub const fn new() -> Self {
        Self {
            records: [[0u8; ENVELOPE_RECORD_BYTES]; N],
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_full(&self) -> bool {
        self.len == N
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn append(&mut self, env: &ControlPlaneEnvelope) -> Result<(), JournalError> {
        if self.is_full() {
            return Err(JournalError::TrailingBytes); // re-using the variant for "no room"
        }
        serialize_envelope(env, &mut self.records[self.len]);
        self.len += 1;
        Ok(())
    }

    /// Write header + all buffered records into `out`.  Returns the
    /// number of bytes written.  If `out` is too small, returns
    /// `Err(JournalError::BadHeader)` (re-using for "no buffer").
    pub fn flush_into(&self, out: &mut [u8]) -> Result<usize, JournalError> {
        let total = HEADER_BYTES + self.len * ENVELOPE_RECORD_BYTES;
        if out.len() < total {
            return Err(JournalError::BadHeader);
        }
        let mut header = [0u8; HEADER_BYTES];
        JournalHeader::current().write_into(&mut header);
        out[..HEADER_BYTES].copy_from_slice(&header);
        for (i, record) in self.records[..self.len].iter().enumerate() {
            let off = HEADER_BYTES + i * ENVELOPE_RECORD_BYTES;
            out[off..off + ENVELOPE_RECORD_BYTES].copy_from_slice(record);
        }
        Ok(total)
    }

    pub fn reset(&mut self) {
        self.len = 0;
    }
}

// ── Phase H.5 — runtime snapshot ────────────────────────────────────────────
//
// The journal records *deltas*: every node/edge/state transition since
// boot.  A reboot replay reconstructs the graph by starting from the
// initial builtin manifest set and applying every envelope.  That's
// fine when the journal is small, but a long-lived deployment needs
// periodic *snapshots* so replay doesn't have to scan years of
// history.
//
// A snapshot is the runtime's whole observable state at one instant —
// every live node, every live edge, every instance — encoded into a
// single blob.  Replay strategy becomes:
//
//   1. Find the most-recent snapshot blob.
//   2. Restore from it.
//   3. Apply only the journal entries that follow the snapshot's
//      tick.
//
// This slice ships the *format* + the type-system contract.  Real
// disk binding (write the blob through gos-vfs, periodic snapshot
// scheduler) is a follow-up F.5.x integration slice.

pub const SNAPSHOT_MAGIC: [u8; 4] = *b"GOSS";
pub const SNAPSHOT_VERSION: u16 = 1;
pub const SNAPSHOT_HEADER_BYTES: usize = 24;

#[derive(Debug, Clone, Copy)]
pub struct SnapshotHeader {
    pub magic: [u8; 4],
    pub version: u16,
    /// Reserved for future flags (compression, encryption marker).
    pub flags: u16,
    /// Tick at which the snapshot was captured.  Replay applies only
    /// journal envelopes whose `tick > snapshot.captured_at_tick`.
    pub captured_at_tick: u64,
    /// Number of node records following the header.
    pub node_count: u32,
    /// Number of edge records following the nodes section.
    pub edge_count: u32,
}

impl SnapshotHeader {
    pub fn new(tick: u64, node_count: u32, edge_count: u32) -> Self {
        Self {
            magic: SNAPSHOT_MAGIC,
            version: SNAPSHOT_VERSION,
            flags: 0,
            captured_at_tick: tick,
            node_count,
            edge_count,
        }
    }

    pub fn write_into(&self, out: &mut [u8; SNAPSHOT_HEADER_BYTES]) {
        out[..4].copy_from_slice(&self.magic);
        out[4..6].copy_from_slice(&self.version.to_le_bytes());
        out[6..8].copy_from_slice(&self.flags.to_le_bytes());
        out[8..16].copy_from_slice(&self.captured_at_tick.to_le_bytes());
        out[16..20].copy_from_slice(&self.node_count.to_le_bytes());
        out[20..24].copy_from_slice(&self.edge_count.to_le_bytes());
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, JournalError> {
        if bytes.len() < SNAPSHOT_HEADER_BYTES {
            return Err(JournalError::BadHeader);
        }
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[..4]);
        if magic != SNAPSHOT_MAGIC {
            return Err(JournalError::BadHeader);
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != SNAPSHOT_VERSION {
            return Err(JournalError::UnsupportedVersion(version));
        }
        let flags = u16::from_le_bytes([bytes[6], bytes[7]]);
        let captured_at_tick = u64::from_le_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14],
            bytes[15],
        ]);
        let node_count = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let edge_count = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        Ok(Self {
            magic,
            version,
            flags,
            captured_at_tick,
            node_count,
            edge_count,
        })
    }
}

/// One node record inside a snapshot blob.  Layout (40 bytes):
///   0..16   node_id (16 bytes)
///   16..24  vector (u64 packed)
///   24..32  plugin_id_low (8 bytes — first half of PluginId)
///   32..40  plugin_id_high
pub const SNAPSHOT_NODE_BYTES: usize = 40;

#[derive(Debug, Clone, Copy)]
pub struct SnapshotNode {
    pub node_id: [u8; 16],
    pub vector: u64,
    pub plugin_id: [u8; 16],
}

impl SnapshotNode {
    pub fn write_into(&self, out: &mut [u8; SNAPSHOT_NODE_BYTES]) {
        out[..16].copy_from_slice(&self.node_id);
        out[16..24].copy_from_slice(&self.vector.to_le_bytes());
        out[24..40].copy_from_slice(&self.plugin_id);
    }

    pub fn parse(record: &[u8; SNAPSHOT_NODE_BYTES]) -> Self {
        let mut node_id = [0u8; 16];
        node_id.copy_from_slice(&record[..16]);
        let vector = u64::from_le_bytes([
            record[16], record[17], record[18], record[19], record[20], record[21], record[22],
            record[23],
        ]);
        let mut plugin_id = [0u8; 16];
        plugin_id.copy_from_slice(&record[24..40]);
        Self {
            node_id,
            vector,
            plugin_id,
        }
    }
}

/// One edge record inside a snapshot blob.  Layout (40 bytes):
///   0..16   edge_id
///   16..24  from_node low (8 bytes)
///   24..32  to_node low (8 bytes)
///   32..36  edge_kind (u32; matches RuntimeEdgeType repr)
///   36..40  reserved
pub const SNAPSHOT_EDGE_BYTES: usize = 40;

#[derive(Debug, Clone, Copy)]
pub struct SnapshotEdge {
    pub edge_id: [u8; 16],
    pub from_node_low: u64,
    pub to_node_low: u64,
    pub edge_kind: u32,
}

impl SnapshotEdge {
    pub fn write_into(&self, out: &mut [u8; SNAPSHOT_EDGE_BYTES]) {
        out[..16].copy_from_slice(&self.edge_id);
        out[16..24].copy_from_slice(&self.from_node_low.to_le_bytes());
        out[24..32].copy_from_slice(&self.to_node_low.to_le_bytes());
        out[32..36].copy_from_slice(&self.edge_kind.to_le_bytes());
        out[36..40].copy_from_slice(&[0u8; 4]);
    }

    pub fn parse(record: &[u8; SNAPSHOT_EDGE_BYTES]) -> Self {
        let mut edge_id = [0u8; 16];
        edge_id.copy_from_slice(&record[..16]);
        let from_node_low = u64::from_le_bytes([
            record[16], record[17], record[18], record[19], record[20], record[21], record[22],
            record[23],
        ]);
        let to_node_low = u64::from_le_bytes([
            record[24], record[25], record[26], record[27], record[28], record[29], record[30],
            record[31],
        ]);
        let edge_kind = u32::from_le_bytes([record[32], record[33], record[34], record[35]]);
        Self {
            edge_id,
            from_node_low,
            to_node_low,
            edge_kind,
        }
    }
}

/// Walk a snapshot blob, calling `on_node` for each `SnapshotNode`
/// then `on_edge` for each `SnapshotEdge`.  Returns the
/// `SnapshotHeader` once the blob has been fully traversed (or an
/// error if the layout is corrupt).
pub fn replay_snapshot<N, E>(
    blob: &[u8],
    mut on_node: N,
    mut on_edge: E,
) -> Result<SnapshotHeader, JournalError>
where
    N: FnMut(SnapshotNode),
    E: FnMut(SnapshotEdge),
{
    let header = SnapshotHeader::parse(blob)?;
    let mut cur = SNAPSHOT_HEADER_BYTES;
    for _ in 0..header.node_count {
        if cur + SNAPSHOT_NODE_BYTES > blob.len() {
            return Err(JournalError::TrailingBytes);
        }
        let mut record = [0u8; SNAPSHOT_NODE_BYTES];
        record.copy_from_slice(&blob[cur..cur + SNAPSHOT_NODE_BYTES]);
        on_node(SnapshotNode::parse(&record));
        cur += SNAPSHOT_NODE_BYTES;
    }
    for _ in 0..header.edge_count {
        if cur + SNAPSHOT_EDGE_BYTES > blob.len() {
            return Err(JournalError::TrailingBytes);
        }
        let mut record = [0u8; SNAPSHOT_EDGE_BYTES];
        record.copy_from_slice(&blob[cur..cur + SNAPSHOT_EDGE_BYTES]);
        on_edge(SnapshotEdge::parse(&record));
        cur += SNAPSHOT_EDGE_BYTES;
    }
    if cur != blob.len() {
        return Err(JournalError::TrailingBytes);
    }
    Ok(header)
}
