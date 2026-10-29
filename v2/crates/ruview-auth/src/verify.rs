//! Cognitum OAuth access-token verification (ADR-271).
//!
//! The accept-rule is ported from `meta-llm/src/auth/oauthBearer.ts` (ADR-045),
//! the only other resource-server-side verifier of these tokens in the org.
//! Divergence from it would be a bug, not a preference — a token meta-llm
//! rejects must not be one RuView accepts.
//!
//! ## The trust chain, narrowly
//!
//! 1. Only identity's ES256 key — fetched from the published JWKS by `kid` —
//!    can sign an accepted token. No shared secret, no static PEM to leak.
//! 2. **The algorithm is fixed to ES256 by this code.** The token header's `alg`
//!    is only ever *compared against* that allowlist, never used to *select* an
//!    algorithm. That is what makes `alg: none` and RSA-substitution
//!    non-starters rather than things we defend against case by case.
//! 3. Signature math is `jsonwebtoken`'s. This module owns claim policy only.
//!
//! ## Why `setup` and `workload` tokens are refused outright
//!
//! Identity also issues long-lived *setup* (365-day) and *workload* credentials.
//! Their revocation lives in identity's `oauth_setup_tokens` table, and RuView —
//! like meta-llm — has **no database and no way to check it**. A 15-minute
//! access token needs no revocation round-trip because it expires faster than
//! any realistic revocation propagates; a 365-day one does. Accepting one would
//! mean honouring a credential that may already have been revoked, so we don't.
//!
//! ## There is no `aud` claim, and no `iss` claim either
//!
//! Verified against real production tokens: the claim set is exactly
//! `typ, sub, account_id, org_id, workspace_id, client_id, scope, family_id,
//! jti, iat, exp, setup, workload`. No audience. No issuer.
//!
//! **What binds a token to its issuer, then?** The JWKS. We accept only
//! signatures made by a key served from the configured `jwks_uri`, so
//! possession of a valid signature *is* proof of issuer. Adding an `iss` claim
//! check on top would not strengthen that — and requiring a claim identity does
//! not emit rejects every genuine token, which is exactly what an earlier
//! revision of this module did.
//!
//! **`client_id` is Cognitum's stand-in for `aud`.** `cognitum-one/freetokens`
//! (live) documents the contract — *"Cognitum access tokens intentionally use
//! custom `client_id` rather than a registered JWT `aud` claim"* — and rejects
//! any token whose `client_id` is not its own. This verifier does the same via
//! [`VerifierConfig::allowed_client_ids`].
//!
//! An earlier revision treated `client_id` as unusable because clients borrow
//! each other's registrations (musica shipped as `meta-proxy` while its own was
//! pending) and relied on scope alone. That was reasoning from a transitional
//! state: RuView has its own registered client, and accepting a token minted for
//! any Cognitum product is a weaker position than the platform intends.
//!
//! So there are now TWO boundaries, not one: audience (`client_id`) and
//! capability (`scope`). Neither is optional garnish.

use jsonwebtoken::{decode, decode_header, Algorithm, Validation};
use serde::Deserialize;

use crate::jwks::{JwksCache, JwksError};
use crate::principal::Principal;

/// The `typ` identity stamps on ordinary interactive access tokens
/// (`jwt.rs`'s `TOKEN_TYP_ACCESS`).
const TYP_ACCESS: &str = "access";

