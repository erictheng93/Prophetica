use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, signal};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Market {
    pub id: String,
    pub title: String,
    pub brief: String,
    pub category: String,
    pub status: String,
    pub yes_price_cents: i32,
    pub yes_bid_cents: i32,
    pub yes_ask_cents: i32,
    pub fee_bps: u32,
    pub volume_24h_points: i64,
    pub volume_total_points: i64,
    pub opened_at: String,
    pub close_at: String,
    pub oracle_source: String,
    pub resolved_outcome: Option<String>,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone)]
struct AppState {
    markets: Arc<Mutex<HashMap<String, Market>>>,
    oracle_submissions: Arc<Mutex<HashMap<String, Vec<OracleSubmission>>>>,
    disputes: Arc<Mutex<HashMap<String, Vec<Dispute>>>>,
    dispute_resolutions: Arc<Mutex<HashMap<String, Vec<DisputeResolution>>>>,
    status_events: Arc<Mutex<HashMap<String, Vec<MarketStatusEvent>>>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ListMarketsQuery {
    status: Option<String>,
    category: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListMarketsResponse {
    markets: Vec<Market>,
}

#[derive(Debug, Clone, Deserialize)]
struct SubmitOracleRequest {
    submitted_by: String,
    outcome_result: String,
    evidence_urls: Vec<String>,
    evidence_hash: String,
    notes: String,
    signature: String,
}

#[derive(Debug, Clone, Serialize)]
struct SubmitOracleResponse {
    submission_id: String,
    ready_for_pending: bool,
    market_status: String,
    dispute_until: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct OracleSubmission {
    id: String,
    submitted_by: String,
    outcome_result: String,
    evidence_urls: Vec<String>,
    evidence_hash: String,
    notes: String,
    signature: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SubmitDisputeRequest {
    user_id: String,
    reason: String,
    evidence_urls: Vec<String>,
    claimed_outcome: String,
}

#[derive(Debug, Clone, Serialize)]
struct SubmitDisputeResponse {
    dispute_id: String,
    status: &'static str,
    market_status: String,
    deadline: String,
}

#[derive(Debug, Clone, Serialize)]
struct Dispute {
    id: String,
    user_id: String,
    reason: String,
    evidence_urls: Vec<String>,
    claimed_outcome: String,
    status: &'static str,
}

#[derive(Debug, Clone, Deserialize)]
struct SubmitDisputeResolutionRequest {
    decision: String,
    votes: Vec<DisputeVote>,
    final_signoff_by: String,
    audit_hash: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DisputeVote {
    adjudicator: String,
    vote: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize)]
struct DisputeResolution {
    id: String,
    decision: String,
    votes: Vec<DisputeVote>,
    final_signoff_by: String,
    audit_hash: String,
}

#[derive(Debug, Clone, Serialize)]
struct DisputeResolutionResponse {
    resolution_id: String,
    market_status: String,
    ledger_txn_type: Option<&'static str>,
}

#[derive(Debug, Clone, Deserialize)]
struct SettleMarketRequest {
    reviewer: String,
}

#[derive(Debug, Clone, Serialize)]
struct SettleMarketResponse {
    market_status: String,
    reviewer: String,
    ledger_txn_type: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct MarketStatusEvent {
    market_id: String,
    old_status: String,
    new_status: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct ListStatusEventsResponse {
    events: Vec<MarketStatusEvent>,
}

#[derive(Debug, Clone, Deserialize)]
struct ListSettlementsQuery {
    since: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SettlementSummary {
    market_id: String,
    title: String,
    status: String,
    resolved_outcome: Option<String>,
    resolved_at: Option<String>,
    oracle_source: String,
}

#[derive(Debug, Serialize)]
struct ListSettlementsResponse {
    settlements: Vec<SettlementSummary>,
}

#[derive(Debug, Serialize)]
struct ApiErrorResponse {
    error: &'static str,
    message: String,
}

pub fn router(markets: impl IntoIterator<Item = Market>) -> Router {
    let state = AppState {
        markets: Arc::new(Mutex::new(
            markets
                .into_iter()
                .map(|market| (market.id.clone(), market))
                .collect(),
        )),
        oracle_submissions: Arc::new(Mutex::new(HashMap::new())),
        disputes: Arc::new(Mutex::new(HashMap::new())),
        dispute_resolutions: Arc::new(Mutex::new(HashMap::new())),
        status_events: Arc::new(Mutex::new(HashMap::new())),
    };

    Router::new()
        .route("/internal/markets", get(list_markets))
        .route("/internal/markets/:market_id", get(get_market))
        .route(
            "/internal/markets/:market_id/oracle-submissions",
            post(submit_oracle_result),
        )
        .route("/api/v1/markets/:market_id/disputes", post(submit_dispute))
        .route(
            "/internal/markets/:market_id/dispute-resolutions",
            post(submit_dispute_resolution),
        )
        .route("/internal/markets/:market_id/settle", post(settle_market))
        .route(
            "/internal/markets/:market_id/status-events",
            get(list_status_events),
        )
        .route("/api/v1/settlements", get(list_settlements))
        .with_state(state)
}

/// Runs the market-service HTTP server with SPEC-015 read facade routes.
///
/// # Errors
///
/// Returns an error when configuration loading, socket binding, or serving fails.
#[tracing::instrument(fields(service.name = crate::SERVICE_NAME), skip_all)]
pub async fn run_http() -> anyhow::Result<()> {
    let config = sdk::config::ServiceConfig::from_env(crate::SERVICE_NAME, crate::DEFAULT_PORT)?;
    tracing::info!(?config, "market service config loaded");

    let app = sdk::service_stub::router(&config).merge(router(seed_markets()));
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "market service listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME), skip(state))]
async fn list_markets(
    State(state): State<AppState>,
    Query(query): Query<ListMarketsQuery>,
) -> Json<ListMarketsResponse> {
    let markets_guard = state
        .markets
        .lock()
        .map_or_else(|_| HashMap::new(), |markets| markets.clone());
    let mut markets = markets_guard
        .values()
        .filter(|market| {
            query
                .status
                .as_ref()
                .is_none_or(|status| market.status == *status)
        })
        .filter(|market| {
            query
                .category
                .as_ref()
                .is_none_or(|category| market.category == *category)
        })
        .cloned()
        .collect::<Vec<_>>();
    markets.sort_by(|a, b| b.opened_at.cmp(&a.opened_at));

    Json(ListMarketsResponse { markets })
}

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME, proph.market_id = %market_id), skip(state))]
async fn get_market(
    State(state): State<AppState>,
    Path(market_id): Path<String>,
) -> Result<Json<Market>, (StatusCode, Json<ApiErrorResponse>)> {
    let markets = state.markets.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse {
                error: "market_store_unavailable",
                message: "market store lock is unavailable".to_owned(),
            }),
        )
    })?;

    markets.get(&market_id).cloned().map(Json).ok_or((
        StatusCode::NOT_FOUND,
        Json(ApiErrorResponse {
            error: "market_not_found",
            message: format!("market {market_id} was not found"),
        }),
    ))
}

async fn submit_oracle_result(
    State(state): State<AppState>,
    Path(market_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SubmitOracleRequest>,
) -> Result<(StatusCode, Json<SubmitOracleResponse>), (StatusCode, Json<ApiErrorResponse>)> {
    require_settlement_step_up(&headers)?;
    validate_oracle_submission_request(&request)?;

    let mut markets = state.markets.lock().map_err(store_error)?;
    let market = markets.get_mut(&market_id).ok_or_else(|| {
        api_error(
            StatusCode::NOT_FOUND,
            "market_not_found",
            "market was not found",
        )
    })?;

    if market.status != "closed" && market.status != "pending" {
        return Err(api_error(
            StatusCode::CONFLICT,
            "market_not_closed",
            "oracle submissions require a closed or pending market",
        ));
    }

    let mut submissions_by_market = state.oracle_submissions.lock().map_err(store_error)?;
    let submissions = submissions_by_market.entry(market_id.clone()).or_default();
    if submissions
        .iter()
        .any(|submission| submission.submitted_by == request.submitted_by)
    {
        return Err(api_error(
            StatusCode::CONFLICT,
            "duplicate_oracle_operator",
            "operator already submitted an oracle result for this market",
        ));
    }

    let submission_id = format!("orasub_{}_{}", market_id, submissions.len() + 1);
    submissions.push(OracleSubmission {
        id: submission_id.clone(),
        submitted_by: request.submitted_by,
        outcome_result: request.outcome_result.clone(),
        evidence_urls: request.evidence_urls,
        evidence_hash: request.evidence_hash,
        notes: request.notes,
        signature: request.signature,
    });

    let summary = summarize_oracle_submissions(submissions, &request.outcome_result);

    if summary.ready_for_pending {
        let new_status = if request.outcome_result == "VOID" {
            "cancelled".to_owned()
        } else {
            "pending".to_owned()
        };
        record_status_change(
            &state,
            &market_id,
            &market.status,
            &new_status,
            "oracle_result",
        )?;
        market.status = new_status;
        market.resolved_outcome = Some(request.outcome_result);
        market.resolved_at = Some(market.close_at.clone());
    } else if summary.has_mismatch {
        record_status_change(
            &state,
            &market_id,
            &market.status,
            "disputed",
            "oracle_mismatch",
        )?;
        "disputed".clone_into(&mut market.status);
    }

    Ok((
        StatusCode::CREATED,
        Json(SubmitOracleResponse {
            submission_id,
            ready_for_pending: summary.ready_for_pending,
            market_status: market.status.clone(),
            dispute_until: market.resolved_at.clone().map(|_| dispute_until(market)),
        }),
    ))
}

fn validate_oracle_submission_request(
    request: &SubmitOracleRequest,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    if !matches!(request.outcome_result.as_str(), "YES" | "NO" | "VOID") {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "invalid_outcome",
            "outcome_result must be YES, NO, or VOID",
        ));
    }
    if request.evidence_urls.is_empty()
        || request.evidence_hash.trim().is_empty()
        || request.signature.trim().is_empty()
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "oracle_evidence_required",
            "oracle submissions require evidence_urls, evidence_hash, and signature",
        ));
    }
    Ok(())
}

