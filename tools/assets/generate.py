#!/usr/bin/env python3
"""
GOS 3D asset generator — pure Python, no external deps.

Outputs PPM (binary P6) into `assets/` at the workspace root.  PPM is
trivial to consume from `pack_palette.py` next stage.

The ray-traced sphere shader mirrors the in-kernel one in
`crates/hypervisor/src/main.rs::draw_node_sphere` so the offline
renderings are a higher-quality "reference" of what the kernel paints
in real time (more samples, true cubemap reflection, no palette
quantization).

Usage:
    python tools/assets/generate.py            # all targets
    python tools/assets/generate.py sphere     # only spheres
    python tools/assets/generate.py env        # only environment cubemap
    python tools/assets/generate.py rope       # only rope strip
    python tools/assets/generate.py brdf       # only BRDF LUT
"""

from __future__ import annotations

import math
import os
import sys
from pathlib import Path
from typing import Iterable

# ── Geometry helpers ────────────────────────────────────────────────

def vec_add(a, b):  return (a[0] + b[0], a[1] + b[1], a[2] + b[2])
def vec_sub(a, b):  return (a[0] - b[0], a[1] - b[1], a[2] - b[2])
def vec_mul(a, s):  return (a[0] * s, a[1] * s, a[2] * s)
def vec_dot(a, b):  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
def vec_len(a):     return math.sqrt(vec_dot(a, a))
def vec_norm(a):
    L = vec_len(a)
    return (a[0] / L, a[1] / L, a[2] / L) if L > 1e-9 else (0.0, 0.0, 1.0)
def vec_reflect(d, n):
    # d - 2 (d·n) n
    k = 2 * vec_dot(d, n)
    return (d[0] - k * n[0], d[1] - k * n[1], d[2] - k * n[2])

# ── Material profiles — mirror crates/hypervisor/src/main.rs ───────

MATERIALS = {
    # name            tint              metallic rough  rim   bumpA bumpF aniso
    "Hardware":     ((0.40, 0.85, 1.00), 0.95,  0.18,  1.20, 0.000,  0.0,  0.00),
    "KernelDriver": ((1.00, 0.55, 0.20), 0.92,  0.25,  1.00, 0.025, 32.0, +0.65),
    "Service":      ((0.35, 1.00, 0.65), 0.80,  0.38,  0.85, 0.018, 22.0, +0.30),
    "Compute":      ((0.95, 0.30, 0.95), 0.88,  0.30,  1.05, 0.030, 28.0, -0.55),
    "Routing":      ((1.00, 0.50, 0.65), 0.70,  0.45,  0.90, 0.022, 18.0, +0.20),
    "Vector":       ((0.65, 0.65, 0.65), 0.40,  0.65,  0.70, 0.055, 14.0,  0.00),
}

LIGHT_KEY  = vec_norm(( 0.55,  0.72, -0.42))
LIGHT_FILL = vec_norm((-0.40,  0.20,  0.30))
LIGHT_SKY  = vec_norm(( 0.00,  1.00,  0.00))
KEY_I,  FILL_I, SKY_I = 1.0, 0.32, 0.18
SUN_ENERGY = 1.85
AMBIENT = 0.10

# ── PBR helpers ─────────────────────────────────────────────────────

def schlick_fresnel(f0, cos_theta):
    return f0 + (1.0 - f0) * (1.0 - cos_theta) ** 5

def ggx_d(n_dot_h, roughness):
    alpha = roughness * roughness
    a2 = alpha * alpha
    denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0
    return a2 / (denom * denom * math.pi + 1e-6)

def saturate(x): return max(0.0, min(1.0, x))

# ── Procedural environment (matches kernel sample_environment) ─────

def env_sample(reflection):
    """Procedural nebula sky.  Used by sphere bake when no cubemap
    is loaded yet; also used to bake the cubemap itself."""
    rx, ry, rz = reflection
    # Sun lobe
    r_dot_l = max(0.0, rx * LIGHT_KEY[0] + ry * LIGHT_KEY[1] + rz * LIGHT_KEY[2])
    sun = r_dot_l ** 24
    # Sky gradient
    sky_t = saturate(ry * 0.5 + 0.5)
    # Horizon glow
    horizon = max(0.0, 1.0 - abs(ry)) * 0.15
    intensity = sun * 0.95 + sky_t * 0.55 + horizon
    intensity = min(intensity, 1.5)
    # Tint: cyan at zenith, magenta at horizon, white near sun lobe
    sky_tint = (0.30, 0.50, 0.85)   # cool blue zenith
    horizon_tint = (0.85, 0.40, 0.65)  # magenta horizon
    sun_tint = (1.00, 0.95, 0.70)
    base = (
        sky_tint[0] * sky_t + horizon_tint[0] * (1 - sky_t),
        sky_tint[1] * sky_t + horizon_tint[1] * (1 - sky_t),
        sky_tint[2] * sky_t + horizon_tint[2] * (1 - sky_t),
    )
    return (
        base[0] * 0.55 + sun_tint[0] * sun + 0.05,
        base[1] * 0.55 + sun_tint[1] * sun + 0.05,
        base[2] * 0.55 + sun_tint[2] * sun + 0.05,
    )

