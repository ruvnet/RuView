//! HTTP/WebSocket bind check: refuse a routable bind with API auth off.
//!
//! Same rule the Docker entrypoint applies (#864) and the UDP receiver applies
//! to its own bind (ADR-296, [`crate::udp_bind::decide_udp_bind`]): loopback is
//! always fine, a routable bind needs either API auth (`RUVIEW_API_TOKEN` or
//! `RUVIEW_OAUTH_ISSUER`) or the explicit `RUVIEW_ALLOW_UNAUTHENTICATED` opt-in.

use std::net::IpAddr;

/// Explicit opt-in to serve the API unauthenticated on a routable address.
/// Same variable the Docker entrypoint reads.
pub const ALLOW_UNAUTHENTICATED_ENV: &str = "RUVIEW_ALLOW_UNAUTHENTICATED";

/// Outcome of [`decide_http_bind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpBindDecision {
    /// Bind is loopback-only (`127.0.0.0/8` / `::1`); not reachable off-host.
    Loopback,
    /// Bind is routable and API auth is on.
    RoutableAuthenticated,
    /// Bind is routable with API auth off, accepted via
    /// `RUVIEW_ALLOW_UNAUTHENTICATED`.
    RoutableUnauthenticated,
}

/// Whether `RUVIEW_ALLOW_UNAUTHENTICATED` holds an opt-in value. Accepts `1`,
/// `true`, `yes` and `on` in any case; anything else, including unset, is off.
pub fn allow_unauthenticated_opt_in(value: Option<&str>) -> bool {
    value.is_some_and(|v| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

/// Decide whether the HTTP/WebSocket listeners may bind to `bind`.
///
/// Pure and socket-free so it can be tested without binding.
pub fn decide_http_bind(
    bind: IpAddr,
    auth_enabled: bool,
    allow_unauthenticated: bool,
) -> Result<HttpBindDecision, String> {
    if bind.is_loopback() {
        return Ok(HttpBindDecision::Loopback);
    }
    if auth_enabled {
        return Ok(HttpBindDecision::RoutableAuthenticated);
    }
    if allow_unauthenticated {
        return Ok(HttpBindDecision::RoutableUnauthenticated);
    }
    Err(format!(
        "Refusing to serve the API on routable address {bind} with API auth off: \
         /api/v1/* and /ws/sensing would be readable by anyone who can reach this \
         host. Set RUVIEW_API_TOKEN=<token> (or RUVIEW_OAUTH_ISSUER) to enable \
         auth, use --bind-addr 127.0.0.1, or set {ALLOW_UNAUTHENTICATED_ENV}=1 on \
         a trusted network only."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn v4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    #[test]
    fn loopback_starts_without_auth() {
        for ip in [
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            v4(127, 0, 1, 1),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
        ] {
            assert_eq!(
                decide_http_bind(ip, false, false),
                Ok(HttpBindDecision::Loopback)
            );
        }
    }

    #[test]
    fn routable_without_auth_is_refused() {
        for ip in [
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            v4(192, 168, 1, 10),
        ] {
            let err = decide_http_bind(ip, false, false).unwrap_err();
            assert!(err.contains("RUVIEW_API_TOKEN"), "{err}");
            assert!(err.contains(ALLOW_UNAUTHENTICATED_ENV), "{err}");
        }
    }

    #[test]
    fn routable_with_auth_starts() {
        assert_eq!(
            decide_http_bind(v4(0, 0, 0, 0), true, false),
            Ok(HttpBindDecision::RoutableAuthenticated)
        );
    }

    #[test]
    fn routable_with_opt_in_starts_unauthenticated() {
        assert_eq!(
            decide_http_bind(v4(0, 0, 0, 0), false, true),
            Ok(HttpBindDecision::RoutableUnauthenticated)
        );
    }

    #[test]
    fn opt_in_accepts_entrypoint_spellings_only() {
        for v in ["1", "true", "TRUE", "yes", "On", " 1 "] {
            assert!(allow_unauthenticated_opt_in(Some(v)), "{v:?}");
        }
        for v in ["", "0", "false", "no", "off", "2", "y"] {
            assert!(!allow_unauthenticated_opt_in(Some(v)), "{v:?}");
        }
        assert!(!allow_unauthenticated_opt_in(None));
    }
}
