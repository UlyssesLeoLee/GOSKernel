# tools/rpa/_common.ps1
#
# Shared helpers for the GOS QEMU RPA scripts.  All other scripts
# `. .\_common.ps1` to pick up the constants + functions below.

$Script:QemuMonitorHost = '127.0.0.1'
$Script:QemuMonitorPort = 45555
$Script:QemuSerialHost  = '127.0.0.1'
$Script:QemuSerialPort  = 14444
$Script:WorkspaceRoot   = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

# Connect a TCP client to the QEMU monitor.  Returns a hashtable
# { Client, Stream, Reader, Writer } the caller passes to subsequent
# helpers.  Throws if connection fails.
function Connect-QemuMonitor {
    param(
        [int] $TimeoutMs = 3000
    )
    $client = New-Object System.Net.Sockets.TcpClient
    $connect = $client.BeginConnect($Script:QemuMonitorHost, $Script:QemuMonitorPort, $null, $null)
    if (-not $connect.AsyncWaitHandle.WaitOne($TimeoutMs)) {
        $client.Close()
        throw "QEMU monitor not reachable at $Script:QemuMonitorHost`:$Script:QemuMonitorPort"
    }
    $client.EndConnect($connect)
    $stream = $client.GetStream()
    $reader = New-Object System.IO.StreamReader($stream, [System.Text.Encoding]::ASCII)
    $writer = New-Object System.IO.StreamWriter($stream, [System.Text.Encoding]::ASCII)
    $writer.AutoFlush = $true

    # Drain the QEMU monitor banner up to the first prompt `(qemu) `.
    $stream.ReadTimeout = 1500
    $banner = ''
    try {
        while ($true) {
            $b = $stream.ReadByte()
            if ($b -lt 0) { break }
            $banner += [char]$b
            if ($banner.EndsWith('(qemu) ')) { break }
        }
    } catch [System.IO.IOException] {
        # Read timeout — banner may already be in place but no prompt char.
    }

    return @{
        Client = $client
        Stream = $stream
        Reader = $reader
        Writer = $writer
        Banner = $banner
    }
}

# Send a single HMP command and read the response up to the next
# `(qemu) ` prompt.  Returns the full response string (without the
# prompt itself).
function Send-QemuMonitor {
    param(
        [hashtable] $Conn,
        [string] $Command,
        [int] $TimeoutMs = 3000
    )
    $Conn.Writer.WriteLine($Command)
    $Conn.Stream.ReadTimeout = $TimeoutMs
    $resp = ''
    try {
        while ($true) {
            $b = $Conn.Stream.ReadByte()
            if ($b -lt 0) { break }
            $resp += [char]$b
            if ($resp.EndsWith('(qemu) ')) {
                $resp = $resp.Substring(0, $resp.Length - 7)
                break
            }
        }
    } catch [System.IO.IOException] {
        # timeout — return whatever we have
    }
    return $resp
}

function Close-QemuMonitor {
    param([hashtable] $Conn)
    if ($null -ne $Conn) {
        try { $Conn.Writer.Close() } catch {}
        try { $Conn.Reader.Close() } catch {}
        try { $Conn.Stream.Close() } catch {}
        try { $Conn.Client.Close() } catch {}
    }
}

# Inject raw bytes into the QEMU COM1 serial stream — simulates the
# kernel receiving bytes as if from an external serial console.  The
# GOS boot UI doesn't currently read serial input, but this exists for
# future-proofing.
function Send-QemuSerial {
    param(
        [byte[]] $Bytes,
        [int] $TimeoutMs = 1000
    )
    $client = New-Object System.Net.Sockets.TcpClient
    $connect = $client.BeginConnect($Script:QemuSerialHost, $Script:QemuSerialPort, $null, $null)
    if (-not $connect.AsyncWaitHandle.WaitOne($TimeoutMs)) {
        $client.Close()
        throw "QEMU serial not reachable at $Script:QemuSerialHost`:$Script:QemuSerialPort"
    }
    $client.EndConnect($connect)
    $stream = $client.GetStream()
    $stream.Write($Bytes, 0, $Bytes.Length)
    $stream.Flush()
    $stream.Close()
    $client.Close()
}

# Read bytes that have streamed out of QEMU's COM1.  This is what
# `raw_serial_println` in the kernel writes to.  If $WaitForMarker is
# given, keep reading until that substring appears or timeout.
function Read-QemuSerial {
    param(
        [int] $TimeoutMs = 5000,
        [string] $WaitForMarker = ''
    )
    $client = New-Object System.Net.Sockets.TcpClient
    $client.Connect($Script:QemuSerialHost, $Script:QemuSerialPort)
    $stream = $client.GetStream()
    $stream.ReadTimeout = 250
    $buf = ''
    $deadline = (Get-Date).AddMilliseconds($TimeoutMs)
    while ((Get-Date) -lt $deadline) {
        try {
            while ($stream.DataAvailable) {
                $b = $stream.ReadByte()
                if ($b -lt 0) { break }
                $buf += [char]$b
            }
        } catch [System.IO.IOException] {}
        if ($WaitForMarker -and $buf.Contains($WaitForMarker)) {
            break
        }
        Start-Sleep -Milliseconds 100
    }
    $stream.Close()
    $client.Close()
    return $buf
}

# Resolve the GOS bootimage path (cached by xtask/cargo).
function Get-GosBootImage {
    $candidate = 'E:\DevCache\cargo\target\x86_64-gos-kernel\debug\bootimage-gos-kernel.bin'
    if (Test-Path $candidate) { return $candidate }
    # Fallback: walk the worktree's target tree.
    $found = Get-ChildItem -Path $Script:WorkspaceRoot -Filter 'bootimage-gos-kernel.bin' -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($found) { return $found.FullName }
    throw 'bootimage-gos-kernel.bin not found — build first with cargo build -p gos-kernel'
}
