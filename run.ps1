# GOS Graph Operating System - Launch Script
# ─────────────────────────────────────────────────────────────────────────────
param(
    [switch]$Clean,   # Use -Clean to force full rebuild
    [switch]$SkipGovernance,
    [switch]$ValidateOnly
)

function Stop-StaleGosQemu {
    $targets = Get-CimInstance Win32_Process |
        Where-Object {
            $_.Name -eq "qemu-system-x86_64.exe" -and
            $_.CommandLine -and
            (
                $_.CommandLine -like "*bootimage-gos-kernel.bin*" -or
                $_.CommandLine -like "*gos-kernel*"
            )
        }

    foreach ($proc in $targets) {
        Write-Host "[GOS] Stopping stale QEMU process $($proc.ProcessId) holding GOS disk image" -ForegroundColor Yellow
        Stop-Process -Id $proc.ProcessId -Force -ErrorAction SilentlyContinue
    }
}

# 1. Setup Environment
$QEMU_PATH = "C:\Program Files\qemu"
if (Test-Path $QEMU_PATH) {
    if ($env:PATH -notlike "*$QEMU_PATH*") {
        $env:PATH = "$QEMU_PATH;" + $env:PATH
        Write-Host "[GOS] QEMU added to PATH" -ForegroundColor Cyan
    }
} else {
    Write-Warning "[GOS] QEMU not found at $QEMU_PATH"
    exit 1
}

# 2. Clean if requested
if ($Clean) {
    Write-Host "[GOS] Full clean rebuild requested..." -ForegroundColor Yellow
    cargo clean 2>$null
}

# 3. Build and Run
if (-not $SkipGovernance) {
    Write-Host "[GOS] Verifying graph governance rules..." -ForegroundColor Cyan
    pwsh -File (Join-Path $PSScriptRoot "tools\verify-graph-architecture.ps1")
    if ($LASTEXITCODE -ne 0) {
        Write-Error "[GOS] Graph governance verification failed."
        exit $LASTEXITCODE
    }
}

if ($ValidateOnly) {
    Write-Host "[GOS] Governance verification completed." -ForegroundColor Green
    exit 0
}

Stop-StaleGosQemu

Write-Host "[GOS] Compiling Kernel & Launching QEMU..." -ForegroundColor Green
# --release: the in-guest desktop is a software rasterizer (fbtest.rs draws
# every pixel of a 1920x1080 framebuffer + z-buffer each frame on the CPU).
# Measured directly off its own PERF/FBF serial telemetry (see tools/fps_test_whpx.py
# for the harness pattern) that the plain `dev` profile ran it at ~5.8 FPS
# (~172ms/frame) -- which surfaces as "鼠标卡顿" (mouse stutter), since the
# cursor only moves -- in one big jump -- about six times a second. --release
# lifts that to ~18-19 FPS (~53ms/frame), 3x smoother, for only a ~1s cost on
# incremental rebuilds (2-3s vs 1-2s; deps are compiled+cached per profile, so
# the ~65s full dependency build is a one-time hit, not a per-run one).
cargo run -p gos-kernel --release
