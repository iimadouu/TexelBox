# TexelBox — Server & Certificate Setup Guide (100% free path)

Operational runbook for the license backend (Supabase + Cloudflare
Worker) and Windows code-signing. Target audience: the developer doing the
production deployment. Estimated time: ~1 hour excluding verification waits.

This guide is tuned for a **zero-cost launch**: Supabase free tier, Cloudflare
Workers free tier (`*.workers.dev` — no custom domain), **Whop.com** for
payments (one-time payment, no monthly fee), and **Brevo** for verification
emails (sent through the Brevo SMTP API over HTTPS — see §5 for why not raw
SMTP). The Worker also serves the tiny account UI (login / signup /
pricing / manage purchase / cancel) so you don't need a separate host.

```
   Client (texelbox.exe)                Cloudflare Worker              Supabase
   ┌──────────────────────┐   HTTPS   ┌──────────────────────┐  HTTPS ┌────────────┐
   │ license_net.rs       │ ────────► │ texelbox-license     │ ─────► │ Auth       │
   │  login/validate/     │           │  JSON API + HTML     │        │ Postgres   │
   │  heartbeat           │ ◄──────── │  account pages       │ ◄───── │ (data)     │
   └──────────────────────┘  signed   └──────────────────────┘        └────────────┘
                                   ▲
                                   │ signed webhook
                               Whop.com        (Brevo SMTP API → verified sender for verify emails)
```

---

## 0. Prerequisites (all free to start)

| Thing | Where | Cost | Notes |
|---|---|---|---|
| Supabase account + project | supabase.com | Free tier OK | Auth + Postgres |
| Cloudflare account | dash.cloudflare.com | Workers free tier OK | Gives you `*.workers.dev` |
| Node.js ≥ 18 + npm | nodejs.org | Free | For wrangler + worker tests |
| Whop account | whop.com | Free (takes a cut of sales) | Digital product payments, webhook support |
| Brevo account | brevo.com | Free (300 emails/day) | Used to *send* verification mail |
| Node.js ≥ 18 + npm | nodejs.org | Free | For wrangler + worker tests |
| OpenSSL *or* Node | — | Free | For Ed25519 key generation |
| Windows SDK `signtool.exe` | VS Build Tools | Free | Only for code signing (§7) — **optional** for launch |

You do **not** need a domain, an email service, or a code-signing certificate to
launch. Those are noted where they would apply.

---

## 1. Supabase

### 1.1 Create the project

1. supabase.com → **New project**. Region: pick the one closest to most users.
2. Store the **database password** in your password manager — Supabase shows it once.

### 1.2 Run the schema

Option A (CLI):

```bash
cd worker
supabase link --project-ref <your-project-ref>     # from the project URL
supabase db push                                   # applies supabase/migrations/*
```

Option B (dashboard): open **SQL Editor** and paste the full contents of
`worker/supabase/migrations/0001_init.sql`, then **Run**.

The migration creates `users` (now with `email` + `email_verified`), `licenses`,
`devices`, `sessions`, `capability_overrides`, enables RLS and pins all access to
the `service_role`. The app's own Supabase keys never get direct access.

> Already applied the old schema? Add the two new columns:
> ```sql
> alter table public.users add column if not exists email text not null default '';
> alter table public.users add column if not exists email_verified boolean not null default false;
> ```

### 1.3 Configure Auth

1. **Authentication → Providers → Email**: enabled (default).
2. **Authentication → Settings → Site URL / Redirect URLs**: not needed — the
   Worker uses the password grant programmatically, and signup creates users
   via the Admin API with `email_confirm: true`.
3. Leave "Allow new signups" enabled — the Worker's `/signup` page creates
   accounts through the Admin API regardless, but keeping it on avoids surprises.

### 1.4 Collect the values you'll need

SUPABASE_URL=https://anrcbaikbmcitgiyzkas.supabase.co
SUPABASE_PUBLISHABLE_KEY=sb_publishable_VWNMCOzx-dSAwq1w9D8WMw_K_nCfmpT
SUPABASE_SECRET_KEY=sb_secret_yVFDXxRNkt7WKv7ugun73w_tqiroFm9
SUPABASE_JWKS_URL=https://anrcbaikbmcitgiyzkas.supabase.co/auth/v1/.well-known/jwks.json


From **Project Settings → API**:

- `Project URL` → Worker var `SUPABASE_URL`
- `anon public` key → Worker secret `SUPABASE_ANON_KEY` (used for Auth sign-in)
- `service_role` key → Worker secret `SUPABASE_SERVICE_ROLE_KEY` (used for DB)

