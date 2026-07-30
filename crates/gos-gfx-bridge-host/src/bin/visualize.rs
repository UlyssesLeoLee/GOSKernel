//! `gos-visualize` — Phase I.2.x cosmic-graph view.
//!
//! Run with: `rustup run stable cargo run --release --bin gos-visualize`
//! (from the bridge-host crate dir).
//!
//! Opens a real window via winit, attaches a wgpu surface, generates
//! synthetic graph nodes + edges, and renders:
//!
//!   * **Nodes** as small metallic spheres (UV-sphere geometry from
//!     `k-scene`), shaded with Blinn-Phong specular + fresnel rim
//!     against a key light and ambient fill — the "metal ball"
//!     reading.
//!   * **Edges** as 6-sided cable tubes between sphere centres, same
//!     lit pipeline — the "rope" reading.
//!   * **Background** as a procedural starfield (full-screen
//!     triangle, hash-noise stars, subtle nebula gradient) at the
//!     far plane so spheres / cables overdraw naturally.
//!
//! Controls (unchanged from earlier slices):
//!   * Auto-rotate yaw, ~23 deg/sec
//!   * Arrow keys nudge yaw / pitch
//!   * Space toggles auto-rotate
//!   * Esc quits
//!
//! Two render pipelines share one render pass.  No bloom yet; that's
//! a follow-up that adds an offscreen HDR colour target + a
//! downsample / blur / composite chain.

use std::sync::Arc;
use std::time::Instant;

use gos_protocol::VectorAddress;
use k_scene::{
    sphere_index_bytes_for, sphere_index_count, sphere_vertex_bytes_for,
    tube_index_bytes_for, tube_index_count, tube_vertex_bytes_for, write_sphere,
    write_tube, BYTES_PER_PBR_VERTEX,
};
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

const NODE_COUNT: usize = 48;
const WINDOW_W: u32 = 1280;
const WINDOW_H: u32 = 800;

const SPHERE_STACKS: usize = 8;
const SPHERE_SLICES: usize = 12;
const SPHERE_RADIUS: f32 = 0.07;

const TUBE_SIDES: usize = 6;
const TUBE_RADIUS: f32 = 0.012;

/// PBR-ish shader: Blinn-Phong specular + Lambert diffuse + Fresnel rim.
/// Vertex stage transforms world position by the push-constant
/// view-proj matrix; fragment stage runs the lighting model in world
/// space.  Background pipeline ships a separate full-screen
/// triangle that paints a starfield + nebula gradient at z = 1.0.
const SHADER_WGSL: &str = r#"
struct Push {
    view_proj:       mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    eye:             vec4<f32>,    // .xyz = camera position
    time:            vec4<f32>,    // .x = seconds since boot
};
var<push_constant> pc: Push;

// ── Scene pipeline (spheres + cables) ─────────────────────────────

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal:   vec3<f32>,
    @location(2) color:    vec3<f32>,
};
struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0)       world_position: vec3<f32>,
    @location(1)       world_normal:   vec3<f32>,
    @location(2)       color:          vec3<f32>,
};

@vertex
fn vs_scene(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_position = pc.view_proj * vec4<f32>(in.position, 1.0);
    out.world_position = in.position;
    out.world_normal = in.normal;
    out.color = in.color;
    return out;
}

// Shadow pass: same geometry as the scene, projected by the light
// view-proj matrix.  Depth-only; no fragment stage.
@vertex
fn vs_shadow(in: VsIn) -> @builtin(position) vec4<f32> {
    return pc.light_view_proj * vec4<f32>(in.position, 1.0);
}

// Shadow sampling — bound on scene pipeline only.  `shadow_tex` is a
// depth texture (Depth32Float); `shadow_samp` is a comparison
// sampler that returns 0.0/1.0 for "in shadow / lit" per tap.
@group(0) @binding(0) var shadow_tex: texture_depth_2d;
@group(0) @binding(1) var shadow_samp: sampler_comparison;

fn sample_shadow_pcf(world_pos: vec3<f32>, n_dot_l: f32) -> f32 {
    // Project world position into the light's clip space.
    let proj = pc.light_view_proj * vec4<f32>(world_pos, 1.0);
    let ndc = proj.xyz / proj.w;
    // Outside the shadow map frustum → fully lit.
    if (any(abs(ndc.xy) > vec2<f32>(1.0)) || ndc.z < 0.0 || ndc.z > 1.0) {
        return 1.0;
    }
    // NDC (-1..1) → UV (0..1) with V flipped (wgpu image-space Y is down).
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 1.0 - (ndc.y * 0.5 + 0.5));
    // Slope-scaled bias: surfaces nearly edge-on to the light need more
    // bias to avoid surface acne.
    let bias = max(0.0008 * (1.0 - n_dot_l), 0.0002);
    let depth_ref = ndc.z - bias;
    // 3×3 PCF — soft shadow edges.  Texel step computed from texture
    // size so sampling is resolution-independent.
    let dims = vec2<f32>(textureDimensions(shadow_tex, 0));
    let step = vec2<f32>(1.0 / dims.x, 1.0 / dims.y);
    var acc = 0.0;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let offset = vec2<f32>(f32(dx) * step.x, f32(dy) * step.y);
            acc = acc + textureSampleCompareLevel(
                shadow_tex, shadow_samp, uv + offset, depth_ref,
            );
        }
    }
    return acc / 9.0;
}

// ── Cook-Torrance / GGX BRDF helpers ──────────────────────────────
//
// Standard real-time microfacet pipeline:
//   * D = GGX (Trowbridge-Reitz) normal distribution
//   * G = Smith pair of Schlick-GGX geometry terms
//   * F = Schlick fresnel
//
// Material model: per-fragment `metallic` + `roughness` in [0,1].
// F0 (base reflectance at normal incidence) is 4% for dielectrics,
// tinted by albedo for metals (the standard physically-based
// approximation).

const PI: f32 = 3.14159265358979;

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a  = roughness * roughness;
    let a2 = a * a;
    let n_dot_h = max(dot(n, h), 0.0);
    let n_dot_h2 = n_dot_h * n_dot_h;
    let denom_part = (n_dot_h2 * (a2 - 1.0) + 1.0);
    return a2 / (PI * denom_part * denom_part);
}

fn geometry_schlick_ggx(n_dot_x: f32, roughness: f32) -> f32 {
    // Direct-lighting form of k.  IBL uses a different k; that
    // branch lives in the IBL slice.
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_x / (n_dot_x * (1.0 - k) + k);
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(n, v), 0.0);
    let n_dot_l = max(dot(n, l), 0.0);
    return geometry_schlick_ggx(n_dot_v, roughness)
         * geometry_schlick_ggx(n_dot_l, roughness);
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0)
              * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// Roughness-aware fresnel for IBL ambient — Sébastien Lagarde 2014.
