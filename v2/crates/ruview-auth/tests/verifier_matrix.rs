//! The verifier accept/reject matrix — gates G-1 and G-2 of the ADR-271 plan.
//!
//! Every token here is a **real ES256 JWT signed at test time** with a
//! freshly generated key, so these exercise the same code path production does
//! rather than asserting against hand-built strings. No network: the JWKS is
//! served from a stub.
//!
//! Keypairs are **generated at test runtime, never committed**. A checked-in
//! `-----BEGIN PRIVATE KEY-----` would be inert here, but it trains scanners and
//! readers to treat committed key material as normal, and this repo has no such
//! precedent. Generating also means no fixture can drift out of sync with the
//! JWKS document it is served by — the two are derived from the same key.

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{encode, EncodingKey, Header};
use ring::{
    rand::SystemRandom,
    signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING},
};
use ruview_auth::{
    jwks::{JwksError, JwksFetcher},
    scope, verify_access_token, JwksCache, VerifierConfig, VerifyError,
};
use serde_json::json;

const TEST_KID: &str = "test-key-1";
const TEST_ISSUER: &str = "https://auth.test.local";

/// A generated P-256 keypair: the PKCS#8 DER to sign with, and the JWK
/// coordinates to serve in the stub JWKS.
struct TestKey {
    pkcs8_der: Vec<u8>,
    x: String,
    y: String,
}

fn generate_key() -> TestKey {
    let rng = SystemRandom::new();
    let der = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng)
        .expect("P-256 key generation");
    let signing = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, der.as_ref(), &rng)
        .expect("generated key parses");
    let point = signing.public_key().as_ref();
    assert_eq!(point.len(), 65);
    assert_eq!(point[0], 4); // Uncompressed SEC1 P-256 public key.
    TestKey {
        pkcs8_der: der.as_ref().to_vec(),
        x: URL_SAFE_NO_PAD.encode(&point[1..33]),
        y: URL_SAFE_NO_PAD.encode(&point[33..65]),
    }
}

/// The key the stub JWKS publishes — i.e. "identity's signing key".
fn primary_key() -> &'static TestKey {
    static K: OnceLock<TestKey> = OnceLock::new();
    K.get_or_init(generate_key)
}

/// A *different* valid P-256 key, published nowhere — for the forged-signature
/// case. Distinct from a malformed token: this is a real, well-formed ES256
/// signature that simply is not identity's.
fn other_key() -> &'static TestKey {
    static K: OnceLock<TestKey> = OnceLock::new();
    K.get_or_init(generate_key)
}

/// `alg: none`, precomputed (jsonwebtoken will not encode one, which is itself
/// reassuring). Claims are otherwise entirely valid.
const ALG_NONE_TOKEN: &str = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIiwia2lkIjoidGVzdC1rZXktMSJ9.eyJ0eXAiOiJhY2Nlc3MiLCJzdWIiOiJzIiwiYWNjb3VudF9pZCI6ImEiLCJvcmdfaWQiOiJvIiwid29ya3NwYWNlX2lkIjoidyIsImNsaWVudF9pZCI6InJ1dmlldyIsInNjb3BlIjoic2Vuc2luZzpyZWFkIiwianRpIjoiaiIsImlhdCI6NDEwMjQ0NDgwMCwiZXhwIjo0MTAyNDQ4NDAwLCJzZXR1cCI6ZmFsc2UsIndvcmtsb2FkIjpmYWxzZSwiaXNzIjoiaHR0cHM6Ly9hdXRoLnRlc3QubG9jYWwifQ.";

struct StaticJwks(String);

impl JwksFetcher for StaticJwks {
    fn fetch(&self, _url: &str) -> Result<String, JwksError> {
        Ok(self.0.clone())
    }
}

