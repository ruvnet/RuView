//! Host-header allowlist for the sensing-server HTTP + WS surface.
//!
//! Defense against DNS rebinding: when the server is bound to loopback
//! (default `127.0.0.1`), a foreign page (e.g. `evil.com`) can lower its DNS
//! TTL and re-resolve to `127.0.0.1` after the browser has already accepted
//! the origin. From the browser's point of view the request is same-origin
//! against `evil.com`, so it reads the response — even though the bytes come
//! from the local sensing-server. Without `Host`-header validation the server
//! happily serves the request because every other axum layer treats it as a
//! normal connection.
//!
//! For RuView this means any website the user visits can stream live pose,
//! breathing rate, and heart-rate data out of the sensing-server (`/ws/sensing`,
//! `/api/v1/pose/current`, `/api/v1/vital-signs`, …), and trigger state-mutating
//! POSTs (`/api/v1/recording/start`, `/api/v1/models/load`, …) when bearer-auth
//! is not configured (the default LAN-only deployment posture from #443).
//!
//! The middleware here rejects any request whose `Host` header is not in the
//! configured allowlist with `421 Misdirected Request`. Defaults cover the
//! common local-only deployment (`localhost`, `127.0.0.1`, `[::1]` with or
//! without `:PORT`). Operators who bind to a routable address (`--bind-addr
//! 0.0.0.0` or a LAN IP) extend the allowlist with `--allowed-host` flags or
//! the `SENSING_ALLOWED_HOSTS` env var.

use std::collections::HashSet;
use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header::HOST, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Environment variable that supplies additional allowed hosts
/// (comma-separated). Whitespace around each entry is trimmed; empty entries
/// are ignored.
pub const ALLOWED_HOSTS_ENV: &str = "SENSING_ALLOWED_HOSTS";

/// Built-in allowlist entries. Each entry is also accepted with an optional
/// trailing `:PORT` (any port).
const DEFAULT_LOOPBACK_HOSTS: &[&str] = &["localhost", "127.0.0.1", "[::1]"];

/// Cheap, cloneable handle to the configured Host allowlist.
#[derive(Debug, Clone, Default)]
pub struct HostAllowlist {
    /// Lower-cased exact-match hostnames (with or without `:PORT` already
    /// baked in). Empty set ⇒ middleware accepts everything and is a no-op,
    /// matching the historical behaviour for callers that want to opt out.
    entries: Arc<HashSet<String>>,
}

impl HostAllowlist {
    /// Build an allowlist with only the default loopback names (bare and
    /// with any `:PORT`). Use this when the server is bound to loopback and
    /// no operator overrides have been supplied.
    pub fn loopback_only() -> Self {
        let mut entries: HashSet<String> = HashSet::new();
        for h in DEFAULT_LOOPBACK_HOSTS {
            entries.insert((*h).to_string());
        }
        HostAllowlist {
            entries: Arc::new(entries),
        }
    }

    /// Build an allowlist from an iterator of additional hostnames (each may
    /// optionally include a `:PORT` suffix). The default loopback set is
    /// always included so `--bind-addr 0.0.0.0` deployments do not lock out
    /// local browsers on `http://localhost:8080/…`.
    pub fn with_extra<I, S>(extras: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut entries: HashSet<String> = HashSet::new();
        for h in DEFAULT_LOOPBACK_HOSTS {
            entries.insert((*h).to_string());
        }
        for h in extras {
            let h = h.as_ref().trim();
            if !h.is_empty() {
                entries.insert(h.to_lowercase());
            }
        }
        HostAllowlist {
            entries: Arc::new(entries),
        }
    }

    /// Build an allowlist by joining (a) the default loopback set, (b) any
    /// CLI-supplied extras, and (c) the comma-separated `SENSING_ALLOWED_HOSTS`
    /// env var. Order of precedence does not matter — the result is a set.
    pub fn from_cli_and_env<I, S>(cli_extras: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let env_extras: Vec<String> = std::env::var(ALLOWED_HOSTS_ENV)
            .ok()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let cli_vec: Vec<String> = cli_extras
            .into_iter()
            .map(|s| s.as_ref().to_string())
            .collect();
        HostAllowlist::with_extra(cli_vec.into_iter().chain(env_extras.into_iter()))
    }

    /// Disable host-header validation entirely. Provided as an explicit escape
    /// hatch for operators who deploy the server behind a reverse proxy that
    /// already canonicalises `Host`, or for unit tests that need to bypass
    /// the layer.
    pub fn disabled() -> Self {
        HostAllowlist::default()
    }

    /// True if the middleware will enforce host validation. `false` ⇒ no-op.
    pub fn is_enabled(&self) -> bool {
        !self.entries.is_empty()
    }

    /// Test-only accessor returning a sorted, lower-cased copy of the
    /// configured allowlist. Exposed via the `pub(crate)` boundary so we can
    /// unit-test the env-var parsing without reaching into the `Arc`.
    pub fn entries_for_test(&self) -> Vec<String> {
        let mut v: Vec<String> = self.entries.iter().cloned().collect();
        v.sort();
        v
    }

