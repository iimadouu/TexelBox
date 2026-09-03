/**
 * Entitlement token wire format — MUST stay byte-compatible with the Rust
 * client (`crates/tbx-entitlements/src/token.rs`). The shared regression
 * vector lives in `token.test.ts` + the Rust `cross_language_token_vector`
 * test; both assert the same signed wire string.
 *
 * Wire: base64url_no_pad(claims JSON) + "." + base64url_no_pad(Ed25519 sig)
 * Claims JSON: field order plan, expires_at, issued_at, device_id,
 * extra_grants, denials (serde struct order). Plan serializes as "Free"/
 * "Pro"; capabilities as their enum variant names (e.g. "MapsAoMap").
 */
import { ed25519 } from "@noble/curves/ed25519";

export interface TokenClaims {
  plan: "Free" | "Pro" | "Trial";
  expires_at: number;
  issued_at: number;
  device_id: string;
  extra_grants: string[];
  denials: string[];
}

export function hexToBytes(hex: string): Uint8Array {
  const clean = hex.trim().toLowerCase();
  if (clean.length % 2 !== 0) throw new Error("invalid hex length");
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

export function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

/** RFC 4648 §5, no padding (matches Rust `URL_SAFE_NO_PAD`). */
export function b64url(bytes: Uint8Array): string {
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export function b64urlDecode(s: string): Uint8Array {
  const std = s.replace(/-/g, "+").replace(/_/g, "/");
  const bin = atob(std + "=".repeat((4 - (std.length % 4)) % 4));
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/**
 * Serialize claims with the exact key order serde emits. JSON.stringify
 * preserves string-key insertion order, so building the object literal in
 * declaration order is sufficient.
 */
export function claimsJson(c: TokenClaims): string {
  return JSON.stringify({
    plan: c.plan,
    expires_at: c.expires_at,
    issued_at: c.issued_at,
    device_id: c.device_id,
    extra_grants: c.extra_grants,
    denials: c.denials,
  });
}

/** Sign claims → wire token. `seedHex` is the 32-byte Ed25519 seed. */
export function signToken(c: TokenClaims, seedHex: string): string {
  const raw = new TextEncoder().encode(claimsJson(c));
  const sig = ed25519.sign(raw, hexToBytes(seedHex));
  return `${b64url(raw)}.${b64url(sig)}`;
}

/** Verify a wire token against a public key (32 bytes, hex). */
export function verifyToken(wire: string, pubHex: string): TokenClaims | null {
  const dot = wire.indexOf(".");
  if (dot <= 0) return null;
  try {
    const raw = b64urlDecode(wire.slice(0, dot));
    const sig = b64urlDecode(wire.slice(dot + 1));
    if (!ed25519.verify(sig, raw, hexToBytes(pubHex))) return null;
    const claims = JSON.parse(new TextDecoder().decode(raw)) as TokenClaims;
    if (typeof claims.plan !== "string" || typeof claims.expires_at !== "number") return null;
    return claims;
  } catch {
    return null;
  }
}
