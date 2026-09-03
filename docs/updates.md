

efore the build:

Bump version in Cargo.toml → version = "0.2.0" (or whatever the new version is)
Run cargo build --release --bin texelbox (or 
release.ps1
)
The binary now has CARGO_PKG_VERSION = "0.2.0" baked in — this is what the update check compares against# TexelBox — Publishing Updates

How to release a new version and how users receive it.

---

## Quick Release Checklist

1. Bump version in `Cargo.toml`
2. `scripts\release.ps1` → builds installer(s)
3. Upload installer(s) to GitHub Releases
4. Update `worker/wrangler.toml` with new version + download URL
5. `npx wrangler deploy`

See `docs/github-release-guide.md` for detailed step-by-step.

---

## How Updates Work

### The mechanism

The client **never downloads or installs anything by itself**. It only notifies the user that a newer build exists.

1. At startup, the app calls `GET {SERVER_URL}/app/version`
2. Server returns `{ "version": "0.2.0", "url": "https://..." }`
3. Client compares with its own version (`CARGO_PKG_VERSION`)
4. If newer → amber banner appears in Settings with "Download Update"
5. User clicks → browser opens the download URL
6. User downloads and runs the installer manually

### Offline users

- Update check fails silently (10s timeout)
- App opens normally, no banner, no error
- Next launch while online → banner appears

### What the installer does

- Replaces the exe in the install folder
- Does NOT touch `%APPDATA%\TexelBox` — presets and license cache survive
- User relaunches → banner is gone

---

## Versioning Rules

- Single source of truth: `Cargo.toml` `[workspace.package] version`
- Use `major.minor.patch` (e.g. `0.2.0`)
- Bump for every public release
- Pre-release suffixes (`0.2.0-beta1`) compare by numeric prefix only

---

## File Map

| Concern | File |
|---|---|
| Version endpoint (server) | `worker/src/handlers.ts` → `appVersion()` |
| Route | `worker/src/index.ts` → `GET /app/version` |
| Worker config | `worker/wrangler.toml` → `LATEST_VERSION`, `LATEST_DOWNLOAD_URL` |
| Client check + compare | `crates/tbx-app/src/update_net.rs` |
| Running version | `update_net.rs` → `APP_VERSION` (`CARGO_PKG_VERSION`) |
| Banner UI | `crates/tbx-app/ui/panels/settings.slint` |
| Open-download action | `crates/tbx-app/src/settings_panel.rs` |
| Build + installer | `scripts/release.ps1`, `installer/texelbox.iss` |