// Pulls F toward F0 as roughness rises, matching the way diffuse
// environment energy bleeds into the specular term on rough surfaces.
fn fresnel_schlick_roughness(cos_theta: f32, f0: vec3<f32>, roughness: f32) -> vec3<f32> {
    let r = max(vec3<f32>(1.0 - roughness), f0);
    return f0 + (r - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

@fragment
fn fs_scene(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let v = normalize(pc.eye.xyz - in.world_position);
    let l = normalize(vec3<f32>(0.55, 0.72, -0.42));
    let h = normalize(l + v);

    // Material — Gen-1 single-material setup.  Roughness picked to
    // give a tight-but-not-mirror highlight (brushed metal); metallic
    // 0.85 means most of the reflection comes from the albedo-tinted
    // F0 rather than a white dielectric.  Per-instance differentiation
    // (spheres vs. cables, plus eventual user-driven palettes) lands
    // when we add a vertex attribute for it.
    let roughness = 0.38;
    let metallic = 0.85;
    let albedo = in.color;

    // F0: 4% baseline for dielectric reflectance, lerped to albedo
    // for metals.
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);

    let n_dot_l = max(dot(n, l), 0.0);
    let n_dot_v = max(dot(n, v), 0.0);
    let h_dot_v = max(dot(h, v), 0.0);

    let d_term = distribution_ggx(n, h, roughness);
    let g_term = geometry_smith(n, v, l, roughness);
    let f_term = fresnel_schlick(h_dot_v, f0);

    let specular_num = d_term * g_term * f_term;
    let specular_den = 4.0 * n_dot_v * n_dot_l + 0.0001;
    let specular = specular_num / specular_den;

    // Energy conservation: diffuse fraction = (1 - fresnel) * (1 - metallic).
    let k_s = f_term;
    let k_d = (vec3<f32>(1.0) - k_s) * (1.0 - metallic);

    // Key-light radiance (slightly warm, well above LDR so bloom has
    // something to feed on).  Direct contribution gated by the
    // shadow-map sample at this fragment.
    let radiance = vec3<f32>(1.0, 0.95, 0.88) * 4.0;
    let shadow_factor = sample_shadow_pcf(in.world_position, n_dot_l);
    let lo = (k_d * albedo / PI + specular) * radiance * n_dot_l * shadow_factor;

    // ── IBL ambient (procedural environment) ─────────────────────
    //
    // Sample the same nebula function fs_bg renders, parameterised
    // by surface normal (diffuse irradiance) and reflection vector
    // (specular env).  This is a procedural stand-in for a real
    // prefiltered cubemap: no convolution, no BRDF LUT, but the
    // ambient term now varies with where on the sky each surface
    // is "looking", which is the visual point of IBL.
    let r_vec = reflect(-v, n);
    let env_diffuse_color  = nebula_color(dir_to_uv(n), 0.0);
    // Roughness drives a low-pass on the reflection sample by
    // thinning the high-frequency octave (passes 0..1 to
    // `nebula_color`; 1 = blurred mirror, 0 = sharp environment).
    let env_specular_color = nebula_color(dir_to_uv(r_vec), roughness);

    // Roughness-aware fresnel for the IBL specular term — Lagarde 2014.
    let f_ibl = fresnel_schlick_roughness(n_dot_v, f0, roughness);
    let k_d_ibl = (vec3<f32>(1.0) - f_ibl) * (1.0 - metallic);

    // Boost factor pushes env-sampled tones into a comfortable
    // perceptual brightness; nebula values are in [0, ~0.4] which
    // would be too dim as-is.
    let env_boost = 4.0;
    let ambient_diffuse  = env_diffuse_color  * env_boost * albedo * k_d_ibl;
    let ambient_specular = env_specular_color * env_boost * f_ibl;
    let ambient = ambient_diffuse + ambient_specular;

    // Fresnel rim retained — physically already inside `specular` /
    // `ambient_specular`, but the explicit boost preserves the
    // sci-fi silhouette glow tuned in the previous slice.
    let rim = albedo * pow(1.0 - n_dot_v, 4.0) * 0.5;

    return vec4<f32>(ambient + lo + rim, 1.0);
}

// ── Background pipeline (starfield + nebula) ──────────────────────

struct BgVsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0)       ndc: vec2<f32>,
};

@vertex
fn vs_bg(@builtin(vertex_index) i: u32) -> BgVsOut {
    // Full-screen triangle trick: three vertices at (-1,-1), (3,-1),
    // (-1,3) cover the whole NDC square with no overdraw.
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    var out: BgVsOut;
    let p = pos[i];
    // z = 1.0 puts us on the far plane; any scene fragment with z < 1
    // overdraws the starfield naturally via depth test.
    out.clip_position = vec4<f32>(p, 1.0, 1.0);
    out.ndc = p;
    return out;
}

// Cheap hash for star placement.  Returns value in [0, 1].
fn hash21(p: vec2<f32>) -> f32 {
    let q = vec2<f32>(127.1, 311.7);
    let h = dot(p, q);
    return fract(sin(h) * 43758.5453);
}

// 2D value noise: bilerp between four corner hashes with a smooth
// (cubic Hermite) interpolant.
fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// 4-octave fractional Brownian motion.  Each octave doubles the
// frequency and halves the amplitude.  Total ~12 noise samples per
// pixel — affordable at full-res background.
fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var amp = 0.5;
    var freq = 1.0;
    for (var i = 0; i < 4; i = i + 1) {
        v = v + amp * value_noise(p * freq);
        freq = freq * 2.0;
        amp = amp * 0.5;
    }
    return v;
}

// Shared nebula evaluator — used by `fs_bg` for the background and
// by `fs_scene`'s IBL ambient term so the environment lighting on
// metallic surfaces actually reflects the sky we're rendering
// behind them.  `flow_uv` is a 2D coordinate in nebula-space (NDC
// for fs_bg, equirectangular projection of a world direction for
// IBL); `freq_lod` thins the high-frequency octave for rough-surface
// blur approximation.
fn nebula_color(flow_uv: vec2<f32>, freq_lod: f32) -> vec3<f32> {
    let drift0 = pc.time.x * 0.018;
    let drift1 = pc.time.x * -0.012;
    let f_lo = mix(1.4, 0.7, freq_lod);
    let f_hi = mix(2.7, 1.0, freq_lod);
    let layer0 = fbm(flow_uv * f_lo + vec2<f32>(drift0, drift0 * 0.6));
    let layer1 = fbm(flow_uv * f_hi + vec2<f32>(drift1, drift1 * -0.8));
    let cloud = pow(layer0 * 0.65 + layer1 * 0.35, 1.6);
    let v_axis = flow_uv.y * 0.5 + 0.5;
    let base = mix(
        vec3<f32>(0.015, 0.020, 0.050),
        vec3<f32>(0.030, 0.050, 0.110),
        clamp(v_axis, 0.0, 1.0),
    );
    let peak_a = vec3<f32>(0.20, 0.10, 0.35);
    let peak_b = vec3<f32>(0.05, 0.25, 0.35);
    let peak_mix = mix(peak_a, peak_b, layer1);
    return base + peak_mix * cloud * 0.55;
}

// Equirectangular projection: world-space direction → 2D coords for
// `nebula_color`.  Maps `dir = (0, 1, 0)` (zenith) to `(_, 1)` so the
// nebula's vertical hue axis aligns with world up.
fn dir_to_uv(dir: vec3<f32>) -> vec2<f32> {
    let phi = atan2(dir.z, dir.x);
    let theta_y = clamp(dir.y, -1.0, 1.0);
    return vec2<f32>(phi / (2.0 * PI), theta_y);
}

