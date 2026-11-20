//! The ephemeral loopback listener the browser redirects back to, plus opening
//! the system browser.
//!
//! A hand-rolled HTTP/1.1 responder rather than a second axum server: it serves
//! exactly one GET, then shuts down. Ported from `meta-proxy`
//! `src/oauth/{callback_server,browser}.rs`.

use std::net::SocketAddr;
#[cfg(not(test))]
use std::process::Stdio;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};

pub struct CallbackServer {
    listener: TcpListener,
    /// The exact value to send as `redirect_uri`.
    pub redirect_uri: String,
}

#[derive(Debug, Clone, Default)]
pub struct CallbackResult {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

const SUCCESS_PAGE: &str = r#"<html>
<body style="background:#0a0a0a; color:#f5f5f5; font-family:system-ui,sans-serif;
    display:flex; align-items:center; justify-content:center; height:100vh; margin:0;">
  <div style="text-align:center;">
    <h1>&#10003; RuView sign-in complete</h1>
    <p>You can close this tab and return to your terminal.</p>
  </div>
</body>
</html>"#;

impl CallbackServer {
    /// Bind `127.0.0.1:0` and derive the redirect URI.
    ///
    /// The path must be **exactly** `/oauth/callback`: identity's
    /// `client::validate_redirect_uri` accepts `http://127.0.0.1:<any-port>/oauth/callback`
    /// and nothing else, so a different path fails the authorize request with a
    /// redirect-URI mismatch rather than anything that names the real problem.
    pub async fn bind() -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let addr: SocketAddr = listener.local_addr()?;
        Ok(Self {
            redirect_uri: format!("http://127.0.0.1:{}/oauth/callback", addr.port()),
            listener,
        })
    }

    pub fn port(&self) -> u16 {
        self.listener.local_addr().map(|a| a.port()).unwrap_or(0)
    }

    /// Serve exactly one callback, reply with the success page, return the
    /// parsed query. Times out so an abandoned browser tab does not hang the
    /// CLI forever.
    pub async fn await_callback(&self, wait_for: Duration) -> std::io::Result<CallbackResult> {
        let (mut stream, _) = timeout(wait_for, self.listener.accept())
            .await
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "timed out waiting for the OAuth callback — was the browser window closed?",
                )
            })??;

        let mut buf = vec![0u8; 8192];
        let n = stream.read(&mut buf).await?;
        let text = String::from_utf8_lossy(&buf[..n]);
        let target = text
            .lines()
            .next()
            .unwrap_or("")
            .split_whitespace()
            .nth(1)
            .unwrap_or("/oauth/callback")
            .to_string();

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            SUCCESS_PAGE.len(),
            SUCCESS_PAGE
        );
        stream.write_all(response.as_bytes()).await?;
        stream.shutdown().await?;

        Ok(parse_callback_query(&target))
    }
}

fn parse_callback_query(path_and_query: &str) -> CallbackResult {
    let query = path_and_query.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut out = CallbackResult::default();
    for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
        match k.as_ref() {
            "code" => out.code = Some(v.into_owned()),
            "state" => out.state = Some(v.into_owned()),
            "error" => out.error = Some(v.into_owned()),
            _ => {}
        }
    }
    out
}

/// Open `url` in the system browser.
///
/// Success means the launcher was spawned, not that a window appeared — which
/// cannot be determined in general. Callers must print the URL regardless.
pub fn open_browser(url: &str) -> std::io::Result<()> {
    let (cmd, args) = browser_command(url);
    launch(cmd, &args)
}

/// The launcher command for `url` on this platform.
fn browser_command(url: &str) -> (&'static str, Vec<String>) {
    if cfg!(target_os = "macos") {
        ("open", vec![url.to_string()])
    } else if cfg!(target_os = "windows") {
        // The empty title argument stops `start` treating a quoted URL as the
        // window title.
        (
            "cmd",
            vec!["/c".into(), "start".into(), String::new(), url.to_string()],
        )
    } else {
        ("xdg-open", vec![url.to_string()])
    }
}

#[cfg(not(test))]
fn launch(cmd: &str, args: &[String]) -> std::io::Result<()> {
    std::process::Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}

