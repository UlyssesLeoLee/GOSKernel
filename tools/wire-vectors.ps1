param([string]$Root = "E:\GOSKernel")

$map = @{
    "crates\k-vga\src\lib.rs"        = @{ old = "VectorAddress::new(1, 1, 0, 0)"; new = "gos_protocol::vectors::CORE_VGA" }
    "crates\k-serial\src\lib.rs"     = @{ old = "VectorAddress::new(1, 2, 0, 0)"; new = "gos_protocol::vectors::CORE_SERIAL" }
    "crates\k-gdt\src\lib.rs"        = @{ old = "VectorAddress::new(1, 3, 0, 0)"; new = "gos_protocol::vectors::CORE_GDT" }
    "crates\k-idt\src\lib.rs"        = @{ old = "VectorAddress::new(1, 4, 0, 0)"; new = "gos_protocol::vectors::CORE_IDT" }
    "crates\k-pic\src\lib.rs"        = @{ old = "VectorAddress::new(1, 5, 0, 0)"; new = "gos_protocol::vectors::CORE_PIC" }
    "crates\k-pit\src\lib.rs"        = @{ old = "VectorAddress::new(1, 6, 0, 0)"; new = "gos_protocol::vectors::CORE_PIT" }
    "crates\k-ps2\src\lib.rs"        = @{ old = "VectorAddress::new(1, 7, 0, 0)"; new = "gos_protocol::vectors::CORE_PS2" }
    "crates\k-cpuid\src\lib.rs"      = @{ old = "VectorAddress::new(1, 8, 0, 0)"; new = "gos_protocol::vectors::CORE_CPUID" }
    "crates\k-pmm\src\lib.rs"        = @{ old = "VectorAddress::new(1, 11, 0, 0)"; new = "gos_protocol::vectors::CORE_PMM" }
    "crates\k-vmm\src\lib.rs"        = @{ old = "VectorAddress::new(2, 2, 0, 0)"; new = "gos_protocol::vectors::MEM_VMM" }
    "crates\k-heap\src\lib.rs"       = @{ old = "VectorAddress::new(2, 3, 0, 0)"; new = "gos_protocol::vectors::MEM_HEAP" }
    "crates\k-shell\src\lib.rs"      = @{ old = "VectorAddress::new(6, 1, 0, 0)"; new = "gos_protocol::vectors::SVC_SHELL" }
    "crates\k-ai\src\lib.rs"         = @{ old = "VectorAddress::new(6, 2, 0, 0)"; new = "gos_protocol::vectors::SVC_AI" }
    "crates\k-ime\src\lib.rs"        = @{ old = "VectorAddress::new(6, 3, 0, 0)"; new = "gos_protocol::vectors::SVC_IME" }
    "crates\k-net\src\lib.rs"        = @{ old = "VectorAddress::new(6, 4, 0, 0)"; new = "gos_protocol::vectors::SVC_NET" }
    "crates\k-mouse\src\lib.rs"      = @{ old = "VectorAddress::new(6, 5, 0, 0)"; new = "gos_protocol::vectors::SVC_MOUSE" }
    "crates\k-cypher\src\lib.rs"     = @{ old = "VectorAddress::new(6, 6, 0, 0)"; new = "gos_protocol::vectors::SVC_CYPHER" }
    "crates\k-cuda-host\src\lib.rs"  = @{ old = "VectorAddress::new(6, 7, 0, 0)"; new = "gos_protocol::vectors::SVC_CUDA" }
}

foreach ($file in $map.Keys) {
    $path = Join-Path $Root $file
    $content = [System.IO.File]::ReadAllText($path)
    $old = $map[$file].old
    $new = $map[$file].new
    $pattern = "pub const NODE_VEC: VectorAddress = $old;"
    $replacement = "pub const NODE_VEC: VectorAddress = $new;"
    if ($content.Contains($pattern)) {
        $content = $content.Replace($pattern, $replacement)
        [System.IO.File]::WriteAllText($path, $content)
        Write-Host "OK: $file"
    } else {
        Write-Host "SKIP: $file (pattern not found)"
    }
}

# Also wire gos-hal
$halVaddr = Join-Path $Root "crates\gos-hal\src\vaddr.rs"
$c = [System.IO.File]::ReadAllText($halVaddr)
$c = $c.Replace("pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 9, 0, 0);", "pub const NODE_VEC: VectorAddress = gos_protocol::vectors::CORE_HAL_VADDR;")
[System.IO.File]::WriteAllText($halVaddr, $c)
Write-Host "OK: gos-hal/vaddr.rs"

$halMeta = Join-Path $Root "crates\gos-hal\src\meta.rs"
$c = [System.IO.File]::ReadAllText($halMeta)
$c = $c.Replace("pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 10, 0, 0);", "pub const NODE_VEC: VectorAddress = gos_protocol::vectors::CORE_HAL_META;")
[System.IO.File]::WriteAllText($halMeta, $c)
Write-Host "OK: gos-hal/meta.rs"

# Also wire k-shell sub-vectors
$shellPath = Join-Path $Root "crates\k-shell\src\lib.rs"
$c = [System.IO.File]::ReadAllText($shellPath)
$c = $c.Replace("pub const THEME_WABI_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 1, 0);", "pub const THEME_WABI_NODE_VEC: VectorAddress = gos_protocol::vectors::SVC_SHELL_THEME_WABI;")
$c = $c.Replace("pub const THEME_SHOJI_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 2, 0);", "pub const THEME_SHOJI_NODE_VEC: VectorAddress = gos_protocol::vectors::SVC_SHELL_THEME_SHOJI;")
$c = $c.Replace("pub const THEME_CURRENT_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 3, 0);", "pub const THEME_CURRENT_NODE_VEC: VectorAddress = gos_protocol::vectors::SVC_SHELL_THEME_CURRENT;")
$c = $c.Replace("pub const CLIPBOARD_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 4, 0);", "pub const CLIPBOARD_NODE_VEC: VectorAddress = gos_protocol::vectors::SVC_SHELL_CLIPBOARD;")
[System.IO.File]::WriteAllText($shellPath, $c)
Write-Host "OK: k-shell sub-vectors"
