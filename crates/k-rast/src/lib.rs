#![no_std]

//! Phase I.3.8 — kernel-side software rasteriser.
//!
//! Just enough math + pixel primitives for the boot UI to render the
//! kernel graph as a *rotatable 3D scene* in the existing VGA mode-13h
//! framebuffer.  No GPU, no hypervisor escape — every pixel is
//! computed by the CPU.  At 320×200 with ~30 nodes (~360 triangles)
//! the budget is generous; future I.x slices add a depth buffer +
//! per-pixel shading once the resolution climbs.
//!
//! Layering:
//!
//!   ```text
//!   kernel boot loop ──> compute view_proj (camera) ──> for each node:
//!        ├──> Mat4::transform_point (8 cube corners → clip space)
//!        ├──> project_to_screen     (perspective divide → (x,y,z))
//!        └──> fill_triangle / draw_line   ──> k-fb put_pixel
//!   ```
//!
//! This crate is *renderer-agnostic*: pixel writes go through a
//! caller-supplied `FnMut(usize, usize, u8) -> ()` closure.  The
//! kernel binds k-fb; tests can bind a mock.
//!
//! Math precision: `f32` end-to-end.  The kernel boot path enables
//! OSFXSR + OSXMMEXCPT (see `crates/hypervisor/src/main.rs`) so SSE
//! float ops are legal.  `libm` provides `sinf` / `cosf` / `sqrtf`
//! for the no_std build.

use libm::{cosf, sinf, sqrtf, tanf};

