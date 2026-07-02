// ```cypher
// CREATE
//   (file:File {name: "main.rs", type: "file", language: "rust"}),
//   (node:Class {name: "Node", type: "class", signature: "struct Node { kx,ky: f32, is_red: bool }"}),
//   (frame:Class {name: "Frame", type: "class", signature: "struct Frame { w,h:u32, bg:[f64;3], nodes:Vec<Node>, edges, id_index }"}),
//   (vtx:Class {name: "Vertex3", type: "class", signature: "struct Vertex3 { pos:[f32;3], normal:[f32;3] }"}),
//   (inst:Class {name: "Instance", type: "class", signature: "struct Instance { model:[[f32;4];4], color:[f32;4] }"}),
//   (uni:Class {name: "Uniforms", type: "class", signature: "struct Uniforms { view_proj, light_dir, camera_pos }"}),
//   (hexc:Function {name: "hexc", type: "function", signature: "fn hexc(s:&str)->(f32,f32,f32)", visibility: "private"}),
//   (envv:Function {name: "env_vec3", type: "function", signature: "fn env_vec3(key:&str,def:Vec3)->Vec3", visibility: "private"}),
//   (envf:Function {name: "env_f32", type: "function", signature: "fn env_f32(key:&str,def:f32)->f32", visibility: "private"}),
//   (parse:Function {name: "parse_last_frame", type: "function", signature: "fn parse_last_frame(text:&str)->Option<Frame>", visibility: "private", complexity: "complex"}),
//   (fib:Function {name: "fib_sphere", type: "function", signature: "fn fib_sphere(i,n)->Vec3", visibility: "private"}),
//   (lforce:Function {name: "layout_force", type: "function", signature: "fn layout_force(n:usize, edges:&[(usize,usize)])->Vec<Vec3>", visibility: "private", complexity: "complex"}),
//   (lrings:Function {name: "layout_rings", type: "function", signature: "fn layout_rings(nodes:&[Node])->Vec<Vec3>", visibility: "private", complexity: "complex"}),
//   (rope:Function {name: "rope_segment", type: "function", signature: "fn rope_segment(p,q,color)->Instance", visibility: "private"}),
//   (scene:Function {name: "build_scene", type: "function", signature: "fn build_scene(f:&Frame)->(Vec<Instance>,Vec<Instance>)", visibility: "private", complexity: "complex"}),
//   (msph:Function {name: "make_sphere", type: "function", signature: "fn make_sphere(stacks,slices)->(Vec<Vertex3>,Vec<u16>)", visibility: "private"}),
//   (mcyl:Function {name: "make_cylinder", type: "function", signature: "fn make_cylinder(slices)->(Vec<Vertex3>,Vec<u16>)", visibility: "private"}),
//   (rend:Function {name: "render3d_to_png", type: "function", signature: "fn render3d_to_png(frame,&[Instance],&[Instance],out)", visibility: "private", complexity: "complex"}),
//   (main:Function {name: "main", type: "function", signature: "fn main()", visibility: "private"}),
//   (vpos:Variable {name: "pos", type: "variable"}),
//   (vframe:Variable {name: "frame", type: "variable"}),
//   (file)-[:CONTAINS]->(node), (file)-[:CONTAINS]->(frame), (file)-[:CONTAINS]->(vtx),
//   (file)-[:CONTAINS]->(inst), (file)-[:CONTAINS]->(uni), (file)-[:CONTAINS]->(hexc),
//   (file)-[:CONTAINS]->(envv), (file)-[:CONTAINS]->(envf), (file)-[:CONTAINS]->(parse),
//   (file)-[:CONTAINS]->(fib), (file)-[:CONTAINS]->(lforce), (file)-[:CONTAINS]->(lrings),
//   (file)-[:CONTAINS]->(rope), (file)-[:CONTAINS]->(scene), (file)-[:CONTAINS]->(msph),
//   (file)-[:CONTAINS]->(mcyl), (file)-[:CONTAINS]->(rend), (file)-[:CONTAINS]->(main),
//   (parse)-[:CALLS]->(hexc), (parse)-[:USES]->(vframe),
//   (lforce)-[:CALLS]->(fib), (lforce)-[:USES]->(vpos), (lrings)-[:USES]->(vpos),
//   (scene)-[:CALLS]->(lforce), (scene)-[:CALLS]->(lrings), (scene)-[:CALLS]->(rope), (scene)-[:USES]->(vpos),
//   (rend)-[:CALLS]->(msph), (rend)-[:CALLS]->(mcyl), (rend)-[:CALLS]->(envv), (rend)-[:CALLS]->(envf),
//   (main)-[:CALLS]->(parse), (main)-[:CALLS]->(scene), (main)-[:CALLS]->(rend), (main)-[:USES]->(vframe);
// ```
//
// GOS HOST TOOLING — gos-vk-viewer (Phase I, Vulkan Gen-1 · 3D volumetric)
// Renders the kernel's @gos.vk graph as the "红白绳红白球" scene: metallic
// red/white spheres + two-tone ropes (rope half nearest a red ball is white,
// nearest a white ball is red). Two deterministic 3D layouts: a stacked-ring
// "pagoda" by tier (default, clean) and a force-directed cloud (VK_LAYOUT=force).
// Camera/fov are env-tunable (VK_EYE="x,y,z", VK_FOV, VK_LAYOUT) so the look can
// be retuned without recompiling. Native wgpu (Vulkan), instanced + depth-tested
// + metallic lighting, offscreen -> PNG. Same @gos.vk protocol; zero kernel change.

