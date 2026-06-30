// gos-journal-harness — V2.11 journal format and snapshot API tests
//
// Verifies that gos-journal's on-disk format layer is correct and that the
// V2.11 bug fix (decode_kind now handles all 12 ControlPlaneMessageKind
// variants, not just 0x01–0x08) is confirmed by tests:
//
//  1.  journal_constants_are_correct         — ENVELOPE_RECORD_BYTES=40, etc.
//  2.  journal_header_roundtrip              — write_into then parse → identical
//  3.  all_twelve_kinds_survive_roundtrip    — all 12 kind tags deserialize correctly
//  4.  replay_empty_journal                  — header + empty body → 0 events
//  5.  replay_three_envelopes_in_order       — insertion order preserved
//  6.  ring_append_flush_replay              — JournalRing<8> round-trip
//  7.  ring_full_returns_error               — append past capacity → Err
//  8.  ring_reset_and_reuse                  — reset after full → reappend succeeds
//  9.  snapshot_header_roundtrip             — SnapshotHeader write_into + parse
//  10. snapshot_node_roundtrip               — SnapshotNode write_into + parse
//  11. snapshot_edge_roundtrip               — SnapshotEdge write_into + parse
//  12. replay_snapshot_full_roundtrip        — multi-node multi-edge blob round-trip
//  13. replay_bad_magic_returns_bad_header   — wrong magic → BadHeader error
//  14. replay_trailing_bytes_returns_error   — partial last record → TrailingBytes error

use gos_journal::{
    deserialize_envelope, replay, replay_snapshot, serialize_envelope,
    JournalError, JournalHeader, JournalRing, SnapshotEdge, SnapshotHeader,
    SnapshotNode, ENVELOPE_RECORD_BYTES, HEADER_BYTES, SNAPSHOT_EDGE_BYTES,
    SNAPSHOT_HEADER_BYTES, SNAPSHOT_NODE_BYTES, SNAPSHOT_VERSION,
    JOURNAL_VERSION,
};
use gos_protocol::{ControlPlaneEnvelope, ControlPlaneMessageKind};

fn make_envelope(kind: ControlPlaneMessageKind, arg0: u64, arg1: u64) -> ControlPlaneEnvelope {
    ControlPlaneEnvelope {
        version: 1,
        kind,
        subject: [0xAB; 16],
        arg0,
        arg1,
    }
}

// ── Test 1 ───────────────────────────────────────────────────────────────────

#[test]
fn journal_constants_are_correct() {
    assert_eq!(ENVELOPE_RECORD_BYTES, 40);
    assert_eq!(HEADER_BYTES, 8);
    assert_eq!(SNAPSHOT_HEADER_BYTES, 24);
    assert_eq!(SNAPSHOT_NODE_BYTES, 40);
    assert_eq!(SNAPSHOT_EDGE_BYTES, 40);
    assert_eq!(JOURNAL_VERSION, 1);
    assert_eq!(SNAPSHOT_VERSION, 1);
}

// ── Test 2 ───────────────────────────────────────────────────────────────────

#[test]
fn journal_header_roundtrip() {
    let hdr = JournalHeader::current();
    let mut buf = [0u8; HEADER_BYTES];
    hdr.write_into(&mut buf);
    let parsed = JournalHeader::parse(&buf).expect("parse should succeed");
    assert_eq!(parsed.magic, *b"GOSJ");
    assert_eq!(parsed.version, JOURNAL_VERSION);
    assert_eq!(parsed.record_size as usize, ENVELOPE_RECORD_BYTES);
}

// ── Test 3 ───────────────────────────────────────────────────────────────────