struct OracleSubmissionSummary {
    ready_for_pending: bool,
    has_mismatch: bool,
}

fn summarize_oracle_submissions(
    submissions: &[OracleSubmission],
    latest_outcome: &str,
) -> OracleSubmissionSummary {
    let independent_operators = submissions
        .iter()
        .map(|submission| submission.submitted_by.as_str())
        .collect::<HashSet<_>>()
        .len();
    let matching_outcomes = submissions
        .iter()
        .filter(|submission| submission.outcome_result == latest_outcome)
        .count();
    let distinct_outcomes = submissions
        .iter()
        .map(|submission| submission.outcome_result.as_str())
        .collect::<HashSet<_>>()
        .len();

    OracleSubmissionSummary {
        ready_for_pending: independent_operators >= 2 && matching_outcomes >= 2,
        has_mismatch: independent_operators >= 2 && distinct_outcomes > 1,
    }
}

async fn submit_dispute(
    State(state): State<AppState>,
    Path(market_id): Path<String>,
    Json(request): Json<SubmitDisputeRequest>,
) -> Result<(StatusCode, Json<SubmitDisputeResponse>), (StatusCode, Json<ApiErrorResponse>)> {
    if !matches!(request.claimed_outcome.as_str(), "YES" | "NO" | "VOID") {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "invalid_claimed_outcome",
            "claimed_outcome must be YES, NO, or VOID",
        ));
    }

    let mut markets = state.markets.lock().map_err(store_error)?;
    let market = markets.get_mut(&market_id).ok_or_else(|| {
        api_error(
            StatusCode::NOT_FOUND,
            "market_not_found",
            "market was not found",
        )
    })?;

    if market.status != "pending" {
        return Err(api_error(
            StatusCode::CONFLICT,
            "dispute_window_closed",
            "disputes are accepted only while the market is pending",
        ));
    }

    let mut disputes_by_market = state.disputes.lock().map_err(store_error)?;
    let disputes = disputes_by_market.entry(market_id.clone()).or_default();
    if disputes
        .iter()
        .any(|dispute| dispute.user_id == request.user_id)
    {
        return Err(api_error(
            StatusCode::CONFLICT,
            "dispute_duplicate",
            "user already submitted a dispute for this market",
        ));
    }

    let dispute_id = format!("disp_{}_{}", market_id, disputes.len() + 1);
    disputes.push(Dispute {
        id: dispute_id.clone(),
        user_id: request.user_id,
        reason: request.reason,
        evidence_urls: request.evidence_urls,
        claimed_outcome: request.claimed_outcome,
        status: "PENDING_REVIEW",
    });
    record_status_change(
        &state,
        &market_id,
        &market.status,
        "disputed",
        "dispute_submitted",
    )?;
    "disputed".clone_into(&mut market.status);

    Ok((
        StatusCode::CREATED,
        Json(SubmitDisputeResponse {
            dispute_id,
            status: "PENDING_REVIEW",
            market_status: market.status.clone(),
            deadline: dispute_until(market),
        }),
    ))
}

