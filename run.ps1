# GOS Graph Operating System - Launch Script
# ─────────────────────────────────────────────────────────────────────────────

# 1. Setup Environment
$QEMU_PATH = "C:\Program Files\qemu"
if (Test-Path $QEMU_PATH) {
    if ($env:PATH -notlike "*$QEMU_PATH*") {
        $env:PATH = "$QEMU_PATH;" + $env:PATH
        Write-Host "[GOS_BUILD] QEMU added to PATH: $QEMU_PATH" -ForegroundColor Cyan
    }
} else {
    Write-Warning "[GOS_ERROR] QEMU not found at $QEMU_PATH. Please check your installation."
}

# 2. Build and Run
Write-Host "[GOS_BUILD] Compiling Kernel & Launching QEMU..." -ForegroundColor Green
cargo run
