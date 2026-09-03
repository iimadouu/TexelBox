/// <reference types="@cloudflare/workers-types" />

export interface Env {
  SUPABASE_URL: string;
  SUPABASE_SERVICE_ROLE_KEY: string;
  SUPABASE_ANON_KEY: string;
  /** 32-byte Ed25519 seed (hex) used to sign entitlement tokens. */
  TOKEN_SIGNING_KEY: string;

  // --- Whop (one-time payments) -------------------------------------------
  /** Whop API key (dashboard → API keys). Keep as a secret. */
  WHOP_API_KEY: string;
  /** Webhook signing secret (dashboard → webhooks). Whop sends it as the
   * `X-Whop-Signature` header (hex HMAC-SHA256 of the raw body). */
  WHOP_WEBHOOK_SECRET: string;
  /** Product id for the Pro lifetime license (dashboard → products). */
  WHOP_PRODUCT_ID: string;
  /** Optional product id for the 1-day trial (falls back to WHOP_PRODUCT_ID). */
  WHOP_TRIAL_PRODUCT_ID?: string;
  /** Trial TTL in seconds (default 86400 = 1 day). */
  TRIAL_TTL_SECS?: string;

  // --- Brevo (verification email over the free Brevo SMTP API, HTTPS-only) ---
  // Cloudflare Workers cannot open raw SMTP/TCP sockets, so we send mail via
  // Brevo's SMTP API (https://api.brevo.com/v3/smtp/email). This needs only
  // one API key (no OAuth consent screen / refresh token dance).
  /** Brevo API key (Settings → API Keys). Keep as a secret. */
  BREVO_API_KEY: string;
  /** Verified sender email address (Settings → Sender IDs). */
  BREVO_FROM_EMAIL: string;
  /** Optional display name for the sender, e.g. "TexelBox". */
  BREVO_FROM_NAME?: string;

  /** Public base URL of this worker, e.g.
   * "https://texelbox-license.<subdomain>.workers.dev". Used to build the
   * email-verify link and the Lemon Squeezy checkout redirect. */
  APP_PUBLIC_URL?: string;

  TOKEN_TTL_SECS?: string;
  SESSION_TTL_SECS?: string;
  /** Auto-update metadata served at GET /app/version (spec §9 Phase 13). */
  LATEST_VERSION?: string;
  LATEST_DOWNLOAD_URL?: string;
}
