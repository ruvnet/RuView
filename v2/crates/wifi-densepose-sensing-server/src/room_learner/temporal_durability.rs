//! Private, bounded serial persistence executor. No packet queue or authority issuer.
use super::*;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::Instant;

pub(super) struct CommitRequest {
    pub id: u64,
    pub saved_receiver_ns: u64,
    pub snapshot: checkpoint::Snapshot,
    pub descriptor: RecoveryDescriptor,
    pub expected: Option<TemporalToken>,
    pub slot: bool,
    pub rows: usize,
    pub seals: usize,
}
pub(super) struct CommitAck {
    pub id: u64,
    pub slot: bool,
    pub rows: usize,
    pub seals: usize,
    pub start_ticket: bool,
    pub completed_at_ns: u64,
    pub outcome: Result<Committed, String>,
}
pub(super) struct Committed {
    pub token: TemporalToken,
    pub bytes: Arc<Vec<u8>>,
    pub durable_sha256: String,
    pub rows_sha256: String,
    pub row_prefix_sha256: String,
    pub restored: Option<Box<TemporalWorkflow>>,
    pub continued:Option<full_run::VerifiedContinuedSession>,
    pub full_hashes:Option<full_run::FullPrefixHashes>,
}
#[cfg(test)]
#[derive(Clone, Copy, Default)]
pub(crate) struct ReclamationStats {
    pub has_ack: bool,
    pub old_prefix_bytes: usize,
    pub old_prefix_last_owner: bool,
    pub raw_tail_rows: usize,
}
#[derive(Default)]
pub(crate) struct DeferredReclamation {
    ack: Option<CommitAck>,
    pending: Option<PendingMeta>,
    previous_bytes: Option<Arc<Vec<u8>>>,
    previous_workflow: Option<Box<TemporalWorkflow>>,
    fences: Option<AssemblerCheckpoint>,
    #[cfg(test)]
    pub probe: Option<Arc<dyn Fn(ReclamationStats) + Send + Sync>>,
}
impl DeferredReclamation {
    pub(crate) fn has_ack(&self) -> bool {
        self.ack.is_some()
    }
}
impl Drop for DeferredReclamation {
    fn drop(&mut self) {
        // Callers declare this value BEFORE Runtime.inner's MutexGuard, so
        // return/error/unwind unlocks before these owning payloads are freed.
        #[cfg(test)]
        if self.has_ack() {
            if let Some(probe) = &self.probe {
                probe(ReclamationStats {
                    has_ack: true,
                    old_prefix_bytes: self.previous_bytes.as_ref().map_or(0, |b| b.len()),
                    old_prefix_last_owner: self
                        .previous_bytes
                        .as_ref()
                        .is_some_and(|b| Arc::strong_count(b) == 1),
                    raw_tail_rows: self.previous_workflow.as_ref().map_or(0, |w| {
                        w.session.rows.len().saturating_sub(w.durable_windows)
                    }),
                });
            }
        }
        drop(self.ack.take());
        drop(self.previous_workflow.take());
        drop(self.pending.take());
        drop(self.previous_bytes.take());
        drop(self.fences.take());
    }
}
enum Work {
    Commit(CommitRequest),
    Recertify {
        id: u64,
        expected: TemporalToken,
        descriptor: RecoveryDescriptor,
        rows: usize,
        seals: usize,
        start_ticket: bool,
        records: checkpoint::SharedRecords,
        model:Option<full_run::FrozenModelVerifier>,
    },
    Repair {
        id: u64,
    },
    Stop,
}
struct Pending {
    intent: TemporalIntent,
    prefix: checkpoint::ValidatedTemporalPrefix,
    id: u64,
    slot: bool,
    rows: usize,
    seals: usize,
    records: checkpoint::SharedRecords,
}
struct Shared {
    cancelled: AtomicBool,
    busy: AtomicBool,
    now_ns: AtomicU64,
    anchor_ns: u64,
    origin: Instant,
    #[cfg(test)]
    lose_next_ack: AtomicBool,
    #[cfg(test)]
    ack_queued: AtomicBool,
    #[cfg(test)]
    corrupt_ack_id: AtomicBool,
    #[cfg(test)]
    hold: Mutex<Option<(mpsc::SyncSender<()>, mpsc::Receiver<()>)>>,
    #[cfg(test)]
    hold_after_prepare: Mutex<Option<(mpsc::SyncSender<()>, mpsc::Receiver<()>)>>,
}
impl Shared {
    fn now(&self) -> Result<u64, String> {
        let elapsed: u64 = self
            .origin
            .elapsed()
            .as_nanos()
            .try_into()
            .map_err(|_| "temporal_writer_clock_overflow")?;
        Ok(self.now_ns.load(Ordering::Acquire).max(
            self.anchor_ns
                .checked_add(elapsed)
                .ok_or("temporal_writer_clock_overflow")?,
        ))
    }
    fn allowed(&self, descriptor: &RecoveryDescriptor) -> Result<u64, String> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err("temporal_writer_cancelled".into());
        }
        let now = self.now()?;
        if now / 1_000_000 >= descriptor.expires_at_ms {
            return Err("temporal_writer_original_deadline_expired".into());
        }
        Ok(now)
    }
}
pub(crate) struct Writer {
    send: mpsc::SyncSender<Work>,
    receive: Mutex<mpsc::Receiver<CommitAck>>,
    shared: Arc<Shared>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}