@fragment
fn fs_bg(in: BgVsOut) -> @location(0) vec4<f32> {
    let nebula = nebula_color(in.ndc, 0.0);

    // Star layer: each pixel is at most one star.  Quantise NDC into
    // a 480×300 grid; for each cell, hash the cell index to a star
    // intensity threshold.  Cells below threshold get a star whose
    // brightness fades with sub-cell distance from centre, plus a
    // slow twinkle modulated by time.
    let grid = vec2<f32>(480.0, 300.0);
    let cell = floor(in.ndc * grid * 0.5);
    let inside = fract(in.ndc * grid * 0.5);
    let star_seed = hash21(cell);
    var star_intensity = 0.0;
    if (star_seed > 0.985) {
        let centred = inside - vec2<f32>(0.5, 0.5);
        let d = length(centred);
        let twinkle = 0.6 + 0.4 * sin(pc.time.x * 1.5 + star_seed * 47.0);
        star_intensity = max(0.0, 1.0 - d * 6.0) * twinkle;
        // Brighter, larger stars when seed is much higher.
        if (star_seed > 0.997) {
            star_intensity = star_intensity * 1.8;
        }
    }
    let star_color = vec3<f32>(0.9, 0.95, 1.0) * star_intensity;

    return vec4<f32>(nebula + star_color, 1.0);
}

// ── Post-process pipelines (bloom + tonemap) ──────────────────────
//
// Both pipelines run a full-screen triangle (no vertex buffer; the
// shader generates positions from the vertex index).  Bind group 0
// supplies a linear sampler + one or two HDR source textures.

struct PostVsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0)       uv: vec2<f32>,
};

@vertex
fn vs_post(@builtin(vertex_index) i: u32) -> PostVsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    var out: PostVsOut;
    let p = pos[i];
    out.clip_position = vec4<f32>(p, 0.0, 1.0);
    // UV = (p + 1) / 2, with V flipped so (0,0) is the top-left
    // texel (matches wgpu's image-space convention).
    out.uv = vec2<f32>((p.x + 1.0) * 0.5, 1.0 - (p.y + 1.0) * 0.5);
    return out;
}

// Bloom extract + blur: sample the HDR target, threshold to keep
// only "above-LDR" energy (luma > 1.0), then 5×5 box-blur the
// surviving pixels.  Half-res target keeps cost cheap; the bilinear
// sampler on the composite pass smooths the upsample.

@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_samp: sampler;

fn relative_luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// ── 3-level bloom mip chain ────────────────────────────────────────
//
// Pipeline (5 render passes):
//   1. fs_bloom_extract: HDR → bloom_h2.  Threshold + 5×5 blur in
//      one pass; pixels under the threshold drop out completely.
//   2. fs_bloom_downsample: bloom_h2 → bloom_h4 (4-sample box).
//   3. fs_bloom_downsample: bloom_h4 → bloom_h8 (same shader,
//      different bind group).
//   4. fs_bloom_upsample: bloom_h8 → bloom_h4 (additive blend
//      via pipeline blend state — LoadOp::Load preserves the
//      existing h4 content from pass 2).
//   5. fs_bloom_upsample: bloom_h4 → bloom_h2 (likewise).
//
// Composite then samples just bloom_h2, which now contains the
// fully accumulated multi-octave glow.

@fragment
fn fs_bloom_extract(in: PostVsOut) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(hdr_tex, 0));
    let step = vec2<f32>(1.0 / dims.x, 1.0 / dims.y);
    var acc = vec3<f32>(0.0);
    let threshold = 0.85;
    let kernel_radius = 2;
    var weight_sum = 0.0;
    for (var dy = -kernel_radius; dy <= kernel_radius; dy = dy + 1) {
        for (var dx = -kernel_radius; dx <= kernel_radius; dx = dx + 1) {
            let uv = in.uv + vec2<f32>(f32(dx) * step.x * 2.0, f32(dy) * step.y * 2.0);
            let c = textureSample(hdr_tex, hdr_samp, uv).rgb;
            let luma = relative_luminance(c);
            let factor = max(luma - threshold, 0.0) / max(luma, 0.0001);
            acc = acc + c * factor;
            weight_sum = weight_sum + 1.0;
        }
    }
    return vec4<f32>(acc / weight_sum, 1.0);
}

// Downsample: 4-tap box average from the higher-res source.  Used
// twice (h2 → h4 and h4 → h8) with different bind groups; same
// pipeline.
@fragment
fn fs_bloom_downsample(in: PostVsOut) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(hdr_tex, 0));
    let step = vec2<f32>(1.0 / dims.x, 1.0 / dims.y);
    let off = vec2<f32>(step.x * 0.5, step.y * 0.5);
    let a = textureSample(hdr_tex, hdr_samp, in.uv + vec2<f32>(-off.x, -off.y)).rgb;
    let b = textureSample(hdr_tex, hdr_samp, in.uv + vec2<f32>( off.x, -off.y)).rgb;
    let c = textureSample(hdr_tex, hdr_samp, in.uv + vec2<f32>(-off.x,  off.y)).rgb;
    let d = textureSample(hdr_tex, hdr_samp, in.uv + vec2<f32>( off.x,  off.y)).rgb;
    return vec4<f32>((a + b + c + d) * 0.25, 1.0);
}

// Upsample: 9-sample tent filter from the lower-res source, written
// with **additive blend** (set on the pipeline) so each call
// accumulates onto whatever lower-frequency content the target
// already holds.  Used twice (h8 → h4 and h4 → h2).
@fragment
fn fs_bloom_upsample(in: PostVsOut) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(hdr_tex, 0));
    let step = vec2<f32>(1.0 / dims.x, 1.0 / dims.y);
    // 3×3 tent kernel; centre weight 4, edge 2, corner 1.  Sum 16.
    let centre = textureSample(hdr_tex, hdr_samp, in.uv).rgb * 4.0;
    let n  = textureSample(hdr_tex, hdr_samp, in.uv + vec2<f32>(0.0, -step.y)).rgb * 2.0;
    let s  = textureSample(hdr_tex, hdr_samp, in.uv + vec2<f32>(0.0,  step.y)).rgb * 2.0;
    let e  = textureSample(hdr_tex, hdr_samp, in.uv + vec2<f32>( step.x, 0.0)).rgb * 2.0;
    let w  = textureSample(hdr_tex, hdr_samp, in.uv + vec2<f32>(-step.x, 0.0)).rgb * 2.0;
    let ne = textureSample(hdr_tex, hdr_samp, in.uv + vec2<f32>( step.x, -step.y)).rgb;
    let nw = textureSample(hdr_tex, hdr_samp, in.uv + vec2<f32>(-step.x, -step.y)).rgb;
    let se = textureSample(hdr_tex, hdr_samp, in.uv + vec2<f32>( step.x,  step.y)).rgb;
    let sw = textureSample(hdr_tex, hdr_samp, in.uv + vec2<f32>(-step.x,  step.y)).rgb;
    let total = (centre + n + s + e + w + ne + nw + se + sw) / 16.0;
    return vec4<f32>(total, 1.0);
}

// Composite: HDR + bloom -> ACES tonemap -> swapchain (sRGB).

