Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$harnessRoot = Join-Path $PSScriptRoot "gos-supervisor-harness"

Push-Location $repoRoot
try {
    cargo check -p gos-supervisor
    cargo check -p gos-kernel
}
finally {
    Pop-Location
}

Push-Location $harnessRoot
try {
    cargo test -- --test-threads=1
}
finally {
    Pop-Location
}
