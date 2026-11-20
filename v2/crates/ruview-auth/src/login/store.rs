//! Stored credentials and the refresh critical section.
//!
//! # Why refresh is the dangerous part
//!
//! Identity **rotates refresh tokens with reuse detection**: presenting one
//! returns a replacement and spends the original, and presenting a spent token
//! revokes the whole session family. So the two obvious implementations are
//! both wrong:
//!
//! * *Refresh concurrently* — two tasks present the same token, the second
//!   looks like replay, and the user is logged out.
//! * *Retry a failed refresh with the same token* — a timeout is not evidence
//!   the server didn't consume it. Retrying is precisely the replay the server
//!   is watching for.
//!
//! [`Session::ensure_fresh`] therefore holds an async mutex **across the
//! await**, re-checks expiry after acquiring it (the task that waited may find
//! the work already done), persists the rotated token **before** returning, and
//! never retries.
//!
//! ## The in-process mutex is not enough
//!
//! Every CLI invocation is a NEW process with its own `Session` and its own
//! mutex, all sharing one credential file. Two `wifi-densepose` commands run
//! close together inside the refresh window would each load the same refresh
//! token and each present it — and the second is replay, so the user is logged
//! out for running two commands at once.
//!
//! So the critical section is also guarded by a cross-process **file lock**
//! ([`super::lock`]: `flock` on Unix, an atomic lock directory elsewhere),
//! polled without blocking the executor — the same mistake this crate had to
//! fix in `jwks.rs`. Under the lock the file is re-read, so a process that
//! waited finds another's refresh already done. A process that cannot get the
//! lock within [`LOCK_WAIT`] does NOT proceed; it reports
//! [`StoreError::RefreshBusy`] and the caller tries again later.
//!
//! ## A sent token is spent (console#857)
//!
//! Before the request, the stored record loses its refresh token, so a process
//! that dies mid-request leaves a record that asks for login, never a spent
//! token that looks usable. Any failure once the request may have reached
//! identity — a timeout, a reset, an error answer, an unreadable body, a failed
//! write of the new pair — clears the stored credentials. Only a connection
//! that was never made puts the old token back. The request is bounded by
//! [`super::client::REFRESH_TIMEOUT`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use super::client::{self, OAuthError, TokenResponse};
use super::lock::FileLock;

/// How long to wait for another process's refresh. Longer than the refresh
/// request's own timeout, so a live holder always finishes first.
pub const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

fn lock_wait() -> std::time::Duration {
    // Tests shorten it; release builds never read the environment for it.
    #[cfg(test)]
    if let Some(ms) = std::env::var("RUVIEW_TEST_LOCK_WAIT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        return std::time::Duration::from_millis(ms);
    }
    LOCK_WAIT
}

/// Refresh this many seconds before `exp`. Matches the figure meta-proxy and
/// musica independently arrived at against the same 15-minute token.
const REFRESH_SKEW_SECS: i64 = 60;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("no stored credentials — run `wifi-densepose login` first")]
    NotLoggedIn,
    #[error("credential file {path} is unreadable: {source}")]
    Unreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("credential file {path} is malformed; run `wifi-densepose login` again")]
    Malformed { path: PathBuf },
    #[error("could not write credentials to {path}: {source}")]
    Unwritable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("session expired and could not be refreshed — run `wifi-densepose login` again: {0}")]
    RefreshFailed(#[from] OAuthError),
    #[error("the authorization server returned no refresh token; re-login is required")]
    NoRefreshToken,
    #[error("another process is refreshing the session; nothing was sent — try again shortly")]
    RefreshBusy,
}

/// The persisted session. Deliberately small: this file holds live credentials.
///
/// `Debug` is hand-written and REDACTING — a derived impl prints both tokens in
/// full, and this type is the obvious thing to log when a session misbehaves.
#[derive(Clone, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub schema_version: u8,
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Unix seconds. Absent ⇒ treated as already expired, never as "valid".
    pub expires_at: Option<i64>,
    pub scope: Option<String>,
    pub account_email: Option<String>,
    pub issuer: String,
}

impl std::fmt::Debug for StoredCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredCredentials")
            .field("schema_version", &self.schema_version)
            .field("access_token", &"<redacted>")
            .field("refresh_token", &self.refresh_token.as_ref().map(|_| "<redacted>"))
            .field("expires_at", &self.expires_at)
            .field("scope", &self.scope)
            .field("account_email", &self.account_email)
            .field("issuer", &self.issuer)
            .finish()
    }
}

