# GOS 3D 资产生成管线

为 GOS 内核生成"模型 + 贴图"资产的离线工具集。当前不需要 Blender；
所有内容用纯 Python 标准库（无 PIL/numpy 依赖）以 ray-tracing 算法
烘焙，输出 PPM。`pack_palette.py` 再把 PPM 转成 GOS 调色板索引的
二进制，最后由 `crates/k-assets/` 通过 `include_bytes!` 嵌入内核。

## 资产清单

| 文件 | 用途 | 算法 |
|---|---|---|
| `assets/sphere_<class>_<size>.ppm` | 每子域参考渲染（6 class × 多尺寸） | 全 PBR ray-traced 一个 sphere with material profile |
| `assets/env_<face>.ppm` | 6 面 nebula cubemap | 程序化噪声 + 渐变 + 星点 |
| `assets/rope_braided.ppm` | 编织电缆纹理条 | 3 股交叉 + per-pixel shading |
| `assets/brdf_lut.ppm` | Split-sum BRDF integration LUT | GGX × Schlick 预积分 |

## 工具

| 脚本 | 输入 | 输出 |
|---|---|---|
| `generate.py [target]` | — | `assets/*.ppm` |
| `pack_palette.py <ppm>` | PPM 文件 | `.pal` 二进制（每像素 1 字节 palette index） |
| `run_all.ps1` | — | 一键重新生成全部 assets |

`target` 可以是 `all`（默认）、`sphere`、`env`、`rope`、`brdf`。

## 调用

```powershell
# 一次生成全部
python tools/assets/generate.py

# 仅 sphere 重做
python tools/assets/generate.py sphere

# pack 转换 (一个文件)
python tools/assets/pack_palette.py assets/sphere_Hardware_64.ppm

# 一键全部 (生成 + pack)
.\tools\assets\run_all.ps1
```

## 调色板对齐

`pack_palette.py` 必须用与 `crates/k-fb/src/lib.rs` 的 `PALETTE` 和
`HUE_PEAKS` 一致的 6-bit-per-channel 调色板查找。每像素找最近欧氏
距离的 palette index。共 256 槽位（slot 0-11 命名颜色、slot
16-55 五条 8-step Lambertian ramp）。

## 当前烘焙参数

ray-trace 参数与 kernel sphere shader (`crates/hypervisor/src/main.rs`
里的 `material_for_sub_domain`) 完全一致：

| Class | metallic | roughness | rim | bump | aniso |
|---|---|---|---|---|---|
| Hardware     | 0.95 | 0.18 | 1.20 | 0.000 / 0   |  0.00 |
| KernelDriver | 0.92 | 0.25 | 1.00 | 0.025 / 32  | +0.65 |
| Service      | 0.80 | 0.38 | 0.85 | 0.018 / 22  | +0.30 |
| Compute      | 0.88 | 0.30 | 1.05 | 0.030 / 28  | -0.55 |
| Routing      | 0.70 | 0.45 | 0.90 | 0.022 / 18  | +0.20 |
| Vector       | 0.40 | 0.65 | 0.70 | 0.055 / 14  |  0.00 |

ray-trace 比实时 sphere shader 多两件事：
1. **2× supersampling** — 抗锯齿
2. **真实 cubemap 采样** — env reflection 不用程序化 sin lobe，而用同一管线烘焙的 `env_<face>.ppm`

## Phase 进度

- **N.6.a** 当前：生成管线 + 第一批 PPM + Rust 嵌入
- **N.6.b** 后续：让 kernel 实时采样 `env_*.ppm` 作为 reflection（替换 procedural `sample_environment`）
- **N.6.c** 后续：远 LOD 用 `sphere_*_16.ppm` sprite blit（性能优化）