use std::collections::{HashMap, VecDeque};
use std::f32::consts::PI;
use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

const RED: [f32; 3] = [0.86, 0.11, 0.13];
const WHITE: [f32; 3] = [0.93, 0.93, 0.95];
const NODE_R: f32 = 0.34;
const ROPE_R: f32 = 0.045;
const FIT_R: f32 = 4.0;
/// Max recently-sent input bytes shown by the B3b input-echo overlay.
const ECHO_CAP: usize = 24;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex3 {
    pos: [f32; 3],
    normal: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    model: [[f32; 4]; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 4],
    camera_pos: [f32; 4],
}

struct Node {
    kx: f32,
    ky: f32,
    is_red: bool,
}

struct Frame {
    w: u32,
    h: u32,
    bg: [f64; 3],
    nodes: Vec<Node>,
    edges: Vec<(String, String)>,
    id_index: HashMap<String, usize>,
}

fn hexc(s: &str) -> (f32, f32, f32) {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return (0.0, 0.0, 0.0);
    }
    let v = u32::from_str_radix(s, 16).unwrap_or(0);
    (((v >> 16) & 0xff) as f32 / 255.0, ((v >> 8) & 0xff) as f32 / 255.0, (v & 0xff) as f32 / 255.0)
}

fn env_vec3(key: &str, def: Vec3) -> Vec3 {
    std::env::var(key)
        .ok()
        .and_then(|s| {
            let p: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if p.len() == 3 { Some(Vec3::new(p[0], p[1], p[2])) } else { None }
        })
        .unwrap_or(def)
}

fn env_f32(key: &str, def: f32) -> f32 {
    std::env::var(key).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(def)
}

/// Walk the @gos.vk stream, return the LAST complete frame. Keeps the kernel's
/// 2D coords (used by the tiered ring layout) + a default red/white alternation.
fn parse_last_frame(text: &str) -> Option<Frame> {
    let mut cur: Option<Frame> = None;
    let mut last: Option<Frame> = None;
    for line in text.replace("\r\n", "\n").split('\n') {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("VKDIM:") {
            let (mut w, mut h) = (800u32, 600u32);
            if let Some((ws, hs)) = rest.to_lowercase().split_once('x') {
                w = ws.trim().parse().unwrap_or(800);
                h = hs.trim().parse().unwrap_or(600);
            }
            cur = Some(Frame { w, h, bg: [0.04, 0.04, 0.06], nodes: Vec::new(), edges: Vec::new(), id_index: HashMap::new() });
        } else if let Some(rest) = line.strip_prefix("VKCLR:") {
            if let Some(f) = cur.as_mut() {
                let (r, g, b) = hexc(rest);
                f.bg = [r as f64, g as f64, b as f64];
            }
        } else if let Some(rest) = line.strip_prefix("VKNOD:") {
            if let Some(f) = cur.as_mut() {
                let p: Vec<&str> = rest.splitn(7, ':').collect();
                if p.len() >= 5 && !p[0].is_empty() {
                    let idx = f.nodes.len();
                    let kx = p[1].parse::<f32>().ok().filter(|v| v.is_finite()).unwrap_or(0.0);
                    let ky = p[2].parse::<f32>().ok().filter(|v| v.is_finite()).unwrap_or(0.0);
                    f.nodes.push(Node { kx, ky, is_red: idx % 2 == 0 });
                    f.id_index.insert(p[0].to_string(), idx);
                }
            }
        } else if let Some(rest) = line.strip_prefix("VKEDG:") {
            if let Some(f) = cur.as_mut() {
                let p: Vec<&str> = rest.splitn(3, ':').collect();
                if p.len() >= 2 {
                    f.edges.push((p[0].to_string(), p[1].to_string()));
                }
            }
        } else if line.starts_with("VKEND:") {
            if let Some(f) = cur.take() {
                if !f.nodes.is_empty() {
                    last = Some(f);
                }
            }
        }
    }
    last
}

fn fib_sphere(i: usize, n: usize) -> Vec3 {
    let golden = PI * (3.0 - 5f32.sqrt());
    let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
    let r = (1.0 - y * y).max(0.0).sqrt();
    let theta = golden * i as f32;
    Vec3::new(theta.cos() * r, y, theta.sin() * r)
}

/// Deterministic 3D force-directed layout (Fruchterman–Reingold), Fibonacci-seeded.
fn layout_force(n: usize, edges: &[(usize, usize)]) -> Vec<Vec3> {
    if n == 0 {
        return Vec::new();
    }
    let mut pos: Vec<Vec3> = (0..n).map(|i| fib_sphere(i, n) * 3.0).collect();
    if n == 1 {
        pos[0] = Vec3::ZERO;
        return pos;
    }
    let k = 2.0f32;
    let mut temp = 2.5f32;
    for _ in 0..400 {
        let mut disp = vec![Vec3::ZERO; n];
        for i in 0..n {
            for j in (i + 1)..n {
                let d = pos[i] - pos[j];
                let dist = d.length().max(0.05);
                let f = (k * k / dist) * (d / dist);
                disp[i] += f;
                disp[j] -= f;
            }
        }
        for &(a, b) in edges {
            let d = pos[a] - pos[b];
            let dist = d.length().max(0.05);
            let f = (dist * dist / k) * (d / dist);
            disp[a] -= f;
            disp[b] += f;
        }
        for i in 0..n {
            let dl = disp[i].length().max(1e-4);
            pos[i] += disp[i] / dl * dl.min(temp);
            pos[i] *= 0.985;
        }
        temp = (temp * 0.985).max(0.05);
    }
    pos
}

