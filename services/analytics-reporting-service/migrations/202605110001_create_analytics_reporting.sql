CREATE TABLE report_templates (
  id BIGSERIAL PRIMARY KEY,
  code TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  category TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  definition JSONB NOT NULL,
  created_by TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  is_active BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE report_jobs (
  id BIGSERIAL PRIMARY KEY,
  public_id TEXT NOT NULL UNIQUE,
  template_id BIGINT NOT NULL REFERENCES report_templates(id),
  requested_by TEXT NOT NULL,
  params JSONB NOT NULL DEFAULT '{}',
  source_snapshot JSONB NOT NULL DEFAULT '{}',
  rows JSONB NOT NULL DEFAULT '[]',
  state TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  report_version TEXT NOT NULL,
  started_at TIMESTAMPTZ,
  finished_at TIMESTAMPTZ,
  generated_at TIMESTAMPTZ NOT NULL,
  output_uri TEXT,
  output_rows BIGINT NOT NULL DEFAULT 0,
  row_hash CHAR(71) NOT NULL,
  retention_category TEXT NOT NULL,
  retention_expires_at TIMESTAMPTZ NOT NULL,
  error_message TEXT
);

CREATE TABLE report_shares (
  id BIGSERIAL PRIMARY KEY,
  report_job_id BIGINT NOT NULL REFERENCES report_jobs(id),
  shared_to TEXT NOT NULL,
  shared_scope TEXT NOT NULL,
  share_type TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  expires_at TIMESTAMPTZ
);

CREATE TABLE report_audit (
  id BIGSERIAL PRIMARY KEY,
  report_job_id BIGINT NOT NULL REFERENCES report_jobs(id),
  actor_id TEXT NOT NULL,
  action TEXT NOT NULL,
  action_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  ip INET,
  user_agent TEXT,
  details JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX report_jobs_template_id_idx ON report_jobs(template_id);
CREATE INDEX report_jobs_requested_by_idx ON report_jobs(requested_by);
CREATE INDEX report_jobs_template_generated_idx ON report_jobs(template_id, generated_at DESC);
CREATE INDEX report_audit_report_job_id_idx ON report_audit(report_job_id);
CREATE INDEX report_audit_actor_id_idx ON report_audit(actor_id);