impl StoredCredentials {
    pub const SCHEMA_VERSION: u8 = 1;

    fn from_response(t: TokenResponse, issuer: String) -> Self {
        let expires_at = t.expires_in.map(|s| now_unix() + s);
        // Identity's /oauth/token response has no top-level `scope` field, but
        // the access token itself carries a `scope` claim — and that claim is
        // the authoritative one, since it is what a resource server actually
        // gates on. Falling back to it turns "(not reported by the server)"
        // into the real answer. Envelope first on the off chance a future
        // response does carry one.
        let scope = t
            .scope
            .clone()
            .or_else(|| scope_from_access_token(&t.access_token));
        Self {
            schema_version: Self::SCHEMA_VERSION,
            access_token: t.access_token,
            refresh_token: t.refresh_token,
            expires_at,
            scope,
            account_email: t.account_email,
            issuer,
        }
    }

    /// The granted scope, falling back to the access token's own claim.
    ///
    /// Resolved at *read* time, not just at write time, so credential files
    /// written before the fallback existed — or by any client that stores only
    /// what the token response carried — still report correctly instead of
    /// showing "(not reported)" forever. The token is the authoritative source
    /// either way; the stored field is a convenience copy.
    pub fn effective_scope(&self) -> Option<String> {
        self.scope
            .clone()
            .filter(|s| !s.is_empty())
            .or_else(|| scope_from_access_token(&self.access_token))
    }

    /// Does the access token need replacing?
    ///
    /// A missing `expires_at` counts as expired. Guessing a lifetime here would
    /// mean confidently sending a token the server may have expired minutes ago.
    pub fn needs_refresh(&self) -> bool {
        match self.expires_at {
            None => true,
            Some(exp) => now_unix() + REFRESH_SKEW_SECS >= exp,
        }
    }
}

/// Read the `scope` claim out of an access token **for display only**.
///
/// # This is NOT verification
///
/// It base64-decodes the JWT payload and does not check the signature, the
/// issuer, `exp`, `typ`, or anything else. Its only legitimate use is telling a
/// user what they just consented to, for a token this process received over TLS
/// directly from the issuer moments ago.
///
/// Never use it to make an authorization decision. Anything that gates access
/// must go through [`crate::verify::verify_access_token`], which checks the
/// signature against identity's published JWKS. A client reading its own freshly
/// issued token is a fundamentally different situation from a server reading a
/// token a stranger handed it.
///
/// Returns `None` rather than guessing if the token is not a well-formed JWT —
/// an unreadable scope must present as unknown, never as empty (which would
/// read as "you were granted nothing").
fn scope_from_access_token(jwt: &str) -> Option<String> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;

    let payload_b64 = jwt.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    claims
        .get("scope")?
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Default credential path: `~/.ruview/credentials.json`, overridable.
pub const CREDENTIALS_PATH_ENV: &str = "RUVIEW_CREDENTIALS_PATH";

pub fn default_credentials_path() -> PathBuf {
    if let Ok(p) = std::env::var(CREDENTIALS_PATH_ENV) {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".ruview").join("credentials.json")
}

