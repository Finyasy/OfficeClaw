create index if not exists idx_audit_tenant_user_time
on audit_events (tenant_id, user_id, created_at desc);

create index if not exists idx_approvals_lookup
on approvals (tenant_id, user_id, status, expires_at);

create index if not exists idx_sessions_updated
on sessions (updated_at desc);

create index if not exists idx_convo_refs_updated
on conversation_refs (updated_at desc);
