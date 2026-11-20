//! console#857 refresh tests: one refresh across processes, no proceed after a
//! lock timeout, and a sent token is never kept. Each scenario runs in a child
//! process (this test binary, re-invoked), so the environment it needs never
//! races another test. Identity is an in-process fixture on 127.0.0.1, and
//! every token is a fixture string.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::*;

#[derive(Clone, Copy)]
enum Answer {
    /// 200 with a new pair after `delay_ms`.
    Rotate { delay_ms: u64 },
    /// 200 with an access token but no refresh token.
    RotateWithoutRefresh,
    /// Read the request, then never answer.
    Hang,
}

/// Returns the base URL and the count of refresh requests that arrived.
fn identity(answer: Answer) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let refreshes = Arc::new(AtomicUsize::new(0));
    let count = refreshes.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let count = count.clone();
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let n = stream.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&buf).to_string();
                    if let Some(end) = text.find("\r\n\r\n") {
                        let len = text[..end]
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                            })
                            .unwrap_or(0);
                        if buf.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                let n = count.fetch_add(1, Ordering::SeqCst) + 1;
                let json = match answer {
                    Answer::Hang => {
                        std::thread::sleep(Duration::from_secs(5));
                        return;
                    }
                    Answer::Rotate { delay_ms } => {
                        std::thread::sleep(Duration::from_millis(delay_ms));
                        format!(
                            r#"{{"access_token":"fixture-access-{n}","refresh_token":"fixture-refresh-{n}","expires_in":900}}"#
                        )
                    }
                    Answer::RotateWithoutRefresh => {
                        format!(r#"{{"access_token":"fixture-access-{n}","expires_in":900}}"#)
                    }
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{json}",
                    json.len()
                );
            });
        }
    });
    (url, refreshes)
}

fn expired(refresh_token: Option<&str>) -> StoredCredentials {
    StoredCredentials {
        schema_version: StoredCredentials::SCHEMA_VERSION,
        access_token: "fixture-access-0".into(),
        refresh_token: refresh_token.map(str::to_owned),
        expires_at: Some(now_unix() - 60),
        scope: None,
        account_email: None,
        issuer: "https://auth.test".into(),
    }
}

const CHILD: &str = "RUVIEW_TEST_REFRESH_CHILD";

/// Runs only as a child of the tests below: one `ensure_fresh` against the
/// inherited credential path and auth URL, outcome printed for the parent.
#[tokio::test]
async fn refresh_child_process() {
    if std::env::var(CHILD).as_deref() != Ok("1") {
        return;
    }
    let path = default_credentials_path();
    let outcome = match Session::load_from(path, reqwest::Client::new()) {
        Ok(s) => match s.ensure_fresh().await {
            Ok(t) => format!("ok {t}"),
            Err(e) => format!("err {e:?}"),
        },
        Err(e) => format!("err {e:?}"),
    };
    println!("CHILD-RESULT {outcome}");
}

fn spawn(path: &Path, url: &str, extra: &[(&str, &str)]) -> std::process::Child {
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        "login::store::refresh_tests::refresh_child_process",
        "--nocapture",
        "--test-threads=1",
    ])
    .env(CHILD, "1")
    .env(CREDENTIALS_PATH_ENV, path)
    .env(client::AUTH_URL_ENV, url)
    .stdout(std::process::Stdio::piped());
    for (k, v) in extra {
        cmd.env(k, v);
    }
    cmd.spawn().unwrap()
}

fn result(child: std::process::Child) -> String {
    let out = String::from_utf8_lossy(&child.wait_with_output().unwrap().stdout).to_string();
    out.lines()
        .find_map(|l| l.split("CHILD-RESULT ").nth(1).map(str::to_owned))
        .unwrap_or_else(|| panic!("child printed no result: {out}"))
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ruview-auth-857-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("credentials.json")
}

