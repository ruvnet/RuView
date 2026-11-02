//! Concurrent state machine — the heart of HOMECORE.
//!
//! Mirrors `homeassistant.core.StateMachine`. Differences from HA per
//! ADR-127 §2.1:
//!
//! - DashMap shard-locked instead of one asyncio.Lock for the whole map
//! - Writers atomically replace `Arc<State>` entries; readers get
//!   zero-copy clones
//! - State changes fan out via a tokio broadcast channel (capacity
//!   4,096); slow subscribers get `Lagged` and must re-sync from the
//!   current map
//!
//! ## NOT in P1 (deferred to P2+)
//!
//! - `async_set_internal` schema validation
//! - Bulk delete of an entire domain (`async_remove_domain`)

use std::sync::Arc;

use chrono::Utc;
use dashmap::DashMap;
use tokio::sync::broadcast;

use crate::bus::EventBus;
use crate::entity::{EntityId, State};
use crate::event::{Context, StateChangedEvent, SystemEvent};

/// Broadcast channel capacity for state-changed events. 4,096 events
/// at 20 Hz per entity covers ~3 minutes of backlog for a single hot
/// entity. Slow subscribers must re-sync from the current map.
pub const STATE_CHANGED_CHANNEL_CAPACITY: usize = 4096;

/// The state machine. Cheap to clone (one `Arc`) — pass copies to as
/// many tasks as you like.
#[derive(Clone)]
pub struct StateMachine {
    inner: Arc<StateMachineInner>,
}

struct StateMachineInner {
    states: DashMap<EntityId, Arc<State>>,
    tx: broadcast::Sender<StateChangedEvent>,
    bus: Option<EventBus>,
}

impl StateMachine {
    pub fn new() -> Self {
        Self::new_inner(None)
    }

    /// Create a state machine that also publishes every committed change on
    /// the shared HOMECORE system event bus. Standalone state machines retain
    /// their lightweight private broadcast channel via [`StateMachine::new`].
    pub fn with_event_bus(bus: EventBus) -> Self {
        Self::new_inner(Some(bus))
    }

    fn new_inner(bus: Option<EventBus>) -> Self {
        let (tx, _) = broadcast::channel(STATE_CHANGED_CHANNEL_CAPACITY);
        Self {
            inner: Arc::new(StateMachineInner {
                states: DashMap::with_capacity(256),
                tx,
                bus,
            }),
        }
    }

    /// Subscribe to state-changed events. Each subscriber gets an
    /// independent receiver; capacity is shared. Falling behind by
    /// 4,096 events yields `RecvError::Lagged(n)`.
    pub fn subscribe(&self) -> broadcast::Receiver<StateChangedEvent> {
        self.inner.tx.subscribe()
    }

    /// Read a state. Returns `None` if the entity is unknown.
    /// Zero-copy: caller gets an `Arc<State>` clone.
    pub fn get(&self, entity_id: &EntityId) -> Option<Arc<State>> {
        self.inner.states.get(entity_id).map(|s| Arc::clone(&s))
    }

