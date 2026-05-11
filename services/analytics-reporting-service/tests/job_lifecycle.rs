#![allow(clippy::expect_used)]

use analytics_reporting_service::{
    error::ReportingError,
    job_lifecycle::{project_rows, row_hash, validate},
    templates::default_templates,
    types::{CreateReportJobRequest, ReportParams, ReportRole, SourceSnapshot},
};
use serde_json::json;

#[test]
fn row_hash_changes_when_source_snapshot_changes() {
    let params = ops_params();
    let rows = vec![json!({ "report_date": "2026-05-10", "approvals_count": 16 })];
    let first = SourceSnapshot::single("kyc-service", "kyc.v1", "2026-05-10T00:00:00Z", "corr-1");
    let second = SourceSnapshot::single("kyc-service", "kyc.v2", "2026-05-10T00:00:00Z", "corr-1");

    assert_ne!(
        row_hash("ops-kpi-daily", &params, &first, &rows),
        row_hash("ops-kpi-daily", &params, &second, &rows)
    );
}

#[test]
fn row_hash_is_stable_for_same_source_snapshot() {
    let params = ops_params();
    let rows = vec![json!({ "report_date": "2026-05-10", "approvals_count": 16 })];
    let snapshot =
        SourceSnapshot::single("kyc-service", "kyc.v1", "2026-05-10T00:00:00Z", "corr-1");

    assert_eq!(
        row_hash("ops-kpi-daily", &params, &snapshot, &rows),
        row_hash("ops-kpi-daily", &params, &snapshot, &rows)
    );
}

#[test]
fn sensitive_email_fields_are_masked_by_shared_helper() {
    let template = default_templates()
        .into_iter()
        .find(|template| template.code == "risk-alert-summary")
        .expect("risk template");
    let rows = vec![json!({
        "member_id": "MEM-001",
        "member_email": "member-001@example.com",
        "risk_score": 92,
        "risk_reason": "device_velocity"
    })];
    let projected = project_rows(&rows, &template.fields, &["member_email".to_owned()], false);

    assert_eq!(projected[0]["member_email"], "m***@example.com");
}

#[test]
fn manager_cannot_request_utc_report_timezone() {
    let template = ops_template();
    let request = create_request(ReportRole::Manager, "UTC");

    assert!(matches!(
        validate(&template, &request),
        Err(ReportingError::Forbidden)
    ));
}

#[test]
fn admin_can_request_utc_report_timezone() {
    let template = ops_template();
    let request = create_request(ReportRole::Admin, "UTC");

    assert!(validate(&template, &request).is_ok());
}

#[test]
fn manager_can_request_platform_report_timezone() {
    let template = ops_template();
    let request = create_request(ReportRole::Manager, "Asia/Taipei");

    assert!(validate(&template, &request).is_ok());
}

fn ops_params() -> ReportParams {
    ReportParams {
        start_at: "2026-05-10T00:00:00+08:00".to_owned(),
        end_at: "2026-05-10T23:59:59+08:00".to_owned(),
        timezone: "Asia/Taipei".to_owned(),
        sort_by: "report_date".to_owned(),
        sort_direction: "asc".to_owned(),
        columns: vec!["report_date".to_owned(), "approvals_count".to_owned()],
    }
}

fn ops_template() -> analytics_reporting_service::types::ReportTemplate {
    default_templates()
        .into_iter()
        .find(|template| template.code == "ops-kpi-daily")
        .expect("ops template")
}

fn create_request(role: ReportRole, timezone: &str) -> CreateReportJobRequest {
    let mut params = ops_params();
    timezone.clone_into(&mut params.timezone);

    CreateReportJobRequest {
        actor_id: Some("U-TEST-001".to_owned()),
        role: Some(role),
        template_code: "ops-kpi-daily".to_owned(),
        params,
        reveal_sensitive_fields: None,
        second_authorization_id: None,
        rerun_of_job_id: None,
    }
}
