import { describe, expect, it } from "vitest";
import { ed25519 } from "@noble/curves/ed25519";
import { hmac } from "@noble/hashes/hmac";
import { sha256 } from "@noble/hashes/sha256";
import { MemoryStore } from "./store";
import {
  account,
  appVersion,
  cancelPurchase,
  forgotPassword,
  heartbeat,
  login,
  profile,
  pricing,
  resendVerification,
  resetPassword,
  sha256Hex,
  signup,
  startTrial,
  purchase,
  validate,
  verifyEmail,
  webhook,
  type AuthAdmin,
  type AuthClient,
  type Ctx,
  type WhopClient,
  type Mailer,
} from "./handlers";
import { verifyWhopSignature } from "./whop";
import { b64urlDecode, hexToBytes, signToken, verifyToken } from "./token";

/**
 * Same DEV seed the Rust client accepts in debug builds
 * (crates/tbx-entitlements/src/secrets.rs DEV_SIGNING_KEY).
 */
const DEV_SEED_HEX =
  "7e58c13b94026df51ab74e83d9602faa0b71e65c9834ad17f06925cc48b38e05";

class MockAuth implements AuthClient {
  users = new Map<string, string>(); // email -> userId
  constructor() {
    this.users.set("user@texelbox.app", "supa-user-1");
  }
  async signIn(email: string, password: string): Promise<{ userId: string; email: string }> {
    const userId = this.users.get(email.toLowerCase());
    if (!userId || password !== "pw1234") throw new Error("bad credentials");
    return { userId, email };
  }
}

class MockAuthAdmin implements AuthAdmin {
  created = new Map<string, string>(); // email -> userId
  passwords = new Map<string, string>(); // userId -> password
  async createUser(email: string, _password: string): Promise<{ userId: string; email: string }> {
    if (this.created.has(email.toLowerCase())) throw new Error("exists");
    const userId = `supa-${this.created.size + 1}`;
    this.created.set(email.toLowerCase(), userId);
    this.passwords.set(userId, _password);
    return { userId, email };
  }
  async updateUserPassword(userId: string, password: string): Promise<void> {
    this.passwords.set(userId, password);
  }
}

class MockMailer implements Mailer {
  sent: { to: string; subject: string; html: string }[] = [];
  async send(to: string, subject: string, html: string): Promise<void> {
    this.sent.push({ to, subject, html });
  }
}

class MockWhop implements WhopClient {
  lastCheckout: {
    userEmail: string;
    redirectUrl: string;
    planId: string;
    isTrial: boolean;
    texelboxUserId: number;
  } | null = null;
  async createCheckout(input: {
    userEmail: string;
    redirectUrl: string;
    planId: string;
    isTrial: boolean;
    texelboxUserId: number;
  }): Promise<string> {
    this.lastCheckout = input;
    return "https://checkout.whop.com/checkout/mock";
  }
}

function testCtx(store = new MemoryStore()): Ctx {
  return {
    store,
    auth: new MockAuth(),
    authAdmin: new MockAuthAdmin(),
    mailer: new MockMailer(),
    whop: new MockWhop(),
    appUrl: "https://worker.example.com",
    signingKeyHex: DEV_SEED_HEX,
    webhookSecret: "whop_test",
    tokenTtlSecs: 3600,
    sessionTtlSecs: 86400,
    trialTtlSecs: 86400,
  };
}

async function seedPro(ctx: Ctx): Promise<{ session: string; store: MemoryStore }> {
  const lr = await login(ctx, { email: "user@texelbox.app", password: "pw1234" });
  expect(lr.status).toBe(200);
  const session = (lr.body as { session_token: string }).session_token;
  const user = (ctx.store as MemoryStore).users[0];
  user.customer_id = "123";
  user.purchase_id = "pur_1";
  user.plan = "Pro";
  user.status = "purchased";
  (ctx.store as MemoryStore).addLicense(user.id, "TBX-TEST-KEY", "Pro", 2);
  return { session, store: ctx.store as MemoryStore };
}

