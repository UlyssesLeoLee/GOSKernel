# tools/rpa/qemu-serial-read.ps1
#
# Read whatever has streamed out of QEMU's COM1 (where the kernel's
# `raw_serial_println` writes).  Useful for verifying boot markers,
# capturing the full boot trace, or grepping for a specific message
# without watching the GUI.
#
# Examples:
#   .\qemu-serial-read.ps1                              # dump everything
#   .\qemu-serial-read.ps1 -WaitForMarker 'enabling interrupts'
#   .\qemu-serial-read.ps1 -OutPath '.\boot.log'

[CmdletBinding()]
param(
    [string] $WaitForMarker = '',
    [int] $TimeoutSec = 30,
    [string] $OutPath = ''
)

. (Join-Path $PSScriptRoot '_common.ps1')

$buf = Read-QemuSerial -TimeoutMs ($TimeoutSec * 1000) -WaitForMarker $WaitForMarker

if ($OutPath) {
    $buf | Out-File -FilePath $OutPath -Encoding utf8
    Write-Host "✓ wrote $($buf.Length) chars to $OutPath"
} else {
    Write-Output $buf
}

if ($WaitForMarker) {
    if ($buf.Contains($WaitForMarker)) {
        Write-Host "✓ marker '$WaitForMarker' observed"
        exit 0
    } else {
        Write-Warning "marker '$WaitForMarker' not observed within ${TimeoutSec}s"
        exit 1
    }
}