@group(0) @binding(2) var bloom_tex: texture_2d<f32>;

fn aces_film(x: vec3<f32>) -> vec3<f32> {
    // ACES approximation (Krzysztof Narkowicz 2015).  Maps HDR to
    // LDR with a filmic toe + shoulder; widely used as a "good
    // enough" tonemap for realtime work.
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e),
                 vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_composite(in: PostVsOut) -> @location(0) vec4<f32> {
    let hdr = textureSample(hdr_tex, hdr_samp, in.uv).rgb;
    // Bloom is on a half-res target; the bilinear sampler upsamples
    // smoothly.  Additive blend; intensity tuned to look strong
    // without nuking the underlying scene.
    let bloom = textureSample(bloom_tex, hdr_samp, in.uv).rgb;
    let combined = hdr + bloom * 1.6;
    let mapped = aces_film(combined);
    // Output is linear into a Rgba8Unorm LDR target consumed by the
    // FXAA pass.  Final sRGB conversion happens at the swapchain
    // write in fs_fxaa (which targets the sRGB swapchain).
    return vec4<f32>(mapped, 1.0);
}

// ── FXAA (Fast Approximate Anti-Aliasing) ──────────────────────────
//
// Simplified FXAA: detect edges by luma min/max contrast in a 5-tap
// cross, classify edge direction by central second-derivative sign,
// blend 1.5 texels perpendicular.  Less than 30 ops per pixel; trades
// some quality vs. full FXAA 3.11 for code size.

@group(0) @binding(3) var ldr_tex: texture_2d<f32>;

fn luma_of(c: vec3<f32>) -> f32 {
    // BT.601 weights — what FXAA was designed around.
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

@fragment
fn fs_fxaa(in: PostVsOut) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(ldr_tex, 0));
    let step = vec2<f32>(1.0 / dims.x, 1.0 / dims.y);
    let c  = textureSample(ldr_tex, hdr_samp, in.uv).rgb;
    let n  = textureSample(ldr_tex, hdr_samp, in.uv + vec2<f32>(0.0, -step.y)).rgb;
    let s  = textureSample(ldr_tex, hdr_samp, in.uv + vec2<f32>(0.0,  step.y)).rgb;
    let e  = textureSample(ldr_tex, hdr_samp, in.uv + vec2<f32>( step.x, 0.0)).rgb;
    let w  = textureSample(ldr_tex, hdr_samp, in.uv + vec2<f32>(-step.x, 0.0)).rgb;
    let lc = luma_of(c);
    let ln = luma_of(n);
    let ls = luma_of(s);
    let le = luma_of(e);
    let lw = luma_of(w);
    let lmin = min(min(min(min(lc, ln), ls), le), lw);
    let lmax = max(max(max(max(lc, ln), ls), le), lw);
    let contrast = lmax - lmin;
    // Threshold below which the pixel is "flat" and gets no AA.
    // 0.06 is a conservative default; tighter values catch more
    // edges but soften legitimate texture detail.
    if (contrast < 0.06) {
        return vec4<f32>(c, 1.0);
    }
    // Edge orientation: second-derivative magnitude along each axis.
    let horiz = abs(le + lw - 2.0 * lc);
    let vert  = abs(ln + ls - 2.0 * lc);
    let is_horizontal = horiz >= vert;
    var dir = vec2<f32>(0.0, 0.0);
    if (is_horizontal) {
        let grad = ln - ls;
        dir = vec2<f32>(0.0, sign(grad) * step.y);
    } else {
        let grad = le - lw;
        dir = vec2<f32>(sign(grad) * step.x, 0.0);
    }
    let blur1 = textureSample(ldr_tex, hdr_samp, in.uv + dir * 0.5).rgb;
    let blur2 = textureSample(ldr_tex, hdr_samp, in.uv + dir * 1.5).rgb;
    // Mix center with a perpendicular blur — 0.7 blend reads as
    // "softened edge" without losing the underlying colour.
    let aa = mix(c, mix(blur1, blur2, 0.5), 0.7);
    return vec4<f32>(aa, 1.0);
}
"#;

#[derive(Debug, Clone, Copy)]
struct Camera {
    radius: f32,
    yaw: f32,
    pitch: f32,
    auto_yaw_per_sec: f32,
    fov_y: f32,
    aspect: f32,
    near: f32,
    far: f32,
}

impl Camera {
    fn new(aspect: f32) -> Self {
        Self {
            radius: 4.5,
            yaw: 0.6,
            pitch: 0.35,
            auto_yaw_per_sec: 0.4,
            fov_y: 50.0_f32.to_radians(),
            aspect,
            near: 0.1,
            far: 100.0,
        }
    }

    fn eye(&self) -> [f32; 3] {
        [
            self.radius * self.pitch.cos() * self.yaw.sin(),
            self.radius * self.pitch.sin(),
            self.radius * self.pitch.cos() * self.yaw.cos(),
        ]
    }

