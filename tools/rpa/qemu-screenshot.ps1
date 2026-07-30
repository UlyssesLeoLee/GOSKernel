# tools/rpa/qemu-screenshot.ps1
#
# Snapshot the QEMU framebuffer to a file via the monitor's `screendump`
# command.  Output is PPM (QEMU's native dump format).  If ImageMagick
# (`magick`) is on PATH, also convert to PNG; otherwise leave the PPM.
#
# Examples:
#   .\qemu-screenshot.ps1 -OutPath '.\frame.png'
#   .\qemu-screenshot.ps1 -OutPath '.\frame.ppm' -NoConvert

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $OutPath,
    [switch] $NoConvert
)

. (Join-Path $PSScriptRoot '_common.ps1')

# screendump writes the file from QEMU's process perspective; use an
# absolute path so we always know where to find it.
$abs = [System.IO.Path]::GetFullPath($OutPath)
$ppm = if ($abs.ToLower().EndsWith('.ppm')) { $abs } else { [System.IO.Path]::ChangeExtension($abs, '.ppm') }

$conn = $null
try {
    $conn = Connect-QemuMonitor
    $cmd = "screendump $ppm"
    [void](Send-QemuMonitor -Conn $conn -Command $cmd -TimeoutMs 5000)
} finally {
    Close-QemuMonitor -Conn $conn
}

# QEMU writes synchronously, but give it a beat for filesystem buffer flush.
Start-Sleep -Milliseconds 200

if (-not (Test-Path $ppm)) {
    throw "screendump did not produce $ppm — check QEMU has display enabled (not -display none)"
}
Write-Host "✓ wrote $ppm"

if (-not $NoConvert -and $ppm -ne $abs) {
    $magick = Get-Command magick -ErrorAction SilentlyContinue
    if ($magick) {
        & magick convert $ppm $abs 2>&1 | Out-Null
        if ($LASTEXITCODE -eq 0 -and (Test-Path $abs)) {
            Remove-Item $ppm
            Write-Host "✓ converted to $abs"
        } else {
            Write-Warning "ImageMagick conversion failed; PPM kept at $ppm"
        }
    } else {
        Write-Warning 'ImageMagick `magick` not on PATH; leaving PPM file unconverted'
    }
}
