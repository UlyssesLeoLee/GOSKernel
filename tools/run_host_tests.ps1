# Run all GOSKernel host-test harnesses and report pass/fail.
# Usage: pwsh -NoProfile -File tools/run_host_tests.ps1 [filter]
# Optional filter: only run harnesses whose name contains the filter string.

param([string]$Filter = "")

$root    = Split-Path $PSScriptRoot -Parent
$testDir = Join-Path $root "host-tests"

$harnesses = Get-ChildItem $testDir -Directory |
    Where-Object { $_.Name -like "gos-*-harness" } |
    Where-Object { $Filter -eq "" -or $_.Name -like "*$Filter*" } |
    Sort-Object Name

$pass   = @()
$fail   = @()
$total  = $harnesses.Count
$idx    = 0

$width = ($harnesses | ForEach-Object { $_.Name.Length } | Measure-Object -Maximum).Maximum

foreach ($h in $harnesses) {
    $idx++
    $label = $h.Name.PadRight($width)
    Write-Host "[$idx/$total] $label " -NoNewline

    Push-Location $h.FullName
    $result = & cargo test --quiet 2>&1
    $exitCode = $LASTEXITCODE
    Pop-Location
    if ($exitCode -eq 0) {
        Write-Host "PASS" -ForegroundColor Green
        $pass += $h.Name
    } else {
        Write-Host "FAIL" -ForegroundColor Red
        # Print first 30 lines of output on failure (skip warnings, show errors/test results)
        $result | Where-Object { $_ -notmatch '^\s*\^' -and $_ -notmatch 'note:' } |
            Select-Object -First 30 | ForEach-Object { Write-Host "    $_" -ForegroundColor Yellow }
        $fail += $h.Name
    }
}

Write-Host ""
Write-Host ("=" * 60)
Write-Host "Results: $($pass.Count) passed, $($fail.Count) failed  (of $total)"
if ($fail.Count -gt 0) {
    Write-Host "FAILED:" -ForegroundColor Red
    $fail | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    exit 1
} else {
    Write-Host "All tests passed." -ForegroundColor Green
    exit 0
}
