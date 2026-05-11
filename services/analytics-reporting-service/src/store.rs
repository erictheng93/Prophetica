use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};

use crate::{
    error::ReportingError,
    templates::default_templates,
    types::{
        ReportAuditRecord, ReportJob, ReportRole, ReportSchedule, ReportTemplate, RetentionPolicy,
    },
};

#[async_trait]
pub trait ReportingStore: Clone + Send + Sync + 'static {
    async fn templates_for_role(
        &self,
        role: ReportRole,
    ) -> Result<Vec<ReportTemplate>, ReportingError>;
    async fn find_template(&self, code: &str) -> Result<Option<ReportTemplate>, ReportingError>;
    async fn insert_job(
        &self,
        job: ReportJob,
        audit: ReportAuditRecord,
    ) -> Result<(), ReportingError>;
    async fn list_jobs(
        &self,
        template_code: Option<&str>,
    ) -> Result<Vec<ReportJob>, ReportingError>;
    async fn find_job(&self, id: &str) -> Result<Option<ReportJob>, ReportingError>;
    async fn latest_per_template(&self) -> Result<Vec<ReportJob>, ReportingError>;
    async fn insert_audit(&self, audit: ReportAuditRecord) -> Result<(), ReportingError>;
    async fn list_audit_for_job(
        &self,
        job_id: &str,
    ) -> Result<Vec<ReportAuditRecord>, ReportingError>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryStore {
    state: Arc<Mutex<State>>,
}

#[derive(Debug, Clone, Default)]
struct State {
    templates: Vec<ReportTemplate>,
    jobs: Vec<ReportJob>,
    audit: Vec<ReportAuditRecord>,
}

impl InMemoryStore {
    #[must_use]
    pub fn with_seed_templates() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                templates: default_templates(),
                jobs: Vec::new(),
                audit: Vec::new(),
            })),
        }
    }

    #[must_use]
    pub fn job_count(&self) -> usize {
        self.state.lock().map_or(0, |state| state.jobs.len())
    }

    #[must_use]
    pub fn audit_count(&self) -> usize {
        self.state.lock().map_or(0, |state| state.audit.len())
    }
}

#[async_trait]
impl ReportingStore for InMemoryStore {
    async fn templates_for_role(
        &self,
        role: ReportRole,
    ) -> Result<Vec<ReportTemplate>, ReportingError> {
        let state = self
            .state
            .lock()
            .map_err(|error| ReportingError::Store(error.to_string()))?;
        Ok(state
            .templates
            .iter()
            .filter(|template| template.viewer_roles.contains(&role))
            .cloned()
            .collect())
    }

    async fn find_template(&self, code: &str) -> Result<Option<ReportTemplate>, ReportingError> {
        let state = self
            .state
            .lock()
            .map_err(|error| ReportingError::Store(error.to_string()))?;
        Ok(state
            .templates
            .iter()
            .find(|template| template.code == code)
            .cloned())
    }

    async fn insert_job(
        &self,
        job: ReportJob,
        audit: ReportAuditRecord,
    ) -> Result<(), ReportingError> {
        let mut state = self
            .state
            .lock()
            .map_err(|error| ReportingError::Store(error.to_string()))?;
        state.jobs.push(job);
        state.audit.push(audit);
        Ok(())
    }

    async fn list_jobs(
        &self,
        template_code: Option<&str>,
    ) -> Result<Vec<ReportJob>, ReportingError> {
        let state = self
            .state
            .lock()
            .map_err(|error| ReportingError::Store(error.to_string()))?;
        let mut jobs = state
            .jobs
            .iter()
            .filter(|job| template_code.is_none_or(|code| job.template_code == code))
            .cloned()
            .collect::<Vec<_>>();
        jobs.sort_by(|left, right| right.generated_at.cmp(&left.generated_at));
        Ok(jobs)
    }

    async fn find_job(&self, id: &str) -> Result<Option<ReportJob>, ReportingError> {
        let state = self
            .state
            .lock()
            .map_err(|error| ReportingError::Store(error.to_string()))?;
        Ok(state.jobs.iter().find(|job| job.id == id).cloned())
    }

    async fn latest_per_template(&self) -> Result<Vec<ReportJob>, ReportingError> {
        let mut jobs = self.list_jobs(None).await?;
        let mut seen = std::collections::HashSet::new();
        jobs.retain(|job| seen.insert(job.template_code.clone()));
        Ok(jobs)
    }

