use std::{collections::BTreeMap, sync::Arc};

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
    routing::{get, post},
};
use serde::Deserialize;

use crate::{
    dashboard_metrics::dashboard_metrics,
    error::ReportingError,
    exports::export_report_job,
    job_lifecycle::{project_rows, row_hash, sort_rows, validate},
    sources::SourceClients,
    store::ReportingStore,
    types::{
        CreateReportJobRequest, RecordExportDeletionRequest, ReportAuditRecord, ReportFormat,
        ReportJob, ReportRole, ScheduleHint,
    },
};

#[derive(Clone)]
pub struct AppState<S> {
    store: S,
    sources: SourceClients,
}

pub fn router<S>(store: S) -> Router
where
    S: ReportingStore,
{
    router_with_sources(store, SourceClients::fixtures())
}

pub fn router_with_sources<S>(store: S, sources: SourceClients) -> Router
where
    S: ReportingStore,
{
    let state = Arc::new(AppState { store, sources });

    Router::new()
        .route("/v1/report-templates", get(list_templates::<S>))
        .route(
            "/v1/report-jobs",
            get(list_jobs::<S>).post(create_report_job::<S>),
        )
        .route(
            "/v1/report-jobs/latest-per-template",
            get(latest_per_template::<S>),
        )
        .route("/v1/report-jobs/:id", get(get_job::<S>))
        .route("/v1/report-jobs/:id/exports/:format", get(export_job::<S>))
        .route(
            "/v1/report-jobs/:id/export-deletions",
            post(record_export_deletion::<S>),
        )
        .route("/v1/report-schedules", get(list_schedules::<S>))
        .route("/v1/dashboard-metrics", get(get_dashboard_metrics::<S>))
        .with_state(state)
}

async fn list_templates<S>(
    State(state): State<Arc<AppState<S>>>,
    headers: HeaderMap,
) -> Result<Json<Vec<crate::types::ReportTemplate>>, ReportingError>
where
    S: ReportingStore,
{
    let role = role_from_headers(&headers)?;
    if !role.allows_admin_reporting() {
        return Err(ReportingError::Forbidden);
    }
    Ok(Json(state.store.templates_for_role(role).await?))
}

async fn create_report_job<S>(
    State(state): State<Arc<AppState<S>>>,
    headers: HeaderMap,
    Json(mut request): Json<CreateReportJobRequest>,
) -> Result<Json<serde_json::Value>, ReportingError>
where
    S: ReportingStore,
{
    let role = role_from_headers(&headers)?;
    let actor_id = actor_from_headers(&headers).or_else(|| request.actor_id.clone());
    request.role = Some(role);
    request.actor_id.clone_from(&actor_id);

    let template = state
        .store
        .find_template(&request.template_code)
        .await?
        .ok_or(ReportingError::NotFound)?;
    validate(&template, &request)?;

    if request.reveal_sensitive_fields.unwrap_or(false) {
        return Err(ReportingError::InvalidRequest(
            "plaintext PII reveal is not available in v1".to_owned(),
        ));
    }

    let request_id = request_id_from_headers(&headers);
    let (source_rows, source_snapshot) = state
        .sources
        .resolve_rows(&template, &request.params, &request_id)
        .await?;
    let rows = project_rows(
        &sort_rows(
            &source_rows,
            &request.params.sort_by,
            &request.params.sort_direction,
        ),
        &template.fields,
        &request.params.columns,
        request.reveal_sensitive_fields.unwrap_or(false),
    );
    let sequence = state.store.list_jobs(Some(&template.code)).await?.len() + 1;
    let total = state.store.list_jobs(None).await?.len() + 1;
    let hash = row_hash(&template.code, &request.params, &source_snapshot, &rows);
    let job = ReportJob {
        id: format!("rpt-job-{total:06}"),
        template_code: template.code.clone(),
        template_name: template.name.clone(),
        requested_by: actor_id.unwrap_or_else(|| "system".to_owned()),
        schema_version: template.schema_version.clone(),
        version: format!("{}.{sequence}", version_prefix(&template.generated_at)),
        state: "completed".to_owned(),
        generated_at: template.generated_at.clone(),
        params: request.params,
        source_snapshot,
        output_rows: rows.len(),
        row_hash: hash.clone(),
        available_formats: vec![ReportFormat::Csv, ReportFormat::Json, ReportFormat::Xlsx],
        retention: template.retention,
        rows,
    };
    let audit = ReportAuditRecord {
        actor_id: job.requested_by.clone(),
        action: if request.rerun_of_job_id.is_some() {
            "report_job_rerun".to_owned()
        } else {
            "report_job_created".to_owned()
        },
        job_id: job.id.clone(),
        action_at: job.generated_at.clone(),
        details: BTreeMap::from([
            ("template_code".to_owned(), job.template_code.clone()),
            ("row_hash".to_owned(), hash),
            (
                "rerun_of_job_id".to_owned(),
                request.rerun_of_job_id.unwrap_or_default(),
            ),
        ]),
    };

    state.store.insert_job(job.clone(), audit.clone()).await?;
    Ok(Json(serde_json::json!({ "job": job, "audit": audit })))
}

