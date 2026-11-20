//! Opt-in bearer-token auth for the sensing-server HTTP API (#443).
//!
//! When the `RUVIEW_API_TOKEN` environment variable is set, every request
//! whose path begins with `/api/v1/` must carry a matching
//! `Authorization: Bearer <token>` header, otherwise the server responds with
//! `401 Unauthorized`. When the env var is unset (or empty), the middleware is
//! a no-op and the API stays unauthenticated — preserving the long-standing
//! LAN-only deployment posture documented in the issue. This is a binary,
//! deployment-time switch with **no default authentication change**.
//!
//! Endpoints outside `/api/v1/*` (`/health*`, `/ws/sensing`, the static `/ui/*`
//! mount, `/`) are intentionally **not** gated:
//! * `/health*` is the liveness/readiness probe that orchestrators hit
//!   anonymously;
//! * `/ws/sensing` and `/ui/*` are served to local browsers that can't easily
//!   inject headers — the sensitive control plane is the `/api/v1/*` tree, and
//!   that is what this layer protects.
//!
//! The header check uses a length-then-byte constant-time compare to avoid
//! leaking the token through timing.
//!
//! # Cognitum OAuth (ADR-271)
//!
//! A second, **additive** credential is supported: a Cognitum OAuth access
//! token, verified offline against `auth.cognitum.one`'s published JWKS. It is
//! enabled by setting [`OAUTH_ISSUER_ENV`], and the two schemes layer:
//!
//! 1. If `RUVIEW_API_TOKEN` is set and the presented bearer matches it exactly,
//!    the request is allowed — byte-for-byte today's behaviour.
//! 2. Otherwise, if OAuth is configured, the bearer is verified as a JWT and
//!    must carry the scope the route requires.
//! 3. Otherwise `401`.
//!
//! Order matters for compatibility, not for security: a static token that
//! matches is not a JWT, and a JWT never matches the static token. Trying the
//! static compare first means an existing deployment's behaviour is unchanged
//! even with OAuth switched on.
//!
//! **Nothing here weakens the unset case.** With neither variable set the
//! middleware is the same no-op it has always been.
//!
//! ## Scope gating
//!
//! Not every route carries the same blast radius, so a single "authenticated"
//! bit is too coarse once we have scopes. [`required_scope_for`] maps a request
//! to `sensing:read` or `sensing:admin` — see its docs for the split and why it
//! is drawn where it is.

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use ruview_auth::{scope, verify_access_token, JwksCache, UreqFetcher, VerifierConfig};

/// Environment variable that gates the middleware. Unset / empty ⇒ auth off.
pub const API_TOKEN_ENV: &str = "RUVIEW_API_TOKEN";

/// Issuer origin of the Cognitum authorization server. Setting this enables
/// OAuth verification; unset ⇒ OAuth off and behaviour is unchanged.
pub const OAUTH_ISSUER_ENV: &str = "RUVIEW_OAUTH_ISSUER";

/// Optional JWKS override. Defaults to `<issuer>/.well-known/jwks.json`, which
/// is where RFC 8414 metadata points for `auth.cognitum.one`. Overridable so a
/// staging issuer or an air-gapped mirror can be pointed at without a rebuild.
pub const OAUTH_JWKS_URL_ENV: &str = "RUVIEW_OAUTH_JWKS_URL";

/// The production Cognitum issuer, for operators who just want it on.
pub const COGNITUM_ISSUER: &str = "https://auth.cognitum.one";

/// Comma-separated `client_id` values whose tokens this server accepts — the
/// AUDIENCE control. Defaults to [`DEFAULT_CLIENT_ID`].
///
/// Cognitum tokens carry no `aud`; `client_id` is the platform's stand-in, and
/// `cognitum-one/freetokens` enforces exactly this. Set to `*` to accept any
/// Cognitum client — only sensible while borrowing another product's
/// registration, and it means any Cognitum token opens this server.
pub const OAUTH_CLIENT_IDS_ENV: &str = "RUVIEW_OAUTH_CLIENT_IDS";

/// RuView's own registered OAuth client (identity migration `0017`).
pub const DEFAULT_CLIENT_ID: &str = "ruview";

fn allowed_client_ids() -> Vec<String> {
    match std::env::var(OAUTH_CLIENT_IDS_ENV) {
        Ok(v) if v.trim() == "*" => Vec::new(), // explicit opt-out
        Ok(v) if !v.trim().is_empty() => {
            let parsed: Vec<String> = v
                .split(',')
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty())
                .collect();
            // An empty Vec is the OPT-OUT sentinel downstream: `verify.rs` skips
            // the audience check entirely when the allowlist is empty. So a value
            // that is non-empty but parses to nothing — `","`, `" , "`, a stray
            // trailing comma — would silently disable the audience boundary and
            // admit a token minted for any other Cognitum product. Only the
            // literal `*` may turn that check off.
            if parsed.is_empty() {
                tracing::warn!(
                    value = %v,
                    "{OAUTH_CLIENT_IDS_ENV} is set but lists no client id; falling back to \
                     {DEFAULT_CLIENT_ID}. Use `*` if you really mean to accept any client."
                );
                return vec![DEFAULT_CLIENT_ID.to_string()];
            }
            parsed
        }
        _ => vec![DEFAULT_CLIENT_ID.to_string()],
    }
}

/// Path prefix the middleware protects when auth is enabled.
///
/// Retained because it names the bulk of the protected surface and is asserted
/// in tests, but it is NO LONGER the rule. The gate is [`is_anonymous`] —
/// deny-by-default. See that function for why.
pub const PROTECTED_PREFIX: &str = "/api/v1/";

/// Paths that stay reachable with no credential when auth is enabled.
///
/// # Why an allowlist
///
/// The gate used to be the inverse: protect `/api/v1/*`, let everything else
/// through. That is exposure-by-default, and it leaked. `/api/field` — the REST
/// sibling of `/ws/field`, serving the same signed `FieldEvent` stream of live
/// presence, pose and vitals — sits at `/api/field`, not `/api/v1/`, so it
/// returned `200` with no credential on BOTH listeners while `/api/v1/models`
/// correctly returned `401`. `/ws/field` was gated in this same PR; its REST
/// twin one path segment over was not.
///
/// Measured, with `RUVIEW_API_TOKEN` set and no credential supplied:
/// `/api/v1/models` -> 401, `/ws/field` -> 401, `/api/field` -> 200 on :8080
/// and :8765.
///
/// With an allowlist, a route added at a new path is gated because nobody
/// remembered to expose it, rather than exposed because nobody remembered to
/// protect it. That is the same inversion already applied to the scope gate in
/// [`required_scope_for`].
const ANONYMOUS_PREFIXES: &[&str] = &[
    // Orchestrator and load-balancer probes. Documented exemption (ADR-272),
    // pinned by `health_stays_anonymous_on_both_listeners`.
    "/health",
    // Sign-in cannot require being signed in.
    "/oauth/",
];

/// Is this path reachable without a credential? See [`ANONYMOUS_PREFIXES`].
pub fn is_anonymous(path: &str) -> bool {
    // The dashboard shell itself, mounted with `nest_service("/ui", …)`. It has
    // to load for the user to reach the sign-in button at all; the data it then
    // fetches is what is protected.
    if path == "/" || path == "/ui" || path.starts_with("/ui/") {
        return true;
    }
    ANONYMOUS_PREFIXES.iter().any(|p| path.starts_with(p))
}

/// WebSocket upgrade endpoints. Previously ungated — `/ws/*` sat outside
/// [`PROTECTED_PREFIX`] and `/api/v1/stream/pose` was an explicit exemption —
/// because a browser's `WebSocket` constructor cannot attach an
/// `Authorization` header to the handshake.
///
/// Measured consequence, with `RUVIEW_API_TOKEN` set and a real handshake
/// carrying no credential: all three returned `101 Switching Protocols` while
/// `/api/v1/models` returned `401`. The control plane was locked and the data
/// plane — live presence, pose, vitals — was open.
///
/// They are now gated, and accept **either** a bearer (native clients, which
/// are not browser-constrained) **or** a single-use ticket (ADR-272).
///
/// This list is the set that exists today, used for the boot warning and tests.
/// The runtime rule is [`is_ws_path`], which matches by prefix so routes added
/// later are gated without an edit here.
pub const WS_PATHS: &[&str] = &[
    "/ws/sensing",
    "/ws/introspection",
    "/api/v1/stream/pose",
];

/// Restore the pre-ADR-272 behaviour: WebSocket upgrades accepted with no
/// credential even when auth is on.
///
/// A migration aid, not a supported configuration. It exists because gating
/// these paths breaks a browser UI that has not yet been updated to fetch a
/// ticket, and some deployments cannot update server and UI in lockstep. It
/// logs a warning naming the exposure on every boot, deliberately hard to
/// ignore in a log.
pub const LEGACY_WS_ENV: &str = "RUVIEW_WS_LEGACY_UNAUTHENTICATED";

fn legacy_ws_unauthenticated() -> bool {
    matches!(
        std::env::var(LEGACY_WS_ENV).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    )
}