async fn list_settlements(
    State(state): State<AppState>,
    Query(query): Query<ListSettlementsQuery>,
) -> Json<ListSettlementsResponse> {
    let markets_guard = state
        .markets
        .lock()
        .map_or_else(|_| HashMap::new(), |markets| markets.clone());
    let mut settlements = markets_guard
        .values()
        .filter(|market| market.status == "settled" || market.status == "cancelled")
        .filter(|market| {
            query
                .since
                .as_ref()
                .is_none_or(|since| market.resolved_at.as_ref().is_some_and(|ts| ts >= since))
        })
        .map(|market| SettlementSummary {
            market_id: market.id.clone(),
            title: market.title.clone(),
            status: market.status.clone(),
            resolved_outcome: market.resolved_outcome.clone(),
            resolved_at: market.resolved_at.clone(),
            oracle_source: market.oracle_source.clone(),
        })
        .collect::<Vec<_>>();
    settlements.sort_by(|a, b| b.resolved_at.cmp(&a.resolved_at));

    Json(ListSettlementsResponse { settlements })
}

async fn submit_dispute_resolution(
    State(state): State<AppState>,
    Path(market_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SubmitDisputeResolutionRequest>,
) -> Result<(StatusCode, Json<DisputeResolutionResponse>), (StatusCode, Json<ApiErrorResponse>)> {
    require_settlement_step_up(&headers)?;
    if !matches!(
        request.decision.as_str(),
        "UPHOLD_INITIAL" | "OVERTURN" | "VOID_MARKET" | "EXTEND_DISPUTE"
    ) {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "invalid_dispute_decision",
            "decision must be UPHOLD_INITIAL, OVERTURN, VOID_MARKET, or EXTEND_DISPUTE",
        ));
    }
    validate_adjudicator_votes(&request)?;

    let mut markets = state.markets.lock().map_err(store_error)?;
    let market = markets.get_mut(&market_id).ok_or_else(|| {
        api_error(
            StatusCode::NOT_FOUND,
            "market_not_found",
            "market was not found",
        )
    })?;
    if market.status != "disputed" {
        return Err(api_error(
            StatusCode::CONFLICT,
            "market_not_disputed",
            "dispute resolutions require a disputed market",
        ));
    }

    let mut resolutions_by_market = state.dispute_resolutions.lock().map_err(store_error)?;
    let resolutions = resolutions_by_market.entry(market_id.clone()).or_default();
    let resolution_id = format!("dispres_{}_{}", market_id, resolutions.len() + 1);
    resolutions.push(DisputeResolution {
        id: resolution_id.clone(),
        decision: request.decision.clone(),
        votes: request.votes,
        final_signoff_by: request.final_signoff_by,
        audit_hash: request.audit_hash,
    });

    let ledger_txn_type = match request.decision.as_str() {
        "VOID_MARKET" => {
            record_status_change(
                &state,
                &market_id,
                &market.status,
                "cancelled",
                "void_market_resolution",
            )?;
            "cancelled".clone_into(&mut market.status);
            market.resolved_outcome = Some("VOID".to_owned());
            Some("TXN_TYPE_MARKET_VOID_REFUND")
        }
        "EXTEND_DISPUTE" => {
            "disputed".clone_into(&mut market.status);
            None
        }
        "UPHOLD_INITIAL" | "OVERTURN" => {
            record_status_change(
                &state,
                &market_id,
                &market.status,
                "settling",
                "dispute_resolution",
            )?;
            "settling".clone_into(&mut market.status);
            None
        }
        _ => None,
    };

    Ok((
        StatusCode::CREATED,
        Json(DisputeResolutionResponse {
            resolution_id,
            market_status: market.status.clone(),
            ledger_txn_type,
        }),
    ))
}