#[test]
fn all_twelve_kinds_survive_roundtrip() {
    use ControlPlaneMessageKind::*;
    let kinds = [
        Hello, PluginDiscovered, NodeUpsert, EdgeUpsert,
        StateDelta, SnapshotChunk, Fault, Metric,
        MutationAudit, CausalOverflow, RuleApplied, SubscribeTriggered,
    ];
    for kind in kinds {
        let env = make_envelope(kind, 0x1122334455667788, 0xAABBCCDDEEFF0011);
        let mut record = [0u8; ENVELOPE_RECORD_BYTES];
        serialize_envelope(&env, &mut record);
        let recovered = deserialize_envelope(&record).expect("all 12 kinds must deserialize");
        assert_eq!(recovered.kind, env.kind, "kind mismatch for {kind:?}");
        assert_eq!(recovered.arg0, env.arg0);
        assert_eq!(recovered.arg1, env.arg1);
        assert_eq!(recovered.subject, env.subject);
    }
}

// ── Test 4 ───────────────────────────────────────────────────────────────────

#[test]
fn replay_empty_journal() {
    let mut blob = vec![0u8; HEADER_BYTES];
    JournalHeader::current().write_into((&mut blob[..HEADER_BYTES]).try_into().unwrap());

    let mut count = 0usize;
    let n = replay(&blob, |_| count += 1).expect("empty replay should succeed");
    assert_eq!(n, 0);
    assert_eq!(count, 0);
}

// ── Test 5 ───────────────────────────────────────────────────────────────────

#[test]
fn replay_three_envelopes_in_order() {
    use ControlPlaneMessageKind::*;
    let envs = [
        make_envelope(NodeUpsert, 1, 10),
        make_envelope(EdgeUpsert, 2, 20),
        make_envelope(Fault, 3, 30),
    ];

    let total_bytes = HEADER_BYTES + 3 * ENVELOPE_RECORD_BYTES;
    let mut blob = vec![0u8; total_bytes];
    JournalHeader::current().write_into((&mut blob[..HEADER_BYTES]).try_into().unwrap());
    for (i, env) in envs.iter().enumerate() {
        let off = HEADER_BYTES + i * ENVELOPE_RECORD_BYTES;
        let mut record = [0u8; ENVELOPE_RECORD_BYTES];
        serialize_envelope(env, &mut record);
        blob[off..off + ENVELOPE_RECORD_BYTES].copy_from_slice(&record);
    }

    let mut recovered: Vec<ControlPlaneEnvelope> = Vec::new();
    let n = replay(&blob, |e| recovered.push(e)).expect("replay should succeed");
    assert_eq!(n, 3);
    assert_eq!(recovered[0].kind, NodeUpsert);
    assert_eq!(recovered[0].arg0, 1);
    assert_eq!(recovered[1].kind, EdgeUpsert);
    assert_eq!(recovered[1].arg0, 2);
    assert_eq!(recovered[2].kind, Fault);
    assert_eq!(recovered[2].arg0, 3);
}

// ── Test 6 ───────────────────────────────────────────────────────────────────

#[test]
fn ring_append_flush_replay() {
    use ControlPlaneMessageKind::*;
    let envs = [
        make_envelope(Hello, 100, 0),
        make_envelope(PluginDiscovered, 200, 0),
        make_envelope(StateDelta, 300, 0),
    ];

    let mut ring: JournalRing<8> = JournalRing::new();
    for env in &envs {
        ring.append(env).expect("ring has capacity");
    }
    assert_eq!(ring.len(), 3);
    assert!(!ring.is_full());

    let blob_size = HEADER_BYTES + 3 * ENVELOPE_RECORD_BYTES;
    let mut blob = vec![0u8; blob_size];
    let written = ring.flush_into(&mut blob).expect("flush should succeed");
    assert_eq!(written, blob_size);

    let mut recovered: Vec<ControlPlaneEnvelope> = Vec::new();
    let n = replay(&blob, |e| recovered.push(e)).expect("replay should succeed");
    assert_eq!(n, 3);
    assert_eq!(recovered[0].kind, Hello);
    assert_eq!(recovered[0].arg0, 100);
    assert_eq!(recovered[1].kind, PluginDiscovered);
    assert_eq!(recovered[1].arg0, 200);
    assert_eq!(recovered[2].kind, StateDelta);
    assert_eq!(recovered[2].arg0, 300);
}

// ── Test 7 ───────────────────────────────────────────────────────────────────