> **Never** put `service_role` anywhere except the Worker secret. It bypasses RLS.

> **No manual user/license creation needed.** Unlike the Stripe-era flow, users
> self-register on the Worker's `/signup` page, which creates the Supabase Auth
> user, the `users` row, and a Free `licenses` row, then emails the license key.
> You only touch SQL for ops (revoke / inspect) — see §6.

---

## 2. Token signing key (Ed25519)

The Worker signs entitlement tokens; the client verifies them with the embedded
public key. Generate the keypair **once** and store the seed in your password
manager/secret store.

### 2.1 Generate

Using Node (already installed for the worker):

```powershell
cd worker
node -e "const {ed25519}=require('@noble/curves/ed25519');const {randomBytes}=require('crypto');const seed=randomBytes(32);console.log('SEED:',Buffer.from(seed).toString('hex'));console.log('PUB :',Buffer.from(ed25519.getPublicKey(seed)).toString('hex'));"
```
SEED: 1c14e90ebae59d1514e80a278360393f974577ac583d422406dfeaf9044cde57
PUB : 89d504d34266ca1ef2aafb5fb6e1bec4570b0dfda727a872a668352077c579a3

```
SEED: <64 hex chars>   → Worker secret TOKEN_SIGNING_KEY
PUB : <64 hex chars>   → client PRODUCTION_VERIFY_KEY
```

### 2.2 Provision the client (release builds)

Edit `crates/tbx-entitlements/src/secrets.rs` — convert the public key hex into
Rust bytes (`secrets.rs:59`):

```rust
pub const PRODUCTION_VERIFY_KEY: [u8; 32] = [
    0xAB, 0xCD, /* … 32 bytes from the PUB line … */
];
```

Quick converter:

```powershell
node -e "process.argv[1].match(/../g).forEach(b=>process.stdout.write('0x'+b.toUpperCase()+', '))" <PUB-HEX>
```

While this array is all zeros, release builds **fail closed** (Pro stays locked
even with a valid token) — that is intentional (`has_production_key()`).

### 2.3 Point the client at the Worker

Same file, `SERVER_URL_PLAIN` (`secrets.rs:44`):

```rust
const SERVER_URL_PLAIN: &str = "https://texelbox-license.<your-subdomain>.workers.dev";
```

The URL is XOR-obfuscated at compile time; no other call site changes. On the
free plan this is your `*.workers.dev` subdomain — **no custom domain required**.
The obfuscated URL must exactly match the worker you deploy (set `APP_PUBLIC_URL`
to the same value in `wrangler.toml`).

---

## 3. Cloudflare Worker (free `*.workers.dev`)

### 3.1 Configure

```powershell
cd worker
npm install
npx wrangler login
```

Edit `wrangler.toml` `[vars]`:

```toml
name = "texelbox-license"            # → https://texelbox-license.<sub>.workers.dev
SUPABASE_URL = "https://<project-ref>.supabase.co"
WHOP_API_URL = "https://api.whop.com"
WHOP_PRODUCT_ID = "<your Whop product ID>"
APP_PUBLIC_URL = "https://texelbox-license.<sub>.workers.dev"
BREVO_FROM_EMAIL = "you@example.com"
BREVO_FROM_NAME = "TexelBox"         # optional
LATEST_VERSION = "0.1.0"
LATEST_DOWNLOAD_URL = "https://github.com/<you>/texelbox/releases/latest"
```

### 3.2 Secrets

```powershell
npx wrangler secret put SUPABASE_SERVICE_ROLE_KEY
npx wrangler secret put SUPABASE_ANON_KEY
npx wrangler secret put TOKEN_SIGNING_KEY      # the SEED from §2.1
npx wrangler secret put WHOP_API_KEY           # §4.1
npx wrangler secret put WHOP_WEBHOOK_SECRET    # §4.2
npx wrangler secret put BREVO_API_KEY          # §5
```

### 3.3 Test, deploy, verify

```powershell
npm test                 # must be green (incl. the Rust-pinned token vector)
npm run typecheck
npx wrangler deploy
```

The Worker exposes two surfaces:

- **JSON API (desktop app):** `POST /auth/login`, `POST /license/validate`,
  `POST /license/heartbeat`, `GET /user/profile`, `GET /app/version`,
  `POST /payments/webhook`.
- **HTML pages (browser):** `/` (home), `/login`, `/signup`, `/pricing`,
  `/account` (manage subscription + cancel), `/verify?token=…` (email confirm),
  `/subscribe` (redirect to Lemon Squeezy checkout), `/cancel` (cancel
  subscription), `/logout`, `/download` (redirects to `LATEST_DOWNLOAD_URL`).

