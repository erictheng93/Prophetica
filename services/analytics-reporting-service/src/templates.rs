use serde_json::json;

use crate::types::{
    ReportField, ReportRole, ReportSchedule, ReportTemplate, RetentionCategory, RetentionPolicy,
};

#[must_use]
pub fn default_templates() -> Vec<ReportTemplate> {
    vec![
        ReportTemplate {
            code: "ops-kpi-daily".to_owned(),
            name: "Ops KPI Daily".to_owned(),
            category: "read_only_operations".to_owned(),
            schema_version: "analytics.v1".to_owned(),
            viewer_roles: vec![ReportRole::Admin, ReportRole::Manager],
            generated_at: "2026-05-10T08:00:00+08:00".to_owned(),
            retention: RetentionPolicy {
                category: RetentionCategory::Operations,
                expires_at: "2027-05-10T08:00:00+08:00".to_owned(),
            },
            fields: vec![
                field("report_date", "Report date", false),
                field("approvals_count", "Approvals", false),
                field("kyc_pass_rate", "KYC pass rate", false),
                field("service_uptime", "Service uptime", false),
            ],
            rows: vec![json!({
                "report_date": "2026-05-10",
                "approvals_count": 16,
                "kyc_pass_rate": "91.4%",
                "service_uptime": "99.92%"
            })],
            schedule: ReportSchedule {
                cron: "0 8 * * *".to_owned(),
                next_fire_hint: "2026-05-11T08:00:00+08:00".to_owned(),
                owner: "Ops Control".to_owned(),
            },
        },
        ReportTemplate {
            code: "risk-alert-summary".to_owned(),
            name: "Risk Alert Summary".to_owned(),
            category: "risk".to_owned(),
            schema_version: "analytics.v1".to_owned(),
            viewer_roles: vec![ReportRole::Admin, ReportRole::Manager],
            generated_at: "2026-05-10T08:15:00+08:00".to_owned(),
            retention: RetentionPolicy {
                category: RetentionCategory::Operations,
                expires_at: "2027-05-10T08:15:00+08:00".to_owned(),
            },
            fields: vec![
                field("member_id", "Member ID", false),
                field("member_email", "Member email", true),
                field("risk_score", "Risk score", false),
                field("risk_reason", "Risk reason", false),
            ],
            rows: vec![json!({
                "member_id": "MEM-001",
                "member_email": "member-001@example.com",
                "risk_score": 92,
                "risk_reason": "device_velocity"
            })],
            schedule: ReportSchedule {
                cron: "0 9 * * MON-FRI".to_owned(),
                next_fire_hint: "2026-05-12T09:00:00+08:00".to_owned(),
                owner: "Risk Ops".to_owned(),
            },
        },
        ReportTemplate {
            code: "finance-reconciliation".to_owned(),
            name: "Finance Reconciliation".to_owned(),
            category: "finance_review".to_owned(),
            schema_version: "analytics.v1".to_owned(),
            viewer_roles: vec![ReportRole::Admin],
            generated_at: "2026-05-10T08:00:00+08:00".to_owned(),
            retention: RetentionPolicy {
                category: RetentionCategory::LegalAudit,
                expires_at: "2033-05-10T08:00:00+08:00".to_owned(),
            },
            fields: vec![
                field("cutoff_at", "Cutoff", false),
                field("source_version", "Source version", false),
                field("settlement_total", "Settlement total", false),
                field("reconciliation_delta", "Reconciliation delta", false),
            ],
            rows: vec![json!({
                "cutoff_at": "2026-05-10T07:59:59+08:00",
                "source_version": "ledger.v2026.05.10",
                "settlement_total": 502_500,
                "reconciliation_delta": 0
            })],
            schedule: ReportSchedule {
                cron: "0 1 * * *".to_owned(),
                next_fire_hint: "2026-05-11T01:00:00+08:00".to_owned(),
                owner: "Compliance".to_owned(),
            },
        },
    ]
}

fn field(key: &str, label: &str, sensitive: bool) -> ReportField {
    ReportField {
        key: key.to_owned(),
        label: label.to_owned(),
        sensitive,
    }
}
