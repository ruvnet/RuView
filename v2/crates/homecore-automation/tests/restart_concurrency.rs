use std::sync::{Arc, Barrier};

use homecore::service::FnHandler;
use homecore::{HomeCore, ServiceCall, ServiceName};
use homecore_automation::{Action, Automation, AutomationEngine, RunMode, Trigger};
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

// The real timer and state-event loop can dispatch the same automation from
// different Tokio workers. Keep action tasks unpolled until all callers return,
// so only the last replacement is allowed to execute its service call.
#[tokio::test]
async fn concurrent_restart_dispatch_keeps_only_one_action_task() {
    let hc = HomeCore::new();
    let (tx, mut calls) = mpsc::unbounded_channel();
    hc.services()
        .register(
            ServiceName::new("test", "record"),
            FnHandler(move |_: ServiceCall| {
                let tx = tx.clone();
                async move {
                    tx.send(()).unwrap();
                    Ok(serde_json::Value::Null)
                }
            }),
        )
        .await;
    let engine = Arc::new(AutomationEngine::new(hc));
    let mut automation = Automation::new(
        "restart",
        vec![Trigger::Time {
            at: "07:30:00".into(),
        }],
        vec![Action::ServiceCall {
            domain: "test".into(),
            service: "record".into(),
            data: serde_json::Value::Null,
        }],
    );
    automation.mode = RunMode::Restart;
    engine.register(automation);

    for round in 0..16 {
        let barrier = Barrier::new(16);
        let runtime = tokio::runtime::Handle::current();
        std::thread::scope(|scope| {
            for _ in 0..16 {
                let engine = &engine;
                let barrier = &barrier;
                let runtime = &runtime;
                scope.spawn(move || {
                    barrier.wait();
                    assert_eq!(runtime.block_on(engine.fire_time_for_test("07:30:00")), 1);
                });
            }
        });
        assert_eq!(
            timeout(Duration::from_secs(1), calls.recv()).await.unwrap(),
            Some(())
        );
        assert!(
            timeout(Duration::from_millis(10), calls.recv())
                .await
                .is_err(),
            "restart left multiple surviving runs in round {round}"
        );
    }
}
