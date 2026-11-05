use super::*;

#[tokio::test]
async fn selection_mutations_are_local_owner_only_even_without_outer_bearer_gate() {
    let (_dir, state) = fixture();
    let routes = owner_selection_routes(
        wifi_densepose_sensing_server::bearer_auth::AuthState::from_token("synthetic-owner"),
    )
    .with_state(state.clone());
    for path in [
        "/api/v1/room-learner/recovery/enable-current",
        "/api/v1/room-learner/qualify",
        "/api/v1/room-learner/select",
        "/api/v1/room-learner/deselect",
    ] {
        for (peer, token) in [
            ("127.0.0.1:41000", None),
            ("127.0.0.1:41000", Some("synthetic-peer-read")),
            ("192.0.2.1:41000", Some("synthetic-owner")),
        ] {
            let mut request = Request::builder()
                .method(Method::POST)
                .uri(path)
                .header("content-type", "application/json");
            if let Some(token) = token {
                request = request.header("authorization", format!("Bearer {token}"));
            }
            let mut request = request.body(Body::from("{}")).unwrap();
            request.extensions_mut().insert(axum::extract::ConnectInfo(
                peer.parse::<SocketAddr>().unwrap(),
            ));
            let response = routes.clone().oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::FORBIDDEN,
                "{path} {peer} {token:?}"
            );
        }
    }
    // A legitimate owner reaches the typed handler; no geometry is published
    // and no data is erased by this negative missing-context operation.
    let expected = state
        .read()
        .await
        .room_learning
        .as_ref()
        .unwrap()
        .mutation_guard()
        .unwrap();
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/room-learner/qualify")
        .header("content-type", "application/json")
        .header("authorization", "Bearer synthetic-owner")
        .body(Body::from(
            serde_json::to_vec(&serde_json::json!({"expected":expected})).unwrap(),
        ))
        .unwrap();
    request.extensions_mut().insert(axum::extract::ConnectInfo(
        "127.0.0.1:41000".parse::<SocketAddr>().unwrap(),
    ));
    let response = routes.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["success"], false);
    assert_eq!(body["error_code"], "learner_qualification_rejected");
}

#[tokio::test]
async fn selection_readback_does_not_expose_owner_or_mutation_authority() {
    let (_dir, state) = fixture();
    let response = Router::new()
        .route("/api/v1/room-learner/selection", get(selection))
        .with_state(state)
        .oneshot(
            Request::builder()
                .uri("/api/v1/room-learner/selection")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["schema"], "ruview.room-learner.selection.v1");
    assert_eq!(body["model_selection"]["current"]["status"], "none");
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    for forbidden in [
        "owner_scope",
        "mutation_guard",
        "encoder_weights",
        "training_samples",
        "head_weights",
        "runtime_epoch",
    ] {
        assert!(
            !text.contains(forbidden),
            "read-scoped selection leaked {forbidden}"
        );
    }
}
