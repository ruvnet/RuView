//! The `auth.cognitum.one` OAuth surface: authorize URL, `POST /oauth/token`
//! (`authorization_code` and `refresh_token` grants), and
//! `POST /v1/oauth/code-exchange` (the OOB fallback).
//!
//! Ported from `cognitum-one/meta-proxy` `src/oauth/client.rs`, with the
//! refresh grant kept — meta-proxy discards its access token after one use,
//! but a RuView session is long-lived and must refresh.
//!
//! **Target the identity origin, not the console.** metaharness ADR-119 found
//! `dashboard.cognitum.one` returns 405 for `POST /oauth/token` (the console
//! SPA swallows the route). `auth.cognitum.one` is the correct direct target.

use serde::{Deserialize, Serialize};

/// RuView's registered client (identity migration `0017`).
pub const CLIENT_ID: &str = "ruview";

/// RFC 8252 out-of-band sentinel. Must match
/// `services/identity/src/oauth/client.rs::FALLBACK_REDIRECT_URI` exactly.
pub const OOB_REDIRECT_URI: &str = "urn:ietf:wg:oauth:2.0:oob";

pub const DEFAULT_AUTH_BASE_URL: &str = "https://auth.cognitum.one";

/// Override the issuer origin (staging, a local identity, a mirror).
pub const AUTH_URL_ENV: &str = "RUVIEW_COGNITUM_AUTH_URL";

/// Override the client id.
///
/// Exists because Cognitum has no dynamic client registration, and products
/// have historically borrowed a registered id while their own was pending —
/// musica shipped as `meta-proxy` for exactly this reason. RuView has its own
/// row now, so this is an escape hatch, not the normal path.
pub const CLIENT_ID_ENV: &str = "RUVIEW_COGNITUM_CLIENT_ID";

pub fn auth_base_url() -> String {
    std::env::var(AUTH_URL_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .unwrap_or_else(|| DEFAULT_AUTH_BASE_URL.to_string())
}

pub fn client_id() -> String {
    std::env::var(CLIENT_ID_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| CLIENT_ID.to_string())
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("network error talking to the authorization server: {0}")]
    Network(#[from] reqwest::Error),
    #[error("authorization server rejected the request: {error} — {description}")]
    Protocol { error: String, description: String },
    #[error("unexpected response shape from the authorization server")]
    UnexpectedShape,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub account_email: Option<String>,
    /// The **rotating** refresh token. Identity revokes the presented one and
    /// returns a replacement; see [`refresh`].
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Access-token lifetime in seconds (identity issues 900). Absent ⇒ treat
    /// the token as already needing refresh rather than assuming a default.
    #[serde(default)]
    pub expires_in: Option<i64>,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

async fn parse_token_response(resp: reqwest::Response) -> Result<TokenResponse, OAuthError> {
    let status = resp.status();
    let body = resp.text().await?;
    if status.is_success() {
        return serde_json::from_str::<TokenResponse>(&body)
            .map_err(|_| OAuthError::UnexpectedShape);
    }
    // A non-JSON error body (an HTML error page, a proxy timeout) must not
    // panic or masquerade as a protocol error we understand.
    match serde_json::from_str::<ErrorBody>(&body) {
        Ok(e) => Err(OAuthError::Protocol {
            error: e.error.unwrap_or_else(|| status.to_string()),
            description: e
                .error_description
                .unwrap_or_else(|| "no description supplied".into()),
        }),
        Err(_) => Err(OAuthError::UnexpectedShape),
    }
}

/// Build the `/oauth/authorize` URL.
///
/// Uses a real URL encoder rather than `format!` so a scope containing a space
/// (`"sensing:read sensing:admin"`) is encoded correctly — hand-formatting this
/// is how a client ends up sending a truncated scope and getting a baffling
/// `Unknown scope`.
pub fn authorize_url(redirect_uri: &str, state: &str, code_challenge: &str, scope: &str) -> String {
    let mut url = url::Url::parse(&format!("{}/oauth/authorize", auth_base_url()))
        .expect("auth base URL is a valid URL");
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &client_id())
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state)
        .append_pair("scope", scope);
    url.to_string()
}

