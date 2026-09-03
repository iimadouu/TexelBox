/**
 * Persistence layer for the license Worker.
 *
 * `MemoryStore` backs the unit tests; `SupabaseStore` is production
 * (Postgres via supabase-js with the service-role key — worker-side only).
 * Email/password credentials live in Supabase Auth; this DB never stores
 * passwords and only keeps emails hashed (global rule 6).
 */

export type Plan = "Free" | "Pro" | "Trial";

export interface UserRow {
  id: number;
  supabase_id: string;
  email: string;
  email_sha256: string;
  email_verified: boolean;
  plan: Plan;
  status: string; // free | active | trial | purchased | cancelled
  customer_id: string | null; // Whop customer id
  purchase_id: string | null; // Whop purchase / order id
  trial_expires_at: number | null; // unix seconds, null = no trial active
}

export interface LicenseRow {
  id: number;
  user_id: number;
  key: string;
  plan: Plan;
  revoked: boolean;
  flagged: boolean;
  max_seats: number;
  purchased_at: number | null; // unix seconds, null = never purchased (Free)
}

export interface DeviceRow {
  id: number;
  license_id: number;
  device_id: string;
  last_seen: number;
}

export interface SessionRow {
  token: string;
  user_id: number;
  expires_at: number;
}

export interface OverrideRow {
  capability: string;
  granted: boolean;
}

export interface PricingRow {
  plan: string;
  amount: number;
  currency: string;
  interval: string; // once | trial
  whop_plan_id: string | null;
  whop_trial_plan_id: string | null;
  active: boolean;
}

export interface Store {
  userBySupabaseId(supabaseId: string): Promise<UserRow | null>;
  userById(id: number): Promise<UserRow | null>;
  userByCustomerId(customerId: string): Promise<UserRow | null>;
  userByEmail(email: string): Promise<UserRow | null>;
  createUser(supabaseId: string, emailSha256: string, email?: string): Promise<UserRow>;
  licenseByKey(key: string): Promise<LicenseRow | null>;
  licenseIdForUser(userId: number): Promise<number | null>;
  licenseKeyForId(licenseId: number): Promise<string | null>;
  licenseKeyForUser(userId: number): Promise<string | null>;
  createLicenseForUser(userId: number, key: string, plan: Plan, maxSeats: number): Promise<LicenseRow>;
  setEmailVerified(userId: number): Promise<void>;
  linkPurchaseByEmail(email: string, customerId: string, purchaseId: string): Promise<void>;
  linkPurchaseById(userId: number, customerId: string, purchaseId: string): Promise<void>;
  updateUserPlan(userId: number, plan: Plan, status: string): Promise<void>;
  setTrialExpires(userId: number, expiresAt: number | null): Promise<void>;
  overridesForUser(userId: number): Promise<OverrideRow[]>;
  devicesForLicense(licenseId: number): Promise<DeviceRow[]>;
  touchDevice(licenseId: number, deviceId: string, now: number): Promise<void>;
  getPricing(plan: string): Promise<PricingRow | null>;
  revokeLicense(key: string): Promise<void>;
  flagLicense(licenseId: number): Promise<void>;
  recordPurchase(
    customerId: string,
    purchaseId: string,
    plan: Plan,
    status: string,
  ): Promise<void>;
  createSession(session: SessionRow): Promise<void>;
  sessionByToken(token: string): Promise<SessionRow | null>;
  deleteSession(token: string): Promise<void>;
  deleteExpiredSessions(now: number): Promise<void>;
  createPasswordReset(email: string, token: string, expiresAt: number): Promise<void>;
  consumePasswordReset(token: string): Promise<{ email: string } | null>;
}

// ---------------------------------------------------------------------------
// In-memory store (tests / local dev)
// ---------------------------------------------------------------------------

export class MemoryStore implements Store {
  users: UserRow[] = [];
  licenses: LicenseRow[] = [];
  devices: DeviceRow[] = [];
  sessions: SessionRow[] = [];
  overrides: { user_id: number; capability: string; granted: boolean }[] = [];
  pricing: PricingRow[] = [];
  passwordResets: { email: string; token: string; expiresAt: number; used: boolean }[] = [];

