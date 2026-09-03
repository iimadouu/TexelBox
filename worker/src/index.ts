/**
 * Cloudflare Worker entrypoint — routes the license + billing endpoints (spec §5)
 * and serves the free, Worker-hosted account pages.
 *
 * Auth: Supabase Auth (email+password) verified at /auth/login; this Worker
 * then issues its own opaque session (cookie for the browser, Bearer token for
 * the desktop app) so the 6-hour heartbeat isn't bound to Supabase's ~1h JWT.
 * DB: Supabase Postgres via supabase-js (service-role key, worker-only).
 * Payments: Whop (one-time payments). Email: Brevo SMTP API (HTTPS) — Workers
 * cannot open raw SMTP sockets, so we use Brevo's HTTPS SMTP API instead.
 */
import { createClient, type SupabaseClient } from "@supabase/supabase-js";
import type { Env } from "./env";
import { MemoryStore, SupabaseStore, type Store } from "./store";
import {
  account,
  appVersion,
  cancelPurchase,
  forgotPassword,
  html,
  heartbeat,
  login,
  profile,
  pricing,
  purchase,
  resendVerification,
  resetPassword,
  resetPasswordPage,
  sessionCookie,
  sessionTokenFromCookie,
  signup,
  startTrial,
  validate,
  verifyEmail,
  webhook,
  type AuthAdmin,
  type AuthClient,
  type Ctx,
  type HandlerResult,
  type Mailer,
} from "./handlers";
import { makeBrevoMailer } from "./email";
import { makeWhopClient } from "./whop";
import {
  accountPage,
  featuresPage,
  forgotPasswordPage,
  loginPage,
  messagePage,
  pricingPage,
  robotsTxt,
  sitemapXml,
  signupPage,
} from "./pages";
import { welcomePage } from "./welcome";
import { MAX_REQUEST_BODY_BYTES, RateLimiter } from "./validation";

// Rate limiters (in-memory, per Worker instance)
const authRateLimiter = new RateLimiter(5 * 60_000, 10);   // 10 auth attempts / 5 min / IP
const apiRateLimiter = new RateLimiter(60_000, 120);        // 120 API calls / min / IP
const authPaths = new Set(["/login", "/signup", "/forgot-password", "/purchase", "/cancel-purchase", "/resend-verification"]);
const apiPaths = new Set(["/auth/login", "/auth/trial", "/license/validate", "/license/heartbeat", "/api/signup", "/api/forgot-password", "/api/reset-password", "/api/account"]);

function clientIp(req: Request): string {
  return req.headers.get("cf-connecting-ip") ?? req.headers.get("x-forwarded-for")?.split(",")[0]?.trim() ?? "unknown";
}

class SupabaseAuth implements AuthClient {
  constructor(private sb: SupabaseClient) {}
  async signIn(email: string, password: string): Promise<{ userId: string; email: string }> {
    const { data, error } = await this.sb.auth.signInWithPassword({ email, password });
    if (error || !data.user) throw new Error(`auth failed: ${error?.message ?? "no user returned"}`);
    return { userId: data.user.id, email: data.user.email ?? email };
  }
}

class SupabaseAuthAdmin implements AuthAdmin {
  constructor(private sb: SupabaseClient) {}
  async createUser(email: string, password: string): Promise<{ userId: string; email: string }> {
    const { data, error } = await this.sb.auth.admin.createUser({
      email,
      password,
      email_confirm: true,
    });
    if (error || !data.user) throw new Error(`admin create failed: ${error?.message ?? "no user returned"}`);
    return { userId: data.user.id, email: data.user.email ?? email };
  }
  async updateUserPassword(userId: string, password: string): Promise<void> {
    const { error } = await this.sb.auth.admin.updateUserById(userId, { password });
    if (error) throw new Error(`admin update password failed: ${error.message}`);
  }
}

