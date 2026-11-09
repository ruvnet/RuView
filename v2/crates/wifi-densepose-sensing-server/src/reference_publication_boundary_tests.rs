//! SYNTHETIC permission/IO boundaries only. No C4C source/fit proof is forged;
//! genuine HeldOut128/HTTP association is a separate integration acceptance.
use super::*;
use axum::{
    body::Body,
    http::{Method, Request},
};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use tower::ServiceExt;
fn ready_snapshot(store: &Arc<ReferenceStore>, grant: &Grant) -> ReferenceSnapshot {
    let at = now() * 1_000_000_000;
    store.permit("phone", true, at / 1_000_000_000).unwrap();
    let begin = store.admit(grant, scope(), begin(), at).unwrap();
    store
        .admit(
            grant,
            scope(),
            observation(&begin.reference_session_id, 1, at),
            at + 100_000,
        )
        .unwrap();
    store.snapshot(at / 1_000_000_000).unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_device_delete_closes_authority_before_drain_and_cannot_renew_in_gap() {
    let (directory, store, _watch, grant) = setup();
    let snapshot = ready_snapshot(&store, &grant);
    let (entered_send, entered) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let target = directory
        .path()
        .join("SYNTHETIC-prior-consent-history.json");
    let store_probe = store.clone();
    let writer = std::thread::spawn(move || {
        snapshot.with_recording(|| {
            assert!(
                store_probe.inner.try_lock().is_ok(),
                "no reference Inner lock held across IO"
            );
            entered_send.send(()).unwrap();
            released.recv().unwrap();
            crate::companion::atomic_private_write(&target, b"SYNTHETIC_DERIVED_METADATA")
                .map_err(|_| "test_io".into())
        })
    });
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    let router = crate::companion::owner_router(store.companion.clone())
        .layer(axum::Extension(store.clone()));
    let delete = tokio::spawn(async move {
        router
            .oneshot(
                Request::builder()
                    .method(Method::DELETE)
                    .uri("/api/v1/companion/devices/phone")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    });
    let until = Instant::now() + Duration::from_secs(1);
    while store.companion.paired_device("phone").is_some() && Instant::now() < until {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    let closed_before_drain = store.companion.paired_device("phone").is_none();
    let early_ack = delete.is_finished();
    let renew_store = store.clone();
    let renewal = std::thread::spawn(move || renew_store.permit("phone", true, now()));
    release.send(()).unwrap();
    let _prior_write = writer.join().unwrap();
    let response = delete.await.unwrap();
    let renewed = renewal.join().unwrap();
    assert!(
        closed_before_drain,
        "paired authority must close before the writer wait"
    );
    assert!(
        !early_ack,
        "DELETE cannot ACK while an entered recording is live"
    );
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert!(
        renewed.is_err(),
        "a suspended deletion cannot recreate purpose"
    );
    assert!(store.snapshot(now()).is_err());
    let retry = crate::companion::owner_router(store.companion.clone())
        .layer(axum::Extension(store.clone()))
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/v1/companion/devices/phone")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retry.status(), axum::http::StatusCode::NOT_FOUND);
}
#[test]
fn publication_transition_closes_admission_before_drain_and_drop_never_revives_rows() {
    let (directory, store, _watch, grant) = setup();
    let snapshot = ready_snapshot(&store, &grant);
    let stale = snapshot.clone();
    let (entered_send, entered) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let target = directory.path().join("SYNTHETIC-history.json");
    let writer = std::thread::spawn(move || {
        snapshot.with_recording(|| {
            entered_send.send(()).unwrap();
            released.recv().unwrap();
            crate::companion::atomic_private_write(&target, b"SYNTHETIC")
                .map_err(|_| "test_io".into())
        })
    });
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    let (token_send, token_received) = mpsc::channel();
    let transitioned = store.clone();
    let transition = std::thread::spawn(move || {
        let token = transitioned.suspend_publication().unwrap();
        token_send.send(token).unwrap();
    });
    let until = Instant::now() + Duration::from_secs(1);
    while store.inner.lock().unwrap().transition.is_none() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(1));
    }
    let closed = store.inner.lock().unwrap().transition.is_some();
    assert!(
        token_received.try_recv().is_err(),
        "cannot return lease before writer drain"
    );
    release.send(()).unwrap();
    let _prior_write = writer.join().unwrap();
    let token = token_received.recv_timeout(Duration::from_secs(2)).unwrap();
    transition.join().unwrap();
    assert!(closed);
    assert!(stale.ensure_live().is_err());
    assert!(store.permit("phone", true, now()).is_err());
    assert!(store.snapshot(now()).is_err());
    assert!(
        store.suspend_publication().is_err(),
        "second transition cannot own/clear first nonce"
    );
    drop(token);
    assert!(store.inner.lock().unwrap().transition.is_none());
    assert_eq!(store.summary(now())["active"], false);
    assert_eq!(store.summary(now())["retained_ram_rows"], 0);
    store.permit("phone", true, now()).unwrap();
    assert!(stale.ensure_live().is_err());
}
#[test]
fn publication_transition_unwind_clears_only_fence_and_does_not_resurrect_consent() {
    let (_dir, store, _watch, grant) = setup();
    let snapshot = ready_snapshot(&store, &grant);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _lease = store.suspend_publication().unwrap();
        panic!("SYNTHETIC transition cancellation");
    }));
    assert!(result.is_err());
    assert!(store.inner.lock().unwrap().transition.is_none());
    assert!(snapshot.ensure_live().is_err());
    assert_eq!(store.summary(now())["retained_ram_rows"], 0);
    store.permit("phone", true, now()).unwrap();
    assert!(snapshot.ensure_live().is_err());
}