# ── Sphere ray-tracer ──────────────────────────────────────────────

def shade_sphere_pixel(nx, ny, nz, material, supersample_x, supersample_y, size):
    """Apply the full PBR composite at one surface point.  Returns
    RGB in [0, ~2] before tone map."""
    tint, metallic, roughness, rim, bump_amp, bump_freq, aniso = material

    # Micro-surface perturbation
    if bump_amp > 0.0:
        bx = math.sin(nx * bump_freq) * math.cos(ny * bump_freq * 0.7) * bump_amp
        by = math.cos(nx * bump_freq * 0.6) * math.sin(ny * bump_freq) * bump_amp
        n = vec_norm((nx + bx, ny + by, nz))
    else:
        n = (nx, ny, nz)

    # Y is screen-down in kernel; ray-trace uses screen-up so flip ny:
    normal = (n[0], -n[1], n[2])

    view = (0.0, 0.0, 1.0)
    half = vec_norm((LIGHT_KEY[0], LIGHT_KEY[1], LIGHT_KEY[2] + view[2]))

    n_dot_l_key  = max(0.0, vec_dot(normal, LIGHT_KEY))
    n_dot_l_fill = max(0.0, vec_dot(normal, LIGHT_FILL))
    n_dot_l_sky  = max(0.0, vec_dot(normal, LIGHT_SKY))
    n_dot_v = max(0.001, vec_dot(normal, view))
    n_dot_h = max(0.001, vec_dot(normal, half))
    v_dot_h = max(0.001, vec_dot(view, half))

    diffuse_lum = (
        n_dot_l_key  * KEY_I  +
        n_dot_l_fill * FILL_I +
        n_dot_l_sky  * SKY_I
    ) * (1.0 - metallic)

    f0 = 0.04 * (1.0 - metallic) + 0.95 * metallic
    d = ggx_d(n_dot_h, roughness)
    f = schlick_fresnel(f0, v_dot_h)
    spec = d * f * 0.30

    # Anisotropy
    if abs(aniso) >= 0.01:
        comp = half[0] if aniso >= 0 else half[1]
        a = 1.0 - abs(comp) * abs(aniso)
        if a < 0.15: a = 0.15
        spec *= a

    # Environment reflection
    two_ndotv = 2 * vec_dot(normal, view)
    refl = (
        two_ndotv * normal[0],
        two_ndotv * normal[1],
        two_ndotv * normal[2] - 1.0,
    )
    env_rgb = env_sample(refl)
    env_scale = (1.0 - roughness * 0.6) * (0.25 + 0.5 * metallic)

    # Rim
    rim_lum = schlick_fresnel(f0 * 0.2, n_dot_v) * rim * 0.65

    # Composite per channel (use tint for diffuse + rim, env_rgb for reflection,
    # white for specular highlight)
    intensity = AMBIENT + diffuse_lum
    base_r = tint[0] * intensity + rim_lum * tint[0]
    base_g = tint[1] * intensity + rim_lum * tint[1]
    base_b = tint[2] * intensity + rim_lum * tint[2]
    # Specular adds white-tinted energy
    base_r += spec * SUN_ENERGY * (1.0 - metallic + metallic * tint[0])
    base_g += spec * SUN_ENERGY * (1.0 - metallic + metallic * tint[1])
    base_b += spec * SUN_ENERGY * (1.0 - metallic + metallic * tint[2])
    # Environment reflection
    base_r += env_rgb[0] * env_scale
    base_g += env_rgb[1] * env_scale
    base_b += env_rgb[2] * env_scale
    return (base_r, base_g, base_b)


def tonemap_aces(c):
    # Narkowicz ACES filmic approximation
    x = max(0.0, c)
    return (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)


