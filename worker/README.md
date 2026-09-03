# TexelBox License & Billing Worker

Cloudflare Worker + Supabase backend (free tier) that issues Ed25519-signed
entitlement tokens verified by the Rust client (`crates/tbx-entitlements`), and
also serves the tiny account UI (login / signup / pricing / manage subscription /
cancel) and verifies emails via the Gmail API.

- **Supabase Auth** — email/password credentials (bcrypt-hosted). This Worker
  never sees or stores passwords.
- **Supabase Postgres** — users / licenses / devices / sessions /
  capability_overrides (schema: `supabase/migrations/0001_init.sql`).
- **Cloudflare Worker** — endpoints + HTML pages (no custom domain required; runs
  on the free `*.workers.dev` subdomain).
- **Lemon Squeezy** — payments (free, no Stripe fee). Webhook + checkout +
  cancellation.
- **Gmail API (HTTPS)** — verification email from your personal Gmail. Workers
  can't open raw SMTP sockets, so mail is sent via `gmail.googleapis.com`.

## Endpoints

JSON API (desktop app):
- `POST /auth/login` — verifies against Supabase Auth, issues our opaque session.
- `POST /license/validate` — session + license key + seat check → signed token.
- `POST /license/heartbeat` — refresh or `{revoked:true}` (remote revoke).
- `GET /user/profile` — session-protected account summary.
- `GET /app/version` — `{version, url}` for the client auto-update check.
- `GET /pricing` — `{plan, amount, currency, interval}` for the pricing page
  (read from the `public.pricing` table; Pro is seeded at $3.99 / month).
- `POST /payments/webhook` — Lemon Squeezy signature, plan updates.

HTML pages (browser):
- `/` home · `/login` · `/signup` (creates account + emails license key) ·
  `/pricing` · `/account` (manage subscription + cancel) ·
  `/verify?token=…` (email confirm) · `/subscribe` (redirect to LS checkout) ·
  `/cancel` (cancel subscription) · `/logout` · `/download` (→ `LATEST_DOWNLOAD_URL`).

## Token contract (do not change casually)

Wire format: `base64url_no_pad(claims).base64url_no_pad(sig)`.
Claims JSON keys in serde order: `plan` ("Free"/"Pro"), `expires_at`,
`issued_at`, `device_id`, `extra_grants`, `denials` (Capability *variant
names*, e.g. `"MapsAoMap"`). The regression vector is pinned in BOTH
`src/handlers.test.ts` (TypeScript) and the Rust
`cross_language_token_vector` test in `crates/tbx-entitlements/src/token.rs`.

## Deploy

```bash
# 1. Supabase project
supabase db push                      # runs supabase/migrations/*
# 2. Worker secrets (see docs/server-setup.md §3.2)
wrangler secret put SUPABASE_SERVICE_ROLE_KEY
wrangler secret put SUPABASE_ANON_KEY
wrangler secret put TOKEN_SIGNING_KEY
wrangler secret put LEMONSQUEEZY_API_KEY
wrangler secret put LEMONSQUEEZY_WEBHOOK_SECRET
wrangler secret put GMAIL_CLIENT_ID
wrangler secret put GMAIL_CLIENT_SECRET
wrangler secret put GMAIL_REFRESH_TOKEN
# 3. edit wrangler.toml [vars]: SUPABASE_URL, LEMONSQUEEZY_*_ID, APP_PUBLIC_URL,
#    GMAIL_FROM, LATEST_VERSION, LATEST_DOWNLOAD_URL
wrangler deploy
```

After deploy, generate the real token key (`openssl rand -hex 32` → seed; derive
its public key) and provision it into the client's
`crates/tbx-entitlements/src/secrets.rs` `PRODUCTION_VERIFY_KEY`, then point
`secrets::server_url()` at `https://<worker>.<account>.workers.dev` and rebuild.

## Test / typecheck

```bash
npm test              # vitest — full flow incl. the Rust-pinned token vector
npm run typecheck     # tsc --noEmit
```

## Notes / TODO (production)

- Rate limiting is in-memory per isolate; replace with the Cloudflare Rate
  Limiting binding before real traffic.
- Lemon Squeezy webhook destination: `https://<worker>/payments/webhook`
  (reads the `X-Signature` header).
- Gmail: the refresh token is exchanged for an access token per send
  (`src/email.ts`); if sending fails, signup still succeeds.
- Session cleanup: schedule `store.deleteExpiredSessions` via a Worker cron
  trigger (currently sessions just expire by check-on-read).
