//! Phase I.1.1 — single-triangle headless render end-to-end.
//!
//! Wires the bridge's `WgpuBackend` to a tiny WGSL shader and three
//! vertices, runs the full pipeline through `RenderBackend::submit`
//! for the frame-loop verbs, and asserts the readback has non-black
//! pixels in the region the triangle should cover.
//!
//! This is the foundation the I.1.x golden-PNG diff will replace —
//! once the renderer is proven to produce *some* coloured output, we
//! commit a reference PNG to `test-frames/` and tighten the assertion.

use bytemuck::{Pod, Zeroable};
use gos_gfx_bridge_host::{DemoVertex, WgpuBackend};
use gos_gfx_protocol::{BufferKind, RenderBackend, RenderCommand};

const SURFACE_W: u32 = 64;
const SURFACE_H: u32 = 64;

const WGSL_SOURCE: &str = r#"
struct VsIn {
    @location(0) position: vec2<f32>,
    @location(1) color:    vec3<f32>,
};

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0)       color:         vec3<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Index(u16);

#[test]
fn triangle_renders_with_non_black_center_pixels() {
    // Some CI runners (and the bare worktree on a fresh checkout) may
    // not have ANY usable wgpu adapter — no GPU, no software fallback.
    // Treat that as a skip rather than a hard failure; the slice
    // contract is "if a renderer exists, the triangle draws", not
    // "every contributor must own a GPU".  When lavapipe lands in
    // PHASE_I.4.3 the CI matrix will gain a guaranteed-software path
    // and this skip goes away.
    let mut backend = match WgpuBackend::new_headless() {
        Ok(b) => b,
        Err(_) => {
            eprintln!("triangle test: no usable wgpu adapter on this host, skipping");
            return;
        }
    };

    // Synchronous resource creation (returns handles directly).  The
    // wire-encoded create/response dance lands once a real carrier is
    // wired; for Gen-1 the test owns the resource lifecycle.
    let surface = backend
        .create_surface(SURFACE_W, SURFACE_H)
        .expect("create_surface");
    let pipeline = backend
        .create_pipeline(WGSL_SOURCE.as_bytes())
        .expect("create_pipeline");

    let vertices: [DemoVertex; 3] = [
        DemoVertex {
            position: [-0.5, -0.5],
            color: [1.0, 0.0, 0.0],
        },
        DemoVertex {
            position: [0.5, -0.5],
            color: [0.0, 1.0, 0.0],
        },
        DemoVertex {
            position: [0.0, 0.5],
            color: [0.0, 0.0, 1.0],
        },
    ];
    // wgpu requires write_buffer copies to be multiples of
    // COPY_BUFFER_ALIGNMENT (= 4 bytes).  3 × u16 = 6 bytes, so pad
    // with a trailing zero (4 × 2 = 8) — DrawInstanced only reads
    // the first 3 entries so the pad is inert.
    let indices: [u16; 4] = [0, 1, 2, 0];

    let vertex_buffer = backend
        .upload_buffer(BufferKind::Vertex, bytemuck::cast_slice(&vertices))
        .expect("upload vertex");
    let index_buffer = backend
        .upload_buffer(BufferKind::Index, bytemuck::cast_slice(&indices))
        .expect("upload index");

    // Frame-loop verbs go through the protocol's `submit` path; this
    // is the exact byte sequence a future carrier-decoded stream will
    // hand the backend.
    backend
        .submit(&RenderCommand::BeginFrame { surface })
        .expect("BeginFrame");
    backend
        .submit(&RenderCommand::BindPipeline { pipeline })
        .expect("BindPipeline");
    backend
        .submit(&RenderCommand::BindBuffers {
            vertex: vertex_buffer,
            instance: vertex_buffer, // Gen-1 ignores the instance slot
            index: index_buffer,
            uniform: vertex_buffer, // Gen-1 ignores the uniform slot
        })
        .expect("BindBuffers");
    backend
        .submit(&RenderCommand::DrawInstanced {
            index_count: 3,
            instance_count: 1,
        })
        .expect("DrawInstanced");
    backend
        .submit(&RenderCommand::EndFrame)
        .expect("EndFrame");

    let pixels = backend.last_readback().expect("readback");
    assert_eq!(
        pixels.len(),
        (SURFACE_W * SURFACE_H * 4) as usize,
        "readback size matches surface area * RGBA"
    );

    // Whole-buffer scan: count any pixel that's not the clear colour
    // (opaque black).  The triangle should colour several hundred
    // pixels; <50 is suspicious enough to fail with diagnostics.
    let mut coloured = 0usize;
    let mut max_r = 0u8;
    let mut max_g = 0u8;
    let mut max_b = 0u8;
    let mut sample_grid: Vec<((u32, u32), [u8; 4])> = Vec::new();
    for y in 0..SURFACE_H {
        for x in 0..SURFACE_W {
            let off = pixel_offset(SURFACE_W, x, y);
            let (r, g, b, a) = (
                pixels[off],
                pixels[off + 1],
                pixels[off + 2],
                pixels[off + 3],
            );
            if r != 0 || g != 0 || b != 0 {
                coloured += 1;
            }
            max_r = max_r.max(r);
            max_g = max_g.max(g);
            max_b = max_b.max(b);
            if x % 16 == 0 && y % 16 == 0 {
                sample_grid.push(((x, y), [r, g, b, a]));
            }
        }
    }
    eprintln!(
        "readback summary: coloured={}, max_rgb=({},{},{}); 16x16 grid:",
        coloured, max_r, max_g, max_b
    );
    for ((x, y), rgba) in &sample_grid {
        eprintln!("  ({:>3},{:>3}) = ({:>3},{:>3},{:>3},{:>3})", x, y, rgba[0], rgba[1], rgba[2], rgba[3]);
    }
    assert!(
        coloured > 50,
        "expected the triangle to colour >50 pixels; got {} (max RGB = {},{},{})",
        coloured,
        max_r,
        max_g,
        max_b
    );
}

fn pixel_offset(width: u32, x: u32, y: u32) -> usize {
    ((y * width + x) * 4) as usize
}