// ---------------------------------------------------------------------------
// Cross-language regression vector (must match Rust
// cross_language_token_vector in tbx-entitlements/src/token.rs)
// ---------------------------------------------------------------------------

const VECTOR_WIRE =
  "eyJwbGFuIjoiUHJvIiwiZXhwaXJlc19hdCI6MjAwMDAwMDAwMCwiaXNzdWVkX2F0IjoxMDAwMDAwMDAwLCJkZXZpY2VfaWQiOiJ2ZWN0b3ItZGV2aWNlIiwiZXh0cmFfZ3JhbnRzIjpbIk1hcHNBb01hcCJdLCJkZW5pYWxzIjpbXX0." +
  "hGC8iaHZnajAGlunHtsCgBZgfmD___UV65XtNVFoc8e1dkc25yVm2uzneN5XwGe_B7M6MQKPb4xGm2CKz2ZcCQ";

describe("token wire contract", () => {
  it("signs the shared vector identically to the Rust client", () => {
    const wire = signToken(
      {
        plan: "Pro",
        expires_at: 2000000000,
        issued_at: 1000000000,
        device_id: "vector-device",
        extra_grants: ["MapsAoMap"],
        denials: [],
      },
      DEV_SEED_HEX,
    );
    expect(wire).toBe(VECTOR_WIRE);
  });

  it("verifies the shared vector and parses claims", () => {
    const pub = ed25519.getPublicKey(hexToBytes(DEV_SEED_HEX));
    const pubHex = Array.from(pub).map((b) => b.toString(16).padStart(2, "0")).join("");
    const claims = verifyToken(VECTOR_WIRE, pubHex);
    expect(claims).not.toBeNull();
    expect(claims!.plan).toBe("Pro");
    expect(claims!.device_id).toBe("vector-device");
    expect(claims!.extra_grants).toEqual(["MapsAoMap"]);
  });

  it("rejects a forged claims payload", () => {
    const [head, sig] = VECTOR_WIRE.split(".");
    const raw = b64urlDecode(head);
    raw[0] ^= 0xff;
    const forged = `${btoa(String.fromCharCode(...raw)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "")}.${sig}`;
    const pub = ed25519.getPublicKey(hexToBytes(DEV_SEED_HEX));
    const pubHex = Array.from(pub).map((b) => b.toString(16).padStart(2, "0")).join("");
    expect(verifyToken(forged, pubHex)).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// License flow (ported from the Axum server tests)
// ---------------------------------------------------------------------------

describe("license flow", () => {
  it("login rejects bad credentials", async () => {
    const ctx = testCtx();
    const r = await login(ctx, { email: "user@texelbox.app", password: "wrong" });
    expect(r.status).toBe(401);
  });

  it("full login → validate → heartbeat → remote revoke flow", async () => {
    const ctx = testCtx();
    const { session, store } = await seedPro(ctx);

    const v = await validate(ctx, {
      license_key: "TBX-TEST-KEY",
      device_id: "dev-1",
      session_token: session,
    });
    expect(v.status).toBe(200);
    const wire = (v.body as { token: string }).token;
    const pub = Array.from(ed25519.getPublicKey(hexToBytes(DEV_SEED_HEX)))
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");
    const claims = verifyToken(wire, pub);
    expect(claims).not.toBeNull();
    expect(claims!.plan).toBe("Pro");
    expect(claims!.device_id).toBe("dev-1");

    const hb = await heartbeat(ctx, {
      license_key: "TBX-TEST-KEY",
      device_id: "dev-1",
      session_token: session,
      token: wire,
    });
    expect(hb.status).toBe(200);
    const hbBody = hb.body as { token: string; revoked: boolean; session_token: string };
    expect(hbBody.revoked).toBe(false);
    expect(verifyToken(hbBody.token, pub)).not.toBeNull();
    expect(hbBody.session_token).toBeTruthy();
    expect(hbBody.session_token).not.toBe(session);

    await store.revokeLicense("TBX-TEST-KEY");
    const hb2 = await heartbeat(ctx, {
      license_key: "TBX-TEST-KEY",
      device_id: "dev-1",
      session_token: hbBody.session_token,
      token: hbBody.token,
    });
    expect(hb2.status).toBe(200);
    expect((hb2.body as { revoked: boolean }).revoked).toBe(true);
  });

  it("heartbeat refuses to refresh an expired trial", async () => {
    const ctx = testCtx();
    const { session, store } = await seedPro(ctx);
    const license = store.licenses.find((l) => l.key === "TBX-TEST-KEY")!;
    license.purchased_at = Math.floor(Date.now() / 1000) - 172800; // 2 days ago
    license.plan = "Trial";
    const user = store.users[0];
    user.plan = "Trial";
    user.status = "trial";
    user.trial_expires_at = Math.floor(Date.now() / 1000) - 10;
    const pub = Array.from(ed25519.getPublicKey(hexToBytes(DEV_SEED_HEX)))
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");

    const hb = await heartbeat(ctx, {
      license_key: "TBX-TEST-KEY",
      device_id: "dev-1",
      session_token: session,
    });
    expect(hb.status).toBe(200);
    const claims = verifyToken((hb.body as { token: string }).token, pub);
    expect(claims!.plan).toBe("Free");
  });

  it("enforces seat limits and flags shared-key abuse", async () => {
    const ctx = testCtx();
    const { session } = await seedPro(ctx);

    for (const dev of ["dev-a", "dev-b"]) {
      const r = await validate(ctx, {
        license_key: "TBX-TEST-KEY",
        device_id: dev,
        session_token: session,
      });
      expect(r.status).toBe(200);
    }
    const third = await validate(ctx, {
      license_key: "TBX-TEST-KEY",
      device_id: "dev-c",
      session_token: session,
    });
    expect(third.status).toBe(409);
    const lic = await ctx.store.licenseByKey("TBX-TEST-KEY");
    expect(lic!.flagged).toBe(true);
  });

  it("rejects a license belonging to another account", async () => {
    const ctx = testCtx();
    const { store } = await seedPro(ctx);
    (ctx.auth as MockAuth).users.set("other@texelbox.app", "supa-user-2");
    const l2 = await login(ctx, { email: "other@texelbox.app", password: "pw1234" });
    const s2 = (l2.body as { session_token: string }).session_token;
    const r = await validate(ctx, {
      license_key: "TBX-TEST-KEY",
      device_id: "dev-x",
      session_token: s2,
    });
    expect(r.status).toBe(403);
    expect(store.users.length).toBe(2);
  });
});

// ---------------------------------------------------------------------------
// Self-serve signup + email verification (free, Gmail-sent)
// ---------------------------------------------------------------------------

describe("signup + verify", () => {
  it("creates an account, a Free license key, and emails a verify link", async () => {
    const ctx = testCtx();
    const r = await signup(ctx, { email: "New@TexelBox.app", password: "supersecret" });
    expect(r.status).toBe(200);
    const body = r.body as { session_token: string; license_key: string; email_sent: boolean };
    expect(body.session_token).toBeTruthy();
    expect(body.license_key.startsWith("TBX-")).toBe(true);
    expect(body.email_sent).toBe(true);

    const user = await ctx.store.userByEmail("new@texelbox.app");
    expect(user).not.toBeNull();
    expect(user!.email_verified).toBe(false);
    const lic = await ctx.store.licenseByKey(body.license_key);
    expect(lic).not.toBeNull();
    expect(lic!.plan).toBe("Free");

    const mail = (ctx.mailer as MockMailer).sent[0];
    expect(mail.to).toBe("new@texelbox.app");
    const m = /token=([^"]+)/.exec(mail.html);
    expect(m).not.toBeNull();

    const v = await verifyEmail(ctx, m![1]);
    expect(v.status).toBe(200);
    expect((await ctx.store.userByEmail("new@texelbox.app"))!.email_verified).toBe(true);
  });

  it("rejects a weak password", async () => {
    const ctx = testCtx();
    const r = await signup(ctx, { email: "a@b.com", password: "short" });
    expect(r.status).toBe(400);
  });

  it("rejects a duplicate email", async () => {
    const ctx = testCtx();
    await signup(ctx, { email: "dup@texelbox.app", password: "supersecret" });
    const r = await signup(ctx, { email: "dup@texelbox.app", password: "supersecret" });
    expect(r.status).toBe(409);
  });

  it("rejects an expired verification link", async () => {
    const ctx = testCtx();
    ctx.now = () => Date.now() - 2 * 86400_000; // now is "2 days ago"
    await signup(ctx, { email: "exp@texelbox.app", password: "supersecret" });
    const token = /token=([^"]+)/.exec((ctx.mailer as MockMailer).sent[0].html)![1];
    ctx.now = () => Date.now(); // back to real now → link is >24h old
    const v = await verifyEmail(ctx, token);
    expect(v.status).toBe(400);
  });

  it("resends verification email for unverified account", async () => {
    const ctx = testCtx();
    const r = await signup(ctx, { email: "resend@texelbox.app", password: "supersecret" });
    const session = (r.body as { session_token: string }).session_token;
    expect((r.body as { email_sent: boolean }).email_sent).toBe(true);

    const r2 = await resendVerification(ctx, `tb_session=${session}`);
    expect(r2.status).toBe(200);
    expect((r2.body as { sent: boolean }).sent).toBe(true);
    expect((ctx.mailer as MockMailer).sent.length).toBe(2);
  });

  it("rejects resend for already verified account", async () => {
    const ctx = testCtx();
    const r = await signup(ctx, { email: "verified@texelbox.app", password: "supersecret" });
    const session = (r.body as { session_token: string }).session_token;
    const token = /token=([^"]+)/.exec((ctx.mailer as MockMailer).sent[0].html)![1];
    await verifyEmail(ctx, token);

    const r2 = await resendVerification(ctx, `tb_session=${session}`);
    expect(r2.status).toBe(200);
    expect((r2.body as { sent: boolean; reason: string }).reason).toBe("already verified");
  });

  it("rejects resend without session", async () => {
    const ctx = testCtx();
    const r = await resendVerification(ctx, null);
    expect(r.status).toBe(401);
  });

  it("rate-limits resend to once per 60s", async () => {
    const ctx = testCtx();
    const r = await signup(ctx, { email: "rate@texelbox.app", password: "supersecret" });
    const session = (r.body as { session_token: string }).session_token;

    const r1 = await resendVerification(ctx, `tb_session=${session}`);
    expect(r1.status).toBe(200);

    const r2 = await resendVerification(ctx, `tb_session=${session}`);
    expect(r2.status).toBe(429);
  });
});

// ---------------------------------------------------------------------------
// Account / billing (Whop)
// ---------------------------------------------------------------------------

function whopSignature(rawBody: string, secret: string): string {
  const mac = hmac(sha256, new TextEncoder().encode(secret), new TextEncoder().encode(rawBody));
  return Array.from(mac).map((b) => b.toString(16).padStart(2, "0")).join("");
}

describe("whop webhook", () => {
  it("upgrades to Pro on payment.succeeded with a valid signature", async () => {
    const ctx = testCtx();
    const { store } = await seedPro(ctx);
    const user = store.users[0];
    const payload = JSON.stringify({
      event: "payment.succeeded",
      data: { id: "pur_1", customer_id: "123", customer_email: user.email, product_id: "prod_pro", status: "purchased", metadata: { texelbox_user_id: String(user.id) } },
    });
    const r = await webhook(ctx, whopSignature(payload, "whop_test"), new TextEncoder().encode(payload));
    expect(r.status).toBe(200);
    const updated = await store.userById(user.id);
    expect(updated!.status).toBe("purchased");
    expect(updated!.plan).toBe("Pro");
    const lic = await store.licenseByKey("TBX-TEST-KEY");
    expect(lic!.plan).toBe("Pro");
  });

  it("starts a trial on payment.succeeded with trial status", async () => {
    const ctx = testCtx();
    const { store } = await seedPro(ctx);
    const user = store.users[0];
    const payload = JSON.stringify({
      event: "payment.succeeded",
      data: { id: "pur_trial", customer_id: "456", customer_email: user.email, product_id: "prod_trial", status: "trial", metadata: { texelbox_user_id: String(user.id) } },
    });
    const r = await webhook(ctx, whopSignature(payload, "whop_test"), new TextEncoder().encode(payload));
    expect(r.status).toBe(200);
    const updated = await store.userById(user.id);
    expect(updated!.status).toBe("trial");
    expect(updated!.plan).toBe("Trial");
    expect(updated!.trial_expires_at).not.toBeNull();
  });

  it("trial webhook sets trial_expires_at to now + 1 day", async () => {
    const ctx = testCtx();
    const { store } = await seedPro(ctx);
    const user = store.users[0];
    const before = Math.floor(Date.now() / 1000);
    const payload = JSON.stringify({
      event: "payment.succeeded",
      data: { id: "pur_trial2", customer_id: "789", customer_email: user.email, product_id: "prod_trial", status: "trial", metadata: { texelbox_user_id: String(user.id) } },
    });
    const r = await webhook(ctx, whopSignature(payload, "whop_test"), new TextEncoder().encode(payload));
    expect(r.status).toBe(200);
    const updated = await store.userById(user.id);
    expect(updated!.trial_expires_at).not.toBeNull();
    expect(updated!.trial_expires_at).toBeGreaterThanOrEqual(before + 86300);
    expect(updated!.trial_expires_at).toBeLessThanOrEqual(before + 86500);
  });

  it("downgrades to Free on payment.canceled", async () => {
    const ctx = testCtx();
    const { store } = await seedPro(ctx);
    const user = store.users[0];
    const payload = JSON.stringify({
      event: "payment.canceled",
      data: { id: "pur_1", customer_id: "123", customer_email: user.email, product_id: "prod_pro", status: "cancelled", metadata: { texelbox_user_id: String(user.id) } },
    });
    const r = await webhook(ctx, whopSignature(payload, "whop_test"), new TextEncoder().encode(payload));
    expect(r.status).toBe(200);
    const updated = await store.userById(user.id);
    expect(updated!.status).toBe("cancelled");
    expect(updated!.plan).toBe("Free");
  });

  it("rejects a forged signature", async () => {
    const body = new TextEncoder().encode("{}");
    expect(verifyWhopSignature("deadbeef", body, "whop_test")).toBe(false);
    expect(verifyWhopSignature("", body, "whop_test")).toBe(false);
    const good = whopSignature("{}", "whop_test");
    expect(verifyWhopSignature(good, body, "wrong-secret")).toBe(false);
  });

  it("rejects webhooks with an invalid signature over HTTP", async () => {
    const ctx = testCtx();
    const r = await webhook(ctx, "deadbeef", new TextEncoder().encode("{}"));
    expect(r.status).toBe(401);
  });

  it("ignores unrelated event types", async () => {
    const ctx = testCtx();
    const payload = JSON.stringify({ event: "order.created", data: {} });
    const r = await webhook(ctx, whopSignature(payload, "whop_test"), new TextEncoder().encode(payload));
    expect(r.status).toBe(200);
    expect((r.body as { ignored?: boolean }).ignored).toBe(true);
  });

  it("handles payment.succeeded → Pro", async () => {
    const ctx = testCtx();
    const { store } = await seedPro(ctx);
    const user = store.users[0];
    const payload = JSON.stringify({
      event: "payment.succeeded",
      data: { id: "pay_123", customer_email: user.email, customer_id: "cus_123", status: "completed", metadata: { texelbox_user_id: String(user.id) } },
    });
    const r = await webhook(ctx, whopSignature(payload, "whop_test"), new TextEncoder().encode(payload));
    expect(r.status).toBe(200);
    expect((r.body as { updated?: boolean }).updated).toBe(true);
    const updated = await ctx.store.userById(user.id);
    expect(updated?.plan).toBe("Pro");
  });

  it("handles payment.canceled → Free", async () => {
    const ctx = testCtx();
    const { store } = await seedPro(ctx);
    const user = store.users[0];
    const payload = JSON.stringify({
      event: "payment.canceled",
      data: { id: "pay_123", customer_email: user.email, customer_id: "cus_123", metadata: { texelbox_user_id: String(user.id) } },
    });
    const r = await webhook(ctx, whopSignature(payload, "whop_test"), new TextEncoder().encode(payload));
    expect(r.status).toBe(200);
    const updated = await ctx.store.userById(user.id);
    expect(updated?.plan).toBe("Free");
  });

  it("ignores payment.pending and payment.created", async () => {
    const ctx = testCtx();
    const { store } = await seedPro(ctx);
    const user = store.users[0];
    for (const evt of ["payment.pending", "payment.created"]) {
      const payload = JSON.stringify({ event: evt, data: { id: "pay_123", customer_email: user.email, metadata: { texelbox_user_id: String(user.id) } } });
      const r = await webhook(ctx, whopSignature(payload, "whop_test"), new TextEncoder().encode(payload));
      expect(r.status).toBe(200);
      expect((r.body as { ignored?: boolean }).ignored).toBe(true);
    }
    const unchanged = await ctx.store.userById(user.id);
    expect(unchanged?.plan).toBe("Pro");
  });
});

describe("account + purchase", () => {
  it("purchase starts a checkout carrying the user email", async () => {
    const ctx = testCtx();
    const { session, store } = await seedPro(ctx);
    (ctx.store as MemoryStore).pricing.push({
      plan: "pro",
      amount: 29.99,
      currency: "usd",
      interval: "once",
      whop_plan_id: "plan_pro",
      whop_trial_plan_id: "plan_trial",
      active: true,
    });
    const r = await purchase(ctx, session, false);
    expect(r.status).toBe(200);
    const whop = ctx.whop as unknown as MockWhop;
    expect(whop.lastCheckout?.userEmail).toBe("user@texelbox.app");
    expect(whop.lastCheckout?.planId).toBe("plan_pro");
    expect(whop.lastCheckout?.isTrial).toBe(false);
    expect(whop.lastCheckout?.texelboxUserId).toBe(store.users[0].id);
    expect((r.body as { checkout_url: string }).checkout_url).toContain("whop.com");
  });

  it("trial purchase uses the trial product id", async () => {
    const ctx = testCtx();
    const { session } = await seedPro(ctx);
    (ctx.store as MemoryStore).pricing.push({
      plan: "pro",
      amount: 29.99,
      currency: "usd",
      interval: "trial",
      whop_plan_id: "plan_pro",
      whop_trial_plan_id: "plan_trial",
      active: true,
    });
    const r = await purchase(ctx, session, true);
    expect(r.status).toBe(200);
    const whop = ctx.whop as unknown as MockWhop;
    expect(whop.lastCheckout?.planId).toBe("plan_trial");
    expect(whop.lastCheckout?.isTrial).toBe(true);
  });

  it("cancel purchase flips the plan to Free", async () => {
    const ctx = testCtx();
    const { session, store } = await seedPro(ctx);
    await store.recordPurchase("123", "pur_1", "Pro", "purchased");
    const r = await cancelPurchase(ctx, session);
    expect(r.status).toBe(200);
    const user = store.users[0];
    expect(user.plan).toBe("Free");
    expect(user.status).toBe("cancelled");
  });

  it("account summary reports plan + device count + license key", async () => {
    const ctx = testCtx();
    const { session } = await seedPro(ctx);
    await validate(ctx, { license_key: "TBX-TEST-KEY", device_id: "dev-1", session_token: session });
    const r = await account(ctx, session);
    expect(r.status).toBe(200);
    const body = r.body as { plan: string; licensedDevices: number; email: string; licenseKey: string | null };
    expect(body.plan).toBe("Pro");
    expect(body.licensedDevices).toBe(1);
    expect(body.email).toBe("user@texelbox.app");
    expect(body.licenseKey).toBe("TBX-TEST-KEY");
  });
});

// ---------------------------------------------------------------------------
// Trial activation (POST /auth/trial)
// ---------------------------------------------------------------------------

describe("trial activation", () => {
  it("creates a trial account and returns a session + token + key", async () => {
    const ctx = testCtx();
    const r = await startTrial(ctx);
    expect(r.status).toBe(200);
    const body = r.body as { session_token: string; plan: string; status: string; license_key: string; email: string };
    expect(body.session_token).toBeTruthy();
    expect(body.plan).toBe("Trial");
    expect(body.status).toBe("trial");
    expect(body.license_key.startsWith("TBX-TRIAL-")).toBe(true);
    expect(body.email).toMatch(/^trial-[^@]+@texelbox\.app$/);

    const store = ctx.store as MemoryStore;
    const user = store.users[0];
    expect(user.plan).toBe("Trial");
    expect(user.status).toBe("trial");
    expect(user.trial_expires_at).not.toBeNull();

    const lic = store.licenses.find((l) => l.user_id === user.id);
    expect(lic).not.toBeNull();
    expect(lic!.plan).toBe("Trial");
    expect(lic!.key).toBe(body.license_key);
  });

  it("returns 500 when authAdmin is missing", async () => {
    const ctx = testCtx();
    ctx.authAdmin = undefined;
    const r = await startTrial(ctx);
    expect(r.status).toBe(500);
  });
});

// ---------------------------------------------------------------------------
// App version endpoint
// ---------------------------------------------------------------------------

describe("app version", () => {
  it("serves configured release metadata", async () => {
    const ctx = testCtx();
    ctx.latestVersion = "9.9.9";
    ctx.latestDownloadUrl = "https://github.com/you/texelbox/releases/latest";
    const r = await appVersion(ctx);
    expect(r.status).toBe(200);
    expect(r.body).toEqual({ version: "9.9.9", url: "https://github.com/you/texelbox/releases/latest" });
  });

  it("404s when no release is configured", async () => {
    const ctx = testCtx();
    const r = await appVersion(ctx);
    expect(r.status).toBe(404);
  });
});

describe("pricing", () => {
  it("returns the configured Pro price from the database", async () => {
    const ctx = testCtx();
    (ctx.store as MemoryStore).pricing.push({
      plan: "pro",
      amount: 29.99,
      currency: "usd",
      interval: "once",
      whop_plan_id: "plan_pro",
      whop_trial_plan_id: "plan_trial",
      active: true,
    });
    const r = await pricing(ctx);
    expect(r.status).toBe(200);
    const body = r.body as { amount: number; currency: string; interval: string };
    expect(body.amount).toBe(29.99);
    expect(body.currency).toBe("usd");
    expect(body.interval).toBe("once");
  });

  it("404s when no pricing row exists", async () => {
    const ctx = testCtx();
    const r = await pricing(ctx);
    expect(r.status).toBe(404);
  });
});

describe("profile", () => {
  it("requires a session", async () => {
    const ctx = testCtx();
    const r = await profile(ctx, "");
    expect(r.status).toBe(401);
  });

  it("returns plan + device count with a valid bearer session", async () => {
    const ctx = testCtx();
    const { session } = await seedPro(ctx);
    await validate(ctx, {
      license_key: "TBX-TEST-KEY",
      device_id: "dev-1",
      session_token: session,
    });
    const r = await profile(ctx, `Bearer ${session}`);
    expect(r.status).toBe(200);
    const body = r.body as { plan: string; licensed_devices: number; email_sha256: string };
    expect(body.plan).toBe("Pro");
    expect(body.licensed_devices).toBe(1);
    expect(body.email_sha256).toBe(sha256Hex("user@texelbox.app"));
  });

  it("applies server-side capability overrides into tokens", async () => {
    const ctx = testCtx();
    const { session, store } = await seedPro(ctx);
    store.addOverride(store.users[0].id, "MapsAoMap", true);
    store.addOverride(store.users[0].id, "BatchDryRun", false);
    const v = await validate(ctx, {
      license_key: "TBX-TEST-KEY",
      device_id: "dev-1",
      session_token: session,
    });
    const pub = Array.from(ed25519.getPublicKey(hexToBytes(DEV_SEED_HEX)))
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");
    const claims = verifyToken((v.body as { token: string }).token, pub);
    expect(claims!.extra_grants).toEqual(["MapsAoMap"]);
    expect(claims!.denials).toEqual(["BatchDryRun"]);
  });
});

describe("forgotPassword + resetPassword", () => {
  it("rejects invalid email", async () => {
    const ctx = testCtx();
    const r = await forgotPassword(ctx, { email: "bad" });
    expect(r.status).toBe(400);
  });

  it("returns noop for unknown email", async () => {
    const ctx = testCtx();
    const r = await forgotPassword(ctx, { email: "missing@example.com" });
    expect(r.status).toBe(200);
    const body = r.body as { sent: boolean; reason?: string };
    expect(body.sent).toBe(false);
    expect(body.reason).toBe("no account with that email");
  });

  it("creates a reset token and emails the link for a known user", async () => {
    const ctx = testCtx();
    const sr = await signup(ctx, { email: "forgot@texelbox.app", password: "supersecret" });
    expect(sr.status).toBe(200);
    const user = (ctx.store as MemoryStore).users[0];
    const r = await forgotPassword(ctx, { email: user.email });
    expect(r.status).toBe(200);
    const body = r.body as { sent: boolean };
    expect(body.sent).toBe(true);
    const mailer = ctx.mailer as MockMailer;
    const last = mailer.sent[mailer.sent.length - 1];
    expect(last.to).toBe(user.email);
    expect(last.subject).toBe("Reset your TexelBox password");
    expect(last.html).toContain(ctx.appUrl! + "/reset-password?token=");
  });

  it("consumes a valid token and updates the Supabase password", async () => {
    const ctx = testCtx();
    const sr = await signup(ctx, { email: "reset@texelbox.app", password: "supersecret" });
    expect(sr.status).toBe(200);
    const user = (ctx.store as MemoryStore).users[0];
    const fr = await forgotPassword(ctx, { email: user.email });
    const forgotBody = fr.body as { sent: boolean };
    expect(forgotBody.sent).toBe(true);
    const mailer = ctx.mailer as MockMailer;
    const html = mailer.sent[mailer.sent.length - 1].html;
    const token = new URL(html.match(/href="([^"]+)"/)![1]).searchParams.get("token")!;

    const rr = await resetPassword(ctx, { token, password: "newpass1234", password2: "newpass1234" });
    expect(rr.status).toBe(200);
    const resetBody = rr.body as { reset: boolean };
    expect(resetBody.reset).toBe(true);

    const admin = ctx.authAdmin as MockAuthAdmin;
    expect(admin.passwords.get(user.supabase_id)).toBe("newpass1234");
  });

  it("rejects reused or expired tokens", async () => {
    const ctx = testCtx();
    const sr = await signup(ctx, { email: "reuse@texelbox.app", password: "supersecret" });
    expect(sr.status).toBe(200);
    const user = (ctx.store as MemoryStore).users[0];
    const fr = await forgotPassword(ctx, { email: user.email });
    const forgotBody = fr.body as { sent: boolean };
    expect(forgotBody.sent).toBe(true);
    const mailer = ctx.mailer as MockMailer;
    const html = mailer.sent[mailer.sent.length - 1].html;
    const token = new URL(html.match(/href="([^"]+)"/)![1]).searchParams.get("token")!;

    const first = await resetPassword(ctx, { token, password: "newpass1234", password2: "newpass1234" });
    expect(first.status).toBe(200);

    const second = await resetPassword(ctx, { token, password: "another1!", password2: "another1!" });
    expect(second.status).toBe(400);
    const errBody = second.body as { error: string };
    expect(errBody.error).toBe("invalid or expired reset link");
  });
});
