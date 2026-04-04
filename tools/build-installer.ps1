[CmdletBinding()]
param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "release",
    [switch]$CheckOnly,
    [switch]$SkipBuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$DistRoot = Join-Path $RepoRoot "dist"
$PackageRoot = Join-Path $DistRoot "gos-installer"
$ZipPath = Join-Path $DistRoot ("gos-installer-{0}.zip" -f $Profile)

function Stop-StaleGosQemu {
    $targets = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
        Where-Object {
            $_.Name -eq "qemu-system-x86_64.exe" -and
            $_.CommandLine -and
            (
                $_.CommandLine -like "*bootimage-gos-kernel.bin*" -or
                $_.CommandLine -like "*gos-kernel*"
            )
        }

    foreach ($proc in $targets) {
        Write-Host "[GOS] Stopping stale QEMU process $($proc.ProcessId) before image packaging..." -ForegroundColor Yellow
        Stop-Process -Id $proc.ProcessId -Force -ErrorAction SilentlyContinue
    }
}

function Assert-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command not found: $Name"
    }
}

function Assert-BootimageInstalled {
    $null = & cargo bootimage --help 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "cargo-bootimage is not installed. Run: cargo install bootimage --locked"
    }
}

function Get-BootImagePath {
    param(
        [string]$RepoRoot,
        [string]$Profile
    )

    $candidate = Join-Path $RepoRoot ("target\x86_64-gos-kernel\{0}\bootimage-gos-kernel.bin" -f $Profile)
    if (Test-Path $candidate) {
        return $candidate
    }

    $fallback = Get-ChildItem -Path (Join-Path $RepoRoot "target") -Filter "bootimage-gos-kernel*.bin" -Recurse -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

    if ($fallback) {
        return $fallback.FullName
    }

    throw "Boot image not found under target/. Expected bootimage-gos-kernel.bin."
}

Write-Host "[GOS] Checking installer toolchain..." -ForegroundColor Cyan
Assert-Command "cargo"
Assert-Command "rustup"
Assert-BootimageInstalled

if ($CheckOnly) {
    Write-Host "[GOS] Installer toolchain check completed." -ForegroundColor Green
    exit 0
}

Stop-StaleGosQemu

if (-not $SkipBuild) {
    $bootimageArgs = @("bootimage", "-p", "gos-kernel")
    if ($Profile -eq "release") {
        $bootimageArgs += "--release"
    }

    Write-Host "[GOS] Building bootable installer image..." -ForegroundColor Cyan
    & cargo @bootimageArgs
    if ($LASTEXITCODE -ne 0) {
        throw "cargo bootimage failed"
    }
}

$BootImagePath = Get-BootImagePath -RepoRoot $RepoRoot -Profile $Profile

if (Test-Path $PackageRoot) {
    Remove-Item -Recurse -Force -LiteralPath $PackageRoot
}
New-Item -ItemType Directory -Path $PackageRoot | Out-Null

$InstallerImagePath = Join-Path $PackageRoot "gos-installer.img"
Copy-Item -LiteralPath $BootImagePath -Destination $InstallerImagePath -Force

$ManifestPath = Join-Path $PackageRoot "installer-manifest.json"
$ImageHash = (Get-FileHash -LiteralPath $InstallerImagePath -Algorithm SHA256).Hash.ToLowerInvariant()
$Manifest = [ordered]@{
    schema_version = 1
    project = "GOS"
    profile = $Profile
    artifact = "gos-installer.img"
    sha256 = $ImageHash
    built_at_utc = (Get-Date).ToUniversalTime().ToString("o")
    target = "x86_64 bare metal"
    boot_mode = "raw boot disk image"
    notes = @(
        "Write the image to USB to boot a target machine without Rust tooling.",
        "The current installer is a bootable system image, not an in-OS partitioning wizard."
    )
}
$Manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $ManifestPath -Encoding UTF8

Copy-Item -LiteralPath (Join-Path $RepoRoot "tools\write-usb-image.ps1") -Destination (Join-Path $PackageRoot "write-usb-image.ps1") -Force
Copy-Item -LiteralPath (Join-Path $RepoRoot "doc\INSTALL_BARE_METAL_zh.md") -Destination (Join-Path $PackageRoot "INSTALL_BARE_METAL_zh.md") -Force
Copy-Item -LiteralPath (Join-Path $RepoRoot "README.md") -Destination (Join-Path $PackageRoot "README.md") -Force

if (Test-Path $ZipPath) {
    Remove-Item -Force -LiteralPath $ZipPath
}
Compress-Archive -Path (Join-Path $PackageRoot "*") -DestinationPath $ZipPath -CompressionLevel Optimal

Write-Host "[GOS] Installer image: $InstallerImagePath" -ForegroundColor Green
Write-Host "[GOS] Manifest:        $ManifestPath" -ForegroundColor Green
Write-Host "[GOS] Archive:         $ZipPath" -ForegroundColor Green
Write-Host "[GOS] SHA256:          $ImageHash" -ForegroundColor Green
