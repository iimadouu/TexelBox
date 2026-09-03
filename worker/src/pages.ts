/**
 * Tiny server-rendered HTML UI for the free, Worker-hosted account area:
 * home, login, signup, pricing, account/billing (with cancel), and status
 * messages. No framework — just static strings with inline CSS so it ships in
 * the single Worker bundle with no extra dependencies.
 */

const SITE_NAME = "TexelBox";
const SITE_URL = "https://texelbox-license.imadedar98.workers.dev";
const DEFAULT_DESCRIPTION = "TexelBox — texture tooling for game developers. Generate normal, height, roughness, and AO maps. Build tileable textures and texture atlases. Native Windows app with free tier and 1-day Pro trial.";
const DEFAULT_KEYWORDS = "texture atlas, tileable textures, seamless textures, normal map generator, roughness map, ambient occlusion AO map, height map, texture tool, game textures, atlas packing, channel packing, DDS compression, TexelBox, build atlas, tileable images, texture generation, PBR textures, texture baking, gamedev tools, Windows";
const OG_IMAGE = "https://raw.githubusercontent.com/iimadouu/TexelBox/master/texelbox.png";

function seo(title: string, description: string, keywords: string, canonical: string, noIndex = false): string {
  const ld = JSON.stringify({
    "@context": "https://schema.org",
    "@type": "WebSite",
    "name": SITE_NAME,
    "url": SITE_URL + "/",
    "description": description,
    "sameAs": ["https://github.com/iimadouu/TexelBox"],
  });
  return `<meta name="description" content="${esc(description)}" />
<meta name="keywords" content="${esc(keywords)}" />
<meta name="google-site-verification" content="evJVZtiWOTEZZ4QAhMWDbK1H5UzQvB6VqmRgHbXI2U0" />
<meta name="author" content="${SITE_NAME}" />
<meta name="robots" content="${noIndex ? "noindex, nofollow" : "index, follow"}, max-snippet:-1, max-image-preview:large, max-video-preview:-1" />
<meta name="googlebot" content="${noIndex ? "noindex, nofollow" : "index, follow"}, max-snippet:-1, max-image-preview:large, max-video-preview:-1" />
<meta name="bingbot" content="${noIndex ? "noindex, nofollow" : "index, follow"}, max-snippet:-1, max-image-preview:large" />
<meta name="theme-color" content="#2563eb" />
<meta name="format-detection" content="telephone=no" />
<link rel="canonical" href="${canonical}" />
<meta property="og:title" content="${esc(title)}" />
<meta property="og:description" content="${esc(description)}" />
<meta property="og:image" content="${OG_IMAGE}" />
<meta property="og:url" content="${canonical}" />
<meta property="og:type" content="website" />
<meta property="og:site_name" content="${SITE_NAME}" />
<meta name="twitter:card" content="summary_large_image" />
<meta name="twitter:title" content="${esc(title)}" />
<meta name="twitter:description" content="${esc(description)}" />
<meta name="twitter:image" content="${OG_IMAGE}" />
<link rel="icon" href="${OG_IMAGE}" sizes="any" />
<script type="application/ld+json">${ld}</script>`;
}

