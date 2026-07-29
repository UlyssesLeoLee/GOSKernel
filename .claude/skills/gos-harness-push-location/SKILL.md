---
name: gos-harness-push-location
description: When running GOSKernel host-test harnesses from a script or loop, use Push-Location/Pop-Location to enter the harness directory before running cargo test — do NOT use cargo test --manifest-path from the workspace root. Apply in tools/run_host_tests.ps1 and any new test runner scripts.
---

# Host Harness Test Runner: Push-Location, Not --manifest-path

## The rule

Run each harness by entering its directory:

```powershell
# CORRECT
Push-Location $h.FullName
$result = & cargo test --quiet 2>&1
$exitCode = $LASTEXITCODE
Pop-Location

# WRONG — causes false-negative exit codes for some harnesses
$result = & cargo test --manifest-path (Join-Path $h.FullName "Cargo.toml") --quiet 2>&1
if ($LASTEXITCODE -eq 0) { ... }
```

## Why it's non-obvious

When `cargo test --manifest-path /path/to/harness/Cargo.toml` is run from the GOSKernel workspace root, PowerShell captures cargo's colored stderr output (ANSI escape codes like `[31;1m`) as `ErrorRecord` objects via `2>&1`. In some harnesses (specifically the ones producing warnings about `private_interfaces` or other diagnostics), these ErrorRecord objects interfere with `$LASTEXITCODE` reporting — the test passes (cargo exits 0) but the script sees a non-zero exit code.

The `Push-Location` approach avoids the interference by running cargo from the harness's own directory, where stderr is cleanly separate.

## GOSKernel context

- 44 host-test harnesses in `host-tests/gos-*-harness/`
- Each is an independent workspace with its own `.cargo/config.toml` (target = x86_64-pc-windows-msvc)
- Test runner: `tools/run_host_tests.ps1`
- Also save `$exitCode = $LASTEXITCODE` immediately after the cargo call, before any other commands can overwrite it

## From this session

Full test suite showed "40 passed, 4 failed" for gos-graph-eccentricity/pagerank/hits/community harnesses via `--manifest-path`. All 4 showed "PASS" when run with `Push-Location`. Root cause: ANSI escape codes in compiler diagnostics (warnings about private_interfaces) being captured as PowerShell ErrorRecord objects. Fixed in commit 9878a3c.