/// WebSocket upgrade paths that do NOT live under [`WS_PREFIX`] and so must be
/// named explicitly.
pub const WS_PATHS_OUTSIDE_PREFIX: &[&str] = &["/api/v1/stream/pose"];

/// Everything under here is treated as a WebSocket upgrade.
pub const WS_PREFIX: &str = "/ws/";

/// Is this a WebSocket upgrade path?
///
/// Matched by **prefix**, not by an allowlist, and that choice is the whole
/// point. An allowlist means every WebSocket route added later is ungated until
/// someone remembers to add it here — which is exactly the bug this module just
/// fixed, reintroduced on a delay. `/ws/train/progress` (ADR-186, arriving with
/// PR #1387) is already referenced by `ui/services/training.service.js` and
/// would have shipped unauthenticated under an allowlist.
///
/// `/api/v1/stream/pose` is the one upgrade endpoint outside the prefix, so it
/// is named. New WebSocket routes should go under `/ws/` and inherit gating for
/// free.
fn is_ws_path(path: &str) -> bool {
    path.starts_with(WS_PREFIX) || WS_PATHS_OUTSIDE_PREFIX.contains(&path)
}

/// Cognitum OAuth verification state. Built once at boot and shared.
pub struct OAuthState {
    jwks: JwksCache,
    issuer: String,
    allowed_client_ids: Vec<String>,
}

impl std::fmt::Debug for OAuthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthState")
            .field("issuer", &self.issuer)
            .finish_non_exhaustive()
    }
}

/// Why OAuth could not be configured. Every variant is fatal at boot — see
/// [`AuthState::from_env`]'s contract about failing closed.
#[derive(Debug, thiserror::Error)]
pub enum OAuthConfigError {
    #[error("{OAUTH_ISSUER_ENV} is set but empty")]
    EmptyIssuer,
    #[error("JWKS at {url} is unreachable, so no token could ever be verified: {source}")]
    JwksUnreachable {
        url: String,
        #[source]
        source: ruview_auth::JwksError,
    },
}

/// Cheap, cloneable handle to the configured credentials.
///
/// `Debug` is hand-written and REDACTING: this holds the raw
/// `RUVIEW_API_TOKEN`. A derived impl would print it in full the first time
/// anyone writes `tracing::debug!(?auth, ...)`. `OAuthState` and `TicketStore`
/// already redact for the same reason; the type actually holding the secret
/// should not be the one that does not.
#[derive(Clone, Default)]
pub struct AuthState {
    /// The expected static bearer token, if any.
    token: Option<Arc<String>>,
    /// Cognitum OAuth verification, if enabled.
    oauth: Option<Arc<OAuthState>>,
    /// Single-use WebSocket tickets (ADR-272).
    tickets: crate::ws_ticket::TicketStore,
    /// Cached at construction so a mid-flight env change cannot silently open
    /// the WebSocket paths on a running server.
    legacy_ws: bool,
}

impl std::fmt::Debug for AuthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthState")
            .field("static_token", &self.token.as_ref().map(|_| "<redacted>"))
            .field("oauth", &self.oauth)
            .field("tickets", &self.tickets)
            .field("legacy_ws", &self.legacy_ws)
            .finish()
    }
}

impl AuthState {
    /// Build an [`AuthState`] from an explicit string. Empty ⇒ disabled.
    pub fn from_token(t: impl Into<String>) -> Self {
        let s = t.into();
        if s.is_empty() {
            AuthState::default()
        } else {
            AuthState {
                token: Some(Arc::new(s)),
                oauth: None,
                tickets: crate::ws_ticket::TicketStore::new(),
                legacy_ws: legacy_ws_unauthenticated(),
            }
        }
    }

    /// Read the auth configuration from the process environment.
    ///
    /// **Fails closed.** If OAuth is requested but cannot be made to work — the
    /// issuer is empty, or the JWKS cannot be fetched at boot — this returns
    /// `Err` and the caller must refuse to serve `/api/v1/*`. Starting anyway
    /// would mean an operator who asked for OAuth silently gets either an open
    /// API or a single-shared-secret one, which is precisely the failure mode
    /// that makes people distrust an auth switch.
    ///
    /// The JWKS is fetched eagerly for the same reason: a misconfigured
    /// `jwks_uri` should fail at boot with a legible message, not as a puzzling
    /// 401 on some user's first request an hour later.
    pub fn from_env() -> Result<Self, OAuthConfigError> {
        let token = match std::env::var(API_TOKEN_ENV) {
            Ok(s) if !s.is_empty() => Some(Arc::new(s)),
            _ => None,
        };

        let oauth = match std::env::var(OAUTH_ISSUER_ENV) {
            Ok(issuer) if !issuer.trim().is_empty() => {
                let issuer = issuer.trim().trim_end_matches('/').to_string();
                let jwks_url = std::env::var(OAUTH_JWKS_URL_ENV)
                    .ok()
                    .filter(|u| !u.trim().is_empty())
                    .unwrap_or_else(|| format!("{issuer}/.well-known/jwks.json"));

                let jwks = JwksCache::new(jwks_url.clone(), Box::new(UreqFetcher::new()));
                let key_count =
                    jwks.warm()
                        .map_err(|source| OAuthConfigError::JwksUnreachable {
                            url: jwks_url.clone(),
                            source,
                        })?;
                tracing::info!(
                    issuer = %issuer,
                    jwks_url = %jwks_url,
                    key_count,
                    "Cognitum OAuth enabled for /api/v1/*"
                );
                let allowed_client_ids = allowed_client_ids();
                if allowed_client_ids.is_empty() {
                    tracing::warn!(
                        "{OAUTH_CLIENT_IDS_ENV}=* — this server accepts a Cognitum token minted \
                         for ANY product, not just RuView. `client_id` is the platform's stand-in \
                         for `aud`; disabling it leaves scope as the only boundary."
                    );
                } else {
                    tracing::info!(accepted_clients = ?allowed_client_ids, "OAuth audience restricted");
                }
                Some(Arc::new(OAuthState { jwks, issuer, allowed_client_ids }))
            }
            Ok(_) => return Err(OAuthConfigError::EmptyIssuer),
            Err(_) => None,
        };

        let legacy_ws = legacy_ws_unauthenticated();
        if legacy_ws && (token.is_some() || oauth.is_some()) {
            tracing::warn!(
                "{LEGACY_WS_ENV} is set: WebSocket upgrades ({}) accept connections with NO \
                 credential even though API auth is ON. The live sensing stream — presence, \
                 pose and vital signs — is readable by anyone who can reach this port. This is \
                 a migration aid for UIs not yet updated to fetch a ticket; unset it as soon \
                 as the UI is updated.",
                WS_PATHS.join(", ")
            );
        }
        Ok(AuthState {
            token,
            oauth,
            tickets: crate::ws_ticket::TicketStore::new(),
            legacy_ws,
        })
    }

    /// The ticket store, for the `POST /api/v1/ws-ticket` handler.
    pub fn tickets(&self) -> &crate::ws_ticket::TicketStore {
        &self.tickets
    }

    /// Issuer origin, when OAuth is enabled.
    pub fn oauth_issuer(&self) -> Option<String> {
        self.oauth.as_ref().map(|o| o.issuer.clone())
    }

    /// The client id to present when starting a browser sign-in.
    pub fn primary_client_id(&self) -> String {
        self.oauth
            .as_ref()
            .and_then(|o| o.allowed_client_ids.first().cloned())
            .unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string())
    }

    /// Verify a token obtained through the browser flow, using the SAME rules
    /// as every other request — a sign-in path must not be a softer one.
    pub fn verify_for_browser(
        &self,
        token: &str,
    ) -> Result<ruview_auth::Principal, ruview_auth::VerifyError> {
        let oauth = self
            .oauth
            .as_ref()
            .ok_or(ruview_auth::VerifyError::MissingBearer)?;
        verify_access_token(
            token,
            &oauth.jwks,
            &VerifierConfig {
                issuer: oauth.issuer.clone(),
                required_scope: scope::SENSING_READ.to_string(),
                allowed_client_ids: oauth.allowed_client_ids.clone(),
            },
        )
    }

    /// Whether the legacy unauthenticated-WebSocket escape hatch is active.
    pub fn legacy_ws_enabled(&self) -> bool {
        self.legacy_ws
    }

    /// Whether the middleware will enforce auth on `/api/v1/*` requests.
    pub fn is_enabled(&self) -> bool {
        self.token.is_some() || self.oauth.is_some()
    }

    /// Whether Cognitum OAuth verification is active.
    pub fn oauth_enabled(&self) -> bool {
        self.oauth.is_some()
    }

    /// Whether the legacy static `RUVIEW_API_TOKEN` is configured.
    pub fn static_token_enabled(&self) -> bool {
        self.token.is_some()
    }
}