export function shell(title: string, body: string, description: string = DEFAULT_DESCRIPTION, keywords: string = DEFAULT_KEYWORDS, canonical: string = SITE_URL, noIndex: boolean = false): string {
  const fullTitle = `${title} · TexelBox`;
  return `<!doctype html><html lang="en"><head><meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>${fullTitle}</title>
${seo(fullTitle, description, keywords, canonical, noIndex)}
<style>
  :root { color-scheme: light; }
  * { box-sizing: border-box; }
  body { margin:0; font-family: system-ui, Segoe UI, Roboto, sans-serif; background:#f1f5f9; color:#0f172a; }
  .wrap { max-width: 480px; margin: 48px auto; padding: 0 16px; }
  .card { background:#fff; border:1px solid #e2e8f0; border-radius:10px; padding:24px; box-shadow:0 1px 3px rgba(0,0,0,.06); }
  h1 { font-size:22px; margin:0 0 4px; }
  h2 { font-size:18px; margin:0 0 16px; }
  p.sub { color:#64748b; font-size:13px; margin:0 0 18px; }
  label { display:block; font-size:12px; font-weight:600; color:#475569; margin:12px 0 4px; }
  input[type=email], input[type=password], input[type=text] { width:100%; padding:10px; border:1px solid #cbd5e1; border-radius:6px; font-size:14px; }
  button { margin-top:16px; width:100%; padding:10px 14px; border:0; border-radius:6px; background:#2563eb; color:#fff; font-size:14px; font-weight:600; cursor:pointer; }
  button.secondary { background:#e2e8f0; color:#0f172a; }
  button.danger { background:#dc2626; }
  a { color:#2563eb; text-decoration:none; }
  .row { display:flex; gap:10px; }
  .row > * { flex:1; }
  .msg { border-radius:6px; padding:10px 12px; font-size:13px; margin-bottom:14px; }
  .msg.err { background:#fef2f2; color:#b91c1c; border:1px solid #fecaca; }
  .msg.ok { background:#f0fdf4; color:#15803d; border:1px solid #bbf7d0; }
  .pill { display:inline-block; padding:2px 8px; border-radius:999px; font-size:12px; font-weight:700; }
  .pill.pro { background:#dcfce7; color:#166534; }
  .pill.free { background:#e2e8f0; color:#475569; }
  .pill.trial { background:#fef3c7; color:#92400e; }
  .price { font-size: 28px; font-weight: 800; margin: 4px 0 12px; color: #0f172a; }
  .muted { color:#64748b; font-size:12px; }
  .nav { margin-top:16px; font-size:13px; }
</style></head><body><div class="wrap"><div class="card">${body}</div></div></body></html>`;
}

