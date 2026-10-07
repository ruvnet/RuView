use super::*;
use crate::action::Action;
use chrono::TimeZone;
use homecore::service::FnHandler;
use homecore::{ServiceCall, ServiceName};
use tokio::sync::mpsc;
use tokio::time::{advance, timeout, Duration};

#[tokio::test(start_paused = true)]
async fn daily_timer_fires_again_on_the_next_date() {
    let hc = HomeCore::new();
    let (actions_tx, mut actions) = mpsc::unbounded_channel();
    hc.services()
        .register(
            ServiceName::new("test", "record"),
            FnHandler(move |_: ServiceCall| {
                let tx = actions_tx.clone();
                async move {
                    tx.send(()).unwrap();
                    Ok(serde_json::Value::Null)
                }
            }),
        )
        .await;
    let engine = AutomationEngine::new(hc);
    engine.register(Automation::new(
        "daily",
        vec![Trigger::Time {
            at: "07:30:00".into(),
        }],
        vec![Action::ServiceCall {
            domain: "test".into(),
            service: "record".into(),
            data: serde_json::Value::Null,
        }],
    ));
    let first = Local.with_ymd_and_hms(2026, 1, 1, 7, 30, 0).unwrap();
    let clock = Arc::new(Mutex::new(first));
    let timer_clock = clock.clone();
    let (ticks_tx, mut ticks) = mpsc::unbounded_channel();
    let timer = engine.start_timer_with_clock(move || {
        let now = *timer_clock.lock().unwrap();
        ticks_tx.send(now).unwrap();
        now
    });
    assert_eq!(ticks.recv().await.unwrap(), first);
    assert_eq!(actions.recv().await, Some(()));

    // Several clock ticks in the same matching wall-clock second must not rerun it.
    advance(Duration::from_secs(1)).await;
    assert_eq!(ticks.recv().await.unwrap(), first);
    tokio::task::yield_now().await;
    assert!(actions.try_recv().is_err());

    // A whole day can pass without any other scheduled automation firing.
    *clock.lock().unwrap() = Local.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
    advance(Duration::from_secs(1)).await;
    ticks.recv().await.unwrap();
    let next = Local.with_ymd_and_hms(2026, 1, 2, 7, 30, 0).unwrap();
    *clock.lock().unwrap() = next;
    advance(Duration::from_secs(1)).await;
    assert_eq!(ticks.recv().await.unwrap(), next);
    let result = timeout(Duration::from_millis(10), actions.recv()).await;
    timer.abort();
    let _ = timer.await;
    assert_eq!(
        result.expect("daily automation must fire on the next date"),
        Some(())
    );
}