    fn view_proj(&self) -> [[f32; 4]; 4] {
        let eye = self.eye();
        let view = look_at(eye, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let proj = perspective(self.fov_y, self.aspect, self.near, self.far);
        mat4_mul(proj, view)
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("create event loop");
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("gos-visualize · cosmic graph")
            .with_inner_size(winit::dpi::PhysicalSize::new(WINDOW_W, WINDOW_H))
            .build(&event_loop)
            .expect("create window"),
    );

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    let surface = instance.create_surface(window.clone()).expect("create surface");
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .expect("request adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("gos-visualize"),
            required_features: wgpu::Features::PUSH_CONSTANTS,
            // Push constant size: 64 (view_proj) + 64 (light_view_proj)
            // + 16 (eye) + 16 (time) = 160 B.  Round up to 192 — well
            // under the 256-byte ceiling most desktop drivers expose.
            required_limits: wgpu::Limits {
                max_push_constant_size: 192,
                ..wgpu::Limits::downlevel_defaults()
            },
        },
        None,
    ))
    .expect("request device");

    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(surface_caps.formats[0]);
    let mut surface_cfg = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: WINDOW_W,
        height: WINDOW_H,
        present_mode: surface_caps
            .present_modes
            .iter()
            .copied()
            .find(|m| *m == wgpu::PresentMode::Mailbox)
            .unwrap_or(wgpu::PresentMode::Fifo),
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &surface_cfg);

    let mut depth_view = create_depth(&device, surface_cfg.width, surface_cfg.height);
    // HDR pipeline: scene + background render into a Rgba16Float
    // offscreen target so post-process (bloom + tonemap) sees real
    // > 1.0 highlights.  Bloom texture is half-res for cheaper blur
    // sampling; the bilinear sampler smooths it back to full size in
    // the composite pass.
    let mut offscreen = make_offscreen(&device, surface_cfg.width, surface_cfg.height);
    let post_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("post-sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    // ── Geometry ─────────────────────────────────────────────────
    let nodes = synth_nodes(NODE_COUNT);
    let edges = synth_edges(NODE_COUNT);

    let sphere_v_bytes = sphere_vertex_bytes_for(SPHERE_STACKS, SPHERE_SLICES);
    let sphere_i_bytes = sphere_index_bytes_for(SPHERE_STACKS, SPHERE_SLICES);
    let tube_v_bytes = tube_vertex_bytes_for(TUBE_SIDES);
    let tube_i_bytes = tube_index_bytes_for(TUBE_SIDES);

    let total_v_bytes = sphere_v_bytes * nodes.len() + tube_v_bytes * edges.len();
    let total_i_bytes = sphere_i_bytes * nodes.len() + tube_i_bytes * edges.len();

    let mut vbuf = vec![0u8; total_v_bytes];
    let mut ibuf = vec![0u8; total_i_bytes];

    let mut v_offset = 0usize;
    let mut i_offset = 0usize;
    let mut base_vertex: u16 = 0;

    // Spheres first.
    for n in &nodes {
        let (vc, ic) = write_sphere(
            n.world,
            SPHERE_RADIUS,
            n.color,
            SPHERE_STACKS,
            SPHERE_SLICES,
            base_vertex,
            &mut vbuf[v_offset..],
            &mut ibuf[i_offset..],
        );
        v_offset += vc * BYTES_PER_PBR_VERTEX;
        i_offset += ic * 2;
        base_vertex = base_vertex
            .checked_add(vc as u16)
            .expect("vertex count fits u16");
    }
    // Cables (cooler hue so they read as "wiring" not "another node").
    let cable_color = [0.45, 0.55, 0.8];
    for e in &edges {
        let (vc, ic) = write_tube(
            nodes[e.from].world,
            nodes[e.to].world,
            TUBE_RADIUS,
            cable_color,
            TUBE_SIDES,
            base_vertex,
            &mut vbuf[v_offset..],
            &mut ibuf[i_offset..],
        );
        v_offset += vc * BYTES_PER_PBR_VERTEX;
        i_offset += ic * 2;
        base_vertex = base_vertex
            .checked_add(vc as u16)
            .expect("vertex count fits u16");
    }

    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("scene-vertices"),
        size: vbuf.len() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&vertex_buffer, 0, &vbuf);
    let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("scene-indices"),
        size: ibuf.len() as u64,
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&index_buffer, 0, &ibuf);

    let total_index_count =
        (sphere_index_count(SPHERE_STACKS, SPHERE_SLICES) * nodes.len()
            + tube_index_count(TUBE_SIDES) * edges.len()) as u32;

    // ── Pipelines ─────────────────────────────────────────────────
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("cosmic-shader"),
        source: wgpu::ShaderSource::Wgsl(SHADER_WGSL.into()),
    });
    // Shadow map: fixed 1024×1024 Depth32Float.  Bound to scene
    // pipeline via `shadow_bgl`; written by shadow pipeline.
    const SHADOW_RES: u32 = 1024;
    let shadow_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shadow-map"),
        size: wgpu::Extent3d {
            width: SHADOW_RES,
            height: SHADOW_RES,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let shadow_view = shadow_tex.create_view(&wgpu::TextureViewDescriptor::default());
    let shadow_comparison_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("shadow-comparison-sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        compare: Some(wgpu::CompareFunction::LessEqual),
        ..Default::default()
    });
    let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("shadow-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
        ],
    });
    let shadow_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shadow-bg"),
        layout: &shadow_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&shadow_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&shadow_comparison_sampler),
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("cosmic-pipeline-layout"),
        bind_group_layouts: &[&shadow_bgl],
        push_constant_ranges: &[wgpu::PushConstantRange {
            stages: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            range: 0..160,
        }],
    });
    // Shadow pass uses the same vertex layout but only the
    // `light_view_proj` push constant.  No fragment stage (depth-only).
    let shadow_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("shadow-pipeline-layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[wgpu::PushConstantRange {
            stages: wgpu::ShaderStages::VERTEX,
            range: 0..160,
        }],
    });
    // `VertexBufferLayout` borrows `attributes`, so two consumers
    // (scene + shadow pipelines) each get an independently
    // constructed value.  The attributes slice is `'static` from a
    // const expression and shared.
    const VERTEX_ATTRS: [wgpu::VertexAttribute; 3] = [
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: 0,
            shader_location: 0,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: 12,
            shader_location: 1,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: 24,
            shader_location: 2,
        },
    ];
    let make_vertex_layout = || wgpu::VertexBufferLayout {
        array_stride: BYTES_PER_PBR_VERTEX as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &VERTEX_ATTRS,
    };
    let vertex_layout = make_vertex_layout();
    let scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("scene-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_scene",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[vertex_layout],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_scene",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: HDR_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("shadow-pipeline"),
        layout: Some(&shadow_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_shadow",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[make_vertex_layout()],
        },
        // Depth-only — no fragment stage, no colour target.
        fragment: None,
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            // Cull FRONT for the shadow pass — common trick to bias
            // the depth bias problem to the back faces instead of
            // the lit front faces (peter-panning over surface acne).
            cull_mode: Some(wgpu::Face::Front),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    let bg_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("background-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_bg",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_bg",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: HDR_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        // Depth = 1.0 in vs_bg; pass when LessEqual so the
        // background actually writes pixels but loses to anything
        // closer.
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    // Post-process bind group layout — four entries shared by all
    // post-process pipelines (bloom, composite, fxaa).  Each
    // pipeline references a subset; unused slots get a harmless
    // dummy binding (the HDR view) to satisfy the layout.
    //
    //   @binding(0) hdr_tex     — composite reads
    //   @binding(1) sampler     — all pipelines read
    //   @binding(2) bloom_tex   — composite reads
    //   @binding(3) ldr_tex     — fxaa reads
    let texture_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    let post_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("post-bgl"),
        entries: &[
            texture_entry(0),
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            texture_entry(2),
            texture_entry(3),
        ],
    });
    let post_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("post-layout"),
        bind_group_layouts: &[&post_bgl],
        push_constant_ranges: &[],
    });
    let make_post_pipeline = |label: &'static str,
                              entry: &'static str,
                              format: wgpu::TextureFormat,
                              blend: Option<wgpu::BlendState>| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&post_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_post",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: entry,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        })
    };
    let bloom_extract_pipeline =
        make_post_pipeline("bloom-extract", "fs_bloom_extract", HDR_FORMAT, None);
    let bloom_down_pipeline =
        make_post_pipeline("bloom-downsample", "fs_bloom_downsample", HDR_FORMAT, None);
    // Upsample uses additive blend so each level accumulates onto the
    // existing higher-res content from the previous downsample chain.
    let additive_blend = wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent::REPLACE,
    };
    let bloom_up_pipeline = make_post_pipeline(
        "bloom-upsample",
        "fs_bloom_upsample",
        HDR_FORMAT,
        Some(additive_blend),
    );
    let composite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("composite-pipeline"),
        layout: Some(&post_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_post",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_composite",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: LDR_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });
    // FXAA: reads the LDR target, smooths luminance edges, writes
    // to the swapchain (sRGB conversion happens at write).
    let fxaa_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("fxaa-pipeline"),
        layout: Some(&post_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_post",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_fxaa",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    let mut camera = Camera::new(WINDOW_W as f32 / WINDOW_H as f32);
    let mut last_frame = Instant::now();
    let start_time = Instant::now();
    let mut manual_yaw: f32 = 0.0;
    let mut manual_pitch: f32 = 0.0;
    let mut auto_rotate_on = true;
    println!(
        "gos-visualize: cosmic graph up — {} nodes, {} cables; \
         arrows orbit, Space pauses, Esc quits",
        nodes.len(),
        edges.len()
    );

    event_loop
        .run(move |event, target| {
            target.set_control_flow(ControlFlow::Poll);
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => target.exit(),
                    WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                        surface_cfg.width = size.width;
                        surface_cfg.height = size.height;
                        surface.configure(&device, &surface_cfg);
                        depth_view = create_depth(&device, size.width, size.height);
                        offscreen = make_offscreen(&device, size.width, size.height);
                        camera.aspect = size.width as f32 / size.height as f32;
                    }
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                physical_key: PhysicalKey::Code(code),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    } => match code {
                        KeyCode::Escape => target.exit(),
                        KeyCode::ArrowLeft => manual_yaw -= 0.1,
                        KeyCode::ArrowRight => manual_yaw += 0.1,
                        KeyCode::ArrowUp => manual_pitch += 0.1,
                        KeyCode::ArrowDown => manual_pitch -= 0.1,
                        KeyCode::Space => auto_rotate_on = !auto_rotate_on,
                        _ => {}
                    },
                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt = (now - last_frame).as_secs_f32();
                        last_frame = now;
                        if auto_rotate_on {
                            camera.yaw += camera.auto_yaw_per_sec * dt;
                        }
                        camera.yaw += manual_yaw;
                        camera.pitch = (camera.pitch + manual_pitch).clamp(-1.4, 1.4);
                        manual_yaw = 0.0;
                        manual_pitch = 0.0;

                        let view_proj = camera.view_proj();
                        let eye = camera.eye();
                        let elapsed = (now - start_time).as_secs_f32();
                        // Light view-proj is static — could be hoisted
                        // out of the loop if the light ever stops
                        // animating.  Cheap to rebuild for now.
                        let lvp = light_view_proj();
                        let push_bytes = build_push_constants(view_proj, lvp, eye, elapsed);

                        let frame = match surface.get_current_texture() {
                            Ok(f) => f,
                            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                surface.configure(&device, &surface_cfg);
                                return;
                            }
                            Err(e) => {
                                eprintln!("surface error: {:?}", e);
                                return;
                            }
                        };
                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                        let mut encoder = device
                            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("frame"),
                            });

                        // Bind groups rebuilt each frame so a resize
                        // (which recreates the offscreen views)
                        // doesn't require manual invalidation.
                        // Helper: bind-group factory.  Every layout
                        // slot must be filled; pipelines that don't
                        // reference a slot just get the HDR view as
                        // a harmless dummy.
                        let make_bg = |label: &'static str,
                                       slot0: &wgpu::TextureView,
                                       slot2: &wgpu::TextureView,
                                       slot3: &wgpu::TextureView| {
                            device.create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some(label),
                                layout: &post_bgl,
                                entries: &[
                                    wgpu::BindGroupEntry {
                                        binding: 0,
                                        resource: wgpu::BindingResource::TextureView(slot0),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 1,
                                        resource: wgpu::BindingResource::Sampler(
                                            &post_sampler,
                                        ),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 2,
                                        resource: wgpu::BindingResource::TextureView(slot2),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 3,
                                        resource: wgpu::BindingResource::TextureView(slot3),
                                    },
                                ],
                            })
                        };
                        // Bind groups: only slot 0 (hdr_tex) varies
                        // across the bloom passes; the others get
                        // harmless dummies.
                        let extract_bg = make_bg(
                            "bloom-extract-bg",
                            &offscreen.hdr_view,
                            &offscreen.hdr_view,
                            &offscreen.hdr_view,
                        );
                        let down_h4_bg = make_bg(
                            "bloom-down-h4-bg",
                            &offscreen.bloom_h2_view,
                            &offscreen.hdr_view,
                            &offscreen.hdr_view,
                        );
                        let down_h8_bg = make_bg(
                            "bloom-down-h8-bg",
                            &offscreen.bloom_h4_view,
                            &offscreen.hdr_view,
                            &offscreen.hdr_view,
                        );
                        let up_h4_bg = make_bg(
                            "bloom-up-h4-bg",
                            &offscreen.bloom_h8_view,
                            &offscreen.hdr_view,
                            &offscreen.hdr_view,
                        );
                        let up_h2_bg = make_bg(
                            "bloom-up-h2-bg",
                            &offscreen.bloom_h4_view,
                            &offscreen.hdr_view,
                            &offscreen.hdr_view,
                        );
                        let composite_bg = make_bg(
                            "composite-bg",
                            &offscreen.hdr_view,
                            &offscreen.bloom_h2_view,
                            &offscreen.hdr_view,
                        );
                        let fxaa_bg = make_bg(
                            "fxaa-bg",
                            &offscreen.hdr_view,
                            &offscreen.hdr_view,
                            &offscreen.ldr_view,
                        );

                        // Pass 0 — shadow map.  Render scene geometry
                        // from the key light's POV, depth-only, into
                        // the shadow texture.  Background pipeline is
                        // skipped (a full-screen quad at the far
                        // plane never occludes anything anyway).
                        {
                            let mut pass = encoder.begin_render_pass(
                                &wgpu::RenderPassDescriptor {
                                    label: Some("shadow-pass"),
                                    color_attachments: &[],
                                    depth_stencil_attachment: Some(
                                        wgpu::RenderPassDepthStencilAttachment {
                                            view: &shadow_view,
                                            depth_ops: Some(wgpu::Operations {
                                                load: wgpu::LoadOp::Clear(1.0),
                                                store: wgpu::StoreOp::Store,
                                            }),
                                            stencil_ops: None,
                                        },
                                    ),
                                    timestamp_writes: None,
                                    occlusion_query_set: None,
                                },
                            );
                            pass.set_pipeline(&shadow_pipeline);
                            pass.set_push_constants(
                                wgpu::ShaderStages::VERTEX,
                                0,
                                &push_bytes,
                            );
                            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                            pass.set_index_buffer(
                                index_buffer.slice(..),
                                wgpu::IndexFormat::Uint16,
                            );
                            pass.draw_indexed(0..total_index_count, 0, 0..1);
                        }

                        // Pass 1 — scene + background into the HDR
                        // offscreen target.  This is where the real
                        // > 1.0 highlights are produced (specular
                        // peaks, fresnel rim, bright stars).
                        {
                            let mut pass = encoder.begin_render_pass(
                                &wgpu::RenderPassDescriptor {
                                    label: Some("scene-pass"),
                                    color_attachments: &[Some(
                                        wgpu::RenderPassColorAttachment {
                                            view: &offscreen.hdr_view,
                                            resolve_target: None,
                                            ops: wgpu::Operations {
                                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                                    r: 0.0,
                                                    g: 0.0,
                                                    b: 0.0,
                                                    a: 1.0,
                                                }),
                                                store: wgpu::StoreOp::Store,
                                            },
                                        },
                                    )],
                                    depth_stencil_attachment: Some(
                                        wgpu::RenderPassDepthStencilAttachment {
                                            view: &depth_view,
                                            depth_ops: Some(wgpu::Operations {
                                                load: wgpu::LoadOp::Clear(1.0),
                                                store: wgpu::StoreOp::Store,
                                            }),
                                            stencil_ops: None,
                                        },
                                    ),
                                    timestamp_writes: None,
                                    occlusion_query_set: None,
                                },
                            );
                            // Background + scene share the layout
                            // that now declares the shadow bind group
                            // at @group(0); both pipelines need it
                            // bound even though only `fs_scene`
                            // actually samples it.
                            pass.set_bind_group(0, &shadow_bg, &[]);
                            pass.set_pipeline(&bg_pipeline);
                            pass.set_push_constants(
                                wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                                0,
                                &push_bytes,
                            );
                            pass.draw(0..3, 0..1);
                            pass.set_pipeline(&scene_pipeline);
                            pass.set_push_constants(
                                wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                                0,
                                &push_bytes,
                            );
                            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                            pass.set_index_buffer(
                                index_buffer.slice(..),
                                wgpu::IndexFormat::Uint16,
                            );
                            pass.draw_indexed(0..total_index_count, 0, 0..1);
                        }

                        // Pass 2a — extract: HDR → bloom_h2 with
                        // luminance threshold.  Clear target first.
                        let bloom_pass = |encoder: &mut wgpu::CommandEncoder,
                                          label: &'static str,
                                          target: &wgpu::TextureView,
                                          pipeline: &wgpu::RenderPipeline,
                                          bind: &wgpu::BindGroup,
                                          load_clear: bool| {
                            let load = if load_clear {
                                wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 1.0,
                                })
                            } else {
                                wgpu::LoadOp::Load
                            };
                            let mut pass = encoder.begin_render_pass(
                                &wgpu::RenderPassDescriptor {
                                    label: Some(label),
                                    color_attachments: &[Some(
                                        wgpu::RenderPassColorAttachment {
                                            view: target,
                                            resolve_target: None,
                                            ops: wgpu::Operations {
                                                load,
                                                store: wgpu::StoreOp::Store,
                                            },
                                        },
                                    )],
                                    depth_stencil_attachment: None,
                                    timestamp_writes: None,
                                    occlusion_query_set: None,
                                },
                            );
                            pass.set_pipeline(pipeline);
                            pass.set_bind_group(0, bind, &[]);
                            pass.draw(0..3, 0..1);
                        };

                        bloom_pass(
                            &mut encoder,
                            "bloom-extract",
                            &offscreen.bloom_h2_view,
                            &bloom_extract_pipeline,
                            &extract_bg,
                            true,
                        );
                        // Pass 2b — downsample h2 → h4.
                        bloom_pass(
                            &mut encoder,
                            "bloom-down-h4",
                            &offscreen.bloom_h4_view,
                            &bloom_down_pipeline,
                            &down_h4_bg,
                            true,
                        );
                        // Pass 2c — downsample h4 → h8.
                        bloom_pass(
                            &mut encoder,
                            "bloom-down-h8",
                            &offscreen.bloom_h8_view,
                            &bloom_down_pipeline,
                            &down_h8_bg,
                            true,
                        );
                        // Pass 2d — upsample h8 → h4 (LoadOp::Load
                        // so the downsampled h4 content survives;
                        // pipeline has additive blend so this
                        // accumulates on top).
                        bloom_pass(
                            &mut encoder,
                            "bloom-up-h4",
                            &offscreen.bloom_h4_view,
                            &bloom_up_pipeline,
                            &up_h4_bg,
                            false,
                        );
                        // Pass 2e — upsample h4 → h2.
                        bloom_pass(
                            &mut encoder,
                            "bloom-up-h2",
                            &offscreen.bloom_h2_view,
                            &bloom_up_pipeline,
                            &up_h2_bg,
                            false,
                        );

                        // Pass 3 — composite HDR + bloom into the
                        // LDR intermediate, applying ACES tonemap.
                        {
                            let mut pass = encoder.begin_render_pass(
                                &wgpu::RenderPassDescriptor {
                                    label: Some("composite-pass"),
                                    color_attachments: &[Some(
                                        wgpu::RenderPassColorAttachment {
                                            view: &offscreen.ldr_view,
                                            resolve_target: None,
                                            ops: wgpu::Operations {
                                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                                    r: 0.0,
                                                    g: 0.0,
                                                    b: 0.0,
                                                    a: 1.0,
                                                }),
                                                store: wgpu::StoreOp::Store,
                                            },
                                        },
                                    )],
                                    depth_stencil_attachment: None,
                                    timestamp_writes: None,
                                    occlusion_query_set: None,
                                },
                            );
                            pass.set_pipeline(&composite_pipeline);
                            pass.set_bind_group(0, &composite_bg, &[]);
                            pass.draw(0..3, 0..1);
                        }

                        // Pass 4 — FXAA from LDR into the swapchain.
                        // sRGB conversion is handled by the surface
                        // format on write.
                        {
                            let mut pass = encoder.begin_render_pass(
                                &wgpu::RenderPassDescriptor {
                                    label: Some("fxaa-pass"),
                                    color_attachments: &[Some(
                                        wgpu::RenderPassColorAttachment {
                                            view: &view,
                                            resolve_target: None,
                                            ops: wgpu::Operations {
                                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                                    r: 0.0,
                                                    g: 0.0,
                                                    b: 0.0,
                                                    a: 1.0,
                                                }),
                                                store: wgpu::StoreOp::Store,
                                            },
                                        },
                                    )],
                                    depth_stencil_attachment: None,
                                    timestamp_writes: None,
                                    occlusion_query_set: None,
                                },
                            );
                            pass.set_pipeline(&fxaa_pipeline);
                            pass.set_bind_group(0, &fxaa_bg, &[]);
                            pass.draw(0..3, 0..1);
                        }

                        queue.submit(Some(encoder.finish()));
                        frame.present();

                        window.request_redraw();
                    }
                    _ => {}
                },
                Event::AboutToWait => {
                    window.request_redraw();
                }
                _ => {}
            }
        })
        .expect("event loop");
}