#[derive(Debug, Deserialize)]
struct JobsQuery {
    template_code: Option<String>,
}

async fn list_jobs<S>(
    State(state): State<Arc<AppState<S>>>,
    Query(query): Query<JobsQuery>,
) -> Result<Json<Vec<ReportJob>>, ReportingError>
where
    S: ReportingStore,
{
    Ok(Json(
        state
            .store
            .list_jobs(query.template_code.as_deref())
            .await?,
    ))
}

async fn get_job<S>(
    State(state): State<Arc<AppState<S>>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ReportingError>
where
    S: ReportingStore,
{
    let job = state
        .store
        .find_job(&id)
        .await?
        .ok_or(ReportingError::NotFound)?;
    let audit = state.store.list_audit_for_job(&id).await?;
    Ok(Json(serde_json::json!({ "job": job, "audit": audit })))
}

async fn export_job<S>(
    State(state): State<Arc<AppState<S>>>,
    Path((id, format)): Path<(String, String)>,
) -> Result<Response, ReportingError>
where
    S: ReportingStore,
{
    let job = state
        .store
        .find_job(&id)
        .await?
        .ok_or(ReportingError::NotFound)?;
    let format = parse_format(&format)?;
    let payload = export_report_job(&job, format)?;
    let mut response = Response::new(Body::from(payload.body));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(payload.content_type),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", payload.filename))
            .map_err(|error| ReportingError::Store(error.to_string()))?,
    );
    Ok(response)
}

async fn record_export_deletion<S>(
    State(state): State<Arc<AppState<S>>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<RecordExportDeletionRequest>,
) -> Result<StatusCode, ReportingError>
where
    S: ReportingStore,
{
    let job = state
        .store
        .find_job(&id)
        .await?
        .ok_or(ReportingError::NotFound)?;
    let audit = ReportAuditRecord {
        actor_id: actor_from_headers(&headers)
            .or(request.actor_id)
            .unwrap_or_else(|| "system".to_owned()),
        action: "report_export_deleted".to_owned(),
        job_id: id,
        action_at: job.generated_at,
        details: BTreeMap::from([
            (
                "format".to_owned(),
                format!("{:?}", request.format).to_ascii_lowercase(),
            ),
            ("reason".to_owned(), request.reason),
            ("row_hash".to_owned(), job.row_hash),
        ]),
    };
    state.store.insert_audit(audit).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_schedules<S>(
    State(state): State<Arc<AppState<S>>>,
    headers: HeaderMap,
) -> Result<Json<Vec<ScheduleHint>>, ReportingError>
where
    S: ReportingStore,
{
    let role = role_from_headers(&headers)?;
    let templates = state.store.templates_for_role(role).await?;
    Ok(Json(
        templates
            .into_iter()
            .map(|template| ScheduleHint {
                template_code: template.code,
                template_name: template.name,
                cron: template.schedule.cron,
                next_fire_hint: template.schedule.next_fire_hint,
                owner: template.schedule.owner,
                retention: template.retention.category,
                status: "enabled".to_owned(),
            })
            .collect(),
    ))
}

async fn get_dashboard_metrics<S>(
    State(state): State<Arc<AppState<S>>>,
) -> Result<Json<Vec<crate::types::AnalyticsReportingMetric>>, ReportingError>
where
    S: ReportingStore,
{
    let jobs = state.store.latest_per_template().await?;
    let mut audit = Vec::new();
    for job in &jobs {
        audit.extend(state.store.list_audit_for_job(&job.id).await?);
    }
    Ok(Json(dashboard_metrics(&jobs, &audit)))
}

async fn latest_per_template<S>(
    State(state): State<Arc<AppState<S>>>,
) -> Result<Json<Vec<ReportJob>>, ReportingError>
where
    S: ReportingStore,
{
    Ok(Json(state.store.latest_per_template().await?))
}

fn role_from_headers(headers: &HeaderMap) -> Result<ReportRole, ReportingError> {
    match headers
        .get("x-operator-role")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("admin")
    {
        "admin" => Ok(ReportRole::Admin),
        "manager" => Ok(ReportRole::Manager),
        "affiliate" => Ok(ReportRole::Affiliate),
        _ => Err(ReportingError::Forbidden),
    }
}

fn actor_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-actor-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn request_id_from_headers(headers: &HeaderMap) -> String {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("analytics-reporting-service")
        .to_owned()
}

fn parse_format(value: &str) -> Result<ReportFormat, ReportingError> {
    match value {
        "csv" => Ok(ReportFormat::Csv),
        "json" => Ok(ReportFormat::Json),
        "xlsx" => Ok(ReportFormat::Xlsx),
        _ => Err(ReportingError::InvalidRequest(
            "unknown export format".to_owned(),
        )),
    }
}

fn version_prefix(generated_at: &str) -> String {
    format!(
        "v{}",
        generated_at
            .get(0..10)
            .unwrap_or("1970-01-01")
            .replace('-', ".")
    )
}
