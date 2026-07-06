# Hardening Log V3.10 — Graph Entropy H(G)

**Date**: 2026-07-06  
**Branch**: feat/vk-auto-live-surface  
**Previous baseline**: V3.09 (graph spectral analysis ρ(A)+λ₂(L), 1063 host tests)  
**New total**: 1073 host tests (+10)

---

## Algorithm: Shannon Entropy of the Degree Distribution

**Graph entropy** measures the *structural diversity* of a graph by applying Shannon's information-theoretic framework to its degree sequence. The degree distribution is treated as a probability distribution p(d) = (# nodes with degree d) / n, and the entropy

> H(G) = −Σ_d p(d) ln p(d)

quantifies how spread-out that distribution is. H = 0 for a perfectly regular graph (all nodes share the same degree); H = ln(n) when every node has a distinct degree (maximum heterogeneity). The **normalised entropy** H' = H/ln(n) ∈ [0, 1] provides a scale-independent diversity index.

### Theoretical Background

**Shannon entropy** (Shannon 1948) is the foundational measure of information content in a probability distribution. Applied to graphs, the degree distribution p(k) is the natural discrete distribution to quantify, since degree captures each node's local connectivity role. Key properties:

- **Regular graphs** (k-regular: every node has the same degree): H = 0. One degree class → p(d) = 1 → −1·ln(1) = 0.
- **Uniform degree distribution** (unlikely in real graphs): H = ln(n_classes).
- **Star K_{1,k}**: two degree classes (one hub at degree k, k leaves at degree 1) → moderate entropy.
- **Paths P_n**: end nodes at degree 1, interior nodes at degree 2 → H = −(2/n)ln(2/n) − ((n-2)/n)ln((n-2)/n) → converges to ln(2) as n → ∞.
- **Complete graph K_n**: all degree n−1 → H = 0.

**Normalised entropy** H' = H / ln(n) is also called **normalised graph entropy** (Dehmer & Mowshowitz 2011). It measures structural regularity on a [0,1] scale independent of graph size.