fn jwks_serving_test_key() -> JwksCache {
    let key = primary_key();
    let doc = json!({
        "keys": [{
            "alg": "ES256", "crv": "P-256", "kty": "EC", "use": "sig",
            "kid": TEST_KID, "x": key.x, "y": key.y
        }]
    })
    .to_string();
    JwksCache::new("https://stub/jwks.json", Box::new(StaticJwks(doc)))
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// A claim set matching identity's real `AccessTokenClaims`, valid unless a
/// test overrides a field.
fn valid_claims() -> serde_json::Value {
    json!({
        "typ": "access",
        "sub": "0f8fad5b-d9cb-469f-a165-70867728950e",
        "account_id": "firebase-uid-abc123",
        "org_id": "org-1",
        "workspace_id": "ws-1",
        "client_id": "ruview",
        "scope": "sensing:read",
        "family_id": "fam-1",
        "jti": "jti-1",
        "iat": now() - 10,
        "exp": now() + 900,          // identity's real 15-minute TTL
        "setup": false,
        "workload": false,
        // NOTE: no `iss`. Real Cognitum access tokens carry none — verified
        // against production. An earlier fixture added one, the verifier was
        // built to require it, and the whole suite passed while rejecting every
        // genuine token. Fixtures mirror production or they prove nothing.
    })
}

fn sign(claims: &serde_json::Value) -> String {
    sign_with(claims, primary_key())
}

fn sign_with(claims: &serde_json::Value, key: &TestKey) -> String {
    let mut header = Header::new(jsonwebtoken::Algorithm::ES256);
    header.kid = Some(TEST_KID.to_string());
    let enc = EncodingKey::from_ec_der(&key.pkcs8_der);
    encode(&header, claims, &enc).expect("signs")
}

fn config_for(required_scope: &str) -> VerifierConfig {
    VerifierConfig {
        issuer: TEST_ISSUER.to_string(),
        required_scope: required_scope.to_string(),
        // Mirrors production: RuView accepts only tokens minted for itself.
        allowed_client_ids: vec!["ruview".to_string()],
    }
}

fn verify(token: &str, required_scope: &str) -> Result<ruview_auth::Principal, VerifyError> {
    verify_access_token(token, &jwks_serving_test_key(), &config_for(required_scope))
}

// ─────────────────────────── accept ───────────────────────────

#[test]
fn a_valid_access_token_is_accepted_and_fully_attributed() {
    let principal = verify(&sign(&valid_claims()), scope::SENSING_READ).expect("accepted");

    assert_eq!(principal.subject, "0f8fad5b-d9cb-469f-a165-70867728950e");
    assert_eq!(principal.account_id, "firebase-uid-abc123");
    assert_eq!(principal.org_id, "org-1");
    assert_eq!(principal.client_id, "ruview");
    assert_eq!(principal.token_id, "jti-1");
    assert!(principal.has_scope(scope::SENSING_READ));
}

#[test]
fn a_token_holding_both_scopes_satisfies_either_requirement() {
    let mut c = valid_claims();
    c["scope"] = json!("sensing:read sensing:admin");
    let token = sign(&c);

    assert!(verify(&token, scope::SENSING_READ).is_ok());
    assert!(verify(&token, scope::SENSING_ADMIN).is_ok());
}

// ─────────────────── signature / algorithm ────────────────────

#[test]
fn a_token_signed_by_a_different_key_is_rejected() {
    let token = sign_with(&valid_claims(), other_key());
    assert!(matches!(
        verify(&token, scope::SENSING_READ),
        Err(VerifyError::BadSignature)
    ));
}

#[test]
fn alg_none_is_rejected() {
    // The classic downgrade. It is rejected two layers deep:
    //
    //  1. `jsonwebtoken`'s `Algorithm` enum has **no `none` variant**, so the
    //     header fails to deserialize at all — `none` is unrepresentable, not
    //     merely disallowed. That is why the variant here is `Malformed` rather
    //     than `WrongAlgorithm`: we never get far enough to compare algorithms.
    //  2. Even if it parsed, `Validation::new(ES256)` would reject it, since
    //     `alg` is only ever compared against an allowlist, never used to
    //     select an algorithm.
    //
    // The assertion below pins layer 1. If a future `jsonwebtoken` ever adds a
    // `none` variant this flips to `WrongAlgorithm` and the failure is a prompt
    // to re-verify layer 2 still holds — which is exactly when we'd want to look.
    let result = verify(ALG_NONE_TOKEN, scope::SENSING_READ);
    assert!(result.is_err(), "alg:none must never authenticate");
    assert!(
        matches!(result, Err(VerifyError::Malformed(_))),
        "expected rejection at header parse; got {result:?}"
    );
}

#[test]
fn a_tampered_payload_invalidates_the_signature() {
    let token = sign(&valid_claims());
    let mut parts: Vec<&str> = token.split('.').collect();
    // Swap the payload for one claiming admin scope, keeping the signature.
    let mut c = valid_claims();
    c["scope"] = json!("sensing:admin");
    let forged = sign(&c);
    let forged_payload = forged.split('.').nth(1).unwrap().to_string();
    parts[1] = &forged_payload;
    let spliced = parts.join(".");

    assert!(matches!(
        verify(&spliced, scope::SENSING_READ),
        Err(VerifyError::BadSignature)
    ));
}

#[test]
fn an_unknown_kid_is_rejected() {
    let mut header = Header::new(jsonwebtoken::Algorithm::ES256);
    header.kid = Some("a-kid-we-have-never-seen".to_string());
    let key = EncodingKey::from_ec_der(&primary_key().pkcs8_der);
    let token = encode(&header, &valid_claims(), &key).unwrap();

    assert!(matches!(
        verify(&token, scope::SENSING_READ),
        Err(VerifyError::Jwks(JwksError::UnknownKid(_)))
    ));
}

// ───────────────────────── time ──────────────────────────────

#[test]
fn an_expired_token_is_rejected_distinguishably() {
    let mut c = valid_claims();
    c["iat"] = json!(now() - 2000);
    c["exp"] = json!(now() - 1000);

    // Distinct from BadSignature on purpose: on an RTC-less Pi this is usually
    // a clock-sync fault, and an operator must be able to tell them apart.
    assert!(matches!(
        verify(&sign(&c), scope::SENSING_READ),
        Err(VerifyError::ExpiredOrNotYetValid)
    ));
}

#[test]
fn a_token_expiring_just_inside_the_leeway_is_still_accepted() {
    let mut c = valid_claims();
    c["exp"] = json!(now() - 5); // within the 30s leeway
    assert!(verify(&sign(&c), scope::SENSING_READ).is_ok());
}

#[test]
fn a_token_expired_beyond_the_leeway_is_rejected() {
    let mut c = valid_claims();
    c["exp"] = json!(now() - 120);
    assert!(matches!(
        verify(&sign(&c), scope::SENSING_READ),
        Err(VerifyError::ExpiredOrNotYetValid)
    ));
}

// ────────────────────── issuer / type ────────────────────────

#[test]
fn a_token_with_no_iss_claim_is_accepted_because_cognitum_issues_none() {
    // THE regression test for this module's worst bug to date.
    //
    // An earlier revision required and validated `iss`. Cognitum access tokens
    // have no `iss` claim, so that rejected every real token — while the suite
    // stayed green, because the fixtures had an `iss` the real thing lacks.
    // `valid_claims()` now mirrors production, so this passing means the
    // verifier accepts the shape that actually exists.
    let claims = valid_claims();
    assert!(
        claims.get("iss").is_none(),
        "fixture must mirror production, which emits no iss"
    );
    verify(&sign(&claims), scope::SENSING_READ).expect("a real-shaped token must verify");
}

#[test]
fn an_unrelated_iss_claim_does_not_change_the_outcome() {
    // If identity ever starts emitting `iss`, we neither require nor reject on
    // it — the JWKS is the issuer binding. This pins that adding the claim
    // cannot silently start failing tokens.
    let mut c = valid_claims();
    c["iss"] = json!("https://something.else.example");
    verify(&sign(&c), scope::SENSING_READ)
        .expect("issuer binding is the JWKS, not a claim");
}

#[test]
fn the_jwks_is_what_actually_binds_a_token_to_its_issuer() {
    // The complement of the two above: with no `iss` check, the signature is
    // the ONLY thing standing between us and a forged token. A different key
    // must therefore be refused — otherwise removing the issuer check would
    // have removed the boundary entirely.
    let token = sign_with(&valid_claims(), other_key());
    assert!(matches!(
        verify(&token, scope::SENSING_READ),
        Err(VerifyError::BadSignature)
    ));
}

#[test]
fn an_inference_typed_token_is_not_an_access_token() {
    let mut c = valid_claims();
    c["typ"] = json!("inference");
    assert!(matches!(
        verify(&sign(&c), scope::SENSING_READ),
        Err(VerifyError::WrongTokenType { .. })
    ));
}

#[test]
fn a_token_with_no_typ_claim_is_rejected() {
    let mut c = valid_claims();
    c.as_object_mut().unwrap().remove("typ");
    // Absence must never be read as a default.
    assert!(matches!(
        verify(&sign(&c), scope::SENSING_READ),
        Err(VerifyError::WrongTokenType { found: None })
    ));
}

// ──────────── long-lived credentials (unverifiable revocation) ────────────

#[test]
fn a_setup_token_is_refused_even_when_typed_as_access() {
    // A 365-day credential whose revocation lives in a table RuView cannot read.
    let mut c = valid_claims();
    c["setup"] = json!(true);
    assert!(matches!(
        verify(&sign(&c), scope::SENSING_READ),
        Err(VerifyError::LongLivedCredential)
    ));
}

#[test]
fn a_workload_token_is_refused_even_when_typed_as_access() {
    let mut c = valid_claims();
    c["workload"] = json!(true);
    assert!(matches!(
        verify(&sign(&c), scope::SENSING_READ),
        Err(VerifyError::LongLivedCredential)
    ));
}

// ───────────────────── attribution ───────────────────────────

#[test]
fn a_token_without_account_id_cannot_be_attributed() {
    let mut c = valid_claims();
    c.as_object_mut().unwrap().remove("account_id");
    assert!(matches!(
        verify(&sign(&c), scope::SENSING_READ),
        Err(VerifyError::MissingAccountId)
    ));
}

#[test]
fn an_empty_account_id_is_treated_as_absent() {
    let mut c = valid_claims();
    c["account_id"] = json!("");
    assert!(matches!(
        verify(&sign(&c), scope::SENSING_READ),
        Err(VerifyError::MissingAccountId)
    ));
}

// ───────────── G-2: scope is the capability boundary ─────────────

#[test]
fn g2_a_genuinely_valid_token_from_another_cognitum_product_cannot_reach_the_sensing_surface() {
    // THE highest-value test in this suite.
    //
    // Correctly signed, unexpired, right issuer, right `typ` — a real token a
    // user legitimately holds for meta-proxy/completions. Cognitum access tokens
    // carry no `aud`, and cross-product identity is intended, so NOTHING about
    // the signature or the identity claims distinguishes it. Only scope does.
    //
    // A naive verifier accepts this. If this test ever passes-by-accepting,
    // an `inference` token has become a key to someone's home sensor.
    let mut c = valid_claims();
    c["client_id"] = json!("meta-proxy");
    c["scope"] = json!("inference");

    // Rejected on AUDIENCE now (client_id), which is the stronger of the two
    // reasons — it fires before scope is even considered.
    assert!(matches!(
        verify(&sign(&c), scope::SENSING_READ),
        Err(VerifyError::WrongAudience { .. })
    ));
}

#[test]
fn a_token_minted_for_another_cognitum_product_is_refused_even_with_the_right_scope() {
    // The audience check standing alone. Same user, same signature, correct
    // sensing:read scope — but minted for freetokens, so not for this server.
    // `cognitum-one/freetokens` enforces the mirror image of this.
    let mut c = valid_claims();
    c["client_id"] = json!("freetokens");
    assert!(matches!(
        verify(&sign(&c), scope::SENSING_READ),
        Err(VerifyError::WrongAudience { .. })
    ));
}

#[test]
fn an_empty_audience_list_accepts_any_client() {
    // The documented opt-out (RUVIEW_OAUTH_CLIENT_IDS=*). Pinned so the
    // behaviour is deliberate rather than accidental.
    let mut c = valid_claims();
    c["client_id"] = json!("some-other-product");
    let cfg = VerifierConfig {
        issuer: TEST_ISSUER.to_string(),
        required_scope: scope::SENSING_READ.to_string(),
        allowed_client_ids: vec![],
    };
    verify_access_token(&sign(&c), &jwks_serving_test_key(), &cfg)
        .expect("an empty allowlist means accept any client");
}

#[test]
fn multiple_allowed_clients_are_honoured() {
    // Migration case: accepting a borrowed registration alongside our own.
    let mut c = valid_claims();
    c["client_id"] = json!("meta-proxy");
    let cfg = VerifierConfig {
        issuer: TEST_ISSUER.to_string(),
        required_scope: scope::SENSING_READ.to_string(),
        allowed_client_ids: vec!["ruview".into(), "meta-proxy".into()],
    };
    verify_access_token(&sign(&c), &jwks_serving_test_key(), &cfg).expect("both accepted");
}

#[test]
fn g2_a_read_scoped_session_cannot_reach_the_admin_surface() {
    // The routine case the least-scope rule exists for: a dashboard streaming
    // poses must not be able to delete the model it streams through.
    let token = sign(&valid_claims()); // scope: sensing:read
    assert!(matches!(
        verify(&token, scope::SENSING_ADMIN),
        Err(VerifyError::MissingScope { .. })
    ));
}

#[test]
fn g2_an_admin_scoped_token_does_not_implicitly_grant_read() {
    // No hierarchy: consent means exactly what it said.
    let mut c = valid_claims();
    c["scope"] = json!("sensing:admin");
    assert!(matches!(
        verify(&sign(&c), scope::SENSING_READ),
        Err(VerifyError::MissingScope { .. })
    ));
}

#[test]
fn a_token_with_no_scope_at_all_grants_nothing() {
    let mut c = valid_claims();
    c["scope"] = json!("");
    assert!(matches!(
        verify(&sign(&c), scope::SENSING_READ),
        Err(VerifyError::MissingScope { .. })
    ));
}