    async fn insert_audit(&self, audit: ReportAuditRecord) -> Result<(), ReportingError> {
        let mut state = self
            .state
            .lock()
            .map_err(|error| ReportingError::Store(error.to_string()))?;
        state.audit.push(audit);
        Ok(())
    }

    async fn list_audit_for_job(
        &self,
        job_id: &str,
    ) -> Result<Vec<ReportAuditRecord>, ReportingError> {
        let state = self
            .state
            .lock()
            .map_err(|error| ReportingError::Store(error.to_string()))?;
        Ok(state
            .audit
            .iter()
            .filter(|record| record.job_id == job_id)
            .cloned()
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn seed_templates(&self) -> Result<(), ReportingError> {
        for template in default_templates() {
            let definition = TemplateDefinition::from_template(&template);
            sqlx::query(
                r#"
                INSERT INTO report_templates (code, name, category, schema_version, definition, created_by)
                VALUES ($1, $2, $3, $4, $5, 'system')
                ON CONFLICT (code) DO UPDATE
                SET name = EXCLUDED.name,
                    category = EXCLUDED.category,
                    schema_version = EXCLUDED.schema_version,
                    definition = EXCLUDED.definition,
                    updated_at = NOW(),
                    is_active = TRUE
                "#,
            )
            .bind(&template.code)
            .bind(&template.name)
            .bind(&template.category)
            .bind(&template.schema_version)
            .bind(serde_json::to_value(definition).map_err(|error| ReportingError::Store(error.to_string()))?)
            .execute(&self.pool)
            .await
            .map_err(|error| ReportingError::Store(error.to_string()))?;
        }
        Ok(())
    }
}

#[async_trait]
impl ReportingStore for PgStore {
    async fn templates_for_role(
        &self,
        role: ReportRole,
    ) -> Result<Vec<ReportTemplate>, ReportingError> {
        let rows = sqlx::query(
            "SELECT code, name, category, schema_version, definition FROM report_templates WHERE is_active = TRUE ORDER BY code",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| ReportingError::Store(error.to_string()))?;

        rows.into_iter()
            .map(template_from_row)
            .filter(|result| {
                result
                    .as_ref()
                    .map_or(true, |template| template.viewer_roles.contains(&role))
            })
            .collect()
    }

    async fn find_template(&self, code: &str) -> Result<Option<ReportTemplate>, ReportingError> {
        let row = sqlx::query(
            "SELECT code, name, category, schema_version, definition FROM report_templates WHERE code = $1 AND is_active = TRUE",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| ReportingError::Store(error.to_string()))?;

        row.map(template_from_row).transpose()
    }

    async fn insert_job(
        &self,
        job: ReportJob,
        audit: ReportAuditRecord,
    ) -> Result<(), ReportingError> {
        let row = sqlx::query(
            r#"
            INSERT INTO report_jobs (
                public_id, template_id, requested_by, params, source_snapshot, rows, state,
                schema_version, report_version, generated_at, output_rows, row_hash,
                retention_category, retention_expires_at
            )
            SELECT $1, id, $2, $3, $4, $5, $6, $7, $8, $9::timestamptz, $10, $11, $12, $13::timestamptz
            FROM report_templates WHERE code = $14
            RETURNING id
            "#,
        )
        .bind(&job.id)
        .bind(&job.requested_by)
        .bind(serde_json::to_value(&job.params).map_err(|error| ReportingError::Store(error.to_string()))?)
        .bind(serde_json::to_value(&job.source_snapshot).map_err(|error| ReportingError::Store(error.to_string()))?)
        .bind(Value::Array(job.rows.clone()))
        .bind(&job.state)
        .bind(&job.schema_version)
        .bind(&job.version)
        .bind(&job.generated_at)
        .bind(i64::try_from(job.output_rows).map_err(|error| ReportingError::Store(error.to_string()))?)
        .bind(&job.row_hash)
        .bind(format!("{:?}", job.retention.category).to_ascii_lowercase())
        .bind(&job.retention.expires_at)
        .bind(&job.template_code)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| ReportingError::Store(error.to_string()))?;
        let report_job_id: i64 = row
            .try_get("id")
            .map_err(|error| ReportingError::Store(error.to_string()))?;
        insert_audit_row(&self.pool, report_job_id, &audit).await
    }

    async fn list_jobs(
        &self,
        template_code: Option<&str>,
    ) -> Result<Vec<ReportJob>, ReportingError> {
        let rows = if let Some(code) = template_code {
            sqlx::query(JOB_SELECT_WITH_FILTER)
                .bind(code)
                .fetch_all(&self.pool)
                .await
        } else {
            sqlx::query(JOB_SELECT).fetch_all(&self.pool).await
        }
        .map_err(|error| ReportingError::Store(error.to_string()))?;

        rows.into_iter().map(job_from_row).collect()
    }

    async fn find_job(&self, id: &str) -> Result<Option<ReportJob>, ReportingError> {
        sqlx::query(JOB_SELECT_BY_ID)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| ReportingError::Store(error.to_string()))?
            .map(job_from_row)
            .transpose()
    }

