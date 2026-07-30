#![no_std]

//! GOS 内核预烘焙资产 — 由 `tools/assets/generate.py` 离线
//! ray-trace 渲染、`tools/assets/pack_palette.py` 量化到 k-fb 调色板，
//! 然后这里通过 `include_bytes!` 嵌入内核 binary.
//!
//! 文件格式 (.pal):
//!   bytes 0..2   magic 'GA' (0x47 0x41)
//!   bytes 2..3   width  (u8)
//!   bytes 3..4   height (u8)
//!   bytes 4..    palette indices (u8 per pixel, row-major)
//!
//! 调用方:
//!   * sphere LOD blit (远景球替换 per-pixel shader)
//!   * env cubemap 采样 (替换 procedural sample_environment)
//!   * rope texture 沿 segment 采样 (braided cable look)
//!   * BRDF LUT pre-integrated split-sum

/// 一个 baked palette-indexed 纹理.  零拷贝引用编译进 binary 的字节.
#[derive(Debug, Clone, Copy)]
pub struct PalTexture {
    pub width: u8,
    pub height: u8,
    /// `width * height` bytes, row-major.  Each byte is a palette
    /// index into the k-fb 256-color table.
    pub data: &'static [u8],
}

impl PalTexture {
    /// 在编译期切片解析 .pal blob: 前 4 字节是头, 余下是数据.
    /// `width()` / `height()` 在常量上下文中也可用.
    pub const fn from_blob(blob: &'static [u8]) -> Self {
        debug_assert!(blob.len() >= 4);
        debug_assert!(blob[0] == b'G' && blob[1] == b'A', "not a .pal blob");
        let w = blob[2];
        let h = blob[3];
        // SAFETY: width * height is computed from blob header which
        // is verified above to be valid; data slice extracted by
        // splitting at offset 4.
        let (_, data) = blob.split_at(4);
        Self { width: w, height: h, data }
    }

    /// 像素采样 (x, y).  超界返回 0 (Background palette index).
    pub const fn sample(&self, x: u8, y: u8) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        self.data[idx]
    }
}

// ── Sphere LOD 参考渲染 (6 子域 × 2 尺寸 = 12 纹理) ─────────────

macro_rules! sphere_tex {
    ($name:ident, $file:literal) => {
        pub static $name: PalTexture =
            PalTexture::from_blob(include_bytes!(concat!("../../../assets/", $file)));
    };
}

sphere_tex!(SPHERE_HARDWARE_64,     "sphere_Hardware_64.pal");
sphere_tex!(SPHERE_HARDWARE_32,     "sphere_Hardware_32.pal");
sphere_tex!(SPHERE_KERNELDRIVER_64, "sphere_KernelDriver_64.pal");
sphere_tex!(SPHERE_KERNELDRIVER_32, "sphere_KernelDriver_32.pal");
sphere_tex!(SPHERE_SERVICE_64,      "sphere_Service_64.pal");
sphere_tex!(SPHERE_SERVICE_32,      "sphere_Service_32.pal");
sphere_tex!(SPHERE_COMPUTE_64,      "sphere_Compute_64.pal");
sphere_tex!(SPHERE_COMPUTE_32,      "sphere_Compute_32.pal");
sphere_tex!(SPHERE_ROUTING_64,      "sphere_Routing_64.pal");
sphere_tex!(SPHERE_ROUTING_32,      "sphere_Routing_32.pal");
sphere_tex!(SPHERE_VECTOR_64,       "sphere_Vector_64.pal");
sphere_tex!(SPHERE_VECTOR_32,       "sphere_Vector_32.pal");

// ── Environment cubemap (6 faces × 32×32) ──────────────────────────

pub static ENV_POS_X: PalTexture = PalTexture::from_blob(include_bytes!("../../../assets/env_posX.pal"));
pub static ENV_NEG_X: PalTexture = PalTexture::from_blob(include_bytes!("../../../assets/env_negX.pal"));
pub static ENV_POS_Y: PalTexture = PalTexture::from_blob(include_bytes!("../../../assets/env_posY.pal"));
pub static ENV_NEG_Y: PalTexture = PalTexture::from_blob(include_bytes!("../../../assets/env_negY.pal"));
pub static ENV_POS_Z: PalTexture = PalTexture::from_blob(include_bytes!("../../../assets/env_posZ.pal"));
pub static ENV_NEG_Z: PalTexture = PalTexture::from_blob(include_bytes!("../../../assets/env_negZ.pal"));

// ── Rope braided cable strip (128×8) ───────────────────────────────

pub static ROPE_BRAIDED: PalTexture =
    PalTexture::from_blob(include_bytes!("../../../assets/rope_braided.pal"));

// ── BRDF split-sum LUT (128×128, R = scale, G = bias) ─────────────

pub static BRDF_LUT: PalTexture =
    PalTexture::from_blob(include_bytes!("../../../assets/brdf_lut.pal"));

// ── N.11 — anti-aliased font atlases (replacing legacy 8×8 bitmap) ──
//
// Pre-rendered offline by `tools/assets/bake_font.py` from
// system-installed TrueType / OpenType fonts.  Each atlas is an
// 8-bit grayscale alpha bitmap on a fixed-cell grid indexed by
// codepoint - `char_first`.  Both base fonts ship under SIL OFL
// and align baselines so a CJK extension atlas drops in pixel-perfect.
//
//   FONT_UI_14    Noto Sans SC 14 px    — body labels / chat
//   FONT_UI_18    Noto Sans SC 18 px    — header brand / callouts
//   FONT_MONO_13  Cascadia Code 13 px   — command line / fixed-width