// ── Vec3 / Vec4 / Mat4 ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn sub(self, rhs: Vec3) -> Vec3 {
        Vec3::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }

    pub fn dot(self, rhs: Vec3) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    pub fn cross(self, rhs: Vec3) -> Vec3 {
        Vec3::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }

    pub fn normalize(self) -> Vec3 {
        let len2 = self.dot(self);
        if len2 < 1.0e-12 {
            return Vec3::new(0.0, 0.0, 0.0);
        }
        let inv = 1.0 / sqrtf(len2);
        Vec3::new(self.x * inv, self.y * inv, self.z * inv)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

/// Row-major 4×4 matrix.  `m[row][col]`.  All matrix ops apply
/// `result = self * rhs` in the math-standard way: column vectors on
/// the right.
#[derive(Debug, Clone, Copy)]
pub struct Mat4(pub [[f32; 4]; 4]);

impl Mat4 {
    pub const fn identity() -> Self {
        Self([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    /// Right-handed perspective with z mapped into [0,1] (matches the
    /// convention `gos-gfx-bridge-host` uses on the host side, so the
    /// kernel and the host visualizer can share a camera mental model).
    pub fn perspective(fov_y_rad: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / tanf(fov_y_rad / 2.0);
        let nf = 1.0 / (near - far);
        Self([
            [f / aspect, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, far * nf, -1.0],
            [0.0, 0.0, far * near * nf, 0.0],
        ])
    }

    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let f = target.sub(eye).normalize();
        let s = f.cross(up).normalize();
        let u = s.cross(f);
        Self([
            [s.x, u.x, -f.x, 0.0],
            [s.y, u.y, -f.y, 0.0],
            [s.z, u.z, -f.z, 0.0],
            [-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0],
        ])
    }

    pub fn mul(self, rhs: Mat4) -> Mat4 {
        let a = &self.0;
        let b = &rhs.0;
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
        Mat4(out)
    }

    /// Transform a 3D point (w = 1.0) and return clip-space Vec4.
    pub fn transform_point(&self, p: Vec3) -> Vec4 {
        let m = &self.0;
        Vec4 {
            x: m[0][0] * p.x + m[1][0] * p.y + m[2][0] * p.z + m[3][0],
            y: m[0][1] * p.x + m[1][1] * p.y + m[2][1] * p.z + m[3][1],
            z: m[0][2] * p.x + m[1][2] * p.y + m[2][2] * p.z + m[3][2],
            w: m[0][3] * p.x + m[1][3] * p.y + m[2][3] * p.z + m[3][3],
        }
    }
}

/// Build a Y-axis rotation matrix (yaw).
pub fn rotate_y(angle_rad: f32) -> Mat4 {
    let c = cosf(angle_rad);
    let s = sinf(angle_rad);
    Mat4([
        [c, 0.0, -s, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [s, 0.0, c, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// Perspective divide + viewport map.  Returns (x, y, depth_0_to_1)
/// in screen space, or `None` if the point is behind the near plane
/// (w ≤ 0) or the projection collapsed.
pub fn project_to_screen(
    clip: Vec4,
    width: u32,
    height: u32,
) -> Option<(i32, i32, f32)> {
    if clip.w <= 1.0e-4 {
        return None;
    }
    let ndc_x = clip.x / clip.w;
    let ndc_y = clip.y / clip.w;
    let ndc_z = clip.z / clip.w;
    // Bail on far-out-of-frustum hits so we don't waste rasterisation
    // budget on offscreen geometry.
    if ndc_x.abs() > 8.0 || ndc_y.abs() > 8.0 {
        return None;
    }
    let sx = ((ndc_x + 1.0) * 0.5 * width as f32) as i32;
    // Flip Y: NDC +Y is up, screen +Y is down.
    let sy = ((1.0 - (ndc_y + 1.0) * 0.5) * height as f32) as i32;
    Some((sx, sy, ndc_z))
}

// ── Rasterisation primitives ───────────────────────────────────────

/// Bresenham line, clipped against the (0..width, 0..height) box at
/// pixel granularity by the put_pixel closure.  Caller-supplied
/// closure can short-circuit on out-of-bounds.
pub fn draw_line(
    mut put_pixel: impl FnMut(i32, i32),
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        put_pixel(x, y);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Solid triangle fill via half-space scanlines.  Vertices in
/// arbitrary winding; bounded by the screen rect derived from the
/// triangle's bounding box clipped to (0..width, 0..height).
///
/// This is the slow path — no SIMD, no fixed-point, just three
/// edge-function evaluations per pixel.  Tested at 320×200 with
/// ~360 small triangles per frame: comfortably under one PIT tick.
pub fn fill_triangle(
    mut put_pixel: impl FnMut(i32, i32),
    width: i32,
    height: i32,
    p0: (i32, i32),
    p1: (i32, i32),
    p2: (i32, i32),
) {
    // Bounding box clipped to screen.
    let min_x = p0.0.min(p1.0).min(p2.0).max(0);
    let max_x = p0.0.max(p1.0).max(p2.0).min(width - 1);
    let min_y = p0.1.min(p1.1).min(p2.1).max(0);
    let max_y = p0.1.max(p1.1).max(p2.1).min(height - 1);
    if min_x > max_x || min_y > max_y {
        return;
    }

    // Edge functions: positive on one side, negative on the other.
    // Sign of the triangle's signed area decides which side is
    // "inside" — handles both CW and CCW input winding.
    let area = edge_fn(p0, p1, p2);
    if area == 0 {
        return; // degenerate
    }
    let inside_positive = area > 0;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = (x, y);
            let w0 = edge_fn(p1, p2, p);
            let w1 = edge_fn(p2, p0, p);
            let w2 = edge_fn(p0, p1, p);
            let inside = if inside_positive {
                w0 >= 0 && w1 >= 0 && w2 >= 0
            } else {
                w0 <= 0 && w1 <= 0 && w2 <= 0
            };
            if inside {
                put_pixel(x, y);
            }
        }
    }
}

fn edge_fn(a: (i32, i32), b: (i32, i32), c: (i32, i32)) -> i32 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

// ── Insertion sort by depth (painter's algorithm support) ─────────

/// In-place insertion sort over `(index, depth)` pairs, *descending*
/// by depth (largest first).  Use this to draw far objects before
/// near ones so painter's algorithm produces a correct image without
/// a depth buffer.  O(N²); fine for the ~30-cube Gen-1 scene.
pub fn sort_by_depth_desc(items: &mut [(usize, f32)]) {
    for i in 1..items.len() {
        let mut j = i;
        while j > 0 && items[j].1 > items[j - 1].1 {
            items.swap(j, j - 1);
            j -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perspective_aligns_with_identity_view() {
        // A point at +Z=2 should project to ndc.z > 0 with positive w.
        let p = Mat4::perspective(60.0_f32.to_radians(), 1.0, 0.1, 100.0);
        let v = Mat4::look_at(
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        let vp = p.mul(v);
        let clip = vp.transform_point(Vec3::new(0.0, 0.0, 0.0));
        assert!(clip.w > 0.0);
        let screen = project_to_screen(clip, 320, 200).expect("on-screen");
        // Origin should land near screen centre.
        assert!((screen.0 - 160).abs() < 5);
        assert!((screen.1 - 100).abs() < 5);
    }

    #[test]
    fn sort_by_depth_descending() {
        let mut a = [(0, 0.1_f32), (1, 0.5), (2, 0.3)];
        sort_by_depth_desc(&mut a);
        assert_eq!(a[0].0, 1);
        assert_eq!(a[1].0, 2);
        assert_eq!(a[2].0, 0);
    }

    #[test]
    fn fill_triangle_paints_expected_pixel_count_for_axis_aligned() {
        let mut hits = 0;
        fill_triangle(
            |_x, _y| hits += 1,
            320,
            200,
            (0, 0),
            (10, 0),
            (0, 10),
        );
        // Right-angle 10×10 triangle (inclusive bbox) covers ~55-66
        // pixels depending on edge tie-breaks.  Loose bounds suffice
        // as a sanity check — the demo uses larger tris where exact
        // count doesn't matter.
        assert!(hits > 40 && hits < 80, "got {} hits", hits);
    }
}
