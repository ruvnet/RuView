use homecore::event::DomainEvent;
use homecore::service::FnHandler;
use homecore::{Context, HomeCore, ServiceCall, ServiceName};
use homecore_automation::{Action, Automation, AutomationEngine, Condition, Trigger};
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn domain_event_triggers_execute_and_honor_conditions() {
    let hc = HomeCore::new();
    let (tx, mut rx) = mpsc::unbounded_channel();
    hc.services()
        .register(
            ServiceName::new("test", "record"),
            FnHandler(move |call: ServiceCall| {
                let tx = tx.clone();
                async move {
                    tx.send(call.data).unwrap();
                    Ok(serde_json::Value::Null)
                }
            }),
        )
        .await;
    let engine = AutomationEngine::new(hc.clone());
    let mut auto = Automation::new(
        "button",
        vec![Trigger::Event {
            event_type: "button_pressed".into(),
        }],
        vec![Action::ServiceCall {
            domain: "test".into(),
            service: "record".into(),
            data: serde_json::json!("ran"),
        }],
    );
    let enabled = homecore::EntityId::parse("input_boolean.enabled").unwrap();
    auto.condition.push(Condition::State {
        entity_id: enabled.clone(),
        state: "on".into(),
    });
    engine.register(auto);
    let task = engine.start();
    hc.states()
        .set(enabled.clone(), "on", serde_json::json!({}), Context::new());
    // Published immediately after start: subscribing must precede spawning.
    hc.bus().fire_domain(DomainEvent::new(
        "button_pressed",
        serde_json::json!({}),
        Context::new(),
    ));
    assert_eq!(
        timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap(),
        serde_json::json!("ran")
    );
    hc.bus().fire_domain(DomainEvent::new(
        "other",
        serde_json::json!({}),
        Context::new(),
    ));
    hc.states()
        .set(enabled, "off", serde_json::json!({}), Context::new());
    hc.bus().fire_domain(DomainEvent::new(
        "button_pressed",
        serde_json::json!({}),
        Context::new(),
    ));
    assert!(timeout(Duration::from_millis(30), rx.recv()).await.is_err());
    task.abort();
}
