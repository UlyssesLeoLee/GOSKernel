# tools/rpa/qemu-quit.ps1
#
# Gracefully shut down a running QEMU instance.  Tries system_powerdown
# first (clean ACPI), falls back to `quit` if the kernel doesn't ACK
# within the timeout.  Returns nothing.

[CmdletBinding()]
param(
    [int] $PowerdownTimeoutSec = 3
)

. (Join-Path $PSScriptRoot '_common.ps1')

$conn = $null
try {
    $conn = Connect-QemuMonitor
    Write-Host '→ requesting system_powerdown ...'
    [void](Send-QemuMonitor -Conn $conn -Command 'system_powerdown')
    Start-Sleep -Seconds $PowerdownTimeoutSec

    # Check if QEMU is still running.
    $stillRunning = $true
    try {
        $resp = Send-QemuMonitor -Conn $conn -Command 'info status' -TimeoutMs 500
        if ($resp -match 'shutdown|paused') { $stillRunning = $false }
    } catch {
        $stillRunning = $false
    }

    if ($stillRunning) {
        Write-Host '→ powerdown timed out, sending quit ...'
        [void](Send-QemuMonitor -Conn $conn -Command 'quit' -TimeoutMs 500)
    } else {
        Write-Host '✓ QEMU acknowledged shutdown.'
    }
} catch {
    Write-Warning "qemu-quit: $($_.Exception.Message)"
} finally {
    Close-QemuMonitor -Conn $conn
}
