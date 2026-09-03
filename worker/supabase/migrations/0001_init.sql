-- TexelBox license backend schema (spec §5, §4.6) on Supabase Postgres.
-- Run with: supabase db push  (or paste into the SQL editor).
--
-- Privacy: no raw payment data (global rule 6) — only processor ids.
-- Passwords live in Supabase Auth (auth.users), never here; emails are
-- stored hashed. The Worker connects with the service-role key, which
-- bypasses RLS, so these tables are locked down to service-role only.

create table if not exists public.users (
  id              bigint generated always as identity primary key,
  supabase_id     uuid not null unique,                  -- auth.users.id
  email           text not null,                        -- plaintext (this is the user's own account; used for web billing + Whop webhook match)
  email_sha256    text not null,
  email_verified  boolean not null default false,       -- set when the user clicks the verify link
  plan            text not null default 'free',          -- free | pro | trial
  status          text not null default 'free',          -- free | purchased | trial | expired | cancelled
  customer_id     text,                                  -- Whop customer id
  purchase_id     text,                                  -- Whop purchase / order id
  trial_expires_at bigint                                -- unix seconds, null = no trial active
);

create table if not exists public.licenses (
  id          bigint generated always as identity primary key,
  user_id     bigint not null references public.users(id) on delete cascade,
  key         text not null unique,
  plan        text not null default 'free',
  revoked     boolean not null default false,
  flagged     boolean not null default false,            -- abuse flag (spec §4.6)
  max_seats   int not null default 1,
  purchased_at bigint                                   -- unix seconds, null = never purchased
);

create table if not exists public.devices (
  id          bigint generated always as identity primary key,
  license_id  bigint not null references public.licenses(id) on delete cascade,
  device_id   text not null,
  last_seen   bigint not null,
  unique (license_id, device_id)
);

create table if not exists public.sessions (
  token       text primary key,
  user_id     bigint not null references public.users(id) on delete cascade,
  expires_at  bigint not null
);

-- Server-side capability overrides (spec §3): grant/deny per user without a
-- client rebuild. capability = Capability variant name (e.g. 'MapsAoMap').
create table if not exists public.capability_overrides (
  user_id     bigint not null references public.users(id) on delete cascade,
  capability  text not null,
  granted     boolean not null,
  primary key (user_id, capability)
);

create index if not exists devices_license_idx on public.devices (license_id);
create index if not exists sessions_expiry_idx on public.sessions (expires_at);

-- Pricing (one-time purchase + trial). The Pro price is stored here (not hardcoded in
-- the Worker UI) so it can be changed without redeploying. Whop plan ids are
-- optional here; when present they override the wrangler.toml vars for checkout creation.
create table if not exists public.pricing (
  plan             text primary key,                 -- 'pro'
  amount           numeric(10,2) not null,           -- 49.99
  currency         text not null default 'usd',
  interval         text not null default 'once',     -- once | trial
  whop_plan_id text,
  whop_trial_plan_id text,
  active           boolean not null default true
);

insert into public.pricing (plan, amount, currency, interval, active)
values ('pro', 49.99, 'usd', 'once', true)
on conflict (plan) do nothing;

-- Lock everything to the service role (the Worker). The anon/authenticated
-- roles must never read/write license data directly.
create table if not exists public.password_resets (
  id          bigint generated always as identity primary key,
  email       text not null,
  token       text not null unique,
  expires_at  bigint not null,
  used        boolean not null default false,
  created_at  bigint not null default extract(epoch from now())
);

create index if not exists password_resets_token_idx on public.password_resets (token);
create index if not exists password_resets_expires_idx on public.password_resets (expires_at);

alter table public.password_resets enable row level security;

create policy "service_role_only_password_resets" on public.password_resets for all to service_role using (true) with check (true);

-- Lock everything to the service role (the Worker). The anon/authenticated
-- roles must never read/write license data directly.
alter table public.users enable row level security;
alter table public.licenses enable row level security;
alter table public.devices enable row level security;
alter table public.sessions enable row level security;
alter table public.capability_overrides enable row level security;
alter table public.pricing enable row level security;
alter table public.password_resets enable row level security;

-- Service-role bypass is automatic; add explicit deny policies so even a
-- leaked JWT from the app can't touch these tables.
do $$
declare t text;
begin
  foreach t in array array['users','licenses','devices','sessions','capability_overrides','pricing','password_resets']
  loop
    execute format('create policy "service_role_only_%s" on public.%I for all to service_role using (true) with check (true)', t, t);
  end loop;
end $$;
