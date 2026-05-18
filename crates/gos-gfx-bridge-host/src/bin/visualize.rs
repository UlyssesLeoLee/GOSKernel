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
    view_proj: mat4x4<f32>,
    eye:       vec4<f32>,    // .xyz = camera position, .w unused
    time:      vec4<f32>,    // .x = seconds since boot, rest unused
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
    // something to feed on).
    let radiance = vec3<f32>(1.0, 0.95, 0.88) * 4.0;
    let lo = (k_d * albedo / PI + specular) * radiance * n_dot_l;

    // Hemispheric ambient: sky tint above, ground tint below, blend
    // by the normal's vertical component.  Stand-in until IBL lands.
    let sky_tint = vec3<f32>(0.10, 0.13, 0.22);
    let ground_tint = vec3<f32>(0.04, 0.03, 0.05);
    let hemi = mix(ground_tint, sky_tint, (n.y + 1.0) * 0.5);
    let ambient = hemi * albedo * (1.0 - metallic * 0.4);

    // Fresnel rim retained — physically it's already in `specular`,
    // but the explicit rim boost preserves the sci-fi silhouette
    // glow we tuned in the previous slice.
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

@fragment
fn fs_bg(in: BgVsOut) -> @location(0) vec4<f32> {
    // Animated nebula: two FBM layers slowly drifting in opposite
    // directions create swirling cloud structure.  Hue picked from a
    // deep purple-to-teal axis so the foreground (warm sphere
    // highlights, electric-yellow stars) reads cleanly on top.
    let drift0 = pc.time.x * 0.018;
    let drift1 = pc.time.x * -0.012;
    let layer0 = fbm(in.ndc * 1.4 + vec2<f32>(drift0, drift0 * 0.6));
    let layer1 = fbm(in.ndc * 2.7 + vec2<f32>(drift1, drift1 * -0.8));
    let cloud = pow(layer0 * 0.65 + layer1 * 0.35, 1.6);

    // Vertical falloff retained — gives the scene a horizon-ish
    // orientation cue under the swirling clouds.
    let v_axis = in.ndc.y * 0.5 + 0.5;
    let base = mix(
        vec3<f32>(0.015, 0.020, 0.050),  // bottom — near-black void
        vec3<f32>(0.030, 0.050, 0.110),  // top — faint deep teal
        v_axis,
    );
    let nebula_peak_a = vec3<f32>(0.20, 0.10, 0.35);  // royal purple
    let nebula_peak_b = vec3<f32>(0.05, 0.25, 0.35);  // cyan-teal
    let nebula_mix = mix(nebula_peak_a, nebula_peak_b, layer1);
    let nebula = base + nebula_mix * cloud * 0.55;

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

@fragment
fn fs_bloom(in: PostVsOut) -> @location(0) vec4<f32> {
    // 5×5 box-average with luminance threshold = 0.9.  Sample step
    // is one HDR-texel relative to the half-res target.  Using
    // texture_dimensions to compute the step avoids hard-coding a
    // resolution.
    let dims = vec2<f32>(textureDimensions(hdr_tex, 0));
    let step = vec2<f32>(1.0 / dims.x, 1.0 / dims.y);
    var acc = vec3<f32>(0.0);
    let threshold = 0.9;
    let kernel_radius = 2;
    let kernel_diameter = 5.0;
    var weight_sum = 0.0;
    for (var dy = -kernel_radius; dy <= kernel_radius; dy = dy + 1) {
        for (var dx = -kernel_radius; dx <= kernel_radius; dx = dx + 1) {
            let uv = in.uv + vec2<f32>(f32(dx) * step.x * 2.0, f32(dy) * step.y * 2.0);
            let c = textureSample(hdr_tex, hdr_samp, uv).rgb;
            // Soft threshold: subtract floor, allow negative clamp.
            let luma = relative_luminance(c);
            let factor = max(luma - threshold, 0.0) / max(luma, 0.0001);
            acc = acc + c * factor;
            weight_sum = weight_sum + 1.0;
        }
    }
    let bloom = acc / weight_sum;
    return vec4<f32>(bloom, 1.0);
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
            // Push constant size: 64 (mat4) + 16 (eye) + 16 (time pad) = 96 B.
            // Bump from 64; downlevel default is 0 so we set it explicitly.
            required_limits: wgpu::Limits {
                max_push_constant_size: 128,
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
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("cosmic-pipeline-layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[wgpu::PushConstantRange {
            stages: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            range: 0..96,
        }],
    });
    let vertex_layout = wgpu::VertexBufferLayout {
        array_stride: BYTES_PER_PBR_VERTEX as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
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
        ],
    };
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
    let bloom_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("bloom-pipeline"),
        layout: Some(&post_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_post",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_bloom",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: HDR_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });
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
                        let push_bytes = build_push_constants(view_proj, eye, elapsed);

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
                        let bloom_bg = make_bg(
                            "bloom-bg",
                            &offscreen.hdr_view,
                            &offscreen.hdr_view,
                            &offscreen.hdr_view,
                        );
                        let composite_bg = make_bg(
                            "composite-bg",
                            &offscreen.hdr_view,
                            &offscreen.bloom_view,
                            &offscreen.hdr_view,
                        );
                        let fxaa_bg = make_bg(
                            "fxaa-bg",
                            &offscreen.hdr_view,
                            &offscreen.hdr_view,
                            &offscreen.ldr_view,
                        );

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

                        // Pass 2 — bloom extract + blur into the
                        // half-res bloom target.  Reads HDR via
                        // bloom_bg.
                        {
                            let mut pass = encoder.begin_render_pass(
                                &wgpu::RenderPassDescriptor {
                                    label: Some("bloom-pass"),
                                    color_attachments: &[Some(
                                        wgpu::RenderPassColorAttachment {
                                            view: &offscreen.bloom_view,
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
                            pass.set_pipeline(&bloom_pipeline);
                            pass.set_bind_group(0, &bloom_bg, &[]);
                            pass.draw(0..3, 0..1);
                        }

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
    _bloom_tex: wgpu::Texture,
    bloom_view: wgpu::TextureView,
    _ldr_tex: wgpu::Texture,
    ldr_view: wgpu::TextureView,
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
    let bloom_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bloom"),
        size: wgpu::Extent3d {
            width: (w / 2).max(1),
            height: (h / 2).max(1),
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
    let bloom_view = bloom_tex.create_view(&wgpu::TextureViewDescriptor::default());
    // LDR intermediate consumed by FXAA.  Full-res; pixel count tied
    // to the surface so FXAA samples are texel-aligned.
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
        _bloom_tex: bloom_tex,
        bloom_view,
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

fn build_push_constants(view_proj: [[f32; 4]; 4], eye: [f32; 3], time: f32) -> [u8; 96] {
    let mut out = [0u8; 96];
    // mat4x4 in column-major (WGSL convention).  Our matrix is
    // row-major (rows are arrays), so transpose during the copy.
    let mut off = 0;
    for col in 0..4 {
        for row in 0..4 {
            out[off..off + 4].copy_from_slice(&view_proj[row][col].to_le_bytes());
            off += 4;
        }
    }
    // eye vec4: xyz + zero padding to match alignment.
    out[64..68].copy_from_slice(&eye[0].to_le_bytes());
    out[68..72].copy_from_slice(&eye[1].to_le_bytes());
    out[72..76].copy_from_slice(&eye[2].to_le_bytes());
    out[76..80].copy_from_slice(&0.0_f32.to_le_bytes());
    // time vec4: x + padding.
    out[80..84].copy_from_slice(&time.to_le_bytes());
    out[84..88].copy_from_slice(&0.0_f32.to_le_bytes());
    out[88..92].copy_from_slice(&0.0_f32.to_le_bytes());
    out[92..96].copy_from_slice(&0.0_f32.to_le_bytes());
    out
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