    async fn latest_per_template(&self) -> Result<Vec<ReportJob>, ReportingError> {
        let rows = sqlx::query(JOB_SELECT_LATEST_PER_TEMPLATE)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| ReportingError::Store(error.to_string()))?;
        rows.into_iter().map(job_from_row).collect()
    }

    async fn insert_audit(&self, audit: ReportAuditRecord) -> Result<(), ReportingError> {
        let row = sqlx::query("SELECT id FROM report_jobs WHERE public_id = $1")
            .bind(&audit.job_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|error| ReportingError::Store(error.to_string()))?;
        let report_job_id: i64 = row
            .try_get("id")
            .map_err(|error| ReportingError::Store(error.to_string()))?;
        insert_audit_row(&self.pool, report_job_id, &audit).await
    }

    async fn list_audit_for_job(
        &self,
        job_id: &str,
    ) -> Result<Vec<ReportAuditRecord>, ReportingError> {
        let rows = sqlx::query(
            r#"
            SELECT a.actor_id, a.action, j.public_id AS job_id, a.action_at::text AS action_at, a.details
            FROM report_audit a
            JOIN report_jobs j ON j.id = a.report_job_id
            WHERE j.public_id = $1
            ORDER BY a.action_at ASC
            "#,
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| ReportingError::Store(error.to_string()))?;

        rows.into_iter().map(audit_from_row).collect()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TemplateDefinition {
    viewer_roles: Vec<ReportRole>,
    fields: Vec<crate::types::ReportField>,
    generated_at: String,
    retention: RetentionPolicy,
    rows: Vec<Value>,
    schedule: ReportSchedule,
}

impl TemplateDefinition {
    fn from_template(template: &ReportTemplate) -> Self {
        Self {
            viewer_roles: template.viewer_roles.clone(),
            fields: template.fields.clone(),
            generated_at: template.generated_at.clone(),
            retention: template.retention.clone(),
            rows: template.rows.clone(),
            schedule: template.schedule.clone(),
        }
    }
}

fn template_from_row(row: sqlx::postgres::PgRow) -> Result<ReportTemplate, ReportingError> {
    let definition: Value = row
        .try_get("definition")
        .map_err(|error| ReportingError::Store(error.to_string()))?;
    let definition: TemplateDefinition = serde_json::from_value(definition)
        .map_err(|error| ReportingError::Store(error.to_string()))?;
    Ok(ReportTemplate {
        code: row
            .try_get("code")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        name: row
            .try_get("name")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        category: row
            .try_get("category")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        schema_version: row
            .try_get("schema_version")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        viewer_roles: definition.viewer_roles,
        fields: definition.fields,
        generated_at: definition.generated_at,
        retention: definition.retention,
        rows: definition.rows,
        schedule: definition.schedule,
    })
}

fn job_from_row(row: sqlx::postgres::PgRow) -> Result<ReportJob, ReportingError> {
    let params: Value = row
        .try_get("params")
        .map_err(|error| ReportingError::Store(error.to_string()))?;
    let source_snapshot: Value = row
        .try_get("source_snapshot")
        .map_err(|error| ReportingError::Store(error.to_string()))?;
    let rows: Value = row
        .try_get("rows")
        .map_err(|error| ReportingError::Store(error.to_string()))?;
    let category: String = row
        .try_get("retention_category")
        .map_err(|error| ReportingError::Store(error.to_string()))?;

    Ok(ReportJob {
        id: row
            .try_get("public_id")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        template_code: row
            .try_get("template_code")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        template_name: row
            .try_get("template_name")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        requested_by: row
            .try_get("requested_by")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        schema_version: row
            .try_get("schema_version")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        version: row
            .try_get("report_version")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        state: row
            .try_get("state")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        generated_at: row
            .try_get("generated_at")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        params: serde_json::from_value(params)
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        source_snapshot: serde_json::from_value(source_snapshot)
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        output_rows: usize::try_from(
            row.try_get::<i64, _>("output_rows")
                .map_err(|error| ReportingError::Store(error.to_string()))?,
        )
        .map_err(|error| ReportingError::Store(error.to_string()))?,
        row_hash: row
            .try_get("row_hash")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        available_formats: vec![
            crate::types::ReportFormat::Csv,
            crate::types::ReportFormat::Json,
            crate::types::ReportFormat::Xlsx,
        ],
        retention: RetentionPolicy {
            category: if category == "legalaudit" || category == "legal_audit" {
                crate::types::RetentionCategory::LegalAudit
            } else {
                crate::types::RetentionCategory::Operations
            },
            expires_at: row
                .try_get("retention_expires_at")
                .map_err(|error| ReportingError::Store(error.to_string()))?,
        },
        rows: rows.as_array().cloned().unwrap_or_default(),
    })
}

fn audit_from_row(row: sqlx::postgres::PgRow) -> Result<ReportAuditRecord, ReportingError> {
    let details: Value = row
        .try_get("details")
        .map_err(|error| ReportingError::Store(error.to_string()))?;
    Ok(ReportAuditRecord {
        actor_id: row
            .try_get("actor_id")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        action: row
            .try_get("action")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        job_id: row
            .try_get("job_id")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        action_at: row
            .try_get("action_at")
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        details: serde_json::from_value(details)
            .map_err(|error| ReportingError::Store(error.to_string()))?,
    })
}

async fn insert_audit_row(
    pool: &PgPool,
    report_job_id: i64,
    audit: &ReportAuditRecord,
) -> Result<(), ReportingError> {
    sqlx::query(
        r#"
        INSERT INTO report_audit (report_job_id, actor_id, action, action_at, details)
        VALUES ($1, $2, $3, $4::timestamptz, $5)
        "#,
    )
    .bind(report_job_id)
    .bind(&audit.actor_id)
    .bind(&audit.action)
    .bind(&audit.action_at)
    .bind(
        serde_json::to_value(&audit.details)
            .map_err(|error| ReportingError::Store(error.to_string()))?,
    )
    .execute(pool)
    .await
    .map_err(|error| ReportingError::Store(error.to_string()))?;
    Ok(())
}

const JOB_SELECT: &str = r#"
SELECT j.public_id, t.code AS template_code, t.name AS template_name, j.requested_by,
       j.schema_version, j.report_version, j.state, j.generated_at::text AS generated_at,
       j.params, j.source_snapshot, j.rows, j.output_rows, j.row_hash,
       j.retention_category, j.retention_expires_at::text AS retention_expires_at
FROM report_jobs j
JOIN report_templates t ON t.id = j.template_id
ORDER BY j.generated_at DESC
"#;

const JOB_SELECT_WITH_FILTER: &str = r#"
SELECT j.public_id, t.code AS template_code, t.name AS template_name, j.requested_by,
       j.schema_version, j.report_version, j.state, j.generated_at::text AS generated_at,
       j.params, j.source_snapshot, j.rows, j.output_rows, j.row_hash,
       j.retention_category, j.retention_expires_at::text AS retention_expires_at
FROM report_jobs j
JOIN report_templates t ON t.id = j.template_id
WHERE t.code = $1
ORDER BY j.generated_at DESC
"#;

const JOB_SELECT_BY_ID: &str = r#"
SELECT j.public_id, t.code AS template_code, t.name AS template_name, j.requested_by,
       j.schema_version, j.report_version, j.state, j.generated_at::text AS generated_at,
       j.params, j.source_snapshot, j.rows, j.output_rows, j.row_hash,
       j.retention_category, j.retention_expires_at::text AS retention_expires_at
FROM report_jobs j
JOIN report_templates t ON t.id = j.template_id
WHERE j.public_id = $1
"#;

const JOB_SELECT_LATEST_PER_TEMPLATE: &str = r#"
SELECT DISTINCT ON (j.template_id) j.public_id, t.code AS template_code, t.name AS template_name,
       j.requested_by, j.schema_version, j.report_version, j.state, j.generated_at::text AS generated_at,
       j.params, j.source_snapshot, j.rows, j.output_rows, j.row_hash,
       j.retention_category, j.retention_expires_at::text AS retention_expires_at
FROM report_jobs j
JOIN report_templates t ON t.id = j.template_id
ORDER BY j.template_id, j.generated_at DESC
"#;