#[test]
fn ring_full_returns_error() {
    let env = make_envelope(ControlPlaneMessageKind::Metric, 0, 0);
    let mut ring: JournalRing<2> = JournalRing::new();
    ring.append(&env).expect("first append succeeds");
    ring.append(&env).expect("second append succeeds");
    assert!(ring.is_full());
    let result = ring.append(&env);
    assert!(result.is_err(), "append past capacity should fail");
}

// ── Test 8 ───────────────────────────────────────────────────────────────────

#[test]
fn ring_reset_and_reuse() {
    let env = make_envelope(ControlPlaneMessageKind::RuleApplied, 42, 0);
    let mut ring: JournalRing<1> = JournalRing::new();
    ring.append(&env).expect("first append succeeds");
    assert!(ring.is_full());

    ring.reset();
    assert!(ring.is_empty());
    assert_eq!(ring.len(), 0);

    ring.append(&env).expect("append after reset should succeed");
    assert_eq!(ring.len(), 1);
}

// ── Test 9 ───────────────────────────────────────────────────────────────────

#[test]
fn snapshot_header_roundtrip() {
    let hdr = SnapshotHeader::new(1234567890, 47, 95);
    let mut buf = [0u8; SNAPSHOT_HEADER_BYTES];
    hdr.write_into(&mut buf);
    let parsed = SnapshotHeader::parse(&buf).expect("parse should succeed");
    assert_eq!(parsed.magic, *b"GOSS");
    assert_eq!(parsed.version, SNAPSHOT_VERSION);
    assert_eq!(parsed.captured_at_tick, 1234567890);
    assert_eq!(parsed.node_count, 47);
    assert_eq!(parsed.edge_count, 95);
    assert_eq!(parsed.flags, 0);
}

// ── Test 10 ──────────────────────────────────────────────────────────────────

#[test]
fn snapshot_node_roundtrip() {
    let node = SnapshotNode {
        node_id: [0x11; 16],
        vector: 0xDEADBEEF_CAFEBABE,
        plugin_id: [0x22; 16],
    };
    let mut buf = [0u8; SNAPSHOT_NODE_BYTES];
    node.write_into(&mut buf);
    let parsed = SnapshotNode::parse(&buf);
    assert_eq!(parsed.node_id, node.node_id);
    assert_eq!(parsed.vector, node.vector);
    assert_eq!(parsed.plugin_id, node.plugin_id);
}

// ── Test 11 ──────────────────────────────────────────────────────────────────

#[test]
fn snapshot_edge_roundtrip() {
    let edge = SnapshotEdge {
        edge_id: [0x33; 16],
        from_node_low: 0x1122334455667788,
        to_node_low: 0xAABBCCDDEEFF0011,
        edge_kind: 7,
    };
    let mut buf = [0u8; SNAPSHOT_EDGE_BYTES];
    edge.write_into(&mut buf);
    let parsed = SnapshotEdge::parse(&buf);
    assert_eq!(parsed.edge_id, edge.edge_id);
    assert_eq!(parsed.from_node_low, edge.from_node_low);
    assert_eq!(parsed.to_node_low, edge.to_node_low);
    assert_eq!(parsed.edge_kind, edge.edge_kind);
}

// ── Test 12 ──────────────────────────────────────────────────────────────────

