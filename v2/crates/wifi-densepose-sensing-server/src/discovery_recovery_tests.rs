//! No live daemon/socket/network: exercise the production reducer and worker
//! through synthetic event/registration ports.
use super::*;
use std::sync::{mpsc, Mutex};

fn add(ip: &str) -> DaemonEvent {
    DaemonEvent::IpAdd(ip.parse().unwrap())
}
fn del(ip: &str) -> DaemonEvent {
    DaemonEvent::IpDel(ip.parse().unwrap())
}

#[test]
fn failed_bind_then_ip_add_refreshes_exact_pristine_service_without_event_address() {
    let template = build_service(
        "same companion",
        "same-installation",
        "ruview-same.local.",
        3443,
        true,
    )
    .unwrap();
    let original = format!("{template:?}");
    let mut state = RecoveryState::default();
    let mut sent = Vec::new();
    // A failed bind/Error itself is not an instruction to repeatedly announce.
    state.event(
        DaemonEvent::Error(mdns_sd::Error::Msg("synthetic failed bind".into())),
        0,
    );
    state.poll(1000, || {
        panic!("failure alone must not enqueue");
    });
    state.event(add("192.168.1.2"), 1000);
    state.poll(1249, || {
        panic!("coalescing boundary");
    });
    state.poll(1250, || {
        sent.push(template.clone());
        Ok(())
    });
    assert_eq!(sent.len(), 1);
    assert_eq!(format!("{:?}", sent[0]), original);
    assert!(
        sent[0].get_addresses().is_empty(),
        "event address must never be injected"
    );
    assert_eq!(sent[0].get_hostname(), "ruview-same.local.");
    assert_eq!(sent[0].get_port(), 3443);
    assert_eq!(sent[0].get_property_val_str("tls"), Some("1"));
}

#[test]
fn duplicate_burst_and_irrelevant_events_are_bounded_without_postponing_refresh() {
    let mut state = RecoveryState::default();
    for i in 1..=255 {
        state.event(add(&format!("10.0.0.{i}")), i as u64);
    }
    for _ in 0..1000 {
        state.event(add("10.0.0.1"), 249);
    }
    assert_eq!(state.recovered.len(), MAX_RECOVERED_IPS);
    assert_eq!(state.due, Some(251));
    let mut calls = 0;
    state.poll(251, || {
        calls += 1;
        Ok(())
    });
    state.event(add("127.0.0.1"), 1000);
    state.event(add("8.8.8.8"), 1000);
    state.event(add("169.254.1.2"), 1000);
    state.event(add("::1"), 1000);
    state.event(
        DaemonEvent::Announce("foreign".into(), "not-health".into()),
        1000,
    );
    state.poll(u64::MAX, || {
        calls += 1;
        Ok(())
    });
    assert_eq!(calls, 1);
}

#[test]
fn same_ip_rebind_refreshes_with_or_without_a_del_notification() {
    let mut state = RecoveryState::default();
    let mut calls = 0;
    state.event(add("10.0.0.1"), 0);
    state.poll(250, || {
        calls += 1;
        Ok(())
    });
    state.event(del("10.0.0.1"), 300);
    state.event(add("10.0.0.1"), 301);
    assert_eq!(state.due, Some(1250));
    state.poll(1250, || {
        calls += 1;
        Ok(())
    });
    // The monitor may drop IpDel when full: do not suppress this real rebind.
    for _ in 0..1000 {
        state.event(add("10.0.0.1"), 1251);
    }
    assert_eq!(state.due, Some(2250));
    state.poll(2249, || panic!("refresh storm"));
    state.poll(2250, || {
        calls += 1;
        Ok(())
    });
    assert_eq!(calls, 3);
}

#[test]
fn settled_wakeups_do_not_fill_a_permanent_interface_cache_when_ip_del_is_lost() {
    let mut state = RecoveryState::default();
    let mut calls = 0;
    for i in 1..=32 {
        let now = i * 2000;
        state.event(add(&format!("10.0.0.{i}")), now);
        state.poll(now + COALESCE_MS, || {
            calls += 1;
            Ok(())
        });
        assert!(state.recovered.is_empty());
    }
    assert_eq!(calls, 32);
}

#[test]
fn queued_ip_del_and_stop_withdraw_pending_refresh() {
    let mut state = RecoveryState::default();
    state.event(add("192.168.1.2"), 0);
    state.event(del("192.168.1.2"), 249);
    state.poll(250, || panic!("deleted wakeup must be withdrawn"));
    state.event(add("192.168.1.2"), 251);
    state.stop();
    state.event(add("10.0.0.1"), 1000);
    state.poll(u64::MAX, || panic!("stopped generation must not register"));
    assert!(state.recovered.is_empty());
}

#[test]
fn again_has_three_bounded_backoffs_then_stops_and_terminal_error_does_not_spin() {
    let mut state = RecoveryState::default();
    state.event(add("10.0.0.1"), 0);
    let mut calls = 0;
    for (now, next) in [
        (250, Some(1250)),
        (1250, Some(3250)),
        (3250, Some(7250)),
        (7250, None),
    ] {
        state.poll(now, || {
            calls += 1;
            Err(mdns_sd::Error::Again)
        });
        assert_eq!(state.due, next);
        state.poll(now, || panic!("no busyloop"));
    }
    assert_eq!(calls, 4);
    state.event(add("10.0.0.2"), 8000);
    state.poll(8250, || {
        Err(mdns_sd::Error::Msg("synthetic closed daemon".into()))
    });
    assert!(state.due.is_none());
}

struct TestEvents(mpsc::Receiver<DaemonEvent>);
impl RecoveryEvents for TestEvents {
    fn read(&self, timeout: Duration) -> EventRead {
        match self.0.recv_timeout(timeout) {
            Ok(event) => EventRead::Event(event),
            Err(mpsc::RecvTimeoutError::Timeout) => EventRead::Idle,
            Err(mpsc::RecvTimeoutError::Disconnected) => EventRead::Closed,
        }
    }
}

#[test]
fn monitor_close_discards_pending_intent_and_worker_joins() {
    let (tx, rx) = mpsc::channel();
    tx.send(add("10.0.0.1")).unwrap();
    drop(tx);
    let mut worker =
        RecoveryWorker::start_with(TestEvents(rx), || panic!("closed monitor cannot register"))
            .unwrap();
    worker.thread.take().unwrap().join().unwrap();
}

#[test]
fn worker_stop_joins_inflight_registration_before_teardown_and_never_enqueues_again() {
    let (tx, rx) = mpsc::channel();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let calls = Arc::new(Mutex::new(0));
    let seen = calls.clone();
    let worker = RecoveryWorker::start_with(TestEvents(rx), move || {
        *seen.lock().unwrap() += 1;
        entered_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        Ok(())
    })
    .unwrap();
    tx.send(add("10.0.0.1")).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    // Force stop publication before releasing the in-flight fake command.
    worker.stop.store(true, Ordering::Release);
    tx.send(del("10.0.0.1")).unwrap();
    tx.send(add("10.0.0.2")).unwrap();
    let stopper = thread::spawn(move || {
        drop(worker);
        done_tx.send(()).unwrap();
    });
    assert!(
        done_rx.try_recv().is_err(),
        "teardown must wait for dispatched registration"
    );
    release_tx.send(()).unwrap();
    done_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    stopper.join().unwrap();
    assert_eq!(*calls.lock().unwrap(), 1);
}