/// A baked TTF-rendered alpha atlas.
#[derive(Debug, Clone, Copy)]
pub struct FontAtlas {
    pub cell_w: u8,
    pub cell_h: u8,
    pub char_first: u8,
    pub char_count: u8,
    /// Atlas bitmap width in pixels.  cells_per_row = `atlas_w / cell_w`.
    pub atlas_w: u16,
    /// Row-major 8-bit alpha; total bytes = `atlas_w * atlas_h`.
    pub alpha: &'static [u8],
}

impl FontAtlas {
    pub const fn from_blob(blob: &'static [u8]) -> Self {
        debug_assert!(blob.len() >= 8);
        debug_assert!(blob[0] == b'F' && blob[1] == b'A', "not a .fnt blob");
        let cell_w = blob[2];
        let cell_h = blob[3];
        let char_first = blob[4];
        let char_count = blob[5];
        let atlas_w = (blob[6] as u16) | ((blob[7] as u16) << 8);
        let (_, alpha) = blob.split_at(8);
        Self { cell_w, cell_h, char_first, char_count, atlas_w, alpha }
    }

    /// Look up the alpha rect for a single codepoint, or None if outside range.
    pub const fn glyph(&self, ch: u8) -> Option<GlyphRect> {
        if ch < self.char_first { return None; }
        let idx = ch - self.char_first;
        if idx >= self.char_count { return None; }
        let cells_per_row = (self.atlas_w / self.cell_w as u16) as u8;
        let col = idx % cells_per_row;
        let row = idx / cells_per_row;
        Some(GlyphRect {
            atlas_x: col as u16 * self.cell_w as u16,
            atlas_y: row as u16 * self.cell_h as u16,
            w: self.cell_w,
            h: self.cell_h,
        })
    }

    /// Read one alpha pixel.  Returns 0 if (x, y) is out of bounds.
    pub const fn alpha_at(&self, x: u16, y: u16) -> u8 {
        let idx = (y as usize) * (self.atlas_w as usize) + (x as usize);
        if idx >= self.alpha.len() { 0 } else { self.alpha[idx] }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphRect {
    pub atlas_x: u16,
    pub atlas_y: u16,
    pub w: u8,
    pub h: u8,
}

pub static FONT_UI_14: FontAtlas =
    FontAtlas::from_blob(include_bytes!("../../../assets/font_ui_14.fnt"));
pub static FONT_UI_18: FontAtlas =
    FontAtlas::from_blob(include_bytes!("../../../assets/font_ui_18.fnt"));
pub static FONT_MONO_13: FontAtlas =
    FontAtlas::from_blob(include_bytes!("../../../assets/font_mono_13.fnt"));

// ── 按子域索引的 sphere LOD 选择器 ─────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum SphereClass {
    Hardware,
    KernelDriver,
    Service,
    Compute,
    Routing,
    Vector,
}

/// 按子域 + 期望尺寸 (px) 挑最近的 baked sphere LOD.  Heuristic:
/// r_px ≥ 48 → 64-tex, 否则 32-tex.
pub const fn sphere_for(class: SphereClass, target_radius_px: u8) -> &'static PalTexture {
    let big = target_radius_px >= 24; // ≥ ~48 px diameter
    match (class, big) {
        (SphereClass::Hardware,     true)  => &SPHERE_HARDWARE_64,
        (SphereClass::Hardware,     false) => &SPHERE_HARDWARE_32,
        (SphereClass::KernelDriver, true)  => &SPHERE_KERNELDRIVER_64,
        (SphereClass::KernelDriver, false) => &SPHERE_KERNELDRIVER_32,
        (SphereClass::Service,      true)  => &SPHERE_SERVICE_64,
        (SphereClass::Service,      false) => &SPHERE_SERVICE_32,
        (SphereClass::Compute,      true)  => &SPHERE_COMPUTE_64,
        (SphereClass::Compute,      false) => &SPHERE_COMPUTE_32,
        (SphereClass::Routing,      true)  => &SPHERE_ROUTING_64,
        (SphereClass::Routing,      false) => &SPHERE_ROUTING_32,
        (SphereClass::Vector,       true)  => &SPHERE_VECTOR_64,
        (SphereClass::Vector,       false) => &SPHERE_VECTOR_32,
    }
}

/// 给定一个 reflection direction (单位向量), 采样 cubemap.
/// 选择 |分量| 最大的轴作为 face, 投影 u,v 到该 face.
pub fn sample_cubemap(reflection: [f32; 3]) -> u8 {
    let [rx, ry, rz] = reflection;
    let ax = libm_abs(rx);
    let ay = libm_abs(ry);
    let az = libm_abs(rz);
    let (face, u, v, denom): (&PalTexture, f32, f32, f32) = if ax >= ay && ax >= az {
        if rx > 0.0 {
            (&ENV_POS_X, -rz, -ry, ax)   // +X
        } else {
            (&ENV_NEG_X, rz, -ry, ax)    // -X
        }
    } else if ay >= ax && ay >= az {
        if ry > 0.0 {
            (&ENV_POS_Y, rx, rz, ay)     // +Y
        } else {
            (&ENV_NEG_Y, rx, -rz, ay)    // -Y
        }
    } else {
        if rz > 0.0 {
            (&ENV_POS_Z, rx, -ry, az)    // +Z
        } else {
            (&ENV_NEG_Z, -rx, -ry, az)   // -Z
        }
    };
    let s = (u / denom * 0.5 + 0.5).clamp(0.0, 0.999);
    let t = (v / denom * 0.5 + 0.5).clamp(0.0, 0.999);
    let px = (s * face.width as f32) as u8;
    let py = (t * face.height as f32) as u8;
    face.sample(px, py)
}

#[inline]
fn libm_abs(x: f32) -> f32 {
    if x < 0.0 { -x } else { x }
}
