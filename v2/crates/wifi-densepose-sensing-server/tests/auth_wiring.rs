//! Boots the REAL `sensing-server` binary and probes BOTH listeners.
//!
//! # Why this test exists
//!
//! Two authentication bypasses shipped in the ADR-272 work, and 526 green unit
//! tests could not see either, because every auth test in this crate builds its
//! OWN `Router` with a hand-picked subset of routes. A synthetic router can
//! never observe how the real one is assembled — and both defects were assembly:
//!
//! 1. The dedicated WebSocket listener (`--ws-port`) was constructed with only
//!    host validation. `require_bearer` was never applied to it at all. That is
//!    the port the shipped UI actually connects to
//!    (`ui/services/sensing.service.js` maps HTTP 8080 -> WS 8765), so the
//!    earlier fix protected a path the browser never takes.
//! 2. `/ws/field` was `.merge()`d AFTER the auth layer on the HTTP router. In
//!    axum a layer wraps only what is already registered, so merging afterwards
//!    silently exempts those routes.
//!
//! Both were found by adversarial review, not by the suite. This test closes
//! that gap: it runs the actual binary, so it sees the actual wiring.
//!
//! It deliberately asserts on **ports and transports**, not on handler logic —
//! handler behaviour is covered by the unit suites. What is unique here is that
//! nothing is synthetic: real process, real listeners, real TCP.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const TOKEN: &str = "integration-test-secret";

/// Reserve a port by binding and immediately releasing it.
///
/// Mildly racy, which is why each test reserves its own set and the server is
/// given several seconds to come up: a collision surfaces as a boot failure,
/// not as a false pass.
fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