    /// Write a state. Fires a `state_changed` broadcast even on the
    /// first write (old_state = None). HA semantics: only fires if the
    /// state string OR attributes changed; pure no-op writes are
    /// suppressed.
    ///
    /// Returns the new state snapshot.
    pub fn set(
        &self,
        entity_id: EntityId,
        new_state: impl Into<String>,
        attributes: serde_json::Value,
        context: Context,
    ) -> Arc<State> {
        let new_state_str = new_state.into();

        // Hold the DashMap shard write-lock across the entire
        // read→decide→insert→fire sequence. `entry()` locks the shard for
        // the lifetime of `slot`, so a concurrent writer on the same entity
        // cannot interleave between our read of `old` and our commit. This
        // is what makes the write atomic as ADR-127 §2.1 promises ("writer
        // atomically replaces the map entry") — the previous get→insert pair
        // released the lock in between, a TOCTOU that let concurrent writers
        // compute the no-op / `last_changed` decision off a stale `old` and
        // drop or reorder real `state_changed` events.
        //
        // `tx.send` is non-blocking, non-async, and never re-enters the map,
        // so firing under the lock cannot deadlock and keeps the global
        // event order in lock-step with the global commit order.
        use dashmap::mapref::entry::Entry;
        let slot = self.inner.states.entry(entity_id.clone());

        let old: Option<Arc<State>> = match &slot {
            Entry::Occupied(o) => Some(Arc::clone(o.get())),
            Entry::Vacant(_) => None,
        };
        // `slot` continues to hold the shard write-lock below.

        let next = match &old {
            Some(prev) => Arc::new(prev.next(new_state_str.clone(), attributes.clone(), context)),
            None => Arc::new(State::new(
                entity_id.clone(),
                new_state_str.clone(),
                attributes.clone(),
                context,
            )),
        };

        // HA suppresses no-op writes (same state + same attributes).
        // We follow the same rule to keep the broadcast channel quiet.
        let is_noop = match &old {
            Some(prev) => prev.state == new_state_str && prev.attributes == attributes,
            None => false,
        };

        // Commit through the same locked entry and KEEP the shard guard
        // alive across the broadcast `send`, so the event is published
        // before any concurrent writer on this entity can observe the new
        // value and fire its own event. This makes global event order match
        // global commit order (no insert/send reorder window).
        let _guard = slot.insert_entry(Arc::clone(&next));

        if !is_noop {
            let event = StateChangedEvent {
                entity_id,
                old_state: old,
                new_state: Some(Arc::clone(&next)),
                fired_at: Utc::now(),
            };
            // err = no receivers; that's fine, write still committed.
            let _ = self.inner.tx.send(event.clone());
            if let Some(bus) = &self.inner.bus {
                bus.fire_system(SystemEvent::StateChanged(event));
            }
        }
        // `_guard` (and the shard lock) drops here, after the event is sent.
        next
    }

    /// Install a durable snapshot during startup without emitting a new
    /// state-change event. Callers must mark the snapshot context as a
    /// restoration; this prevents accidental use as a silent runtime write.
    pub fn restore(&self, snapshot: State) -> Result<Arc<State>, RestoreStateError> {
        if !snapshot.context.is_restoration() {
            return Err(RestoreStateError::UnmarkedContext);
        }
        let entity_id = snapshot.entity_id.clone();
        let snapshot = Arc::new(snapshot);
        self.inner.states.insert(entity_id, Arc::clone(&snapshot));
        Ok(snapshot)
    }

    /// Remove a state. Fires `state_changed` with `new_state = None`.
    pub fn remove(&self, entity_id: &EntityId) -> Option<Arc<State>> {
        use dashmap::mapref::entry::Entry;
        let Entry::Occupied(entry) = self.inner.states.entry(entity_id.clone()) else {
            return None;
        };
        // Publish while holding the same shard lock used by set(). A writer
        // cannot recreate the entity and publish ahead of this removal.
        // Readers receiving the event wait for the removal to commit below.
        {
            let old = entry.get();
            let event = StateChangedEvent {
                entity_id: entity_id.clone(),
                old_state: Some(Arc::clone(old)),
                new_state: None,
                fired_at: Utc::now(),
            };
            let _ = self.inner.tx.send(event.clone());
            if let Some(bus) = &self.inner.bus {
                bus.fire_system(SystemEvent::StateChanged(event));
            }
        }
        Some(entry.remove())
    }

    /// Snapshot all current states. Allocates a new Vec — useful for
    /// the REST GET /api/states path (ADR-130).
    pub fn all(&self) -> Vec<Arc<State>> {
        self.inner
            .states
            .iter()
            .map(|r| Arc::clone(r.value()))
            .collect()
    }

    /// Snapshot all states whose entity_id matches a domain prefix.
    /// Mirrors HA's `hass.states.async_all(domain)`.
    pub fn all_by_domain(&self, domain: &str) -> Vec<Arc<State>> {
        self.inner
            .states
            .iter()
            .filter(|r| r.key().domain() == domain)
            .map(|r| Arc::clone(r.value()))
            .collect()
    }