#[test]
fn replay_snapshot_full_roundtrip() {
    let nodes = vec![
        SnapshotNode { node_id: [0xA1; 16], vector: 0x0001, plugin_id: [0xB1; 16] },
        SnapshotNode { node_id: [0xA2; 16], vector: 0x0002, plugin_id: [0xB2; 16] },
        SnapshotNode { node_id: [0xA3; 16], vector: 0x0003, plugin_id: [0xB3; 16] },
    ];
    let edges = vec![
        SnapshotEdge { edge_id: [0xE1; 16], from_node_low: 1, to_node_low: 2, edge_kind: 1 },
        SnapshotEdge { edge_id: [0xE2; 16], from_node_low: 2, to_node_low: 3, edge_kind: 3 },
    ];

    let blob_len = SNAPSHOT_HEADER_BYTES
        + nodes.len() * SNAPSHOT_NODE_BYTES
        + edges.len() * SNAPSHOT_EDGE_BYTES;
    let mut blob = vec![0u8; blob_len];

    let hdr = SnapshotHeader::new(9999, nodes.len() as u32, edges.len() as u32);
    let mut hdr_buf = [0u8; SNAPSHOT_HEADER_BYTES];
    hdr.write_into(&mut hdr_buf);
    blob[..SNAPSHOT_HEADER_BYTES].copy_from_slice(&hdr_buf);

    let mut off = SNAPSHOT_HEADER_BYTES;
    for node in &nodes {
        let mut buf = [0u8; SNAPSHOT_NODE_BYTES];
        node.write_into(&mut buf);
        blob[off..off + SNAPSHOT_NODE_BYTES].copy_from_slice(&buf);
        off += SNAPSHOT_NODE_BYTES;
    }
    for edge in &edges {
        let mut buf = [0u8; SNAPSHOT_EDGE_BYTES];
        edge.write_into(&mut buf);
        blob[off..off + SNAPSHOT_EDGE_BYTES].copy_from_slice(&buf);
        off += SNAPSHOT_EDGE_BYTES;
    }

    let mut recovered_nodes: Vec<SnapshotNode> = Vec::new();
    let mut recovered_edges: Vec<SnapshotEdge> = Vec::new();
    let recovered_hdr = replay_snapshot(
        &blob,
        |n| recovered_nodes.push(n),
        |e| recovered_edges.push(e),
    )
    .expect("replay_snapshot should succeed");

    assert_eq!(recovered_hdr.captured_at_tick, 9999);
    assert_eq!(recovered_hdr.node_count, 3);
    assert_eq!(recovered_hdr.edge_count, 2);
    assert_eq!(recovered_nodes.len(), 3);
    assert_eq!(recovered_edges.len(), 2);
    for (i, (orig, got)) in nodes.iter().zip(recovered_nodes.iter()).enumerate() {
        assert_eq!(got.node_id, orig.node_id, "node {i} id mismatch");
        assert_eq!(got.vector, orig.vector, "node {i} vector mismatch");
    }
    for (i, (orig, got)) in edges.iter().zip(recovered_edges.iter()).enumerate() {
        assert_eq!(got.edge_id, orig.edge_id, "edge {i} id mismatch");
        assert_eq!(got.edge_kind, orig.edge_kind, "edge {i} kind mismatch");
    }
}

// ── Test 13 ──────────────────────────────────────────────────────────────────

#[test]
fn replay_bad_magic_returns_bad_header() {
    let mut blob = vec![0u8; HEADER_BYTES + ENVELOPE_RECORD_BYTES];
    blob[..4].copy_from_slice(b"BAAD"); // wrong magic
    blob[4..6].copy_from_slice(&1u16.to_le_bytes());
    blob[6..8].copy_from_slice(&(ENVELOPE_RECORD_BYTES as u16).to_le_bytes());

    let result = replay(&blob, |_| {});
    assert_eq!(result, Err(JournalError::BadHeader));
}

// ── Test 14 ──────────────────────────────────────────────────────────────────

#[test]
fn replay_trailing_bytes_returns_error() {
    // A valid header but the body has 1 extra byte (not a multiple of record size).
    let total = HEADER_BYTES + ENVELOPE_RECORD_BYTES + 1;
    let mut blob = vec![0u8; total];
    JournalHeader::current().write_into((&mut blob[..HEADER_BYTES]).try_into().unwrap());

    // Write a valid record at position HEADER_BYTES
    let env = make_envelope(ControlPlaneMessageKind::Hello, 0, 0);
    let mut record = [0u8; ENVELOPE_RECORD_BYTES];
    serialize_envelope(&env, &mut record);
    blob[HEADER_BYTES..HEADER_BYTES + ENVELOPE_RECORD_BYTES].copy_from_slice(&record);
    // The extra byte at the end makes body length non-multiple of ENVELOPE_RECORD_BYTES.

    let result = replay(&blob, |_| {});
    assert_eq!(result, Err(JournalError::TrailingBytes));
}