/// Deterministic stacked-ring "pagoda" layout: bucket nodes into tiers by their
/// kernel row (ky), give each tier a height, and spread its members around a
/// horizontal ring (radius grows with member count). Single-member tiers center.
fn layout_rings(nodes: &[Node]) -> Vec<Vec3> {
    let n = nodes.len();
    if n == 0 {
        return Vec::new();
    }
    let mut keys: Vec<f32> = nodes.iter().map(|n| n.ky).collect();
    keys.sort_by(|a, b| a.total_cmp(b));
    keys.dedup_by(|a, b| (*a - *b).abs() < 1.0);
    let nt = keys.len().max(1);
    let tier_of = |ky: f32| keys.iter().position(|&t| (t - ky).abs() < 1.0).unwrap_or(0);

    let mut members: Vec<Vec<usize>> = vec![Vec::new(); nt];
    for (i, nd) in nodes.iter().enumerate() {
        members[tier_of(nd.ky)].push(i);
    }
    for m in &mut members {
        m.sort_by(|&a, &b| nodes[a].kx.total_cmp(&nodes[b].kx));
    }

    let mut pos = vec![Vec3::ZERO; n];
    let hspan = 6.5f32;
    for (t, m) in members.iter().enumerate() {
        let y = if nt <= 1 { 0.0 } else { hspan * 0.5 - hspan * t as f32 / (nt - 1) as f32 };
        let cnt = m.len().max(1);
        let radius = (0.19 * cnt as f32).max(1.7);
        let twist = t as f32 * 0.55;
        for (j, &ni) in m.iter().enumerate() {
            if cnt == 1 {
                pos[ni] = Vec3::new(0.0, y, 0.0);
            } else {
                let ang = 2.0 * PI * j as f32 / cnt as f32 + twist;
                pos[ni] = Vec3::new(radius * ang.cos(), y, radius * ang.sin());
            }
        }
    }
    pos
}

fn rope_segment(p: Vec3, q: Vec3, color: [f32; 3]) -> Instance {
    let dir = q - p;
    let len = dir.length().max(1e-4);
    let rot = Quat::from_rotation_arc(Vec3::Y, dir / len);
    let model = Mat4::from_translation(p) * Mat4::from_quat(rot) * Mat4::from_scale(Vec3::new(ROPE_R, len, ROPE_R));
    Instance { model: model.to_cols_array_2d(), color: [color[0], color[1], color[2], 1.0] }
}

fn build_scene(f: &Frame) -> (Vec<Instance>, Vec<Instance>) {
    let n = f.nodes.len();
    let resolved: Vec<(usize, usize)> = f
        .edges
        .iter()
        .filter_map(|(a, b)| Some((*f.id_index.get(a)?, *f.id_index.get(b)?)))
        .collect();

    let mode = std::env::var("VK_LAYOUT").unwrap_or_else(|_| "rings".into());
    let mut pos = if mode == "force" { layout_force(n, &resolved) } else { layout_rings(&f.nodes) };

    let centroid = pos.iter().fold(Vec3::ZERO, |acc, p| acc + *p) / n.max(1) as f32;
    for p in &mut pos {
        *p -= centroid;
    }
    let maxr = pos.iter().map(|p| p.length()).fold(0.01f32, f32::max);
    let s = FIT_R / maxr;
    for p in &mut pos {
        *p *= s;
    }

    let mut spheres = Vec::with_capacity(n);
    for (i, node) in f.nodes.iter().enumerate() {
        let c = if node.is_red { RED } else { WHITE };
        let model = Mat4::from_translation(pos[i]) * Mat4::from_scale(Vec3::splat(NODE_R));
        spheres.push(Instance { model: model.to_cols_array_2d(), color: [c[0], c[1], c[2], 1.0] });
    }
    let mut ropes = Vec::with_capacity(resolved.len() * 2);
    for &(ia, ib) in &resolved {
        let (pa, pb) = (pos[ia], pos[ib]);
        let mid = (pa + pb) * 0.5;
        let near_a = if f.nodes[ia].is_red { WHITE } else { RED };
        let near_b = if f.nodes[ib].is_red { WHITE } else { RED };
        ropes.push(rope_segment(pa, mid, near_a));
        ropes.push(rope_segment(mid, pb, near_b));
    }
    (spheres, ropes)
}

fn make_sphere(stacks: u32, slices: u32) -> (Vec<Vertex3>, Vec<u16>) {
    let mut v = Vec::new();
    let mut idx = Vec::new();
    for i in 0..=stacks {
        let phi = PI * i as f32 / stacks as f32;
        let (sp, cp) = phi.sin_cos();
        for j in 0..=slices {
            let theta = 2.0 * PI * j as f32 / slices as f32;
            let (st, ct) = theta.sin_cos();
            let n = [sp * ct, cp, sp * st];
            v.push(Vertex3 { pos: n, normal: n });
        }
    }
    let stride = slices + 1;
    for i in 0..stacks {
        for j in 0..slices {
            let a = i * stride + j;
            let b = a + stride;
            idx.extend_from_slice(&[a as u16, b as u16, (a + 1) as u16, (a + 1) as u16, b as u16, (b + 1) as u16]);
        }
    }
    (v, idx)
}