/// Write credentials atomically and `0600`.
///
/// Same discipline the seed applies to its cloud key and meta-proxy to its
/// config: temp file in the destination directory, restrict the mode *before*
/// the rename, then rename. A partial credential file is worse than none, and a
/// world-readable one is a live session anyone on the box can steal.
pub fn save(path: &Path, creds: &StoredCredentials) -> Result<(), StoreError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|source| StoreError::Unwritable {
            path: path.to_path_buf(),
            source,
        })?;
    }
    let json = serde_json::to_vec_pretty(creds).expect("credentials serialize");
    let tmp = path.with_extension("tmp");

    // Create with 0600 ALREADY SET, rather than write-then-chmod.
    //
    // `fs::write` creates at `0666 & !umask` — 0644 on a default umask — so the
    // refresh token was world-readable at a predictable path for the window
    // between the write and the chmod. `save` runs on every silent refresh
    // (REFRESH_SKEW_SECS against a 15-minute token), so that window recurred
    // every few minutes, and the refresh token is the highest-value credential
    // here: identity rotates with reuse detection, so a thief who presents it
    // first takes the session family and logs the real user out.
    write_private(&tmp, &json).map_err(|source| StoreError::Unwritable {
        path: tmp.clone(),
        source,
    })?;
    std::fs::rename(&tmp, path).map_err(|source| StoreError::Unwritable {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// Write `bytes` to a file that is never readable by anyone else, at any point.
///
/// `create_new` also means a pre-existing `.tmp` — a symlink planted by a local
/// attacker, or a leftover from a crash — is an error rather than a target.
#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let _ = std::fs::remove_file(path); // clear our own leftover, not a race
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|source| {
        StoreError::Unwritable {
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(not(unix))]
fn restrict_permissions(path: &Path) -> Result<(), StoreError> {
    // Windows: inherit the user profile directory's ACL. `icacls` would be the
    // stricter equivalent; noted rather than silently pretended.
    let _ = path;
    Ok(())
}

pub fn load(path: &Path) -> Result<StoredCredentials, StoreError> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(StoreError::NotLoggedIn),
        Err(source) => {
            return Err(StoreError::Unreadable {
                path: path.to_path_buf(),
                source,
            })
        }
    };
    serde_json::from_slice(&bytes).map_err(|_| StoreError::Malformed {
        path: path.to_path_buf(),
    })
}

/// Remove stored credentials. Idempotent.
///
/// This forgets the local copy; it does not revoke server-side. That is a
/// deliberate split (meta-proxy makes the same one): "this machine can no
/// longer act as me" is the fail-secure local action, and revocation is a
/// separate, account-level decision.
pub fn clear(path: &Path) -> Result<bool, StoreError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(StoreError::Unwritable {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// A live session that refreshes itself, safely, at most once at a time.
#[derive(Clone)]
pub struct Session {
    path: PathBuf,
    http: reqwest::Client,
    inner: Arc<Mutex<StoredCredentials>>,
}

impl Session {
    pub fn load_from(path: PathBuf, http: reqwest::Client) -> Result<Self, StoreError> {
        let creds = load(&path)?;
        Ok(Self {
            path,
            http,
            inner: Arc::new(Mutex::new(creds)),
        })
    }

    pub fn from_response(
        path: PathBuf,
        http: reqwest::Client,
        token: TokenResponse,
        issuer: String,
    ) -> Result<Self, StoreError> {
        let creds = StoredCredentials::from_response(token, issuer);
        save(&path, &creds)?;
        Ok(Self {
            path,
            http,
            inner: Arc::new(Mutex::new(creds)),
        })
    }

    pub async fn snapshot(&self) -> StoredCredentials {
        self.inner.lock().await.clone()
    }

    /// Return a non-expired access token, refreshing if needed.
    ///
    /// The mutex is held **across the network call** on purpose. That
    /// serialises refreshes, which is the entire point: identity's reuse
    /// detection turns a concurrent second refresh into a session revocation.
    /// The re-check after acquiring means a task that queued behind another's
    /// refresh returns the fresh token instead of spending the rotated one.
    pub async fn ensure_fresh(&self) -> Result<String, StoreError> {
        let mut guard = self.inner.lock().await;

        if !guard.needs_refresh() {
            return Ok(guard.access_token.clone());
        }

        let Some(_file_lock) = FileLock::acquire(&self.path, lock_wait()).await else {
            return Err(StoreError::RefreshBusy);
        };
        // Re-read under the lock: another process may have refreshed (or
        // logged out, or logged in again) since this one loaded the file.
        *guard = load(&self.path)?;
        if !guard.needs_refresh() {
            return Ok(guard.access_token.clone());
        }

        // A record with no refresh token is either a login that never had one
        // or a refresh that was interrupted after it may have sent the token.
        let Some(refresh_token) = guard.refresh_token.clone() else {
            return Err(StoreError::NoRefreshToken);
        };

        // Token-less marker before sending. If this fails nothing was sent.
        let mut marker = guard.clone();
        marker.refresh_token = None;
        save(&self.path, &marker)?;

        // Deliberately not retried. A timeout is not evidence the server did
        // not consume the token, and re-presenting it is exactly the replay
        // that revokes the session.
        let refreshed = match client::refresh(&self.http, &refresh_token).await {
            Ok(t) => t,
            Err(OAuthError::Network(e)) if e.is_connect() => {
                // Never sent: the token is still good.
                let _ = save(&self.path, &guard);
                return Err(StoreError::RefreshFailed(OAuthError::Network(e)));
            }
            Err(e) => {
                // Possibly consumed: this machine must never present it again.
                *guard = marker;
                let _ = clear(&self.path);
                return Err(StoreError::RefreshFailed(e));
            }
        };

        let issuer = guard.issuer.clone();
        // No fallback to the old refresh token if the answer omits a new one:
        // the old one was just spent, and presenting it again revokes the family.
        let next = StoredCredentials::from_response(refreshed, issuer);

        // Persist BEFORE handing the new access token out: a crash between the
        // two otherwise leaves a rotated-away token on disk and a live one only
        // in memory.
        if let Err(e) = save(&self.path, &next) {
            *guard = marker;
            let _ = clear(&self.path);
            return Err(e);
        }
        let token = next.access_token.clone();
        *guard = next;
        Ok(token)
    }
}

#[cfg(test)]
#[path = "store_refresh_tests.rs"]
mod refresh_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn creds(expires_at: Option<i64>) -> StoredCredentials {
        StoredCredentials {
            schema_version: StoredCredentials::SCHEMA_VERSION,
            access_token: "at".into(),
            refresh_token: Some("rt".into()),
            expires_at,
            scope: Some("sensing:read".into()),
            account_email: Some("a@b.c".into()),
            issuer: "https://auth.test".into(),
        }
    }

    #[test]
    fn a_token_with_no_expiry_is_treated_as_expired() {
        // Guessing a lifetime would mean confidently sending a token the
        // server may have expired minutes ago.
        assert!(creds(None).needs_refresh());
    }

    #[test]
    fn a_freshly_issued_token_does_not_need_refreshing() {
        assert!(!creds(Some(now_unix() + 900)).needs_refresh());
    }

    #[test]
    fn refresh_is_triggered_inside_the_skew_window() {
        // 30s left, 60s skew — refresh now rather than racing expiry mid-request.
        assert!(creds(Some(now_unix() + 30)).needs_refresh());
    }

    #[test]
    fn an_already_expired_token_needs_refreshing() {
        assert!(creds(Some(now_unix() - 1)).needs_refresh());
    }

    #[test]
    fn redacted_debug_never_prints_token_material() {
        // A derived Debug prints both tokens in full, and this is the obvious
        // type to log when a session misbehaves.
        // Distinctive values — an earlier version of this test used "at"/"rt",
        // which collide with `expires_at` and produce a false failure.
        let mut c = creds(Some(1));
        c.access_token = "SECRET-ACCESS-VALUE".into();
        c.refresh_token = Some("SECRET-REFRESH-VALUE".into());
        let rendered = format!("{c:?}");
        assert!(
            !rendered.contains("SECRET-ACCESS-VALUE"),
            "access token leaked: {rendered}"
        );
        assert!(
            !rendered.contains("SECRET-REFRESH-VALUE"),
            "refresh token leaked: {rendered}"
        );
        assert!(rendered.contains("<redacted>"));
        // Non-secret fields stay visible or the type is useless for debugging.
        assert!(rendered.contains("https://auth.test"));
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("ruview-auth-test-{}", std::process::id()));
        let path = dir.join("credentials.json");
        let _ = std::fs::remove_dir_all(&dir);

        save(&path, &creds(Some(123))).unwrap();
        let back = load(&path).unwrap();
        assert_eq!(back.access_token, "at");
        assert_eq!(back.expires_at, Some(123));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_saved_credential_file_is_not_readable_by_anyone_else() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("ruview-auth-perm-{}", std::process::id()));
        let path = dir.join("credentials.json");
        let _ = std::fs::remove_dir_all(&dir);

        save(&path, &creds(Some(1))).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "credentials must be 0600, got {mode:o}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn the_temp_file_is_never_world_readable_even_for_an_instant() {
        // The test above checks the FINAL file. It passed while `save` wrote via
        // `fs::write` (0644 under a default umask) and chmodded afterwards — so
        // the refresh token sat world-readable at a predictable path in between,
        // on every silent refresh. Asserting on the destination could never see
        // that; this asserts on the temp file `save` actually creates.
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("ruview-auth-tmpperm-{}", std::process::id()));
        let path = dir.join("credentials.json");
        let tmp = path.with_extension("tmp");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        write_private(&tmp, b"secret").unwrap();
        let mode = std::fs::metadata(&tmp).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "temp file must be created 0600, got {mode:o}");
        assert_eq!(mode & 0o077, 0, "group/other must have no access at all");

        // A leftover from a crashed run is cleared and replaced, and the
        // replacement is 0600 too — the mode must not be inherited from
        // whatever was there before.
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o666));
        write_private(&tmp, b"replacement").unwrap();
        let mode = std::fs::metadata(&tmp).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "a replaced temp file must also be 0600, got {mode:o}");
        assert_eq!(std::fs::read(&tmp).unwrap(), b"replacement", "must replace, not append");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn loading_a_missing_file_says_not_logged_in_rather_than_erroring_obscurely() {
        let path = std::env::temp_dir().join("ruview-auth-definitely-absent.json");
        let _ = std::fs::remove_file(&path);
        assert!(matches!(load(&path), Err(StoreError::NotLoggedIn)));
    }

    #[test]
    fn a_corrupt_credential_file_is_reported_as_malformed_not_as_absent() {
        let dir = std::env::temp_dir().join(format!("ruview-auth-bad-{}", std::process::id()));
        let path = dir.join("credentials.json");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, b"{not json").unwrap();
        assert!(matches!(load(&path), Err(StoreError::Malformed { .. })));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clearing_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("ruview-auth-clear-{}", std::process::id()));
        let path = dir.join("credentials.json");
        let _ = std::fs::remove_dir_all(&dir);

        save(&path, &creds(Some(1))).unwrap();
        assert!(clear(&path).unwrap(), "first clear removes the file");
        assert!(!clear(&path).unwrap(), "second clear is a no-op, not an error");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_leaves_no_temp_file_behind() {
        let dir = std::env::temp_dir().join(format!("ruview-auth-tmp-{}", std::process::id()));
        let path = dir.join("credentials.json");
        let _ = std::fs::remove_dir_all(&dir);

        save(&path, &creds(Some(1))).unwrap();
        assert!(!path.with_extension("tmp").exists(), "temp file must be renamed away");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The scope-from-token fallback. Split out so its "display only, never an
/// authorization input" contract is pinned by name.
#[cfg(test)]
mod scope_display_tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;

    fn jwt_with_payload(payload: serde_json::Value) -> String {
        // Header and signature are irrelevant here — that is the whole point:
        // this path never inspects them, so the test must not imply it does.
        format!(
            "eyJhbGciOiJFUzI1NiJ9.{}.not-a-real-signature",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap())
        )
    }

    #[test]
    fn reads_the_scope_claim_when_the_envelope_omits_it() {
        // The live behaviour that motivated this: identity's /oauth/token
        // response carries no top-level `scope`, but the token does.
        let t = jwt_with_payload(serde_json::json!({"scope": "sensing:read"}));
        assert_eq!(scope_from_access_token(&t).as_deref(), Some("sensing:read"));
    }

    #[test]
    fn reads_a_multi_scope_claim_intact() {
        let t = jwt_with_payload(serde_json::json!({"scope": "sensing:read sensing:admin"}));
        assert_eq!(
            scope_from_access_token(&t).as_deref(),
            Some("sensing:read sensing:admin")
        );
    }

    #[test]
    fn an_unparseable_token_reads_as_unknown_not_as_empty() {
        // "" would render as "you were granted nothing", which is a different
        // and wrong claim.
        assert_eq!(scope_from_access_token("not-a-jwt"), None);
        assert_eq!(scope_from_access_token(""), None);
        assert_eq!(scope_from_access_token("a.!!!not-base64!!!.c"), None);
    }

    #[test]
    fn an_empty_scope_claim_reads_as_unknown() {
        let t = jwt_with_payload(serde_json::json!({"scope": ""}));
        assert_eq!(scope_from_access_token(&t), None);
    }

    #[test]
    fn a_token_with_no_scope_claim_reads_as_unknown() {
        let t = jwt_with_payload(serde_json::json!({"sub": "u1"}));
        assert_eq!(scope_from_access_token(&t), None);
    }

    #[test]
    fn the_response_envelope_wins_when_it_does_carry_a_scope() {
        let token = TokenResponse {
            access_token: jwt_with_payload(serde_json::json!({"scope": "from:token"})),
            token_type: None,
            account_email: None,
            refresh_token: None,
            expires_in: Some(900),
            scope: Some("from:envelope".into()),
        };
        let c = StoredCredentials::from_response(token, "https://auth.test".into());
        assert_eq!(c.scope.as_deref(), Some("from:envelope"));
    }

    #[test]
    fn the_token_claim_is_used_when_the_envelope_is_silent() {
        let token = TokenResponse {
            access_token: jwt_with_payload(serde_json::json!({"scope": "sensing:read"})),
            token_type: None,
            account_email: None,
            refresh_token: None,
            expires_in: Some(900),
            scope: None,
        };
        let c = StoredCredentials::from_response(token, "https://auth.test".into());
        assert_eq!(c.scope.as_deref(), Some("sensing:read"));
    }
}