/// Clock leeway for `exp`/`iat`.
///
/// Deliberately small. Against a 15-minute token a generous window is a real
/// extension of a revoked credential's life, so this absorbs ordinary NTP jitter
/// and nothing more. Hosts without a battery-backed clock (Pi-class) need real
/// time sync — see [`VerifyError::ExpiredOrNotYetValid`], which is reported
/// distinctly so "your clock is wrong" is diagnosable rather than presenting as
/// a generic 401.
const CLOCK_LEEWAY_SECS: u64 = 30;

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("authorization header is missing or not a Bearer token")]
    MissingBearer,
    #[error("token is not a well-formed JWT: {0}")]
    Malformed(String),
    #[error("token algorithm is not ES256")]
    WrongAlgorithm,
    #[error("could not resolve a verification key: {0}")]
    Jwks(#[from] JwksError),
    #[error("token signature is not valid for identity's published key")]
    BadSignature,
    /// `exp`/`iat` outside the accepted window. Distinct from `BadSignature` on
    /// purpose: on an RTC-less host this is usually a clock-sync problem, not an
    /// attack, and an operator needs to be able to tell those apart.
    #[error("token is expired or not yet valid (check host clock sync)")]
    ExpiredOrNotYetValid,
    #[error("token type {found:?} is not an interactive access token")]
    WrongTokenType { found: Option<String> },
    #[error("long-lived setup/workload credentials are not accepted (unverifiable revocation)")]
    LongLivedCredential,
    #[error("token carries no account_id and cannot be attributed")]
    MissingAccountId,
    #[error("token does not contain a bounded interactive user ownership context")]
    InvalidOwnership,
    #[error("token does not carry the required scope {required:?}")]
    MissingScope { required: String },
    /// Minted for a different Cognitum product. `client_id` is the platform's
    /// audience mechanism in the absence of `aud`.
    #[error("token was issued to client {found:?}, which this server does not accept")]
    WrongAudience { found: String },
}

/// Identity's access-token claims. Mirrors `AccessTokenClaims` in
/// `dashboard/services/identity/src/jwt.rs`.
///
/// `typ` is `Option` because identity types it that way — absence must be
/// treated as "not an access token", never as a default.
#[derive(Debug, Deserialize)]
struct AccessTokenClaims {
    #[serde(default)]
    typ: Option<String>,
    sub: String,
    #[serde(default)]
    account_id: Option<String>,
    #[serde(default)]
    org_id: String,
    #[serde(default)]
    workspace_id: String,
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    jti: String,
    exp: i64,
    #[serde(default)]
    iat: i64,
    #[serde(default)]
    iss: Option<String>,
    #[serde(default)]
    aud: Option<String>,
    /// Long-lived, non-rotating setup credential. Absent on older tokens.
    #[serde(default)]
    setup: bool,
    /// Machine workload credential. Absent on older tokens.
    #[serde(default)]
    workload: bool,
}

/// Verifier configuration.
pub struct VerifierConfig {
    /// The authorization server's origin, e.g. `https://auth.cognitum.one`.
    ///
    /// **Not validated against a claim** — Cognitum access tokens carry no
    /// `iss` (see module docs); the JWKS provides the issuer binding. This is
    /// here for logging and for deriving the default `jwks_uri`, so that the
    /// configured issuer and the keys we trust cannot drift apart silently.
    pub issuer: String,
    /// The scope a caller must hold for the route being served.
    pub required_scope: String,
    /// `client_id` values whose tokens this server accepts — the AUDIENCE check.
    ///
    /// Cognitum tokens carry no `aud`; the platform uses `client_id` for this
    /// instead. `freetokens` (cognitum-one/freetokens, live) states the contract
    /// plainly — *"Cognitum access tokens intentionally use custom `client_id`
    /// rather than a registered JWT `aud` claim"* — and enforces
    /// `payload.client_id !== OAUTH_CLIENT_ID` on every request.
    ///
    /// Empty means accept any client, which is what an earlier revision did on
    /// the reasoning that clients borrow each other's registrations (musica
    /// shipped as `meta-proxy` while its own was pending). That was a
    /// transitional state, not the model: RuView has its own registered client,
    /// so leaving this empty means accepting a token minted for ANY Cognitum
    /// product. Configure it.
    pub allowed_client_ids: Vec<String>,
}