fn make_cylinder(slices: u32) -> (Vec<Vertex3>, Vec<u16>) {
    let mut v = Vec::new();
    let mut idx = Vec::new();
    for j in 0..=slices {
        let theta = 2.0 * PI * j as f32 / slices as f32;
        let (st, ct) = theta.sin_cos();
        let n = [ct, 0.0, st];
        v.push(Vertex3 { pos: [ct, 0.0, st], normal: n });
        v.push(Vertex3 { pos: [ct, 1.0, st], normal: n });
    }
    for j in 0..slices {
        let b0 = (j * 2) as u16;
        let t0 = b0 + 1;
        let b1 = ((j + 1) * 2) as u16;
        let t1 = b1 + 1;
        idx.extend_from_slice(&[b0, t0, b1, b1, t0, t1]);
    }
    (v, idx)
}

/// Unit quad in the XY plane (z=0, normal=+Z), centered at the origin,
/// spanning [-0.5, 0.5]^2. Used by the B3b input-echo HUD overlay, drawn
/// with `view_proj = IDENTITY` so instance positions map directly to NDC,
/// independent of the orbit camera.
fn make_quad() -> (Vec<Vertex3>, Vec<u16>) {
    let n = [0.0, 0.0, 1.0];
    let v = vec![
        Vertex3 { pos: [-0.5, -0.5, 0.0], normal: n },
        Vertex3 { pos: [0.5, -0.5, 0.0], normal: n },
        Vertex3 { pos: [0.5, 0.5, 0.0], normal: n },
        Vertex3 { pos: [-0.5, 0.5, 0.0], normal: n },
    ];
    (v, vec![0, 1, 2, 0, 2, 3])
}

const SHADER: &str = r#"
struct Uniforms { view_proj: mat4x4<f32>, light_dir: vec4<f32>, camera_pos: vec4<f32> };
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsIn {
  @location(0) pos: vec3<f32>,
  @location(1) normal: vec3<f32>,
  @location(2) m0: vec4<f32>, @location(3) m1: vec4<f32>,
  @location(4) m2: vec4<f32>, @location(5) m3: vec4<f32>,
  @location(6) color: vec4<f32>,
};
struct VsOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) world: vec3<f32>,
  @location(1) normal: vec3<f32>,
  @location(2) color: vec4<f32>,
};

@vertex fn vs(in: VsIn) -> VsOut {
  let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
  let wp = model * vec4<f32>(in.pos, 1.0);
  var o: VsOut;
  o.clip = u.view_proj * wp;
  o.world = wp.xyz;
  o.normal = (model * vec4<f32>(in.normal, 0.0)).xyz;
  o.color = in.color;
  return o;
}

@fragment fn fs(in: VsOut) -> @location(0) vec4<f32> {
  let N = normalize(in.normal);
  let L = normalize(u.light_dir.xyz);
  let V = normalize(u.camera_pos.xyz - in.world);
  let H = normalize(L + V);
  let diff = max(dot(N, L), 0.0);
  let spec = pow(max(dot(N, H), 0.0), 80.0);
  let rim = pow(1.0 - max(dot(N, V), 0.0), 3.0) * 0.15;
  let base = in.color.rgb;
  let spec_col = mix(vec3<f32>(1.0, 1.0, 1.0), base, 0.55) * spec * 0.9;
  let col = base * (0.16 + 0.85 * diff) + spec_col + rim;
  return vec4<f32>(pow(col, vec3<f32>(0.4545)), 1.0);
}
"#;

