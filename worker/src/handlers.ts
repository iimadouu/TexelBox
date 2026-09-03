/**
 * License endpoint logic, dependency-injected for testability.
 * All decisions fail CLOSED (spec §4). Framework-agnostic: `index.ts`
 * adapts fetch requests to these functions; tests call them directly.
 *
 * Payments are handled by Whop (one-time payments) — see `webhook()`.
 * A small set of browser-facing handlers (signup / verify-email / account /
 * purchase / cancel) power the Worker-hosted account pages.
 */
import { sha256 } from "@noble/hashes/sha256";
import { hmac } from "@noble/hashes/hmac";
import { ed25519 } from "@noble/curves/ed25519";
import { bytesToHex, hexToBytes, signToken, verifyToken, b64url, type TokenClaims } from "./token";
import { verifyWhopSignature, type WhopClient } from "./whop";
import type { Plan, SessionRow, Store } from "./store";
import { esc, shell } from "./pages";
import {
  MAX_TOKEN_LENGTH,
  validateDelimitedToken,
  validateDeviceId,
  validateEmail,
  validateHexToken,
  validateLicenseKey,
  validatePassword,
} from "./validation";

/** Credential verification is delegated to Supabase Auth (spec: hosted
 * auth; no passwords ever touch this Worker/DB). */
export interface AuthClient {
  /** Throws on bad credentials; resolves with the Supabase user id + email. */
  signIn(email: string, password: string): Promise<{ userId: string; email: string }>;
}

/** Admin user creation (Supabase Auth admin API) for the self-serve signup. */
export interface AuthAdmin {
  createUser(email: string, password: string): Promise<{ userId: string; email: string }>;
  updateUserPassword(userId: string, password: string): Promise<void>;
}

/** Verification-email sender (Gmail API over HTTPS in production). */
export interface Mailer {
  send(to: string, subject: string, html: string): Promise<void>;
}

/** Lemon Squeezy client (checkout creation + cancellation). */
export interface LemonClient {
  createCheckout(input: {
    userEmail: string;
    redirectUrl: string;
    storeId?: string;
    productId?: string;
    variantId?: string;
  }): Promise<string>;
  cancelSubscription(subscriptionId: string): Promise<void>;
}

export interface Ctx {
  store: Store;
  auth: AuthClient;
  authAdmin?: AuthAdmin;
  mailer?: Mailer;
  whop?: WhopClient;
  /** Public base URL of this worker (for verify links + checkout redirect). */
  appUrl?: string;
  /** 32-byte Ed25519 seed, hex. */
  signingKeyHex: string;
  webhookSecret: string;
  tokenTtlSecs: number;
  sessionTtlSecs: number;
  /** Trial TTL in seconds (default 1 day). */
  trialTtlSecs: number;
  /** Auto-update (spec §9 Phase 13): latest release metadata. */
  latestVersion?: string;
  latestDownloadUrl?: string;
  now?: () => number;
}

export interface HandlerResult {
  status: number;
  body: unknown;
  /** Extra response headers (e.g. content-type for HTML pages). */
  headers?: Record<string, string>;
  /** Set-Cookie header values (browser session). */
  cookies?: string[];
}

export const SESSION_COOKIE = "tb_session";

const ok = (body: unknown): HandlerResult => ({ status: 200, body });
const err = (status: number, message: string): HandlerResult => ({ status, body: { error: message } });
const html = (status: number, body: string): HandlerResult => ({
  status,
  body,
  headers: { "content-type": "text/html; charset=utf-8" },
});

export function nowSec(ctx: Ctx): number {
  return Math.floor((ctx.now?.() ?? Date.now()) / 1000);
}

export function sha256Hex(text: string): string {
  return bytesToHex(sha256(new TextEncoder().encode(text)));
}

function randomHex(bytes = 32): string {
  const buf = new Uint8Array(bytes);
  crypto.getRandomValues(buf);
  return bytesToHex(buf);
}

/** Extract the browser session token from a `Cookie` header. */
export function sessionTokenFromCookie(cookieHeader: string | null): string {
  if (!cookieHeader) return "";
  for (const part of cookieHeader.split(";")) {
    const [name, ...rest] = part.trim().split("=");
    if (name === SESSION_COOKIE) return rest.join("=");
  }
  return "";
}