/// Verify a raw JWT and produce a [`Principal`].
///
/// Every rejection path returns a typed error; none of them return a partially
/// trusted principal.
fn verified_interactive_claims(
    token: &str,
    jwks: &JwksCache,
    config: &VerifierConfig,
) -> Result<AccessTokenClaims, VerifyError> {
    let header = decode_header(token).map_err(|e| VerifyError::Malformed(e.to_string()))?;

    // Compared, never selected. `decode` below independently enforces the same
    // allowlist; this early check exists so the failure is legible.
    if header.alg != Algorithm::ES256 {
        return Err(VerifyError::WrongAlgorithm);
    }
    let kid = header.kid.ok_or(JwksError::MissingKid)?;
    let key = jwks.decoding_key_for(&kid)?;

    let mut validation = Validation::new(Algorithm::ES256);
    validation.leeway = CLOCK_LEEWAY_SECS;
    validation.validate_exp = true;
    // No `aud` and no `iss` validation — Cognitum access tokens carry neither.
    // See "What binds a token to its issuer" in the module docs: the JWKS is
    // the binding, and requiring a claim identity does not emit rejects every
    // real token. meta-llm's verifier makes the same two omissions.
    validation.validate_aud = false;
    validation.set_required_spec_claims(&["exp"]);

    let data = decode::<AccessTokenClaims>(token, &key, &validation).map_err(map_jwt_error)?;
    let claims = data.claims;

    // ---- Claim policy. Mirrors meta-llm's oauthBearer.ts accept-rule. ----

    if claims.typ.as_deref() != Some(TYP_ACCESS) {
        return Err(VerifyError::WrongTokenType { found: claims.typ });
    }
    // AUDIENCE. Cognitum's stand-in for `aud` (see VerifierConfig docs).
    if !config.allowed_client_ids.is_empty()
        && !config
            .allowed_client_ids
            .iter()
            .any(|c| c == &claims.client_id)
    {
        return Err(VerifyError::WrongAudience {
            found: claims.client_id,
        });
    }
    if claims.setup || claims.workload {
        // Belt and braces alongside the `typ` check: identity stamps these as
        // booleans as well, and a credential that sets either must never be
        // honoured here regardless of how it types itself.
        return Err(VerifyError::LongLivedCredential);
    }

    Ok(claims)
}

pub fn verify_access_token(
    token: &str,
    jwks: &JwksCache,
    config: &VerifierConfig,
) -> Result<Principal, VerifyError> {
    let claims = verified_interactive_claims(token, jwks, config)?;
    let account_id = claims.account_id.filter(|a| !a.is_empty());
    let Some(account_id) = account_id else {
        // meta-llm requires this so a token cannot bill an account it doesn't
        // belong to. RuView's reason is attribution: an unattributable principal
        // cannot appear in an audit trail, which is most of the point of moving
        // off a shared static bearer.
        return Err(VerifyError::MissingAccountId);
    };

    let principal = Principal::new(
        claims.sub,
        account_id,
        claims.org_id,
        claims.workspace_id,
        claims.client_id,
        claims.jti,
        &claims.scope,
        claims.exp,
    );

    if !principal.has_scope(&config.required_scope) {
        return Err(VerifyError::MissingScope {
            required: config.required_scope.clone(),
        });
    }

    Ok(principal)
}

/// Local-data ownership is a user/tenant/workspace identity, not a billing
/// principal. This separate type cannot be used as a metering Principal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountIdentity {
    pub issuer: String,
    pub subject: String,
    pub org_id: String,
    pub workspace_id: String,
    pub expires_at: i64,
    _verified: (),
}

fn canonical_uuid(value: &str) -> Option<String> {
    (value.len() == 36 && value.bytes().enumerate().all(|(index, byte)| {
        if [8, 13, 18, 23].contains(&index) { byte == b'-' } else { byte.is_ascii_hexdigit() }
    })).then(|| value.to_ascii_lowercase())
}

