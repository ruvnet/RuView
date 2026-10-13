use homecore::service::FnHandler;
use homecore::{Context, EntityId, HomeCore, ServiceCall, ServiceName};
use homecore_automation::{Automation, AutomationEngine, RunMode};
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn engine_choose_templates_use_live_state() {
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
    let mut automation: Automation = serde_yaml::from_str(
        r#"
id: choose_template
trigger:
  - platform: time
    at: "07:30:00"
action:
  - action: choose
    choices:
      - conditions:
          - condition: template
            value_template: "{{ is_state('light.kitchen', 'on') }}"
        sequence:
          - action: service_call
            domain: test
            service: record
            data: { branch: matched }
    default:
      - action: service_call
        domain: test
        service: record
        data: { branch: default }
"#,
    )
    .unwrap();
    automation.mode = RunMode::Parallel;
    let engine = AutomationEngine::new(hc.clone());
    engine.register(automation);

    for (state, branch) in [("on", "matched"), ("off", "default")] {
        hc.states().set(
            EntityId::parse("light.kitchen").unwrap(),
            state,
            serde_json::json!({}),
            Context::new(),
        );
        assert_eq!(engine.fire_time_for_test("07:30:00").await, 1);
        let result = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("automation should execute a branch")
            .unwrap();
        assert_eq!(result["branch"], branch);
    }
}