async fn settle_market(
    State(state): State<AppState>,
    Path(market_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SettleMarketRequest>,
) -> Result<Json<SettleMarketResponse>, (StatusCode, Json<ApiErrorResponse>)> {
    require_settlement_step_up(&headers)?;
    if request.reviewer.trim().is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "missing_settlement_reviewer",
            "reviewer is required before settlement execution",
        ));
    }

    let mut markets = state.markets.lock().map_err(store_error)?;
    let market = markets.get_mut(&market_id).ok_or_else(|| {
        api_error(
            StatusCode::NOT_FOUND,
            "market_not_found",
            "market was not found",
        )
    })?;
    if market.status != "settling" {
        return Err(api_error(
            StatusCode::CONFLICT,
            "market_not_settling",
            "settlement execution requires a settling market",
        ));
    }

    record_status_change(
        &state,
        &market_id,
        &market.status,
        "settled",
        "settle_batch",
    )?;
    "settled".clone_into(&mut market.status);
    if market.resolved_at.is_none() {
        market.resolved_at = Some(market.close_at.clone());
    }

    Ok(Json(SettleMarketResponse {
        market_status: market.status.clone(),
        reviewer: request.reviewer,
        ledger_txn_type: "TXN_TYPE_MARKET_SETTLE",
    }))
}