    /// Check whether `host` (the raw `Host` header value, e.g.
    /// `127.0.0.1:8080` or `[::1]`) is permitted. Comparison is case-insensitive
    /// on the host part; ports are matched verbatim if the allowlist entry
    /// pins one, otherwise the port is ignored.
    pub fn is_allowed(&self, host: &str) -> bool {
        if self.entries.is_empty() {
            return true;
        }
        let host = host.trim().to_lowercase();
        if host.is_empty() {
            return false;
        }

        // Exact match (e.g. allowlist contains `127.0.0.1:8080` and request
        // sent `Host: 127.0.0.1:8080`).
        if self.entries.contains(&host) {
            return true;
        }

        // Match on host-only when the allowlist entry has no port and the
        // request includes a port. Handles `Host: 127.0.0.1:8080` against
        // `127.0.0.1` in the allowlist, and `Host: [::1]:8080` against
        // `[::1]`.
        let host_only = strip_port(&host);
        if self.entries.contains(host_only) {
            return true;
        }

        false
    }
}

/// Strip a `:PORT` suffix from `host`, leaving the host portion. IPv6 literals
/// are wrapped in brackets (`[::1]:PORT`) so the last `:` is the port
/// separator; bracketed IPv6 without a port stays intact.
fn strip_port(host: &str) -> &str {
    if let Some(close) = host.strip_prefix('[').and_then(|_| host.find(']')) {
        // Bracketed IPv6: `[::1]` or `[::1]:8080`.
        if let Some(after) = host.get(close + 1..) {
            if after.starts_with(':') {
                return &host[..=close];
            }
        }
        return host;
    }
    match host.rfind(':') {
        Some(idx) => &host[..idx],
        None => host,
    }
}