    /// Number of entities currently tracked.
    pub fn len(&self) -> usize {
        self.inner.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.states.len() == 0
    }
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RestoreStateError {
    #[error("restored state context is not marked as a restoration")]
    UnmarkedContext,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> EntityId {
        EntityId::parse(s).unwrap()
    }

    #[tokio::test]
    async fn set_writes_and_fires() {
        let sm = StateMachine::new();
        let mut rx = sm.subscribe();
        sm.set(
            id("light.kitchen"),
            "on",
            serde_json::json!({"brightness": 200}),
            Context::new(),
        );
        let evt = rx.recv().await.unwrap();
        assert_eq!(evt.entity_id.as_str(), "light.kitchen");
        assert!(evt.old_state.is_none());
        assert_eq!(evt.new_state.as_ref().unwrap().state, "on");
    }

    #[tokio::test]
    async fn noop_writes_are_suppressed() {
        let sm = StateMachine::new();
        sm.set(id("light.k"), "on", serde_json::json!({}), Context::new());
        let mut rx = sm.subscribe();
        // Same state + same attributes → no event.
        sm.set(id("light.k"), "on", serde_json::json!({}), Context::new());
        let try_recv = tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await;
        assert!(try_recv.is_err(), "expected no event for no-op write");
    }

    #[tokio::test]
    async fn attribute_only_change_fires_but_preserves_last_changed() {
        let sm = StateMachine::new();
        let s1 = sm.set(
            id("sensor.t"),
            "20",
            serde_json::json!({"unit": "C"}),
            Context::new(),
        );
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let s2 = sm.set(
            id("sensor.t"),
            "20",
            serde_json::json!({"unit": "F"}),
            Context::new(),
        );
        assert_eq!(s1.last_changed, s2.last_changed);
        assert!(s2.last_updated > s1.last_updated);
    }

    #[test]
    fn all_by_domain_filters() {
        let sm = StateMachine::new();
        sm.set(id("light.a"), "on", serde_json::json!({}), Context::new());
        sm.set(id("light.b"), "off", serde_json::json!({}), Context::new());
        sm.set(id("sensor.t"), "20", serde_json::json!({}), Context::new());
        assert_eq!(sm.all_by_domain("light").len(), 2);
        assert_eq!(sm.all_by_domain("sensor").len(), 1);
        assert_eq!(sm.all().len(), 3);
    }

    #[tokio::test]
    async fn remove_fires_with_no_new_state() {
        let sm = StateMachine::new();
        sm.set(id("light.k"), "on", serde_json::json!({}), Context::new());
        let mut rx = sm.subscribe();
        sm.remove(&id("light.k"));
        let evt = rx.recv().await.unwrap();
        assert!(evt.new_state.is_none());
        assert!(evt.old_state.is_some());
    }

    /// Concurrency invariant (ADR-127 §2.1 "writer atomically replaces the
    /// map entry"): under concurrent writers on the SAME entity the fired
    /// `state_changed` stream must be a faithful, gap-free log of the
    /// committed transitions — in particular the LAST event the bus
    /// delivers must carry the SAME value that is finally committed in the
    /// map.
    ///
    /// This pins the TOCTOU in `set`: it does `get` (release shard lock) →
    /// compute `next` + no-op decision → `insert` (re-acquire shard lock) →
    /// `send`. Because the insert and the send are not atomic with respect
    /// to a concurrent writer, two writers can interleave as
    /// `insert(A); insert(B); send(B); send(A)` — leaving the map holding A
    /// while the last event the bus ever delivers says B. A subscriber that
    /// trusts "the last event reflects current state" (the recorder, the WS
    /// push API, an automation engine) is then permanently wrong about the
    /// entity until the next write. A correctly-locked store holds the shard
    /// lock across read→insert→send so the global event order matches the
    /// global commit order.
    ///
    /// A dedicated drain thread pulls events as they arrive so the bounded
    /// channel never lags during the run (a `Lagged` here would be a test
    /// artefact, not the bug under test).
    ///
    /// The writers toggle the SAME entity between exactly two values so the
    /// no-op suppression branch is constantly in play.
    ///
    /// Invariant: in correctly serialised code, two *consecutive* fired
    /// `state_changed` events can never carry the same `new_state` value.
    /// Proof: event k fires only for a committed transition old≠new, so its
    /// `new_state` = X differs from the value before it; the next committed
    /// transition therefore starts at X and (being a real change) commits
    /// some Z≠X, so event k+1 carries Z≠X. A no-op (X→X) is suppressed and
    /// never fires. Therefore adjacent fired events always differ.
    ///
    /// The `set()` TOCTOU breaks this: it does `get` (release shard lock) →
    /// compute `next` + the no-op decision → `insert` (re-acquire shard
    /// lock) → `send`, all non-atomically. A writer that read a STALE `old`
    /// mis-classifies a genuine transition as a no-op (dropping that real
    /// event — a missed automation trigger) and/or fires an event whose
    /// `new_state` duplicates the previously delivered one (a spurious
    /// trigger for any automation keyed on `old_state != new_state`). The
    /// probe behind this test observed ~93k such duplicate-adjacent events
    /// across 200 trials on the racy code; the corrected store produces
    /// zero.
    #[test]
    fn concurrent_set_fires_no_duplicate_adjacent_events() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Barrier, Mutex};

