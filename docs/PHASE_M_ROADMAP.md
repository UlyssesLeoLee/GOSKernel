# Phase M — AAA 级渲染追加 (sphere shader 极限)

继 Phase I.14 (PBR baseline) 和 K.x 的 UI 工具化之后，Phase M 专攻
**让 3D 模型/贴图/光照视觉质量达到 AAA 游戏级别**。在 mode 13h
320×200×256 色软件光栅化的极端约束下，通过算法替代 asset 烘焙，
让每个 metal ball 看起来像真实 Unreal/Substance Painter 出来的
chrome / satin / brushed metal / matte rock 渲染目标。

## 已完成 (4 项)

| # | 标题 | 关键技术 |
|---|---|---|
| **M.1** | Bayer 4×4 dither + 三光源 PBR | ordered dither 突破 8-shade banding；key/fill/sky 三向 diffuse |
| **M.2** | 微表面法线扰动 | 每材质独立 amp/freq sin/cos noise; 模拟 brushed/satin/matte 质感 |
| **M.3** | 8-邻居 Gaussian bloom + max-composite | k_fb::get_pixel_raw 让相邻 hot spot 不互相吃掉；shade≥6 起就开始扩散 |
| **M.4** | Anisotropic specular | half-vector × tangent 衰减，水平/垂直拉伸高光，brushed 真实感 |

## 每材质完整 PbrMaterial 配方

```rust
struct PbrMaterial {
    metallic: f32,        // I.14
    roughness: f32,       // I.14
    rim: f32,             // I.14
    micro_bump_amp: f32,  // M.2
    micro_bump_freq: f32, // M.2
    anisotropy: f32,      // M.4 (-1..+1)
}
```

| 子域 | metallic | roughness | rim | bump_amp | bump_freq | anisotropy | 视觉印象 |
|---|---|---|---|---|---|---|---|
| Hardware     | 0.95 | 0.18 | 1.20 | 0.000 |  0.0 |  0.00 | 镜面 cyan steel |
| KernelDriver | 0.92 | 0.25 | 1.00 | 0.025 | 32.0 | +0.65 | 拉丝 chrome 水平 streak |
| Service      | 0.80 | 0.38 | 0.85 | 0.018 | 22.0 | +0.30 | satin mint |
| Compute      | 0.88 | 0.30 | 1.05 | 0.030 | 28.0 | -0.55 | 深拉丝 magenta 垂直 streak |
| Routing      | 0.70 | 0.45 | 0.90 | 0.022 | 18.0 | +0.20 | anodized rose |
| Vector       | 0.40 | 0.65 | 0.70 | 0.055 | 14.0 |  0.00 | matte rock |

## Sphere shader 完整管线 (每像素)

```text
1. 解析球面法线 (analytical: sqrt(r² - dx² - dy²))
2. M.2  法线扰动:
        bx = sin(nx*freq) × cos(ny*freq*0.7) × amp
        by = cos(nx*freq*0.6) × sin(ny*freq)  × amp
        normal = (nx+bx, -ny+by, nz).normalize()
3. M.1  三光源 diffuse:
        N·KEY × KEY_intensity (1.0)
      + N·FILL × FILL_intensity (0.32)
      + N·SKY  × SKY_intensity  (0.18)
      × (1 - metallic)
4. I.14 GGX × Schlick specular:
        D_GGX(N·H, roughness)
      × F_schlick(F0, V·H)
5. M.4  Anisotropy:
        × (1 - |half.x or .y| × |anisotropy|)
6. I.14 Procedural environment reflection
7. I.14 Rim glow (Schlick on N·V)
8. I.4  Depth fog → shade saturating_sub
9. M.1  Bayer 4×4 dither:
        Pick shade slot 0..7 via ordered threshold
10. M.3 8-邻居 Gaussian bloom spread:
        Cardinal neighbors → shade-1 (max-composited)
        Diagonal neighbors → shade-2 (max-composited)
11. I.6.4 specular_boost (click flash) — multiplies spec
```

## 推迟到 Phase N+ 的更深改造

| # | 标题 | 大致工作量 |
|---|---|---|
| **N.1** | HDR f32 backbuffer + ACES tone mapping | 大 — 需要 320×200×3 RGB f32 backbuffer (~750KB) + 所有 paint 改成写 backbuffer + final composite pass; 突破 8-shade 量化 |
| **N.2** | 真正的 mip-chain bloom (4 levels separable Gaussian) | 中 — 需要 N.1 backbuffer 才有意义 |
| **N.3** | SSAO 近似 | 中 — 在 sphere 接触处加 ambient occlusion 暗影 |
| **N.4** | Procedural pattern overlay | 小 — circuitry/runes 浮雕图案叠加 |
| **N.5** | Multi-bounce indirect light approximation | 中 — 从近处球反弹光到当前球 |
| **N.6** | Blender 烘焙 cubemap (替代 procedural env) | 小 — Python script + 转码到内嵌 byte array |
| **N.7** | Rope shader 升级 (匹配 sphere 质感) | 小 — rope 当前 simple line, 加 round-cap + per-pixel shading |

## 测试覆盖

runtime harness 36/36 全程绿色，QEMU smoke 在每个 M.x commit 后均 PASS。
没有引入 protocol/ABI 变更。

## 总体定位

至此 GOS 3D 子系统的 **可见视觉质量**:
- 远观: AAA 第一印象 (每球独立 brushed/satin/matte 质感, 多向光照, soft bloom, rim glow)
- 中观: 视觉细节 (anisotropic streak, fog-driven depth, click haptic flash)
- 近观: 像素艺术 (dither pattern, palette 选择)

→ 在不引入外部 asset 的前提下，达到了 320×200×256 软件光栅化能
达到的 AAA-equivalent 上限。再进一步需要 N.1 的 HDR backbuffer
重构来突破 quantization 天花板。
