//! Read-only phone companion projection. Counts record capture, never accuracy.
use super::*;
use crate::room_learner::{capture::PhaseStatus,
    selection::{SelectedBinaryEvidence, ModelSelectionStatus}};

const MAX_SAFE: u64 = 9_007_199_254_740_991;
pub const PAIRED_PROGRESS_TTL_MS: u64 = 10_000;
#[derive(Debug, Clone, Serialize)]
pub struct PublishedProgressRoom { pub binding_digest: String, pub revision: u64 }
#[derive(Debug, Clone, Serialize)]
pub struct RecordedCondition { pub label: RoomLabel, pub train_recorded: usize, pub held_out_recorded: usize }
#[derive(Debug, Clone, Serialize)]
pub struct ProgressPhase {
    pub role: DataRole, pub label: RoomLabel, pub stage: &'static str,
    pub target_ms: u64, pub lead_in_remaining_ms: u64, pub capture_elapsed_ms: u64,
    pub representative_cohorts: usize, pub required_cohorts: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct ProgressSources { pub required: usize, pub ready: usize }
#[derive(Debug, Clone, Serialize)]
pub struct ProgressQualification { pub state: &'static str, pub receipt_sha256: Option<String>, pub model_sha256: Option<String> }
#[derive(Debug, Clone, Serialize)]
pub struct ProgressSelection { pub state: &'static str, pub selection_sha256: Option<String>, pub model_sha256: Option<String>, pub current_usable: bool }
#[derive(Debug, Clone, Serialize)]
pub struct PairedCalibrationProgress {
    pub schema: &'static str, pub success: bool, pub installation_id: String,
    /// Existing public calibration process identity, never the owner mutation guard.
    pub runtime_epoch: String, pub observed_at_unix_ms: u64, pub expires_at_unix_ms: u64,
    pub room: Option<PublishedProgressRoom>, pub context_match: bool,
    pub stage: &'static str, pub state: &'static str,
    pub conditions: Option<Vec<RecordedCondition>>, pub phase: Option<ProgressPhase>,
    pub sources: Option<ProgressSources>, pub qualification: ProgressQualification,
    pub selection: ProgressSelection, pub save_status: &'static str, pub blocker: Option<&'static str>,
}
fn bounded_text(value: &str) -> bool { !value.is_empty() && value.encode_utf16().count() <= 128 && !value.chars().any(char::is_control) }
fn digest(value: &str) -> bool { value.len()==64 && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) }
impl RoomLearningRuntime {
    /// Does not call owner status/checkpoint or advance workflow/model state.
    /// Artifact readback reuses existing owner-private store enforcement.
    /// Caller holds the published binding stable.
    pub fn paired_progress_snapshot(&self, installation_id: &str, runtime_epoch: &str,
        room: Option<PublishedProgressRoom>, externally_ready_sources: &[u8], ready_context_digest: Option<&str>, now_ns: u64,
    ) -> Result<PairedCalibrationProgress, String> {
        let observed = now_ns / 1_000_000;
        if !bounded_text(installation_id) || !bounded_text(runtime_epoch) || observed==0 || observed>MAX_SAFE-PAIRED_PROGRESS_TTL_MS
            || room.as_ref().is_some_and(|r| !digest(&r.binding_digest) || r.revision==0 || r.revision>MAX_SAFE)
        { return Err("progress identity/time unavailable".into()); }
        let state=self.inner.lock().map_err(|_| "learner state unavailable")?;
        state.require_legacy_kind()?;
        let mut result=PairedCalibrationProgress {
            schema:"ruview.room-learner.progress.v1",success:true,installation_id:installation_id.into(),runtime_epoch:runtime_epoch.into(),
            observed_at_unix_ms:observed,expires_at_unix_ms:observed+PAIRED_PROGRESS_TTL_MS,room,
            context_match:false,stage:"setup",state:"idle",conditions:None,phase:None,sources:None,
            qualification:ProgressQualification{state:"not_reported",receipt_sha256:None,model_sha256:None},
            selection:ProgressSelection{state:"not_reported",selection_sha256:None,model_sha256:None,current_usable:false},
            save_status:"not_reported",blocker:Some("room_not_published"),
        };
        let Some(published)=result.room.as_ref() else { return Ok(result); };
        let Some(capture)=state.legacy_capture() else { result.blocker=Some("learner_not_configured");return Ok(result); };
        let context=capture.context();
        if context.installation_id!=installation_id || context.room_digest!=published.binding_digest {
            result.stage="blocked";result.blocker=Some("context_mismatch");return Ok(result);
        }
        if ready_context_digest!=Some(context.digest().as_str()) {return Err("progress context changed".into());}
        if context.sources.is_empty() || context.sources.len()>8 {return Err("invalid progress source count".into());}
        if state.model.as_ref().is_some_and(|m|m.context()!=context) {return Err("invalid progress candidate".into());}
        let candidate_sha=state.model.as_ref().map(|m|m.export().map(|bytes|{
            use sha2::{Digest,Sha256};format!("{:x}",Sha256::digest(bytes))
        })).transpose().map_err(|_|"invalid progress candidate")?;
        result.context_match=true;
        result.state=match state.job.phase {JobPhase::Idle=>"idle",JobPhase::Running=>"running",JobPhase::Cancelling=>"cancelling",JobPhase::Ready=>"ready",JobPhase::Failed=>"failed",JobPhase::Cancelled=>"cancelled"};
        let labels=[RoomLabel::Empty,RoomLabel::OccupiedStatic,RoomLabel::OccupiedMoving,RoomLabel::AdjacentOnly];
        let conditions:Vec<_>=labels.into_iter().map(|label| {
            let count=|role| capture.history().iter().filter(|p| p.status==PhaseStatus::Sealed && p.role==role && p.label==label)
                .map(|p|p.cohort_count).sum::<usize>().min(128);
            RecordedCondition{label,train_recorded:count(DataRole::Train),held_out_recorded:count(DataRole::HeldOut)}
        }).collect();
        let train_complete=conditions.iter().all(|c|c.train_recorded>=4);
        let heldout_complete=conditions.iter().all(|c|c.held_out_recorded>=4);
        result.conditions=Some(conditions);
        let progress=capture.progress(now_ns);
        result.sources=Some(ProgressSources{required:context.sources.len(),ready:progress.sources.iter().filter(|s| s.window_eligible && externally_ready_sources.contains(&s.source_id)).count()});
        result.phase=progress.phase.map(|p| ProgressPhase {
            role:p.role,label:p.label,
            stage:if p.lead_in_remaining_ms>0 {"lead_in"} else if p.capture_elapsed_ms<p.target_ms {"collecting"} else if p.representative_cohorts==p.required_cohorts {"awaiting_confirmation"} else {"incomplete"},
            target_ms:p.target_ms,lead_in_remaining_ms:p.lead_in_remaining_ms.min(120_000),capture_elapsed_ms:p.capture_elapsed_ms.min(p.target_ms),
            representative_cohorts:p.representative_cohorts.min(4),required_cohorts:4,
        });
        // Reuse the actual immutable selection/assessment validators. No caller
        // supplied pass, owner error strings or private payload enters this DTO.
        let snapshot=self.selection_status_locked(&state,now_ns,Some(&published.binding_digest));
        let verified=(|| -> Result<(),String> {
            let stored=self.store.load_selection()?;
            if stored.as_ref().map_or(0,|e|e.receipt.revision)!=snapshot.revision {return Err("selection changed".into());}
            match (stored.as_ref().and_then(|e|e.receipt.selected.as_ref()),snapshot.selected.as_ref()) {
                (Some(target),Some(identity))=>{
                    if stored.as_ref().map(|e|e.receipt_sha256.as_str())!=Some(identity.selection_sha256.as_str())
                        || target.model_sha256!=identity.model_sha256 || target.qualification_sha256!=identity.qualification_sha256 {return Err("selection changed".into());}
                    let (model,q)=self.store.resolve_selection_target(target)?;self.store.current_qualification_assessment(&model,&q)?;
                },
                (None,None)=>{},_=>return Err("selection changed".into()),
            }
            Ok(())
        })();
        result.qualification.state=if candidate_sha.is_some() {"not_qualified"} else {"not_reported"};
        result.qualification.model_sha256=candidate_sha;
        result.selection.state="not_selected";
        if snapshot.qualification_error.is_some() { result.qualification.state="failed"; }
        else if let (Some(q),Some(model))=(state.qualification.as_ref(),state.model.as_ref()) {
            let loaded=self.store.load_qualification(model,&q.receipt_sha256).and_then(|loaded| {
                if loaded.receipt.room_digest!=published.binding_digest {return Err("qualification context changed".into());}
                self.store.current_qualification_assessment(model,&loaded)?;Ok(loaded)
            });
            match loaded {
                Ok(loaded)=>result.qualification=ProgressQualification{state:"qualified",receipt_sha256:Some(loaded.receipt_sha256),model_sha256:Some(loaded.receipt.model_sha256)},
                Err(_)=>result.qualification.state="failed",
            }
        }
        project_selection(&mut result,&snapshot,verified.is_ok(),&context.digest());
        let source_unready=result.sources.as_ref().is_none_or(|s|s.ready!=s.required);
        if source_unready {result.selection.current_usable=false;}
        if let Some(phase)=&result.phase {
            result.stage=if phase.role==DataRole::Train {"train"} else {"held_out"};
            result.blocker=if phase.stage=="incomplete" {Some("phase_incomplete")} else if source_unready {Some("sources_unready")} else {None};
        } else if busy(&state.job) {result.stage="training";result.blocker=None;}
        else if state.job.phase==JobPhase::Failed {result.stage="blocked";result.blocker=Some("training_failed");}
        else if result.qualification.state=="failed" {result.stage="blocked";result.blocker=Some("qualification_failed");}
        else if result.selection.state=="unavailable" {result.stage="blocked";result.blocker=Some("selection_unavailable");}
        else if result.qualification.state=="qualified" && result.qualification.model_sha256!=result.selection.model_sha256 {result.stage="selection";result.blocker=Some("selection_required");}
        else if result.selection.state=="selected" {result.stage="selected";result.blocker=if result.selection.current_usable {None} else if source_unready {Some("sources_unready")} else {Some("selection_unavailable")};}
        else if result.qualification.state=="qualified" {result.stage="selection";result.blocker=Some("selection_required");}
        // Zero temporary rows do not invalidate a durable managed proof. This
        // base projection does not report that proof's readiness or counts.
        else if state.model.is_some() && !train_complete {result.stage=if heldout_complete {"qualification"} else {"held_out"};result.blocker=Some("training_lineage_not_reported");}
        else if state.model.is_some() {result.stage=if heldout_complete {"qualification"} else {"held_out"};result.blocker=if source_unready {Some("sources_unready")} else if heldout_complete {Some("qualification_required")} else {None};}
        else {result.stage="train";result.blocker=if source_unready {Some("sources_unready")} else {None};}
        Ok(result)
    }
}
fn project_selection(result:&mut PairedCalibrationProgress,snapshot:&ModelSelectionStatus,verified:bool,context_digest:&str) {
    if let Some(selected)=&snapshot.selected {
        if Some(selected.room_digest.as_str())!=result.room.as_ref().map(|r|r.binding_digest.as_str()) || selected.context_digest!=context_digest {
            result.selection.state="not_reported";return;
        }
    }
    if !verified || snapshot.integrity_error.is_some() {result.selection.state="unavailable";return;}
    if let Some(selected)=&snapshot.selected {
        result.selection=ProgressSelection {state:"selected",selection_sha256:Some(selected.selection_sha256.clone()),model_sha256:Some(selected.model_sha256.clone()),
            current_usable:matches!(snapshot.current,SelectedBinaryEvidence::Ready{..})};
    }
}

#[cfg(test)]
#[path = "paired_progress_tests.rs"]
mod tests;