fn create_depth(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

/// HDR colour format for the offscreen target.  16-bit float per
/// channel lets specular highlights + fresnel rims exceed 1.0 so
/// the bloom extract has actual highlight energy to gather.
const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// LDR intermediate format consumed by FXAA.  `Rgba8Unorm` (linear
/// 8-bit) — the FXAA pass writes the gamma-corrected output to the
/// sRGB swapchain.  The 8-bit quantisation is fine since the
/// composite pass has already tonemapped HDR → [0,1].
const LDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

struct Offscreen {
    _hdr_tex: wgpu::Texture,
    hdr_view: wgpu::TextureView,
    // 3-level bloom pyramid.  h2 is the final result composite reads
    // from; h4 / h8 are intermediates accumulated into during the
    // upsample chain.
    _bloom_h2_tex: wgpu::Texture,
    bloom_h2_view: wgpu::TextureView,
    _bloom_h4_tex: wgpu::Texture,
    bloom_h4_view: wgpu::TextureView,
    _bloom_h8_tex: wgpu::Texture,
    bloom_h8_view: wgpu::TextureView,
    _ldr_tex: wgpu::Texture,
    ldr_view: wgpu::TextureView,
}

fn make_bloom_level(device: &wgpu::Device, label: &str, w: u32, h: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn make_offscreen(device: &wgpu::Device, w: u32, h: u32) -> Offscreen {
    let hdr_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hdr-color"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let hdr_view = hdr_tex.create_view(&wgpu::TextureViewDescriptor::default());
    let (bloom_h2_tex, bloom_h2_view) = make_bloom_level(device, "bloom-h2", w / 2, h / 2);
    let (bloom_h4_tex, bloom_h4_view) = make_bloom_level(device, "bloom-h4", w / 4, h / 4);
    let (bloom_h8_tex, bloom_h8_view) = make_bloom_level(device, "bloom-h8", w / 8, h / 8);
    let ldr_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ldr"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: LDR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let ldr_view = ldr_tex.create_view(&wgpu::TextureViewDescriptor::default());
    Offscreen {
        _hdr_tex: hdr_tex,
        hdr_view,
        _bloom_h2_tex: bloom_h2_tex,
        bloom_h2_view,
        _bloom_h4_tex: bloom_h4_tex,
        bloom_h4_view,
        _bloom_h8_tex: bloom_h8_tex,
        bloom_h8_view,
        _ldr_tex: ldr_tex,
        ldr_view,
    }
}

struct DemoNode {
    _vector: VectorAddress,
    world: [f32; 3],
    color: [f32; 3],
}

struct DemoEdge {
    from: usize,
    to: usize,
}

fn synth_nodes(n: usize) -> Vec<DemoNode> {
    // Lay nodes out on a slightly noisy sphere shell so the cables
    // form interesting 3D arcs instead of a flat grid.
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32;
        let phi = t * 2.39996323; // golden angle
        let y = 1.0 - (t / (n as f32 - 1.0).max(1.0)) * 2.0;
        let r = (1.0 - y * y).max(0.0).sqrt();
        let x = phi.cos() * r;
        let z = phi.sin() * r;
        // Mild radius variation per node (1.0..1.4) so the ball
        // cluster has visible depth.
        let layer_r = 1.0 + ((i % 5) as f32) * 0.08;
        let world = [x * layer_r, y * layer_r, z * layer_r];
        // Five-hue palette mirroring k-fb / kernel side.
        let hue = match i % 5 {
            0 => [0.20, 0.78, 1.0],  // cyan
            1 => [1.0, 0.30, 0.85],  // magenta
            2 => [1.0, 0.92, 0.30],  // yellow
            3 => [0.30, 0.95, 0.65], // mint
            _ => [1.0, 0.55, 0.70],  // rose
        };
        out.push(DemoNode {
            _vector: VectorAddress::new(
                ((i >> 24) & 0xFF) as u8,
                ((i >> 16) & 0xFFFF) as u16,
                ((i >> 8) & 0xFFFF) as u16,
                (i & 0xFFFF) as u16,
            ),
            world,
            color: hue,
        });
    }
    out
}

fn synth_edges(n: usize) -> Vec<DemoEdge> {
    // Each node gets two cables to its successors (mod n) — guarantees
    // a connected graph that reads as a "network" rather than just
    // disconnected spheres.
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        out.push(DemoEdge {
            from: i,
            to: (i + 1) % n,
        });
        if i % 3 == 0 {
            out.push(DemoEdge {
                from: i,
                to: (i + 5) % n,
            });
        }
    }
    out
}

