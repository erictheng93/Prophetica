use std::collections::BTreeSet;

use serde_json::Value;

use crate::types::{AnalyticsReportingMetric, ReportAuditRecord, ReportJob};

#[must_use]
pub fn dashboard_metrics(
    jobs: &[ReportJob],
    audit: &[ReportAuditRecord],
) -> Vec<AnalyticsReportingMetric> {
    let kpi_value = latest_ops_service_uptime(jobs).unwrap_or_else(|| "n/a".to_owned());
    let alert_rows = latest_risk_alert_rows(jobs);
    let alert_rows_value = alert_rows.to_string();
    let versioned_reports_value = jobs.len().to_string();
    let audit_coverage_value = audit_coverage(jobs, audit);

    vec![
        metric(
            "kpi",
            "KPI",
            &kpi_value,
            "Latest service uptime from ops KPI report output.",
            if kpi_value == "n/a" {
                "neutral"
            } else {
                "success"
            },
        ),
        metric(
            "alerts",
            "Alerts",
            &alert_rows_value,
            "Rows in the latest risk alert report output.",
            if alert_rows == 0 {
                "success"
            } else {
                "warning"
            },
        ),
        metric(
            "versioned-reports",
            "Versioned reports",
            &versioned_reports_value,
            "Completed jobs with row hash and source snapshot metadata.",
            "neutral",
        ),
        metric(
            "audit-trail",
            "Audit coverage",
            &audit_coverage_value,
            "Latest report jobs with at least one audit record.",
            "neutral",
        ),
    ]
}

fn latest_ops_service_uptime(jobs: &[ReportJob]) -> Option<String> {
    jobs.iter()
        .find(|job| job.template_code == "ops-kpi-daily")
        .and_then(|job| {
            job.rows
                .iter()
                .find_map(|row| row.get("service_uptime").and_then(value_to_metric_string))
        })
}

fn latest_risk_alert_rows(jobs: &[ReportJob]) -> usize {
    jobs.iter()
        .find(|job| job.template_code == "risk-alert-summary")
        .map_or(0, |job| job.output_rows)
}

fn audit_coverage(jobs: &[ReportJob], audit: &[ReportAuditRecord]) -> String {
    if jobs.is_empty() {
        return "0%".to_owned();
    }

    let audited_job_ids = audit
        .iter()
        .map(|record| record.job_id.as_str())
        .collect::<BTreeSet<_>>();
    let covered_jobs = jobs
        .iter()
        .filter(|job| audited_job_ids.contains(job.id.as_str()))
        .count();
    let percentage = (covered_jobs * 100 + jobs.len() / 2) / jobs.len();

    format!("{percentage}%")
}

fn value_to_metric_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) if !value.trim().is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn metric(
    id: &str,
    label: &str,
    value: &str,
    detail: &str,
    tone: &str,
) -> AnalyticsReportingMetric {
    AnalyticsReportingMetric {
        id: id.to_owned(),
        label: label.to_owned(),
        value: value.to_owned(),
        detail: detail.to_owned(),
        tone: tone.to_owned(),
    }
}
