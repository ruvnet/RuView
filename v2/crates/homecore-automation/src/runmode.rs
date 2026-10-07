//! Per-automation run-mode machinery (ADR-162, completes ADR-161 §A5).
//!
//! ADR-161 implemented `RunMode::Single` (a per-automation `AtomicBool`
//! re-entrancy guard) and `Parallel`, but honestly left `Restart`, `Queued`
//! and `max: N` as "ACCEPTED-FUTURE / unbounded parallel" — every non-Single
//! mode spawned an unbounded task. This module makes them real:
//!
//! | Mode | Semantics implemented |
//! |------|-----------------------|
//! | `Single` / `IgnoreFirst` | re-entrancy guard: skip while a run is in flight (ADR-161). |
//! | `Restart` | **cancel** the in-flight run (`tokio::task::AbortHandle`) and start a fresh one. |
//! | `Queued` | **serialize**: runs execute sequentially in arrival order via a per-automation async mutex — nothing is dropped. |
//! | `Parallel` | spawn on every trigger (optionally capped, see below). |
//! | `max: N` | cap concurrency at **N** via a per-automation semaphore; triggers beyond N **queue** (await a permit) rather than running concurrently — matching HA's bounded `parallel`/`queued`. |
//!
//! Each registered automation owns one [`RunState`]; the engine calls
//! [`RunState::dispatch`] on every (trigger + conditions-passed) event.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::{Mutex as AsyncMutex, Semaphore};

use homecore::HomeCore;

use crate::action::ExecutionContext;
use crate::automation::{Automation, RunMode};

/// Per-automation runtime state backing the run-mode dispatch.
///
/// Cheap to clone (all fields are `Arc`); the engine clones it into each
/// spawned run so the machinery (abort handle, queue mutex, semaphore) is
/// shared across all triggers of the same automation.
#[derive(Clone)]
pub struct RunState {
    /// `Single`/`IgnoreFirst` re-entrancy guard (ADR-161 §A5).
    running: Arc<AtomicBool>,
    /// `Restart`: handle to the currently-running action task, so a new
    /// trigger can abort it before starting a fresh one.
    current: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
    /// `Queued`: serializes runs in arrival order (one at a time, FIFO via
    /// fair async mutex acquisition).
    queue_lock: Arc<AsyncMutex<()>>,
    /// `max: N` (and bounded `Parallel`): caps concurrent runs at N.
    /// `None` when no cap applies.
    semaphore: Option<Arc<Semaphore>>,
}

impl RunState {
    /// Build run-state for an automation, sizing the concurrency semaphore
    /// from its `max:` field (only meaningful for `Queued`/`Parallel`).
    pub fn new(automation: &Automation) -> Self {
        let semaphore = automation
            .max
            .filter(|n| *n > 0)
            .map(|n| Arc::new(Semaphore::new(n)));
        Self {
            running: Arc::new(AtomicBool::new(false)),
            current: Arc::new(Mutex::new(None)),
            queue_lock: Arc::new(AsyncMutex::new(())),
            semaphore,
        }
    }

    /// Dispatch one trigger for `automation` according to its `RunMode`.
    /// Honors Single re-entrancy, Restart cancel-and-replace, Queued
    /// serialization, and `max:` concurrency capping.
    pub fn dispatch(&self, hc: &HomeCore, automation: Arc<Automation>) {
        match automation.mode {
            RunMode::Single | RunMode::IgnoreFirst => self.dispatch_single(hc, automation),
            RunMode::Restart => self.dispatch_restart(hc, automation),
            RunMode::Queued => self.dispatch_queued(hc, automation),
            RunMode::Parallel => self.dispatch_parallel(hc, automation),
        }
    }

    /// `Single`: skip if a run is already in flight; clear the flag on done.
    fn dispatch_single(&self, hc: &HomeCore, automation: Arc<Automation>) {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return; // already running — skip re-entrant trigger.
        }
        let hc = hc.clone();
        let running = Arc::clone(&self.running);
        tokio::spawn(async move {
            run_actions(&hc, &automation).await;
            running.store(false, Ordering::SeqCst);
        });
    }

    /// `Restart`: abort the in-flight run (if any), then start a fresh one
    /// and record its abort handle.
    fn dispatch_restart(&self, hc: &HomeCore, automation: Arc<Automation>) {
        // Timer and event triggers can dispatch concurrently. Keep cancellation
        // and replacement in one critical section so no abort handle is lost.
        let mut current = self.current.lock().unwrap();
        if let Some(prev) = current.take() {
            prev.abort();
        }
        let hc = hc.clone();
        let handle = tokio::spawn(async move {
            run_actions(&hc, &automation).await;
        });
        *current = Some(handle.abort_handle());
    }

    /// `Queued`: serialize via the per-automation async mutex. Each trigger
    /// spawns a task that waits its turn, so all triggers run in arrival
    /// order, one at a time — nothing is dropped.
    fn dispatch_queued(&self, hc: &HomeCore, automation: Arc<Automation>) {
        let hc = hc.clone();
        let lock = Arc::clone(&self.queue_lock);
        let sem = self.semaphore.clone();
        tokio::spawn(async move {
            // Optional `max:` cap still applies on top of serialization.
            let _permit = match &sem {
                Some(s) => Some(s.acquire().await.expect("semaphore not closed")),
                None => None,
            };
            let _guard = lock.lock().await; // FIFO turn — sequential execution.
            run_actions(&hc, &automation).await;
        });
    }

    /// `Parallel`: spawn on every trigger, capped at `max:` if set.
    fn dispatch_parallel(&self, hc: &HomeCore, automation: Arc<Automation>) {
        let hc = hc.clone();
        let sem = self.semaphore.clone();
        tokio::spawn(async move {
            let _permit = match &sem {
                Some(s) => Some(s.acquire().await.expect("semaphore not closed")),
                None => None,
            };
            run_actions(&hc, &automation).await;
        });
    }
}

/// Execute an automation's action sequence once.
async fn run_actions(hc: &HomeCore, automation: &Automation) {
    let mut exec_ctx = ExecutionContext::new(hc.clone(), automation.id.clone());
    for action in &automation.action {
        if let Err(e) = action.execute(&mut exec_ctx).await {
            eprintln!(
                "[homecore-automation] action error in {}: {e}",
                automation.id
            );
            break;
        }
    }
}
