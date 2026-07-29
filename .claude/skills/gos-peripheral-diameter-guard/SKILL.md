---
name: gos-peripheral-diameter-guard
description: When filtering graph boundary nodes by equality with a global scalar (peripheral: ecc==diameter, center: ecc==radius), always guard with `scalar > 0` first — without it, all-isolated graphs (ecc=0 for all, scalar=0) falsely match every node because 0==0. Also: compute radius as min-nonzero using a u32::MAX sentinel, then collapse to 0.
---

# Graph Boundary Nodes: `scalar > 0` Guard + min-nonzero Sentinel

## The rule — peripheral (max equality)

When collecting peripheral nodes (`ecc[v] == diameter`), guard with `diameter > 0`:

```rust
// CORRECT
if diameter > 0 && ecc[s] == diameter {
    periph_slots[periph_count] = s;
    periph_count += 1;
}

// WRONG — matches 0==0 for every isolated node when diameter=0
if ecc[s] == diameter { ... }
```

## The rule — center (min-nonzero equality)

When collecting center nodes (`ecc[v] == radius`), compute radius via u32::MAX sentinel,
then guard with `radius > 0`:

```rust
// Step 1: compute radius = min nonzero ecc
let mut radius: u32 = u32::MAX;
for si in 0..node_count {
    let s = node_slots[si];
    if ecc[s] > 0 && ecc[s] < radius { radius = ecc[s]; }
}
if radius == u32::MAX { radius = 0; }   // all isolated → collapse sentinel to 0

// Step 2: collect center nodes
if radius > 0 && ecc[s] == radius { ... }
```

The `ecc[s] > 0` filter in Step 1 ensures isolated nodes (ecc=0) never lower the radius.
The `radius == u32::MAX` collapse means "no non-isolated node found" → radius=0 (empty).
The `radius > 0` guard in Step 2 prevents any match when all nodes are isolated.

## Why it's non-obvious

Both peripheral and center use `ecc[v] == some_global_scalar`, but the scalar is computed
differently:
- **Peripheral**: `diameter = max(all ecc)` — includes ecc=0, so diameter=0 when all isolated
- **Center**: `radius = min(nonzero ecc)` — must explicitly exclude ecc=0 or isolated nodes contaminate the min

Without the `ecc[s] > 0` filter in the radius loop, isolated nodes (ecc=0) set radius=0,
which then matches every node in the center filter. Same bug, different mechanism.

The u32::MAX sentinel is the idiomatic GOSKernel pattern: initialize to MAX, take min while excluding zeros, then collapse MAX→0 to signal "all isolated".

## GOSKernel context

- Peripheral (V2.72): `crates/gos-runtime/src/lib.rs`, `graph_peripheral_inner<N>()`
  - Public: `graph_peripheral<N>() -> ([VectorAddress; N], [u32; N], usize, usize, u32)`
  - Shell: "graph peripheral" / "gperiph"; L4=48
- Center (V2.73): `crates/gos-runtime/src/lib.rs`, `graph_center_inner<N>()`
  - Public: `graph_center<N>() -> ([VectorAddress; N], [u32; N], usize, usize, u32)`
  - Shell: "graph center" / "gcenter"; L4=49
- Same signature shape: `(vecs, ecc, boundary_count, node_count, scalar)`
- Both sort boundary nodes ascending by `VectorAddress.as_u64()`

## From this session

V2.72 (peripheral): `diameter > 0` guard added for all-isolated edge case.
V2.73 (center): `radius > 0` guard + u32::MAX sentinel added. All 10 tests on first run.
Test 2 in both harnesses (`isolated_node_not_*`) locks in the invariant: single isolated node → count=0.
