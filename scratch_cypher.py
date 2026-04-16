"""
GOS Kernel — Cypher Graph Comment Injector
将每个内核插件的完整拓扑描述（Neo4j Cypher）以普通注释的形式注入到各 crate 的 lib.rs 顶部。
注释格式为 `// ...`，对 Rust 编译器完全透明，不影响任何功能。
"""

import os
import re

bundle_path = r"e:\GOSKernel\crates\hypervisor\src\builtin_bundle.rs"
with open(bundle_path, "r", encoding="utf-8") as f:
    bundle = f.read()

# ── 所有模块的静态补充描述，用于 Cypher 注释生成 ──────────────────────────────
MODULE_META = {
    "K_CORE":   {"crate": "k-core",       "executor": "native.core",    "node_type": "Service",     "schema": "0x2021", "hw": []},
    "K_PANIC":  {"crate": "k-panic",      "executor": "native.panic",   "node_type": "Service",     "schema": "0x2001", "hw": []},
    "K_SERIAL": {"crate": "k-serial",     "executor": "native.serial",  "node_type": "Driver",      "schema": "0x2002",
                 "hw": [("PortRange", "0x3F8", "8", "COM1 Serial Port")]},
    "K_GDT":    {"crate": "k-gdt",        "executor": "native.gdt",     "node_type": "Service",     "schema": "0x2004", "hw": []},
    "K_CPUID":  {"crate": "k-cpuid",      "executor": "native.cpuid",   "node_type": "Service",     "schema": "0x2005", "hw": []},
    "K_PIC":    {"crate": "k-pic",        "executor": "native.pic",     "node_type": "Driver",      "schema": "0x2006",
                 "hw": [("PortRange", "0x20", "0xA1", "PIC Master+Slave"),
                         ("InterruptLine", "ALL", "", "IRQ 0-15 Master")]},
    "K_PIT":    {"crate": "k-pit",        "executor": "native.pit",     "node_type": "Driver",      "schema": "0x2007",
                 "hw": [("PortRange", "0x40", "0x43", "PIT Channels"),
                         ("InterruptLine", "0", "", "IRQ0 Timer")]},
    "K_PS2":    {"crate": "k-ps2",        "executor": "native.ps2",     "node_type": "Driver",      "schema": "0x2008",
                 "hw": [("PortRange", "0x60", "0x64", "PS/2 Data+Status"),
                         ("InterruptLine", "1", "", "IRQ1 Keyboard")]},
    "K_IDT":    {"crate": "k-idt",        "executor": "native.idt",     "node_type": "Service",     "schema": "0x2009",
                 "hw": [("InterruptLine", "ALL", "", "全局中断向量表")]},
    "K_PMM":    {"crate": "k-pmm",        "executor": "native.pmm",     "node_type": "Service",     "schema": "0x200A", "hw": []},
    "K_VMM":    {"crate": "k-vmm",        "executor": "native.vmm",     "node_type": "Service",     "schema": "0x200B", "hw": []},
    "K_HEAP":   {"crate": "k-heap",       "executor": "native.heap",    "node_type": "Service",     "schema": "0x200C", "hw": []},
    "K_VGA":    {"crate": "k-vga",        "executor": "native.vga",     "node_type": "Driver",      "schema": "0x2003",
                 "hw": [("PortRange", "0x3C8", "0x3C9", "VGA DAC Palette"),
                         ("PhysMap",   "0xA0000", "65536", "VGA VRAM 64K")]},
    "K_IME":    {"crate": "k-ime",        "executor": "native.ime",     "node_type": "Router",      "schema": "0x2011", "hw": []},
    "K_NET":    {"crate": "k-net",        "executor": "native.net",     "node_type": "Driver",      "schema": "0x2015",
                 "hw": [("PortRange", "0xCF8", "8", "PCI Config Address+Data")]},
    "K_MOUSE":  {"crate": "k-mouse",      "executor": "native.mouse",   "node_type": "Driver",      "schema": "0x2013",
                 "hw": [("PortRange", "0x60", "0x64", "PS/2 Data+Status"),
                         ("InterruptLine", "12", "", "IRQ12 Mouse")]},
    "K_CYPHER": {"crate": "k-cypher",     "executor": "native.cypher",  "node_type": "Router",      "schema": "0x2014", "hw": []},
    "K_CUDA":   {"crate": "k-cuda-host",  "executor": "native.cuda",    "node_type": "Compute",     "schema": "0x2016", "hw": []},
    "K_SHELL":  {"crate": "k-shell",      "executor": "native.shell",   "node_type": "PluginEntry", "schema": "0x200E", "hw": []},
    "K_AI":     {"crate": "k-ai",         "executor": "native.ai",      "node_type": "Aggregator",  "schema": "0x2011", "hw": []},
}

def extract_array(block_name):
    m = re.search(r"const\s+" + re.escape(block_name) + r"\s*:[^=]+=\s*&\[(.*?)\];", bundle, re.DOTALL)
    return m.group(1).strip() if m else ""

def parse_dep_plugins(dep_constant):
    block = extract_array(dep_constant)
    return re.findall(r"K_[A-Z0-9]+(?=_ID)", block)

def parse_exports(exp_constant):
    block = extract_array(exp_constant)
    return re.findall(r'namespace:\s*"([^"]+)",\s*name:\s*"([^"]+)"', block)

def parse_imports(imp_constant):
    block = extract_array(imp_constant)
    return re.findall(r'namespace:\s*"([^"]+)",\s*capability:\s*"([^"]+)"', imp_constant_expanded := extract_array(imp_constant))

