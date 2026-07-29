---
name: gos-windows-lnk1104-retry
description: On Windows/MSVC, an immediate second `cargo test` after a successful first run can fail with LNK1104 "cannot open file ...exe" because the first test executable is still file-locked. Add `Start-Sleep -Milliseconds 2000` before retrying, or wait before the second run in any PowerShell test loop. Apply whenever a `cargo test` fails with LNK1104 exit code 1104 and the file path is the harness output .exe.
---

# Windows MSVC LNK1104 Transient File Lock

## The rule

When a `cargo test` run fails with:
```
LINK : fatal error LNK1104: 无法打开文件"...exe"
```
the executable from the previous run is still held open (Windows file locking). Do NOT assume a code error — just wait a couple seconds and retry:

```powershell
Start-Sleep -Milliseconds 2000
Push-Location $harness
cargo test --quiet
Pop-Location
```

## Why it's non-obvious

On Linux, a test binary is immediately freed after the test runner exits. On Windows, the MSVC runtime can hold the file handle open briefly after process exit, preventing the linker from overwriting it on the next build. This looks identical to a genuine linker error but is purely transient — the same code compiles and links successfully two seconds later.

## GOSKernel context

- Affects any `host-tests/gos-*-harness/` run under PowerShell on Windows
- Typically occurs when you remove a warning and immediately re-run the same harness
- The exit code is 101 (cargo propagates the linker exit code 1104), which can look like a test failure

## From this session

V2.60: After removing an unused variable warning from `node_attr_list_u8.rs`, the immediate re-run of `gos-node-attr-list-u8-harness` failed with LNK1104 exit code 101. A 2-second sleep fixed it on the next attempt. 10/10 tests passed cleanly.
