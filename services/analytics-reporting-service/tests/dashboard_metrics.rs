#![allow(clippy::expect_used)]

use std::collections::BTreeMap;

use analytics_reporting_service::{
    dashboard_metrics::dashboard_metrics,
    types::{
        AnalyticsReportingMetric, ReportAuditRecord, ReportFormat, ReportJob, ReportParams,
        RetentionCategory, RetentionPolicy, SourceSnapshot,
    },
};
use serde_json::{Value, json};

#[test]
fn dashboard_metrics_are_derived_from_latest_reports_and_audit_records() {
    let jobs = vec![
        report_job(
            "job-ops",
            "ops-kpi-daily",
            "Ops KPI Daily",
            1,
            vec![json!({
                "report_date": "2026-05-10",
                "service_uptime": "98.50%"
            })],
        ),
        report_job(
            "job-risk",
            "risk-alert-summary",
            "Risk Alert Summary",
            3,
            vec![
                json!({ "member_id": "MEM-001", "risk_score": 92 }),
                json!({ "member_id": "MEM-002", "risk_score": 88 }),
                json!({ "member_id": "MEM-003", "risk_score": 85 }),
            ],
        ),
        report_job(
            "job-finance",
            "finance-reconciliation",
            "Finance Reconciliation",
            9,
            vec![json!({ "settlement_total": 502_500 })],
        ),
    ];
    let audit = vec![audit_record("job-ops"), audit_record("job-risk")];

    let metrics = dashboard_metrics(&jobs, &audit);

    assert_eq!(metric_value(&metrics, "kpi"), "98.50%");
    assert_eq!(metric_value(&metrics, "alerts"), "3");
    assert_eq!(metric_value(&metrics, "versioned-reports"), "3");
    assert_eq!(metric_value(&metrics, "audit-trail"), "67%");
}

#[test]
fn empty_dashboard_metrics_do_not_show_fixture_kpi_or_alerts() {
    let metrics = dashboard_metrics(&[], &[]);

    assert_eq!(metric_value(&metrics, "kpi"), "n/a");
    assert_eq!(metric_value(&metrics, "alerts"), "0");
    assert_eq!(metric_value(&metrics, "audit-trail"), "0%");
}

fn metric_value(metrics: &[AnalyticsReportingMetric], id: &str) -> String {
    metrics
        .iter()
        .find(|metric| metric.id == id)
        .expect("metric exists")
        .value
        .clone()
}

fn report_job(
    id: &str,
    template_code: &str,
    template_name: &str,
    output_rows: usize,
    rows: Vec<Value>,
) -> ReportJob {
    ReportJob {
        id: id.to_owned(),
        template_code: template_code.to_owned(),
        template_name: template_name.to_owned(),
        requested_by: "U-ADMIN-001".to_owned(),
        schema_version: "analytics.v1".to_owned(),
        version: "20260510T000000Z".to_owned(),
        state: "completed".to_owned(),
        generated_at: "2026-05-10T00:00:00Z".to_owned(),
        params: ReportParams {
            start_at: "2026-05-10T00:00:00+08:00".to_owned(),
            end_at: "2026-05-10T23:59:59+08:00".to_owned(),
            timezone: "Asia/Taipei".to_owned(),
            sort_by: "report_date".to_owned(),
            sort_direction: "asc".to_owned(),
            columns: vec!["report_date".to_owned()],
        },
        source_snapshot: SourceSnapshot::single(
            "analytics-reporting-service",
            "analytics.v1",
            "2026-05-10T00:00:00Z",
            "corr-dashboard",
        ),
        output_rows,
        row_hash: format!("hash-{id}"),
        available_formats: vec![ReportFormat::Json],
        retention: RetentionPolicy {
            category: RetentionCategory::Operations,
            expires_at: "2027-05-10T00:00:00Z".to_owned(),
        },
        rows,
    }
}

fn audit_record(job_id: &str) -> ReportAuditRecord {
    ReportAuditRecord {
        actor_id: "U-ADMIN-001".to_owned(),
        action: "report.created".to_owned(),
        job_id: job_id.to_owned(),
        action_at: "2026-05-10T00:00:00Z".to_owned(),
        details: BTreeMap::new(),
    }
}