def parse_imports_from_bundle(plugin_id):
    imp_key = plugin_id.replace("K_", "") + "_IMPORTS"
    block = extract_array(imp_key)
    return re.findall(r'namespace:\s*"([^"]+)",\s*capability:\s*"([^"]+)"', block)

def parse_exports_from_bundle(plugin_id):
    exp_key = plugin_id.replace("K_", "") + "_EXPORTS"
    block = extract_array(exp_key)
    return re.findall(r'namespace:\s*"([^"]+)",\s*name:\s*"([^"]+)"', block)

def parse_deps_from_bundle(plugin_id):
    dep_key = "DEP_" + plugin_id.replace("K_", "")
    block = extract_array(dep_key)
    return re.findall(r"K_[A-Z0-9]+(?=_ID)", block)

def build_cypher_comment(plugin_id, meta):
    crate       = meta["crate"]
    executor    = meta["executor"]
    node_type   = meta["node_type"]
    schema      = meta["schema"]
    hw_list     = meta["hw"]
    deps        = parse_deps_from_bundle(plugin_id)
    exports     = parse_exports_from_bundle(plugin_id)
    imports     = parse_imports_from_bundle(plugin_id)

    # Also check CLIPBOARD_EXPORTS for shell
    if plugin_id == "K_SHELL":
        exports += parse_exports_from_bundle("CLIPBOARD")

    lines = []
    div = "=" * 62
    lines.append(f"// {div}")
    lines.append(f"// GOS KERNEL TOPOLOGY — {crate} ({executor})")
    lines.append(f"// 以下 Cypher 脚本可直接导入 Neo4j，与其他模块共同还原内核完整图谱。")
    lines.append(f"//")
    lines.append(f"// MERGE (p:Plugin {{id: \"{plugin_id}\", name: \"{crate}\"}})")
    lines.append(f"// SET p.executor = \"{executor}\", p.node_type = \"{node_type}\", p.state_schema = \"{schema}\"")
    
    if deps:
        lines.append(f"//")
        lines.append(f"// // ── 启动依赖 (DEPENDS_ON) ──────────────────────────────────")
        for dep in deps:
            var = dep.lower()
            lines.append(f"// MERGE ({var}:Plugin {{id: \"{dep}\"}})")
            lines.append(f"// MERGE (p)-[:DEPENDS_ON {{required: true}}]->({var})")

    if hw_list:
        lines.append(f"//")
        lines.append(f"// // ── 硬件资源边界 ──────────────────────────────────────────")
        for hw in hw_list:
            kind, a, b, label = hw
            if kind == "PortRange":
                lines.append(f"// MERGE (hw_{a.replace('0x','').lower()}:PortRange {{start: \"{a}\", end: \"{b}\", label: \"{label}\"}})")
                lines.append(f"// MERGE (p)-[:REQUIRES_PORT]->(hw_{a.replace('0x','').lower()})")
            elif kind == "InterruptLine":
                lines.append(f"// MERGE (irq_{a}:InterruptLine {{irq: \"{a}\", label: \"{label}\"}})")
                lines.append(f"// MERGE (p)-[:BINDS_IRQ]->(irq_{a})")
            elif kind == "PhysMap":
                lines.append(f"// MERGE (physmap_{a.replace('0x','').lower()}:PhysMap {{base: \"{a}\", size: \"{b}\", label: \"{label}\"}})")
                lines.append(f"// MERGE (p)-[:MAPS_PHYS]->(physmap_{a.replace('0x','').lower()})")

    if exports:
        lines.append(f"//")
        lines.append(f"// // ── 能力导出 (EXPORTS Capability) ────────────────────────")
        for ns, nm in exports:
            var = f"cap_{ns}_{nm}".replace("-", "_")
            lines.append(f"// MERGE ({var}:Capability {{namespace: \"{ns}\", name: \"{nm}\"}})")
            lines.append(f"// MERGE (p)-[:EXPORTS]->({var})")

    if imports:
        lines.append(f"//")
        lines.append(f"// // ── 能力消费 (IMPORTS Capability, resolved at on_init) ───")
        for ns, nm in imports:
            var = f"cap_{ns}_{nm}".replace("-", "_")
            lines.append(f"// MERGE ({var}:Capability {{namespace: \"{ns}\", name: \"{nm}\"}})")
            lines.append(f"// MERGE (p)-[:IMPORTS]->({var})")

    lines.append(f"// {div}")
    return "\n".join(lines) + "\n"

def inject_into_file(file_path, cypher_block):
    with open(file_path, "r", encoding="utf-8") as f:
        content = f.read()

    # Idempotent guard
    if "GOS KERNEL TOPOLOGY" in content:
        print(f"  [SKIP] Already has topology header: {file_path}")
        return

    lines = content.split("\n")

    # Find insert position: after all #![...] inner attributes and blank lines
    insert_idx = 0
    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("#!["):
            insert_idx = i + 1
        elif i == insert_idx and stripped == "":
            insert_idx = i + 1  # skip blank lines immediately after attrs

    lines.insert(insert_idx, "\n" + cypher_block)
    with open(file_path, "w", encoding="utf-8", newline="\r\n") as f:
        f.write("\n".join(lines))
    print(f"  [OK] Injected into {file_path}")

for plugin_id, meta in MODULE_META.items():
    crate = meta["crate"]
    file_path = os.path.join(r"e:\GOSKernel\crates", crate, "src", "lib.rs")
    if not os.path.exists(file_path):
        print(f"  [MISS] {file_path}")
        continue
    cypher = build_cypher_comment(plugin_id, meta)
    inject_into_file(file_path, cypher)

print("\nDone. Run 'cargo check' to verify compilation is unaffected.")
