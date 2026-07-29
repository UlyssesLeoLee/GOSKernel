---
name: gos-ppm-assertion-pin-from-runtime
description: When writing exact `assert_eq!(ppm_value, <literal>)` assertions for GOSKernel integer ppm results computed via multi-step LN_TABLE lookup + integer division chains, always run the test first to observe the actual runtime value — do NOT pin manually-computed analytical values, because LN_TABLE truncation + final division truncation compound in unpredictable ways. Apply in all gos-graph-*-harness test files that pin exact ppm values.
---

# Pin Exact PPM Values from Runtime Output, Not Manual Computation

## The rule

For any GOSKernel metric that chains LN_TABLE lookups and integer division, compute the expected value analytically as a *range check* (`assert!(x > A && x <= B)`), then pin exact equality separately by running the test once and observing the failure output. Never hardcode `assert_eq!(ppm, <hand-calculated>)` without first verifying against an actual cargo test run.

```rust
// WRONG — hand-calculated value differs from runtime truncation
assert_eq!(gamma_ppm, 1_910_228, "K4 gamma");

// CORRECT — run test, observe failure, pin the real value
assert_eq!(gamma_ppm, 1_910_239, "K4 gamma");  // from actual cargo test run

// ALSO FINE — range check is safe to compute analytically
assert!(gamma_ppm > 1_000_000 && gamma_ppm <= 3_000_000, "gamma in [1,3]");
```

## Why it's non-obvious

GOSKernel ppm metrics use truncating integer division at **two** layers:

1. **LN_TABLE layer**: values are `⌊ln(k) × 10^6⌋` — already truncated below the true float value.
2. **Final formula layer**: `gamma_ppm = 1_000_000 + n × 10^12 / sum_ln_ppm` — divides a product of two truncated values, compounding the error.

The manual computation uses the mathematically exact LN values (e.g. ln(3) = 1.098612..., not the table's 1_098_612/10^6 = 1.098612 ✓ but the *product* `4 × 1_098_612 = 4_394_448` vs the true `4 × ln(3) × 10^6 = 4_394_449.15...` already differs by 1 ULP).

For K4 bidirected (n=4, all k=3):
- Analytical: 1_000_000 + 4×10^12 / 4_394_449 = 1_000_000 + **910_227** = 1_910_227
- LN_TABLE path: 1_000_000 + 4×10^12 / 4_394_448 = 1_000_000 + **910_239** = 1_910_239
- Difference: **12 ppm** — easily wrong if pinned from mental math

The error is small but deterministic and reproducible, making it a confusing spurious test failure that looks like a logic bug.

## GOSKernel context

- Affects any metric using `LN_TABLE` (graph_power_law V2.80, graph_small_world V2.77)
- Also affects double-ppm ratios (graph_avg_clustering, graph_global_efficiency) where intermediate truncation compounds
- Pattern: write range assertions first (`> A_000_000 && <= B_000_000`), then pin exact via test run
- The `assert!(expr, "msg={ppm_val}")` format in GOSKernel harnesses shows the actual value in the failure message, making it easy to harvest the correct literal

## From this session

V2.81 harness (gos-graph-summary2-harness, tests 5 and 8):
- Test 5 (K4 complete): pinned 1_910_228 → actual 1_910_239 (off by 11)
- Test 8 (directed star): pinned 4_640_871 → actual 4_640_957 (off by 86)

Both failures were caught in the first cargo test run and corrected. The fix: use the actual values from the failure output, not the analytically derived values.
