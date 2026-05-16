use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default)]
pub struct InMemoryRiskEventStore {
    events: Arc<Mutex<Vec<MemberActionRiskEvent>>>,
}

impl InMemoryRiskEventStore {
    #[must_use]
    pub fn events_for_user(&self, target_user_id: &str) -> Vec<MemberActionRiskEvent> {
        self.events
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .filter(|event| event.target_user_id == target_user_id)
            .cloned()
            .collect()
    }

    fn record(&self, request: MemberActionRiskEventRequest) -> MemberActionRiskEvent {
        let mut events = self
            .events
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let event = MemberActionRiskEvent {
            event_id: format!("risk-member-action-{:06}", events.len() + 1),
            target_user_id: request.target_user_id,
            action_id: request.action_id,
            action_type: request.action_type,
            correlation_id: request.correlation_id,
            reason: request.reason,
            source: "risk-antifraud-service:member-actions".to_owned(),
        };
        events.push(event.clone());
        event
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberActionRiskEvent {
    pub event_id: String,
    pub target_user_id: String,
    pub action_id: String,
    pub action_type: String,
    pub correlation_id: String,
    pub reason: String,
    pub source: String,
}

#[derive(Debug, Deserialize)]
pub struct MemberActionRiskEventRequest {
    pub target_user_id: String,
    pub action_id: String,
    pub action_type: String,
    pub correlation_id: String,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    target_user_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListResponse {
    events: Vec<MemberActionRiskEvent>,
    source: &'static str,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: &'static str,
}

pub fn router(store: InMemoryRiskEventStore) -> Router {
    Router::new()
        .route("/internal/risk/member-actions", post(record_event))
        .route("/internal/risk/member-actions", get(list_events))
        .with_state(store)
}

async fn record_event(
    State(store): State<InMemoryRiskEventStore>,
    Json(request): Json<MemberActionRiskEventRequest>,
) -> Result<(StatusCode, Json<MemberActionRiskEvent>), (StatusCode, Json<ErrorResponse>)> {
    if request.target_user_id.trim().is_empty()
        || request.action_id.trim().is_empty()
        || request.action_type.trim().is_empty()
        || request.correlation_id.trim().is_empty()
        || request.reason.trim().is_empty()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_member_action_risk_event",
            }),
        ));
    }
    Ok((StatusCode::CREATED, Json(store.record(request))))
}

async fn list_events(
    State(store): State<InMemoryRiskEventStore>,
    Query(query): Query<ListQuery>,
) -> Json<ListResponse> {
    let events = query
        .target_user_id
        .as_deref()
        .map_or_else(Vec::new, |target_user_id| {
            store.events_for_user(target_user_id)
        });
    Json(ListResponse {
        events,
        source: "risk-antifraud-service:member-actions",
    })
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    #[tokio::test]
    async fn records_and_lists_member_action_risk_event() {
        let store = super::InMemoryRiskEventStore::default();
        let app = super::router(store);

        let created = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/risk/member-actions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "target_user_id": "MEM-1",
                            "action_id": "mem-act-1",
                            "action_type": "restrict_withdrawals",
                            "correlation_id": "corr-1",
                            "reason": "risk hold"
                        })
                        .to_string(),
                    ))
                    .expect("request builds"),
            )
            .await
            .expect("request handled");
        assert_eq!(created.status(), StatusCode::CREATED);
        assert_eq!(
            response_json(created).await["event_id"],
            "risk-member-action-000001"
        );

        let listed = app
            .oneshot(
                Request::builder()
                    .uri("/internal/risk/member-actions?target_user_id=MEM-1")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("request handled");
        assert_eq!(listed.status(), StatusCode::OK);
        let body = response_json(listed).await;
        assert_eq!(body["events"][0]["action_type"], "restrict_withdrawals");
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let collected = response
            .into_body()
            .collect()
            .await
            .expect("response body should collect");
        serde_json::from_slice(&collected.to_bytes()).expect("response should be json")
    }
}
