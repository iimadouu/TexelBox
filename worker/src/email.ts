/**
 * Verification-email sender built on the **Brevo SMTP API** (HTTPS).
 *
 * Why Brevo SMTP API and not raw SMTP? Cloudflare Workers only allow outbound
 * `fetch()` to HTTPS endpoints — they cannot open raw TCP/TLS sockets, so the
 * usual `smtp-relay.brevo.com:465` + password flow is impossible in a Worker.
 * Brevo's SMTP API is plain HTTPS, so it works from the free Worker tier and
 * sends mail through Brevo's relay without any OAuth setup.
 *
 * One-time setup (see docs/server-setup.md §5):
 * 1. Create a Brevo account (free tier: 300 emails/day).
 * 2. Verify your sender domain/email in Brevo → Settings → Sender IDs.
 * 3. Create an API key (Settings → API Keys) and store it as the Worker secret
 *    `BREVO_API_KEY`.
 * 4. Set `BREVO_FROM_EMAIL` in wrangler.toml `[vars]` to the verified sender.
 */

const SEND_URL = "https://api.brevo.com/v3/smtp/email";

export interface BrevoConfig {
  apiKey: string;
  fromEmail: string;
  fromName: string;
}

export interface Mailer {
  send(to: string, subject: string, html: string): Promise<void>;
}

export function makeBrevoMailer(cfg: BrevoConfig): Mailer {
  return {
    async send(to: string, subject: string, html: string): Promise<void> {
      const res = await fetch(SEND_URL, {
        method: "POST",
        headers: {
          "api-key": cfg.apiKey,
          "content-type": "application/json",
        },
        body: JSON.stringify({
          sender: {
            name: cfg.fromName,
            email: cfg.fromEmail,
          },
          to: [{ email: to }],
          subject,
          htmlContent: html,
        }),
      });
      if (!res.ok) {
        const text = await res.text().catch(() => "");
        console.error(`[brevo] send failed: ${res.status} ${text.slice(0, 500)}`);
        throw new Error(`brevo send failed: ${res.status} ${text.slice(0, 200)}`);
      }
      console.log(`[brevo] send ok to=${to} subject=${subject}`);
    },
  };
}
