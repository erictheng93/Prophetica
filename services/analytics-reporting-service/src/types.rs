use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportRole {
    Admin,
    Manager,
    Affiliate,
}

impl ReportRole {
    #[must_use]
    pub const fn allows_admin_reporting(self) -> bool {
        matches!(self, Self::Admin | Self::Manager)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionCategory {
    Operations,
    LegalAudit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportFormat {
    Csv,
    Json,
    Xlsx,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportField {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub sensitive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionPolicy {
    pub category: RetentionCategory,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportTemplate {
    pub code: String,
    pub name: String,
    pub category: String,
    pub schema_version: String,
    pub viewer_roles: Vec<ReportRole>,
    pub fields: Vec<ReportField>,
    pub generated_at: String,
    pub retention: RetentionPolicy,
    #[serde(default)]
    pub rows: Vec<Value>,
    pub schedule: ReportSchedule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportSchedule {
    pub cron: String,
    pub next_fire_hint: String,
    pub owner: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportParams {
    pub start_at: String,
    pub end_at: String,
    pub timezone: String,
    pub sort_by: String,
    pub sort_direction: String,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSnapshot {
    pub source_service: String,
    pub source_version: String,
    pub cutoff_at: String,
    pub source_generated_at: String,
    pub correlation_id: String,
    pub sources: Vec<SourceProvenance>,
}

impl SourceSnapshot {
    #[must_use]
    pub fn single(
        service: impl Into<String>,
        version: impl Into<String>,
        cutoff_at: impl Into<String>,
        correlation_id: impl Into<String>,
    ) -> Self {
        let source_service = service.into();
        let source_version = version.into();
        let cutoff_at = cutoff_at.into();
        let source_generated_at = cutoff_at.clone();
        let correlation_id = correlation_id.into();
        let provenance = SourceProvenance {
            source_service: source_service.clone(),
            source_version: source_version.clone(),
            cutoff_at: cutoff_at.clone(),
            source_generated_at: source_generated_at.clone(),
            correlation_id: correlation_id.clone(),
        };

        Self {
            source_service,
            source_version,
            cutoff_at,
            source_generated_at,
            correlation_id,
            sources: vec![provenance],
        }
    }

    #[must_use]
    pub fn from_sources(sources: Vec<SourceProvenance>) -> Self {
        let first = sources
            .first()
            .cloned()
            .unwrap_or_else(|| SourceProvenance {
                source_service: "analytics-reporting-service".to_owned(),
                source_version: "analytics.v1".to_owned(),
                cutoff_at: "1970-01-01T00:00:00Z".to_owned(),
                source_generated_at: "1970-01-01T00:00:00Z".to_owned(),
                correlation_id: String::new(),
            });
        let source_service = if sources.len() == 1 {
            first.source_service.clone()
        } else {
            "analytics-reporting-service".to_owned()
        };
        let source_version = if sources.len() == 1 {
            first.source_version.clone()
        } else {
            sources
                .iter()
                .map(|source| format!("{}:{}", source.source_service, source.source_version))
                .collect::<Vec<_>>()
                .join("|")
        };

        Self {
            source_service,
            source_version,
            cutoff_at: first.cutoff_at,
            source_generated_at: first.source_generated_at,
            correlation_id: first.correlation_id,
            sources,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProvenance {
    #[serde(alias = "source_service")]
    pub source_service: String,
    #[serde(alias = "source_version")]
    pub source_version: String,
    #[serde(alias = "cutoff_at")]
    pub cutoff_at: String,
    #[serde(alias = "source_generated_at")]
    pub source_generated_at: String,
    #[serde(alias = "correlation_id")]
    pub correlation_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportJob {
    pub id: String,
    pub template_code: String,
    pub template_name: String,
    pub requested_by: String,
    pub schema_version: String,
    pub version: String,
    pub state: String,
    pub generated_at: String,
    pub params: ReportParams,
    pub source_snapshot: SourceSnapshot,
    pub output_rows: usize,
    pub row_hash: String,
    pub available_formats: Vec<ReportFormat>,
    pub retention: RetentionPolicy,
    pub rows: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportAuditRecord {
    pub actor_id: String,
    pub action: String,
    pub job_id: String,
    pub action_at: String,
    pub details: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReportJobRequest {
    #[serde(default)]
    pub actor_id: Option<String>,
    #[serde(default)]
    pub role: Option<ReportRole>,
    pub template_code: String,
    pub params: ReportParams,
    #[serde(default)]
    pub reveal_sensitive_fields: Option<bool>,
    #[serde(default)]
    pub second_authorization_id: Option<String>,
    #[serde(default)]
    pub rerun_of_job_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordExportDeletionRequest {
    #[serde(default)]
    pub actor_id: Option<String>,
    pub format: ReportFormat,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsReportingMetric {
    pub id: String,
    pub label: String,
    pub value: String,
    pub detail: String,
    pub tone: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleHint {
    pub template_code: String,
    pub template_name: String,
    pub cron: String,
    pub next_fire_hint: String,
    pub owner: String,
    pub retention: RetentionCategory,
    pub status: String,
}