def render_sphere(material_name, size: int, supersample: int = 2):
    """Return a list of (R, G, B) ints in 0..255, size×size."""
    material = MATERIALS[material_name]
    radius = (size / 2) - 1
    cx, cy = (size - 1) / 2, (size - 1) / 2
    pixels = [(0, 0, 0)] * (size * size)
    for py in range(size):
        for px in range(size):
            # Supersample
            r_acc = g_acc = b_acc = 0.0
            n_samples = supersample * supersample
            for sy in range(supersample):
                for sx in range(supersample):
                    fx = px + (sx + 0.5) / supersample
                    fy = py + (sy + 0.5) / supersample
                    dx = (fx - cx) / radius
                    dy = (fy - cy) / radius
                    d2 = dx * dx + dy * dy
                    if d2 > 1.0:
                        # Outside sphere — transparent (use bg = pure black)
                        continue
                    nz = math.sqrt(1.0 - d2)
                    rgb = shade_sphere_pixel(dx, dy, nz, material,
                                              sx, sy, size)
                    r_acc += rgb[0]
                    g_acc += rgb[1]
                    b_acc += rgb[2]
            r = tonemap_aces(r_acc / n_samples)
            g = tonemap_aces(g_acc / n_samples)
            b = tonemap_aces(b_acc / n_samples)
            pixels[py * size + px] = (
                min(255, int(r * 255)),
                min(255, int(g * 255)),
                min(255, int(b * 255)),
            )
    return pixels


# ── Environment cubemap ─────────────────────────────────────────────

def cubemap_face_dir(face: int, u: float, v: float):
    """Face indices: 0=+X 1=-X 2=+Y 3=-Y 4=+Z 5=-Z.  u, v in [-1, 1]."""
    if face == 0: return vec_norm(( 1.0,  -v,  -u))   # +X
    if face == 1: return vec_norm((-1.0,  -v,   u))   # -X
    if face == 2: return vec_norm((   u, 1.0,   v))   # +Y
    if face == 3: return vec_norm((   u, -1.0, -v))   # -Y
    if face == 4: return vec_norm((   u,  -v, 1.0))   # +Z
    return                vec_norm((  -u,  -v,-1.0))  # -Z

def render_cubemap_face(face: int, size: int):
    """Procedural nebula + stars on a single cubemap face."""
    pixels = [(0, 0, 0)] * (size * size)
    for py in range(size):
        for px in range(size):
            u = (px + 0.5) / size * 2 - 1
            v = (py + 0.5) / size * 2 - 1
            dirv = cubemap_face_dir(face, u, v)
            rgb = env_sample(dirv)
            # Procedural stars: tiny bright pixels where hash > threshold
            h = hash((face, px, py)) & 0xFFFF
            if h < 50:
                # very rare bright star
                rgb = (1.0, 1.0, 0.9)
            r = tonemap_aces(rgb[0])
            g = tonemap_aces(rgb[1])
            b = tonemap_aces(rgb[2])
            pixels[py * size + px] = (
                min(255, int(r * 255)),
                min(255, int(g * 255)),
                min(255, int(b * 255)),
            )
    return pixels


# ── Rope braided cable strip ────────────────────────────────────────

def render_rope_strip(length: int = 128, thickness: int = 8):
    """Three-strand braided cable.  Width = length, height = thickness."""
    pixels = [(0, 0, 0)] * (length * thickness)
    base_color = (0.55, 0.45, 0.35)
    for px in range(length):
        # Three strands cross sinusoidally
        for py in range(thickness):
            # Normalize to [-1, 1] for thickness axis
            ty = (py + 0.5) / thickness * 2 - 1
            # Per-strand vertical offset advances with px
            phase = px * 0.25
            best = -1.0
            for strand in range(3):
                center = math.sin(phase + strand * (2 * math.pi / 3)) * 0.55
                dist = abs(ty - center)
                if dist < 0.35:
                    # Cylinder shading: ty distance to strand center →
                    # height into cylinder
                    h = math.sqrt(max(0.0, 0.35 * 0.35 - dist * dist)) / 0.35
                    lum = AMBIENT + h * (1.0 - AMBIENT) + 0.4 * h ** 6
                    if lum > best:
                        best = lum
            if best < 0:
                pixels[py * length + px] = (10, 12, 20)   # background dim
            else:
                r = tonemap_aces(base_color[0] * best)
                g = tonemap_aces(base_color[1] * best)
                b = tonemap_aces(base_color[2] * best)
                pixels[py * length + px] = (
                    min(255, int(r * 255)),
                    min(255, int(g * 255)),
                    min(255, int(b * 255)),
                )
    return pixels


