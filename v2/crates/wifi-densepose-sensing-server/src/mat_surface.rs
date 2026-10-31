//! Host the existing MAT router without creating demo incidents or detections.
//! Incident state is in-memory; CSI ingest must be configured explicitly.

/// Attach before host, bearer, companion and privacy middleware are applied.
pub fn router(privacy_mode: bool) -> axum::Router {
    router_with_state(wifi_densepose_mat::api::AppState::new(), privacy_mode)
}

fn router_with_state(state: wifi_densepose_mat::api::AppState, privacy_mode: bool) -> axum::Router {
    let router = wifi_densepose_mat::api::create_router(state);
    if privacy_mode {
        router.layer(axum::Extension(
            wifi_densepose_mat::api::websocket::StreamOutputFilter(
                crate::privacy_filter::redact_json_str,
            ),
        ))
    } else {
        router
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn real_mat_router_starts_empty_and_reports_optional_ml_truthfully() {
        let app = super::router(false);
        for path in [
            "/api/v1/mat/events?page=0&page_size=100",
            "/api/v1/mat/scan/status",
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            let body = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
            let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            if path.contains("events") {
                assert_eq!(value["events"].as_array().unwrap().len(), 0);
                assert_eq!(value["total"], 0);
            } else {
                assert_eq!(value["scanning"], false);
                assert_eq!(value["ml_enabled"], false);
                assert_eq!(value["ml_ready"], false);
            }
        }
    }

    #[tokio::test]
    async fn mat_reads_mutations_and_stream_remain_behind_host_auth_layers() {
        let app = super::router(false).layer(axum::middleware::from_fn_with_state(
            crate::bearer_auth::AuthState::from_token("mat-test-fixture-token"),
            crate::bearer_auth::require_bearer,
        ));
        for (method, path) in [
            ("GET", "/api/v1/mat/events"),
            ("POST", "/api/v1/mat/scan/control"),
            ("GET", "/ws/mat/stream"),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{method} {path}"
            );
        }
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/mat/scan/status")
                    .header("authorization", "Bearer mat-test-fixture-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn real_incident_create_and_read_use_only_explicit_test_state() {
        let app = super::router(false);
        let body = r#"{"event_type":"Earthquake","latitude":0,"longitude":0,"description":"EXPLICIT ISOLATED TEST FIXTURE"}"#;
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/mat/events")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/mat/events?page=0&page_size=100")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["total"], 1);
        assert_eq!(value["events"][0]["survivor_count"], 0);
        assert_eq!(
            value["events"][0]["description"],
            "EXPLICIT ISOLATED TEST FIXTURE"
        );
    }

    // SYNTHETIC values in isolated in-memory incident state. This exercises the
    // real MAT handlers, upgrade, broadcaster and host output/auth policies;
    // it makes no claim about sensor detection or the packaged app.
    #[tokio::test]
    async fn authenticated_nonempty_mat_stream_and_snapshot_obey_privacy() {
        use futures_util::StreamExt;
        use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};
        use wifi_densepose_mat::{api::dto::*, domain::*};

        for privacy in [false, true] {
            let state = wifi_densepose_mat::api::AppState::new();
            let app = super::router_with_state(state.clone(), privacy)
                .layer(axum::middleware::from_fn_with_state(
                    crate::privacy_filter::PrivacyFilter::new(privacy),
                    crate::privacy_filter::redact_json_responses,
                ))
                .layer(axum::middleware::from_fn_with_state(
                    crate::bearer_auth::AuthState::from_token("mat-test-fixture-token"),
                    crate::bearer_auth::require_bearer,
                ));
            let response = app.clone().oneshot(Request::builder()
                .method("POST").uri("/api/v1/mat/events")
                .header("authorization", "Bearer mat-test-fixture-token")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"event_type":"Earthquake","latitude":0,"longitude":0,"description":"SYNTHETIC PRIVACY REGRESSION"}"#)).unwrap()
            ).await.unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
            let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
            let event: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            let event_id = event["id"].as_str().unwrap().parse().unwrap();
            let zone = ScanZone::new("SYNTHETIC", ZoneBounds::rectangle(0., 0., 10., 10.));
            let zone_id = zone.id().clone();
            state
                .update_event(event_id, |event| {
                    event.add_zone(zone);
                    event
                        .record_detection(
                            zone_id.clone(),
                            VitalSignsReading {
                                breathing: Some(BreathingPattern {
                                    rate_bpm: 16.,
                                    amplitude: 0.8,
                                    regularity: 0.9,
                                    pattern_type: BreathingType::Normal,
                                }),
                                heartbeat: None,
                                movement: Default::default(),
                                timestamp: chrono::Utc::now(),
                                confidence: ConfidenceScore::new(0.9),
                            },
                            None,
                        )
                        .unwrap();
                })
                .unwrap();
            let domain_survivor = state.get_event(event_id).unwrap().survivors()[0].clone();
            state
                .event_store()
                .append(events::DomainEvent::Detection(
                    events::DetectionEvent::SurvivorDetected {
                        survivor_id: domain_survivor.id().clone(),
                        zone_id: zone_id.clone(),
                        vital_signs: domain_survivor.vital_signs().latest().unwrap().clone(),
                        location: None,
                        timestamp: chrono::Utc::now(),
                    },
                ))
                .unwrap();
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v1/mat/events/domain")
                        .header("authorization", "Bearer mat-test-fixture-token")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
            let domain_events: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(domain_events["total"], 1);
            assert_eq!(domain_events["events"][0]["event_type"], "SurvivorDetected");
            assert_eq!(domain_events["events"][0].get("details").is_none(), privacy);
            if !privacy {
                assert!(domain_events["events"][0]["details"]
                    .as_str()
                    .unwrap()
                    .contains("rate_bpm: 16.0"));
            }
            let alert = wifi_densepose_mat::alerting::AlertGenerator::new()
                .generate(&domain_survivor)
                .unwrap();
            assert!(alert.payload().message.contains("Breathing: 16.0 BPM"));
            state.store_alert(alert.clone(), event_id);
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/api/v1/mat/events/{event_id}/alerts"))
                        .header("authorization", "Bearer mat-test-fixture-token")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
            let alerts: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(alerts["total"], 1);
            assert_eq!(alerts["alerts"][0].get("message").is_none(), privacy);
            if !privacy {
                assert!(alerts["alerts"][0]["message"]
                    .as_str()
                    .unwrap()
                    .contains("Breathing: 16.0 BPM"));
            }
            // Non-empty REST snapshot, including an array-nested vital_signs
            // object: policy parity with the socket is the security invariant.
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/api/v1/mat/events/{event_id}/survivors"))
                        .header("authorization", "Bearer mat-test-fixture-token")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
            let snapshot: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(snapshot["total"], 1);
            assert_eq!(
                snapshot["survivors"][0].get("vital_signs").is_none(),
                privacy
            );
            if !privacy {
                assert_eq!(
                    snapshot["survivors"][0]["vital_signs"]["breathing_rate"],
                    16.
                );
            }

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let served_app = app.clone();
            let server =
                tokio::spawn(async move { axum::serve(listener, served_app).await.unwrap() });
            let url = format!("ws://{addr}/ws/mat/stream");
            match connect_async(&url).await {
                Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
                }
                other => panic!("unauthenticated MAT upgrade was not denied: {other:?}"),
            }
            let mut request = url.into_client_request().unwrap();
            request.headers_mut().insert(
                "authorization",
                "Bearer mat-test-fixture-token".parse().unwrap(),
            );
            let (mut socket, response) = connect_async(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                while state.subscriber_count() == 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            let survivor = SurvivorResponse {
                id: *state.get_event(event_id).unwrap().survivors()[0]
                    .id()
                    .as_uuid(),
                zone_id: *zone_id.as_uuid(),
                status: SurvivorStatusDto::Active,
                triage_status: TriageStatusDto::Minor,
                location: None,
                vital_signs: VitalSignsSummaryDto {
                    breathing_rate: Some(16.),
                    breathing_type: Some("Normal".into()),
                    heart_rate: Some(72.),
                    has_heartbeat: true,
                    has_movement: false,
                    movement_type: None,
                    timestamp: chrono::Utc::now(),
                },
                confidence: 0.9,
                first_detected: chrono::Utc::now(),
                last_updated: chrono::Utc::now(),
                is_deteriorating: false,
                metadata: None,
            };
            for (kind, message) in [
                (
                    "survivor_detected",
                    WebSocketMessage::SurvivorDetected {
                        event_id,
                        survivor: survivor.clone(),
                    },
                ),
                (
                    "survivor_updated",
                    WebSocketMessage::SurvivorUpdated { event_id, survivor },
                ),
                (
                    "event_status_changed",
                    WebSocketMessage::EventStatusChanged {
                        event_id,
                        old_status: EventStatusDto::Initializing,
                        new_status: EventStatusDto::Active,
                    },
                ),
            ] {
                state.broadcast(message);
                let frame = tokio::time::timeout(std::time::Duration::from_secs(5), socket.next())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
                let value: serde_json::Value =
                    serde_json::from_str(frame.to_text().unwrap()).unwrap();
                assert_eq!(value["type"], kind);
                assert_eq!(value["event_id"], event_id.to_string());
                if kind.starts_with("survivor_") {
                    assert_eq!(value["survivor"]["confidence"], 0.9);
                    assert_eq!(
                        value["survivor"].get("vital_signs").is_none(),
                        privacy,
                        "{value}"
                    );
                    if !privacy {
                        assert_eq!(value["survivor"]["vital_signs"]["heart_rate"], 72.);
                        assert_eq!(value["survivor"]["vital_signs"]["breathing_rate"], 16.);
                    }
                } else {
                    assert_eq!(value["new_status"], "Active");
                }
            }
            let alert_response = AlertResponse {
                id: *alert.id().as_uuid(),
                survivor_id: *alert.survivor_id().as_uuid(),
                priority: alert.priority().into(),
                status: alert.status().clone().into(),
                title: alert.payload().title.clone(),
                message: alert.payload().message.clone(),
                triage_status: alert.payload().triage_status.clone().into(),
                location: None,
                recommended_action: Some(alert.payload().recommended_action.clone()),
                created_at: *alert.created_at(),
                acknowledged_at: None,
                acknowledged_by: None,
                escalation_count: 0,
            };
            for message in [
                WebSocketMessage::AlertCreated {
                    event_id,
                    alert: alert_response.clone(),
                },
                WebSocketMessage::AlertUpdated {
                    event_id,
                    alert: alert_response,
                },
            ] {
                state.broadcast(message);
                let frame = tokio::time::timeout(std::time::Duration::from_secs(5), socket.next())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
                let value: serde_json::Value =
                    serde_json::from_str(frame.to_text().unwrap()).unwrap();
                assert_eq!(value["alert"]["id"], alert.id().to_string());
                assert_eq!(value["alert"].get("message").is_none(), privacy, "{value}");
                assert!(value["alert"].get("recommended_action").is_some());
                if !privacy {
                    assert!(value["alert"]["message"]
                        .as_str()
                        .unwrap()
                        .contains("Breathing: 16.0 BPM"));
                }
            }
            // Actual REST mutation -> generated AlertUpdated -> authenticated
            // socket, not only a manually constructed broadcast fixture.
            let response = app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/api/v1/mat/alerts/{}/acknowledge", alert.id()))
                        .header("authorization", "Bearer mat-test-fixture-token")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            r#"{"acknowledged_by":"SYNTHETIC PRIVACY TEST"}"#,
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
            let acknowledgement: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(acknowledgement["alert"].get("message").is_none(), privacy);
            let frame = tokio::time::timeout(std::time::Duration::from_secs(5), socket.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let value: serde_json::Value = serde_json::from_str(frame.to_text().unwrap()).unwrap();
            assert_eq!(value["type"], "alert_updated");
            assert_eq!(value["alert"]["status"], "Acknowledged");
            assert_eq!(value["alert"].get("message").is_none(), privacy);
            socket.close(None).await.unwrap();
            server.abort();
        }
    }
}