/// `POST /oauth/token`, `grant_type=authorization_code`.
pub async fn exchange_code(
    http: &reqwest::Client,
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, OAuthError> {
    let resp = http
        .post(format!("{}/oauth/token", auth_base_url()))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", code_verifier),
            ("client_id", &client_id()),
            ("redirect_uri", redirect_uri),
        ])
        .send()
        .await?;
    parse_token_response(resp).await
}

/// Upper bound on one refresh request, connect to last body byte. A request
/// that hangs must end, and is then treated as spent (see the store).
pub const REFRESH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

fn refresh_timeout() -> std::time::Duration {
    // Tests shorten it; release builds never read the environment for it.
    #[cfg(test)]
    if let Some(ms) = std::env::var("RUVIEW_TEST_REFRESH_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        return std::time::Duration::from_millis(ms);
    }
    REFRESH_TIMEOUT
}

/// `POST /oauth/token`, `grant_type=refresh_token`. Bounded by [`REFRESH_TIMEOUT`].
///
/// **Identity rotates refresh tokens with reuse detection.** The response
/// carries a NEW refresh token and spends the old one; presenting a spent token
/// revokes the entire session family. Two consequences the caller must honour:
///
/// 1. Persist the returned `refresh_token` **before** using the new access
///    token — a crash in between otherwise strands the session.
/// 2. Never retry a failed refresh with the same token. A timeout is not proof
///    the server did not consume it.
///
/// [`super::store::Session::ensure_fresh`] does both; prefer it to calling this
/// directly.
pub async fn refresh(
    http: &reqwest::Client,
    refresh_token: &str,
) -> Result<TokenResponse, OAuthError> {
    let resp = http
        .post(format!("{}/oauth/token", auth_base_url()))
        .timeout(refresh_timeout())
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &client_id()),
        ])
        .send()
        .await?;
    parse_token_response(resp).await
}

/// `POST /v1/oauth/code-exchange` — the OOB manual-paste fallback for hosts
/// with no browser and no reachable loopback (SSH into a Pi, a container).
pub async fn exchange_manual_code(
    http: &reqwest::Client,
    code: &str,
    code_verifier: &str,
) -> Result<TokenResponse, OAuthError> {
    #[derive(Serialize)]
    struct Req<'a> {
        code: &'a str,
        code_verifier: &'a str,
        client_id: &'a str,
    }
    let resp = http
        .post(format!("{}/v1/oauth/code-exchange", auth_base_url()))
        .json(&Req {
            code,
            code_verifier,
            client_id: &client_id(),
        })
        .send()
        .await?;
    parse_token_response(resp).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_url_targets_the_identity_origin_not_the_console() {
        // The console origin 405s POST /oauth/token (metaharness ADR-119).
        let u = authorize_url("http://127.0.0.1:1/oauth/callback", "s", "c", "sensing:read");
        assert!(u.starts_with("https://auth.cognitum.one/oauth/authorize"), "{u}");
        assert!(!u.contains("dashboard.cognitum.one"));
    }

    #[test]
    fn authorize_url_carries_every_required_parameter() {
        let u = authorize_url("http://127.0.0.1:1/oauth/callback", "st8", "chal", "sensing:read");
        for expected in [
            "response_type=code",
            "client_id=ruview",
            "code_challenge=chal",
            "code_challenge_method=S256",
            "state=st8",
        ] {
            assert!(u.contains(expected), "missing {expected} in {u}");
        }
    }

    #[test]
    fn a_multi_scope_request_is_url_encoded_not_truncated() {
        // The space in "sensing:read sensing:admin" must survive as %20/+.
        // Hand-formatting this is how a client silently requests one scope.
        let u = authorize_url("http://127.0.0.1:1/oauth/callback", "s", "c", "sensing:read sensing:admin");
        assert!(
            u.contains("scope=sensing%3Aread+sensing%3Aadmin")
                || u.contains("scope=sensing%3Aread%20sensing%3Aadmin"),
            "scope not encoded correctly: {u}"
        );
    }

    #[test]
    fn the_oob_sentinel_matches_the_servers_constant_exactly() {
        // Any drift here fails the headless path with an opaque redirect_uri
        // mismatch.
        assert_eq!(OOB_REDIRECT_URI, "urn:ietf:wg:oauth:2.0:oob");
    }

    #[test]
    fn the_default_client_id_is_ruviews_own_registration() {
        assert_eq!(CLIENT_ID, "ruview");
    }
}
