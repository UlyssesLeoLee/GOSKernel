# Hardening Log V2.84 — Graph Link Prediction (CN / Jaccard / Adamic-Adar / Resource Allocation)

**Date:** 2026-07-04  
**Branch:** feat/vk-auto-live-surface  
**Commit:** f7f9fc7  
**Host-test total:** 813 (803 prior + 10 new)

---

## Feature: `graph predict <u> <v>` / `gpredict <u> <v>`

### Motivation

Production graph analysis platforms (NetworkX, igraph, Neo4j GDS) provide **link prediction**
metrics that quantify how likely a missing edge between two nodes is to form, based on the
shared structure of their neighbourhoods. GOSKernel already had rich topology analytics
(centrality, efficiency, community detection) but lacked any mechanism to reason about
*potential* future connections in the kernel dependency graph.

V2.84 adds four classical link-prediction scores from the Liben-Nowell & Kleinberg (2003)
and Adamic & Adar (2003) literature:

| Metric | Formula | Interpretation |
|---|---|---|
| Common Neighbors (CN) | \|N(u) ∩ N(v)\| | count of shared neighbours |
| Jaccard Coefficient | CN / \|N(u) ∪ N(v)\| | normalised overlap |
| Adamic-Adar (AA) | Σ_{w∈CN} 1/ln(deg(w)) | inverse-log-degree weighted |
| Resource Allocation (RA) | Σ_{w∈CN} 1/deg(w) | capacity-weighted |

All scores are higher → stronger prediction that a missing edge u→v will form.

OS analogy: LLDP / CDP neighbour-table prediction — which kernel subsystems are structurally
primed to form a new dependency edge?

---

## Implementation

### crates/gos-runtime/src/lib.rs

**New method** on `GraphRuntime` (inside `impl GraphRuntime`):
```rust
pub fn graph_link_predict_inner(
    &self,
    u: VectorAddress,
    v: VectorAddress,
) -> (usize, u32, u32, u32, usize)
// returns (cn, jaccard_ppm, aa_ppm, ra_ppm, node_count)
```

**Algorithm:**
1. Resolve u and v to node slots; return all-zeros if either is unknown or u == v.
2. Scan all edges once (O(E)) to build two 128-bit neighbour bitvectors (`[u64; 2]`)
   and accumulate per-slot total undirected degree (`deg[slot]`).
3. Clear u and v from each other's bitvectors (mutual exclusion).
4. Intersection bit-count → CN; union bit-count → |N(u) ∪ N(v)| for Jaccard.
5. Bit-scan the intersection word-by-word for AA and RA accumulation:
   - AA: uses embedded LN_TABLE[k] (same table as V2.77/V2.80); term = 1e12/LN_TABLE[deg] (ppm).
   - RA: term = 1_000_000 / deg(w).
6. Saturate accumulators to u32::MAX to prevent wrapping.

**New public function:**
```rust
pub fn graph_link_predict(u: VectorAddress, v: VectorAddress) -> (usize, u32, u32, u32, usize)
```

**Key invariants:**
- Neighbourhood is undirected: edge u→w or w→u both contribute w to N(u).
- u and v mutually excluded: `nbr_u[v_slot / 64] &= !(1 << (v_slot % 64))` etc.
- Degenerate guard: `if u_slot == v_slot { return (0,0,0,0,node_count); }`
- AA skips deg ≤ 1 via `if ln_d > 0` (LN_TABLE[0]=LN_TABLE[1]=0).
- RA skips deg = 0 via `if d > 0` (isolated self-loop nodes).
- Self-loops counted once in degree: `if fs == ts { deg[fs] += 1 }`.
- Complexity: O(V + E) per call (one edge scan + one bit scan).

### crates/k-shell/src/lib.rs

**New function** `dispatch_graph_predict(sink, u, v)`:
- Header: `graph predict u → v`
- Table with 4 metric rows: common neighbors (raw count), jaccard, adamic-adar, resource allocation.
- Each metric has a colour coding: grey=0, yellow=weak, green=strong.
- Footer: `N node(s)  prediction: likely / weak / none`.
- 6-decimal ppm display via inline `print_predict_ppm` helper.

### crates/k-shell/src/proc.rs

**New routing** (placed after `graph compare` / `gcompare` dispatch):
```
graph predict <u> <v>   →  dispatch_graph_predict(u, v)
gpredict <u> <v>        →  alias
link predict <u> <v>    →  alias
predict <u> <v>         →  alias
```

**Help text** added alongside `graph snapshot` / `graph compare` entries.

---

## Test Harness: `host-tests/gos-graph-link-predict-harness`

**VectorAddress L4=60** identifies this harness namespace.

| Test | Graph | Prediction | Expected |
|---|---|---|---|
| 1 | empty | any pair | CN=0, all zeros, nc=0 |
| 2 | single node A | (A, B) | CN=0, nc=1 |
| 3 | A, B (no edges) | (A, B) | CN=0, all zeros |
| 4 | A→B | (A, B) | CN=0 (exclusion removes B from N(A)) |
| 5 | A→B→C | (A, C) | CN=1, J=1M, AA≈1.443M, RA=500K |
| 6 | any graph | (A, A) degenerate | all zeros |
| 7 | star A→{B,C,D} | (B, C) | CN=1, J=1M, AA≈910K, RA=333K |
| 8 | diamond A→{B,C}→D | (A, D) | CN=2, J=1M, AA≈2.885M, RA=1M |
| 9 | {A→B} ∥ {C→D} | (A, D) | CN=0, all zeros |
| 10 | A→B | (A, unknown) | CN=0, nc=2 |

**Result:** 10/10 pass.

---

## Metric Value Derivation

For test 5 (path A→B→C, predict A,C): `deg(B)=2`:
- AA = 1_000_000_000_000 / LN_TABLE[2] = 1e12 / 693_147 = 1_442_695 (≈ 1/ln(2) × 10^6)
- RA = 1_000_000 / 2 = 500_000

For test 7 (star, predict leaf-pair B,C): `deg(A)=3`:
- AA = 1e12 / LN_TABLE[3] = 1e12 / 1_098_612 ≈ 910_239 (≈ 1/ln(3) × 10^6)
- RA = 1_000_000 / 3 = 333_333 (integer division)

For test 8 (diamond, predict A,D): `deg(B)=deg(C)=2`:
- AA = 2 × 1_442_695 = 2_885_390
- RA = 2 × 500_000 = 1_000_000

---

## VectorAddress L4 Namespace Update

| L4 | Harness |
|---|---|
| 59 | gos-graph-snapshot-harness (V2.83) |
| **60** | **gos-graph-link-predict-harness (V2.84)** |

---

## Literature Reference

- D. Liben-Nowell & J. Kleinberg, "The Link-Prediction Problem for Social Networks," CIKM 2003.
- L. Adamic & E. Adar, "Friends and Neighbors on the Web," Social Networks 25(3), 2003.
- T. Zhou, L. Lü & Y.-C. Zhang, "Predicting Missing Links via Local Information," EPJB 71, 2009.

Adamic-Adar is the standard benchmark metric; Resource Allocation (Zhou 2009) often outperforms
it on sparse graphs. Both are included for completeness.
