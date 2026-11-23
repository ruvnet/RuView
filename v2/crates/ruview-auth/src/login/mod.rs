//! Interactive Cognitum sign-in (ADR-271 phase 2). Feature `login`.
//!
//! The counterpart to this crate's verifier: the verifier checks tokens a
//! server receives, this obtains one for a user to present.
//!
//! Ported from `cognitum-one/meta-proxy` `src/oauth/`, cross-checked against
//! `musica`'s `cognitum_provider.rs` — the two independent implementations
//! against this same authorization server. Where they agree (exact
//! `/oauth/callback` redirect path, 60-second refresh skew, OOB fallback on
//! SSH/container) this follows both.
//!
//! ```no_run
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! use ruview_auth::login::{login, LoginOptions};
//!
//! let opts = LoginOptions::default(); // requests sensing:read only
//! let mut out = std::io::stdout();
//! let mut input = std::io::stdin().lock();
//! let session = login(&opts, &mut out, &mut input).await?;
//!
//! // Always go through ensure_fresh — never read access_token directly.
//! let bearer = session.ensure_fresh().await?;
//! # let _ = bearer;
//! # Ok(())
//! # }
//! ```
//!
//! # Two things that will bite if ignored
//!
//! 1. **Refresh tokens rotate with reuse detection.** Presenting a spent one
//!    revokes the session family, so refresh is serialised and never retried.
//!    Use [`store::Session::ensure_fresh`]; do not call [`client::refresh`]
//!    directly unless you are reimplementing that guarantee.
//! 2. **Least scope by default.** [`LoginOptions::default`] asks for
//!    `sensing:read`. Requesting `sensing:admin` should be a deliberate act for
//!    an administrative operation, not the standing state of every session.

pub mod callback;
/// Re-exported from the crate root; PKCE is usable without this feature.
pub use crate::pkce;
pub mod client;
pub mod flow;
pub mod lock;
pub mod store;

pub use client::{OAuthError, TokenResponse, CLIENT_ID, CLIENT_ID_ENV, OOB_REDIRECT_URI};
pub use flow::{login, logout, LoginError, LoginOptions};
pub use store::{
    default_credentials_path, Session, StoreError, StoredCredentials, CREDENTIALS_PATH_ENV,
};
