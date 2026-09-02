<div align="center">

# TexelBox

**One app. The entire texture pipeline for game developers.**

Stop jumping between services, standalone normal-map generators, and texture atlas packers. TexelBox handles everything from raw source image to game-ready export — in one fast, native Windows app.

[Download](https://github.com/iimadouu/TexelBox/releases) · [Features & Pricing](https://texelbox-license.imadedar98.workers.dev/pricing) · [Support](mailto:imadedar98@gmail.com)

</div>

---

## What is TexelBox?

TexelBox is a **lightweight, native Windows app** for game developers who need to turn raw images into game-ready textures fast.

No subscriptions. No bloat. No jumping between five different tools. Just open an image, run through the pipeline, export.

**Who's this for?** Indie devs, hobbyists, and technical artists who want professional textures without the Substance Designer learning curve.

**What this is NOT:** A node-based shader editor, sculpting tool, or asset library. We're a shortcut tool — fast, focused, and affordable.

---

## Eight Tools, One Pipeline

| # | Tool | What It Does |
|---|---|---|
| 1 | **Maps Generation** | Derive normal, height, roughness, and AO maps from a single source image |
| 2 | **Tileable Prep** | Make any photo seamlessly loopable with offset, auto-heal, mirror, and brush tools |
| 3 | **Channel Packing** | Merge maps into one RGBA texture with engine presets (Unreal, Unity, Godot) |
| 4 | **Atlas Generation** | Pack multiple textures into a single atlas with UV sidecar |
| 5 | **Optimization** | Resize, snap to power-of-two, export to PNG/TGA/DDS with BC compression |
| 6 | **Quick Preview** | Live 3D viewport with automated validation that catches common mistakes |
| 7 | **Batch Processing** | Run any operation or chain across an entire folder with progress logging |
| 8 | **Presets** | Save, load, and share parameter configurations across projects |

The whole pipeline flows together automatically. Maps output feeds straight into channel packing. Packed textures go right into atlas building. No manual re-exporting between steps.

---

## Free vs Pro vs Trial

### Free — $0 forever
- Normal, height, roughness, AO maps
- Tileable prep with offset, mirror, and brush
- Basic atlas packing (up to 16 images, 2048×2048)
- PNG/TGA export
- Plane preview with basic validation
- Single batch operations (up to 10 files)
- 3 presets per functionality

### Pro — $49.99 one-time
- Everything in Free, plus:
- Full slider control for all map types
- Auto-heal and live 3×3 preview for tileable prep
- Unlimited atlas size (8192×8192), rotation, trim-sheet
- DDS with BC1/BC3/BC5/BC7 compression
- Engine presets for channel packing
- Multi-operation batch chains with dry-run
- Unlimited presets + export/import
- Sphere preview and full validation suite

### Trial — $0 
- Everything in Pro for 24 hours
- No credit card required
- After trial expires, user switched automatically to Free


**One-time purchase.** No subscription. Lifetime license includes all future updates.

[See the full feature comparison →](https://texelbox-license.imadedar98.workers.dev/pricing)

---

## Download

1. Grab the latest `texelbox-setup.exe` from [Releases](https://github.com/iimadouu/TexelBox/releases)
2. Run the installer (Inno Setup — opt-in desktop icon, x64 only)
3. Launch — opens maximized and adapts to any screen

**System requirements:** Windows 10/11, 64-bit.

---

## Quick Start

1. Open **Maps Generation** → **Open Image**
2. Click **Generate Maps** — all four maps generate instantly
3. Click **Send to Channel Packer** to auto-assign channels
4. Switch to **Quick Preview** to see your material live
5. Export from **Optimize / Export**

**Activate Pro:**
1. Create an account at [texelbox website](https://texelbox-license.imadedar98.workers.dev/)
2. Purchase a license or start a free trial (free forever after trial) at [texelbox plans](https://texelbox-license.imadedar98.workers.dev/pricing)
3. Download texelbox installer
4. Open **Settings** → enter email, password, and license key
5. Click **Activate License**
6. Done. Your license is cached for offline use within a 5-day grace period.

---

## Built With

- **Rust** — native binary, no runtime, no webview
- **Slint** — native UI toolkit
- **Rayon** — all CPU cores for image processing
- **wgpu** — GPU-accelerated 3D preview

---

## License

TexelBox is proprietary software. Free tier is permanently free for personal and commercial use. Pro requires a purchased license key.

See [LICENSE](https://github.com/iimadouu/TexelBox/blob/main/LICENSE) for full terms.

---

<div align="center">

**[Download TexelBox](https://github.com/iimadouu/TexelBox/releases)** · [Support](mailto:imadedar98@gmail.com) · [GitHub](https://github.com/iimadouu/TexelBox)

</div>