/// Axum middleware: rejects any request whose `Host` header is not in the
/// configured allowlist. Use with [`axum::middleware::from_fn_with_state`].
///
/// Behaviour:
/// * No `Host` header → `400 Bad Request` (HTTP/1.1 requires one; HTTP/2
///   synthesises it from `:authority`, so a missing value is a real protocol
///   violation, not a rebinding signal).
/// * `Host` header present but not in the allowlist → `421 Misdirected Request`.
/// * Empty allowlist → no-op (the operator explicitly opted out).
pub async fn require_allowed_host(
    State(allowlist): State<HostAllowlist>,
    request: Request,
    next: Next,
) -> Response {
    if !allowlist.is_enabled() {
        return next.run(request).await;
    }
    let host_header = request
        .headers()
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let host_header = match host_header {
        Some(h) => h,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                "missing Host header\n",
            )
                .into_response();
        }
    };
    if allowlist.is_allowed(&host_header) {
        next.run(request).await
    } else {
        (
            StatusCode::MISDIRECTED_REQUEST,
            "Host header not in allowlist (DNS-rebinding defense). \
             Set --allowed-host <name[:port]> or SENSING_ALLOWED_HOSTS=<comma-list> \
             to permit this hostname.\n",
        )
            .into_response()
    }
}

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

    fn router(allowlist: HostAllowlist) -> Router {
        Router::new()
            .route("/health", get(|| async { "ok" }))
            .route("/api/v1/pose/current", get(|| async { "ok" }))
            .route("/ws/sensing", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                allowlist,
                require_allowed_host,
            ))
    }

    async fn status(router: Router, path: &str, host: Option<&str>) -> StatusCode {
        let mut req = Request::builder().method("GET").uri(path);
        if let Some(h) = host {
            req = req.header(HOST, h);
        }
        let req = req.body(Body::empty()).unwrap();
        router.oneshot(req).await.unwrap().status()
    }

    #[tokio::test]
    async fn loopback_only_allows_default_hosts_with_any_port() {
        let r = router(HostAllowlist::loopback_only());
        for h in [
            "localhost",
            "localhost:8080",
            "127.0.0.1",
            "127.0.0.1:8080",
            "127.0.0.1:65535",
            "[::1]",
            "[::1]:8080",
        ] {
            assert_eq!(
                status(r.clone(), "/api/v1/pose/current", Some(h)).await,
                StatusCode::OK,
                "host {h} should be allowed under loopback_only()"
            );
        }
    }

    #[tokio::test]
    async fn loopback_only_rejects_foreign_hosts() {
        let r = router(HostAllowlist::loopback_only());
        for h in [
            "evil.com",
            "evil.com:8080",
            "127.0.0.1.evil.com",
            "192.168.1.10",
            "192.168.1.10:8080",
            "sensing.local",
        ] {
            assert_eq!(
                status(r.clone(), "/api/v1/pose/current", Some(h)).await,
                StatusCode::MISDIRECTED_REQUEST,
                "host {h} should be rejected under loopback_only()"
            );
        }
    }

    #[tokio::test]
    async fn rejects_missing_host_header() {
        let r = router(HostAllowlist::loopback_only());
        assert_eq!(
            status(r, "/api/v1/pose/current", None).await,
            StatusCode::BAD_REQUEST,
        );
    }

    #[tokio::test]
    async fn rejects_empty_host_header() {
        let r = router(HostAllowlist::loopback_only());
        assert_eq!(
            status(r, "/api/v1/pose/current", Some("")).await,
            StatusCode::MISDIRECTED_REQUEST,
        );
    }

    #[tokio::test]
    async fn rejection_applies_to_health_and_ws_routes_too() {
        // The whole router is fronted by the middleware — there is no
        // bypass for `/health` or `/ws/*`, because rebinding doesn't care
        // which route it targets, it cares about what bytes flow back.
        let r = router(HostAllowlist::loopback_only());
        assert_eq!(
            status(r.clone(), "/health", Some("evil.com")).await,
            StatusCode::MISDIRECTED_REQUEST,
        );
        assert_eq!(
            status(r, "/ws/sensing", Some("evil.com")).await,
            StatusCode::MISDIRECTED_REQUEST,
        );
    }

    #[tokio::test]
    async fn extras_extend_loopback_set() {
        let r = router(HostAllowlist::with_extra(["sensing.local", "192.168.1.10"]));
        assert_eq!(
            status(r.clone(), "/api/v1/pose/current", Some("sensing.local")).await,
            StatusCode::OK,
        );
        assert_eq!(
            status(r.clone(), "/api/v1/pose/current", Some("sensing.local:8080")).await,
            StatusCode::OK,
        );
        assert_eq!(
            status(r.clone(), "/api/v1/pose/current", Some("192.168.1.10:8080")).await,
            StatusCode::OK,
        );
        // Loopback defaults are still in:
        assert_eq!(
            status(r.clone(), "/api/v1/pose/current", Some("127.0.0.1")).await,
            StatusCode::OK,
        );
        // Foreign hosts still rejected:
        assert_eq!(
            status(r, "/api/v1/pose/current", Some("evil.com")).await,
            StatusCode::MISDIRECTED_REQUEST,
        );
    }

    #[tokio::test]
    async fn disabled_allowlist_is_no_op() {
        let r = router(HostAllowlist::disabled());
        assert_eq!(
            status(r.clone(), "/api/v1/pose/current", Some("evil.com")).await,
            StatusCode::OK,
        );
        assert_eq!(
            status(r, "/api/v1/pose/current", None).await,
            StatusCode::OK,
        );
    }

    #[tokio::test]
    async fn case_insensitive_host_match() {
        let r = router(HostAllowlist::loopback_only());
        for h in ["LOCALHOST", "LocalHost:8080", "127.0.0.1"] {
            assert_eq!(
                status(r.clone(), "/api/v1/pose/current", Some(h)).await,
                StatusCode::OK,
                "host {h} should be allowed (case-insensitive)"
            );
        }
        let r2 = router(HostAllowlist::with_extra(["Sensing.Local"]));
        assert_eq!(
            status(r2, "/api/v1/pose/current", Some("sensing.local:8080")).await,
            StatusCode::OK,
        );
    }

    #[test]
    fn strip_port_handles_ipv4_ipv6_and_bare_hostnames() {
        assert_eq!(strip_port("localhost"), "localhost");
        assert_eq!(strip_port("localhost:8080"), "localhost");
        assert_eq!(strip_port("127.0.0.1"), "127.0.0.1");
        assert_eq!(strip_port("127.0.0.1:8080"), "127.0.0.1");
        assert_eq!(strip_port("[::1]"), "[::1]");
        assert_eq!(strip_port("[::1]:8080"), "[::1]");
        // No `:` at all
        assert_eq!(strip_port("sensing.local"), "sensing.local");
    }

    #[test]
    fn with_extra_trims_whitespace_and_skips_empty() {
        let allowlist = HostAllowlist::with_extra(["  sensing.local  ", "", "192.168.1.10"]);
        let entries = allowlist.entries_for_test();
        assert!(entries.contains(&"sensing.local".to_string()));
        assert!(entries.contains(&"192.168.1.10".to_string()));
        assert!(!entries.iter().any(|s| s.is_empty()));
    }

    #[test]
    fn loopback_only_includes_all_three_defaults() {
        let entries = HostAllowlist::loopback_only().entries_for_test();
        assert!(entries.contains(&"localhost".to_string()));
        assert!(entries.contains(&"127.0.0.1".to_string()));
        assert!(entries.contains(&"[::1]".to_string()));
    }

    #[test]
    fn empty_input_to_with_extra_still_includes_loopback_defaults() {
        // Calling `with_extra` with no extras (e.g. operator passed no
        // `--allowed-host` flags) must keep the loopback defaults so a fresh
        // 127.0.0.1 deployment isn't bricked.
        let entries: Vec<String> = Vec::new();
        let allowlist = HostAllowlist::with_extra(entries);
        assert!(allowlist.is_allowed("127.0.0.1"));
        assert!(allowlist.is_allowed("127.0.0.1:8080"));
        assert!(allowlist.is_allowed("localhost"));
        assert!(!allowlist.is_allowed("evil.com"));
    }

    #[test]
    fn env_constants_are_stable() {
        assert_eq!(ALLOWED_HOSTS_ENV, "SENSING_ALLOWED_HOSTS");
    }
}
