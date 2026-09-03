<div align="center">

# TexelBox

**One app. Eight tools. The entire texture pipeline for game developers.**

Stop jumping between diffrent services and softwares, standalone normal-map generators, and texture atlas packers. TexelBox handles everything from raw source image to game-ready export — in one fast, native Windows app.

[Download](https://github.com/iimadouu/TexelBox/releases) · [Features & Pricing](https://texelbox-license.imadedar98.workers.dev/pricing) · [Support](mailto:imadedar98@gmail.com)

</div>

---

## What is TexelBox?

TexelBox is a **lightweight, native Windows app** for game developers who need to turn raw images into game-ready textures fast.

No subscriptions. No bloat. No jumping between five different tools. Just open an image, run through the pipeline, export.

**Who's this for?** Indie devs, hobbyists, and technical artists who want professional textures without the Substance Designer learning curve.

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

## Free vs Pro

### Free — $0 forever
- Normal, height, roughness, and AO maps — all four, full slider control
- Tileable prep: offset, mirror, brush, and auto-heal
- Channel packing with all 4 channels including alpha
- Basic atlas packing (up to 32 images, 2048×2048)
- PNG + TGA export
- Plane preview with basic validation
- Single batch operations (up to 25 files)
- 5 presets per functionality
- Quick Export in every panel
- Source image remembered between sessions

### Pro — $49.99 one-time
- Everything in Free, plus:
- Atlas above 2048×2048 (up to 8192×8192), rotation, and trim-sheet
- DDS with BC1/BC3/BC5/BC7 compression
- Engine export profiles: Unreal 5 (ORM), Unity HDRP (Mask), Godot 4 (PBR)
- Channel packing engine presets (Unreal ORM, Unity, Godot)
- Multi-operation batch chains with dry-run and unlimited files
- Unlimited presets + export/import
- Sphere preview and full validation suite
- Live 3×3 tileable repeat preview

**One-time purchase.** No subscription. Lifetime license includes all future updates.

[See the full feature comparison →](https://texelbox-license.imadedar98.workers.dev/pricing)

**Free trial:** Test drive Pro for 24 hours — no credit card needed.

---

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
6. Done. Your license is cached for offline use within a 2-day grace period.

---

## Built With

- **Rust** — native binary, no runtime, no webview
- **Slint** — native UI toolkit
- **Rayon** — all CPU cores for image processing
- **wgpu** — GPU-accelerated 3D preview

---

## License

TexelBox is proprietary software. Free tier is permanently free for personal and commercial use. Pro requires a purchased license key.

See [LICENSE](./LICENSE) for full terms.

---

<div align="center">

**[Download TexelBox](https://github.com/iimadouu/TexelBox/releases)** · [Support](mailto:imadedar98@gmail.com) · [GitHub](https://github.com/iimadouu/TexelBox)

</div>