/// The scope a request must carry, split by **blast radius** (ADR-060): can
/// this call destroy something, or only observe?
///
/// **Reads are open; writes are closed unless explicitly allowlisted.**
///
/// An earlier revision enumerated the admin routes by prefix and let everything
/// else fall through to `sensing:read`. That is the wrong polarity for a
/// security gate and it shipped a real hole: `POST /api/v1/adaptive/train`
/// trains a classifier, overwrites the on-disk model and swaps the live one —
/// but it does not start with `/api/v1/train/`, so it landed on `sensing:read`,
/// the scope `wifi-densepose login` requests BY DEFAULT. A denylist for a scope
/// gate will keep missing routes as routes keep being added.
///
/// So: `GET`/`HEAD`/`OPTIONS` need `sensing:read`. Any other method needs
/// `sensing:admin` unless its exact path is in [`READ_SAFE_MUTATIONS`] — routes
/// that change runtime state but destroy nothing. A new mutating route added
/// without thought is therefore admin-gated by default, which is the safe way
/// to be wrong.
///
/// `sensing:read` is not "harmless": for a presence and vital-signs sensor,
/// read access tells the holder who is home. It is *non-destructive*, which is
/// a weaker claim.
pub fn required_scope_for(method: &Method, path: &str) -> &'static str {
    // Reads are open to `sensing:read`.
    if !is_mutating(method) {
        return scope::SENSING_READ;
    }
    // A small, explicit allowlist of mutating routes that change runtime state
    // but destroy nothing — a dashboard doing its ordinary job.
    if READ_SAFE_MUTATIONS.contains(&path)
        || (path.starts_with("/api/v1/rf/vendors/") && path.ends_with("/events"))
    {
        return scope::SENSING_READ;
    }
    // Everything else that mutates requires admin. FAIL CLOSED — see the docs
    // above for why this is a default rather than a list.
    scope::SENSING_ADMIN
}

