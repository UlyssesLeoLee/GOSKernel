import os
import re

# Paths
bundle_path = r"e:\GOSKernel\crates\hypervisor\src\builtin_bundle.rs"
crates_dir = r"e:\GOSKernel\crates"

# Map Crate Name to Plugin ID (as defined in builtin_bundle.rs)
crates = {
    "k-core": "K_CORE",
    "k-panic": "K_PANIC",
    "k-serial": "K_SERIAL",
    "k-gdt": "K_GDT",
    "k-cpuid": "K_CPUID",
    "k-pic": "K_PIC",
    "k-pit": "K_PIT",
    "k-ps2": "K_PS2",
    "k-idt": "K_IDT",
    "k-pmm": "K_PMM",
    "k-vmm": "K_VMM",
    "k-heap": "K_HEAP",
    "k-vga": "K_VGA",
    "k-ime": "K_IME",
    "k-net": "K_NET",
    "k-mouse": "K_MOUSE",
    "k-cypher": "K_CYPHER",
    "k-cuda-host": "K_CUDA",
    "k-shell": "K_SHELL",
    "k-ai": "K_AI"
}

with open(bundle_path, "r", encoding="utf-8") as f:
    bundle = f.read()

def extract_array(name):
    """Extracts the contents of a constant array from the bundle."""
    # Find something like: const NAME: &[...] = &[ ... ];
    pattern = r"const\s+" + name + r"\s*(?::\s*&\[[^\]]+\])?\s*=\s*(?:&|)\s*\[(.*?)(?=\];)"
    m = re.search(pattern, bundle, re.DOTALL)
    if m:
        return m.group(1).strip()
    return ""

def get_node_specs(plugin_id):
    """Extracts node specs for a plugin."""
    spec_key = plugin_id.replace("K_", "") + "_NODE_SPECS"
    block = extract_array(spec_key)
    # Search for executor_id and type
    execs = re.findall(r"executor_id:\s*([a-zA-Z0-9_:]+::EXECUTOR_ID|ExecutorId::from_ascii\(\"([^\"]+)\"\))", block)
    node_types = re.findall(r"node_type:\s*RuntimeNodeType::(\w+)", block)
    schemas = re.findall(r"state_schema_hash:\s*(0x[0-9A-Fa-f]+)", block)
    
    results = []
    for i in range(len(execs)):
        executor = execs[i][1] if execs[i][1] else execs[i][0]
        ntype = node_types[i] if i < len(node_types) else "Unknown"
        schema = schemas[i] if i < len(schemas) else "0x0"
        results.append((executor, ntype, schema))
    return results

def get_perms(plugin_id):
    """Extracts permissions (HW resources)."""
    perm_key = plugin_id.replace("K_", "") + "_PERMS"
    block = extract_array(perm_key)
    items = []
    # PermissionSpec { kind: PermissionKind::PortIo, arg0: 0x60, arg1: 0x64 }
    pattern = r"PermissionSpec\s*{\s*kind:\s*PermissionKind::(\w+),\s*arg0:\s*([^,]+),\s*arg1:\s*([^ ]+)\s*}"
    for m in re.finditer(pattern, block):
        items.append((m.group(1), m.group(2), m.group(3)))
    return items

def get_capabilities(plugin_id, suffix="EXPORTS"):
    """Extracts exported or imported capabilities."""
    key = plugin_id.replace("K_", "") + "_" + suffix
    block = extract_array(key)
    # CapabilitySpec { namespace: "console", name: "write" } or ImportSpec { namespace: "console", capability: "write", ... }
    pattern = r"(?:CapabilitySpec|ImportSpec)\s*{\s*namespace:\s*\"([^\"]+)\",\s*(?:name|capability):\s*\"([^\"]+)\""
    return re.findall(pattern, block)

def get_dependencies(plugin_id):
    """Extracts plugin dependencies."""
    key = "DEP_" + plugin_id.replace("K_", "")
    block = extract_array(key)
    return re.findall(r"(K_[A-Z0-9_]+)_ID", block)