// ── Push-constant packing ───────────────────────────────────────────

fn write_mat4_col_major(out: &mut [u8], m: [[f32; 4]; 4]) {
    let mut off = 0;
    for col in 0..4 {
        for row in 0..4 {
            out[off..off + 4].copy_from_slice(&m[row][col].to_le_bytes());
            off += 4;
        }
    }
}

fn build_push_constants(
    view_proj: [[f32; 4]; 4],
    light_view_proj: [[f32; 4]; 4],
    eye: [f32; 3],
    time: f32,
) -> [u8; 160] {
    let mut out = [0u8; 160];
    write_mat4_col_major(&mut out[0..64], view_proj);
    write_mat4_col_major(&mut out[64..128], light_view_proj);
    // eye vec4: xyz + zero padding to match alignment.
    out[128..132].copy_from_slice(&eye[0].to_le_bytes());
    out[132..136].copy_from_slice(&eye[1].to_le_bytes());
    out[136..140].copy_from_slice(&eye[2].to_le_bytes());
    out[140..144].copy_from_slice(&0.0_f32.to_le_bytes());
    // time vec4: x + padding.
    out[144..148].copy_from_slice(&time.to_le_bytes());
    out[148..152].copy_from_slice(&0.0_f32.to_le_bytes());
    out[152..156].copy_from_slice(&0.0_f32.to_le_bytes());
    out[156..160].copy_from_slice(&0.0_f32.to_le_bytes());
    out
}