fn is_mutating(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// Mutating routes that need only `sensing:read`.
///
/// Deliberately an ALLOWLIST. Everything absent from it that mutates requires
/// `sensing:admin`, so adding a route without thinking about scope fails safe
/// instead of silently landing on read.
const READ_SAFE_MUTATIONS: &[&str] = &[
    // Browsers must be able to obtain a WebSocket ticket with a read token,
    // or the read scope cannot open a stream at all.
    "/api/v1/ws-ticket",
    // Load/unload/activate: reversible, destroy nothing.
    "/api/v1/models/load",
    "/api/v1/models/unload",
    "/api/v1/models/lora/activate",
    "/api/v1/model/sona/activate",
    "/api/v1/adaptive/unload",
    // Capture and calibration: create data, never destroy it.
    "/api/v1/calibration/start",
    "/api/v1/calibration/stop",
    // Promotion consumes only server measured held out evidence and stores a
    // bounded privacy reduced prior. Cancel discards only an unfinished run.
    "/api/v1/calibration/bootstrap/promote",
    "/api/v1/calibration/cancel",
    "/api/v1/pose/calibrate",
    "/api/v1/recording/start",
    "/api/v1/recording/stop",
];

/// Constant-time byte slice equality. Returns `false` immediately on length
/// mismatch (lengths are not secret here — both sides are fixed tokens).
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Axum middleware: enforces `Authorization: Bearer <token>` on `/api/v1/*`
/// requests when [`AuthState::is_enabled`] returns `true`. Wires up via
/// [`axum::middleware::from_fn_with_state`].
pub async fn require_bearer(
    State(auth): State<AuthState>,
    mut request: Request,
    next: Next,
) -> Response {
    if !auth.is_enabled() {
        return next.run(request).await;
    }
    let path = request.uri().path().to_string();

    // WebSocket upgrades: bearer OR single-use ticket (ADR-272). Checked before
    // the prefix test because `/api/v1/stream/pose` is both a WS path and under
    // the protected prefix.
    if is_ws_path(&path) {
        if auth.legacy_ws {
            return next.run(request).await;
        }
        if let Some(ticket) = crate::ws_ticket::ticket_from_uri(request.uri()) {
            // Consumed here — one attempt per ticket, valid or not, so a
            // guessed value cannot be retried and a real one cannot be replayed.
            if let Some(grant) = auth.tickets.consume(&ticket) {
                let holds_read = grant
                    .scopes
                    .as_deref()
                    // `None` = issued by the legacy static token, which predates
                    // scopes and carries full authority.
                    .map_or(true, |s| s.split_whitespace().any(|x| x == scope::SENSING_READ));
                if holds_read {
                    tracing::debug!(path = %path, subject = ?grant.subject, "WebSocket authorized by ticket");
                    return next.run(request).await;
                }
                tracing::debug!(path = %path, "ticket lacked the scope this stream requires");
            }
            return unauthorized(&auth);
        }
        // No ticket: fall through to the bearer path below, which is how a
        // native (non-browser) client authenticates a WebSocket.
    } else if is_anonymous(&path) {
        return next.run(request).await;
    }

    let Some(supplied) = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        // RFC 6750 §2.1 / RFC 7235 §2.1: the auth-scheme ("Bearer") is
        // case-insensitive. Match it as such (and tolerate extra leading
        // whitespace before the token) so a correct token isn't rejected
        // just because a client sent `bearer`/`BEARER`. The token compare
        // below stays exact + constant-time.
        .and_then(|s| {
            let (scheme, token) = s.split_once(' ')?;
            scheme
                .eq_ignore_ascii_case("Bearer")
                .then(|| token.trim_start())
        })
    else {
        // No bearer header at all — a browser session may still authorize this.
        return session_or_unauthorized(&auth, request, next).await;
    };

    // 1. Legacy static token. Unchanged, and tried first so an existing
    //    deployment behaves identically even with OAuth switched on.
    if let Some(expected) = auth.token.as_ref() {
        if ct_eq(supplied.as_bytes(), expected.as_bytes()) {
            return next.run(request).await;
        }
    }

    // 2. Cognitum OAuth (ADR-271).
    if let Some(oauth) = auth.oauth.as_ref() {
        let required = if is_ws_path(&path) {
            // A stream is a read, regardless of the HTTP verb on the upgrade.
            scope::SENSING_READ
        } else {
            required_scope_for(request.method(), &path)
        };
        // Verification can hit the network: a `kid` miss or an expired cache
        // makes `JwksCache` perform a BLOCKING `ureq` fetch (3s connect + 3s
        // read). Doing that inline parks the tokio worker running this request,
        // and on Pi-class hardware with few workers a handful of concurrent
        // misses stalls the whole server — including `/health`.
        //
        // `main.rs` already does exactly this for the token exchange, noting it
        // as "the same mistake this codebase had to fix in jwks.rs". The hot
        // verification path had not been given the same treatment.
        let token = supplied.to_string();
        let oauth = Arc::clone(oauth);
        let required_owned = required.to_string();
        let verified = tokio::task::spawn_blocking(move || {
            let config = VerifierConfig {
                issuer: oauth.issuer.clone(),
                required_scope: required_owned,
                allowed_client_ids: oauth.allowed_client_ids.clone(),
            };
            verify_access_token(&token, &oauth.jwks, &config)
        })
        .await;

        let verified = match verified {
            Ok(v) => v,
            Err(join) => {
                // The blocking task panicked or was cancelled. Fail closed:
                // "we could not verify" is never "the request is authorized".
                tracing::error!(error = %join, path = %path.as_str(), "JWKS verification task failed");
                return unauthorized(&auth);
            }
        };

        match verified {
            Ok(principal) => {
                tracing::debug!(
                    sub = %principal.subject,
                    account_id = %principal.account_id,
                    client_id = %principal.client_id,
                    jti = %principal.token_id,
                    scope = %required,
                    path = %path.as_str(),
                    "OAuth request authorized"
                );
                // Downstream handlers can attribute the request without
                // re-parsing the token.
                request.extensions_mut().insert(principal);
                return next.run(request).await;
            }
            Err(e) => {
                // Logged, never returned: the reason a token failed is useful
                // to an operator and useful to an attacker probing for which
                // claim to forge next. The response stays a flat 401.
                tracing::debug!(error = %e, path = %path.as_str(), required_scope = %required, "OAuth verification failed");
                return unauthorized(&auth);
            }
        }
    }

    // 3. Browser session cookie (ADR-271 browser half). Checked last: it is the
    //    weakest-bound credential (host-only, no proof-of-possession), so a
    //    presented bearer or ticket should win.
    if auth.oauth.is_some() {
        if let Some(cookie_header) = request
            .headers()
            .get(axum::http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
        {
            if let Some(session) = crate::browser_session::from_cookie_header(cookie_header) {
                let required = if is_ws_path(&path) {
                    scope::SENSING_READ
                } else {
                    required_scope_for(request.method(), &path)
                };
                // Step-up: a privileged action needs a RECENT authentication,
                // not merely a live session. The session outlives the access
                // token that created it and Cognitum offers no introspection,
                // so a stale session is authority we cannot revoke — bounded
                // here to the routes where that authority actually does damage.
                //
                // Ordered AFTER the scope check on purpose. Asking someone who
                // does not hold `sensing:admin` to re-authenticate sends them
                // through a redirect that cannot possibly help: they come back
                // with the same scopes and are refused again. Only a caller who
                // actually holds the capability is asked to prove it is fresh.
                if session.has_scope(required)
                    && required == scope::SENSING_ADMIN
                    && !session.recently_authenticated()
                {
                    tracing::debug!(
                        sub = %session.subject,
                        path = %path.as_str(),
                        "browser session is too old for a privileged action; re-authentication required"
                    );
                    return reauthentication_required(&auth);
                }
                if session.has_scope(required) {
                    tracing::debug!(
                        sub = %session.subject,
                        scope = %required,
                        path = %path.as_str(),
                        "request authorized by browser session"
                    );
                    return next.run(request).await;
                }
                tracing::debug!(
                    path = %path.as_str(),
                    required_scope = %required,
                    "browser session lacks the scope this route requires"
                );
            }
        }
    }

    unauthorized(&auth)
}

/// The no-bearer path: try a browser session cookie, else 401.
async fn session_or_unauthorized(auth: &AuthState, request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    if auth.oauth.is_some() {
        if let Some(h) = request
            .headers()
            .get(axum::http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
        {
            if let Some(session) = crate::browser_session::from_cookie_header(h) {
                let required = if is_ws_path(&path) {
                    scope::SENSING_READ
                } else {
                    required_scope_for(request.method(), &path)
                };
                // Step-up: a privileged action needs a RECENT authentication,
                // not merely a live session. The session outlives the access
                // token that created it and Cognitum offers no introspection,
                // so a stale session is authority we cannot revoke — bounded
                // here to the routes where that authority actually does damage.
                //
                // Ordered AFTER the scope check on purpose. Asking someone who
                // does not hold `sensing:admin` to re-authenticate sends them
                // through a redirect that cannot possibly help: they come back
                // with the same scopes and are refused again. Only a caller who
                // actually holds the capability is asked to prove it is fresh.
                if session.has_scope(required)
                    && required == scope::SENSING_ADMIN
                    && !session.recently_authenticated()
                {
                    tracing::debug!(
                        sub = %session.subject,
                        path = %path.as_str(),
                        "browser session is too old for a privileged action; re-authentication required"
                    );
                    return reauthentication_required(&auth);
                }
                if session.has_scope(required) {
                    tracing::debug!(sub = %session.subject, path = %path.as_str(), "browser session authorized");
                    return next.run(request).await;
                }
            }
        }
    }
    unauthorized(auth)
}

/// A uniform 401. The hint names whichever credentials are actually accepted,
/// so an operator is not told to set a variable this server ignores — but it
/// never says *why* a presented token failed.
fn unauthorized(auth: &AuthState) -> Response {
    let body = match (auth.token.is_some(), auth.oauth.is_some()) {
        (true, true) => concat!(
            "missing or invalid bearer token\n",
            "accepted: Authorization: Bearer <RUVIEW_API_TOKEN>, ",
            "or a Cognitum OAuth access token with the scope this route requires\n"
        ),
        (false, true) => concat!(
            "missing or invalid bearer token\n",
            "accepted: a Cognitum OAuth access token with the scope this route requires\n"
        ),
        _ => "missing or invalid bearer token (set Authorization: Bearer <RUVIEW_API_TOKEN>)\n",
    };
    (StatusCode::UNAUTHORIZED, body).into_response()
}

/// 401 for a browser session that is valid but too old for a privileged action.
///
/// Distinguished from a plain 401 by an RFC 6750 §3 `WWW-Authenticate` error
/// code, because the client's correct response is different: not "sign in",
/// which it already has, but "prove it again". Without a distinguishable signal
/// the UI would surface a stale-session delete as a generic failure, and the
/// user would have no idea that re-signing-in fixes it.
///
/// This leaks nothing: the caller already knows it was refused, and the code
/// says only that the *session age* was the reason.
fn reauthentication_required(auth: &AuthState) -> Response {
    let mut resp = unauthorized(auth);
    resp.headers_mut().insert(
        axum::http::header::WWW_AUTHENTICATE,
        axum::http::HeaderValue::from_static(
            r#"Bearer error="invalid_token", error_description="reauthentication required for a privileged action""#,
        ),
    );
    resp
}

/// Convenience re-export so handlers can name the type they pull out of
/// request extensions without depending on `ruview-auth` directly.
pub use ruview_auth::Principal as AuthenticatedPrincipal;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    /// ONE test, not three, because `allowed_client_ids` reads process-global
    /// env and cargo runs tests on parallel threads — three tests mutating
    /// `RUVIEW_OAUTH_CLIENT_IDS` race and fail intermittently.
    #[test]
    fn client_id_allowlist_parsing_never_silently_opts_out() {
        let read = |v: &str| {
            std::env::set_var(OAUTH_CLIENT_IDS_ENV, v);
            let ids = allowed_client_ids();
            std::env::remove_var(OAUTH_CLIENT_IDS_ENV);
            ids
        };

        // An empty allowlist is the OPT-OUT sentinel in `verify.rs` — it skips
        // the audience check entirely. So a value that is non-empty but parses
        // to nothing (a stray trailing comma, `","`) would silently turn the
        // boundary off and admit a token minted for any other Cognitum product.
        // Same fail-open shape as the scope denylist this PR already inverted.
        for bad in [",", " , ", ",,,", "  "] {
            let ids = read(bad);
            assert!(
                !ids.is_empty(),
                "{bad:?} produced an empty allowlist, which disables the audience check"
            );
            assert_eq!(ids, vec![DEFAULT_CLIENT_ID.to_string()]);
        }

        // The deliberate escape hatch must keep working, or the fix above would
        // be a behaviour change wearing a security label.
        assert!(read("*").is_empty(), "`*` must remain the way to accept any client");

        // And a real list must still parse, trailing comma and all.
        assert_eq!(
            read(" ruview , musica ,"),
            vec!["ruview".to_string(), "musica".to_string()]
        );
    }

    fn ok_handler() -> Router {
        Router::new()
            .route("/health", get(|| async { "ok" }))
            .route("/api/v1/info", get(|| async { "ok" }))
            .route("/api/v1/sensitive", axum::routing::post(|| async { "ok" }))
            .route("/api/v1/stream/pose", get(|| async { "ok" }))
            .route("/ui/index.html", get(|| async { "<html/>" }))
    }

    fn wrap(auth: AuthState) -> Router {
        ok_handler().layer(axum::middleware::from_fn_with_state(auth, require_bearer))
    }

    async fn status(router: Router, method: &str, path: &str, auth: Option<&str>) -> StatusCode {
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .body(Body::empty())
            .unwrap();
        if let Some(t) = auth {
            req.headers_mut()
                .insert(AUTHORIZATION, format!("Bearer {t}").parse().unwrap());
        }
        router.oneshot(req).await.unwrap().status()
    }

    #[tokio::test]
    async fn middleware_is_no_op_when_token_unset() {
        let r = wrap(AuthState::default());
        assert_eq!(
            status(r.clone(), "GET", "/api/v1/info", None).await,
            StatusCode::OK
        );
        assert_eq!(
            status(r.clone(), "POST", "/api/v1/sensitive", None).await,
            StatusCode::OK
        );
        assert_eq!(
            status(r.clone(), "GET", "/health", None).await,
            StatusCode::OK
        );
        assert_eq!(
            status(r, "GET", "/ui/index.html", None).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn enabled_blocks_api_without_bearer() {
        let r = wrap(AuthState::from_token("s3cr3t"));
        assert_eq!(
            status(r.clone(), "GET", "/api/v1/info", None).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status(r, "POST", "/api/v1/sensitive", None).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn accepts_case_insensitive_bearer_scheme() {
        // RFC 6750 §2.1 / RFC 7235 §2.1: the auth-scheme is case-insensitive.
        // A correct token must authenticate regardless of scheme casing or
        // extra whitespace; a wrong token must still be rejected.
        async fn req_status(auth_value: &str) -> StatusCode {
            let r = wrap(AuthState::from_token("s3cr3t"));
            let mut req = Request::builder()
                .method("GET")
                .uri("/api/v1/info")
                .body(Body::empty())
                .unwrap();
            req.headers_mut()
                .insert(AUTHORIZATION, auth_value.parse().unwrap());
            r.oneshot(req).await.unwrap().status()
        }
        assert_eq!(req_status("Bearer s3cr3t").await, StatusCode::OK);
        assert_eq!(req_status("bearer s3cr3t").await, StatusCode::OK);
        assert_eq!(req_status("BEARER s3cr3t").await, StatusCode::OK);
        assert_eq!(req_status("Bearer  s3cr3t").await, StatusCode::OK); // extra space
        // Scheme leniency must NOT weaken the token check.
        assert_eq!(req_status("bearer nope").await, StatusCode::UNAUTHORIZED);
        assert_eq!(req_status("Basic s3cr3t").await, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn enabled_blocks_api_with_wrong_bearer() {
        let r = wrap(AuthState::from_token("s3cr3t"));
        assert_eq!(
            status(r.clone(), "GET", "/api/v1/info", Some("nope")).await,
            StatusCode::UNAUTHORIZED
        );
        // Wrong scheme (Basic / token) — only "Bearer <token>" is accepted.
        let mut req = Request::builder()
            .method("GET")
            .uri("/api/v1/info")
            .body(Body::empty())
            .unwrap();
        req.headers_mut()
            .insert(AUTHORIZATION, "Basic s3cr3t".parse().unwrap());
        assert_eq!(
            r.oneshot(req).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn enabled_allows_api_with_correct_bearer() {
        let r = wrap(AuthState::from_token("s3cr3t"));
        assert_eq!(
            status(r.clone(), "GET", "/api/v1/info", Some("s3cr3t")).await,
            StatusCode::OK
        );
        assert_eq!(
            status(r, "POST", "/api/v1/sensitive", Some("s3cr3t")).await,
            StatusCode::OK
        );
    }

    /// REGRESSION (ADR-080 #3, CWE-598 — token in URL query string).
    ///
    /// ADR-080 flagged "JWT in URL" as a HIGH finding (tokens in query strings
    /// leak into logs, proxies, browser history, `Referer`). The current
    /// sensing-server only ever reads the token from the `Authorization: Bearer`
    /// header — there is no `?token=` / `?access_token=` query path in
    /// `require_bearer` (see [`require_bearer`] above, which only inspects the
    /// `AUTHORIZATION` header). This test pins that: a request carrying the
    /// correct token *only* in the query string is still `401`, while the same
    /// token in the header is `200`. If anyone ever re-introduces a query-string
    /// token path, this fails.
    #[tokio::test]
    async fn query_string_token_is_never_accepted() {
        let r = wrap(AuthState::from_token("s3cr3t"));
        // Correct token, but supplied only in the URL — must NOT authenticate.
        assert_eq!(
            status(r.clone(), "GET", "/api/v1/info?token=s3cr3t", None).await,
            StatusCode::UNAUTHORIZED,
            "?token= in the query string must not authenticate (CWE-598)"
        );
        assert_eq!(
            status(
                r.clone(),
                "GET",
                "/api/v1/info?access_token=s3cr3t",
                None
            )
            .await,
            StatusCode::UNAUTHORIZED,
            "?access_token= in the query string must not authenticate (CWE-598)"
        );
        // A query token must not "help" a request that also lacks the header,
        // even combined with an unrelated param.
        assert_eq!(
            status(
                r.clone(),
                "GET",
                "/api/v1/info?foo=bar&token=s3cr3t",
                None
            )
            .await,
            StatusCode::UNAUTHORIZED
        );
        // The header path is the only accepted channel — same token, header,
        // succeeds. (Proves we didn't just break auth entirely.)
        assert_eq!(
            status(r, "GET", "/api/v1/info?token=s3cr3t", Some("s3cr3t")).await,
            StatusCode::OK,
            "the Authorization: Bearer header is the supported channel"
        );
    }

    /// REGRESSION (ADR-080 #1 — X-Forwarded-For spoofing).
    ///
    /// The bearer middleware authenticates on the token alone and must be
    /// completely insensitive to a client-supplied `X-Forwarded-For` header:
    /// an attacker cannot flip an auth decision by spoofing XFF. A wrong token
    /// stays `401` and a right token stays `200` regardless of XFF. (The
    /// sensing-server has no IP-based rate-limit / allowlist that XFF could
    /// bypass; this locks in that auth itself never consults XFF.)
    #[tokio::test]
    async fn xff_header_never_affects_auth_decision() {
        let r = wrap(AuthState::from_token("s3cr3t"));
        async fn with_xff(router: Router, token: Option<&str>, xff: &str) -> StatusCode {
            let mut req = Request::builder()
                .method("GET")
                .uri("/api/v1/info")
                .header("X-Forwarded-For", xff)
                .body(Body::empty())
                .unwrap();
            if let Some(t) = token {
                req.headers_mut()
                    .insert(AUTHORIZATION, format!("Bearer {t}").parse().unwrap());
            }
            router.oneshot(req).await.unwrap().status()
        }
        // Spoofed XFF + no/ wrong token ⇒ still rejected.
        assert_eq!(
            with_xff(r.clone(), None, "127.0.0.1").await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            with_xff(r.clone(), Some("nope"), "10.0.0.1, 127.0.0.1").await,
            StatusCode::UNAUTHORIZED
        );
        // Spoofed XFF + correct token ⇒ still accepted (XFF is irrelevant).
        assert_eq!(
            with_xff(r, Some("s3cr3t"), "evil-proxy").await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn enabled_never_gates_paths_outside_api_v1() {
        let r = wrap(AuthState::from_token("s3cr3t"));
        // Even with auth ON, `/health` and `/ui/*` are reachable without a token:
        // orchestrator probes and the local UI need to load unchallenged.
        assert_eq!(
            status(r.clone(), "GET", "/health", None).await,
            StatusCode::OK
        );
        assert_eq!(
            status(r, "GET", "/ui/index.html", None).await,
            StatusCode::OK
        );
    }

    /// SUPERSEDED by ADR-272. This was `enabled_exempts_pose_stream_websocket`,
    /// which asserted `/api/v1/stream/pose` stayed reachable with no bearer
    /// because a browser cannot set `Authorization` on an upgrade (PR #1313).
    ///
    /// That reasoning about browsers is still true — the conclusion was not.
    /// Measured on a server with auth ON: a credential-less handshake to
    /// `/api/v1/stream/pose`, `/ws/sensing` and `/ws/introspection` all
    /// returned `101`, so the REST control plane was locked while the live
    /// sensing stream was open. The browser limitation is now answered by a
    /// single-use ticket rather than by an exemption.
    ///
    /// The half of the original test that still matters is kept: whatever the
    /// WebSocket rule is, it must not leak to other `/api/v1/*` paths.
    #[tokio::test]
    async fn the_pose_stream_websocket_is_no_longer_exempt() {
        let r = wrap(AuthState::from_token("s3cr3t"));
        assert_eq!(
            status(r.clone(), "GET", "/api/v1/stream/pose", None).await,
            StatusCode::UNAUTHORIZED,
            "the pose stream must no longer accept a credential-less upgrade"
        );
        // Preserved from the original: the WebSocket rule stays narrow.
        assert_eq!(
            status(r, "GET", "/api/v1/info", None).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn ct_eq_basics() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"ab")); // length mismatch
        assert!(!ct_eq(b"", b"x"));
        assert!(ct_eq(b"", b""));
    }

    #[test]
    fn from_env_treats_empty_as_disabled() {
        // Avoid touching the real env in a thread-shared test — exercise the
        // string ctor directly with the same trim logic.
        assert!(!AuthState::from_token("").is_enabled());
        assert!(AuthState::from_token("x").is_enabled());
    }

    #[test]
    fn protected_prefix_and_env_constants_are_stable() {
        // These are documented in the issue body and the README; keep them locked.
        assert_eq!(API_TOKEN_ENV, "RUVIEW_API_TOKEN");
        assert_eq!(PROTECTED_PREFIX, "/api/v1/");
    }
}

/// ADR-271 — the OAuth path and the scope gate, exercised end to end through a
/// real Router: request → middleware → `ruview-auth` verifier → handler.
///
/// Tokens are real ES256 JWTs signed with a key generated at test runtime; no
/// key material is committed. The verifier's own accept/reject matrix lives in
/// `ruview-auth`; what is tested here is the wiring — layering with the legacy
/// static token, which scope each route demands, and that a rejected token
/// never reaches a handler.
#[cfg(test)]
mod oauth_tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::{delete, get, post},
        Router,
    };
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use jsonwebtoken::{encode, EncodingKey, Header};
    use ring::{
        rand::SystemRandom,
        signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING},
    };
    use ruview_auth::jwks::{JwksError, JwksFetcher};
    use std::sync::OnceLock;
    use tower::ServiceExt;

    const KID: &str = "test-kid";
    const ISSUER: &str = "https://auth.test.local";

    struct TestKey {
        der: Vec<u8>,
        x: String,
        y: String,
    }

    fn key() -> &'static TestKey {
        static K: OnceLock<TestKey> = OnceLock::new();
        K.get_or_init(|| {
            let rng = SystemRandom::new();
            let der = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
            let sk = EcdsaKeyPair::from_pkcs8(
                &ECDSA_P256_SHA256_FIXED_SIGNING,
                der.as_ref(),
                &rng,
            )
            .unwrap();
            let point = sk.public_key().as_ref();
            assert_eq!(point.len(), 65);
            assert_eq!(point[0], 4);
            TestKey {
                der: der.as_ref().to_vec(),
                x: URL_SAFE_NO_PAD.encode(&point[1..33]),
                y: URL_SAFE_NO_PAD.encode(&point[33..65]),
            }
        })
    }

    struct StaticJwks(String);
    impl JwksFetcher for StaticJwks {
        fn fetch(&self, _url: &str) -> Result<String, JwksError> {
            Ok(self.0.clone())
        }
    }

    fn oauth_state() -> Arc<OAuthState> {
        let k = key();
        let doc = format!(
            r#"{{"keys":[{{"kty":"EC","crv":"P-256","alg":"ES256","use":"sig","kid":"{KID}","x":"{}","y":"{}"}}]}}"#,
            k.x, k.y
        );
        Arc::new(OAuthState {
            jwks: JwksCache::new("https://stub/jwks.json", Box::new(StaticJwks(doc))),
            issuer: ISSUER.to_string(),
            allowed_client_ids: vec!["ruview".to_string()],
        })
    }

    fn token_with_scope(scope_claim: &str) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let claims = serde_json::json!({
            "typ": "access",
            "sub": "user-1",
            "account_id": "acct-1",
            "org_id": "org-1",
            "workspace_id": "ws-1",
            "client_id": "ruview",
            "scope": scope_claim,
            "jti": "jti-1",
            "iat": now - 10,
            "exp": now + 900,
            "setup": false,
            "workload": false,
            // NO `iss`. Real Cognitum tokens carry none — mirroring production
            // here matters even though the verifier ignores the claim either
            // way: a fixture that invents a claim reality lacks is exactly what
            // hid the original `iss` bug for a day. See ruview-auth's
            // `a_token_with_no_iss_claim_is_accepted_because_cognitum_issues_none`.
        });
        let mut header = Header::new(jsonwebtoken::Algorithm::ES256);
        header.kid = Some(KID.to_string());
        encode(
            &header,
            &claims,
            &EncodingKey::from_ec_der(&key().der),
        )
        .unwrap()
    }

    /// Mirrors the real route shapes the scope gate keys off.
    fn app(auth: AuthState) -> Router {
        Router::new()
            .route("/api/v1/info", get(|| async { "ok" }))
            .route("/api/v1/models", get(|| async { "ok" }))
            .route("/api/v1/models/m1", delete(|| async { "deleted" }))
            .route("/api/v1/recording/r1", delete(|| async { "deleted" }))
            .route("/api/v1/train/start", post(|| async { "training" }))
            .layer(axum::middleware::from_fn_with_state(auth, require_bearer))
    }

    async fn call(auth: AuthState, method: &str, path: &str, bearer: Option<&str>) -> StatusCode {
        let mut req = Request::builder().method(method).uri(path);
        if let Some(b) = bearer {
            req = req.header(AUTHORIZATION, format!("Bearer {b}"));
        }
        app(auth)
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    /// Same as [`call`] but presents a browser session cookie instead of a
    /// bearer. The cookie is minted through `browser_session`'s real signing
    /// path, so this exercises genuine verification.
    async fn call_with_session(auth: AuthState, method: &str, path: &str, cookie: &str) -> StatusCode {
        let req = Request::builder()
            .method(method)
            .uri(path)
            .header(axum::http::header::COOKIE, cookie);
        app(auth)
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    fn session_cookie(scope_claim: &str, ttl: i64) -> String {
        format!(
            "ruview_session={}",
            crate::browser_session::test_cookie_value("sub-b", "acct-b", scope_claim, ttl)
        )
    }

    // ── browser session cookie as a credential ────────────────────────
    //
    // The session cookie authorizes /api/v1/* and WebSocket upgrades, and had
    // no test presenting one at any level — the newest credential in the system
    // was the one with no executable evidence behind it.

    #[tokio::test]
    async fn a_read_scoped_browser_session_can_read() {
        let c = session_cookie(scope::SENSING_READ, 3600);
        assert_eq!(
            call_with_session(oauth_only(), "GET", "/api/v1/models", &c).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn a_read_scoped_browser_session_cannot_delete_or_train() {
        // MUTANT THIS KILLS: replacing `session.has_scope(required)` with
        // `true` in the cookie branch. Without this, anyone signed in through
        // the browser — the least-bound credential in the system, host-only
        // with no proof-of-possession — could delete models and recordings and
        // start training, regardless of what they consented to.
        let c = session_cookie(scope::SENSING_READ, 3600);
        for (method, path) in [
            ("DELETE", "/api/v1/models/m1"),
            ("DELETE", "/api/v1/recording/r1"),
            ("POST", "/api/v1/train/start"),
        ] {
            assert_eq!(
                call_with_session(oauth_only(), method, path, &c).await,
                StatusCode::UNAUTHORIZED,
                "{method} {path} accepted a read-only browser session"
            );
        }
    }

    #[tokio::test]
    async fn an_admin_scoped_browser_session_can_delete() {
        // The negative above must not pass merely because cookies never work.
        let c = session_cookie(
            &format!("{} {}", scope::SENSING_READ, scope::SENSING_ADMIN),
            3600,
        );
        assert_eq!(
            call_with_session(oauth_only(), "DELETE", "/api/v1/models/m1", &c).await,
            StatusCode::OK
        );
    }

    // ── step-up: privileged actions need a RECENT authentication ──────

    fn aged_admin_cookie(age: i64) -> String {
        format!(
            "ruview_session={}",
            crate::browser_session::test_cookie_value_aged(
                "sub-b",
                "acct-b",
                &format!("{} {}", scope::SENSING_READ, scope::SENSING_ADMIN),
                3600,
                age,
            )
        )
    }

    #[tokio::test]
    async fn a_stale_but_live_session_can_still_read() {
        // Step-up must not degrade into "re-authenticate every 5 minutes". The
        // dashboard's primary use is watching a live stream; reads ride the
        // full session lifetime.
        let c = aged_admin_cookie(crate::browser_session::ADMIN_REVERIFY_SECS + 60);
        assert_eq!(
            call_with_session(oauth_only(), "GET", "/api/v1/models", &c).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn a_stale_session_cannot_delete_even_holding_the_admin_scope() {
        // The session outlives the ~15-minute access token that created it, and
        // Cognitum publishes no introspection endpoint — so a stale session is
        // authority nobody can revoke. Bounded to the routes where it does
        // damage: holding `sensing:admin` is necessary but no longer sufficient.
        let c = aged_admin_cookie(crate::browser_session::ADMIN_REVERIFY_SECS + 60);
        for (method, path) in [
            ("DELETE", "/api/v1/models/m1"),
            ("DELETE", "/api/v1/recording/r1"),
            ("POST", "/api/v1/train/start"),
        ] {
            assert_eq!(
                call_with_session(oauth_only(), method, path, &c).await,
                StatusCode::UNAUTHORIZED,
                "{method} {path} accepted a stale session for a privileged action"
            );
        }
    }

    #[tokio::test]
    async fn a_read_only_user_is_not_sent_to_reauthenticate_pointlessly() {
        // The scope check must come FIRST. A caller without `sensing:admin`
        // cannot be helped by re-authenticating — they return with the same
        // scopes and are refused again — so sending the step-up challenge would
        // cost them a redirect and tell them something untrue about why they
        // were refused. Only a caller who actually HOLDS the capability is
        // asked to prove it is fresh.
        let stale_read_only = format!(
            "ruview_session={}",
            crate::browser_session::test_cookie_value_aged(
                "sub-b",
                "acct-b",
                scope::SENSING_READ,
                3600,
                crate::browser_session::ADMIN_REVERIFY_SECS + 60,
            )
        );
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/v1/models/m1")
            .header(axum::http::header::COOKIE, &stale_read_only);
        let resp = app(oauth_only())
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let challenge = resp
            .headers()
            .get(axum::http::header::WWW_AUTHENTICATE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(
            !challenge.contains("reauthentication required"),
            "a read-only caller must not be told to re-authenticate: {challenge:?}"
        );
    }

    #[tokio::test]
    async fn an_admin_holder_with_a_stale_session_does_get_the_challenge() {
        // The counterpart: the signal must actually fire for the caller it can
        // help, or the UI never learns to send them back through /oauth/start.
        let c = aged_admin_cookie(crate::browser_session::ADMIN_REVERIFY_SECS + 60);
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/v1/models/m1")
            .header(axum::http::header::COOKIE, &c);
        let resp = app(oauth_only())
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let challenge = resp
            .headers()
            .get(axum::http::header::WWW_AUTHENTICATE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(
            challenge.contains("reauthentication required"),
            "an admin holder with a stale session must be told to re-authenticate: {challenge:?}"
        );
    }

    #[tokio::test]
    async fn a_freshly_authenticated_session_can_delete() {
        // The negative above must not pass merely because admin never works.
        let c = aged_admin_cookie(0);
        assert_eq!(
            call_with_session(oauth_only(), "DELETE", "/api/v1/models/m1", &c).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn a_session_cookie_predating_auth_time_cannot_perform_admin_actions() {
        // Cookies issued before `auth_time` existed deserialize with 0, which is
        // infinitely stale. They must degrade to read-only rather than being
        // treated as freshly authenticated — fail closed, and self-healing the
        // next time the user signs in.
        let legacy = serde_json::json!({
            "subject": "sub-old",
            "account_id": "acct-old",
            "scope": "sensing:read sensing:admin",
            "exp": chrono::Utc::now().timestamp() + 3600,
        });
        let raw = crate::browser_session::test_sign_for_tests(&serde_json::to_vec(&legacy).unwrap());
        let c = format!("ruview_session={raw}");

        assert_eq!(
            call_with_session(oauth_only(), "GET", "/api/v1/models", &c).await,
            StatusCode::OK,
            "an old cookie must keep working for reads"
        );
        assert_eq!(
            call_with_session(oauth_only(), "DELETE", "/api/v1/models/m1", &c).await,
            StatusCode::UNAUTHORIZED,
            "an old cookie must not carry privileged authority"
        );
    }

    #[tokio::test]
    async fn an_expired_browser_session_is_refused() {
        let c = session_cookie(scope::SENSING_READ, -1);
        assert_eq!(
            call_with_session(oauth_only(), "GET", "/api/v1/models", &c).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn a_bad_bearer_beats_a_good_cookie_rather_than_falling_back() {
        // Pins REAL precedence, which is not what the ordering comment in
        // `require_bearer` implies. When an Authorization header is present the
        // OAuth step returns on BOTH arms, so the cookie branch that follows it
        // is unreachable — a browser holding a valid session that also sends a
        // stale bearer gets 401 rather than falling back to its cookie.
        //
        // Verified empirically: mutating that branch's `has_scope` to `true`
        // changes no test outcome, while mutating the one in
        // `session_or_unauthorized` fails `a_read_scoped_browser_session_
        // cannot_delete_or_train`.
        //
        // This fails CLOSED, so it is not a hole — but it was undocumented and
        // untested, and "try the next credential" is what the code reads like.
        // Pinned here so changing it is a decision rather than an accident.
        let c = session_cookie(scope::SENSING_READ, 3600);
        let req = Request::builder()
            .method("GET")
            .uri("/api/v1/models")
            .header(AUTHORIZATION, "Bearer not-a-valid-token")
            .header(axum::http::header::COOKIE, &c);
        let status = app(oauth_only())
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status();
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "a presented bearer is authoritative; the cookie is not consulted after it fails"
        );
    }

    #[tokio::test]
    async fn a_forged_browser_session_is_refused() {
        let c = "ruview_session=Zm9yZ2Vk.bm90LWEtdmFsaWQtbWFj";
        assert_eq!(
            call_with_session(oauth_only(), "GET", "/api/v1/models", c).await,
            StatusCode::UNAUTHORIZED
        );
    }

    fn oauth_only() -> AuthState {
        AuthState {
            token: None,
            oauth: Some(oauth_state()),
            tickets: crate::ws_ticket::TicketStore::new(),
            legacy_ws: false,
        }
    }

    // ── scope policy (pure) ───────────────────────────────────────────

    #[test]
    fn training_requires_the_admin_scope() {
        assert_eq!(
            required_scope_for(&Method::POST, "/api/v1/train/start"),
            scope::SENSING_ADMIN
        );
    }

    #[test]
    fn deleting_a_model_or_recording_requires_the_admin_scope() {
        assert_eq!(
            required_scope_for(&Method::DELETE, "/api/v1/models/m1"),
            scope::SENSING_ADMIN
        );
        assert_eq!(
            required_scope_for(&Method::DELETE, "/api/v1/recording/r1"),
            scope::SENSING_ADMIN
        );
    }

    #[test]
    fn reading_models_is_not_admin_merely_because_the_path_matches() {
        // The gate is (method, path), not path alone — GET on the same prefix
        // must stay a read.
        assert_eq!(
            required_scope_for(&Method::GET, "/api/v1/models/m1"),
            scope::SENSING_READ
        );
    }

    #[test]
    fn non_destructive_mutations_stay_read_scoped() {
        // Loading a model changes server state but destroys nothing. Putting it
        // behind the destructive scope would push routine dashboard use into
        // asking for delete capability — the opposite of least privilege.
        assert_eq!(
            required_scope_for(&Method::POST, "/api/v1/models/load"),
            scope::SENSING_READ
        );
        assert_eq!(
            required_scope_for(&Method::POST, "/api/v1/recording/start"),
            scope::SENSING_READ
        );
    }

    // ── wiring ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn a_read_scoped_token_reaches_a_read_route() {
        let t = token_with_scope(scope::SENSING_READ);
        assert_eq!(
            call(oauth_only(), "GET", "/api/v1/info", Some(&t)).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn a_read_scoped_token_cannot_delete_a_model() {
        // The whole point of the split: a dashboard session streaming poses
        // must not be able to destroy the model it streams through.
        let t = token_with_scope(scope::SENSING_READ);
        assert_eq!(
            call(oauth_only(), "DELETE", "/api/v1/models/m1", Some(&t)).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn a_read_scoped_token_cannot_start_training() {
        let t = token_with_scope(scope::SENSING_READ);
        assert_eq!(
            call(oauth_only(), "POST", "/api/v1/train/start", Some(&t)).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn an_admin_scoped_token_may_delete_and_train() {
        let t = token_with_scope("sensing:read sensing:admin");
        assert_eq!(
            call(oauth_only(), "DELETE", "/api/v1/models/m1", Some(&t)).await,
            StatusCode::OK
        );
        assert_eq!(
            call(oauth_only(), "POST", "/api/v1/train/start", Some(&t)).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn an_inference_token_from_another_cognitum_product_is_refused_everywhere() {
        // The cross-product case. Correctly signed, unexpired, right issuer —
        // only the scope stops it. Asserted here at the middleware layer too,
        // because this is where a wiring mistake would actually let it through.
        let t = token_with_scope("inference");
        for (m, p) in [
            ("GET", "/api/v1/info"),
            ("DELETE", "/api/v1/models/m1"),
            ("POST", "/api/v1/train/start"),
        ] {
            assert_eq!(
                call(oauth_only(), m, p, Some(&t)).await,
                StatusCode::UNAUTHORIZED,
                "{m} {p} must reject an inference-only token"
            );
        }
    }

    #[tokio::test]
    async fn a_garbage_bearer_is_refused_when_only_oauth_is_configured() {
        assert_eq!(
            call(oauth_only(), "GET", "/api/v1/info", Some("not-a-jwt")).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn no_credential_is_refused_when_only_oauth_is_configured() {
        assert_eq!(
            call(oauth_only(), "GET", "/api/v1/info", None).await,
            StatusCode::UNAUTHORIZED
        );
    }

    // ── layering with the legacy static token ─────────────────────────

    fn both() -> AuthState {
        AuthState {
            token: Some(Arc::new("legacy-secret".to_string())),
            oauth: Some(oauth_state()),
            tickets: crate::ws_ticket::TicketStore::new(),
            legacy_ws: false,
        }
    }

    #[tokio::test]
    async fn the_legacy_static_token_still_works_with_oauth_enabled() {
        // Backward compatibility: turning OAuth on must not break a deployment
        // that has been using RUVIEW_API_TOKEN.
        assert_eq!(
            call(both(), "GET", "/api/v1/info", Some("legacy-secret")).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn the_legacy_static_token_is_not_scope_gated() {
        // It predates scopes and carries no claims, so it keeps the full
        // access it has always had. Narrowing it here would be a silent
        // breaking change to existing deployments; migrating to OAuth is how
        // an operator opts into the finer split.
        assert_eq!(
            call(both(), "POST", "/api/v1/train/start", Some("legacy-secret")).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn an_oauth_token_works_alongside_a_configured_static_token() {
        let t = token_with_scope(scope::SENSING_READ);
        assert_eq!(
            call(both(), "GET", "/api/v1/info", Some(&t)).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn a_wrong_static_token_falls_through_to_oauth_and_is_refused() {
        assert_eq!(
            call(both(), "GET", "/api/v1/info", Some("wrong-secret")).await,
            StatusCode::UNAUTHORIZED
        );
    }

    // ── attribution ───────────────────────────────────────────────────

    #[tokio::test]
    async fn the_verified_principal_is_available_to_handlers() {
        // The reason for moving off a shared secret: requests become
        // attributable. If the principal is not in extensions, no handler and
        // no audit log can name who called.
        async fn echo(req: Request<Body>) -> String {
            match req.extensions().get::<ruview_auth::Principal>() {
                Some(p) => format!("{}|{}|{}", p.subject, p.account_id, p.client_id),
                None => "none".to_string(),
            }
        }
        let router = Router::new()
            .route("/api/v1/whoami", get(echo))
            .layer(axum::middleware::from_fn_with_state(
                oauth_only(),
                require_bearer,
            ));
        let t = token_with_scope(scope::SENSING_READ);
        let resp = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/whoami")
                    .header(AUTHORIZATION, format!("Bearer {t}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&body), "user-1|acct-1|ruview");
    }

    // ── the unset case must stay untouched ────────────────────────────

    #[tokio::test]
    async fn with_neither_credential_configured_the_middleware_is_still_a_no_op() {
        assert_eq!(
            call(AuthState::default(), "POST", "/api/v1/train/start", None).await,
            StatusCode::OK
        );
    }
}

/// ADR-272 — WebSocket gating. These pin the hole that was measured open:
/// with auth ON, a credential-less upgrade to `/ws/sensing` returned 101.
#[cfg(test)]
mod ws_gate_tests {
    use super::*;
    use crate::ws_ticket::TicketGrant;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    fn app(auth: AuthState) -> Router {
        Router::new()
            .route("/ws/sensing", get(|| async { "stream" }))
            .route("/ws/introspection", get(|| async { "introspect" }))
            .route("/api/v1/stream/pose", get(|| async { "pose" }))
            .route("/api/v1/models", get(|| async { "models" }))
            .layer(axum::middleware::from_fn_with_state(auth, require_bearer))
    }

    async fn get_status(auth: AuthState, uri: &str, bearer: Option<&str>) -> StatusCode {
        let mut req = Request::builder().method("GET").uri(uri);
        if let Some(b) = bearer {
            req = req.header(AUTHORIZATION, format!("Bearer {b}"));
        }
        app(auth)
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    fn static_auth() -> AuthState {
        AuthState {
            token: Some(Arc::new("secret".into())),
            oauth: None,
            tickets: crate::ws_ticket::TicketStore::new(),
            legacy_ws: false,
        }
    }

    #[tokio::test]
    async fn every_websocket_path_refuses_an_unauthenticated_upgrade() {
        // The measured regression: all three answered 101 before this change.
        for p in WS_PATHS {
            assert_eq!(
                get_status(static_auth(), p, None).await,
                StatusCode::UNAUTHORIZED,
                "{p} must not accept a credential-less upgrade"
            );
        }
    }

    #[tokio::test]
    async fn a_native_client_may_authenticate_a_websocket_with_a_bearer() {
        // Python / CLI / MCP are not browser-constrained and must not be forced
        // through the ticket round-trip.
        for p in WS_PATHS {
            assert_eq!(
                get_status(static_auth(), p, Some("secret")).await,
                StatusCode::OK,
                "{p} must accept a bearer on the upgrade"
            );
        }
    }

    #[tokio::test]
    async fn a_valid_ticket_authorizes_exactly_one_upgrade() {
        let auth = static_auth();
        let ticket = auth
            .tickets()
            .issue(TicketGrant { scopes: None, subject: None })
            .unwrap();
        let uri = format!("/ws/sensing?ticket={ticket}");

        assert_eq!(get_status(auth.clone(), &uri, None).await, StatusCode::OK);
        assert_eq!(
            get_status(auth, &uri, None).await,
            StatusCode::UNAUTHORIZED,
            "a replayed ticket must fail — this is what makes a URL credential tolerable"
        );
    }

    #[tokio::test]
    async fn an_unknown_ticket_is_refused() {
        assert_eq!(
            get_status(static_auth(), "/ws/sensing?ticket=deadbeef", None).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn a_ticket_without_the_streams_scope_is_refused() {
        // A ticket inherits its issuer's authority and cannot exceed it.
        let auth = static_auth();
        let ticket = auth
            .tickets()
            .issue(TicketGrant {
                scopes: Some("inference".into()),
                subject: Some("u".into()),
            })
            .unwrap();
        assert_eq!(
            get_status(auth, &format!("/ws/sensing?ticket={ticket}"), None).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn a_ticket_is_not_a_credential_for_the_rest_api() {
        // Containment: a ticket buys one WebSocket, never REST access.
        let auth = static_auth();
        let ticket = auth
            .tickets()
            .issue(TicketGrant { scopes: None, subject: None })
            .unwrap();
        assert_eq!(
            get_status(auth, &format!("/api/v1/models?ticket={ticket}"), None).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn the_legacy_escape_hatch_restores_unauthenticated_websockets() {
        let mut auth = static_auth();
        auth.legacy_ws = true;
        assert_eq!(get_status(auth, "/ws/sensing", None).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn the_legacy_escape_hatch_does_not_weaken_the_rest_api() {
        // The blast radius of the hatch must be exactly the WebSocket paths.
        let mut auth = static_auth();
        auth.legacy_ws = true;
        assert_eq!(
            get_status(auth, "/api/v1/models", None).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn with_auth_off_websockets_stay_open_as_before() {
        // Unconfigured deployments must see no behaviour change at all.
        assert_eq!(
            get_status(AuthState::default(), "/ws/sensing", None).await,
            StatusCode::OK
        );
    }
}

#[cfg(test)]
mod ws_path_matching_tests {
    use super::*;

    #[test]
    fn every_currently_known_websocket_path_matches() {
        for p in WS_PATHS {
            assert!(is_ws_path(p), "{p} must be recognised as a WebSocket path");
        }
    }

    #[test]
    fn a_websocket_route_that_does_not_exist_yet_is_already_gated() {
        // `/ws/train/progress` arrives with ADR-186 (PR #1387) and is already
        // referenced by the UI. Under an exact-match allowlist it would ship
        // unauthenticated. Prefix matching means it is gated on arrival.
        assert!(is_ws_path("/ws/train/progress"));
        assert!(is_ws_path("/ws/anything-added-in-future"));
    }

    #[test]
    fn ordinary_rest_paths_are_not_treated_as_websockets() {
        for p in [
            "/api/v1/models",
            "/api/v1/stream/status", // a plain GET, not an upgrade
            "/health",
            "/ui/index.html",
            "/",
        ] {
            assert!(!is_ws_path(p), "{p} must not be treated as a WebSocket path");
        }
    }

    #[test]
    fn a_path_merely_starting_with_ws_is_not_the_ws_prefix() {
        // `/wsx/...` must not match `/ws/`.
        assert!(!is_ws_path("/wsx/sensing"));
        assert!(!is_ws_path("/ws"));
    }
}

/// The scope classifier, after inverting it to fail-closed. The charge that
/// forced this: `POST /api/v1/adaptive/train` trains and overwrites the live
/// model, but did not match the `/api/v1/train/` prefix, so it was reachable
/// with `sensing:read` — the scope `login` requests by default.
#[cfg(test)]
mod scope_gate_polarity_tests {
    use super::*;

    #[test]
    fn adaptive_train_requires_admin() {
        // The reported bypass. Handler calls train_from_recordings(), writes
        // the model to disk and swaps the live one.
        assert_eq!(
            required_scope_for(&Method::POST, "/api/v1/adaptive/train"),
            scope::SENSING_ADMIN
        );
    }

    #[test]
    fn every_known_destructive_route_requires_admin() {
        for (m, p) in [
            (Method::POST, "/api/v1/train/start"),
            (Method::POST, "/api/v1/train/stop"),
            (Method::POST, "/api/v1/adaptive/train"),
            (Method::DELETE, "/api/v1/models/m1"),
            (Method::DELETE, "/api/v1/recording/r1"),
            (Method::POST, "/api/v1/config/ground-truth"),
            (Method::POST, "/api/v1/calibration/reset"),
        ] {
            assert_eq!(
                required_scope_for(&m, p),
                scope::SENSING_ADMIN,
                "{m} {p} must require admin"
            );
        }
    }

    #[test]
    fn an_unknown_mutating_route_defaults_to_admin() {
        // THE property the old denylist lacked. A route added tomorrow is
        // admin-gated until someone consciously classifies it as read-safe.
        for p in [
            "/api/v1/some/route/invented/later",
            "/api/v1/adaptive/retrain-everything",
            "/api/v1/models/nuke",
        ] {
            assert_eq!(
                required_scope_for(&Method::POST, p),
                scope::SENSING_ADMIN,
                "unknown mutating route {p} must fail closed to admin"
            );
            assert_eq!(
                required_scope_for(&Method::DELETE, p),
                scope::SENSING_ADMIN
            );
        }
    }

    #[test]
    fn reads_stay_open_to_the_read_scope() {
        for p in [
            "/api/v1/models",
            "/api/v1/models/m1",
            "/api/v1/recording/list",
            "/api/v1/adaptive/status",
            "/api/v1/anything/at/all",
        ] {
            assert_eq!(
                required_scope_for(&Method::GET, p),
                scope::SENSING_READ,
                "GET {p} must stay open to read"
            );
        }
    }

    #[test]
    fn allowlisted_mutations_stay_read_scoped() {
        // Non-destructive state changes a dashboard makes routinely. Pushing
        // these to admin would force ordinary use to hold delete capability.
        for p in READ_SAFE_MUTATIONS {
            assert_eq!(
                required_scope_for(&Method::POST, p),
                scope::SENSING_READ,
                "{p} is allowlisted and must stay read-scoped"
            );
        }
    }

    #[test]
    fn a_read_token_can_still_mint_a_websocket_ticket() {
        // Load-bearing: if this needed admin, the read scope could never open
        // a stream from a browser at all.
        assert_eq!(
            required_scope_for(&Method::POST, "/api/v1/ws-ticket"),
            scope::SENSING_READ
        );
    }

    #[test]
    fn vendor_event_ingest_is_read_scoped_by_prefix() {
        // Path carries a `:vendor` segment, so it cannot be an exact match.
        assert_eq!(
            required_scope_for(&Method::POST, "/api/v1/rf/vendors/netgear/events"),
            scope::SENSING_READ
        );
        // ...but the prefix must not become a wildcard for anything under it.
        assert_eq!(
            required_scope_for(&Method::POST, "/api/v1/rf/vendors/netgear/delete-all"),
            scope::SENSING_ADMIN
        );
    }
}