  async userBySupabaseId(supabaseId: string): Promise<UserRow | null> {
    return this.users.find((u) => u.supabase_id === supabaseId) ?? null;
  }
  async userById(id: number): Promise<UserRow | null> {
    return this.users.find((u) => u.id === id) ?? null;
  }
  async userByCustomerId(customerId: string): Promise<UserRow | null> {
    return this.users.find((u) => u.customer_id === customerId) ?? null;
  }
  async userByEmail(email: string): Promise<UserRow | null> {
    const e = email.trim().toLowerCase();
    return this.users.find((u) => u.email.toLowerCase() === e) ?? null;
  }
  async createUser(supabaseId: string, emailSha256: string, email = ""): Promise<UserRow> {
    const row: UserRow = {
      id: this.users.length + 1,
      supabase_id: supabaseId,
      email,
      email_sha256: emailSha256,
      email_verified: false,
      plan: "Free",
      status: "free",
      customer_id: null,
      purchase_id: null,
      trial_expires_at: null,
    };
    this.users.push(row);
    return row;
  }
  async licenseByKey(key: string): Promise<LicenseRow | null> {
    return this.licenses.find((l) => l.key === key) ?? null;
  }
  async licenseIdForUser(userId: number): Promise<number | null> {
    return this.licenses.find((l) => l.user_id === userId)?.id ?? null;
  }
  async licenseKeyForId(licenseId: number): Promise<string | null> {
    return this.licenses.find((l) => l.id === licenseId)?.key ?? null;
  }
  async licenseKeyForUser(userId: number): Promise<string | null> {
    return this.licenses.find((l) => l.user_id === userId)?.key ?? null;
  }
  async createLicenseForUser(userId: number, key: string, plan: Plan, maxSeats: number): Promise<LicenseRow> {
    return this.addLicense(userId, key, plan, maxSeats);
  }
  async setEmailVerified(userId: number): Promise<void> {
    const u = this.users.find((x) => x.id === userId);
    if (u) u.email_verified = true;
  }
  async linkPurchaseByEmail(email: string, customerId: string, purchaseId: string): Promise<void> {
    const u = this.users.find((x) => x.email.toLowerCase() === email.trim().toLowerCase());
    if (u) {
      u.customer_id = customerId;
      u.purchase_id = purchaseId;
    }
  }
  async linkPurchaseById(userId: number, customerId: string, purchaseId: string): Promise<void> {
    const u = this.users.find((x) => x.id === userId);
    if (u) {
      u.customer_id = customerId;
      u.purchase_id = purchaseId;
    }
  }
  async updateUserPlan(userId: number, plan: Plan, status: string): Promise<void> {
    const u = this.users.find((x) => x.id === userId);
    if (!u) return;
    u.plan = plan;
    u.status = status;
    for (const l of this.licenses.filter((l) => l.user_id === u.id)) l.plan = plan;
  }
  async setTrialExpires(userId: number, expiresAt: number | null): Promise<void> {
    const u = this.users.find((x) => x.id === userId);
    if (u) u.trial_expires_at = expiresAt;
  }
  async overridesForUser(userId: number): Promise<OverrideRow[]> {
    return this.overrides
      .filter((o) => o.user_id === userId)
      .map(({ capability, granted }) => ({ capability, granted }));
  }
  async getPricing(plan: string): Promise<PricingRow | null> {
    return this.pricing.find((p) => p.plan === plan && p.active) ?? null;
  }
  async devicesForLicense(licenseId: number): Promise<DeviceRow[]> {
    return this.devices.filter((d) => d.license_id === licenseId);
  }
  async touchDevice(licenseId: number, deviceId: string, now: number): Promise<void> {
    const row = this.devices.find(
      (d) => d.license_id === licenseId && d.device_id === deviceId,
    );
    if (row) row.last_seen = now;
    else
      this.devices.push({
        id: this.devices.length + 1,
        license_id: licenseId,
        device_id: deviceId,
        last_seen: now,
      });
  }
  async revokeLicense(key: string): Promise<void> {
    const l = this.licenses.find((x) => x.key === key);
    if (l) l.revoked = true;
  }
  async flagLicense(licenseId: number): Promise<void> {
    const l = this.licenses.find((x) => x.id === licenseId);
    if (l) l.flagged = true;
  }
  async recordPurchase(
    customerId: string,
    purchaseId: string,
    plan: Plan,
    status: string,
  ): Promise<void> {
    const u = this.users.find((x) => x.customer_id === customerId);
    if (!u) return;
    u.plan = plan;
    u.status = status;
    u.customer_id = customerId;
    u.purchase_id = purchaseId;
    u.trial_expires_at = plan === "Trial" ? Date.now() / 1000 + 86400 : null;
    for (const l of this.licenses.filter((l) => l.user_id === u.id)) {
      l.plan = plan;
      l.purchased_at = Date.now() / 1000;
    }
  }
  async createSession(session: SessionRow): Promise<void> {
    this.sessions.push(session);
  }
  async sessionByToken(token: string): Promise<SessionRow | null> {
    return this.sessions.find((s) => s.token === token) ?? null;
  }
  async deleteSession(token: string): Promise<void> {
    this.sessions = this.sessions.filter((s) => s.token !== token);
  }
  async deleteExpiredSessions(now: number): Promise<void> {
    this.sessions = this.sessions.filter((s) => s.expires_at > now);
  }