export function sessionCookie(ctx: Ctx, token: string): string {
  return `${SESSION_COOKIE}=${token}; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=${ctx.sessionTtlSecs}`;
}

async function checkSession(ctx: Ctx, token: string): Promise<{ userId?: number; error?: HandlerResult }> {
  if (!token) return { error: err(401, "invalid or expired session") };
  // Defense-in-depth: reject non-hex tokens before hitting the store
  if (!validateHexToken(token, 128)) return { error: err(401, "invalid or expired session") };
  const session = await ctx.store.sessionByToken(token);
  if (!session || session.expires_at <= nowSec(ctx)) {
    return { error: err(401, "invalid or expired session") };
  }
  return { userId: session.user_id };
}

function issueToken(ctx: Ctx, plan: Plan, deviceId: string, grants: string[], denials: string[]) {
  const now = nowSec(ctx);
  const claims: TokenClaims = {
    plan,
    expires_at: now + ctx.tokenTtlSecs,
    issued_at: now,
    device_id: deviceId,
    extra_grants: grants,
    denials,
  };
  return { token: signToken(claims, ctx.signingKeyHex), expires_at: claims.expires_at };
}

// ---------------------------------------------------------------------------
// Email verification token (HMAC, signed with the Ed25519 seed as the key)
// ---------------------------------------------------------------------------

function signEmailToken(ctx: Ctx, email: string): string {
  const payload = JSON.stringify({ email: email.toLowerCase(), exp: nowSec(ctx) + 86400 });
  const raw = new TextEncoder().encode(payload);
  const sig = hmac(sha256, hexToBytes(ctx.signingKeyHex), raw);
  return `${b64url(raw)}.${b64url(sig)}`;
}

function verifyEmailToken(ctx: Ctx, wire: string): string | null {
  if (!wire || wire.length > MAX_TOKEN_LENGTH) return null;
  const dot = wire.indexOf(".");
  if (dot <= 0) return null;
  try {
    const raw = new TextDecoder().decode(b64urlDecodeLocal(wire.slice(0, dot)));
    const sig = b64urlDecodeLocal(wire.slice(dot + 1));
    const expect = hmac(sha256, hexToBytes(ctx.signingKeyHex), new TextEncoder().encode(raw));
    if (sig.length !== expect.length) return null;
    let diff = 0;
    for (let i = 0; i < expect.length; i++) diff |= expect[i] ^ sig[i];
    if (diff !== 0) return null;
    const claims = JSON.parse(raw) as { email: string; exp: number };
    if (typeof claims.exp !== "number" || claims.exp < nowSec(ctx)) return null;
    return claims.email;
  } catch {
    return null;
  }
}