fn render3d_to_png(frame: &Frame, spheres: &[Instance], ropes: &[Instance], out: &str) {
    let (w, h) = (frame.w, frame.h);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::VULKAN, ..Default::default() });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .expect("no Vulkan adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("gos-vk-viewer"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        },
        None,
    ))
    .expect("request_device");

    let fmt = wgpu::TextureFormat::Rgba8Unorm;
    let color_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: fmt,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let color_view = color_tex.create_view(&wgpu::TextureViewDescriptor::default());
    let depth_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth_tex.create_view(&wgpu::TextureViewDescriptor::default());

    let aspect = w as f32 / h as f32;
    let eye = env_vec3("VK_EYE", Vec3::new(6.5, 1.8, 9.5));
    let fov = env_f32("VK_FOV", 46.0);
    let proj = Mat4::perspective_rh(fov.to_radians(), aspect, 0.1, 100.0);
    let view = Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y);
    let view_proj = (proj * view).to_cols_array_2d();
    let ld = Vec3::new(-0.35, 0.85, 0.65).normalize();
    let uniforms = Uniforms { view_proj, light_dir: [ld.x, ld.y, ld.z, 0.0], camera_pos: [eye.x, eye.y, eye.z, 1.0] };
    let ubuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("uniforms"),
        contents: bytemuck::bytes_of(&uniforms),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        }],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bg"),
        layout: &bgl,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() }],
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("lit"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&bgl], push_constant_ranges: &[] });
    let mesh_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
    let inst_attrs = wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4];
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("lit"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs",
            buffers: &[
                wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex3>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &mesh_attrs },
                wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Instance>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &inst_attrs },
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs",
            targets: &[Some(wgpu::ColorTargetState { format: fmt, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
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

    let (sv, si) = make_sphere(20, 28);
    let (cv, ci) = make_cylinder(14);
    let sphere_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("sv"), contents: bytemuck::cast_slice(&sv), usage: wgpu::BufferUsages::VERTEX });
    let sphere_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("si"), contents: bytemuck::cast_slice(&si), usage: wgpu::BufferUsages::INDEX });
    let cyl_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("cv"), contents: bytemuck::cast_slice(&cv), usage: wgpu::BufferUsages::VERTEX });
    let cyl_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("ci"), contents: bytemuck::cast_slice(&ci), usage: wgpu::BufferUsages::INDEX });
    let sphere_inst = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("sphere_inst"), contents: bytemuck::cast_slice(spheres), usage: wgpu::BufferUsages::VERTEX });
    let rope_inst = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("rope_inst"), contents: bytemuck::cast_slice(ropes), usage: wgpu::BufferUsages::VERTEX });

    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let unpadded = w * 4;
    let padded = unpadded.div_ceil(align) * align;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (padded * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: frame.bg[0], g: frame.bg[1], b: frame.bg[2], a: 1.0 }), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rp.set_pipeline(&pipeline);
        rp.set_bind_group(0, &bind_group, &[]);
        rp.set_vertex_buffer(0, cyl_vb.slice(..));
        rp.set_vertex_buffer(1, rope_inst.slice(..));
        rp.set_index_buffer(cyl_ib.slice(..), wgpu::IndexFormat::Uint16);
        rp.draw_indexed(0..ci.len() as u32, 0, 0..ropes.len() as u32);
        rp.set_vertex_buffer(0, sphere_vb.slice(..));
        rp.set_vertex_buffer(1, sphere_inst.slice(..));
        rp.set_index_buffer(sphere_ib.slice(..), wgpu::IndexFormat::Uint16);
        rp.draw_indexed(0..si.len() as u32, 0, 0..spheres.len() as u32);
    }
    enc.copy_texture_to_buffer(
        wgpu::ImageCopyTexture { texture: &color_tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::ImageCopyBuffer { buffer: &readback, layout: wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);

    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().unwrap();
    let data = slice.get_mapped_range();
    let mut img = image::RgbaImage::new(w, h);
    for y in 0..h {
        let row = &data[(y * padded) as usize..(y * padded + unpadded) as usize];
        for x in 0..w {
            let i = (x * 4) as usize;
            img.put_pixel(x, y, image::Rgba([row[i], row[i + 1], row[i + 2], row[i + 3]]));
        }
    }
    img.save(out).expect("save png");
}

// ── Live mode (`--live`): a winit window + wgpu surface that renders the
// kernel's live @gos.vk stream on the host GPU (smooth, high-res, host VRAM).
// Reuses the same shader/meshes/scene-builder as the PNG path; only the target
// (surface vs offscreen texture) and the frame source (TCP stream vs file)
// differ. Camera is fixed (env VK_EYE/VK_FOV) for B1 — interactive orbit/input
// is B3 (forward window input back to the kernel). ─────────────────────────────

type SharedScene = std::sync::Arc<std::sync::Mutex<Option<(Vec<Instance>, Vec<Instance>, [f64; 3])>>>;
/// Write-half of the live socket, published by the reader once connected, so the
/// event loop can forward keyboard back to the kernel (B3b) on the same stream.
type InputTx = std::sync::Arc<std::sync::Mutex<Option<std::net::TcpStream>>>;

/// Background thread: connect to the kernel's @gos.vk bridge (COM3 → TCP) and
/// keep `shared` updated with the most recent complete frame. Reconnects on drop.
fn spawn_frame_reader(addr: String, shared: SharedScene, input_tx: InputTx) {
    use std::io::Read;
    std::thread::spawn(move || loop {
        match std::net::TcpStream::connect(&addr) {
            Ok(mut stream) => {
                eprintln!("[viewer] connected to {addr}; waiting for @gos.vk frames…");
                // Publish a write-half so the event loop can send keyboard back.
                // Non-blocking so a stalled kernel never freezes the event thread.
                if let Ok(c) = stream.try_clone() {
                    let _ = c.set_nonblocking(true);
                    if let Ok(mut g) = input_tx.lock() {
                        *g = Some(c);
                    }
                }
                let mut acc = String::new();
                let mut buf = [0u8; 4096];
                let mut last_n = usize::MAX;
                loop {
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => break, // disconnected → outer loop reconnects
                        Ok(n) => {
                            acc.push_str(&String::from_utf8_lossy(&buf[..n]));
                            // Bound memory: @gos.vk frames are small; keep only the tail.
                            if acc.len() > 1 << 16 {
                                let cut = acc.len() - (1 << 15);
                                acc.drain(..cut);
                            }
                            if let Some(frame) = parse_last_frame(&acc) {
                                if frame.nodes.len() != last_n {
                                    last_n = frame.nodes.len();
                                    eprintln!("[viewer] @gos.vk frame: {} nodes ({}x{})", frame.nodes.len(), frame.w, frame.h);
                                }
                                // Only adopt frames that actually have content. The kernel
                                // can emit a CLEAR/HELLO frame (no nodes) between graph
                                // frames; rendering that as an empty scene would blink the
                                // graph out for a frame. Hold the last non-empty scene.
                                if !frame.nodes.is_empty() {
                                    let (s, r) = build_scene(&frame);
                                    if let Ok(mut g) = shared.lock() {
                                        *g = Some((s, r, frame.bg));
                                    }
                                }
                            }
                        }
                    }
                }
                if let Ok(mut g) = input_tx.lock() {
                    *g = None; // disconnected → stop sending until reconnect
                }
            }
            Err(e) => eprintln!("[viewer] connect {addr} failed ({e}); retrying…"),
        }
        std::thread::sleep(std::time::Duration::from_millis(500)); // retry backoff
    });
}

