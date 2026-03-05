create table if not exists tenants (
  tenant_id text primary key,
  created_at timestamptz not null default now()
);

create table if not exists users (
  tenant_id text not null references tenants(tenant_id),
  user_id text not null,
  display_name text,
  created_at timestamptz not null default now(),
  primary key (tenant_id, user_id)
);

create table if not exists conversation_refs (
  tenant_id text not null,
  user_id text not null,
  channel text not null,
  conversation_id text not null,
  ref_json jsonb not null,
  updated_at timestamptz not null default now(),
  primary key (tenant_id, user_id, channel, conversation_id)
);

create table if not exists sessions (
  tenant_id text not null,
  user_id text not null,
  channel text not null,
  conversation_id text not null,
  state_json jsonb not null default '{}'::jsonb,
  updated_at timestamptz not null default now(),
  primary key (tenant_id, user_id, channel, conversation_id)
);

create table if not exists oauth_tokens (
  tenant_id text not null,
  user_id text not null,
  provider text not null,
  encrypted_blob bytea not null,
  key_version text not null,
  updated_at timestamptz not null default now(),
  primary key (tenant_id, user_id, provider)
);

create type approval_status as enum (
  'PENDING',
  'APPROVED',
  'REJECTED',
  'EXPIRED',
  'CANCELLED',
  'EXECUTED',
  'FAILED'
);

create table if not exists approvals (
  approval_id uuid primary key,
  tenant_id text not null,
  user_id text not null,
  channel text not null,
  conversation_id text not null,
  kind text not null,
  status approval_status not null,
  risk_level text not null,
  payload_json jsonb not null,
  policy_snapshot_json jsonb not null,
  expires_at timestamptz not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table if not exists audit_events (
  event_id uuid primary key,
  tenant_id text not null,
  user_id text not null,
  channel text not null,
  conversation_id text not null,
  correlation_id text not null,
  event_type text not null,
  event_json jsonb not null,
  created_at timestamptz not null default now()
);