export function esc(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function robotsTxt(): string {
  return `# robots.txt for TexelBox
# https://texelbox-license.imadedar98.workers.dev/

# Default ruleset — allow all crawlers, block auth-only pages
User-agent: *
Allow: /
Disallow: /login
Disallow: /signup
Disallow: /forgot-password
Disallow: /account
Disallow: /reset-password
Disallow: /verify

# --- Search engine bots ---
User-agent: Googlebot
Allow: /

User-agent: Bingbot
Allow: /

User-agent: Applebot
Allow: /

User-agent: YandexBot
Allow: /

User-agent: Baiduspider
Allow: /

User-agent: Bytespider
Allow: /

User-agent: FacebookExternalHit
Allow: /

User-agent: Twitterbot
Allow: /

User-agent: LinkedInBot
Allow: /

# --- AI / LLM training & search bots ---
# Explicitly allowed so AI assistants (Gemini via Google, ChatGPT via OpenAI,
# Claude via Anthropic, Copilot via Bing) can surface TexelBox when users ask
# for texture atlas, tileable image, or map generation software.
User-agent: GPTBot
User-agent: ChatGPT-User
User-agent: OAI-Searchbot
User-agent: ClaudeBot
User-agent: Claude-Web
User-agent: Google-Extended
User-agent: CCBot
User-agent: Applebot-ML
Allow: /

# Sitemap
Sitemap: ${SITE_URL}/sitemap.xml
`;
}

export function sitemapXml(): string {
  const urls = ["/", "/pricing", "/features"].map(
    (path) => `  <url><loc>${SITE_URL}${path}</loc></url>`,
  );
  return `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${urls.join("\n")}
</urlset>`;
}

export function homePage(): string {
  const body = `
    <h1>TexelBox</h1>
    <p class="sub">Free texture tooling for game artists. Log in to activate your license or buy TexelBox Pro.</p>
    <div class="row">
      <a href="/login"><button>Log in</button></a>
      <a href="/signup"><button class="secondary">Create account</button></a>
    </div>
    <div class="nav"><a href="/pricing">View Pro pricing →</a></div>`;
  return shell("TexelBox", body,
    "TexelBox — free texture tooling for game artists. Generate normal, height, roughness, and AO maps. Build tileable textures and texture atlases. Native Windows app. Log in to activate your license or buy TexelBox Pro.",
    DEFAULT_KEYWORDS, `${SITE_URL}/`, false);
}

export function loginPage(error?: string): string {
  const msg = error ? `<div class="msg err">${esc(error)}</div>` : "";
  const body = `
     <h2>Log in</h2>
     ${msg}
      <form method="post" action="/login">
        <label for="email">Email</label>
        <input id="email" name="email" type="email" required autocomplete="username" />
        <label for="password">Password</label>
        <input id="password" name="password" type="password" required autocomplete="current-password" />
        <button type="submit">Log in</button>
      </form>
      <div class="nav"><a href="/signup">Create one</a> · <a href="/forgot-password">Forgot password?</a></div>`;
  return shell("Log in · TexelBox", body,
    "Log in to your TexelBox account. Access Pro features and download the app.",
    DEFAULT_KEYWORDS, `${SITE_URL}/login`, true);
}

export function signupPage(error?: string): string {
  const msg = error ? `<div class="msg err">${esc(error)}</div>` : "";
  const body = `
     <h2>Create account</h2>
     ${msg}
    <form method="post" action="/signup">
       <label for="email">Email</label>
      <input id="email" name="email" type="email" required autocomplete="username" />
       <label for="password">Password</label>
      <input id="password" name="password" type="password" required minlength="8" autocomplete="new-password" />
      <button type="submit">Create account</button>
     </form>
    <p class="muted">We email a verification link and your license key.</p>
     <div class="nav">Already have one? <a href="/login">Log in</a></div>`;
  return shell("Sign up · TexelBox", body,
    "Create a TexelBox account. Get a free license key and start building textures for your game.",
    DEFAULT_KEYWORDS, `${SITE_URL}/signup`, true);
}

export function forgotPasswordPage(): string {
  const body = `
    <h2>Reset your password</h2>
    <p class="sub">Enter the email address associated with your account and we'll send you a link to reset your password.</p>
    <form method="post" action="/forgot-password">
      <label for="email">Email</label>
      <input id="email" name="email" type="email" required autocomplete="email" />
      <button type="submit">Send reset link</button>
    </form>
    <div class="nav"><a href="/login">Back to log in</a></div>`;
  return shell("Forgot password · TexelBox", body,
    "Reset your TexelBox account password. Enter your email to receive a reset link.",
    DEFAULT_KEYWORDS, `${SITE_URL}/forgot-password`, true);
}

export interface PriceView {
  amount: number;
  currency: string;
  interval: string;
  trialAmount?: number | null;
}

function formatPrice(p: PriceView): string {
  const cur = p.currency.toLowerCase() === "usd" ? "$" : `${p.currency.toUpperCase()} `;
  if (p.trialAmount && p.trialAmount > 0) {
    return `${cur}${Number(p.trialAmount).toFixed(2)} for ${p.interval === "trial" ? "1-day trial" : p.interval}`;
  }
  return `${cur}${Number(p.amount).toFixed(2)}`;
}

export function pricingPage(price?: PriceView): string {
  const priceLine = price
    ? `<div class="price">${formatPrice(price)}</div>`
    : `<div class="price">$49.99</div>`;
  const body = `
    <h2>Choose your plan</h2>
    <div class="row" style="gap:12px; margin-bottom:16px;">
      <div class="card" style="flex:1; margin:0;">
        <h3>Free</h3>
        <p class="price" style="font-size:20px;">$0</p>
        <ul style="font-size:13px; padding-left:18px;">
          <li>All 4 maps: normal, height, roughness &amp; AO</li>
          <li>Full slider control, auto-heal, alpha packing</li>
          <li>Atlas up to 32 images, 2048×2048</li>
          <li>Quick Export in every panel</li>
          <li>Undo/redo in tileable (10 steps)</li>
          <li>Up to 25 files per batch</li>
          <li>5 presets per functionality</li>
          <li>PNG + TGA export</li>
          <li>1-day Pro trial included</li>
        </ul>
      </div>
      <div class="card" style="flex:1; margin:0; border-color:#2563eb;">
        <h3>Pro <span class="pill pro">RECOMMENDED</span></h3>
        ${priceLine}
        <ul style="font-size:13px; padding-left:18px;">
          <li>Atlas above 2048px (up to 8192×8192)</li>
          <li>DDS / BC1/BC3/BC5/BC7 compression</li>
          <li>Engine export profiles (Unreal 5, Unity HDRP, Godot 4)</li>
          <li>Channel packing engine presets</li>
          <li>Unlimited batch files + multi-op chains + dry-run</li>
          <li>Unlimited presets + export/import</li>
          <li>Sphere preview + full validation suite</li>
          <li>Lifetime license + all future updates</li>
        </ul>
      </div>
    </div>
    <div class="row">
      <a href="/signup?trial=1"><button class="secondary">Start Free Trial</button></a>
      <a href="/signup"><button>Buy Pro — $49.99</button></a>
    </div>
    <p class="muted">Payments handled by Whop. One-time purchase, no subscription.</p>
    <div class="nav"><a href="/features">View full feature comparison →</a></div>`;
  return shell("Pricing · TexelBox", body,
    "TexelBox pricing — Free forever tier and Pro one-time purchase ($49.99). Lifetime license with all future updates. Start a 1-day Pro trial.",
    DEFAULT_KEYWORDS, `${SITE_URL}/pricing`, false);
}

export function featuresPage(): string {
  const row = (feat: string, free: string, pro: string, highlight = false) => {
    const bg = highlight ? ' style="background:#fffbeb;"' : '';
    return `<tr${bg}><td style="padding:8px 12px; border-bottom:1px solid #f1f5f9;">${feat}</td><td style="text-align:center; padding:8px 12px; border-bottom:1px solid #f1f5f9;">${free}</td><td style="text-align:center; padding:8px 12px; border-bottom:1px solid #f1f5f9; background:#f0f9ff;">${pro}</td></tr>`;
  };
  const section = (icon: string, title: string) =>
    `<tr><td colspan="3" style="padding:12px; font-weight:700; background:#f8fafc; border-bottom:1px solid #e2e8f0;">${icon} ${title}</td></tr>`;
  const yes = "✅";
  const no = "❌";
  const new_ = (s: string) => `${s} <span style="background:#dcfce7;color:#166534;font-size:10px;font-weight:700;padding:1px 5px;border-radius:999px;">NEW</span>`;

  const body = `
    <h2>Free vs Pro — Full Feature Comparison</h2>
    <p class="sub">v0.2.0 — everything TexelBox does, side by side.</p>
    <div style="overflow-x:auto;">
      <table style="width:100%; border-collapse:collapse; font-size:13px; background:#fff; border-radius:8px; overflow:hidden; border:1px solid #e2e8f0;">
        <thead>
          <tr style="background:#f1f5f9;">
            <th style="text-align:left; padding:10px 12px; border-bottom:2px solid #e2e8f0;">Feature</th>
            <th style="text-align:center; padding:10px 12px; border-bottom:2px solid #e2e8f0; width:80px;">Free</th>
            <th style="text-align:center; padding:10px 12px; border-bottom:2px solid #2563eb; width:80px; background:#eff6ff;">Pro</th>
          </tr>
        </thead>
        <tbody>
          ${section("🗺️", "Maps Generation")}
          ${row("Normal map (Scharr 3×3 / Sobel 5×5)", yes, yes)}
          ${row("Height map", yes, yes)}
          ${row("Roughness map", yes, yes)}
          ${row("AO (ambient occlusion) map", yes, yes)}
          ${row("Full slider control (contrast, brightness, blur, strength, kernel)", yes, yes, true)}
          ${row("Height channel selector (R/G/B/Luminance/Alpha)", new_(yes), new_(yes), true)}
          ${row("Normal auto-strength from source contrast", new_(yes), new_(yes), true)}
          ${row("Detail enhance pre-pass (sharper normals on flat photos)", new_(yes), new_(yes), true)}
          ${row("High resolution export (&gt;1024px)", no + ' <span class="muted" style="font-size:11px;">1024px cap</span>', yes)}
          ${row("Batch generation across folder", no, yes)}

          ${section("🔄", "Tileable Texture Prep")}
          ${row("50% offset wrap (expose seam)", yes, yes)}
          ${row("Mirror mode (instant tileability)", yes, yes)}
          ${row("Manual clone/heal brush", yes, yes)}
          ${row("Auto-heal (content-aware seam blend)", yes, yes, true)}
          ${row("Undo/redo (10 steps)", new_(yes), new_(yes), true)}
          ${row("Live 3×3 tiled-repeat preview", no, yes)}
          ${row("Unlimited resolution", no + ' <span class="muted" style="font-size:11px;">2048px cap</span>', yes)}

          ${section("📦", "Channel Packing")}
          ${row("Manual R/G/B/A channel assignment", yes, yes)}
          ${row("Alpha channel as pack target", yes, yes, true)}
          ${row("Custom per-channel image loading", yes, yes)}
          ${row("Engine presets (Unreal ORM, Unity, Godot)", no, yes)}
          ${row("Batch channel-packing across folder", no, yes)}

          ${section("🗂️", "Atlas Texture Generation")}
          ${row("Source images", "32 max", "Unlimited", true)}
          ${row("Max atlas size", "2048×2048", "8192×8192", true)}
          ${row("PNG export", yes, yes)}
          ${row("TGA export", no, yes)}
          ${row("Edge bleed / per-tile padding control", no, yes)}
          ${row("Rotation packing (tighter density)", no, yes)}
          ${row("Trim-sheet mode", no, yes)}
          ${row("JSON UV sidecar", yes, yes)}
          ${row("XML UV sidecar", no, yes)}
          ${row("Priority-based collage arrangement", no, yes)}

          ${section("⚙️", "Format / Resolution Optimization")}
          ${row("PNG / TGA export", yes, yes)}
          ${row("Basic resize (bilinear)", yes, yes)}
          ${row("Power-of-two snap (nearest/up/down)", yes, yes)}
          ${row("Output file size estimate before export", new_(yes), new_(yes), true)}
          ${row("DDS with BC1/BC3/BC5/BC7 compression", no, yes)}
          ${row("Full resampling choice (bilinear/bicubic/Lanczos)", no, yes)}
          ${row("Batch templated export", no, yes)}

          ${section("🚀", "Quick Export")}
          ${row(new_("Quick Export in Maps panel"), new_(yes), new_(yes), true)}
          ${row(new_("Quick Export in Tileable panel"), new_(yes), new_(yes), true)}
          ${row(new_("Quick Export in Packing panel"), new_(yes), new_(yes), true)}
          ${row(new_("Quick Export in Atlas panel"), new_(yes), new_(yes), true)}
          ${row(new_("Last-used folder remembered per panel"), new_(yes), new_(yes), true)}
          ${row(new_("Source image remembered between sessions"), new_(yes), new_(yes), true)}

          ${section("🎮", "Engine Export Profiles (v0.2 New)")}
          ${row(new_("Unreal Engine 5 — ORM pack + T_name_D/ORM/N naming"), no, new_(yes), true)}
          ${row(new_("Unity HDRP — Mask map (R=Metallic, G=AO, B=Detail, A=Smooth)"), no, new_(yes), true)}
          ${row(new_("Godot 4 — standard PBR naming (_albedo/_normal/_roughness/_ao)"), no, new_(yes), true)}

          ${section("👁️", "Quick Preview / Validation")}
          ${row("Plane preview", yes, yes)}
          ${row("One lighting preset", yes, yes)}
          ${row("Basic warning set (errors only)", yes, yes)}
          ${row("Sphere preview", no, yes)}
          ${row("Multiple lighting rigs", no, yes)}
          ${row("Full validation suite (warnings + info)", no, yes)}

          ${section("📁", "Batch Processing")}
          ${row("Single operation across folder", yes, yes)}
          ${row("Max files per run", "25", "Unlimited", true)}
          ${row("Sequential processing", yes, yes)}
          ${row("Multi-operation chains", no, yes)}
          ${row("Dry-run preview (first file only)", no, yes)}
          ${row("Background / parallel processing", no, yes)}

          ${section("💾", "Presets / Profiles")}
          ${row("Saved presets per functionality", "5 max", "Unlimited", true)}
          ${row("Save / load / delete", yes, yes)}
          ${row("Export / import .texelbox-preset files", no, yes)}
          ${row("Project bundles (multi-functionality)", no, yes)}
        </tbody>
      </table>
    </div>
    <div style="margin-top:20px;">
      <div class="row">
        <a href="/signup?trial=1"><button class="secondary">Start Free Trial</button></a>
        <a href="/signup"><button>Buy Pro — $49.99</button></a>
      </div>
    </div>
    <p class="muted" style="margin-top:12px;">One-time purchase. No subscription. Lifetime license includes all future updates.</p>
    <div class="nav"><a href="/pricing">← Back to pricing</a></div>`;
  return shell("Features · TexelBox", body,
    "TexelBox v0.2.0 feature comparison — all four maps (normal, height, roughness, AO) free, Quick Export, undo/redo, engine export profiles for Unreal 5 / Unity HDRP / Godot 4. Free vs Pro details.",
    DEFAULT_KEYWORDS, `${SITE_URL}/features`, false);
}

export interface AccountView {
  email: string;
  plan: string;
  status: string;
  licensedDevices: number;
  hasPurchase: boolean;
  verified: boolean;
  licenseKey: string | null;
}

export function accountPage(v: AccountView) {
  const planPill = v.plan === "Pro"
    ? `<span class="pill pro">PRO</span>`
    : v.plan === "Trial"
      ? `<span class="pill trial">TRIAL</span>`
      : `<span class="pill free">FREE</span>`;
  const verifyNote = v.verified
    ? `<div class="msg ok">Email verified.</div>`
    : `<div class="msg err">Email not verified yet — check your inbox for the verification link. If you cannot find the link, check your spam folder.</div>`;

  const downloadBlock = `<div class="msg ok">
    <strong>Download TexelBox</strong><br/>
    <a href="/download"><button>Download for Windows</button></a>
    <span class="muted">Install then paste your license key in Settings → Activate License.</span>
  </div>`;

  let upgradeBlock: string;
  if (v.plan === "Pro" && v.hasPurchase) {
    upgradeBlock = `<p class="sub">You own TexelBox Pro — lifetime license. Thank you!</p>`;
  } else if (v.plan === "Trial") {
    upgradeBlock = `<p class="sub">Your trial is active. Upgrade to Pro before it expires.</p>
      <a href="/purchase"><button>Buy Pro — $49.99</button></a>`;
  } else {
    upgradeBlock = `<p class="sub">Unlock all features with Pro, or try free for 24 hours.</p>
      <div class="row">
        <a href="/purchase?trial=1"><button class="secondary">Start 1-Day Trial (Free)</button></a>
        <a href="/purchase"><button>Buy Pro — $49.99</button></a>
      </div>`;
  }

  const licenseKeyBlock = v.licenseKey
    ? `<div class="msg ok"><strong>Your license key:</strong> <code>${esc(v.licenseKey)}</code><br/>
        <span class="muted">Paste this into the desktop app: Settings → Activate License.</span></div>`
    : v.hasPurchase
      ? `<div class="msg err">Your purchase is being processed. If your license key does not appear shortly, contact support.</div>`
      : `<div class="msg err">No license key yet. Sign up first to get your free license key.</div>`;

  const resendForm = v.verified
    ? ""
    : `<form method="post" action="/resend-verification" style="margin-top:12px;">
        <button type="submit" class="secondary">Resend verification email</button>
       </form>
       <p class="muted">Didn't receive it? Check your spam folder, or request another link.</p>`;

  const body = `
    <h2>My account ${planPill}</h2>
    <p class="sub">${esc(v.email)} · ${v.licensedDevices} activated device(s)</p>
    ${verifyNote}
    ${downloadBlock}
    ${licenseKeyBlock}
    ${upgradeBlock}
    ${resendForm}
     <div class="nav"><a href="/logout">Log out</a></div>`;
  return shell("Account · TexelBox", body,
    "View your TexelBox account details, license key, activated devices, and subscription status.",
    DEFAULT_KEYWORDS, `${SITE_URL}/account`, true);
}

export function messagePage(title: string, html: string, kind: "ok" | "err"): string {
  const cls = kind === "ok" ? "ok" : "err";
  const body = `<h2>${esc(title)}</h2><div class="msg ${cls}">${html}</div>
    <div class="nav"><a href="/">Back to home</a></div>`;
  return shell(title, body, `TexelBox — ${title}`, DEFAULT_KEYWORDS, `${SITE_URL}/`, true);
}
