# tools/rpa/qemu-cypher.ps1
#
# High-level wrapper: send a Cypher query / mutation to the running
# kernel's command bar via the QEMU monitor's sendkey, then pause so
# the screen redraws.  Caller follows up with qemu-screenshot.ps1 or
# qemu-serial-read.ps1 to capture the response.
#
# Examples:
#   .\qemu-cypher.ps1 -Statement 'show stats'
#   .\qemu-cypher.ps1 -Statement 'show capabilities'
#   .\qemu-cypher.ps1 -Statement "create use '6.6.0.0' -> '0.0.0.1'"
#   .\qemu-cypher.ps1 -Statement 'invoke ''0.0.0.0'' with 42'

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $Statement,
    [int] $PostDelayMs = 350
)

# Ensure we're in kernel view first — type 'kernel' + Enter.  No-op
# if already there.  (Idempotent: typing the command in kernel-view
# mode just produces an `unknown: kernel` echo, harmless.)
& (Join-Path $PSScriptRoot 'qemu-sendkey.ps1') -Keys 'kernel' -Then 'ret'
Start-Sleep -Milliseconds 200

# Type the statement, then Enter.
& (Join-Path $PSScriptRoot 'qemu-sendkey.ps1') -Keys $Statement -Then 'ret'

# Give the kernel a beat to repaint with the response.
Start-Sleep -Milliseconds $PostDelayMs
Write-Host "✓ submitted: $Statement"