  async createPasswordReset(email: string, token: string, expiresAt: number): Promise<void> {
    this.passwordResets = this.passwordResets ?? [];
    this.passwordResets.push({ email, token, expiresAt, used: false });
  }
  async consumePasswordReset(token: string): Promise<{ email: string } | null> {
    const row = this.passwordResets?.find((r) => r.token === token);
    if (!row || row.used || row.expiresAt <= Math.floor(Date.now() / 1000)) return null;
    row.used = true;
    return { email: row.email };
  }

  // -- test helpers ----------------------------------------------------------
  addLicense(userId: number, key: string, plan: Plan, maxSeats: number): LicenseRow {
    const norm: Plan = plan === "Pro" || plan === "Trial" ? plan : "Free";
    const row: LicenseRow = {
      id: this.licenses.length + 1,
      user_id: userId,
      key,
      plan: norm,
      revoked: false,
      flagged: false,
      max_seats: maxSeats,
      purchased_at: norm === "Free" ? null : Date.now() / 1000,
    };
    this.licenses.push(row);
    return row;
  }
  addOverride(userId: number, capability: string, granted: boolean): void {
    this.overrides.push({ user_id: userId, capability, granted });
  }
}

// ---------------------------------------------------------------------------
// Supabase (production)
// ---------------------------------------------------------------------------

import type { SupabaseClient } from "@supabase/supabase-js";

export class SupabaseStore implements Store {
  constructor(private sb: SupabaseClient) {}

  private static userFrom(r: Record<string, unknown>): UserRow {
    return {
      id: Number(r.id),
      supabase_id: String(r.supabase_id),
      email: String(r.email ?? ""),
      email_sha256: String(r.email_sha256 ?? ""),
      email_verified: Boolean(r.email_verified),
      plan: (r.plan === "pro" || r.plan === "trial" ? r.plan.charAt(0).toUpperCase() + r.plan.slice(1) : "Free") as Plan,
      status: String(r.status ?? "free"),
      customer_id: (r.customer_id as string | null) ?? null,
      purchase_id: (r.purchase_id as string | null) ?? null,
      trial_expires_at: r.trial_expires_at == null ? null : Number(r.trial_expires_at),
    };
  }

  private static licenseFrom(r: Record<string, unknown>): LicenseRow {
    return {
      id: Number(r.id),
      user_id: Number(r.user_id),
      key: String(r.key),
      plan: (r.plan === "pro" || r.plan === "trial" ? r.plan.charAt(0).toUpperCase() + r.plan.slice(1) : "Free") as Plan,
      revoked: Boolean(r.revoked),
      flagged: Boolean(r.flagged),
      max_seats: Number(r.max_seats ?? 1),
      purchased_at: r.purchased_at == null ? null : Number(r.purchased_at),
    };
  }

  private async one(table: string, eq: [string, unknown]): Promise<Record<string, unknown> | null> {
    const { data, error } = await this.sb.from(table).select("*").eq(eq[0], eq[1]).limit(1);
    if (error) throw new Error(`supabase ${table}: ${error.message}`);
    return (data?.[0] as Record<string, unknown>) ?? null;
  }