function buildCtx(env: Env, storeOverride?: Store): Ctx {
  let store: Store;
  let sb: SupabaseClient | undefined;
  if (storeOverride) {
    store = storeOverride;
  } else if (env.SUPABASE_URL.includes("YOUR-PROJECT")) {
    throw new Error("SUPABASE_URL not configured");
  } else {
    sb = createClient(env.SUPABASE_URL, env.SUPABASE_SERVICE_ROLE_KEY, {
      auth: { persistSession: false, autoRefreshToken: false },
    });
    store = new SupabaseStore(sb);
  }
  const sbAnon = createClient(env.SUPABASE_URL, env.SUPABASE_ANON_KEY, {
    auth: { persistSession: false, autoRefreshToken: false },
  });
  const brevoKey = env.BREVO_API_KEY;
  const brevoEmail = env.BREVO_FROM_EMAIL;
  console.log(`[buildCtx] BREVO_API_KEY set: ${!!brevoKey}, length: ${brevoKey?.length ?? 0}`);
  console.log(`[buildCtx] BREVO_FROM_EMAIL set: ${!!brevoEmail}, value: ${brevoEmail ?? "undefined"}`);
  console.log(`[buildCtx] APP_PUBLIC_URL: ${env.APP_PUBLIC_URL ?? "undefined"}`);
  const mailer: Mailer | undefined = brevoKey
    ? makeBrevoMailer({
        apiKey: brevoKey,
        fromEmail: brevoEmail,
        fromName: env.BREVO_FROM_NAME ?? "TexelBox",
      })
    : undefined;
  console.log(`[buildCtx] mailer created: ${!!mailer}`);
  const whop = env.WHOP_API_KEY ? makeWhopClient(env.WHOP_API_KEY) : undefined;

  return {
    store,
    auth: new SupabaseAuth(sbAnon),
    authAdmin: storeOverride ? undefined : new SupabaseAuthAdmin(sb ?? sbAnon),
    mailer,
    whop,
    appUrl: env.APP_PUBLIC_URL,
    signingKeyHex: env.TOKEN_SIGNING_KEY,
    webhookSecret: env.WHOP_WEBHOOK_SECRET,
    tokenTtlSecs: Number(env.TOKEN_TTL_SECS ?? 43200),
    sessionTtlSecs: Number(env.SESSION_TTL_SECS ?? 86400),
    trialTtlSecs: Number(env.TRIAL_TTL_SECS ?? 86400),
    latestVersion: env.LATEST_VERSION,
    latestDownloadUrl: env.LATEST_DOWNLOAD_URL,
  };
}

function withCors(h: Record<string, string>): Record<string, string> {
  return {
    ...h,
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type, Authorization, X-Requested-With",
    "Access-Control-Allow-Credentials": "false",
    "Vary": "Origin",
    "X-Content-Type-Options": "nosniff",
    "X-Frame-Options": "DENY",
    "Referrer-Policy": "strict-origin-when-cross-origin",
    "Strict-Transport-Security": "max-age=31536000; includeSubDomains",
  };
}

function respond(r: HandlerResult): Response {
  const headers: Record<string, string> = withCors({ ...(r.headers ?? {}) });
  if (!headers["content-type"]) headers["content-type"] = "application/json";
  if (r.cookies) for (const c of r.cookies) {
    headers["set-cookie"] = c;
  }
  const body = typeof r.body === "string" && headers["content-type"].includes("text/html")
    ? r.body
    : JSON.stringify(r.body);
  return new Response(body, { status: r.status, headers });
}

function redirect(location: string): Response {
  return new Response(null, { status: 302, headers: withCors({ location }) });
}

async function readJson(req: Request): Promise<Record<string, unknown>> {
  try {
    return (await req.json()) as Record<string, unknown>;
  } catch {
    return {};
  }
}

async function readForm(req: Request): Promise<Record<string, string>> {
  try {
    const text = await req.text();
    const out: Record<string, string> = {};
    for (const [k, v] of new URLSearchParams(text)) out[k] = v;
    return out;
  } catch {
    return {};
  }
}

