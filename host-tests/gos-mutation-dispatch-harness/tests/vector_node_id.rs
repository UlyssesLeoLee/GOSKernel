//! ADR-008 option B — `node_id_for_vector`: a pure, deterministic
//! `VectorAddress -> gos_protocol::NodeId` derivation, giving host-harness
//! tests a concrete `NodeId` for any of the 22 builtin modules that today
//! are only addressed by a `VectorAddress` (e.g. `k_vga::NODE_VEC`), instead
//! of a hand-picked placeholder literal that carries no relationship to the
//! module it stands for.
//!
//! These tests pin down the two properties the ADR requires of the mapping:
//! injectivity (distinct `VectorAddress` values never collide) and
//! determinism (calling it twice on the same `VectorAddress` is the same
//! `NodeId`). Neither `route_signal` nor any dispatch path is touched here —
//! see `doc/ADR-008` §一 for why that's out of scope until B.4.6.
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "vector_node_id.rs", type: "file", language: "rust"}),
//!   (nifv:Function {name: "node_id_for_vector", type: "function", visibility: "pub"}),
//!   (t1:Function {name: "distinct_vectors_map_to_distinct_node_ids", type: "function"}),
//!   (t2:Function {name: "same_vector_always_maps_to_the_same_node_id", type: "function"}),
//!   (t3:Function {name: "zero_vector_maps_to_a_stable_non_zero_node_id", type: "function"}),
//!   (t4:Function {name: "adjacent_vectors_do_not_collide", type: "function"}),
//!   (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2), (f)-[:CONTAINS]->(t3), (f)-[:CONTAINS]->(t4),
//!   (t1)-[:USES]->(nifv), (t2)-[:USES]->(nifv), (t3)-[:USES]->(nifv), (t4)-[:USES]->(nifv);
//! ```

use gos_mutation_dispatch::capability::node_id_for_vector;
use gos_protocol::VectorAddress;

/// A handful of `VectorAddress`es spanning every field, mirroring the shape
/// of real builtin `NODE_VEC` constants (`k_vga`, `k_vk_host`, ... all use
/// small, hand-picked l4/l3/l2/offset values).
fn sample_vectors() -> [VectorAddress; 8] {
    [
        VectorAddress::new(0, 0, 0, 0),
        VectorAddress::new(1, 0, 0, 0),
        VectorAddress::new(6, 1, 0, 0),   // shape of k_vga::NODE_VEC-style constants
        VectorAddress::new(6, 7, 0, 0),
        VectorAddress::new(0xBF, 0, 0, 0),
        VectorAddress::new(0xBE, 0, 0, 0),
        VectorAddress::new(0xFF, 0xFFF, 0xFFF, 0xFFF),
        VectorAddress::new(0x10, 0x0ABC, 0x0DEF, 0x0123),
    ]
}

#[test]
fn distinct_vectors_map_to_distinct_node_ids() {
    let vectors = sample_vectors();
    for i in 0..vectors.len() {
        for j in 0..vectors.len() {
            if i == j {
                continue;
            }
            assert_ne!(
                node_id_for_vector(vectors[i]),
                node_id_for_vector(vectors[j]),
                "vectors[{i}]={:?} and vectors[{j}]={:?} must not collide",
                vectors[i],
                vectors[j]
            );
        }
    }
}

#[test]
fn same_vector_always_maps_to_the_same_node_id() {
    for v in sample_vectors() {
        assert_eq!(node_id_for_vector(v), node_id_for_vector(v));
    }
}

#[test]
fn zero_vector_maps_to_a_stable_non_zero_node_id() {
    // The all-zero VectorAddress is a legitimate value (l4=0 is not reserved
    // the way ADR-005's provisional-node tag 0xC0 is) -- it must still
    // produce a well-formed, non-placeholder NodeId, not gos_protocol::NodeId::ZERO.
    let id = node_id_for_vector(VectorAddress::new(0, 0, 0, 0));
    assert_ne!(id, gos_protocol::NodeId::ZERO);
    assert_eq!(id, node_id_for_vector(VectorAddress::new(0, 0, 0, 0)));
}

#[test]
fn adjacent_vectors_do_not_collide() {
    // Off-by-one in any single field must still produce a distinct NodeId --
    // guards against an encoding that accidentally truncates or ignores a
    // field.
    let base = VectorAddress::new(6, 1, 0, 0);
    let bump_l4 = VectorAddress::new(7, 1, 0, 0);
    let bump_l3 = VectorAddress::new(6, 2, 0, 0);
    let bump_l2 = VectorAddress::new(6, 1, 1, 0);
    let bump_offset = VectorAddress::new(6, 1, 0, 1);

    let base_id = node_id_for_vector(base);
    assert_ne!(base_id, node_id_for_vector(bump_l4));
    assert_ne!(base_id, node_id_for_vector(bump_l3));
    assert_ne!(base_id, node_id_for_vector(bump_l2));
    assert_ne!(base_id, node_id_for_vector(bump_offset));
}
