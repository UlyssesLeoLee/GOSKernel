// gos-uname-harness — V2.28 RuntimeCapacity / uname tests
//
// Verifies the runtime_capacity() API added to gos-runtime in V2.28.
// Analogous to `sysctl kern.*` / `getrlimit` on Linux:
// a stable, queryable view of the kernel's compile-time capacity limits.
//
//  1. max_nodes matches the MAX_NODES constant.
//  2. max_edges matches the MAX_EDGES constant.
//  3. max_plugins matches the MAX_PLUGINS constant.
//  4. max_ready_queue matches the MAX_READY_QUEUE constant.
//  5. max_signal_queue matches the MAX_SIGNAL_QUEUE constant.
//  6. max_fault_queue matches the MAX_FAULT_QUEUE constant.
//  7. max_diff_ring matches the MAX_DIFF_RING constant.
//  8. max_node_trace matches the MAX_NODE_TRACE constant.
//  9. max_node_log matches the MAX_NODE_LOG constant.
// 10. abi_major == 2 and protocol_version == 1.

use gos_runtime::{
    MAX_NODES, MAX_EDGES, MAX_PLUGINS,
    MAX_READY_QUEUE, MAX_SIGNAL_QUEUE, MAX_FAULT_QUEUE,
    MAX_DIFF_RING, MAX_NODE_TRACE, MAX_NODE_LOG,
};
use gos_protocol::{GOS_ABI_MAJOR, GOS_ABI_MINOR, CONTROL_PLANE_PROTOCOL_VERSION};

// ── 1. max_nodes matches MAX_NODES ──────────────────────────────────────────

#[test]
fn capacity_max_nodes_matches_constant() {
    let cap = gos_runtime::runtime_capacity();
    assert_eq!(
        cap.max_nodes, MAX_NODES,
        "RuntimeCapacity.max_nodes must equal MAX_NODES ({})",
        MAX_NODES
    );
}

// ── 2. max_edges matches MAX_EDGES ──────────────────────────────────────────

#[test]
fn capacity_max_edges_matches_constant() {
    let cap = gos_runtime::runtime_capacity();
    assert_eq!(
        cap.max_edges, MAX_EDGES,
        "RuntimeCapacity.max_edges must equal MAX_EDGES ({})",
        MAX_EDGES
    );
}

// ── 3. max_plugins matches MAX_PLUGINS ──────────────────────────────────────

#[test]
fn capacity_max_plugins_matches_constant() {
    let cap = gos_runtime::runtime_capacity();
    assert_eq!(
        cap.max_plugins, MAX_PLUGINS,
        "RuntimeCapacity.max_plugins must equal MAX_PLUGINS ({})",
        MAX_PLUGINS
    );
}

// ── 4. max_ready_queue matches MAX_READY_QUEUE ──────────────────────────────

#[test]
fn capacity_max_ready_queue_matches_constant() {
    let cap = gos_runtime::runtime_capacity();
    assert_eq!(
        cap.max_ready_queue, MAX_READY_QUEUE,
        "RuntimeCapacity.max_ready_queue must equal MAX_READY_QUEUE ({})",
        MAX_READY_QUEUE
    );
}

// ── 5. max_signal_queue matches MAX_SIGNAL_QUEUE ────────────────────────────

#[test]
fn capacity_max_signal_queue_matches_constant() {
    let cap = gos_runtime::runtime_capacity();
    assert_eq!(
        cap.max_signal_queue, MAX_SIGNAL_QUEUE,
        "RuntimeCapacity.max_signal_queue must equal MAX_SIGNAL_QUEUE ({})",
        MAX_SIGNAL_QUEUE
    );
}

// ── 6. max_fault_queue matches MAX_FAULT_QUEUE ──────────────────────────────

#[test]
fn capacity_max_fault_queue_matches_constant() {
    let cap = gos_runtime::runtime_capacity();
    assert_eq!(
        cap.max_fault_queue, MAX_FAULT_QUEUE,
        "RuntimeCapacity.max_fault_queue must equal MAX_FAULT_QUEUE ({})",
        MAX_FAULT_QUEUE
    );
}

// ── 7. max_diff_ring matches MAX_DIFF_RING ──────────────────────────────────

#[test]
fn capacity_max_diff_ring_matches_constant() {
    let cap = gos_runtime::runtime_capacity();
    assert_eq!(
        cap.max_diff_ring, MAX_DIFF_RING,
        "RuntimeCapacity.max_diff_ring must equal MAX_DIFF_RING ({})",
        MAX_DIFF_RING
    );
}

// ── 8. max_node_trace matches MAX_NODE_TRACE ────────────────────────────────

#[test]
fn capacity_max_node_trace_matches_constant() {
    let cap = gos_runtime::runtime_capacity();
    assert_eq!(
        cap.max_node_trace, MAX_NODE_TRACE,
        "RuntimeCapacity.max_node_trace must equal MAX_NODE_TRACE ({})",
        MAX_NODE_TRACE
    );
}

// ── 9. max_node_log matches MAX_NODE_LOG ────────────────────────────────────

#[test]
fn capacity_max_node_log_matches_constant() {
    let cap = gos_runtime::runtime_capacity();
    assert_eq!(
        cap.max_node_log, MAX_NODE_LOG,
        "RuntimeCapacity.max_node_log must equal MAX_NODE_LOG ({})",
        MAX_NODE_LOG
    );
}

// ── 10. abi_major == GOS_ABI_MAJOR and protocol_version == CONTROL_PLANE_PROTOCOL_VERSION

#[test]
fn capacity_abi_and_protocol_version_correct() {
    let cap = gos_runtime::runtime_capacity();
    assert_eq!(
        cap.abi_major, GOS_ABI_MAJOR,
        "abi_major must equal GOS_ABI_MAJOR ({})",
        GOS_ABI_MAJOR
    );
    assert_eq!(
        cap.abi_minor, GOS_ABI_MINOR,
        "abi_minor must equal GOS_ABI_MINOR ({})",
        GOS_ABI_MINOR
    );
    assert_eq!(
        cap.protocol_version, CONTROL_PLANE_PROTOCOL_VERSION,
        "protocol_version must equal CONTROL_PLANE_PROTOCOL_VERSION ({})",
        CONTROL_PLANE_PROTOCOL_VERSION
    );
}
