//! Authentication, OAuth browser sign-in, and single-use WebSocket ticket endpoints (ADR-271, ADR-272).

use axum::{
    extract::{Extension, Query, Request},
    http::{header, HeaderMap, StatusCode},
    response::{AppendHeaders, IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use wifi_densepose_sensing_server::bearer_auth::AuthState;
use wifi_densepose_sensing_server::browser_session as bs;
use wifi_densepose_sensing_server::ws_ticket::{TicketGrant, TICKET_TTL};
use crate::SharedState;

/// Create sub-router for auth & OAuth endpoints.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/v1/ws-ticket", post(ws_ticket_handler))
        .route("/oauth/start", get(oauth_start))
        .route("/oauth/callback", get(oauth_callback))
        .route("/oauth/logout", get(oauth_logout))
        .route("/oauth/status", get(oauth_status))
}

pub(crate) async fn ws_ticket_handler(
    Extension(auth): Extension<AuthState>,
    request: Request,
) -> Response {
    let principal = request.extensions().get::<ruview_auth::Principal>();
    let grant = TicketGrant {
        scopes: principal.map(|p| p.scopes().collect::<Vec<_>>().join(" ")),
        subject: principal.map(|p| p.subject.clone()),
    };

    match auth.tickets().issue(grant) {
        Some(ticket) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "ticket": ticket,
                "expires_in_secs": TICKET_TTL.as_secs(),
                "usage": "append as ?ticket=<value> to the WebSocket URL; valid once",
            })),
        )
            .into_response(),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "too many outstanding WebSocket tickets; retry shortly\n",
        )
            .into_response(),
    }
}

fn request_is_tls(headers: &HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|p| p.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
        || bs::public_base_url().starts_with("https://")
}

pub(crate) async fn oauth_start(
    Extension(auth): Extension<AuthState>,
    headers: HeaderMap,
) -> Response {
    let Some(issuer) = auth.oauth_issuer() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "OAuth is not enabled on this server (set RUVIEW_OAUTH_ISSUER)\n",
        )
            .into_response();
    };
    let secure = request_is_tls(&headers);
    match bs::begin(&issuer, &auth.primary_client_id(), bs::BROWSER_SIGNIN_SCOPE, secure) {
        Ok((location, cookie)) => (
            StatusCode::FOUND,
            [
                (header::LOCATION, location),
                (header::SET_COOKIE, cookie),
            ],
        )
            .into_response(),
        Err(e) => (StatusCode::SERVICE_UNAVAILABLE, format!("{e}\n")).into_response(),
    }
}

#[derive(serde::Deserialize)]
pub(crate) struct OAuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub(crate) async fn oauth_callback(
    Extension(auth): Extension<AuthState>,
    headers: HeaderMap,
    Query(q): Query<OAuthCallbackQuery>,
) -> Response {
    let secure = request_is_tls(&headers);
    let bad = |code: StatusCode, msg: String| {
        (code, [(header::SET_COOKIE, bs::clear_transaction(secure))], msg).into_response()
    };

    if let Some(err) = q.error {
        return bad(StatusCode::BAD_REQUEST, format!("Cognitum declined the sign-in: {err}\n"));
    }
    let (Some(code), Some(state)) = (q.code, q.state) else {
        return bad(StatusCode::BAD_REQUEST, "Incomplete sign-in response\n".into());
    };
    let cookie_header = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();

    let verifier = match bs::verifier_for_callback(&cookie_header, &state) {
        Ok(v) => v,
        Err(e) => return bad(StatusCode::BAD_REQUEST, format!("{e}\n")),
    };

    let Some(issuer) = auth.oauth_issuer() else {
        return bad(StatusCode::SERVICE_UNAVAILABLE, "OAuth is not enabled\n".into());
    };
    let client_id = auth.primary_client_id();

    let exchange = tokio::task::spawn_blocking(move || {
        ureq::post(&format!("{issuer}/oauth/token"))
            .send_form(&[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("code_verifier", &verifier),
                ("client_id", &client_id),
                ("redirect_uri", &bs::redirect_uri()),
            ])
            .map_err(|e| e.to_string())
            .and_then(|r| r.into_string().map_err(|e| e.to_string()))
    })
    .await;

    let body = match exchange {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => return bad(StatusCode::BAD_GATEWAY, format!("token exchange failed: {e}\n")),
        Err(e) => return bad(StatusCode::INTERNAL_SERVER_ERROR, format!("token exchange task failed: {e}\n")),
    };
    let access_token = match serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("access_token")?.as_str().map(str::to_owned))
    {
        Some(t) => t,
        None => return bad(StatusCode::BAD_GATEWAY, "token endpoint returned no access_token\n".into()),
    };

    let principal = match auth.verify_for_browser(&access_token) {
        Ok(p) => p,
        Err(e) => return bad(StatusCode::UNAUTHORIZED, format!("{e}\n")),
    };

    let session_cookie = match bs::issue(&principal, secure) {
        Ok(c) => c,
        Err(e) => return bad(StatusCode::SERVICE_UNAVAILABLE, format!("{e}\n")),
    };
    tracing::info!(sub = %principal.subject, "browser sign-in complete");

    (
        StatusCode::FOUND,
        AppendHeaders([
            (header::LOCATION, format!("/ui/?signed_in={}", now_millis())),
            (header::SET_COOKIE, session_cookie),
            (header::SET_COOKIE, bs::clear_transaction(secure)),
        ]),
    )
        .into_response()
}

pub(crate) async fn oauth_logout(headers: HeaderMap) -> Response {
    let secure = request_is_tls(&headers);
    (
        StatusCode::FOUND,
        AppendHeaders([
            (header::LOCATION, format!("/ui/?signed_out={}", now_millis())),
            (header::SET_COOKIE, bs::clear_session(secure)),
            (header::SET_COOKIE, bs::clear_transaction(secure)),
        ]),
    )
        .into_response()
}

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

pub(crate) async fn oauth_status(
    Extension(auth): Extension<AuthState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let raw = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok());
    let session = raw.and_then(bs::from_cookie_header);
    Json(serde_json::json!({
        "auth_required": auth.is_enabled(),
        "oauth_enabled": auth.oauth_enabled(),
        "browser_signin": auth.oauth_enabled() && bs::is_configured(),
        "signed_in": session.is_some(),
        "account": session.as_ref().map(|s| s.account_id.clone()),
        "scope": session.as_ref().map(|s| s.scope.clone()),
    }))
}