struct Server {
    child: Child,
    http: u16,
    ws: u16,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Server {
    /// Spawn the real binary. `env` lets a test choose the auth configuration.
    ///
    /// PANICS rather than returning `None` on failure. This used to return an
    /// `Option` that every test turned into `return`, which meant all five
    /// assertions were skipped precisely when the server was broken — including
    /// broken BY an auth change. A boot failure printed one line that `cargo
    /// test` swallows without `--nocapture` and reported `5 passed`. The only
    /// test in the suite that observes real wiring disarmed itself exactly when
    /// it mattered; a boot-time panic in the auth path would have shipped green.
    fn start(env: &[(&str, &str)]) -> Self {
        // `free_port()` releases the port before the child binds it, so under
        // parallel `cargo test` execution another test's server can grab the
        // same ephemeral port first (observed as `Os { code: 10048/98, kind:
        // AddrInUse }` on the child's actual bind, distinct from a genuine
        // auth-wiring break). Retry a bounded number of times on THAT specific
        // signature only — any other boot failure (including a real wiring
        // regression) still panics on the first attempt, preserving the
        // fail-loud property described above.
        const MAX_ATTEMPTS: u32 = 3;
        for attempt in 1..=MAX_ATTEMPTS {
            let (http, ws, udp) = (free_port(), free_port(), free_port());
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_sensing-server"));
            cmd.args([
                "--http-port", &http.to_string(),
                "--ws-port", &ws.to_string(),
                "--udp-port", &udp.to_string(),
                "--bind-addr", "127.0.0.1",
                "--no-edge-registry",
                "--source", "simulate",
            ])
            // Inherit nothing auth-related from the developer's shell, or a local
            // RUVIEW_* export would silently change what this test proves.
            .env_remove("RUVIEW_API_TOKEN")
            .env_remove("RUVIEW_OAUTH_ISSUER")
            .env_remove("RUVIEW_WS_LEGACY_UNAUTHENTICATED")
            .stdout(Stdio::null())
            // Captured, not discarded: if the server dies at boot, its stderr is the
            // only thing that says why, and the panic below reproduces it.
            .stderr(Stdio::piped());
            for (k, v) in env {
                cmd.env(k, v);
            }
            let mut child = cmd.spawn().expect("spawn sensing-server");
            if await_ready(http, ws) {
                return Server { child, http, ws };
            }
            let mut err = String::new();
            if let Some(mut s) = child.stderr.take() {
                let _ = s.read_to_string(&mut err);
            }
            let _ = child.kill();
            let _ = child.wait();
            let looks_like_port_collision =
                err.contains("AddrInUse") || err.contains("Address already in use")
                    || err.contains("code: 10048") || err.contains("code: 98");
            if looks_like_port_collision && attempt < MAX_ATTEMPTS {
                continue;
            }
            panic!(
                "sensing-server did not become ready on :{http} (http) and :{ws} (ws) \
                 within 30s (attempt {attempt}/{MAX_ATTEMPTS}). This is a FAILURE, not a skip — \
                 the wiring assertions below cannot run, and a boot-time break in the auth path \
                 is exactly what they exist to catch.\n\
                 --- server stderr ---\n{err}"
            );
        }
        unreachable!("loop always returns or panics");
    }
}

fn await_ready(http: u16, ws: u16) -> bool {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", http)).is_ok()
            && TcpStream::connect(("127.0.0.1", ws)).is_ok()
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

/// One raw HTTP/1.1 request; returns the status code.
fn status(port: u16, method: &str, path: &str, headers: &[(&str, &str)]) -> u16 {
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let mut s = TcpStream::connect(addr).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("Connection: close\r\n\r\n");
    s.write_all(req.as_bytes()).expect("write");

    let mut line = String::new();
    BufReader::new(&mut s).read_line(&mut line).expect("status line");
    line.split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or_else(|| panic!("unparseable status line: {line:?}"))
}

/// A genuine WebSocket upgrade. 101 means the connection was ACCEPTED.
fn ws_upgrade(port: u16, path: &str, bearer: Option<&str>) -> u16 {
    let mut headers: Vec<(&str, &str)> = vec![
        ("Upgrade", "websocket"),
        ("Connection", "Upgrade"),
        ("Sec-WebSocket-Version", "13"),
        ("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ=="),
    ];
    let auth;
    if let Some(b) = bearer {
        auth = format!("Bearer {b}");
        headers.push(("Authorization", &auth));
    }
    // Not `Connection: close` — that would contradict the upgrade.
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let mut s = TcpStream::connect(addr).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n");
    for (k, v) in &headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).expect("write");

    let mut buf = [0u8; 256];
    let n = s.read(&mut buf).expect("read");
    let head = String::from_utf8_lossy(&buf[..n]);
    let line = head.lines().next().unwrap_or_default();
    line.split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or_else(|| panic!("unparseable status line: {line:?}"))
}

/// Every WebSocket path, on every listener. This list is the point of the test.
const WS_PATHS: &[&str] = &["/ws/sensing", "/ws/introspection", "/api/v1/stream/pose", "/ws/field"];

/// Non-WebSocket routes that carry sensing data and must be gated.
///
/// `/api/field` is here because it was NOT gated: it serves the same signed
/// `FieldEvent` stream as `/ws/field`, but sits outside `/api/v1/`, and the gate
/// protected `/api/v1/*` by prefix. `/ws/field` was gated in this PR and its
/// REST twin, one path segment over, returned 200 to an anonymous caller on
/// both listeners. That is why the gate is now deny-by-default.
const PROTECTED_REST_PATHS: &[&str] = &["/api/field", "/api/v1/models", "/api/v1/vital-signs"];

#[test]
fn actual_vital_diagnostics_require_the_existing_bearer_boundary() {
    let server = Server::start(&[("RUVIEW_API_TOKEN", TOKEN)]);
    assert_eq!(status(server.http, "GET", "/api/v1/vital-signs", &[]), 401);
    let mut connection = TcpStream::connect(("127.0.0.1", server.http)).unwrap();
    connection.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(connection, "GET /api/v1/vital-signs HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n", server.http, TOKEN).unwrap();
    let mut response = String::new();
    connection.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
    let (_, body) = response.split_once("\r\n\r\n").unwrap();
    let body: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["diagnostics"]["schema"], "ruview.vital-publication-diagnostics.v1");
    assert_eq!(body["authority"], "abstained");
    assert!(body["vital_signs"]["heart_rate_bpm"].is_null());
}

#[test]
fn with_auth_on_no_listener_serves_sensing_data_anonymously() {
    let server = Server::start(&[("RUVIEW_API_TOKEN", TOKEN)]);

    for (label, port) in [("http", server.http), ("ws", server.ws)] {
        for path in PROTECTED_REST_PATHS {
            assert_eq!(
                status(port, "GET", path, &[]),
                401,
                "{label} port served {path} to an anonymous caller"
            );
            // And the credential must actually work, or the assertion above
            // could pass because the route simply does not exist.
            let ok = status(port, "GET", path, &[("Authorization", &format!("Bearer {TOKEN}"))]);
            assert_ne!(ok, 401, "{label} port {path} rejected a VALID bearer");
        }
    }
}

