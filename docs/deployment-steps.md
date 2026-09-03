# TexelBox — Deployment Steps (100% free, in order)

A linear, do-this-then-that checklist for launching the license + billing
backend and shipping updates, using only free tiers: Supabase, Cloudflare
Workers (`*.workers.dev`), **Whop**, and **Brevo** (for verification emails
via the Brevo SMTP API over HTTPS). Deep reference for each step is in
`docs/server-setup.md`; update publishing is in `docs/updates.md`.

Work top-to-bottom. Steps marked ★ are "do once".

---

## Step 1 ★ — Supabase project + database

1. supabase.com → **New project**. Save the DB password.
2. `cd worker` then either:
   ```bash
   supabase link --project-ref <ref>
   supabase db push
   ```
   or paste `worker/supabase/migrations/0001_init.sql` into **SQL Editor → Run**.
3. **Authentication → Providers → Email**: enabled (default). Leave "Allow new
   signups" on.
4. **Project Settings → API** — copy:
   - `Project URL` → `SUPABASE_URL`
   - `anon public` key → `SUPABASE_ANON_KEY`
   - `service_role` key → `SUPABASE_SERVICE_ROLE_KEY`

---

## Step 2 ★ — Ed25519 token signing key

```powershell
cd worker
node -e "const {ed25519}=require('@noble/curves/ed25519');const {randomBytes}=require('crypto');const s=randomBytes(32);console.log('SEED:'+s.toString('hex'));console.log('PUB:'+Buffer.from(ed25519.getPublicKey(s)).toString('hex'));"
```
- `SEED` → Worker secret `TOKEN_SIGNING_KEY` (Step 5).
- `PUB` → client `PRODUCTION_VERIFY_KEY` (`crates/tbx-entitlements/src/secrets.rs:59`).
  Convert with:
  ```powershell
  node -e "process.argv[1].match(/../g).forEach(b=>process.stdout.write('0x'+b.toUpperCase()+', '))" <PUB-HEX>
  ```

---

## Step 3 ★ — Whop (payments)

1. whop.com → **Products** → create "TexelBox Pro" as a
   **one-time purchase** (lifetime license). Note **Product ID**.
2. (Optional) create "TexelBox Pro Trial" as a **one-time purchase** with
   1-day access. Note its **Product ID**.
3. **Settings → API Keys** → create a key → `WHOP_API_KEY`.
4. **Settings → Webhooks → Add endpoint**:
    `https://texelbox-license.<sub>.workers.dev/payments/webhook`
    Subscribe to `payment.succeeded`, `payment.canceled`,
    `payment.failed`, `payment.authorized`, `payment.created`,
    `payment.pending`. Copy the **Signing secret** →
    `WHOP_WEBHOOK_SECRET`.

---

## Step 4 ★ — Brevo verification email (API key, one-time)

1. Create a free account at [brevo.com](https://www.brevo.com/).
2. **Settings → Sender IDs** → add + verify the email address you want to send
    from (e.g. `texelboxlicense@gmail.com`). Brevo sends a confirmation link.
3. **Settings → API Keys** → **Create a new API key** → copy it.
4. Store it as a Worker secret in Step 5: `BREVO_API_KEY`.

---

## Step 5 — Configure + deploy the Worker

1. Edit `worker/wrangler.toml` `[vars]`:
   ```toml
    SUPABASE_URL = "https://<ref>.supabase.co"
     WHOP_PRODUCT_ID = "<pro product id>"
     WHOP_TRIAL_PRODUCT_ID = "<trial product id>"   # optional
    APP_PUBLIC_URL = "https://texelbox-license.<sub>.workers.dev"
    BREVO_FROM_EMAIL = "you@example.com"
    BREVO_FROM_NAME = "TexelBox"         # optional
    LATEST_VERSION = "0.1.0"
    LATEST_DOWNLOAD_URL = "https://github.com/<you>/texelbox/releases/latest"
   ```
2. Set secrets:
   ```powershell
   npx wrangler secret put SUPABASE_SERVICE_ROLE_KEY
   npx wrangler secret put SUPABASE_ANON_KEY
   npx wrangler secret put TOKEN_SIGNING_KEY
    npx wrangler secret put WHOP_API_KEY
    npx wrangler secret put WHOP_WEBHOOK_SECRET
   npx wrangler secret put BREVO_API_KEY
   ```
3. Test + deploy:
   ```powershell
   npm test            # green
   npm run typecheck   # clean
   npx wrangler deploy
   ```
4. Smoke test:
   ```powershell
   $base = "https://texelbox-license.<sub>.workers.dev"
   Invoke-RestMethod "$base/auth/login" -Method Post -ContentType "application/json" `
     -Body '{"email":"<test>","password":"<pw>"}'
   ```
    Open `$base/` in a browser — you should see the account UI. Try `/signup`
    with a real email: you should get a verification link + license key from Brevo.

---

## Step 6 — Provision the client + release build

1. In `crates/tbx-entitlements/src/secrets.rs`:
   - set `PRODUCTION_VERIFY_KEY` to the `PUB` bytes (Step 2).
   - set `SERVER_URL_PLAIN` to `https://texelbox-license.<sub>.workers.dev`
     (must exactly match the deployed worker / `APP_PUBLIC_URL`).
2. Build + test:
   ```powershell
   cargo test --workspace      # green
   cargo build --release
   ```
3. (Optional, paid later) sign with `scripts/sign.ps1`.
4. Build the installer:
   ```powershell
   powershell -File scripts\release.ps1
   ```
5. Activate from the app: **Settings → Activate License** (email + password +
   the license key emailed at signup). Confirm a **release** build unlocks Pro
   (debug builds accept the DEV key and are NOT proof).

---

## Step 7 — Publish an update (repeat per release)

1. Bump the version in the root `Cargo.toml` (`[workspace.package] version`).
2. `cargo build --release`, optionally `scripts/sign.ps1`, then
   `powershell -File scripts\release.ps1` → `dist/TexelBox/TexelBox-Setup.exe`.
3. Create a **GitHub Release** (free) for the new version; upload
   `TexelBox-Setup.exe`; copy its direct download URL.
4. In `worker/wrangler.toml` set:
   ```toml
   LATEST_VERSION = "0.2.0"
   LATEST_DOWNLOAD_URL = "https://github.com/<you>/texelbox/releases/download/v0.2.0/TexelBox-Setup.exe"
   ```
   then `npx wrangler deploy` (only the Worker config changes — no client rebuild).
5. Users see the **Download Update** banner on next launch → open release → run
   installer (in-place upgrade; presets + license cache preserved).

---

## Quick sanity checklist

- [ ] Supabase project + migration applied
- [ ] `TOKEN_SIGNING_KEY` set; `PRODUCTION_VERIFY_KEY` + `SERVER_URL_PLAIN` set in client; release build activates Pro
- [ ] `npm test` + `cargo test --workspace` green; `wrangler deploy` done
- [ ] Signup emails a verify link + license key from your Brevo sender
- [ ] Whop purchase → Pro/Trial; cancel from `/account` → Free
- [ ] `LATEST_VERSION` / `LATEST_DOWNLOAD_URL` point at a live release
