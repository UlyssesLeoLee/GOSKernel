# tools/rpa/qemu-monitor.ps1
#
# Send an arbitrary QEMU HMP monitor command and print the response.
#
# Examples:
#   .\qemu-monitor.ps1 -Command 'info status'
#   .\qemu-monitor.ps1 -Command 'info network'
#   .\qemu-monitor.ps1 -Command 'info qtree'

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $Command,
    [int] $TimeoutMs = 3000
)

. (Join-Path $PSScriptRoot '_common.ps1')

$conn = $null
try {
    $conn = Connect-QemuMonitor
    $resp = Send-QemuMonitor -Conn $conn -Command $Command -TimeoutMs $TimeoutMs
    Write-Output $resp
} finally {
    Close-QemuMonitor -Conn $conn
}