function b64urlDecodeLocal(s: string): Uint8Array {
  const std = s.replace(/-/g, "+").replace(/_/g, "/");
  const bin = atob(std + "=".repeat((4 - (std.length % 4)) % 4));
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function verifyEmailHtml(url: string, key: string): string {
  return `<div style="font-family:system-ui,Arial,sans-serif;max-width:520px;margin:0 auto">
    <h2>Welcome to TexelBox</h2>
    <p>Thanks for creating your account. Confirm your email address to finish setup:</p>
    <p><a href="${url}" style="background:#2563eb;color:#fff;padding:10px 16px;border-radius:6px;text-decoration:none;display:inline-block">Verify email address</a></p>
    <p style="margin-top:24px">Your license key (needed to activate TexelBox on your PC):</p>
    <p style="background:#f1f5f9;padding:10px;border-radius:6px;font-family:monospace;font-size:15px">${key}</p>
    <p class="muted">Log in with your email + password and this key in TexelBox → Settings → Activate License.</p>
  </div>`;
}

function passwordResetHtml(url: string): string {
  return `<div style="font-family:system-ui,Arial,sans-serif;max-width:520px;margin:0 auto">
    <h2>Reset your TexelBox password</h2>
    <p>You requested a password reset. Click the button below to choose a new password:</p>
    <p><a href="${url}" style="background:#2563eb;color:#fff;padding:10px 16px;border-radius:6px;text-decoration:none;display:inline-block">Reset password</a></p>
    <p style="margin-top:24px;color:#64748b">This link expires in 1 hour. If you didn't request this, you can safely ignore it.</p>
  </div>`;
}

// ---------------------------------------------------------------------------
// POST /auth/login — verify against Supabase Auth, issue opaque session
// ---------------------------------------------------------------------------

export async function login(
  ctx: Ctx,
  input: { email?: string; password?: string },
): Promise<HandlerResult> {
  const email = validateEmail(input.email);
  if (!email) return err(400, "valid email and password required");
  const password = validatePassword(input.password);
  if (!password) return err(400, "valid email and password required");

  let supa: { userId: string; email: string };
  try {
    supa = await ctx.auth.signIn(email, password);
  } catch {
    return err(401, "unknown email or password");
  }

  let user = await ctx.store.userBySupabaseId(supa.userId);
  if (!user) {
    user = await ctx.store.createUser(supa.userId, sha256Hex(supa.email ?? email), supa.email ?? email);
  }

  const session: SessionRow = {
    token: randomHex(32),
    user_id: user.id,
    expires_at: nowSec(ctx) + ctx.sessionTtlSecs,
  };
  await ctx.store.createSession(session);
  return ok({ session_token: session.token, plan: user.plan, status: user.status });
}

// ---------------------------------------------------------------------------
// POST /auth/signup — self-serve account creation (web). Creates the Supabase
// Auth user, our user row, a Free license key, and emails a verify link.
// ---------------------------------------------------------------------------

export async function signup(
  ctx: Ctx,
  input: { email?: string; password?: string },
): Promise<HandlerResult> {
  const email = validateEmail(input.email);
  if (!email) return err(400, "a valid email is required");
  const password = validatePassword(input.password);
  if (!password) return err(400, "password is required");
  if (password.length < 8) return err(400, "password must be at least 8 characters");
  if (!ctx.authAdmin) return err(500, "signup is not configured on this server");

  if (await ctx.store.userByEmail(email)) return err(409, "that email is already registered");

  let authUser: { userId: string; email: string };
  try {
    authUser = await ctx.authAdmin.createUser(email, password);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return err(409, `could not create the account: ${msg}`);
  }

  const user = await ctx.store.createUser(authUser.userId, sha256Hex(email), email);
  const key = `TBX-${randomHex(8)}`;
  await ctx.store.createLicenseForUser(user.id, key, "Free", 1);

  const session: SessionRow = {
    token: randomHex(32),
    user_id: user.id,
    expires_at: nowSec(ctx) + ctx.sessionTtlSecs,
  };
  await ctx.store.createSession(session);

  let emailSent = false;
  if (ctx.mailer && ctx.appUrl) {
    try {
      const vt = signEmailToken(ctx, email);
      const url = `${ctx.appUrl.replace(/\/$/, "")}/verify?token=${encodeURIComponent(vt)}`;
      await ctx.mailer.send(email, "Verify your TexelBox account", verifyEmailHtml(url, key));
      emailSent = true;
      console.log(`[email] verification sent to ${email}`);
    } catch (e) {
      emailSent = false;
      console.error(`[email] verification failed for ${email}:`, e);
    }
  }

  return ok({
    session_token: session.token,
    plan: user.plan,
    status: user.status,
    license_key: key,
    email_sent: emailSent,
  });
}

// ---------------------------------------------------------------------------
// GET /verify?token=… — confirm the email address (clicked from the Gmail email)
// ---------------------------------------------------------------------------

export async function verifyEmail(ctx: Ctx, token: string): Promise<HandlerResult> {
  const email = verifyEmailToken(ctx, token);
  if (!email) return err(400, "invalid or expired verification link");
  const user = await ctx.store.userByEmail(email);
  if (!user) return err(404, "account not found");
  if (!user.email_verified) await ctx.store.setEmailVerified(user.id);
  return ok({ verified: true, email });
}

// ---------------------------------------------------------------------------
// POST /resend-verification — re-send the verification email (rate-limited)
// ---------------------------------------------------------------------------

const resendCooldownSecs = 60;
const resendTimers: Record<string, number> = {};

export async function resendVerification(
  ctx: Ctx,
  cookieHeader: string | null,
): Promise<HandlerResult> {
  const token = cookieHeader ? sessionTokenFromCookie(cookieHeader) : "";
  const { userId, error } = await checkSession(ctx, token);
  if (error || userId === undefined) return error ?? err(401, "unauthorized");

  const user = await ctx.store.userById(userId);
  if (!user) return err(404, "account not found");
  if (user.email_verified) return ok({ sent: false, reason: "already verified" });

  const key = `resend:${user.email}`;
  const now = nowSec(ctx);
  const last = resendTimers[key] ?? 0;
  if (now - last < resendCooldownSecs) {
    const wait = resendCooldownSecs - (now - last);
    return err(429, `please wait ${wait}s before requesting another email`);
  }
  resendTimers[key] = now;

  if (!ctx.mailer || !ctx.appUrl) return err(500, "email is not configured on this server");

  const license = await ctx.store.licenseIdForUser(userId);
  const licenseKey = license ? await ctx.store.licenseKeyForId(license) : "";

  try {
    if (!user.email) {
      console.error("[email] resend abort: user.email is empty for", user.id);
      return err(500, "account has no email address");
    }
    const vt = signEmailToken(ctx, user.email);
    const url = `${ctx.appUrl.replace(/\/$/, "")}/verify?token=${encodeURIComponent(vt)}`;
    const html = verifyEmailHtml(url, licenseKey ?? "");
    await ctx.mailer.send(user.email, "Verify your TexelBox account", html);
    return ok({ sent: true });
  } catch (e) {
    console.error("[email] resend failed:", e);
    console.error("[email] resend stack:", e instanceof Error ? e.stack : "no stack");
    return err(502, "email send failed — try again later");
  }
}

// ---------------------------------------------------------------------------
// POST /auth/forgot-password — issue a reset token and email the link
// ---------------------------------------------------------------------------

export async function forgotPassword(
  ctx: Ctx,
  input: { email?: string },
): Promise<HandlerResult> {
  const email = validateEmail(input.email);
  if (!email) return err(400, "a valid email is required");
  console.log(`[forgot-password] received email: "${email}"`);
  if (!ctx.mailer || !ctx.appUrl) {
    const missing: string[] = [];
    if (!ctx.mailer) missing.push("mailer");
    if (!ctx.appUrl) missing.push("appUrl");
    console.error(`[forgot-password] misconfigured: missing ${missing.join(", ")}`);
    return err(500, `email is not configured on this server (missing: ${missing.join(", ")})`);
  }
  console.log(`[forgot-password] mailer and appUrl OK, looking up user`);

  const user = await ctx.store.userByEmail(email);
  if (!user) return ok({ sent: false, reason: "no account with that email" });
  console.log(`[forgot-password] user found: ${user.id}, creating reset token`);

  const token = randomHex(32);
  const expiresAt = nowSec(ctx) + 3600;
  await ctx.store.createPasswordReset(email, token, expiresAt);

  const resetUrl = `${ctx.appUrl.replace(/\/$/, "")}/reset-password?token=${encodeURIComponent(token)}`;
  console.log(`[forgot-password] sending email to ${email}`);
  try {
    await ctx.mailer.send(email, "Reset your TexelBox password", passwordResetHtml(resetUrl));
    console.log(`[forgot-password] email sent successfully`);
    return ok({ sent: true });
  } catch (e) {
    console.error("[email] forgot-password failed:", e);
    return err(502, "email send failed — try again later");
  }
}

// ---------------------------------------------------------------------------
// GET /reset-password?token=… — show the reset form
// ---------------------------------------------------------------------------

export function resetPasswordPage(token: string): string {
  const body = `
    <h2>Reset password</h2>
    <form method="post" action="/reset-password">
      <input type="hidden" name="token" value="${esc(token)}" />
      <label for="password">New password</label>
      <input id="password" name="password" type="password" required minlength="8" autocomplete="new-password" />
      <label for="password2">Confirm new password</label>
      <input id="password2" name="password2" type="password" required minlength="8" autocomplete="new-password" />
      <button type="submit">Update password</button>
    </form>
    <p class="muted">Choose a password with at least 8 characters.</p>`;
  return shell("Reset password · TexelBox", body,
    "Reset your TexelBox account password. Enter a new password to continue.",
    "texture atlas, tileable textures, normal map, roughness map, AO map, height map, TexelBox, build atlas, tileable images, texture generation, gamedev tools",
    "https://texelbox-license.imadedar98.workers.dev/reset-password", true);
}

// ---------------------------------------------------------------------------
// POST /reset-password — consume the token and update the Supabase Auth password
// ---------------------------------------------------------------------------

export async function resetPassword(
  ctx: Ctx,
  input: { token?: string; password?: string; password2?: string },
): Promise<HandlerResult> {
  const token = validateHexToken(input.token);
  if (!token) return err(400, "invalid or expired reset link");
  const password = validatePassword(input.password);
  if (!password) return err(400, "password is required");
  const password2 = input.password2 ?? "";
  if (password.length < 8) return err(400, "password must be at least 8 characters");
  if (password !== password2) return err(400, "passwords do not match");

  const row = await ctx.store.consumePasswordReset(token);
  if (!row) return err(400, "invalid or expired reset link");

  const user = await ctx.store.userByEmail(row.email);
  if (!user) return err(404, "account not found");

  if (!ctx.authAdmin) return err(500, "password reset is not configured on this server");
  try {
    await ctx.authAdmin.updateUserPassword(user.supabase_id, password);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return err(502, `could not update password: ${msg}`);
  }

  return ok({ reset: true });
}

// ---------------------------------------------------------------------------
// POST /license/validate
// ---------------------------------------------------------------------------

export async function validate(
  ctx: Ctx,
  input: { license_key?: string; device_id?: string; session_token?: string },
): Promise<HandlerResult> {
  const key = validateLicenseKey(input.license_key);
  const deviceId = validateDeviceId(input.device_id);
  if (!key || !deviceId) return err(400, "license_key and device_id required");

  const { userId, error } = await checkSession(ctx, input.session_token ?? "");
  if (error || userId === undefined) return error ?? err(401, "unauthorized");

  const license = await ctx.store.licenseByKey(key);
  if (!license) return err(401, "unknown license key");
  if (license.revoked) return err(403, "license revoked");
  if (license.user_id !== userId) return err(403, "license belongs to another account");

  // Seat limiting + shared-key abuse flag (spec §4.6).
  const devices = await ctx.store.devicesForLicense(license.id);
  const known = devices.some((d) => d.device_id === deviceId);
  if (!known && devices.length >= license.max_seats) {
    await ctx.store.flagLicense(license.id);
    return err(409, "seat limit reached — too many devices on this license");
  }
  await ctx.store.touchDevice(license.id, deviceId, nowSec(ctx));

  const user = await ctx.store.userById(userId);
  if (!user) return err(500, "store error");
  const overrides = await ctx.store.overridesForUser(userId);
  const grants = overrides.filter((o) => o.granted).map((o) => o.capability);
  const denials = overrides.filter((o) => !o.granted).map((o) => o.capability);

  // Effective plan: license row drives the tier directly (one-time purchase,
  // no subscription renewal). Trial auto-downgrades on expiry.
  let plan: Plan = license.plan;
  if (plan === "Trial" && user.trial_expires_at !== null && nowSec(ctx) > user.trial_expires_at) {
    plan = "Free";
    await ctx.store.updateUserPlan(userId, "Free", "expired");
  }
  return ok(issueToken(ctx, plan, deviceId, grants, denials));
}

// ---------------------------------------------------------------------------
// POST /license/heartbeat — periodic re-validation + remote revoke
// ---------------------------------------------------------------------------

export async function heartbeat(
  ctx: Ctx,
  input: { license_key?: string; device_id?: string; session_token?: string; token?: string },
): Promise<HandlerResult> {
  const key = (input.license_key ?? "").trim();
  const deviceId = (input.device_id ?? "").trim();

  const { userId, error } = await checkSession(ctx, input.session_token ?? "");
  if (error || userId === undefined) return error ?? err(401, "unauthorized");

  // Sanity: the client's cached token must be ours and bound to this device.
  if (input.token) {
    const clientToken = validateDelimitedToken(input.token);
    if (!clientToken) {
      return err(401, "client token failed verification");
    }
    const pub = ed25519PubHex(ctx.signingKeyHex);
    const claims = verifyToken(clientToken, pub);
    if (!claims || claims.device_id !== deviceId) {
      return err(401, "client token failed verification");
    }
  }

  const license = await ctx.store.licenseByKey(key);
  if (!license) return err(401, "unknown license key");
  if (license.user_id !== userId) return err(403, "license belongs to another account");
  if (license.revoked) {
    // Remote revocation is the point of the heartbeat (spec §4.6).
    return ok({ token: "", expires_at: 0, revoked: true });
  }

  await ctx.store.touchDevice(license.id, deviceId, nowSec(ctx));
  const user = await ctx.store.userById(userId);
  if (!user) return err(500, "store error");
  const overrides = await ctx.store.overridesForUser(userId);
  const grants = overrides.filter((o) => o.granted).map((o) => o.capability);
  const denials = overrides.filter((o) => !o.granted).map((o) => o.capability);

  // Effective plan mirrors validate(): trial auto-downgrades on expiry.
  let plan: Plan = license.plan;
  if (plan === "Trial" && user.trial_expires_at !== null && nowSec(ctx) > user.trial_expires_at) {
    plan = "Free";
    await ctx.store.updateUserPlan(userId, "Free", "expired");
  }

  // Rotate the session: sessions expire after sessionTtlSecs (24h), and a
  // continuously-running client heartbeats every 6h — without rotation the
  // session would die after ~24h and every later heartbeat would 401,
  // silently dropping a valid license to Free. The response carries the new
  // session token so the client updates its cache.
  const session: SessionRow = {
    token: randomHex(32),
    user_id: userId,
    expires_at: nowSec(ctx) + ctx.sessionTtlSecs,
  };
  await ctx.store.createSession(session);
  await ctx.store.deleteSession(input.session_token ?? "");

  return ok({
    ...issueToken(ctx, plan, deviceId, grants, denials),
    session_token: session.token,
    revoked: false,
  });
}

function ed25519PubHex(seedHex: string): string {
  return bytesToHex(ed25519.getPublicKey(hexToBytes(seedHex)));
}

// ---------------------------------------------------------------------------
// POST /payments/webhook — Whop payment events, purchase/plan updates
// ---------------------------------------------------------------------------

interface WhopWebhook {
  event?: string;
  data?: {
    id?: string;
    customer_email?: string;
    customer_id?: string;
    product_id?: string;
    status?: string;
    metadata?: {
      texelbox_user_id?: string;
      is_trial?: string;
    };
  };
}

export async function webhook(
  ctx: Ctx,
  signatureHeader: string,
  rawBody: Uint8Array,
): Promise<HandlerResult> {
  if (!verifyWhopSignature(signatureHeader, rawBody, ctx.webhookSecret)) {
    return err(401, "invalid webhook signature");
  }
  let event: WhopWebhook;
  try {
    event = JSON.parse(new TextDecoder().decode(rawBody)) as WhopWebhook;
  } catch {
    return err(400, "malformed event");
  }
  const evt = event.event ?? "";
  const data = event.data ?? {};
  const metadata = data.metadata ?? {};
  const customerId = typeof data.customer_id === "string" ? data.customer_id : null;
  const purchaseId = typeof data.id === "string" ? data.id : null;
  const customerEmail = typeof data.customer_email === "string" ? data.customer_email : null;
  const status = typeof data.status === "string" ? data.status : "";
  const texelboxUserIdRaw = typeof metadata.texelbox_user_id === "string" ? metadata.texelbox_user_id : null;
  const texelboxUserId = texelboxUserIdRaw ? parseInt(texelboxUserIdRaw, 10) : null;

  // Map Whop payment events to plan changes.
  // payment.succeeded / payment.authorized → Pro (or Trial if status is trial)
  // payment.canceled / payment.failed → Free
  // payment.created / payment.pending → ignored (transitional states)
  let plan: Plan | null = null;
  let planStatus: string | null = null;

  if (evt === "payment.succeeded" || evt === "payment.authorized") {
    if (status === "trial") {
      plan = "Trial";
      planStatus = "trial";
    } else {
      plan = "Pro";
      planStatus = "purchased";
    }
  } else if (evt === "payment.canceled" || evt === "payment.failed") {
    plan = "Free";
    planStatus = "cancelled";
  } else {
    // payment.created, payment.pending, or any other event → ignore
    return ok({ ignored: true });
  }

  // Identify the user: prefer texelbox_user_id from checkout metadata (reliable),
  // fall back to email match only if metadata is missing.
  let userId: number | null = null;
  if (texelboxUserId && Number.isInteger(texelboxUserId)) {
    const u = await ctx.store.userById(texelboxUserId);
    if (u) userId = u.id;
  }
  if (!userId && customerEmail) {
    const u = await ctx.store.userByEmail(customerEmail);
    if (u) userId = u.id;
  }
  if (!userId) {
    console.warn(`[webhook] could not resolve user for payment ${purchaseId}`);
    return ok({ ignored: true, reason: "unknown user" });
  }

  // Link the processor customer/purchase to the user, then apply the plan.
  if (customerId && purchaseId) {
    await ctx.store.linkPurchaseById(userId, customerId, purchaseId);
  }
  await ctx.store.updateUserPlan(userId, plan, planStatus);
  if (plan === "Trial") {
    const expiresAt = Math.floor(Date.now() / 1000) + ctx.trialTtlSecs;
    await ctx.store.setTrialExpires(userId, expiresAt);
  }
  return ok({ updated: true });
}

// ---------------------------------------------------------------------------
// GET /app/version — latest-release info for the client's auto-update check
// (spec §9 Phase 13). Config-driven; unauthenticated on purpose (read-only,
// no sensitive data — just a version string + download URL).
// ---------------------------------------------------------------------------

export async function appVersion(ctx: Ctx): Promise<HandlerResult> {
  if (!ctx.latestVersion) return err(404, "no release configured");
  return ok({ version: ctx.latestVersion, url: ctx.latestDownloadUrl ?? "" });
}

// ---------------------------------------------------------------------------
// GET /user/profile — session-protected account summary (JSON, desktop app)
// ---------------------------------------------------------------------------

export async function profile(ctx: Ctx, authorizationHeader: string): Promise<HandlerResult> {
  const bearer = authorizationHeader.startsWith("Bearer ")
    ? authorizationHeader.slice(7)
    : "";
  const { userId, error } = await checkSession(ctx, bearer);
  if (error || userId === undefined) return error ?? err(401, "unauthorized");

  const user = await ctx.store.userById(userId);
  if (!user) return err(500, "store error");
  const licenseId = await ctx.store.licenseIdForUser(userId);
  const devices = licenseId === null ? [] : await ctx.store.devicesForLicense(licenseId);
  return ok({
    email_sha256: user.email_sha256,
    plan: user.plan,
    status: user.status,
    licensed_devices: devices.length,
  });
}

// ---------------------------------------------------------------------------
// Account / billing (browser pages) — HTML rendered by `index.ts`
// ---------------------------------------------------------------------------

export interface AccountView {
  email: string;
  plan: string;
  status: string;
  licensedDevices: number;
  hasPurchase: boolean;
  verified: boolean;
  licenseKey: string | null;
}

export async function account(ctx: Ctx, authorizationHeader: string): Promise<HandlerResult> {
  const { userId, error } = await checkSession(ctx, authorizationHeader);
  if (error || userId === undefined) return error ?? err(401, "unauthorized");
  const user = await ctx.store.userById(userId);
  if (!user) return err(500, "store error");
  const licenseId = await ctx.store.licenseIdForUser(userId);
  const devices = licenseId === null ? [] : await ctx.store.devicesForLicense(licenseId);
  const licenseKey = licenseId === null ? null : await ctx.store.licenseKeyForId(licenseId);
  return ok({
    email: user.email,
    plan: user.plan,
    status: user.status,
    licensedDevices: devices.length,
    hasPurchase: !!user.purchase_id,
    verified: user.email_verified,
    licenseKey,
  } as AccountView);
}

/** Browser "purchase" entry: returns a redirect to the Whop hosted checkout. */
export async function purchase(
  ctx: Ctx,
  authorizationHeader: string,
  isTrial: boolean,
): Promise<HandlerResult> {
  const { userId, error } = await checkSession(ctx, authorizationHeader);
  if (error || userId === undefined) return error ?? err(401, "unauthorized");
  const user = await ctx.store.userById(userId);
  if (!user) return err(500, "store error");
  if (!ctx.whop || !ctx.appUrl) return err(500, "billing is not configured");

  const planKey = isTrial ? "pro" : "pro";
  const pricing = await ctx.store.getPricing(planKey);
  if (!pricing) return err(500, `no pricing configured for ${planKey}`);

  const redirectUrl = `${ctx.appUrl.replace(/\/$/, "")}/account`;
  const planId = isTrial && pricing.whop_trial_plan_id
    ? pricing.whop_trial_plan_id
    : pricing.whop_plan_id ?? "";
  if (!planId) return err(500, "no plan id configured");

  try {
    const checkoutUrl = await ctx.whop.createCheckout({
      userEmail: user.email,
      redirectUrl,
      planId,
      isTrial,
      texelboxUserId: user.id,
    });
    return ok({ checkout_url: checkoutUrl });
  } catch (e) {
    console.error("[purchase] checkout failed:", e);
    return err(502, `checkout failed: ${e instanceof Error ? e.message : String(e)}`);
  }
}

/** Public pricing (JSON) for the Worker's pricing page. */
export async function pricing(ctx: Ctx): Promise<HandlerResult> {
  const row = await ctx.store.getPricing("pro");
  if (!row) return err(404, "no pricing configured");
  return ok({
    plan: row.plan,
    amount: row.amount,
    currency: row.currency,
    interval: row.interval,
    trialAmount: row.interval === "trial" ? row.amount : null,
  });
}

/** Browser "cancel purchase" entry — resets to Free locally. */
export async function cancelPurchase(
  ctx: Ctx,
  authorizationHeader: string,
): Promise<HandlerResult> {
  const { userId, error } = await checkSession(ctx, authorizationHeader);
  if (error || userId === undefined) return error ?? err(401, "unauthorized");
  const user = await ctx.store.userById(userId);
  if (!user) return err(500, "store error");
  if (!user.purchase_id) return err(400, "no active purchase to cancel");
  await ctx.store.updateUserPlan(userId, "Free", "cancelled");
  return ok({ cancelled: true });
}

// ---------------------------------------------------------------------------
// POST /auth/trial — one-click trial activation (no email/password needed).
// Creates a minimal local user row + trial license and returns a signed
// entitlement token directly.  Does NOT create a Supabase Auth account —
// trial users are anonymous and never need to log in via a browser.
// ---------------------------------------------------------------------------

export async function startTrial(ctx: Ctx): Promise<HandlerResult> {
  // Generate a unique anonymous identity for this trial device.
  // supabase_id must be a valid UUID because the users table column is typed uuid.
  // We generate a random UUID v4 (no external dep — crypto.randomUUID is
  // available in the Workers runtime).
  const supabaseId = crypto.randomUUID();
  const trialId    = randomHex(8);
  const email      = `trial-${trialId}@texelbox.internal`;

  // Create a minimal user row directly in the store — no Supabase Auth call.
  let user;
  try {
    user = await ctx.store.createUser(supabaseId, sha256Hex(email), email);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return err(500, `could not create trial account: ${msg}`);
  }

  const trialSecs = ctx.trialTtlSecs || 86400;
  const expiresAt = nowSec(ctx) + trialSecs;

  await ctx.store.updateUserPlan(user.id, "Trial", "trial");
  await ctx.store.setTrialExpires(user.id, expiresAt);

  const key = `TBX-TRIAL-${trialId.toUpperCase()}`;
  await ctx.store.createLicenseForUser(user.id, key, "Trial", 1);

  const session: SessionRow = {
    token: randomHex(32),
    user_id: user.id,
    expires_at: nowSec(ctx) + ctx.sessionTtlSecs,
  };
  await ctx.store.createSession(session);

  // Issue the signed entitlement token immediately so the desktop app can
  // install it without a separate /license/validate round-trip.
  // device_id is empty here — the client will populate it on first heartbeat.
  const now = nowSec(ctx);
  const claims: import("./token").TokenClaims = {
    plan: "Trial",
    expires_at: expiresAt,
    issued_at: now,
    device_id: "",
    extra_grants: [],
    denials: [],
  };
  const { signToken } = await import("./token");
  const tokenWire = signToken(claims, ctx.signingKeyHex);

  return ok({
    session_token: session.token,
    plan: "Trial",
    status: "trial",
    license_key: key,
    email,
    token: tokenWire,
    trial_expires_at: expiresAt,
  });
}

export { html, type WhopClient };