fn make_depth_view(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn live_uniforms(w: u32, h: u32, yaw: f32, pitch: f32, dist: f32) -> Uniforms {
    let aspect = w as f32 / h.max(1) as f32;
    // Orbit camera looking at the origin: azimuth `yaw`, elevation `pitch`,
    // radius `dist` — driven by mouse drag/wheel (B3a), with a gentle idle spin.
    let cp = pitch.cos();
    let eye = Vec3::new(dist * cp * yaw.sin(), dist * pitch.sin(), dist * cp * yaw.cos());
    let fov = env_f32("VK_FOV", 46.0);
    let proj = Mat4::perspective_rh(fov.to_radians(), aspect, 0.1, 100.0);
    let view = Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y);
    let view_proj = (proj * view).to_cols_array_2d();
    let ld = Vec3::new(-0.35, 0.85, 0.65).normalize();
    Uniforms { view_proj, light_dir: [ld.x, ld.y, ld.z, 0.0], camera_pos: [eye.x, eye.y, eye.z, 1.0] }
}

fn run_live(addr: &str) {
    use wgpu::util::DeviceExt;
    use winit::event::{Event, WindowEvent};
    use winit::event_loop::EventLoop;
    use winit::window::WindowBuilder;

    let shared: SharedScene = std::sync::Arc::new(std::sync::Mutex::new(None));
    let input_tx: InputTx = std::sync::Arc::new(std::sync::Mutex::new(None));
    // Seed a built-in demo scene so the window is NEVER blank. Diagnostic value:
    // if you see THESE balls but not your live graph, the renderer is fine and
    // the problem is the @gos.vk data link (kernel not emitting / not connected).
    // The first live frame from the kernel replaces this immediately.
    const DEMO_VK: &str = "VKDIM:800x600\nVKCLR:101018\n\
VKNOD:1:320:60:160:64:2a6cff:gos-kernel\n\
VKNOD:2:120:300:140:56:23a06a:k-vga\n\
VKNOD:3:540:300:140:56:c2410c:k-shell\n\
VKEDG:1:2:5566aa\nVKEDG:1:3:5566aa\nVKEND:\n";
    if let Some(frame) = parse_last_frame(DEMO_VK) {
        let (s, r) = build_scene(&frame);
        if let Ok(mut g) = shared.lock() {
            *g = Some((s, r, frame.bg));
        }
    }
    spawn_frame_reader(addr.to_string(), shared.clone(), input_tx.clone());

    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let window = std::sync::Arc::new(
        WindowBuilder::new()
            .with_title("gos-vk-viewer — live @gos.vk")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0))
            .build(&event_loop)
            .expect("window"),
    );

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::VULKAN, ..Default::default() });
    let surface = instance.create_surface(window.clone()).expect("surface");
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .expect("no Vulkan adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("gos-vk-viewer-live"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        },
        None,
    ))
    .expect("request_device");

    let caps = surface.get_capabilities(&adapter);
    // Pick a non-sRGB format so the shader's manual gamma (pow 0.4545) matches
    // the PNG path (which uses Rgba8Unorm).
    let fmt = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
    let init = window.inner_size();
    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: fmt,
        width: init.width.max(1),
        height: init.height.max(1),
        present_mode: wgpu::PresentMode::Fifo, // vsync → smooth, no tearing
        desired_maximum_frame_latency: 2,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
    };
    surface.configure(&device, &config);
    let mut depth_view = make_depth_view(&device, config.width, config.height);

    // Orbit-camera state, seeded from VK_EYE. Mouse drag rotates (yaw/pitch),
    // wheel zooms (dist); a gentle idle spin runs when not dragging.
    let cam_base = env_vec3("VK_EYE", Vec3::new(6.5, 1.8, 9.5));
    let mut cam_dist = cam_base.length().max(2.0);
    let mut cam_yaw = cam_base.x.atan2(cam_base.z);
    let mut cam_pitch = (cam_base.y / cam_dist).asin();

    let ubuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("uniforms"),
        contents: bytemuck::bytes_of(&live_uniforms(config.width, config.height, cam_yaw, cam_pitch, cam_dist)),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        }],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bg"),
        layout: &bgl,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() }],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("lit"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&bgl], push_constant_ranges: &[] });
    let mesh_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
    let inst_attrs = wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4];
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("lit"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs",
            buffers: &[
                wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex3>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &mesh_attrs },
                wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Instance>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &inst_attrs },
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs",
            targets: &[Some(wgpu::ColorTargetState { format: fmt, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
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

    // ── B3b input-echo overlay ──────────────────────────────────────────
    // A second pipeline that draws flat quads directly in NDC space
    // (view_proj = IDENTITY, depth test = Always/no-write), independent of
    // the orbit camera. Reuses the main shader and bind-group layout.
    let overlay_uniforms = Uniforms { view_proj: Mat4::IDENTITY.to_cols_array_2d(), light_dir: [0.0, 0.0, 1.0, 0.0], camera_pos: [0.0, 0.0, 1.0, 1.0] };
    let overlay_ubuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("overlay-uniforms"),
        contents: bytemuck::bytes_of(&overlay_uniforms),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let overlay_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("overlay-bg"),
        layout: &bgl,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: overlay_ubuf.as_entire_binding() }],
    });
    let overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("overlay"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs",
            buffers: &[
                wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex3>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &mesh_attrs },
                wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Instance>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &inst_attrs },
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs",
            targets: &[Some(wgpu::ColorTargetState { format: fmt, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::Always,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    let (sv, si) = make_sphere(20, 28);
    let (cv, ci) = make_cylinder(14);
    let (qv, qi) = make_quad();
    let sphere_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("sv"), contents: bytemuck::cast_slice(&sv), usage: wgpu::BufferUsages::VERTEX });
    let sphere_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("si"), contents: bytemuck::cast_slice(&si), usage: wgpu::BufferUsages::INDEX });
    let cyl_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("cv"), contents: bytemuck::cast_slice(&cv), usage: wgpu::BufferUsages::VERTEX });
    let cyl_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("ci"), contents: bytemuck::cast_slice(&ci), usage: wgpu::BufferUsages::INDEX });
    let quad_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("qv"), contents: bytemuck::cast_slice(&qv), usage: wgpu::BufferUsages::VERTEX });
    let quad_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("qi"), contents: bytemuck::cast_slice(&qi), usage: wgpu::BufferUsages::INDEX });
    let si_len = si.len() as u32;
    let ci_len = ci.len() as u32;
    let qi_len = qi.len() as u32;

    let idle_spin = env_f32("VK_SPIN", 0.005); // gentle idle rotation; pauses while dragging
    let mut dragging = false;
    let mut last_cursor = (0.0f64, 0.0f64);
    // B3b input-echo overlay: ring buffer of recently-sent input bytes.
    let mut echo_log: VecDeque<u8> = VecDeque::with_capacity(ECHO_CAP);
    let mut fps_frames = 0u32;
    let mut fps_since = std::time::Instant::now();
    println!("gos-vk-viewer live: window up, reading @gos.vk from {addr} (close window to exit)");
    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(new) => {
                    config.width = new.width.max(1);
                    config.height = new.height.max(1);
                    surface.configure(&device, &config);
                    depth_view = make_depth_view(&device, config.width, config.height);
                    window.request_redraw();
                }
                WindowEvent::RedrawRequested => {
                    if !dragging {
                        cam_yaw += idle_spin; // gentle idle rotation when not interacting
                    }
                    queue.write_buffer(&ubuf, 0, bytemuck::bytes_of(&live_uniforms(config.width, config.height, cam_yaw, cam_pitch, cam_dist)));
                    let (spheres, ropes, bg) = shared
                        .lock()
                        .ok()
                        .and_then(|g| g.clone())
                        .unwrap_or_else(|| (Vec::new(), Vec::new(), [0.06, 0.06, 0.10]));
                    let sphere_inst = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("sphere_inst"), contents: bytemuck::cast_slice(&spheres), usage: wgpu::BufferUsages::VERTEX });
                    let rope_inst = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("rope_inst"), contents: bytemuck::cast_slice(&ropes), usage: wgpu::BufferUsages::VERTEX });

                    // B3b input-echo overlay: one tile per recently-sent input
                    // byte (green=printable, amber=Enter/Backspace/Esc,
                    // gray=other) plus a connection-status dot (green=B3b
                    // link up, red=down) — bottom-left HUD, in NDC space.
                    let connected = input_tx.lock().map(|g| g.is_some()).unwrap_or(false);
                    let mut overlay_inst: Vec<Instance> = echo_log
                        .iter()
                        .enumerate()
                        .map(|(i, &b)| {
                            let model = Mat4::from_translation(Vec3::new(-0.94 + i as f32 * 0.07, -0.92, 0.5)) * Mat4::from_scale(Vec3::splat(0.055));
                            let color = match b {
                                0x20..=0x7e => [0.25, 0.85, 0.35, 1.0],
                                b'\r' | 8 | 27 => [0.95, 0.65, 0.15, 1.0],
                                _ => [0.55, 0.55, 0.6, 1.0],
                            };
                            Instance { model: model.to_cols_array_2d(), color }
                        })
                        .collect();
                    let status_color = if connected { [0.2, 0.85, 0.3, 1.0] } else { [0.85, 0.2, 0.2, 1.0] };
                    let status_model = Mat4::from_translation(Vec3::new(-0.94, -0.84, 0.5)) * Mat4::from_scale(Vec3::splat(0.04));
                    overlay_inst.push(Instance { model: status_model.to_cols_array_2d(), color: status_color });
                    let overlay_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("overlay_inst"), contents: bytemuck::cast_slice(&overlay_inst), usage: wgpu::BufferUsages::VERTEX });
                    let frame_tex = match surface.get_current_texture() {
                        Ok(t) => t,
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            surface.configure(&device, &config);
                            return;
                        }
                        Err(wgpu::SurfaceError::Timeout) => return,
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            eprintln!("[viewer] surface out of memory; exiting");
                            elwt.exit();
                            return;
                        }
                    };
                    let view = frame_tex.texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                    {
                        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: bg[0], g: bg[1], b: bg[2], a: 1.0 }), store: wgpu::StoreOp::Store },
                            })],
                            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                                view: &depth_view,
                                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                                stencil_ops: None,
                            }),
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });
                        rp.set_pipeline(&pipeline);
                        rp.set_bind_group(0, &bind_group, &[]);
                        if !ropes.is_empty() {
                            rp.set_vertex_buffer(0, cyl_vb.slice(..));
                            rp.set_vertex_buffer(1, rope_inst.slice(..));
                            rp.set_index_buffer(cyl_ib.slice(..), wgpu::IndexFormat::Uint16);
                            rp.draw_indexed(0..ci_len, 0, 0..ropes.len() as u32);
                        }
                        if !spheres.is_empty() {
                            rp.set_vertex_buffer(0, sphere_vb.slice(..));
                            rp.set_vertex_buffer(1, sphere_inst.slice(..));
                            rp.set_index_buffer(sphere_ib.slice(..), wgpu::IndexFormat::Uint16);
                            rp.draw_indexed(0..si_len, 0, 0..spheres.len() as u32);
                        }
                        rp.set_pipeline(&overlay_pipeline);
                        rp.set_bind_group(0, &overlay_bind_group, &[]);
                        rp.set_vertex_buffer(0, quad_vb.slice(..));
                        rp.set_vertex_buffer(1, overlay_buf.slice(..));
                        rp.set_index_buffer(quad_ib.slice(..), wgpu::IndexFormat::Uint16);
                        rp.draw_indexed(0..qi_len, 0, 0..overlay_inst.len() as u32);
                    }
                    queue.submit([enc.finish()]);
                    frame_tex.present();
                    fps_frames += 1;
                    if fps_since.elapsed().as_secs_f32() >= 1.0 {
                        eprintln!("[viewer] render {fps_frames} fps");
                        fps_frames = 0;
                        fps_since = std::time::Instant::now();
                    }
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if button == winit::event::MouseButton::Left {
                        dragging = state == winit::event::ElementState::Pressed;
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    if dragging {
                        let dx = (position.x - last_cursor.0) as f32;
                        let dy = (position.y - last_cursor.1) as f32;
                        cam_yaw -= dx * 0.01;
                        cam_pitch = (cam_pitch + dy * 0.01).clamp(-1.5, 1.5);
                    }
                    last_cursor = (position.x, position.y);
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let s = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                        winit::event::MouseScrollDelta::PixelDelta(p) => (p.y as f32) * 0.02,
                    };
                    cam_dist = (cam_dist * (1.0 - s * 0.1)).clamp(2.0, 60.0);
                }
                WindowEvent::KeyboardInput { event: kev, .. } => {
                    // B3b: forward keypresses back to the kernel over the live
                    // socket. The kernel echoes them to its boot serial and
                    // feeds the key queue, so typing here drives the OS.
                    use winit::keyboard::{Key, NamedKey};
                    if kev.state == winit::event::ElementState::Pressed {
                        let byte: Option<u8> = match &kev.logical_key {
                            Key::Named(NamedKey::Enter) => Some(b'\r'),
                            Key::Named(NamedKey::Backspace) => Some(8),
                            Key::Named(NamedKey::Escape) => Some(27),
                            Key::Named(NamedKey::Space) => Some(b' '),
                            Key::Character(s) => s.bytes().next(),
                            _ => None,
                        };
                        if let Some(b) = byte {
                            let mut sent = false;
                            if let Ok(mut g) = input_tx.lock() {
                                if let Some(s) = g.as_mut() {
                                    match std::io::Write::write(s, &[b]) {
                                        Ok(_) => sent = true,
                                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                                        Err(_) => *g = None, // disconnected
                                    }
                                }
                            }
                            // On-screen echo: mirrors the kernel's COM1
                            // `vk-input:` log so a human watching the live
                            // window (no serial console open) can confirm
                            // the B3b round trip without leaving the GUI.
                            if sent {
                                if echo_log.len() == ECHO_CAP {
                                    echo_log.pop_front();
                                }
                                echo_log.push_back(b);
                            }
                        }
                    }
                }
                _ => {}
            },
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        })
        .expect("event loop");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Live host-GPU window: `gos-vk-viewer --live [addr]` (default 127.0.0.1:14445).
    if let Some(i) = args.iter().position(|a| a == "--live") {
        let addr = args.get(i + 1).filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "127.0.0.1:14445".to_string());
        run_live(&addr);
        return;
    }

    let (inp, outp) = match args.iter().position(|a| a == "--render") {
        Some(i) if args.len() > i + 2 => (args[i + 1].clone(), args[i + 2].clone()),
        _ => {
            eprintln!("usage: gos-vk-viewer --render <in.bin> <out.png>   [env: VK_LAYOUT=rings|force VK_EYE=x,y,z VK_FOV=deg]");
            eprintln!("       gos-vk-viewer --live [host:port]            (live host-GPU window; default 127.0.0.1:14445)");
            std::process::exit(2);
        }
    };
    let bytes = std::fs::read(&inp).expect("read dump");
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let frame = parse_last_frame(&text).expect("no @gos.vk frame in input");
    let (spheres, ropes) = build_scene(&frame);
    render3d_to_png(&frame, &spheres, &ropes, &outp);
    println!("3D {} spheres / {} ropes ({}x{}) via Vulkan -> {}", spheres.len(), ropes.len() / 2, frame.w, frame.h, outp);
}
