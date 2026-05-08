//! PoC-A Step 2 — Server-side EdDSA token issue + verify reference.
//!
//! Standalone Rust binary that mirrors what `services/user-service` will do
//! once SPEC-005 §0.3 wires real auth. Engineer should copy this into a
//! scratch crate (NOT into services/) and run with `cargo run` to exercise
//! the round-trip across the 5 client environments listed in README §2.3.
//!
//! Dependencies (add to Cargo.toml of the scratch crate):
//!
//! ```toml
//! [package]
//! name = "eddsa-poc-server"
//! version = "0.0.0"
//! edition = "2024"
//! publish = false
//!
//! [dependencies]
//! axum         = "0.7"
//! tokio        = { version = "1", features = ["full"] }
//! jsonwebtoken = "9"
//! serde        = { version = "1", features = ["derive"] }
//! serde_json   = "1"
//! tower-http   = { version = "0.5", features = ["cors"] }
//! ed25519-dalek = "2"
//! base64       = "0.22"
//! rand_core    = "0.6"
//! ```
//!
//! Run:
//! ```bash
//! cargo run --bin eddsa-poc-server  # listens on 0.0.0.0:8888
//! ```
//!
//! Then expose via `ngrok http 8888` so a phone-side LIFF webview can reach it.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use ed25519_dalek::{SigningKey, VerifyingKey};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode,
};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

const COOKIE_NAME: &str = "proph_session";
const ACCESS_TOKEN_TTL_SEC: u64 = 15 * 60;

#[derive(Clone)]
struct AppState {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    iss: String,
    aud: String,
    sub: String,
    iat: u64,
    exp: u64,
    proph_kyc_tier: u8,
    proph_country: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Generate a fresh Ed25519 key pair on every boot (PoC only; production
    // pulls from Vault per SPEC-010 §3.3).
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key: VerifyingKey = signing_key.verifying_key();
    let pkcs8 = signing_key.to_pkcs8_der().expect("pkcs8");
    let spki = verifying_key.to_public_key_der().expect("spki");

    let state = Arc::new(AppState {
        encoding_key: EncodingKey::from_ed_der(pkcs8.as_bytes()),
        decoding_key: DecodingKey::from_ed_der(spki.as_bytes()),
    });

    let app = Router::new()
        .route("/api/auth/login", post(login))
        .route("/api/me", get(me))
        .layer(CorsLayer::very_permissive()) // PoC only; production uses strict CORS
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8888").await?;
    eprintln!("eddsa-poc-server listening on http://0.0.0.0:8888");
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Deserialize)]
struct LoginBody {
    user_id: String,
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginBody>,
) -> Response {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let claims = Claims {
        iss: "auth.prophetica".into(),
        aud: "api.prophetica".into(),
        sub: body.user_id,
        iat: now,
        exp: now + ACCESS_TOKEN_TTL_SEC,
        proph_kyc_tier: 1,
        proph_country: "TW".into(),
    };

    let header = Header::new(Algorithm::EdDSA);
    let token = match encode(&header, &claims, &state.encoding_key) {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    // HTTP-only cookie; SameSite=Lax to allow LIFF webview to round-trip on subsequent fetch.
    // Production should add `Secure` (TLS-only) and a longer Path scoping.
    let cookie = format!(
        "{}={}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}",
        COOKIE_NAME, token, ACCESS_TOKEN_TTL_SEC
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("cookie header"),
    );
    (StatusCode::OK, headers, Json(serde_json::json!({"ok": true}))).into_response()
}

async fn me(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = cookie
        .split(';')
        .map(str::trim)
        .find_map(|kv| kv.strip_prefix(&format!("{}=", COOKIE_NAME)));

    let Some(token) = token else {
        return (StatusCode::UNAUTHORIZED, "no session").into_response();
    };

    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.set_audience(&["api.prophetica"]);
    validation.set_issuer(&["auth.prophetica"]);

    match decode::<Claims>(token, &state.decoding_key, &validation) {
        Ok(data) => Json(data.claims).into_response(),
        Err(e) => (StatusCode::UNAUTHORIZED, e.to_string()).into_response(),
    }
}