**Relationship to other metrics**:
- **Power-law exponent γ̂** (V2.80): quantifies the tail of the degree distribution; entropy quantifies its overall spread.
- **Degree assortativity** (V2.65): measures degree-degree correlations; entropy measures degree diversity.
- **Spectral radius ρ(A)** (V3.09): bounded by √(2mH') where m = edges; high-entropy graphs tend toward larger spectral radii.

---

## Implementation

### Key Formula

```
H(G) = −Σ_{d: count[d]>0} (count[d]/n) · ln(count[d]/n)
      = (1/n) · Σ_{d: count[d]>0} count[d] · (ln(n) − ln(count[d]))
```

In integer arithmetic using the LN_TABLE (ln(k) × 10^6 tabulated for k = 0..128):

```
entropy_scaled = Σ_{d: count[d]>0} count[d] · (LN_TABLE[n] − LN_TABLE[count[d]])
entropy_ppm    = entropy_scaled / n   ≈ H × 10^6   (floor division, ≤1 ppm error)
normalized_ppm = entropy_ppm × 10^6 / LN_TABLE[n]   (H' × 10^6)
```

This is **exact integer arithmetic** — no floating-point, no_std safe. The LN_TABLE has been in the codebase since V2.77 (small-world coefficient). The formula reuses the same `Σ count·ln(count)` accumulation pattern established for entropy computations.

### Overflow Analysis

- `count[d]` ≤ n ≤ 128, so `LN_TABLE[count[d]]` is always in bounds.
- `entropy_scaled` ≤ n × LN_TABLE[n] ≤ 128 × 4,852,030 = 620,659,840 < 2^30. Fits u64.
- `entropy_ppm` = entropy_scaled / n ≤ LN_TABLE[128] = 4,852,030 < 2^23. Fits u32.
- `entropy_ppm × 1_000_000` ≤ 4,852,030,000,000 < 2^43. Intermediate u64; result ≤ 1,000,000 fits u32.

### Public API

```rust
pub fn graph_entropy() -> (u32, u32, usize)
```

Returns `(entropy_ppm, normalized_ppm, node_count)`:
- `entropy_ppm` = H × 10^6 (Shannon entropy in nats, scaled to ppm)
- `normalized_ppm` = H/ln(n) × 10^6 ∈ [0, 1,000,000]
- `node_count` = number of alive nodes in the runtime

### Shell Commands

| Command | Description |
|---------|-------------|
| `graph entropy` | Full entropy panel |
| `gentropy` | Short alias |
| `degree entropy` | Descriptive alias |
| `graph deg entropy` | Explicit alias |

### Display Output

```
 graph entropy  (H = −Σ p(d) ln p(d)  degree distribution)
 ───────────────────────────────────────────────────────────
  entropy      H   =  0.636   nat
  normalised  H'  =  0.579   [H / ln(n)]   moderate diversity
  max entropy      =  1.099   [ln(n)]
 ───────────────────────────────────────────────────────────
 3 node(s)  Shannon 1948  Dehmer & Mowshowitz 2011
```

---

## Test Cases (10)

### Analytical Derivation

All values are computed with exact integer arithmetic using LN_TABLE values. No floating-point tolerance needed — all `assert_eq!` with exact values.

| # | Graph | Degrees | entropy_ppm | normalized_ppm | nc |
|---|-------|---------|-------------|----------------|----|
| 1 | Empty | — | 0 | 0 | 0 |
| 2 | Single node | {0} | 0 | 0 | 1 |
| 3 | Edge A-B | {1,1} | 0 | 0 | 2 |
| 4 | Path P₃ | {1,2,1} | **636,514** | **579,380** | 3 |
| 5 | Triangle K₃ | {2,2,2} | 0 | 0 | 3 |
| 6 | Star K_{1,4} | {4,1,1,1,1} | **500,401** | **310,916** | 5 |
| 7 | Path P₄ | {1,2,2,1} | **693,147** | **500,000** | 4 |
| 8 | Complete K₄ | {3,3,3,3} | 0 | 0 | 4 |
| 9 | Two isolated | {0,0} | 0 | 0 | 2 |
| 10 | K_{2,3} | {3,3,2,2,2} | **673,011** | **418,165** | 5 |

### Notable Exact Relationships

**Test 7 (P₄)** is the cleanest cross-check:
- P₄ has two equal-size degree groups: {1,1} and {2,2} (n=4, each group has 2/4 = 1/2 of nodes)
- H = ln(2) ≈ 0.693147 (maximum entropy for 2-class distribution with equal proportions)
- entropy_ppm = 693,147 exactly (this is LN_TABLE[2] × 1 = ln(2) × 10^6)
- normalized_ppm = 693,147 × 10^6 / LN_TABLE[4] = 693,147 × 10^6 / (2 × 693,147) = 500,000 (exact)
- H' = 1/2 exactly, since ln(4) = 2 ln(2)

**Test 6 (Star K_{1,4})**:
```
entropy_scaled = 4 × (LN_TABLE[5] − LN_TABLE[4]) + 1 × (LN_TABLE[5] − LN_TABLE[1])
              = 4 × (1,609,437 − 1,386,294) + 1 × (1,609,437 − 0)
              = 4 × 223,143 + 1,609,437
              = 892,572 + 1,609,437 = 2,502,009
entropy_ppm   = 2,502,009 / 5 = 500,401  (rem=4)
normalized_ppm = 500,401 × 10^6 / 1,609,437 = 310,916
```

**Test 10 (K_{2,3})**:
```
entropy_scaled = 3 × (LN_TABLE[5] − LN_TABLE[3]) + 2 × (LN_TABLE[5] − LN_TABLE[2])
              = 3 × (1,609,437 − 1,098,612) + 2 × (1,609,437 − 693,147)
              = 3 × 510,825 + 2 × 916,290
              = 1,532,475 + 1,832,580 = 3,365,055
entropy_ppm   = 3,365,055 / 5 = 673,011
normalized_ppm = 673,011 × 10^6 / 1,609,437 = 418,165
```

---

## OS Analogy

In an operating system dependency graph where nodes are kernel subsystems/services and edges are IPC dependencies:

**Entropy as a structural diversity index:**

- **H = 0** (regular graph): every subsystem has identical connectivity. Examples: ring topology, complete mesh. Predictable, uniform IPC scheduling — all modules have the same number of dependency links.

- **Low H** (e.g., H' < 0.3): near-homogeneous — most subsystems have similar degree. Structured, well-layered topology. Easy to schedule and audit.

- **Moderate H** (e.g., H' ≈ 0.5): mixture of roles — some hubs, some leaves. Common in real kernels: a few critical subsystems (scheduler, memory manager) have many dependencies; most modules are more peripheral.

- **High H** (H' → 1): maximally heterogeneous — all subsystems have different numbers of dependency links. Organic/evolutionary topology, harder to formally verify.

**Entropy-driven operations:**

```bash
# Analogue: diversity of kernel module connectivity
graph entropy      # Measure structural diversity H(G)
graph summary      # Full topology panel (includes density, CC, efficiency)
graph power law    # γ̂: tail exponent (how extreme are the hubs?)
graph kcore        # k-core: who are the most densely connected subsystems?
```

**Epoch monitoring**: graph entropy does NOT bump the epoch (pure read-only metric). It can be polled rapidly to detect topology changes (e.g., after module hot-loading or dependency injection).

---

## VectorAddress Namespace Update

```
L4=86: graph-entropy (gos-graph-entropy-harness)
```

Full updated L4 namespace:
```
82=graph-ebc, 83=graph-vconn, 84=graph-ecolor, 85=graph-spectral, 86=graph-entropy
```

---

## Literature

| Reference | Contribution |
|-----------|-------------|
| Shannon 1948 | Foundational entropy: H = −Σ p log p |
| Trucco 1956 | First application of entropy to graph theory |
| Dehmer & Mowshowitz 2011 | Comprehensive survey of graph entropy measures |
| Clauset, Newman & Shalizi 2009 | MLE for power-law (V2.80, complements this) |
| Newman 2002 | Degree assortativity (V2.65, complements this) |