Smoke-test the deployment (PowerShell):

```powershell
$base = "https://texelbox-license.imadedar98.workers.dev/"
Invoke-RestMethod "$base/auth/login" -Method Post -ContentType "application/json" `
  -Body '{"email":"<test user>","password":"<password>"}'
# → { session_token = …, plan = …, status = … }
```

Also open `$base/` in a browser to see the account UI. Then activate from the
actual client (Settings → Activate License).

### 3.4 Hardening before real traffic

- **Rate limiting:** dashboard → Workers → Rate Limiting → bind to the worker;
  replace the in-memory limiter in `handlers.ts` with the binding (per license
  key on `/license/validate` + `/auth/login`).
- **Session cleanup:** add a cron trigger (`[triggers] crons = ["0 * * * *"]`)
  and call `store.deleteExpiredSessions()` from the scheduled handler.
- **Logging:** `npx wrangler tail` for live logs while testing.

---

## 4. Whop.com (payments — free, one-time)

### 4.1 Product setup


pro: https://whop.com/dashboard/biz_rU3ifOObYpyddm/products/prod_vHqy3gWrcoN3v/

trial: https://whop.com/dashboard/biz_rU3ifOObYpyddm/products/prod_DuLXwFGocWv07/

1. Whop.com dashboard → **Products** → create "TexelBox Pro" as a
   **one-time purchase** (lifetime license). Note the **Product ID**.
2. Optionally create a second product "TexelBox Pro Trial" as a **one-time
   purchase** priced at **$0 USD** with a 1-day access window (set the trial TTL in the Worker).
   Note its **Product ID**.
3. **Settings → API Keys** → create an API key → Worker secret `WHOP_API_KEY`.
4. **Settings → Webhooks** → **Add endpoint**:
   `https://texelbox-license.<your-subdomain>.workers.dev/payments/webhook`
   Subscribe to the following events:
   - `payment.authorized`
   - `payment.canceled`
   - `payment.created`
   - `payment.failed`
   - `payment.pending`
   - `payment.succeeded`
   
   Copy the **Signing secret** → `WHOP_WEBHOOK_SECRET`.

### 4.2 Checkout + purchase (handled by the Worker pages)

- **Buy Pro / Start Trial:** the client Settings → "Buy Pro" (or the `/pricing`
  page) opens `/purchase` (or `/purchase?trial=1`), which creates a Whop
  checkout link with the user's email and redirects the browser to it. After payment,
  Whop redirects back to `/account` and the webhook flips the plan to Pro/Trial.
- **Cancel:** `/account` shows a **Cancel purchase** button → `POST /cancel-purchase`
  → the Worker immediately sets the user to Free/cancelled (the webhook reconciles
  on the next event). Pro stays usable until the client next validates.

No manual license-key emailing is needed — signup sends the key automatically (§5).

### 4.3 Pricing

The Pro **price** lives in the `public.pricing` table (seeded
by the migration `0001_init.sql` with `amount = 29.99, currency = 'usd',
interval = 'once'`). The Worker's `/pricing` page reads it and shows it — so
you change the displayed price there, **not** in code. The migration also has
nullable `whop_product_id` / `whop_trial_product_id` columns;
when you fill those in, the checkout uses the database values instead of the
`wrangler.toml` vars (handy if you later add more plans). For launch, leave
them empty and keep the ids in `wrangler.toml`.

---

## 5. Verification email via Brevo (free SMTP API over HTTPS)

**Cloudflare Workers cannot open raw SMTP/TCP sockets**, so the usual
`smtp-relay.brevo.com:465` + password flow is impossible in a Worker. Instead we
send mail through **Brevo's SMTP API** (`https://api.brevo.com/v3/smtp/email`),
which is plain HTTPS and works from the free Worker tier.

Free tier: **300 emails/day** — more than enough for signup verification.

### 5.1 One-time setup