async fn list_status_events(
    State(state): State<AppState>,
    Path(market_id): Path<String>,
) -> Result<Json<ListStatusEventsResponse>, (StatusCode, Json<ApiErrorResponse>)> {
    let markets = state.markets.lock().map_err(store_error)?;
    if !markets.contains_key(&market_id) {
        return Err(api_error(
            StatusCode::NOT_FOUND,
            "market_not_found",
            "market was not found",
        ));
    }
    drop(markets);

    let events = state
        .status_events
        .lock()
        .map_err(store_error)?
        .get(&market_id)
        .cloned()
        .unwrap_or_default();
    Ok(Json(ListStatusEventsResponse { events }))
}

fn record_status_change(
    state: &AppState,
    market_id: &str,
    old_status: &str,
    new_status: &str,
    reason: &str,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    if old_status == new_status {
        return Ok(());
    }

    let mut events_by_market = state.status_events.lock().map_err(store_error)?;
    events_by_market
        .entry(market_id.to_owned())
        .or_default()
        .push(MarketStatusEvent {
            market_id: market_id.to_owned(),
            old_status: old_status.to_owned(),
            new_status: new_status.to_owned(),
            reason: reason.to_owned(),
        });
    Ok(())
}

fn require_settlement_step_up(
    headers: &HeaderMap,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    let role = headers
        .get("x-operator-role")
        .and_then(|value| value.to_str().ok());
    let has_operator_role = matches!(
        role,
        Some(
            "oracle_operator"
                | "settlement_reviewer"
                | "dispute_adjudicator"
                | "compliance_officer"
        )
    );
    let has_fresh_mfa = headers
        .get("x-step-up-mfa")
        .and_then(|value| value.to_str().ok())
        == Some("fresh");
    let has_allowlisted_ip = headers
        .get("x-settlement-ip-allowlisted")
        .and_then(|value| value.to_str().ok())
        == Some("true");

    if has_operator_role && has_fresh_mfa && has_allowlisted_ip {
        return Ok(());
    }

    Err(api_error(
        StatusCode::UNAUTHORIZED,
        "settlement_step_up_required",
        "settlement writes require operator role, fresh MFA, and allowlisted IP",
    ))
}