impl Writer {
    pub fn new(store: Arc<RecoveryStore>, now: u64) -> Result<Self, String> {
        let (send, requests) = mpsc::sync_channel(1);
        let (replies, receive) = mpsc::sync_channel(1);
        let shared = Arc::new(Shared {
            cancelled: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            now_ns: AtomicU64::new(now),
            anchor_ns: now,
            origin: Instant::now(),
            #[cfg(test)]
            lose_next_ack: AtomicBool::new(false),
            #[cfg(test)]
            ack_queued: AtomicBool::new(false),
            #[cfg(test)]
            corrupt_ack_id: AtomicBool::new(false),
            #[cfg(test)]
            hold: Mutex::new(None),
            #[cfg(test)]
            hold_after_prepare: Mutex::new(None),
        });
        let state = shared.clone();
        let thread = std::thread::Builder::new()
            .name("ruview-temporal-durable".into())
            .spawn(move || {
                let mut pending: Option<Pending> = None;
                while let Ok(work) = requests.recv() {
                    let (id, slot, rows, seals, start_ticket, result) = match work {
                        Work::Stop => break,
                        Work::Commit(mut request) => {
                            let meta = (request.id, request.slot, request.rows, request.seals);
                            let result = (|| {
                                state.allowed(&request.descriptor)?;
                                #[cfg(test)]
                                if let Some((entered, release)) = state
                                    .hold
                                    .lock()
                                    .map_err(|_| "temporal_writer_test_lock")?
                                    .take()
                                {
                                    let _ = entered.send(());
                                    let _ = release.recv();
                                }
                                state.allowed(&request.descriptor)?;
                                if pending.is_some() {
                                    return Err("temporal_writer_exact_repair_required".into());
                                }
                                let records = checkpoint::snapshot_records(&request.snapshot);
                                let full_permit=checkpoint::snapshot_full_permit(&mut request.snapshot);
                                let bytes = checkpoint::encode_snapshot(request.snapshot)?;
                                let at = request.saved_receiver_ns;
                                let mut prefix =
                                    checkpoint::validated_prefix(&bytes, &request.descriptor, at)?;
                                if let Some(permit)=full_permit{prefix.attach_full_grant(permit.encoded(&bytes))?;}
                                prefix.verify_full_artifact()?;
                                let intent = store.prepare_temporal(
                                    request.expected.as_ref(),
                                    &prefix,
                                    state.allowed(&request.descriptor)?,
                                )?;
                                pending = Some(Pending {
                                    intent,
                                    prefix,
                                    id: meta.0,
                                    slot: meta.1,
                                    rows: meta.2,
                                    seals: meta.3,
                                    records,
                                });
                                #[cfg(test)]
                                if let Some((entered, release)) = state.hold_after_prepare.lock().map_err(|_|"temporal_writer_test_lock")?.take() {
                                    let _ = entered.send(());
                                    let _ = release.recv();
                                }
                                let p = pending.as_ref().unwrap();
                                let token = store.publish_temporal(
                                    &p.intent,
                                    Some(&p.prefix),
                                    state.allowed(p.prefix.descriptor())?,
                                )?;
                                #[cfg(test)]
                                if state.lose_next_ack.swap(false, Ordering::AcqRel) {
                                    return Err("synthetic_lost_publish_ack".into());
                                }
                                readback(&store, &state, p, token)
                            })();
                            if result.is_ok() {
                                pending = None;
                            }
                            (meta.0, meta.1, meta.2, meta.3, false, result)
                        }
                        Work::Recertify {
                            id,
                            expected,
                            descriptor,
                            rows,
                            seals,
                            start_ticket,
                            records,model,
                        } => {
                            let result = (|| {
                                state.allowed(&descriptor)?;
                                #[cfg(test)]
                                if let Some((entered, release)) = state
                                    .hold
                                    .lock()
                                    .map_err(|_| "temporal_writer_test_lock")?
                                    .take()
                                {
                                    let _ = entered.send(());
                                    let _ = release.recv();
                                }
                                if pending.is_some() {
                                    return Err("temporal_writer_exact_repair_required".into());
                                }
                                if let Some(model)=&model{model.verify()?;}
                                let recertified = store
                                    .recertify_temporal(&expected, state.allowed(&descriptor)?)?;
                                let restored = checkpoint::decode_shared(
                                    recertified.prefix.bytes(),
                                    recertified.prefix.descriptor(),
                                    state.allowed(&descriptor)?,
                                    &records,
                                )?;
                                if restored.full_run.as_ref().is_some_and(|m|m.frozen.is_some())&&model.is_none(){return Err("temporal_current_model_recertification_required".into());}
                                let continued=match &model{Some(m)=>{m.check_workflow_data(&restored)?;m.verify()?;m.continued_witness(&restored,&recertified.token)?},None=>None};
                                let full_hashes=if restored.full_run.is_some(){Some(full_run::FullPrefixHashes::from_workflow(&restored)?)}else{None};
                                let rows_sha256 = hash(&restored.session.rows)?;
                                let row_prefix_sha256 = hash(&(
                                    &restored.session.rows,
                                    &restored.session.evidence,
                                    &restored.ledger.spans,
                                ))?;
                                state.allowed(&descriptor)?;
                                Ok(Committed {
                                    token: recertified.token,
                                    durable_sha256: bytes_hash(recertified.prefix.bytes()),
                                    bytes: Arc::new(recertified.prefix.bytes().to_vec()),
                                    rows_sha256,
                                    row_prefix_sha256,continued,full_hashes,
                                    restored: Some(Box::new(restored)),
                                })
                            })();
                            (id, false, rows, seals, start_ticket, result)
                        }
                        Work::Repair { id } => {
                            let Some(p) = pending.as_ref() else {
                                let _ = replies.send(CommitAck {
                                    id,
                                    slot: false,
                                    rows: 0,
                                    seals: 0,
                                    start_ticket: false,
                                    completed_at_ns: state.now().unwrap_or(0),
                                    outcome: Err("temporal_writer_pending_intent_missing".into()),
                                });
                                continue;
                            };
                            let meta = (p.id, p.slot, p.rows, p.seals);
                            let result = (|| {
                                if id != p.id {
                                    return Err("temporal_writer_repair_ticket_changed".into());
                                }
                                let token = store.repair_same_temporal(
                                    &p.intent,
                                    Some(&p.prefix),
                                    state.allowed(p.prefix.descriptor())?,
                                )?;
                                readback(&store, &state, p, token)
                            })();
                            if result.is_ok() {
                                pending = None;
                            }
                            (meta.0, meta.1, meta.2, meta.3, false, result)
                        }
                    };
                    // Capacity one and only one accepted transaction. The receiver
                    // must consume this result before another request is admitted.
                    #[cfg(test)]
                    let id = if state.corrupt_ack_id.swap(false, Ordering::AcqRel) {
                        id.wrapping_add(1)
                    } else {
                        id
                    };
                    if replies
                        .send(CommitAck {
                            id,
                            slot,
                            rows,
                            seals,
                            start_ticket,
                            completed_at_ns: state.now().unwrap_or(0),
                            outcome: result,
                        })
                        .is_err()
                    {
                        break;
                    }
                    #[cfg(test)]
                    state.ack_queued.store(true, Ordering::Release);
                    if state.cancelled.load(Ordering::Acquire) {
                        break;
                    }
                }
            })
            .map_err(|_| "temporal_writer_spawn_failed")?;
        Ok(Self {
            send,
            receive: Mutex::new(receive),
            shared,
            thread: Mutex::new(Some(thread)),
        })
    }
    pub fn observe_now(&self, now: u64) {
        self.shared.now_ns.fetch_max(now, Ordering::AcqRel);
    }
    pub fn submit(&self, request: CommitRequest) -> Result<(), String> {
        if self.shared.cancelled.load(Ordering::Acquire) {
            return Err("temporal_writer_cancelled".into());
        }
        if self
            .shared
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("temporal_storage_busy".into());
        }
        #[cfg(test)]
        self.shared.ack_queued.store(false, Ordering::Release);
        if self.send.try_send(Work::Commit(request)).is_err() {
            self.shared.busy.store(false, Ordering::Release);
            return Err("temporal_writer_unavailable".into());
        }
        Ok(())
    }
    fn recertify(
        &self,
        id: u64,
        expected: TemporalToken,
        descriptor: RecoveryDescriptor,
        rows: usize,
        seals: usize,
        start_ticket: bool,
        records: checkpoint::SharedRecords,
        model:Option<full_run::FrozenModelVerifier>,
    ) -> Result<(), String> {
        if self.shared.cancelled.load(Ordering::Acquire)
            || self
                .shared
                .busy
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return Err("temporal_storage_busy".into());
        }
        #[cfg(test)]
        self.shared.ack_queued.store(false, Ordering::Release);
        if self
            .send
            .try_send(Work::Recertify {
                id,
                expected,
                descriptor,
                rows,
                seals,
                start_ticket,
                records,model,
            })
            .is_err()
        {
            self.shared.busy.store(false, Ordering::Release);
            return Err("temporal_writer_unavailable".into());
        }
        Ok(())
    }
    pub fn poll(&self) -> Result<Option<CommitAck>, String> {
        match self
            .receive
            .lock()
            .map_err(|_| "temporal_writer_reply_lock")?
            .try_recv()
        {
            Ok(ack) => {
                self.shared.busy.store(false, Ordering::Release);
                Ok(Some(ack))
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(_) => Err("temporal_writer_disconnected".into()),
        }
    }
    pub fn repair(&self, id: u64) -> Result<(), String> {
        if self
            .shared
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("temporal_storage_busy".into());
        }
        #[cfg(test)]
        self.shared.ack_queued.store(false, Ordering::Release);
        if self.send.try_send(Work::Repair { id }).is_err() {
            self.shared.busy.store(false, Ordering::Release);
            return Err("temporal_writer_unavailable".into());
        }
        Ok(())
    }
    pub fn cancel(&self) {
        self.shared.cancelled.store(true, Ordering::Release);
        let _ = self.send.try_send(Work::Stop);
    }
    /// Caller MUST be outside Runtime.inner. Finishing a publication cannot be
    /// undone; this barrier prevents an old worker from publishing after erase.
    pub fn drain(&self) -> Result<(), String> {
        self.cancel();
        let joined = if let Some(thread) = self
            .thread
            .lock()
            .map_err(|_| "temporal_writer_join_lock")?
            .take()
        {
            thread
                .join()
                .map_err(|_| "temporal_writer_panicked".to_string())
        } else {
            Ok(())
        };
        // A revoked/failed capture may never poll its final ACK. Release its
        // large serialized/candidate buffers here, not when Core drops inner.
        if let Ok(receiver) = self.receive.lock() {
            while let Ok(ack) = receiver.try_recv() {
                drop(ack);
            }
        }
        joined
    }
    #[cfg(test)]
    pub fn ack_is_queued(&self) -> bool {
        self.shared.ack_queued.load(Ordering::Acquire)
    }
    #[cfg(test)]
    pub fn corrupt_next_ack_id(&self) {
        self.shared.corrupt_ack_id.store(true, Ordering::Release);
    }
    #[cfg(test)]
    pub fn lose_next_ack(&self) {
        self.shared.lose_next_ack.store(true, Ordering::Release);
    }
    #[cfg(test)]
    pub fn hold_next_after_prepare(&self, entered:mpsc::SyncSender<()>, release:mpsc::Receiver<()>) {
        *self.shared.hold_after_prepare.lock().unwrap()=Some((entered,release));
    }
    #[cfg(test)]
    pub fn hold_next(&self, entered: mpsc::SyncSender<()>, release: mpsc::Receiver<()>) {
        *self.shared.hold.lock().unwrap() = Some((entered, release));
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        self.shared.cancelled.store(true, Ordering::Release);
        let _ = self.send.try_send(Work::Stop);
    }
}
fn readback(
    store: &RecoveryStore,
    state: &Shared,
    p: &Pending,
    token: TemporalToken,
) -> Result<Committed, String> {
    let now = state.allowed(p.prefix.descriptor())?;
    let loaded = store
        .load_temporal(now)?
        .ok_or("temporal_readback_missing")?;
    if loaded.token != token || loaded.prefix.as_ref().map(|x| x.bytes()) != Some(p.prefix.bytes())
    {
        return Err("temporal_readback_changed".into());
    }
    let restored =
        checkpoint::decode_shared(p.prefix.bytes(), p.prefix.descriptor(), now, &p.records)?;
    p.prefix.verify_full_artifact()?;
    let continued=p.prefix.continued_witness(&restored,&token)?;
    let full_hashes=if restored.full_run.is_some(){Some(full_run::FullPrefixHashes::from_workflow(&restored)?)}else{None};
    let rows_sha256 = hash(&restored.session.rows)?;
    let row_prefix_sha256 = hash(&(
        &restored.session.rows,
        &restored.session.evidence,
        &restored.ledger.spans,
    ))?;
    state.allowed(p.prefix.descriptor())?;
    Ok(Committed {
        token,
        durable_sha256: bytes_hash(p.prefix.bytes()),
        bytes: Arc::new(p.prefix.bytes().to_vec()),
        rows_sha256,
        row_prefix_sha256,continued,full_hashes,
        restored: Some(Box::new(restored)),
    })
}

