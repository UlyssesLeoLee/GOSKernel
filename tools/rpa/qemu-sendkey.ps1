# tools/rpa/qemu-sendkey.ps1
#
# Send a string + optional terminator key as QEMU `sendkey` commands.
# `sendkey` syntax accepts named keys (ret, esc, tab, backspace,
# ctrl-c, …) and lowercase letters / digits / punctuation.
#
# Examples:
#   .\qemu-sendkey.ps1 -Keys 'kernel' -Then ret           # types 'kernel' then Enter
#   .\qemu-sendkey.ps1 -Keys 'help'   -Then ret
#   .\qemu-sendkey.ps1 -Keys 'show stats' -Then ret
#   .\qemu-sendkey.ps1 -Then esc                          # just Esc

[CmdletBinding()]
param(
    [string] $Keys = '',
    [string] $Then = '',
    [int] $DelayMs = 35
)

. (Join-Path $PSScriptRoot '_common.ps1')

# Mapping of US-ASCII chars → QEMU sendkey tokens.  Only printable
# ASCII; anything else is dropped with a warning.
function Get-QemuKeyToken {
    param([char] $Ch)
    switch ($Ch) {
        ' '   { return 'spc' }
        '.'   { return 'dot' }
        ','   { return 'comma' }
        ';'   { return 'semicolon' }
        ':'   { return 'shift-semicolon' }
        '-'   { return 'minus' }
        '='   { return 'equal' }
        '/'   { return 'slash' }
        '\\'  { return 'backslash' }
        '`'   { return 'grave_accent' }
        "'"   { return 'apostrophe' }
        '['   { return 'bracket_left' }
        ']'   { return 'bracket_right' }
        '!'   { return 'shift-1' }
        '@'   { return 'shift-2' }
        '#'   { return 'shift-3' }
        '$'   { return 'shift-4' }
        '%'   { return 'shift-5' }
        '^'   { return 'shift-6' }
        '&'   { return 'shift-7' }
        '*'   { return 'shift-8' }
        '('   { return 'shift-9' }
        ')'   { return 'shift-0' }
        '_'   { return 'shift-minus' }
        '+'   { return 'shift-equal' }
        '{'   { return 'shift-bracket_left' }
        '}'   { return 'shift-bracket_right' }
        '|'   { return 'shift-backslash' }
        '"'   { return 'shift-apostrophe' }
        '<'   { return 'shift-comma' }
        '>'   { return 'shift-dot' }
        '?'   { return 'shift-slash' }
        '~'   { return 'shift-grave_accent' }
        default {
            $code = [int][char]$Ch
            if ($code -ge 48 -and $code -le 57)  { return [string]$Ch }                          # 0..9
            if ($code -ge 97 -and $code -le 122) { return [string]$Ch }                          # a..z (lowercase)
            if ($code -ge 65 -and $code -le 90)  { return "shift-$([char]($code + 32))" }        # A..Z (shift+lower)
            return $null
        }
    }
}

$conn = $null
try {
    $conn = Connect-QemuMonitor
    foreach ($ch in $Keys.ToCharArray()) {
        $tok = Get-QemuKeyToken -Ch $ch
        if (-not $tok) { Write-Warning "drop unmappable char: $ch"; continue }
        [void](Send-QemuMonitor -Conn $conn -Command "sendkey $tok" -TimeoutMs 500)
        if ($DelayMs -gt 0) { Start-Sleep -Milliseconds $DelayMs }
    }
    if ($Then) {
        [void](Send-QemuMonitor -Conn $conn -Command "sendkey $Then" -TimeoutMs 500)
    }
} finally {
    Close-QemuMonitor -Conn $conn
}