#[test]
fn two_processes_refresh_once() {
    let (url, refreshes) = identity(Answer::Rotate { delay_ms: 400 });
    let path = scratch("two");
    save(&path, &expired(Some("fixture-refresh-0"))).unwrap();
    let a = spawn(&path, &url, &[]);
    let b = spawn(&path, &url, &[]);
    let (a, b) = (result(a), result(b));
    assert_eq!(
        refreshes.load(Ordering::SeqCst),
        1,
        "exactly one refresh reached identity: {a} / {b}"
    );
    assert_eq!(a, "ok fixture-access-1");
    assert_eq!(
        b, "ok fixture-access-1",
        "the waiter re-read the file under the lock"
    );
    assert_eq!(
        load(&path).unwrap().refresh_token.as_deref(),
        Some("fixture-refresh-1")
    );
}

#[test]
fn a_timeout_after_sending_clears_the_stored_token() {
    let (url, refreshes) = identity(Answer::Hang);
    let path = scratch("timeout");
    save(&path, &expired(Some("fixture-refresh-0"))).unwrap();
    let r = result(spawn(
        &path,
        &url,
        &[("RUVIEW_TEST_REFRESH_TIMEOUT_MS", "300")],
    ));
    assert!(r.starts_with("err RefreshFailed"), "{r}");
    assert_eq!(
        refreshes.load(Ordering::SeqCst),
        1,
        "the request reached identity"
    );
    assert!(
        matches!(load(&path), Err(StoreError::NotLoggedIn)),
        "the spent token is not kept"
    );
    // The next run has nothing to present and sends nothing.
    assert_eq!(result(spawn(&path, &url, &[])), "err NotLoggedIn");
    assert_eq!(refreshes.load(Ordering::SeqCst), 1);
}

#[test]
fn an_interrupted_refresh_leaves_a_token_less_record_that_is_never_sent() {
    let (url, refreshes) = identity(Answer::Rotate { delay_ms: 0 });
    let path = scratch("marker");
    // What a process that died mid-request leaves behind.
    save(&path, &expired(None)).unwrap();
    assert_eq!(result(spawn(&path, &url, &[])), "err NoRefreshToken");
    assert_eq!(refreshes.load(Ordering::SeqCst), 0);
}

#[test]
fn a_held_lock_is_not_proceeded_past_and_sends_nothing() {
    let (url, refreshes) = identity(Answer::Rotate { delay_ms: 0 });
    let path = scratch("busy");
    save(&path, &expired(Some("fixture-refresh-0"))).unwrap();
    let held = FileLock::try_acquire(&path).unwrap();
    let r = result(spawn(&path, &url, &[("RUVIEW_TEST_LOCK_WAIT_MS", "300")]));
    assert_eq!(r, "err RefreshBusy");
    assert_eq!(
        refreshes.load(Ordering::SeqCst),
        0,
        "no proceed after a lock timeout"
    );
    assert_eq!(
        load(&path).unwrap().refresh_token.as_deref(),
        Some("fixture-refresh-0")
    );
    drop(held);
}

#[test]
fn a_connection_never_made_keeps_the_token() {
    let path = scratch("refused");
    save(&path, &expired(Some("fixture-refresh-0"))).unwrap();
    let r = result(spawn(&path, "http://127.0.0.1:1", &[]));
    assert!(r.starts_with("err RefreshFailed(Network"), "{r}");
    assert_eq!(
        load(&path).unwrap().refresh_token.as_deref(),
        Some("fixture-refresh-0"),
        "unsent, so still good"
    );
}

#[test]
fn an_answer_without_a_new_refresh_token_never_keeps_the_spent_one() {
    let (url, refreshes) = identity(Answer::RotateWithoutRefresh);
    let path = scratch("norefresh");
    save(&path, &expired(Some("fixture-refresh-0"))).unwrap();
    assert_eq!(result(spawn(&path, &url, &[])), "ok fixture-access-1");
    assert_eq!(refreshes.load(Ordering::SeqCst), 1);
    assert_eq!(load(&path).unwrap().refresh_token, None);
}
