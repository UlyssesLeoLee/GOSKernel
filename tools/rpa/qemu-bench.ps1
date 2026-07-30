# tools/rpa/qemu-bench.ps1
#
# Drive the kernel's BENCH RPC command N times and collect the cycle
# stats from the screenshots.  Useful for measuring RPC latency
# stability across builds.
#
# Examples:
#   .\qemu-bench.ps1                      # 3 rounds × 1000 invocations each
#   .\qemu-bench.ps1 -Rounds 10 -Per 5000
#
# Caveat: parsing the BENCH result from a screenshot requires OCR.
# This script just submits the commands; manual inspection of the
# generated PNGs gives the numbers.  A future revision can parse the
# serial stream once the bench results are mirrored there.

[CmdletBinding()]
param(
    [int] $Rounds = 3,
    [int] $Per = 1000,
    [string] $OutDir = ''
)

. (Join-Path $PSScriptRoot '_common.ps1')

if (-not $OutDir) {
    $ts = Get-Date -Format 'yyyyMMdd-HHmmss'
    $OutDir = Join-Path $PSScriptRoot "out\bench-$ts"
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
Write-Host "→ bench output: $OutDir  (rounds=$Rounds per=$Per)"

for ($i = 1; $i -le $Rounds; $i++) {
    Write-Host "  round $i / $Rounds ..."
    & (Join-Path $PSScriptRoot 'qemu-cypher.ps1') -Statement "bench rpc $Per"
    Start-Sleep -Milliseconds 800
    & (Join-Path $PSScriptRoot 'qemu-screenshot.ps1') -OutPath (Join-Path $OutDir "bench-round-$i.png")
}
Write-Host "✓ bench complete; PNGs at $OutDir"
