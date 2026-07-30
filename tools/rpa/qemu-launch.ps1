# tools/rpa/qemu-launch.ps1
#
# Launch QEMU with the GOS kernel bootimage in the background.  Wires
# in the monitor + serial TCP listeners so other RPA scripts can drive
# the running VM.
#
# Returns the QEMU process ID.  If -Wait is given, blocks until the
# kernel reaches `boot: enabling interrupts; entering steady-state`.

[CmdletBinding()]
param(
    [switch] $Wait,
    [int] $WaitTimeoutSec = 60,
    [switch] $FullScreen,
    [switch] $NoBuild
)

. (Join-Path $PSScriptRoot '_common.ps1')

if (-not $NoBuild) {
    Write-Host '→ building kernel ...'
    Push-Location $Script:WorkspaceRoot
    try {
        & cargo build -p gos-kernel 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }
    } finally {
        Pop-Location
    }
}

$image = Get-GosBootImage
Write-Host "→ bootimage: $image"

$qemuArgs = @(
    '-drive', "format=raw,file=$image",
    '-serial',   "stdio",
    '-serial',   "tcp:$($Script:QemuSerialHost):$($Script:QemuSerialPort),server,nowait",
    '-no-reboot',
    '-monitor',  "telnet:$($Script:QemuMonitorHost):$($Script:QemuMonitorPort),server,nowait",
    '-netdev',   "user,id=gosnet0",
    '-device',   "e1000,netdev=gosnet0,mac=52:54:00:12:34:56"
)
if ($FullScreen) { $qemuArgs += '-full-screen' }

Write-Host '→ launching QEMU ...'
$proc = Start-Process -FilePath 'qemu-system-x86_64' -ArgumentList $qemuArgs -PassThru -WindowStyle Hidden -RedirectStandardOutput "$env:TEMP\gos-qemu-stdout.log"

Start-Sleep -Seconds 2  # give the monitor TCP listener time to bind

if ($Wait) {
    Write-Host "→ waiting for boot marker (timeout ${WaitTimeoutSec}s) ..."
    $deadline = (Get-Date).AddSeconds($WaitTimeoutSec)
    $seen = $false
    while ((Get-Date) -lt $deadline -and -not $proc.HasExited) {
        $log = Get-Content "$env:TEMP\gos-qemu-stdout.log" -ErrorAction SilentlyContinue -Raw
        if ($log -and $log.Contains('enabling interrupts')) {
            $seen = $true
            break
        }
        Start-Sleep -Milliseconds 500
    }
    if ($seen) {
        Write-Host "✓ kernel reached steady-state (PID $($proc.Id))"
    } else {
        Write-Warning "boot marker not seen within ${WaitTimeoutSec}s"
    }
}

return $proc.Id
