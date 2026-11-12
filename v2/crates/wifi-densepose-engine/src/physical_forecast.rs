//! Optional PAN2 hypothesis integration, outside the real-time sensing cycle.
//!
//! Preparation is denied under anonymous/identity-suppressed modes. Completion
//! checks the current privacy attestation and expiry, then returns a graph based
//! on the captured source snapshot, never mutating the live observed graph.
use crate::StreamingEngine;
use wifi_densepose_bfld::PrivacyMode;
use wifi_densepose_worldgraph::{WorldGraph, WorldId};
pub use wifi_densepose_worldmodel::physical::{
    Pan2PhysicalBridge, Pan2PhysicalResponse, PhysicalBounds, PhysicalContext,
    PhysicalExecution, PhysicalForecastBranch, PhysicalObservations, PhysicalScore,
    PreparedPhysicalScene,
};
use wifi_densepose_worldmodel::WorldModelError;

/// Runtime admission in addition to the disabled-by-default Cargo feature.
#[derive(Debug, Clone, Copy, Default)]
pub struct PhysicalForecastConfig {
    /// Explicit opt-in for local experimental per-track forecasting.
    pub enabled: bool,
    /// Allow synthetic fixture responses. False unless explicitly enabled.
    pub allow_fixtures: bool,
}
/// Immutable source ticket passed to an external worker off the sensing loop.
pub struct PhysicalForecastTicket {
    scene: PreparedPhysicalScene,
    source: WorldGraph,
    privacy_hash: [u8;32],
    source_cycle: u64,
    observed_at_ms: i64,
    expires_at_ms: i64,
}
impl PhysicalForecastTicket {
    /// Authorized scene for `Pan2PhysicalBridge::predict` on a separate task.
    pub fn scene(&self)->&PreparedPhysicalScene {&self.scene}
    /// Source engine cycle. Later sensor cycles do not overwrite this source.
    pub fn source_cycle(&self)->u64 {self.source_cycle}
}
/// Separate hypothesis result; not a sensing output or automation trigger.
pub struct PhysicalForecastResult {
    hypothesis: WorldGraph,
    branch: PhysicalForecastBranch,
    source_cycle: u64,
}
impl PhysicalForecastResult {
    /// A source-snapshot graph copy containing explicitly marked hypotheses.
    pub fn hypothesis_graph(&self)->&WorldGraph {&self.hypothesis}
    /// Validated branch for comparison with later, independently observed data.
    pub fn branch(&self)->&PhysicalForecastBranch {&self.branch}
    /// Engine cycle from which the prediction was prepared.
    pub fn source_cycle(&self)->u64 {self.source_cycle}
}
fn invalid(message:&str)->WorldModelError {WorldModelError::InvalidInput(message.into())}

impl StreamingEngine {
    fn admit_physical_forecast(&self,config:PhysicalForecastConfig)->Result<(),WorldModelError> {
        if !config.enabled {return Err(invalid("physical forecasting is disabled"));}
        // Anonymous/aggregate modes do not authorize per-track trajectories.
        if !matches!(self.privacy().active_mode(),PrivacyMode::RawResearch|PrivacyMode::CareWithConsent) {
            return Err(invalid("active privacy mode does not authorize per-track forecasting"));
        }
        Ok(())
    }
    /// Capture selected engine-owned tracks and an immutable observed graph.
    /// The caller supplies calibrated geometry/action and a trusted observation
    /// time. No raw CSI, re-ID embeddings or model invocation enters the cycle.
    pub fn prepare_physical_forecast(&self,config:PhysicalForecastConfig,track_ids:&[u64],mut context:PhysicalContext)->Result<PhysicalForecastTicket,WorldModelError> {
        self.admit_physical_forecast(config)?;
        if track_ids.is_empty() || track_ids.len()>64 {return Err(invalid("select 1..=64 engine track IDs"));}
        let ids=track_ids.iter().map(|id|self.person_tracks.get(id).copied().ok_or_else(||invalid("unknown engine track ID"))).collect::<Result<Vec<WorldId>,_>>()?;
        let proof=self.privacy().latest_proof().hash;
        let hash=proof.iter().map(|b|format!("{b:02x}")).collect::<String>();
        context.privacy.projection_id=format!("ruview:{:?}:{hash}",self.privacy().active_mode());
        context.privacy.hosted_processing_approved=false;
        let observed_at_ms=context.observed_at_ms;
        let expires_at_ms=observed_at_ms.checked_add(i64::from(context.horizon_ms)).ok_or_else(||invalid("forecast expiry overflow"))?;
        let bytes=self.snapshot_json().map_err(|e|invalid(&e.to_string()))?;
        let source=WorldGraph::from_json(&bytes).map_err(|e|invalid(&e.to_string()))?;
        let scene=PreparedPhysicalScene::from_graph(&source,&ids,context)?;
        Ok(PhysicalForecastTicket{scene,source,privacy_hash:proof,source_cycle:self.cycle_count(),observed_at_ms,expires_at_ms})
    }
    /// Admit a completed worker branch using current privacy and a trusted clock.
    /// New sensing cycles are allowed during generation; policy changes and
    /// expired horizons reject. There is no write to the live graph or tracker.
    pub fn finish_physical_forecast(&self,config:PhysicalForecastConfig,ticket:&PhysicalForecastTicket,branch:PhysicalForecastBranch,now_ms:i64)->Result<PhysicalForecastResult,WorldModelError> {
        self.admit_physical_forecast(config)?;
        if self.privacy().latest_proof().hash!=ticket.privacy_hash {return Err(invalid("privacy attestation changed since forecast preparation"));}
        if now_ms<ticket.observed_at_ms || now_ms>ticket.expires_at_ms {return Err(invalid("forecast is expired or clock predates source observation"));}
        if branch.response().request_sha256!=ticket.scene.request_sha256()? {return Err(invalid("forecast does not match the source ticket"));}
        if branch.response().execution==PhysicalExecution::Fixture && !config.allow_fixtures {return Err(invalid("fixture forecast requires explicit fixture opt-in"));}
        let hypothesis=branch.hypothesis_graph(&ticket.source)?;
        Ok(PhysicalForecastResult{hypothesis,branch,source_cycle:ticket.source_cycle})
    }
    /// Offline replay of an untrusted extractor response through the same gate.
    pub fn replay_physical_forecast(&self,config:PhysicalForecastConfig,ticket:&PhysicalForecastTicket,response:&[u8],now_ms:i64)->Result<PhysicalForecastResult,WorldModelError> {
        // Recheck policy before parsing potentially large external JSON.
        self.admit_physical_forecast(config)?;
        let branch=ticket.scene.accept_json(response)?;
        self.finish_physical_forecast(config,ticket,branch,now_ms)
    }
}
