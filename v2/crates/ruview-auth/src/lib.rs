//! Cognitum OAuth access-token verification for RuView (ADR-271).
//!
//! RuView is an OAuth **resource server**, not a Cognitum API client: it makes
//! no authenticated calls to `cognitum.one`. A user signs in to their *own*
//! RuView instance with their Cognitum identity, and this crate verifies the
//! resulting access token **offline**, against identity's published JWKS.
//!
//! Offline is the requirement, not an optimisation — RuView runs on Pi-class
//! hardware that loses WAN, and there is no token-introspection endpoint to call
//! even when the network is up.
//!
//! The transport is injected, so this compiles with or without the
//! `ureq-transport` feature. With it enabled, pass
//! [`UreqFetcher::new()`][jwks::UreqFetcher] instead of writing your own.
//!
//! ```no_run
//! use ruview_auth::{
//!     jwks::{JwksError, JwksFetcher},
//!     scope, verify_access_token, JwksCache, VerifierConfig,
//! };
//!
//! struct MyFetcher;
//! impl JwksFetcher for MyFetcher {
//!     fn fetch(&self, url: &str) -> Result<String, JwksError> {
//!         # let _ = url;
//!         // ... GET `url`, return the body ...
//!         # unimplemented!()
//!     }
//! }
//!
//! let jwks = JwksCache::new(
//!     "https://auth.cognitum.one/.well-known/jwks.json",
//!     Box::new(MyFetcher),
//! );
//! // Fail at boot, not on a user's first request.
//! jwks.warm().expect("JWKS reachable at startup");
//!
//! let config = VerifierConfig {
//!     issuer: "https://auth.cognitum.one".to_string(),
//!     required_scope: scope::SENSING_READ.to_string(),
//!     // Audience: Cognitum has no `aud`, so `client_id` carries it.
//!     allowed_client_ids: vec!["ruview".to_string()],
//! };
//!
//! let principal = verify_access_token("<jwt>", &jwks, &config)?;
//! println!("{} on account {}", principal.subject, principal.account_id);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Scope is the capability boundary
//!
//! Cognitum access tokens carry no `aud`, and `client_id` is unreliable because
//! clients borrow each other's registrations. So the scope claim is the only
//! thing separating "may watch the sensing stream" from "may delete the trained
//! model". Callers pick [`VerifierConfig::required_scope`] per route:
//! [`scope::SENSING_READ`] for streams and inference,
//! [`scope::SENSING_ADMIN`] for training, model delete and recording delete.
//!
//! ## What this crate deliberately does not do
//!
//! - **No login flow by default.** Obtaining a token (PKCE, loopback, OOB
//!   paste) lives behind the non-default `login` feature, so a server that only
//!   verifies never compiles it. See [`login`] and ADR-271's 2026-07-22
//!   amendment for why it lives here rather than in a second crate.
//! - **No revocation check.** There is no introspection endpoint. The 15-minute
//!   token lifetime *is* the revocation window, which is precisely why
//!   long-lived setup/workload credentials are refused outright.
//! - **No crypto.** Signature math is `jsonwebtoken`'s.

pub mod jwks;
pub mod principal;
pub mod verify;

/// PKCE generation (RFC 7636). Available without the full `login` stack so a
/// resource server can drive its own browser redirect.
#[cfg(feature = "pkce")]
pub mod pkce;

/// Interactive sign-in (PKCE, loopback, OOB paste, credential storage,
/// single-flight refresh). Off by default — a sensing server verifies tokens
/// and never obtains them, so it must not pay for the HTTP client this needs.
#[cfg(feature = "login")]
pub mod login;

pub use jwks::{JwksCache, JwksError, JwksFetcher};
#[cfg(feature = "ureq-transport")]
pub use jwks::UreqFetcher;
pub use principal::{scope, Principal};
pub use verify::{extract_bearer, verify_access_token, verify_account_identity, AccountIdentity, VerifierConfig, VerifyError};
