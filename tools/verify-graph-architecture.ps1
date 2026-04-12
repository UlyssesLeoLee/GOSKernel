param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Assert-Rule {
    param(
        [bool]$Condition,
        [string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Read-RepoFile {
    param([string]$RelativePath)
    $path = Join-Path $RepoRoot $RelativePath
    Assert-Rule (Test-Path $path) "Missing required file: $RelativePath"
    return Get-Content -Path $path -Raw
}

function Test-VectorRefBlocks {
    param(
        [string]$FilePath,
        [string]$TypeName
    )

    $lines = Get-Content -Path $FilePath
    for ($idx = 0; $idx -lt $lines.Count; $idx++) {
        $line = $lines[$idx]
        if ($line -match "\b$TypeName\s*\{" -and $line -notmatch "pub struct $TypeName\s*\{") {
            $windowEnd = [Math]::Min($idx + 20, $lines.Count - 1)
            $window = ($lines[$idx..$windowEnd] -join "`n")
            Assert-Rule ($window -match "vector_ref\s*:") "$TypeName literal in $FilePath near line $($idx + 1) is missing vector_ref"
        }
    }
}

$legacyAllowlist = @(
    "k-pit",
    "k-ps2",
    "k-idt",
    "k-pmm",
    "k-vmm",
    "k-heap"
)

$legacyBundleCrateAllowlist = @(
    "k_pit",
    "k_ps2",
    "k_idt",
    "k_pmm",
    "k_vmm",
    "k_heap"
)

$allowedStaticMutFiles = @(
    (Join-Path $RepoRoot "crates\gos-hal\src\vaddr.rs"),
    (Join-Path $RepoRoot "crates\k-gdt\src\lib.rs"),
    (Join-Path $RepoRoot "crates\k-idt\src\lib.rs"),
    (Join-Path $RepoRoot "crates\k-pmm\src\lib.rs"),
    (Join-Path $RepoRoot "crates\k-vmm\src\lib.rs")
)

$primeRules = Join-Path $RepoRoot "doc\RULE_GRAPH_PRIME.md"
$governance = Join-Path $RepoRoot "doc\GOS_GOVERNANCE_v0_2.md"
Assert-Rule (Test-Path $primeRules) "Missing graph prime rules document"
Assert-Rule (Test-Path $governance) "Missing governance document"

$main = Read-RepoFile "crates\hypervisor\src\main.rs"
Assert-Rule ($main -match "builtin_bundle::boot_builtin_graph") "kernel_main must bootstrap builtin graph through builtin_bundle::boot_builtin_graph"
Assert-Rule ($main -match "gos_supervisor::service_system_cycle") "kernel_main must delegate runtime service to gos_supervisor::service_system_cycle"
Assert-Rule ($main -notmatch "gos_loader::load_bundle") "kernel_main must not route boot through gos_loader::load_bundle anymore"
Assert-Rule ($main -notmatch "gos_runtime::pump") "kernel_main must not directly pump gos_runtime"
Assert-Rule ($main -notmatch "plugin_main\s*\(") "kernel_main must not directly call plugin_main"
Assert-Rule ($main -notmatch "k_[a-z0-9_]+::NODE_VEC") "kernel_main must not hardcode plugin node vectors"
Assert-Rule ($main -notmatch "post_signal\s*\(") "kernel_main must not directly post startup signals"

$bundle = Read-RepoFile "crates\hypervisor\src\builtin_bundle.rs"
Assert-Rule ($bundle -match "BuiltinModule::Native") "Builtin boot registry must include native modules"
Assert-Rule ($bundle -match "boot_builtin_graph") "Builtin boot registry must expose direct graph bootstrap"

$legacyBundleMatches = [regex]::Matches($bundle, "legacy_node\((k_[a-z0-9_]+)::NODE_VEC")
foreach ($match in $legacyBundleMatches) {
    $crateIdent = $match.Groups[1].Value
    Assert-Rule ($legacyBundleCrateAllowlist -contains $crateIdent) "Legacy bundle module $crateIdent is outside the approved migration island"
}

$crateDirs = Get-ChildItem -Path (Join-Path $RepoRoot "crates") -Directory | Where-Object { $_.Name -like "k-*" }
foreach ($crate in $crateDirs) {
    $libPath = Join-Path $crate.FullName "src\lib.rs"
    if (-not (Test-Path $libPath)) {
        continue
    }

    $content = Get-Content -Path $libPath -Raw
    $isLegacy = $legacyAllowlist -contains $crate.Name
    $hasLegacyTrait = $content -match "impl\s+NodeCell" -or $content -match "impl\s+PluginEntry" -or $content -match "try_mount_cell\s*\("
    $hasNativeExecutor = $content -match "EXECUTOR_ID" -and $content -match "EXECUTOR_VTABLE"

    if ($isLegacy) {
        continue
    }

    Assert-Rule (-not $hasLegacyTrait) "$($crate.Name) is outside the approved legacy island but still uses NodeCell/PluginEntry/try_mount_cell"
    Assert-Rule ($content -match "pub const NODE_VEC") "$($crate.Name) must define NODE_VEC"
    Assert-Rule ($hasNativeExecutor) "$($crate.Name) must define EXECUTOR_ID and EXECUTOR_VTABLE"
}

$rustFiles = Get-ChildItem -Path (Join-Path $RepoRoot "crates") -Recurse -File -Filter *.rs
foreach ($file in $rustFiles) {
    $staticMutMatches = Select-String -Path $file.FullName -Pattern "^\s*static mut\s+" -AllMatches
    if ($staticMutMatches) {
        Assert-Rule ($allowedStaticMutFiles -contains $file.FullName) "Unexpected static mut global in $($file.FullName)"
    }

    Test-VectorRefBlocks -FilePath $file.FullName -TypeName "NodeSpec"
    Test-VectorRefBlocks -FilePath $file.FullName -TypeName "EdgeSpec"
}

Write-Host "[graph-governance] OK: repository satisfies graph and vector architecture rules." -ForegroundColor Green