fn validate_adjudicator_votes(
    request: &SubmitDisputeResolutionRequest,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    let adjudicators = request
        .votes
        .iter()
        .map(|vote| vote.adjudicator.as_str())
        .collect::<HashSet<_>>();
    if adjudicators.len() < 3 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "insufficient_adjudicator_votes",
            "at least 3 independent adjudicator votes are required",
        ));
    }

    let matching_votes = request
        .votes
        .iter()
        .filter(|vote| vote.vote == request.decision)
        .count();
    if matching_votes < 2 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "decision_lacks_majority",
            "at least 2 adjudicators must vote for the final decision",
        ));
    }
    Ok(())
}

fn dispute_until(market: &Market) -> String {
    let Some((date, time)) = market.close_at.split_once('T') else {
        return market.close_at.clone();
    };
    let mut parts = date.split('-');
    let (Some(year), Some(month), Some(day)) = (parts.next(), parts.next(), parts.next()) else {
        return market.close_at.clone();
    };
    let Ok(day) = day.parse::<u32>() else {
        return market.close_at.clone();
    };
    if day < 28 {
        return format!("{year}-{month}-{:02}T{time}", day + 1);
    }
    market.close_at.clone()
}

fn api_error(
    status: StatusCode,
    error: &'static str,
    message: impl Into<String>,
) -> (StatusCode, Json<ApiErrorResponse>) {
    (
        status,
        Json(ApiErrorResponse {
            error,
            message: message.into(),
        }),
    )
}

fn store_error<T>(_: std::sync::PoisonError<T>) -> (StatusCode, Json<ApiErrorResponse>) {
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "market_store_unavailable",
        "market store lock is unavailable",
    )
}

fn seed_markets() -> Vec<Market> {
    vec![Market {
        id: "wbsc-2026-tw-semifinal".to_owned(),
        title: "Taiwan reaches the 2026 WBSC semifinal".to_owned(),
        brief: "Seed market used until persistent SPEC-015 storage lands.".to_owned(),
        category: "sports".to_owned(),
        status: "open".to_owned(),
        yes_price_cents: 38,
        yes_bid_cents: 37,
        yes_ask_cents: 39,
        fee_bps: 400,
        volume_24h_points: 184_500,
        volume_total_points: 2_840_300,
        opened_at: "2026-04-02T00:00:00+08:00".to_owned(),
        close_at: "2026-09-15T20:00:00+08:00".to_owned(),
        oracle_source: "WBSC official result".to_owned(),
        resolved_outcome: None,
        resolved_at: None,
    }]
}

async fn shutdown_signal() {
    if let Err(error) = signal::ctrl_c().await {
        tracing::warn!(%error, "failed to install ctrl-c handler");
    }
}
