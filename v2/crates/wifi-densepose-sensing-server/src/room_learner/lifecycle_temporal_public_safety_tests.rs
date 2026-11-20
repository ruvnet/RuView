//! Reusable actual actor controls over file copies of a genuine producer's
//! Complete/qualified model. No decoded DATA is turned into a permission.
use super::*;
use std::sync::Condvar;

fn branch(source: &Path, name: &str) -> std::path::PathBuf {
    let dir = tempfile::Builder::new()
        .prefix(&format!("SYNTHETIC-c4c-{name}-"))
        .tempdir()
        .unwrap()
        .keep();
    copy_private(source, &dir);
    dir
}
fn profile_root(root: &Path) -> std::path::PathBuf {
    root.join(&fixture::context().owner_scope)
        .join("SensingServer")
}
fn artifact_root(root: &Path) -> std::path::PathBuf {
    profile_root(root).join("room-learner")
}
fn file_clock(root: &Path) -> u64 {
    // Test clock candidate only. Actual owner/current AEAD cold restore and
    // immutable metadata loaders must authenticate all data before admission.
    fn visit(root: &Path, at: &mut u64) {
        for e in std::fs::read_dir(root).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                visit(&p, at)
            } else {
                let name = p.file_name().unwrap().to_string_lossy();
                if name.starts_with("slot-") && name.ends_with(".bin") {
                    let b = std::fs::read(&p).unwrap();
                    let n = u32::from_be_bytes(b[8..12].try_into().unwrap()) as usize;
                    let v: Value = serde_json::from_slice(&b[12..12 + n]).unwrap();
                    *at = (*at).max(v["receiver_ns"].as_u64().unwrap());
                } else if name.starts_with("evaluation-") {
                    let v: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
                    *at = (*at).max(
                        v["body"]["evaluated_at_receiver_ns"]
                            .as_str()
                            .unwrap()
                            .parse()
                            .unwrap(),
                    );
                } else if name.starts_with("assessment-") {
                    let v: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
                    *at = (*at).max(
                        v["receipt"]["summary"]["assessed_at_unix_ms"]
                            .as_u64()
                            .unwrap()
                            * 1_000_000,
                    );
                } else if name == "model-selection-v1.json" {
                    let v: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
                    *at = (*at)
                        .max(v["receipt"]["selected_at_unix_ms"].as_u64().unwrap() * 1_000_000);
                }
            }
        }
    }
    let mut at = 0;
    visit(root, &mut at);
    at + 1_000_000_000
}
fn metadata_hashes(root: &Path) -> Vec<(String, String)> {
    use sha2::Digest;
    fn walk(base: &Path, path: &Path, out: &mut Vec<(String, String)>) {
        for e in std::fs::read_dir(path).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(base, &p, out)
            } else {
                out.push((
                    p.strip_prefix(base).unwrap().to_string_lossy().into(),
                    format!("{:x}", sha2::Sha256::digest(std::fs::read(p).unwrap())),
                ));
            }
        }
    }
    let mut out = vec![];
    walk(root, root, &mut out);
    out.sort();
    out
}
async fn terminal(rt: &RoomLearningRuntime) -> JobPhase {
    let start = Instant::now();
    loop {
        let state = rt.inner.lock().unwrap();
        let done = match state.capture.as_ref() {
            Some(CaptureWorkflow::Temporal(c)) => c.evidence_job.as_ref().is_some_and(|j| j.done()),
            _ => true,
        };
        if done && !busy(&state.job) {
            return state.job.phase;
        }
        drop(state);
        assert!(start.elapsed() < Duration::from_secs(90));
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}
async fn qualified(root: &Path) -> (Arc<RoomLearningRuntime>, TemporalSelectRequestV2) {
    let now = file_clock(root);
    let rt = Arc::new(fixture::runtime(root, now));
    restore_public(&rt, now).await;
    let status = rt.temporal_status_v2(rt.receiver_now_ns()).unwrap();
    let c: TemporalCaptureWire = serde_json::from_value(status["capture"].clone()).unwrap();
    let e: TemporalEvaluationWire = serde_json::from_value(status["evaluation"].clone()).unwrap();
    rt.temporal_qualify_v2(
        TemporalQualifyRequestV2 {
            schema: QUALIFY_REQUEST_SCHEMA.into(),
            expected: rt.mutation_guard().unwrap(),
            capture_generation_id: c.capture_generation_id.clone(),
            refs: e.refs.clone(),
        },
        rt.receiver_now_ns(),
    )
    .unwrap();
    assert_eq!(terminal(&rt).await, JobPhase::Ready);
    let s = rt.temporal_status_v2(rt.receiver_now_ns()).unwrap();
    assert_eq!(s["assessment"]["outcome"], "qualified");
    let request = TemporalSelectRequestV2 {
        schema: SELECT_REQUEST_SCHEMA.into(),
        expected: rt.mutation_guard().unwrap(),
        capture_generation_id: c.capture_generation_id,
        refs: TemporalQualificationRefsWire {
            evaluation: e.refs,
            qualification_sha256: s["assessment"]["qualification_sha256"]
                .as_str()
                .unwrap()
                .into(),
        },
    };
    (rt, request)
}
fn selected(root: &Path) -> Arc<RoomLearningRuntime> {
    let rt = Arc::new(fixture::runtime(root, file_clock(root)));
    assert!(rt.inner.lock().unwrap().capture.is_none());
    assert!(rt.inner.lock().unwrap().selection.temporal.is_some());
    rt
}
fn deselect(rt: &RoomLearningRuntime) -> TemporalDeselectRequestV2 {
    let identity = rt
        .inner
        .lock()
        .unwrap()
        .selection
        .temporal
        .as_ref()
        .unwrap()
        .stored
        .identity();
    TemporalDeselectRequestV2 {
        schema: DESELECT_REQUEST_SCHEMA.into(),
        model_kind: WholeTemporalKind::ManagedTemporalV3,
        expected: rt.mutation_guard().unwrap(),
        selection_sha256: identity.selection_sha256,
        selection_revision: identity.selection_revision,
    }
}
struct Release {
    state: Arc<(Mutex<bool>, Condvar)>,
}
impl Release {
    fn new() -> Self {
        Self {
            state: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }
    fn open(&self) {
        *self.state.0.lock().unwrap() = true;
        self.state.1.notify_all();
    }
}
impl Drop for Release {
    fn drop(&mut self) {
        self.open();
    }
}

pub(super) async fn run(preselect: &Path, selected_source: &Path) {
    // Restore must never advertise future stored evaluation as Ready. The
    // candidate clock is deliberately ONLY the older authenticated raw floor.
    let root = branch(preselect, "future-evaluation");
    let mut raw = 0;
    fn raw_clock(root: &Path, raw: &mut u64) {
        for e in std::fs::read_dir(root).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                raw_clock(&p, raw)
            } else if p
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("slot-")
            {
                let b = std::fs::read(p).unwrap();
                let n = u32::from_be_bytes(b[8..12].try_into().unwrap()) as usize;
                let h: Value = serde_json::from_slice(&b[12..12 + n]).unwrap();
                *raw = (*raw).max(h["receiver_ns"].as_u64().unwrap());
            }
        }
    }
    raw_clock(&root, &mut raw);
    let rt = Arc::new(fixture::runtime(&root, raw + 1));
    let prior = metadata_hashes(&root);
    let s = rt.temporal_status_v2(raw + 1).unwrap();
    let id = s["recovery"]["capture_generation_id"].as_str().unwrap();
    rt.temporal_restore_v2(
        &rt.mutation_guard().unwrap(),
        &fixture::context(),
        id,
        raw + 1,
    )
    .unwrap();
    assert_eq!(terminal(&rt).await, JobPhase::Failed);
    assert_eq!(
        rt.inner.lock().unwrap().job.error.as_deref(),
        Some("temporal_evaluation_stored_clock_reversed")
    );
    assert_eq!(metadata_hashes(&root), prior);
    drop(rt);
    println!("C4C_ACTOR_SAFETY future_evaluation_refused=true original_bytes=true");

    // The actual current source/job observes expiry AFTER staged file fsync,
    // immediately before authoritative pointer rename. No pointer is installed.
    for cancel in [false, true] {
        let root = branch(
            preselect,
            if cancel {
                "preinstall-cancel"
            } else {
                "preinstall-expiry"
            },
        );
        let (rt, request) = qualified(&root).await;
        let expiry = capture(&rt, rt.receiver_now_ns())
            .original_deadline
            .expires_at_unix_ms
            * 1_000_000;
        let weak = Arc::downgrade(&rt);
        let called = Arc::new(AtomicBool::new(false));
        let seen = called.clone();
        *rt.temporal_evidence_io_hook.lock().unwrap() = Some(Arc::new(move |stage| {
            if stage == "selection_before_pointer_install" {
                let owner = weak.upgrade().unwrap();
                let state = owner.inner.lock().unwrap();
                if cancel {
                    state.job.cancel.store(true, Ordering::Release)
                } else {
                    let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
                        panic!()
                    };
                    c.evidence_job.as_ref().unwrap().observe_now(expiry);
                }
                seen.store(true, Ordering::Release);
            }
            Ok(())
        }));
        assert!(rt
            .temporal_select_v2(request, rt.receiver_now_ns())
            .is_err());
        assert!(called.load(Ordering::Acquire));
        assert!(rt.inner.lock().unwrap().selection.temporal.is_none());
        assert!(!artifact_root(&root)
            .join("model-selection-v1.json")
            .exists());
        assert!(!std::fs::read_dir(artifact_root(&root)).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("model-selection-v1.tmp-")));
        drop(rt);
    }
    println!("C4C_ACTOR_SAFETY staged_expiry_and_cancel_refused=true no_pointer_or_staging=true");

    // A valid rename is the commit point even if the later directory barrier
    // or ACK fails. RAM remains unavailable; cold trusted DAG reconciles it.
    let root = branch(preselect, "select-after-rename");
    let (rt, request) = qualified(&root).await;
    *rt.temporal_evidence_io_hook.lock().unwrap() = Some(Arc::new(|stage| {
        if stage == "selection_after_pointer_rename" {
            Err("synthetic_postrename_sync_failure".into())
        } else {
            Ok(())
        }
    }));
    assert_eq!(
        rt.temporal_select_v2(request, rt.receiver_now_ns())
            .unwrap_err(),
        "temporal_selection_pointer_write_unknown"
    );
    assert!(rt.inner.lock().unwrap().selection.temporal.is_none());
    assert!(rt.inner.lock().unwrap().selection.error.is_some());
    let committed = rt
        .store
        .load_temporal_selection()
        .unwrap()
        .unwrap()
        .identity();
    drop(rt);
    let cold = selected(&root);
    assert_eq!(
        cold.inner
            .lock()
            .unwrap()
            .selection
            .temporal
            .as_ref()
            .unwrap()
            .stored
            .identity(),
        committed
    );
    drop(cold);
    println!("C4C_ACTOR_SAFETY select_postrename_unknown_cold_reconciled=true");

    let root = branch(selected_source, "deselect-after-rename");
    let rt = selected(&root);
    let identity = rt
        .inner
        .lock()
        .unwrap()
        .selection
        .temporal
        .as_ref()
        .unwrap()
        .stored
        .identity();
    *rt.temporal_evidence_io_hook.lock().unwrap() = Some(Arc::new(|stage| {
        if stage == "deselect_after_pointer_rename" {
            Err("synthetic_postrename_sync_failure".into())
        } else {
            Ok(())
        }
    }));
    assert_eq!(
        rt.temporal_deselect_v2(deselect(&rt), rt.receiver_now_ns())
            .unwrap_err(),
        "selection_pointer_write_unknown"
    );
    assert!(rt.inner.lock().unwrap().selection.error.is_some());
    assert_ne!(
        rt.temporal_selected_binary_v2(
            rt.receiver_now_ns(),
            Some(&fixture::context().room_digest),
            Some("synthetic-safety-boot")
        )
        .unwrap()["status"],
        "ready"
    );
    drop(rt);
    let cold = Arc::new(fixture::runtime(&root, file_clock(&root)));
    assert!(cold.inner.lock().unwrap().selection.temporal.is_none());
    assert!(cold
        .store
        .load_selection()
        .unwrap()
        .unwrap()
        .receipt
        .selected
        .is_none());
    assert_eq!(
        cold.store
            .load_selection()
            .unwrap()
            .unwrap()
            .receipt
            .revision,
        identity.selection_revision + 1
    );
    drop(cold);
    println!("C4C_ACTOR_SAFETY deselect_postrename_unknown_cold_off=true");

    // Real pending Select IO is drained before a simulated owner erasure ACK.
    let root = branch(preselect, "owner-erase-drain");
    let (rt, request) = qualified(&root).await;
    let release = Release::new();
    let held = release.state.clone();
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    *rt.temporal_evidence_io_hook.lock().unwrap() = Some(Arc::new(move |stage| {
        if stage == "selection_before_pointer_install" {
            entered_tx.send(()).unwrap();
            let mut open = held.0.lock().unwrap();
            while !*open {
                open = held.1.wait(open).unwrap();
            }
        }
        Ok(())
    }));
    let worker = rt.clone();
    let operation =
        std::thread::spawn(move || worker.temporal_select_v2(request, worker.receiver_now_ns()));
    entered_rx.recv_timeout(Duration::from_secs(15)).unwrap();
    let owner = rt.clone();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let cancellation = std::thread::spawn(move || {
        let r = owner.cancel_temporal_durability(&owner.mutation_guard().unwrap());
        tx.send(r).unwrap();
    });
    assert!(
        rx.recv_timeout(Duration::from_millis(100)).is_err(),
        "eraseACKescapedpendingactualstore"
    );
    release.open();
    assert!(operation.join().unwrap().is_err());
    rx.recv_timeout(Duration::from_secs(15)).unwrap().unwrap();
    cancellation.join().unwrap();
    drop(rt);
    let protected = profile_root(&root);
    std::fs::remove_dir_all(&protected).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!protected.exists(), "latewriterrecreatederasedowner");
    println!("C4C_ACTOR_SAFETY owner_erase_waited_real_store_and_no_recreation=true");

    // Learning of a corrupt current durable chain fences the SAME hot Arc;
    // successful exact recertification restores only that committed choice.
    let root = branch(selected_source, "hot-integrity");
    let rt = selected(&root);
    let arc = rt.inner.lock().unwrap().selection.temporal.clone().unwrap();
    let identity = arc.stored.identity();
    let feed = live_feed(&rt);
    current_ready(&rt, &feed).await;
    fn model_file(root: &Path, sha: &str) -> Option<std::path::PathBuf> {
        for e in std::fs::read_dir(root).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                if let Some(v) = model_file(&p, sha) {
                    return Some(v);
                }
            } else if p.file_name().unwrap().to_string_lossy() == format!("{sha}.json") {
                return Some(p);
            }
        }
        None
    }
    let file = model_file(&root, &identity.model_sha256).unwrap();
    let bytes = std::fs::read(&file).unwrap();
    std::fs::write(&file, b"corrupt-owned-model-control").unwrap();
    let snapshot = rt
        .temporal_selection_v2(
            rt.receiver_now_ns(),
            Some(&fixture::context().room_digest),
            Some("synthetic-safety-boot"),
        )
        .unwrap();
    assert_eq!(
        snapshot["model_selection"]["current"]["status"],
        "unavailable"
    );
    assert!(arc.invalid.load(Ordering::Acquire));
    assert_eq!(
        rt.temporal_selected_binary_v2(
            rt.receiver_now_ns(),
            Some(&fixture::context().room_digest),
            Some("synthetic-safety-boot")
        )
        .unwrap()["status"],
        "unavailable"
    );
    std::fs::write(&file, bytes).unwrap();
    let snapshot = rt
        .temporal_selection_v2(
            rt.receiver_now_ns(),
            Some(&fixture::context().room_digest),
            Some("synthetic-safety-boot"),
        )
        .unwrap();
    assert_eq!(snapshot["model_selection"]["current"]["status"], "ready");
    assert!(!arc.invalid.load(Ordering::Acquire));
    assert!(Arc::ptr_eq(
        &arc,
        rt.inner
            .lock()
            .unwrap()
            .selection
            .temporal
            .as_ref()
            .unwrap()
    ));
    drop(feed);
    drop(rt);
    println!("C4C_ACTOR_SAFETY deep_failure_fences_hot_arc_exact_recert_same_arc=true");
}
