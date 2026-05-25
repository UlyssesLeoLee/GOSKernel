# run-gos.ps1 — one-click launcher for the GOS kernel under QEMU.
#
# Behaviour:
#   1. Kills any stale qemu-system-x86_64 from a previous run so the
#      monitor / serial ports (45555, 14444) aren't held.
#   2. `cargo run -p gos-kernel` — compiles the kernel + bootloader if
#      anything changed, then hands off to bootimage's runner which
#      spawns QEMU in full-screen mode at 1920×1200 HD VBE.
#   3. Streams QEMU's serial stdio (boot trace + chat HUD lines) to
#      this PowerShell console so the operator can see kernel logs
#      live while clicking around the 3D view.
#
# Tested on Windows 11 + PowerShell 7 + Rust nightly toolchain.
# From a fresh shell:
#       cd E:\GOSKernel
#       .\run-gos.ps1
# or double-click the .ps1 (if PowerShell .ps1 association is set).

[CmdletBinding()]
param(
    [switch]$NoFullScreen,   # pass through to QEMU args (windowed mode)
    [switch]$Release         # build with --release for fastest TCG path
)

$ErrorActionPreference = 'Stop'
$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Definition
Set-Location $scriptRoot

Write-Host '── GOS kernel launcher ─────────────────────────────────'
Write-Host ('worktree:  {0}' -f $scriptRoot)
Write-Host ''

# Step 1 — clear any stale QEMU instance.
Get-Process qemu-system-x86_64 -ErrorAction SilentlyContinue | ForEach-Object {
    Write-Host ('stopping stale qemu pid {0} ...' -f $_.Id) -ForegroundColor Yellow
    $_.Kill()
    $_.WaitForExit(2000)
}

# Step 2 — build + run via cargo + bootimage.
$cargoArgs = @('run', '-p', 'gos-kernel')
if ($Release) { $cargoArgs += '--release' }

# Mode info — purely for the operator's serial console.
Write-Host '── build phase ────'
Write-Host ('cargo {0}' -f ($cargoArgs -join ' '))
Write-Host '── runtime phase (QEMU full-screen, monitor on tcp 45555, serial on 14444) ────'
Write-Host ''

# bootimage's runner reads metadata.bootimage.run-args from the kernel
# crate's Cargo.toml — those include -full-screen, -monitor, -serial,
# -device e1000.  No extra QEMU args needed here.  Streams stdio for
# the kernel's raw_serial_println output.
& cargo @cargoArgs

$exitCode = $LASTEXITCODE
Write-Host ''
Write-Host ('── qemu exit code: {0} ───────────────────────────────' -f $exitCode)
exit $exitCode