/** Serve a HandlerResult that returns JSON account data as an HTML page. */
function renderAccount(_ctx: Ctx, r: HandlerResult): Response {
  if (r.status === 401) return redirect("/login");
  const v = r.body as {
    email: string;
    plan: string;
    status: string;
    licensedDevices: number;
    hasPurchase: boolean;
    verified: boolean;
    licenseKey: string | null;
  };
  return respond(html(200, accountPage(v)));
}

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    console.log(`[worker] ${req.method} ${new URL(req.url).pathname}`);
    const url = new URL(req.url);
    const path = url.pathname;
    const method = req.method;

     if (method === "OPTIONS") {
       return new Response(null, { status: 204, headers: withCors({}) });
     }

     // --- Body size guard (DoS prevention) ---
     if (method === "POST" || method === "PUT" || method === "PATCH") {
       const contentLength = req.headers.get("content-length");
       if (contentLength && Number(contentLength) > MAX_REQUEST_BODY_BYTES) {
         return respond({ status: 413, body: { error: "request body too large" } });
       }
     }

     // --- Rate limiting ---
     const ip = clientIp(req);
     if (method === "POST" && authPaths.has(path)) {
       if (!authRateLimiter.isAllowed(`${ip}:${path}`)) {
         console.warn(`[rate-limit] auth blocked: ${ip} -> ${path}`);
         return respond({ status: 429, body: { error: "too many requests" } });
       }
     }
     if (method === "POST" && apiPaths.has(path)) {
       if (!apiRateLimiter.isAllowed(ip)) {
         console.warn(`[rate-limit] api blocked: ${ip} -> ${path}`);
         return respond({ status: 429, body: { error: "too many requests" } });
       }
     }

     let ctx: Ctx;
     try {
       ctx = buildCtx(env);
    } catch (e) {
      console.error("[worker] buildCtx failed:", e);
      return respond({ status: 500, body: { error: String(e) } });
    }

    const cookieSession = sessionTokenFromCookie(req.headers.get("cookie"));

    try {
      // ---- Static bot files ----
      if (path === "/robots.txt" && method === "GET") {
        return new Response(robotsTxt(), {
          status: 200,
          headers: { "content-type": "text/plain; charset=utf-8", ...withCors({}) },
        });
      }
      if (path === "/sitemap.xml" && method === "GET") {
        return new Response(sitemapXml(), {
          status: 200,
          headers: { "content-type": "application/xml; charset=utf-8", ...withCors({}) },
        });
      }

      // ---- Browser pages (HTML) --------------------------------------------
      if (path === "/" && method === "GET") return respond(html(200, welcomePage()));
      if (path === "/login" && method === "GET") return respond(html(200, loginPage()));
      if (path === "/signup" && method === "GET") return respond(html(200, signupPage()));
      if (path === "/forgot-password" && method === "GET") return respond(html(200, forgotPasswordPage()));
      if (path === "/pricing" && method === "GET") {
        const pr = await pricing(ctx);
        const view = pr.status === 200 ? (pr.body as { amount: number; currency: string; interval: string }) : undefined;
        return respond(html(200, pricingPage(view)));
      }
      if (path === "/features" && method === "GET") return respond(html(200, featuresPage()));
      if (path === "/logout" && method === "GET") {
          return new Response(null, {
            status: 302,
            headers: withCors({ location: "/", "set-cookie": "tb_session=; Path=/; HttpOnly; Max-Age=0" }),
          });
      }
      if (path === "/verify" && method === "GET") {
        const token = url.searchParams.get("token") ?? "";
        const r = await verifyEmail(ctx, token);
        if (r.status === 200) {
          return respond(html(200, messagePage("Email verified", "Your email address is confirmed. You can now log in and activate TexelBox.", "ok")));
        }
        return respond(html(400, messagePage("Verification failed", "This link is invalid or has expired. Try signing up again.", "err")));
      }
      if (path === "/account" && method === "GET") {
        return renderAccount(ctx, await account(ctx, cookieSession));
      }
      if (path === "/download" && method === "GET") {
        if (ctx.latestDownloadUrl) return redirect(ctx.latestDownloadUrl);
        return respond(html(200, messagePage("No download yet", "A new version has not been published.", "ok")));
      }

      // ---- Browser form actions -------------------------------------------
      if (path === "/login" && method === "POST") {
        const form = await readForm(req);
        const r = await login(ctx, { email: form.email, password: form.password });
        if (r.status !== 200) {
          return respond(html(401, loginPage((r.body as { error: string }).error)));
        }
        const token = (r.body as { session_token: string }).session_token;
        return new Response(null, {
          status: 302,
          headers: { location: "/account", "set-cookie": sessionCookie(ctx, token) },
        });
      }
      if (path === "/signup" && method === "POST") {
        const form = await readForm(req);
        const r = await signup(ctx, { email: form.email, password: form.password });
        if (r.status !== 200) {
          return respond(html(r.status, signupPage((r.body as { error: string }).error)));
        }
        const token = (r.body as { session_token: string }).session_token;
        return new Response(null, {
          status: 302,
          headers: { location: "/account", "set-cookie": sessionCookie(ctx, token) },
        });
      }
      if (path === "/purchase" && method === "GET") {
        const isTrial = url.searchParams.get("trial") === "1";
        const r = await purchase(ctx, cookieSession, isTrial);
        if (r.status !== 200) {
          const msg = (r.body as { error: string }).error;
          return respond(html(r.status, messagePage("Checkout unavailable", msg, "err")));
        }
        const checkoutUrl = (r.body as { checkout_url: string }).checkout_url;
        console.log("[purchase] redirecting to:", checkoutUrl);
        const htmlBody = `<!doctype html><html><head><meta charset="utf-8" />
          <meta http-equiv="refresh" content="0; url=${checkoutUrl.replace(/"/g, "&quot;")}" />
          <title>Redirecting…</title>
          <script>window.location.replace(${JSON.stringify(checkoutUrl)});</script>
          </head><body>Redirecting to checkout…</body></html>`;
        return new Response(htmlBody, { status: 200, headers: { "content-type": "text/html; charset=utf-8" } });
      }
      if (path === "/cancel-purchase" && method === "POST") {
        const r = await cancelPurchase(ctx, cookieSession);
        if (r.status !== 200) {
          const msg = (r.body as { error: string }).error;
          return respond(html(r.status, messagePage("Could not cancel", msg, "err")));
        }
        return redirect("/account");
      }
      if (path === "/resend-verification" && method === "POST") {
        const r = await resendVerification(ctx, req.headers.get("cookie"));
        if (r.status !== 200) {
          const msg = (r.body as { error: string }).error;
          return respond(html(r.status, messagePage("Resend failed", msg, "err")));
        }
        return respond(html(200, messagePage("Email sent", "Check your inbox for the verification link. If you cannot find the link, check your spam folder.", "ok")));
      }
      if (path === "/forgot-password" && method === "POST") {
        const form = await readForm(req);
        const r = await forgotPassword(ctx, { email: form.email });
        if (r.status !== 200) {
          const msg = (r.body as { error: string }).error;
          return respond(html(r.status, messagePage("Reset failed", msg, "err")));
        }
        const body = r.body as { sent: boolean; reason?: string };
        const msg = body.sent
          ? "If an account exists for that email, a password reset link has been sent."
          : (body.reason ?? "If an account exists for that email, a password reset link has been sent.");
        return respond(html(200, messagePage("Check your email", msg, "ok")));
      }
      if (path === "/reset-password" && method === "GET") {
        const token = url.searchParams.get("token") ?? "";
        if (!token) return respond(html(400, messagePage("Bad request", "Missing reset token.", "err")));
        return respond(html(200, resetPasswordPage(token)));
      }
      if (path === "/reset-password" && method === "POST") {
        const form = await readForm(req);
        const r = await resetPassword(ctx, { token: form.token, password: form.password, password2: form.password2 });
        if (r.status !== 200) {
          const msg = (r.body as { error: string }).error;
          return respond(html(r.status, messagePage("Reset failed", msg, "err")));
        }
        return respond(html(200, messagePage("Password updated", "Your password has been changed. You can now log in.", "ok")));
      }

      // ---- Desktop app API (JSON) ------------------------------------------
      if (path === "/auth/login" && method === "POST") {
        return respond(await login(ctx, await readJson(req)));
      }
      if (path === "/auth/trial" && method === "POST") {
        return respond(await startTrial(ctx));
      }
      if (path === "/license/validate" && method === "POST") {
        return respond(await validate(ctx, await readJson(req)));
      }
      if (path === "/license/heartbeat" && method === "POST") {
        return respond(await heartbeat(ctx, await readJson(req)));
      }
      if (path === "/payments/webhook" && method === "POST") {
        const sig = req.headers.get("x-signature") ?? "";
        const raw = new Uint8Array(await req.arrayBuffer());
        return respond(await webhook(ctx, sig, raw));
      }
      if (path === "/user/profile" && method === "GET") {
        return respond(await profile(ctx, req.headers.get("authorization") ?? ""));
      }
      if (path === "/app/version" && method === "GET") {
        return respond(await appVersion(ctx));
      }
      if (path === "/api/signup" && method === "POST") {
        return respond(await signup(ctx, await readJson(req)));
      }
      if (path === "/api/forgot-password" && method === "POST") {
        return respond(await forgotPassword(ctx, await readJson(req)));
      }
      if (path === "/api/reset-password" && method === "POST") {
        return respond(await resetPassword(ctx, await readJson(req)));
      }
      if (path === "/api/account" && method === "GET") {
        return respond(await account(ctx, req.headers.get("authorization") ?? ""));
      }
      return respond({ status: 404, body: { error: "not found" } });
    } catch (e) {
      console.error("[worker] unhandled exception:", e);
      console.error("[worker] stack:", e instanceof Error ? e.stack : "no stack");
      return respond({ status: 500, body: { error: "internal error" } });
    }
  },
};

export { MemoryStore };