# ── BRDF LUT (split-sum approximation) ──────────────────────────────

def render_brdf_lut(size: int = 128):
    """X axis = N·V (0..1), Y axis = roughness (0..1).  Encodes the
    pre-integrated GGX × Schlick fresnel terms as (R = scale, G = bias)."""
    pixels = [(0, 0, 0)] * (size * size)
    n_samples = 64
    for py in range(size):
        roughness = (py + 0.5) / size
        for px in range(size):
            n_dot_v = (px + 0.5) / size
            v = (math.sqrt(1 - n_dot_v ** 2), 0.0, n_dot_v)
            scale = 0.0
            bias = 0.0
            # Hammersley quasi-Monte-Carlo importance sample
            for i in range(n_samples):
                u1 = (i + 0.5) / n_samples
                u2 = ((i * 13 + 7) % n_samples) / n_samples
                # GGX sample direction
                alpha = roughness * roughness
                phi = 2 * math.pi * u1
                cos_t = math.sqrt((1 - u2) / (1 + (alpha * alpha - 1) * u2))
                sin_t = math.sqrt(1 - cos_t * cos_t)
                h = (sin_t * math.cos(phi), sin_t * math.sin(phi), cos_t)
                v_dot_h = max(0.0, vec_dot(v, h))
                # Reflect v around h → L
                l = (2 * v_dot_h * h[0] - v[0],
                     2 * v_dot_h * h[1] - v[1],
                     2 * v_dot_h * h[2] - v[2])
                n_dot_l = max(0.0, l[2])
                if n_dot_l > 0:
                    g = (n_dot_l * n_dot_v) / max(0.001, n_dot_l + n_dot_v - n_dot_l * n_dot_v)
                    g_vis = g * v_dot_h / max(0.001, h[2] * n_dot_v)
                    fc = (1 - v_dot_h) ** 5
                    scale += (1 - fc) * g_vis
                    bias += fc * g_vis
            scale /= n_samples
            bias /= n_samples
            r = min(255, int(scale * 255))
            g = min(255, int(bias * 255))
            pixels[py * size + px] = (r, g, 0)
    return pixels


# ── PPM I/O ─────────────────────────────────────────────────────────

def write_ppm(path: Path, pixels, width: int, height: int):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('wb') as f:
        f.write(f"P6\n{width} {height}\n255\n".encode('ascii'))
        for (r, g, b) in pixels:
            f.write(bytes([r, g, b]))


# ── Entry point ─────────────────────────────────────────────────────

def workspace_root() -> Path:
    cur = Path(__file__).resolve()
    for _ in range(5):
        cur = cur.parent
        if (cur / 'Cargo.lock').is_file() and (cur / 'crates').is_dir():
            return cur
    raise SystemExit("could not locate workspace root")


def main(args: Iterable[str]):
    args = list(args)
    target = args[0] if args else 'all'

    root = workspace_root()
    out_dir = root / 'assets'

    def do_sphere():
        for cls in MATERIALS:
            for sz in (64, 32):
                print(f"  sphere {cls} {sz}×{sz}")
                pix = render_sphere(cls, sz)
                write_ppm(out_dir / f"sphere_{cls}_{sz}.ppm", pix, sz, sz)

    def do_env():
        face_names = ['posX', 'negX', 'posY', 'negY', 'posZ', 'negZ']
        for face_i, face_name in enumerate(face_names):
            print(f"  env {face_name} 32×32")
            pix = render_cubemap_face(face_i, 32)
            write_ppm(out_dir / f"env_{face_name}.ppm", pix, 32, 32)

    def do_rope():
        print(f"  rope 128×8")
        pix = render_rope_strip(128, 8)
        write_ppm(out_dir / "rope_braided.ppm", pix, 128, 8)

    def do_brdf():
        print(f"  brdf_lut 128×128")
        pix = render_brdf_lut(128)
        write_ppm(out_dir / "brdf_lut.ppm", pix, 128, 128)

    targets = {'sphere': do_sphere, 'env': do_env, 'rope': do_rope, 'brdf': do_brdf}
    print(f"workspace: {root}")
    print(f"output:    {out_dir}")
    if target == 'all':
        for name, fn in targets.items():
            print(f"→ {name}")
            fn()
    elif target in targets:
        print(f"→ {target}")
        targets[target]()
    else:
        print(f"unknown target: {target}", file=sys.stderr)
        print(f"available: all, " + ", ".join(targets), file=sys.stderr)
        return 1
    print("done.")
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