#[derive(Clone)]
pub(super) struct PendingMeta {
    id: u64,
    slot: bool,
    rows: usize,
    seals: usize,
    pub start_ticket: bool,
    predecessor: Option<TemporalToken>,
    row_lineage: Vec<SharedRecord<CaptureCohort>>,
    evidence_lineage: Vec<SharedRecord<WindowEvidence>>,
}
pub(super) struct AsyncState {
    pub writer: Arc<Writer>,
    pub pending: Option<PendingMeta>,
    next_id: u64,
    pub busy_pause: bool,
    repairing: bool,
    durable_sha256: Option<String>,
    pub rows_sha256: Option<String>,
    pub row_prefix_sha256: Option<String>,
    pub error: Option<String>,
    pub accepted_seals: usize,
    pub accepted_held_out_seals:usize,
    pub full_hashes:Option<full_run::FullPrefixHashes>,
    // At most the same ONE completion, awaiting an actually fresh receiver
    // timestamp for new full-run authority. Never a packet or work queue.
    deferred_ack:Option<CommitAck>,
}
impl AsyncState {
    pub fn new(writer: Arc<Writer>) -> Self {
        Self {
            writer,
            pending: None,
            next_id: 0,
            busy_pause: false,
            repairing: false,
            durable_sha256: None,
            rows_sha256: None,
            row_prefix_sha256: None,
            error: None,
            accepted_seals: 0,accepted_held_out_seals:0,full_hashes:None,
            deferred_ack:None,
        }
    }
    pub fn seed_restored(&mut self, w: &TemporalWorkflow) -> Result<(), String> {
        self.durable_sha256 = w.durable.as_ref().map(|b| bytes_hash(b));
        self.rows_sha256 = Some(hash(&w.session.rows)?);
        self.row_prefix_sha256 = Some(hash(&(
            &w.session.rows,
            &w.session.evidence,
            &w.ledger.spans,
        ))?);
        self.accepted_seals = w.bridge.as_ref().map_or(0, |b| b.seals.len());
        self.accepted_held_out_seals=w.full_run.as_ref().map_or(0,|m|m.held_out_seals());
        self.full_hashes=if w.full_run.is_some(){Some(full_run::FullPrefixHashes::from_workflow(w)?)}else{None};
        Ok(())
    }
    fn id(&mut self) -> Result<u64, String> {
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or("temporal_writer_revision_exhausted")?;
        Ok(self.next_id)
    }
}
impl TemporalWorkflow {
    pub(super) fn durable_digest(&self) -> Option<String> {
        self.async_state
            .as_ref()
            .map(|a| a.durable_sha256.clone())
            .unwrap_or_else(|| self.durable.as_ref().map(|b| bytes_hash(b)))
    }
    pub(super) fn submit_async_commit(&mut self, slot: bool) -> Result<(), String> {
        let snapshot = checkpoint::snapshot(self)?;
        let state = self.async_state.as_mut().ok_or("temporal_async_required")?;
        let id = state.id()?;
        let seals = self.bridge.as_ref().map_or(0, |b| b.seals.len());
        let rows = self.session.rows.len();
        state.writer.observe_now(self.last_now_ns);
        state.writer.submit(CommitRequest {
            id,
            saved_receiver_ns: self.last_now_ns,
            snapshot,
            descriptor: self.descriptor.clone(),
            expected: self.commit_token.clone(),
            slot,
            rows,
            seals,
        })?;
        state.pending = Some(PendingMeta {
            id,
            slot,
            rows,
            seals,
            start_ticket: false,
            predecessor: self.commit_token.clone(),
            row_lineage: self.session.rows.clone(),
            evidence_lineage: self.session.evidence.clone(),
        });
        state.error = None;
        if slot {
            self.pending_window = None;
        }
        Ok(())
    }
    pub(super) fn request_async_start(&mut self, now: u64) -> Result<(), String> {
        self.check_clock(now)?;
        if !matches!(
            self.stage,
            Stage::DurableReady | Stage::PausedNeedsReconfirmation
        ) {
            return Err("temporal_start_barrier_state".into());
        }
        if self.session.rows.len() != self.durable_windows {
            return Err("temporal_unconfirmed_rows".into());
        }
        let records = checkpoint::workflow_records(self);
        let model=self.full_run.as_ref().and_then(|m|m.model_verifier());
        let state = self.async_state.as_mut().ok_or("temporal_async_required")?;
        if state.pending.is_some() || state.busy_pause {
            return Err("temporal_storage_busy".into());
        }
        let token = self
            .commit_token
            .clone()
            .ok_or("temporal_initial_barrier_unconfirmed")?;
        let id = state.id()?;
        let rows = self.durable_windows;
        let seals = self.bridge.as_ref().map_or(0, |b| b.seals.len());
        state.writer.observe_now(now);
        state.writer.recertify(
            id,
            token,
            self.descriptor.clone(),
            rows,
            seals,
            true,
            records,model,
        )?;
        state.pending = Some(PendingMeta {
            id,
            slot: false,
            rows,
            seals,
            start_ticket: true,
            predecessor: self.commit_token.clone(),
            row_lineage: self.session.rows.clone(),
            evidence_lineage: self.session.evidence.clone(),
        });
        state.error = None;
        Ok(())
    }
    pub(super) fn request_async_pause(&mut self, now: u64) -> Result<(), String> {
        self.check_clock(now)?;
        if self.stage != Stage::Observing {
            return Err("temporal_pause_state".into());
        }
        let records = checkpoint::workflow_record_prefix(self, self.durable_windows)?;
        let model=self.full_run.as_ref().and_then(|m|m.model_verifier());
        let state = self.async_state.as_mut().ok_or("temporal_async_required")?;
        state.busy_pause = true;
        state.error = Some("temporal_pause_pending_durability".into());
        if state.pending.is_none() {
            let id = state.id()?;
            let rows = self.durable_windows;
            let seals = state.accepted_seals;
            let token = self
                .commit_token
                .clone()
                .ok_or("temporal_initial_barrier_unconfirmed")?;
            state.writer.observe_now(now);
            state.writer.recertify(
                id,
                token,
                self.descriptor.clone(),
                rows,
                seals,
                false,
                records,model,
            )?;
            state.pending = Some(PendingMeta {
                id,
                slot: false,
                rows,
                seals,
                start_ticket: false,
                predecessor: self.commit_token.clone(),
                row_lineage: self.session.rows.clone(),
                evidence_lineage: self.session.evidence.clone(),
            });
        }
        self.stage = Stage::BlockedDurability;
        Ok(())
    }
    /// Only the exact current operation can admit a durable result. Live source
    /// floors, current custody and uncheckpointed rows are never replaced by an
    /// older normal ACK. Returns true only for a consumed one-use StartTicket.
    pub(super) fn poll_async(
        &mut self,
        now: u64,
        retired: &mut DeferredReclamation,
    ) -> Result<Option<(bool, u64)>, String> {
        let full_run=self.full_run.is_some();
        let Some(state) = self.async_state.as_mut() else {
            return Ok(None);
        };
        if retired.has_ack() {
            return Err("temporal_reclamation_already_occupied".into());
        }
        state.writer.observe_now(now);
        let next=if state.deferred_ack.is_some(){state.deferred_ack.take()}else{state.writer.poll()?};
        let Some(ack) = next else {
            return Ok(None);
        };
        if full_run && now<ack.completed_at_ns{
            state.deferred_ack=Some(ack);return Ok(None);
        }
        // Own the WHOLE response before any fallible validation. Rejected or
        // stale successful responses also contain a large plaintext buffer.
        retired.ack = Some(ack);
        let ack = retired.ack.as_mut().unwrap();
        let now = if full_run{now}else{now.max(ack.completed_at_ns)};
        retired.pending = Some(
            state
                .pending
                .clone()
                .ok_or("temporal_obsolete_writer_ack")?,
        );
        let pending = retired.pending.as_ref().unwrap();
        if ack.id != pending.id
            || ack.slot != pending.slot
            || ack.rows != pending.rows
            || ack.seals != pending.seals
            || ack.start_ticket != pending.start_ticket
            || self.commit_token != pending.predecessor
        {
            return Err("temporal_writer_ack_identity_changed".into());
        }
        if self.session.rows.len() < pending.rows
            || self.session.evidence.len() < pending.rows
            || self
                .session
                .rows
                .iter()
                .zip(&pending.row_lineage)
                .any(|(a, b)| !a.same_record(b))
            || self
                .session
                .evidence
                .iter()
                .zip(&pending.evidence_lineage)
                .any(|(a, b)| !a.same_record(b))
        {
            return Err("temporal_writer_ack_row_lineage_changed".into());
        }
        if now < self.last_now_ns || now / 1_000_000 >= self.descriptor.expires_at_ms {
            return Err("temporal_writer_ack_clock_or_expiry".into());
        }
        let committed = match &mut ack.outcome {
            Ok(result) => result,
            Err(error) => {
                state.error = Some(error.clone());
                if pending.start_ticket {
                    state.pending = None;
                    self.stage = Stage::PausedNeedsReconfirmation;
                } else {
                    self.stage = Stage::BlockedDurability;
                }
                return Err(error.clone());
            }
        };
        let candidate = committed
            .restored
            .as_ref()
            .ok_or("temporal_writer_restore_missing")?;
        if candidate.context() != self.context()
            || candidate.session.run_id != self.session.run_id
            || candidate.descriptor.id != self.descriptor.id
            || candidate.descriptor.expires_at_ms != self.descriptor.expires_at_ms
            || candidate.session.rows.len() != pending.rows
        {
            return Err("temporal_writer_ack_binding_changed".into());
        }
        if match (&self.full_run,&candidate.full_run) {
            (None,None)=>false,
            (Some(a),Some(b))=>a.policy.sha256()!=b.policy.sha256(),
            _=>true,
        } {return Err("temporal_writer_ack_continuation_changed".into());}
        let frozen_transition=self.prepare_frozen_ack(candidate,now)?;
        if let Some(continued)=&committed.continued{continued.check_owner(self,&committed.token)?;}
        let complete_transition=self.prepare_complete_ack(candidate,&committed.token)?;
        let candidate_held_out_seals=candidate.full_run.as_ref().map_or(0,|m|m.held_out_seals());
        let must_restore = self
            .async_state
            .as_ref()
            .is_some_and(|s| s.busy_pause || s.repairing) && frozen_transition.is_none() && complete_transition.is_none();
        if must_restore {
            retired.fences = Some(self.session.assembler.fence_checkpoint()?);
            let restored = committed.restored.as_mut().unwrap();
            let fences=retired.fences.as_ref().unwrap().clone().for_prefix(pending.rows)?;
            if let Some(mode)=&self.full_run{
                let budget=mode.policy.replay_fence_budget(&self.session.managed.as_ref().ok_or("managed_session_required")?.binding)?;
                restored.session.assembler.restore_full_run_prefix_fences(fences,pending.rows,&budget)?;
            }else{restored.session.assembler.restore_prefix_fences(fences,pending.rows)?;}
            restored.source_high = self.source_high.clone();
            restored.mapping = self.mapping.clone();
            restored.live_observed_ns = self.live_observed_ns;
            restored.last_now_ns = now;
            if let Some(continued)=&committed.continued{continued.install_raw(restored)?;}
            restored.stage = Stage::PausedNeedsReconfirmation;
            restored.set_current_disposition(bridge::Disposition::Paused)?;
        }
        // All fallible candidate checks finished while its ownership remained
        // in retired. From here transfer old owners, never destroy them here.
        let mut state = self.async_state.take().unwrap();
        state.pending = None;
        state.error = None;
        state.durable_sha256 = Some(committed.durable_sha256.clone());
        state.accepted_seals = pending.seals;
        state.accepted_held_out_seals=candidate_held_out_seals;
        state.full_hashes=committed.full_hashes.clone();
        state.rows_sha256 = Some(committed.rows_sha256.clone());
        state.row_prefix_sha256 = Some(committed.row_prefix_sha256.clone());
        if frozen_transition.is_some()||complete_transition.is_some(){state.busy_pause=false;state.repairing=false;}
        retired.previous_bytes = self.durable.take();
        self.commit_token = Some(committed.token.clone());
        self.durable = Some(committed.bytes.clone());
        self.durable_windows = pending.rows;
        if must_restore {
            let restored = committed.restored.as_mut().unwrap();
            restored.commit_token = self.commit_token.clone();
            restored.durable = self.durable.clone();
            let persist_pause = state.busy_pause;
            state.busy_pause = false;
            state.repairing = false;
            restored.async_state = Some(state);
            if let Some(old)=self.full_run.as_mut(){
                // Exact continuation presence was already validated above;
                // all fallible checks finished before moving live custody.
                let recovered=restored.full_run.as_mut().unwrap();
                recovered.owner=old.owner.take();
                recovered.local_start_not_before_ns=old.local_start_not_before_ns;
            }
            let replacement = *committed.restored.take().unwrap();
            retired.previous_workflow = Some(Box::new(std::mem::replace(self, replacement)));
            if persist_pause {
                self.descriptor.saved_at_ms = now / 1_000_000;
                self.submit_async_commit(false)?;
            }
        } else {
            self.async_state = Some(state);
        }
        if let Some(transition)=frozen_transition{self.apply_frozen_ack(transition);}
        if let Some(transition)=complete_transition{self.apply_complete_ack(transition);}
        if pending.start_ticket{if let Some(mode)=self.full_run.as_mut(){mode.local_start_not_before_ns=Some(now);}}
        self.last_now_ns = self.last_now_ns.max(now);
        Ok(Some((pending.start_ticket, now)))
    }
    pub(super) fn request_async_repair(&mut self, now: u64) -> Result<(), String> {
        self.check_clock(now)?;
        if self.stage != Stage::BlockedDurability {
            return Err("temporal_pending_repair_state".into());
        }
        let state = self.async_state.as_mut().ok_or("temporal_async_required")?;
        let pending = state
            .pending
            .as_ref()
            .ok_or("temporal_pending_intent_missing")?;
        state.writer.observe_now(now);
        state.writer.repair(pending.id)?;
        state.repairing = true;
        Ok(())
    }
}
