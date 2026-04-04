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
cargo run -p gos-kernel