/// Verify account ownership with the existing fixed ES256/JWKS chain. Unlike
/// verify_access_token, this does not require the optional Firebase billing ID.
/// Existing billing/attribution acceptance is unchanged.
pub fn verify_account_identity(
    token: &str,
    jwks: &JwksCache,
    config: &VerifierConfig,
) -> Result<AccountIdentity, VerifyError> {
    if token.len() > 16384 { return Err(VerifyError::InvalidOwnership); }
    let claims = verified_interactive_claims(token, jwks, config)?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| VerifyError::InvalidOwnership)?.as_secs() as i64;
    let issuer = config.issuer.trim_end_matches('/');
    // Older Identity tokens omit iss/aud. The configured trusted JWKS binds
    // their issuer, and exact client_id binds their audience. Present values
    // must match; no token field can choose a key URL or broaden the client.
    if claims.client_id != "ruview"
        || claims.iss.as_deref().is_some_and(|value| value != issuer)
        || claims.aud.as_deref().is_some_and(|value| value != "ruview")
        || claims.iat > now + 30 || claims.exp <= now || claims.exp <= claims.iat
        || claims.exp.checked_sub(claims.iat).is_none_or(|ttl| ttl > 3600)
    { return Err(VerifyError::InvalidOwnership); }
    let scopes: Vec<&str> = claims.scope.split_whitespace().collect();
    if !scopes.contains(&config.required_scope.as_str()) {
        return Err(VerifyError::MissingScope { required: config.required_scope.clone() });
    }
    Ok(AccountIdentity {
        issuer: issuer.into(),
        subject: canonical_uuid(&claims.sub).ok_or(VerifyError::InvalidOwnership)?,
        org_id: canonical_uuid(&claims.org_id).ok_or(VerifyError::InvalidOwnership)?,
        workspace_id: canonical_uuid(&claims.workspace_id).ok_or(VerifyError::InvalidOwnership)?,
        expires_at: claims.exp,
        _verified: (),
    })
}

/// Extract a bearer token from an `Authorization` header value.
///
/// The scheme is matched **case-insensitively** per RFC 7235 §2.1, and leading
/// whitespace before the token is tolerated. This mirrors what
/// `wifi-densepose-sensing-server`'s existing `bearer_auth` already does
/// deliberately, so a client sending `bearer`/`BEARER` is not rejected by one
/// layer and accepted by the other. The token itself is never normalised.
pub fn extract_bearer(header_value: &str) -> Result<&str, VerifyError> {
    let (scheme, token) = header_value
        .split_once(' ')
        .ok_or(VerifyError::MissingBearer)?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return Err(VerifyError::MissingBearer);
    }
    let token = token.trim();
    if token.is_empty() {
        return Err(VerifyError::MissingBearer);
    }
    Ok(token)
}

fn map_jwt_error(e: jsonwebtoken::errors::Error) -> VerifyError {
    use jsonwebtoken::errors::ErrorKind;
    match e.kind() {
        ErrorKind::InvalidSignature => VerifyError::BadSignature,
        ErrorKind::ExpiredSignature | ErrorKind::ImmatureSignature => {
            VerifyError::ExpiredOrNotYetValid
        }
        ErrorKind::InvalidAlgorithm | ErrorKind::InvalidAlgorithmName => {
            VerifyError::WrongAlgorithm
        }
        _ => VerifyError::Malformed(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_bearer_accepts_a_well_formed_header() {
        assert_eq!(extract_bearer("Bearer abc.def.ghi").unwrap(), "abc.def.ghi");
    }

    #[test]
    fn extract_bearer_rejects_a_missing_prefix() {
        assert!(matches!(
            extract_bearer("abc.def.ghi"),
            Err(VerifyError::MissingBearer)
        ));
    }

    #[test]
    fn extract_bearer_accepts_any_scheme_casing() {
        // RFC 7235 §2.1: the auth-scheme is case-insensitive. The sensing
        // server's own middleware already matches it that way on purpose, and
        // the two layers must not disagree about what a valid header looks like.
        for header in ["Bearer t.o.k", "bearer t.o.k", "BEARER t.o.k"] {
            assert_eq!(extract_bearer(header).unwrap(), "t.o.k", "for {header:?}");
        }
    }

    #[test]
    fn extract_bearer_tolerates_extra_space_before_the_token() {
        assert_eq!(extract_bearer("Bearer   t.o.k").unwrap(), "t.o.k");
    }

    #[test]
    fn extract_bearer_rejects_a_different_scheme() {
        assert!(matches!(
            extract_bearer("Basic dXNlcjpwYXNz"),
            Err(VerifyError::MissingBearer)
        ));
    }

    #[test]
    fn extract_bearer_rejects_an_empty_token() {
        assert!(matches!(
            extract_bearer("Bearer   "),
            Err(VerifyError::MissingBearer)
        ));
    }
}
