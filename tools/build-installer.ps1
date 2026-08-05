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
                $_.CommandLine -like "*gos-kernel-uefi.img*" -or
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

# ADR-018: image building is now `xtask image` (bootloader_api 0.11.9,
# UEFI-only), a normal build-dependency resolved by cargo -- no separate
# `cargo install bootimage` step needed anymore.
function Get-BootImagePath {
    param(
        [string]$RepoRoot,
        [string]$Profile
    )

    # Respects CARGO_TARGET_DIR the same way xtask's own
    # cargo_target_dir() does -- this dev environment sets it to a
    # shared cache dir outside the repo, not <repo>/target.
    $TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $RepoRoot "target" }
    $candidate = Join-Path $TargetDir ("x86_64-gos-kernel/{0}/gos-kernel-uefi.img" -f $Profile)
    if (Test-Path $candidate) {
        return $candidate
    }

    throw "UEFI disk image not found at $candidate. Expected gos-kernel-uefi.img (run `xtask image` first, or omit -SkipBuild)."
}

Write-Host "[GOS] Checking installer toolchain..." -ForegroundColor Cyan
Assert-Command "cargo"
Assert-Command "rustup"

if ($CheckOnly) {
    Write-Host "[GOS] Installer toolchain check completed." -ForegroundColor Green
    exit 0
}

Stop-StaleGosQemu

if (-not $SkipBuild) {
    $xtaskArgs = @("run", "--", "image")
    if ($Profile -eq "release") {
        $xtaskArgs += "--release"
    }

    Write-Host "[GOS] Building bootable UEFI installer image..." -ForegroundColor Cyan
    Push-Location (Join-Path $RepoRoot "xtask")
    try {
        & cargo @xtaskArgs
        if ($LASTEXITCODE -ne 0) {
            throw "xtask image failed"
        }
    } finally {
        Pop-Location
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
    target = "x86_64 UEFI bare metal"
    boot_mode = "UEFI disk image (GPT/ESP, bootloader_api 0.11.9) -- write raw, boot via EFI, not BIOS/legacy"
    notes = @(
        "Write the image to USB to boot a target machine without Rust tooling.",
        "The current installer is a bootable system image, not an in-OS partitioning wizard.",
        "Target machine must boot via native UEFI (hold Option on Mac hardware and select the EFI Boot entry) -- this image is not a legacy BIOS/MBR boot sector."
    )
}
$Manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $ManifestPath -Encoding UTF8

Copy-Item -LiteralPath (Join-Path $RepoRoot "tools/write-usb-image.ps1") -Destination (Join-Path $PackageRoot "write-usb-image.ps1") -Force
# ADR-018: doc/INSTALL_BARE_METAL_zh.md is a deliberate 1-line stub
# redirecting to doc/06_运维维护/INSTALL_BARE_METAL_zh.md (single-source-
# of-truth convention) -- this was copying the stub, not the real
# 137-line guide, into every installer package. Found while updating
# this script for the bootloader migration, fixed in the same pass.
Copy-Item -LiteralPath (Join-Path $RepoRoot "doc/06_运维维护/INSTALL_BARE_METAL_zh.md") -Destination (Join-Path $PackageRoot "INSTALL_BARE_METAL_zh.md") -Force
Copy-Item -LiteralPath (Join-Path $RepoRoot "README.md") -Destination (Join-Path $PackageRoot "README.md") -Force

if (Test-Path $ZipPath) {
    Remove-Item -Force -LiteralPath $ZipPath
}
Compress-Archive -Path (Join-Path $PackageRoot "*") -DestinationPath $ZipPath -CompressionLevel Optimal

Write-Host "[GOS] Installer image: $InstallerImagePath" -ForegroundColor Green
Write-Host "[GOS] Manifest:        $ManifestPath" -ForegroundColor Green
Write-Host "[GOS] Archive:         $ZipPath" -ForegroundColor Green
Write-Host "[GOS] SHA256:          $ImageHash" -ForegroundColor Green
