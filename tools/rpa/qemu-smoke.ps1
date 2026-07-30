# tools/rpa/qemu-smoke.ps1
#
# End-to-end automated smoke test:
#   1. build kernel
#   2. launch QEMU in background
#   3. wait for boot steady-state marker on serial
#   4. take a screenshot of the boot view
#   5. send a few Cypher queries, screenshot after each
#   6. quit QEMU
#
# Outputs land in $OutDir (default: tools/rpa/out/<timestamp>/).
# Exit 0 if every step succeeded, non-zero on first failure.

[CmdletBinding()]
param(
    [string] $OutDir = '',
    [int] $BootTimeoutSec = 60,
    [switch] $SkipBuild
)

. (Join-Path $PSScriptRoot '_common.ps1')

$ts = Get-Date -Format 'yyyyMMdd-HHmmss'
if (-not $OutDir) {
    $OutDir = Join-Path $PSScriptRoot "out\$ts"
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
Write-Host "→ smoke run output: $OutDir"

# Step 1+2: launch.
$launchArgs = @('-Wait', '-WaitTimeoutSec', $BootTimeoutSec)
if ($SkipBuild) { $launchArgs += '-NoBuild' }
$pid = & (Join-Path $PSScriptRoot 'qemu-launch.ps1') @launchArgs

if (-not $pid) {
    Write-Error 'qemu-launch failed; aborting smoke.'
    exit 2
}
Write-Host "✓ QEMU PID $pid"

try {
    # Step 3: verify boot marker via serial.
    Write-Host '→ checking serial boot trace ...'
    $serial = Read-QemuSerial -TimeoutMs 5000 -WaitForMarker 'enabling interrupts'
    $serial | Out-File -FilePath (Join-Path $OutDir 'boot.log') -Encoding utf8
    if (-not $serial.Contains('enabling interrupts')) {
        Write-Error 'boot marker not in serial trace; smoke FAIL'
        exit 3
    }
    Write-Host '✓ serial boot trace captured'

    # Step 4: boot screenshot.
    & (Join-Path $PSScriptRoot 'qemu-screenshot.ps1') -OutPath (Join-Path $OutDir 'boot.png')

    # Step 5: query series — exercise the J/K/L command surface.
    $queries = @(
        'show stats',
        'show plugins',
        'show capabilities',
        'show nodes of class driver',
        'show edges of kind use',
        'bench rpc 1000'
    )
    foreach ($q in $queries) {
        Write-Host "→ Cypher: $q"
        & (Join-Path $PSScriptRoot 'qemu-cypher.ps1') -Statement $q
        $safe = ($q -replace '[^a-z0-9]+','-').Trim('-')
        & (Join-Path $PSScriptRoot 'qemu-screenshot.ps1') -OutPath (Join-Path $OutDir "after-$safe.png")
    }

    Write-Host "✓ smoke complete; artifacts at $OutDir"
    exit 0
} finally {
    # Step 6: quit.
    & (Join-Path $PSScriptRoot 'qemu-quit.ps1')
}