1. Create a free account at [brevo.com](https://www.brevo.com/).
2. **Settings → Sender IDs** → add + verify the email address you want to send
   from (e.g. `texelboxlicense@gmail.com`). Brevo sends a confirmation link.
3. **Settings → API Keys** → **Create a new API key** → copy it.
4. Store it as a Worker secret (§3.2): `BREVO_API_KEY`.
5. Set the sender vars in `wrangler.toml`:
   ```toml
   BREVO_FROM_EMAIL = "texelboxlicense@gmail.com"
   BREVO_FROM_NAME = "TexelBox"   # optional
   ```

### 5.2 What the Worker sends

On `/signup` the Worker emails the user a **verification link** (clicking it hits
`/verify?token=…`, which sets `email_verified = true`) **and the license key**
they paste into the desktop app (Settings → Activate License). The mailer lives
in `worker/src/email.ts`; it POSTs to Brevo's SMTP API on each send. If email
sending fails, signup still succeeds and the user can request the key again after
logging in.

### 5.3 Troubleshooting

- **"Sender not verified"**: the `BREVO_FROM_EMAIL` must match a verified sender
  in your Brevo account.
- **"Invalid API key"**: re-check the secret was stored with
  `npx wrangler secret put BREVO_API_KEY`.
- **Daily limit reached**: Brevo free tier is 300 emails/day. Monitor in the
  Brevo dashboard.

---

## 6. Revoking / abusing licenses (ops)

```sql
-- remote revoke: the next heartbeat (≤6h) tells the client to drop Pro
update public.licenses set revoked = true where key = 'TBX-…';

-- un-revoke
update public.licenses set revoked = false where key = 'TBX-…';

-- inspect flagged (shared-key abuse) licenses
select key, flagged, max_seats from public.licenses where flagged;

-- kick all devices off a license (they must re-validate)
delete from public.devices where license_id = (select id from public.licenses where key = 'TBX-…');

-- force a user to Free immediately (takes effect next validate/heartbeat)
update public.users set plan = 'free', status = 'cancelled' where id = …;
```

Users are created by the signup page, so you rarely insert rows by hand.

---

## 7. Code-signing certificate (Windows) — OPTIONAL for a free launch

Purpose (spec §4.5): SmartScreen trust + tamper evidence for the installer and
exe. It is **not** anti-cracking.

> **Free-start note:** a code-signing cert (OV/EV) costs money. You can ship an
> **unsigned** `texelbox.exe` + installer at zero cost; Windows SmartScreen will
> warn first-time downloaders ("Windows protected your PC"). That is acceptable
> for an early free launch — just tell users it's expected. Buy a cert later
> (§7.1–§7.5 of the original guide) and run `scripts/sign.ps1` + the installer
> script; nothing else changes.

If/when you do buy a cert, the steps are unchanged from the previous guide:
choose OV/EV, buy from any CA/B Forum member (SSL.com, DigiCert, Sectigo,
Certum), and `scripts/sign.ps1` wraps `signtool` (SHA-256 + RFC 3161
timestamping). Verify with `signtool verify /pa /v target\release\texelbox.exe`.

---

## 8. Full production checklist

- [ ] Supabase project + migration applied (incl. `email` / `email_verified`), RLS service-role-only verified
- [ ] Test signup via the `/signup` page works; verify email arrives from your Brevo sender; license key received
- [ ] Ed25519 keypair generated; seed in `TOKEN_SIGNING_KEY`, pub in
      `PRODUCTION_VERIFY_KEY`, URL in `SERVER_URL_PLAIN`; **release** build
      activates successfully (debug builds alone are NOT proof — they accept the DEV key too)
- [ ] `cargo test --workspace` green (vector test pins client/Worker compat)
- [ ] `worker` `npm test` + `typecheck` green, `wrangler deploy` done
- [ ] Whop.com product ids set; API key + webhook secret set;
      webhook test event processed (`order.completed` → Pro/Trial)
- [ ] Purchase → checkout → Pro/Trial; cancel from `/account` → Free
- [ ] Brevo API key stored; verification email sends
- [ ] Rate limiting + session-cleanup cron added
- [ ] Release build: `cargo build --release`, optionally signed
      (`signtool verify /pa` passes), packaged with the installer script
      (`installer/texelbox.iss`)
- [ ] Auto-update endpoint live (`GET /app/version`) + `LATEST_DOWNLOAD_URL` points
      at the published release — see `docs/updates.md`

---

## 9. Key rotation / disaster recovery

- **TOKEN_SIGNING_KEY rotation:** generate a new keypair, update Worker secret,
  ship a client update with the new `PRODUCTION_VERIFY_KEY`. During the rollout
  window, tokens signed by the old key stop verifying → heartbeats reissue with
  the new key within 6h; offline users re-validate when they reconnect (5-day
  grace covers the gap).
- **Supabase restore:** point-in-time restore (paid plan) or re-run the
  migration + re-import the `licenses` table — keep a periodic SQL dump of
  `users` + `licenses` (the only tables that can't be rebuilt). Users re-signup
  if needed (their Supabase Auth row is separate).
- **Worker rollback:** `npx wrangler deployments list` / `rollback` to a previous version.
- **Whop.com:** refunds/cancellations are done in the Whop dashboard; the webhook
  keeps Supabase in sync on the next event.