  async userBySupabaseId(supabaseId: string): Promise<UserRow | null> {
    const r = await this.one("users", ["supabase_id", supabaseId]);
    return r ? SupabaseStore.userFrom(r) : null;
  }
  async userById(id: number): Promise<UserRow | null> {
    const r = await this.one("users", ["id", id]);
    return r ? SupabaseStore.userFrom(r) : null;
  }
  async userByCustomerId(customerId: string): Promise<UserRow | null> {
    const r = await this.one("users", ["customer_id", customerId]);
    return r ? SupabaseStore.userFrom(r) : null;
  }
  async userByEmail(email: string): Promise<UserRow | null> {
    const r = await this.one("users", ["email", email.trim().toLowerCase()]);
    return r ? SupabaseStore.userFrom(r) : null;
  }
  async createUser(supabaseId: string, emailSha256: string, email = ""): Promise<UserRow> {
    const { data, error } = await this.sb
      .from("users")
      .insert({ supabase_id: supabaseId, email_sha256: emailSha256, email })
      .select()
      .single();
    if (error) throw new Error(`supabase insert users: ${error.message}`);
    return SupabaseStore.userFrom(data as Record<string, unknown>);
  }
  async licenseByKey(key: string): Promise<LicenseRow | null> {
    const r = await this.one("licenses", ["key", key]);
    return r ? SupabaseStore.licenseFrom(r) : null;
  }
  async licenseIdForUser(userId: number): Promise<number | null> {
    const r = await this.one("licenses", ["user_id", userId]);
    return r ? Number(r.id) : null;
  }
  async licenseKeyForId(licenseId: number): Promise<string | null> {
    const r = await this.one("licenses", ["id", licenseId]);
    return r ? String(r.key ?? "") : null;
  }
  async licenseKeyForUser(userId: number): Promise<string | null> {
    const r = await this.one("licenses", ["user_id", userId]);
    return r ? String(r.key ?? "") : null;
  }
  async createLicenseForUser(userId: number, key: string, plan: Plan, maxSeats: number): Promise<LicenseRow> {
    const { data, error } = await this.sb
      .from("licenses")
      .insert({ user_id: userId, key, plan: plan === "Pro" ? "pro" : "free", max_seats: maxSeats })
      .select()
      .single();
    if (error) throw new Error(`supabase insert licenses: ${error.message}`);
    return SupabaseStore.licenseFrom(data as Record<string, unknown>);
  }
  async setEmailVerified(userId: number): Promise<void> {
    const { error } = await this.sb.from("users").update({ email_verified: true }).eq("id", userId);
    if (error) throw new Error(`supabase setEmailVerified: ${error.message}`);
  }
  async linkPurchaseByEmail(email: string, customerId: string, purchaseId: string): Promise<void> {
    const { error } = await this.sb
      .from("users")
      .update({ customer_id: customerId, purchase_id: purchaseId })
      .eq("email", email.trim().toLowerCase());
    if (error) throw new Error(`supabase linkPurchaseByEmail: ${error.message}`);
  }
  async linkPurchaseById(userId: number, customerId: string, purchaseId: string): Promise<void> {
    const { error } = await this.sb
      .from("users")
      .update({ customer_id: customerId, purchase_id: purchaseId })
      .eq("id", userId);
    if (error) throw new Error(`supabase linkPurchaseById: ${error.message}`);
  }
  async updateUserPlan(userId: number, plan: Plan, status: string): Promise<void> {
    const planDb = plan === "Pro" || plan === "Trial" ? plan.toLowerCase() : "free";
    const { error } = await this.sb.from("users").update({ plan: planDb, status }).eq("id", userId);
    if (error) throw new Error(`supabase updateUserPlan: ${error.message}`);
    const { error: e2 } = await this.sb.from("licenses").update({ plan: planDb }).eq("user_id", userId);
    if (e2) throw new Error(`supabase updateUserPlan licenses: ${e2.message}`);
  }
  async setTrialExpires(userId: number, expiresAt: number | null): Promise<void> {
    const { error } = await this.sb.from("users").update({ trial_expires_at: expiresAt }).eq("id", userId);
    if (error) throw new Error(`supabase setTrialExpires: ${error.message}`);
  }
  async overridesForUser(userId: number): Promise<OverrideRow[]> {
    const { data, error } = await this.sb
      .from("capability_overrides")
      .select("capability, granted")
      .eq("user_id", userId);
    if (error) throw new Error(`supabase overrides: ${error.message}`);
    return (data ?? []).map((r) => ({
      capability: String(r.capability),
      granted: Boolean(r.granted),
    }));
  }
  async getPricing(plan: string): Promise<PricingRow | null> {
    const r = await this.one("pricing", ["plan", plan]);
    if (!r) return null;
    return {
      plan: String(r.plan),
      amount: Number(r.amount ?? 0),
      currency: String(r.currency ?? "usd"),
      interval: String(r.interval ?? "once"),
      whop_plan_id: (r.whop_plan_id as string | null) ?? null,
      whop_trial_plan_id: (r.whop_trial_plan_id as string | null) ?? null,
      active: Boolean(r.active),
    };
  }
  async devicesForLicense(licenseId: number): Promise<DeviceRow[]> {
    const { data, error } = await this.sb
      .from("devices")
      .select("*")
      .eq("license_id", licenseId);
    if (error) throw new Error(`supabase devices: ${error.message}`);
    return (data ?? []).map((r) => ({
      id: Number(r.id),
      license_id: Number(r.license_id),
      device_id: String(r.device_id),
      last_seen: Number(r.last_seen),
    }));
  }
  async touchDevice(licenseId: number, deviceId: string, now: number): Promise<void> {
    const { error } = await this.sb
      .from("devices")
      .upsert(
        { license_id: licenseId, device_id: deviceId, last_seen: now },
        { onConflict: "license_id,device_id" },
      );
    if (error) throw new Error(`supabase upsert devices: ${error.message}`);
  }
  async revokeLicense(key: string): Promise<void> {
    const { error } = await this.sb.from("licenses").update({ revoked: true }).eq("key", key);
    if (error) throw new Error(`supabase revoke: ${error.message}`);
  }
  async flagLicense(licenseId: number): Promise<void> {
    const { error } = await this.sb.from("licenses").update({ flagged: true }).eq("id", licenseId);
    if (error) throw new Error(`supabase flag: ${error.message}`);
  }
  async recordPurchase(
    customerId: string,
    purchaseId: string,
    plan: Plan,
    status: string,
  ): Promise<void> {
    const planDb = plan === "Pro" || plan === "Trial" ? plan.toLowerCase() : "free";
    const user = await this.userByCustomerId(customerId);
    if (!user) return;
    const { error } = await this.sb
      .from("users")
      .update({
        plan: planDb,
        status,
        customer_id: customerId,
        purchase_id: purchaseId,
        ...(plan === "Trial" ? { trial_expires_at: Date.now() / 1000 + 86400 } : { trial_expires_at: null }),
      })
      .eq("id", user.id);
    if (error) throw new Error(`supabase recordPurchase users: ${error.message}`);
    const { error: e2 } = await this.sb
      .from("licenses")
      .update({ plan: planDb, purchased_at: Date.now() / 1000 })
      .eq("user_id", user.id);
    if (e2) throw new Error(`supabase recordPurchase licenses: ${e2.message}`);
  }
  async createSession(session: SessionRow): Promise<void> {
    const { error } = await this.sb.from("sessions").insert({
      token: session.token,
      user_id: session.user_id,
      expires_at: session.expires_at,
    });
    if (error) throw new Error(`supabase insert sessions: ${error.message}`);
  }
  async sessionByToken(token: string): Promise<SessionRow | null> {
    const r = await this.one("sessions", ["token", token]);
    if (!r) return null;
    return {
      token: String(r.token),
      user_id: Number(r.user_id),
      expires_at: Number(r.expires_at),
    };
  }
  async deleteSession(token: string): Promise<void> {
    const { error } = await this.sb.from("sessions").delete().eq("token", token);
    if (error) throw new Error(`supabase delete session: ${error.message}`);
  }
  async deleteExpiredSessions(now: number): Promise<void> {
    const { error } = await this.sb.from("sessions").delete().lte("expires_at", now);
    if (error) throw new Error(`supabase delete sessions: ${error.message}`);
  }
  async createPasswordReset(email: string, token: string, expiresAt: number): Promise<void> {
    const { error } = await this.sb
      .from("password_resets")
      .insert({ email: email.trim().toLowerCase(), token, expires_at: expiresAt, used: false });
    if (error) throw new Error(`supabase insert password_resets: ${error.message}`);
  }
  async consumePasswordReset(token: string): Promise<{ email: string } | null> {
    const r = await this.one("password_resets", ["token", token]);
    if (!r) return null;
    if (Boolean(r.used) || Number(r.expires_at) <= Math.floor(Date.now() / 1000)) return null;
    await this.sb.from("password_resets").update({ used: true }).eq("token", token);
    return { email: String(r.email) };
  }
}