/// Orthographic projection matching the wgpu/Vulkan convention
/// (z in [0, 1]).  Used by the shadow camera so the depth values
/// the comparison sampler reads back are in the expected range.
fn ortho(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let rl = right - left;
    let tb = top - bottom;
    let fn_ = far - near;
    [
        [2.0 / rl, 0.0, 0.0, 0.0],
        [0.0, 2.0 / tb, 0.0, 0.0],
        [0.0, 0.0, -1.0 / fn_, 0.0],
        [
            -(right + left) / rl,
            -(top + bottom) / tb,
            -near / fn_,
            1.0,
        ],
    ]
}

/// Build the directional light's view-projection matrix.  Matches the
/// LIGHT direction in `fs_scene` so the shadow ray walks back toward
/// the same key light position.
fn light_view_proj() -> [[f32; 4]; 4] {
    let light_dir = vec3_normalize([0.55, 0.72, -0.42]);
    let light_eye = [
        light_dir[0] * 6.0,
        light_dir[1] * 6.0,
        light_dir[2] * 6.0,
    ];
    let view = look_at(light_eye, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    // Ortho frustum sized to comfortably contain the node-sphere
    // shell + cable network.  Tweaked by feeling.
    let proj = ortho(-2.5, 2.5, -2.5, 2.5, 0.1, 12.0);
    mat4_mul(proj, view)
}

// ── Mat4 math (host-side, kept inline; the kernel side uses k-rast) ──

fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0f32; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += a[r][k] * b[k][c];
            }
            out[r][c] = s;
        }
    }
    out
}

fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (fov_y / 2.0).tan();
    let nf = 1.0 / (near - far);
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far * nf, -1.0],
        [0.0, 0.0, far * near * nf, 0.0],
    ]
}

fn look_at(eye: [f32; 3], centre: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let f = vec3_normalize(vec3_sub(centre, eye));
    let s = vec3_normalize(vec3_cross(f, up));
    let u = vec3_cross(s, f);
    [
        [s[0], u[0], -f[0], 0.0],
        [s[1], u[1], -f[1], 0.0],
        [s[2], u[2], -f[2], 0.0],
        [-vec3_dot(s, eye), -vec3_dot(u, eye), vec3_dot(f, eye), 1.0],
    ]
}

fn vec3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn vec3_normalize(v: [f32; 3]) -> [f32; 3] {
    let len = vec3_dot(v, v).sqrt().max(1e-6);
    [v[0] / len, v[1] / len, v[2] / len]
}
