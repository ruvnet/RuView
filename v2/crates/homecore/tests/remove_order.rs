use homecore::{Context, EntityId, StateMachine};
use std::sync::{Arc, Barrier};

#[test]
fn concurrent_remove_and_set_preserve_transition_order() {
    for trial in 0..100 {
        let sm = StateMachine::new();
        let id = EntityId::parse("sensor.race").unwrap();
        let mut rx = sm.subscribe();
        let barrier = Arc::new(Barrier::new(4));
        let workers: Vec<_> = (0..4)
            .map(|worker| {
                let (sm, id, barrier) = (sm.clone(), id.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    for step in 0..100 {
                        sm.set(
                            id.clone(),
                            format!("{worker}-{step}"),
                            serde_json::json!({}),
                            Context::new(),
                        );
                        sm.remove(&id);
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        let mut previous = None;
        let mut observed = 0;
        loop {
            let event = match rx.try_recv() {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                Err(error) => panic!("unexpected broadcast error: {error}"),
            };
            observed += 1;
            let old = event.old_state.as_ref().map(|s| s.state.clone());
            assert_eq!(
                old, previous,
                "event stream is out of commit order in trial {trial}"
            );
            previous = event.new_state.as_ref().map(|s| s.state.clone());
        }
        assert!(observed >= 400, "all 400 unique writes must be observed");
        assert_eq!(previous, sm.get(&id).map(|s| s.state.clone()));
    }
}