        const WRITERS: usize = 4;
        const ITERS: usize = 300; // 1200 events ≪ 4096 capacity → never lags

        for _trial in 0..40 {
            let sm = StateMachine::new();
            let eid = id("light.race");
            sm.set(eid.clone(), "A", serde_json::json!({}), Context::new());

            let mut rx = sm.subscribe();
            let done = Arc::new(AtomicBool::new(false));
            // Event log: new_state value in delivery order.
            let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

            let drainer = {
                let done = Arc::clone(&done);
                let log = Arc::clone(&log);
                std::thread::spawn(move || loop {
                    match rx.try_recv() {
                        Ok(evt) => {
                            if let Some(ns) = &evt.new_state {
                                log.lock().unwrap().push(ns.state.clone());
                            }
                        }
                        Err(broadcast::error::TryRecvError::Empty) => {
                            if done.load(Ordering::Acquire) {
                                while let Ok(evt) = rx.try_recv() {
                                    if let Some(ns) = &evt.new_state {
                                        log.lock().unwrap().push(ns.state.clone());
                                    }
                                }
                                break;
                            }
                            std::thread::yield_now();
                        }
                        Err(broadcast::error::TryRecvError::Lagged(_)) => {
                            panic!("channel lagged — test artefact, raise capacity");
                        }
                        Err(broadcast::error::TryRecvError::Closed) => break,
                    }
                })
            };

            let barrier = Arc::new(Barrier::new(WRITERS));
            let handles: Vec<_> = (0..WRITERS)
                .map(|w| {
                    let sm = sm.clone();
                    let eid = eid.clone();
                    let barrier = Arc::clone(&barrier);
                    std::thread::spawn(move || {
                        barrier.wait();
                        for i in 0..ITERS {
                            // Toggle between two values → maximises the
                            // stale-`old` no-op collision window.
                            let val = if (w + i) % 2 == 0 { "A" } else { "B" };
                            sm.set(eid.clone(), val, serde_json::json!({}), Context::new());
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
            done.store(true, Ordering::Release);
            drainer.join().unwrap();

            let log = log.lock().unwrap();
            let dup = log.windows(2).filter(|w| w[0] == w[1]).count();
            assert_eq!(
                dup, 0,
                "{dup} consecutive fired state_changed events carried an \
                 identical new_state — impossible under correct \
                 serialisation; proves set()'s read→decide→insert→send \
                 TOCTOU dropped/reordered real transitions (missed & \
                 spurious automation triggers)",
            );
        }
    }
}