#[test]
fn the_dashboard_shell_and_sign_in_stay_reachable_when_auth_is_on() {
    // Deny-by-default must not lock the user out of the page that renders the
    // sign-in button, or of sign-in itself. This is the other half of the
    // allowlist: too tight is as broken as too loose, just louder.
    let server = Server::start(&[("RUVIEW_API_TOKEN", TOKEN)]);
    for path in ["/health", "/oauth/status"] {
        assert_ne!(
            status(server.http, "GET", path, &[]),
            401,
            "{path} must stay anonymous — sign-in depends on it"
        );
    }
}

#[test]
fn with_auth_on_no_listener_accepts_an_unauthenticated_websocket() {
    let server = Server::start(&[("RUVIEW_API_TOKEN", TOKEN)]);

    // Control first: if REST is not gated, the server is misconfigured and the
    // WebSocket assertions below would pass for the wrong reason.
    assert_eq!(
        status(server.http, "GET", "/api/v1/models", &[]),
        401,
        "REST must be gated, or this test proves nothing"
    );

    for &port_label in &["http", "ws"] {
        let port = if port_label == "http" { server.http } else { server.ws };
        for path in WS_PATHS {
            let code = ws_upgrade(port, path, None);
            assert_ne!(
                code, 101,
                "{port_label} port ACCEPTED an unauthenticated upgrade to {path} — \
                 this is the bypass that shipped twice"
            );
            assert_eq!(
                code, 401,
                "{port_label} port {path} should refuse with 401, got {code}"
            );
        }
    }
}

#[test]
fn a_bearer_on_the_upgrade_is_accepted_on_both_listeners() {
    let server = Server::start(&[("RUVIEW_API_TOKEN", TOKEN)]);
    // Native clients (Python, CLI, MCP) are not browser-constrained and must be
    // able to authenticate a WebSocket without the ticket round-trip.
    for (label, port) in [("http", server.http), ("ws", server.ws)] {
        assert_eq!(
            ws_upgrade(port, "/ws/sensing", Some(TOKEN)),
            101,
            "{label} port must accept a valid bearer on the upgrade"
        );
    }
}

#[test]
fn with_auth_off_both_listeners_stay_open() {
    // The compatibility promise: an unconfigured deployment sees no change.
    let server = Server::start(&[]);
    assert_eq!(status(server.http, "GET", "/api/v1/models", &[]), 200);
    for (label, port) in [("http", server.http), ("ws", server.ws)] {
        assert_eq!(
            ws_upgrade(port, "/ws/sensing", None),
            101,
            "{label} port must stay open when no credential is configured"
        );
    }
}

#[test]
fn the_legacy_escape_hatch_opens_websockets_without_weakening_rest() {
    let server = Server::start(&[
        ("RUVIEW_API_TOKEN", TOKEN),
        ("RUVIEW_WS_LEGACY_UNAUTHENTICATED", "1"),
    ]);
    // The hatch is scoped to WebSockets on purpose. If it ever widened to REST
    // it would be a bypass wearing a migration label.
    assert_eq!(
        status(server.http, "GET", "/api/v1/models", &[]),
        401,
        "the escape hatch must not weaken REST"
    );
    for (label, port) in [("http", server.http), ("ws", server.ws)] {
        assert_eq!(
            ws_upgrade(port, "/ws/sensing", None),
            101,
            "{label} port should be open while the hatch is set"
        );
    }
}

#[test]
fn health_stays_anonymous_on_both_listeners() {
    // Documented exemption (ADR-272): orchestrator probes are anonymous by
    // design. Pinned so it is a decision, not an accident nobody re-checks.
    let server = Server::start(&[("RUVIEW_API_TOKEN", TOKEN)]);
    for (label, port) in [("http", server.http), ("ws", server.ws)] {
        assert_eq!(
            status(port, "GET", "/health", &[]),
            200,
            "{label} port /health must remain anonymous"
        );
    }
}
