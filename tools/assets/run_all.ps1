# tools/assets/run_all.ps1
#
# 一键重新生成 + pack 全部 GOS 3D 资产.
#   1. 调用 generate.py 生成 PPM 到 assets/
#   2. 调用 pack_palette.py --all 转成 .pal
#   3. (可选 -Build) 顺带跑 cargo build -p k-assets 验证 include_bytes! 成功

[CmdletBinding()]
param(
    [switch] $Build,
    [string] $PythonExe = 'python'
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

Write-Host "→ Generating PPMs ..."
& $PythonExe (Join-Path $PSScriptRoot 'generate.py') all
if ($LASTEXITCODE -ne 0) { throw "generate.py failed" }

Write-Host "→ Packing to palette-indexed .pal ..."
& $PythonExe (Join-Path $PSScriptRoot 'pack_palette.py') --all
if ($LASTEXITCODE -ne 0) { throw "pack_palette.py failed" }

if ($Build) {
    Write-Host "→ Building k-assets to verify embed paths ..."
    Push-Location $root
    try {
        & cargo build -p k-assets 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "cargo build -p k-assets failed" }
    } finally {
        Pop-Location
    }
}

$ppmCount = (Get-ChildItem -Path (Join-Path $root 'assets') -Filter '*.ppm').Count
$palCount = (Get-ChildItem -Path (Join-Path $root 'assets') -Filter '*.pal').Count
Write-Host "✓ done. $ppmCount PPM, $palCount .pal in assets/"
