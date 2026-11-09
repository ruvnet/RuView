//! Actual listener composition and policy; synthetic ports/identities only.
use super::*;
use axum::{
    body::Body,
    http::{Method, Request},
};
use serde_json::{json, Value};
use tower::ServiceExt;
const PATH: &str = "/api/v1/companion/room/node-labels";
async fn call(
    router: Router,
    method: Method,
    path: &str,
    body: Value,
    token: &str,
    installation: &str,
    local: bool,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .header("x-ruview-installation-id", installation)
        .body(Body::from(body.to_string()))
        .unwrap();
    if local {
        request
            .extensions_mut()
            .insert(ConnectInfo("127.0.0.1:1".parse::<SocketAddr>().unwrap()));
    }
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn paired(service: Arc<Companion>) -> String {
    let owner = companion::owner_router(service.clone());
    let public = companion::public_router(service);
    let (_, invite) = call(
        owner.clone(),
        Method::POST,
        "/api/v1/companion/invites",
        json!({}),
        "",
        "",
        true,
    )
    .await;
    let proof = companion::secret();
    let (s,enrolled)=call(public.clone(),Method::POST,"/companion/v1/enroll",json!({"installation_id":"installation-1","invite_id":invite["invite_id"],"invite_secret":invite["invite_secret"],"device_id":"phone","device_name":"Synthetic phone","pairing_secret":proof}),"","",true).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        owner,
        Method::POST,
        &format!(
            "/api/v1/companion/requests/{}/approve",
            enrolled["request_id"].as_str().unwrap()
        ),
        json!({"comparison_code":enrolled["comparison_code"]}),
        "",
        "",
        true,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, session) = call(
        public,
        Method::POST,
        "/companion/v1/session",
        json!({"installation_id":"installation-1","device_id":"phone","pairing_secret":proof}),
        "",
        "",
        true,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    session["access_token"].as_str().unwrap().into()
}
async fn fixture() -> (tempfile::TempDir, Integration, Router, Router, String) {
    let dir = tempfile::tempdir().unwrap();
    let identity =
        companion_tls::Identity::load_or_create(dir.path(), Some("installation-1")).unwrap();
    let service = Companion::load_for_account(
        dir.path(),
        "installation-1".into(),
        identity.cert_sha256,
        "https://synthetic.local:3443".into(),
        Some("a".repeat(64)),
    )
    .unwrap();
    let token = paired(service.clone()).await;
    let room = companion_room::SharedRoomStore::load_for_account(
        dir.path(),
        "installation-1",
        Some(&"a".repeat(64)),
    )
    .unwrap();
    let state = Arc::new(RwLock::new(AppStateInner::minimal()));
    let control = Control {
        state,
        serial: Arc::new(tokio::sync::Mutex::new(())),
    };
    let integration = Integration {
        service: service.clone(),
        authority: "synthetic.local:3443".into(),
        auth: AuthState::from_token("synthetic-owner"),
        listener: tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(),
        tls: identity.config,
        room,
        control,
        sensors: companion_sensors::SensorStore::new(service),
        spatial: None,
        advertiser: None,
    };
    // Same canonical production peer builder and clone-before-owner merge order.
    let shared = Router::new().route("/health", axum::routing::get(|| async { StatusCode::OK }));
    let peer = integration.peer_router(shared.clone());
    let owner = shared.merge(integration.owner_router());
    (dir, integration, owner, peer, token)
}
fn room() -> Value {
    json!({"schema":"ruview.companion.room.v1","id":"room-1","name":"Room","widthM":5,"lengthM":6,"heightM":2.6,"nodes":[{"nodeId":1,"slot":"north_west","xM":0.45,"yM":0.45,"role":"receiver","headingDeg":90,"orientationConfirmed":true,"confirmedAtUnixMs":1}],"accessPoint":{"slot":"north_east","xM":4.55,"yM":0.45,"provenance":"operator_entered","confirmedAtUnixMs":1}})
}
#[tokio::test]
async fn node_labels_actual_router_owner_peer_cas_privacy_and_legacy_bytes() {
    let (dir, integration, owner, peer, token) = fixture().await;
    assert_eq!(policy(&Method::GET, PATH), Some(false));
    assert_eq!(policy(&Method::HEAD, PATH), Some(false));
    for method in [Method::PUT, Method::POST, Method::DELETE, Method::PATCH] {
        assert_eq!(policy(&method, PATH), None);
        assert_eq!(
            call(
                peer.clone(),
                method,
                PATH,
                json!({}),
                &token,
                "installation-1",
                false
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    let (s, empty) = call(
        peer.clone(),
        Method::GET,
        PATH,
        Value::Null,
        &token,
        "installation-1",
        false,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(empty["status"], "room_required");
    assert!(empty["publication"].is_null());
    let (s, published) = call(
        owner.clone(),
        Method::PUT,
        "/api/v1/companion/room",
        json!({"expected_revision":0,"room":room()}),
        "synthetic-owner",
        "",
        true,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let roombytes = std::fs::read(dir.path().join("companion-room.json")).unwrap();
    let (_, unreported) = call(
        peer.clone(),
        Method::GET,
        PATH,
        Value::Null,
        &token,
        "installation-1",
        false,
    )
    .await;
    assert_eq!(unreported["status"], "not_reported");
    assert_eq!(unreported["metadata_revision"], 0);
    let request = json!({"schema":"ruview.companion.room-node-labels-update.v1","installation_id":"installation-1","expected_publication":unreported["publication"],"expected_metadata_revision":0,"labels":[{"node_id":1,"display_name":"West transmitter"}]});
    for (credential, local) in [
        ("synthetic-owner", false),
        (token.as_str(), true),
        ("wrong", true),
    ] {
        assert_eq!(
            call(
                owner.clone(),
                Method::PUT,
                PATH,
                request.clone(),
                credential,
                "",
                local
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    let mut unknown = request.clone();
    unknown["model"] = json!({"qualified":true});
    assert_eq!(
        call(
            owner.clone(),
            Method::PUT,
            PATH,
            unknown,
            "synthetic-owner",
            "",
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut oversized = request.clone();
    oversized["labels"][0]["display_name"] = json!("x".repeat(9000));
    assert_eq!(
        call(
            owner.clone(),
            Method::PUT,
            PATH,
            oversized,
            "synthetic-owner",
            "",
            true
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let (s, ack) = call(
        owner.clone(),
        Method::PUT,
        PATH,
        request.clone(),
        "synthetic-owner",
        "",
        true,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(ack["effect"], "updated");
    let (s, current) = call(
        peer.clone(),
        Method::GET,
        PATH,
        Value::Null,
        &token,
        "installation-1",
        false,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(current, ack["snapshot"]);
    assert_eq!(
        current["publication"]["binding_digest"],
        published["binding_digest"]
    );
    assert_eq!(
        std::fs::read(dir.path().join("companion-room.json")).unwrap(),
        roombytes
    );
    assert_eq!(
        call(
            owner.clone(),
            Method::PUT,
            PATH,
            request,
            "synthetic-owner",
            "",
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    for (install, tok) in [("foreign", token.as_str()), ("installation-1", "bad")] {
        assert_eq!(
            call(
                peer.clone(),
                Method::GET,
                PATH,
                Value::Null,
                tok,
                install,
                false
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        call(
            peer.clone(),
            Method::GET,
            &format!("{PATH}?token=bad"),
            Value::Null,
            &token,
            "installation-1",
            false
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    integration.control.state.write().await.privacy_mode = true;
    let (s, denied) = call(
        peer.clone(),
        Method::GET,
        PATH,
        Value::Null,
        &token,
        "installation-1",
        false,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(
        denied,
        json!({"schema":"ruview.companion.room-node-labels.v1","success":false,"error_code":"forbidden"})
    );
    assert!(!denied.to_string().contains("transmitter"));
    integration.control.state.write().await.privacy_mode = false;
    let (s, _) = call(
        companion::owner_router(integration.service.clone()),
        Method::DELETE,
        "/api/v1/companion/devices/phone",
        Value::Null,
        "",
        "",
        true,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        call(
            peer,
            Method::GET,
            PATH,
            Value::Null,
            &token,
            "installation-1",
            false
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}
#[tokio::test]
async fn node_labels_actual_producer_matches_agreed_cross_language_fixture() {
    let (dir, _integration, owner, peer, token) = fixture().await;
    let agreed: Value =
        serde_json::from_str(include_str!("companion_room/node_labels_wire.json")).unwrap();
    let expected = &agreed["cases"];
    let (_, required) = call(
        peer.clone(),
        Method::GET,
        PATH,
        Value::Null,
        &token,
        "installation-1",
        false,
    )
    .await;
    assert_eq!(required, expected["room_required"]);
    assert_eq!(
        call(
            owner.clone(),
            Method::PUT,
            "/api/v1/companion/room",
            json!({"expected_revision":0,"room":room()}),
            "synthetic-owner",
            "",
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, unreported) = call(
        peer.clone(),
        Method::GET,
        PATH,
        Value::Null,
        &token,
        "installation-1",
        false,
    )
    .await;
    assert_eq!(unreported, expected["not_reported"]);
    let mut request = expected["update"].clone();
    request["expected_metadata_revision"] = json!(0);
    request["labels"][0]["display_name"] = json!("First name");
    assert_eq!(
        call(
            owner.clone(),
            Method::PUT,
            PATH,
            request,
            "synthetic-owner",
            "",
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    let (s, ack) = call(
        owner.clone(),
        Method::PUT,
        PATH,
        expected["update"].clone(),
        "synthetic-owner",
        "",
        true,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(ack, expected["updated"]);
    let (_, current) = call(
        peer,
        Method::GET,
        PATH,
        Value::Null,
        &token,
        "installation-1",
        false,
    )
    .await;
    assert_eq!(current, expected["current"]);
    if let Ok(path) = std::env::var("RUVIEW_TEST_NODE_LABELS_OUTPUT") {
        let path = std::path::PathBuf::from(path);
        assert!(
            path.is_absolute()
                && path
                    .parent()
                    .unwrap()
                    .canonicalize()
                    .unwrap()
                    .starts_with(std::env::temp_dir().canonicalize().unwrap())
        );
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path).unwrap();
        file.write_all(serde_json::to_string_pretty(&json!({"evidence":"SYNTHETIC_ACTUAL_RUST_ROUTER_PRODUCER","current":current,"not_reported":unreported,"room_required":required,"updated":ack})).unwrap().as_bytes()).unwrap();
    }
    assert!(
        dir.path()
            .join("companion-room-node-labels-v1.json")
            .metadata()
            .unwrap()
            .len()
            < 8192
    );
}
