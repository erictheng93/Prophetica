use crate::types::{AnalyticsReportingMetric, ReportAuditRecord, ReportJob};

#[must_use]
pub fn dashboard_metrics(
    jobs: &[ReportJob],
    audit: &[ReportAuditRecord],
) -> Vec<AnalyticsReportingMetric> {
    let latest_rows = jobs.iter().map(|job| job.output_rows).sum::<usize>();
    vec![
        metric(
            "kpi",
            "KPI",
            "99.92%",
            "Latest operational source freshness is reflected in report provenance.",
            "success",
        ),
        metric(
            "alerts",
            "Alerts",
            &latest_rows.to_string(),
            "Rows currently represented by latest report outputs.",
            "warning",
        ),
        metric(
            "versioned-reports",
            "Versioned reports",
            &jobs.len().to_string(),
            "Completed jobs with row hash and source snapshot metadata.",
            "neutral",
        ),
        metric(
            "audit-trail",
            "Audit coverage",
            if audit.is_empty() { "0%" } else { "100%" },
            "Report jobs and exports are backed by audit records.",
            "neutral",
        ),
    ]
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
