[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = "High")]
param(
    [string]$ImagePath = (Join-Path (Split-Path -Parent $PSScriptRoot) "dist\gos-installer\gos-installer.img"),
    [int]$DiskNumber = -1,
    [switch]$List,
    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Test-Administrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Show-DiskTable {
    Get-Disk |
        Select-Object Number, FriendlyName, BusType, PartitionStyle, IsBoot, IsSystem,
            @{ Name = "SizeGB"; Expression = { [math]::Round($_.Size / 1GB, 2) } } |
        Format-Table -AutoSize
}

function Assert-SafeDisk {
    param(
        [Microsoft.Management.Infrastructure.CimInstance]$Disk,
        [switch]$Force
    )

    if ($Disk.IsBoot -or $Disk.IsSystem) {
        throw "Refusing to write to a boot/system disk."
    }

    $allowedBusTypes = @("USB", "SD", "MMC")
    if (-not $Force -and $Disk.BusType.ToString() -notin $allowedBusTypes) {
        throw "Disk $($Disk.Number) is not a removable disk. Re-run with -Force only if you intend to overwrite a fixed disk."
    }
}

if ($List -or $DiskNumber -lt 0) {
    Show-DiskTable
    if ($DiskNumber -lt 0) {
        exit 0
    }
}

if (-not $IsWindows) {
    throw "write-usb-image.ps1 currently supports Windows hosts only."
}

if (-not (Test-Administrator)) {
    throw "Please run PowerShell as Administrator."
}

$ResolvedImage = Resolve-Path -LiteralPath $ImagePath | Select-Object -ExpandProperty Path
$ImageInfo = Get-Item -LiteralPath $ResolvedImage
$TargetDisk = Get-Disk -Number $DiskNumber -ErrorAction Stop

Assert-SafeDisk -Disk $TargetDisk -Force:$Force

if ($TargetDisk.Size -lt $ImageInfo.Length) {
    throw "Disk $DiskNumber is smaller than the image."
}

Write-Host "[GOS] Target disk" -ForegroundColor Yellow
$TargetDisk | Select-Object Number, FriendlyName, BusType, PartitionStyle, IsBoot, IsSystem,
    @{ Name = "SizeGB"; Expression = { [math]::Round($_.Size / 1GB, 2) } } | Format-List

Write-Host "[GOS] Image: $ResolvedImage" -ForegroundColor Yellow
Write-Host "[GOS] Size:  $([math]::Round($ImageInfo.Length / 1MB, 2)) MiB" -ForegroundColor Yellow

if (-not $Force) {
    $confirmation = Read-Host "Type WRITE to overwrite disk $DiskNumber"
    if ($confirmation -ne "WRITE") {
        throw "Write cancelled by user."
    }
}

try {
    Set-Disk -Number $DiskNumber -IsReadOnly $false -ErrorAction SilentlyContinue | Out-Null
} catch {
}

$PhysicalDrivePath = "\\.\PhysicalDrive$DiskNumber"
$BufferSize = 4MB
$Buffer = New-Object byte[] $BufferSize
$TotalWritten = 0L

$ImageStream = [System.IO.File]::Open($ResolvedImage, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::Read)
try {
    $DiskStream = New-Object System.IO.FileStream(
        $PhysicalDrivePath,
        [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Write,
        [System.IO.FileShare]::ReadWrite
    )
    try {
        while (($Read = $ImageStream.Read($Buffer, 0, $Buffer.Length)) -gt 0) {
            $DiskStream.Write($Buffer, 0, $Read)
            $TotalWritten += $Read
            $Percent = [int](($TotalWritten * 100) / $ImageInfo.Length)
            Write-Progress -Activity "Writing GOS installer image" -Status "$Percent% complete" -PercentComplete $Percent
        }
        $DiskStream.Flush()
    } catch {
        throw "Raw disk write failed. Close any programs using the target USB drive, or use Rufus/balenaEtcher as fallback. $($_.Exception.Message)"
    } finally {
        $DiskStream.Dispose()
    }
} finally {
    $ImageStream.Dispose()
    Write-Progress -Activity "Writing GOS installer image" -Completed
}

Write-Host "[GOS] Image written successfully to disk $DiskNumber." -ForegroundColor Green