/// Test builds never start a process, so no test can open a tab in the
/// developer's browser. The command that would have run is recorded instead.
#[cfg(test)]
fn launch(cmd: &str, args: &[String]) -> std::io::Result<()> {
    LAUNCHED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((cmd.to_string(), args.to_vec()));
    Ok(())
}

#[cfg(test)]
static LAUNCHED: std::sync::Mutex<Vec<(String, Vec<String>)>> = std::sync::Mutex::new(Vec::new());

/// Does this process look like it has no usable browser?
///
/// Mirrors meta-proxy's detection exactly (`login.rs:105-108`). It is a
/// heuristic, which is why `--no-browser` exists: a wrong guess costs the user
/// one flag, not a failed login.
pub fn looks_headless() -> bool {
    std::env::var("SSH_CONNECTION").is_ok()
        || std::env::var("SSH_TTY").is_ok()
        || std::env::var("CONTAINER").is_ok()
        || std::path::Path::new("/.dockerenv").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_code_and_state() {
        let r = parse_callback_query("/oauth/callback?code=abc&state=xyz");
        assert_eq!(r.code.as_deref(), Some("abc"));
        assert_eq!(r.state.as_deref(), Some("xyz"));
        assert!(r.error.is_none());
    }

    #[test]
    fn parses_a_denial() {
        let r = parse_callback_query("/oauth/callback?error=access_denied&state=xyz");
        assert_eq!(r.error.as_deref(), Some("access_denied"));
        assert!(r.code.is_none());
    }

    #[test]
    fn percent_encoded_values_are_decoded() {
        let r = parse_callback_query("/oauth/callback?code=a%2Bb%2Fc&state=s");
        assert_eq!(r.code.as_deref(), Some("a+b/c"));
    }

    #[test]
    fn a_query_less_callback_yields_nothing_rather_than_panicking() {
        let r = parse_callback_query("/oauth/callback");
        assert!(r.code.is_none() && r.state.is_none() && r.error.is_none());
    }

    #[tokio::test]
    async fn the_redirect_uri_has_the_exact_shape_identity_requires() {
        let s = CallbackServer::bind().await.unwrap();
        assert!(s.redirect_uri.starts_with("http://127.0.0.1:"));
        assert!(s.redirect_uri.ends_with("/oauth/callback"));
        assert_ne!(s.port(), 0, "must bind a real ephemeral port");
    }

    #[tokio::test]
    async fn a_real_tcp_callback_round_trips() {
        let server = CallbackServer::bind().await.unwrap();
        let port = server.port();
        let client = tokio::spawn(async move {
            let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .unwrap();
            s.write_all(
                b"GET /oauth/callback?code=real&state=st HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
            )
            .await
            .unwrap();
            let mut buf = vec![0u8; 4096];
            let n = s.read(&mut buf).await.unwrap();
            String::from_utf8_lossy(&buf[..n]).to_string()
        });

        let r = server.await_callback(Duration::from_secs(5)).await.unwrap();
        assert_eq!(r.code.as_deref(), Some("real"));
        assert_eq!(r.state.as_deref(), Some("st"));

        let page = client.await.unwrap();
        assert!(page.contains("200 OK"));
        assert!(page.contains("RuView sign-in complete"));
    }

    #[tokio::test]
    async fn an_abandoned_login_times_out_instead_of_hanging() {
        let server = CallbackServer::bind().await.unwrap();
        assert!(server
            .await_callback(Duration::from_millis(50))
            .await
            .is_err());
    }

    #[test]
    fn opening_a_browser_records_the_launcher_command_instead_of_running_it() {
        let url = "http://127.0.0.1:1/nope";
        open_browser(url).unwrap();
        let launched = LAUNCHED.lock().unwrap_or_else(|e| e.into_inner());
        let (cmd, args) = launched.last().expect("open_browser recorded a command");
        let (want_cmd, want_args) = browser_command(url);
        assert_eq!((cmd.as_str(), args), (want_cmd, &want_args));
        if cfg!(target_os = "linux") {
            assert_eq!(cmd, "xdg-open");
            assert_eq!(args, &[url.to_string()]);
        }
    }
}
