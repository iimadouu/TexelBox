## v0.2.0 — 2026-09-03

### Free tier expanded
- All four map types (normal, height, roughness, AO) now free — no plan required
- Full slider control for map generation free
- Auto-heal in tileable prep free
- Alpha channel packing free
- Atlas cap bumped: 32 images (was 16), up to 2048×2048
- Batch cap bumped: 25 files per run (was 10)
- Presets cap bumped: 5 per functionality (was 3)

### New features
- **Quick Export** — one-click save to last-used folder in Maps, Tileable, Packing, and Atlas panels
- **File size estimate** — optimize panel shows estimated output size before export (e.g. ~142 KB PNG)
- **Undo/redo** — 10-step history for all tileable edits (offset, mirror, brush, auto-heal, reset)
- **Engine export profiles** (Pro) — new panel with one-click engine-ready export:
  - Unreal 5: ORM pack + `T_name_D / T_name_ORM / T_name_N` naming
  - Unity HDRP: Mask map (R=Metallic, G=AO, B=Detail, A=Smooth)
  - Godot 4: standard PBR naming (`_albedo / _normal / _roughness / _ao`)
- **Source image remembered** between sessions — reopen app, last source reloads automatically
- **Last-used folder** remembered per panel

## v0.1.1 — 2026-09-02

- Small UI patch

## v0.1.0 — 2026-07-28

- Initial release
- Maps, tileable, channel packing, atlas, optimize, preview, batch, presets