for crate, pid in crates.items():
    specs = get_node_specs(pid)
    perms = get_perms(pid)
    exports = get_capabilities(pid, "EXPORTS")
    imports = get_capabilities(pid, "IMPORTS")
    deps = get_dependencies(pid)
    
    if not specs:
        # Fallback for simple manifests
        manifest_name = pid.replace("K_", "") + "_MANIFEST"
        # manifest(K_PIT_ID, "K_PIT", DEP_PIT, PIT_PERMS, &[], &[])
        m = re.search(manifest_name + r"\s*:\s*PluginManifest\s*=\s*(?:manifest|manifest_with_nodes)\(\s*" + pid + r"_ID,\s*\"([^\"]+)\",\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+)", bundle)
        if m:
            deps = re.findall(r"K_[A-Z0-9_]+(?=_ID)", extract_array(m.group(2)) if "DEP_" in m.group(2) else m.group(2))
            perms = get_perms(pid) # try again with PID
            exports = get_capabilities(pid, "EXPORTS")
            imports = get_capabilities(pid, "IMPORTS")
            specs = [("Unknown", "Generic", "0x0")]

    # Build Cypher
    lines = []
    lines.append("// " + "="*60)
    lines.append(f"// GOS KERNEL TOPOLOGY — {crate}")
    lines.append("// This Cypher script documents the plugin's place in the kernel graph.")
    lines.append("//")
    lines.append(f"MERGE (p:Plugin {{id: \"{pid}\", name: \"{crate}\"}})")
    
    for i, (executor, ntype, schema) in enumerate(specs):
        lines.append(f"SET p.executor = \"{executor}\", p.node_type = \"{ntype}\", p.state_schema = \"{schema}\"")
    
    if deps:
        lines.append("//")
        lines.append("// -- Dependencies")
        for d in deps:
            lines.append(f"MERGE (dep_{d}:Plugin {{id: \"{d}\"}})")
            lines.append(f"MERGE (p)-[:DEPENDS_ON]->(dep_{d})")
            
    if perms:
        lines.append("//")
        lines.append("// -- Hardware Resources")
        for kind, a0, a1 in perms:
            if kind == "PortIo":
                lines.append(f"MERGE (pr_{a0.replace('0x','')}:PortRange {{start: \"{a0}\", end: \"{a1}\"}})")
                lines.append(f"MERGE (p)-[:REQUIRES_PORT]->(pr_{a0.replace('0x','')})")
            elif kind == "IrqBind":
                lines.append(f"MERGE (irq_{a0}:InterruptLine {{irq: \"{a0}\"}})")
                lines.append(f"MERGE (p)-[:BINDS_IRQ]->(irq_{a0})")
            elif kind == "PhysMap":
                lines.append(f"MERGE (pm_{a0.replace('0x','')}:PhysMap {{base: \"{a0}\", size: \"{a1}\"}})")
                lines.append(f"MERGE (p)-[:MAPS_PHYS]->(pm_{a0.replace('0x','')})")

    if exports:
        lines.append("//")
        lines.append("// -- Exported Capabilities (APIs)")
        for ns, nm in exports:
            lines.append(f"MERGE (cap_{ns}_{nm}:Capability {{namespace: \"{ns}\", name: \"{nm}\"}})")
            lines.append(f"MERGE (p)-[:EXPORTS]->(cap_{ns}_{nm})")

    if imports:
        lines.append("//")
        lines.append("// -- Imported Capabilities (Dependencies)")
        for ns, nm in imports:
            lines.append(f"MERGE (cap_{ns}_{nm}:Capability {{namespace: \"{ns}\", name: \"{nm}\"}})")
            lines.append(f"MERGE (p)-[:IMPORTS]->(cap_{ns}_{nm})")

    lines.append("// " + "="*60)
    
    cypher_block = "\n".join(["// " + l if not l.startswith("//") else l for l in lines]) + "\n"
    
    # Inject
    file_path = os.path.join(crates_dir, crate, "src", "lib.rs")
    if os.path.exists(file_path):
        with open(file_path, "r", encoding="utf-8") as f:
            content = f.read()
        
        # Identity logic: if it has the header, replace it. Otherwise prepend.
        if "// GOS KERNEL TOPOLOGY" in content:
            # Replace existing block
            new_content = re.sub(r"// =+.*?// =+", cypher_block, content, flags=re.DOTALL)
        else:
            # Prepend after #![no_std]
            m = re.search(r"(#![no_std]\s*)", content)
            if m:
                new_content = content[:m.end()] + "\n" + cypher_block + content[m.end():]
            else:
                new_content = cypher_block + "\n" + content
        
        with open(file_path, "w", encoding="utf-8") as f:
            f.write(new_content)
        print(f"Updated {crate}")
    else:
        print(f"Skipping {crate}: file not found")
