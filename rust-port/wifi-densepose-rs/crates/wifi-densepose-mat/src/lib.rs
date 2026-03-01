//! # WiFi-DensePose MAT (Mass Casualty Assessment Tool)
//!
//! A modular extension for WiFi-based disaster survivor detection and localization.
//!
//! This crate provides capabilities for detecting human survivors trapped in rubble,
//! debris, or collapsed structures using WiFi Channel State Information (CSI) analysis.
//!
//! ## Features
//!
//! - **Vital Signs Detection**: Breathing patterns, heartbeat signatures, and movement
//! - **Survivor Localization**: 3D position estimation through debris
//! - **Triage Classification**: Automatic START protocol-compatible triage
//! - **Real-time Alerting**: Priority-based alert generation and dispatch
//!
//! ## Use Cases
//!
//! - Earthquake search and rescue
//! - Building collapse response
//! - Avalanche victim location
//! - Flood rescue operations
//! - Mine collapse detection
//!
//! ## Architecture
//!
//! The crate follows Domain-Driven Design (DDD) principles with clear bounded contexts:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    wifi-densepose-mat                    │
//! ├─────────────────────────────────────────────────────────┤
//! │  ┌───────────┐  ┌─────────────┐  ┌─────────────────┐   │
//! │  │ Detection │  │Localization │  │    Alerting     │   │
//! │  │  Context  │  │   Context   │  │    Context      │   │
//! │  └─────┬─────┘  └──────┬──────┘  └────────┬────────┘   │
//! │        └───────────────┼──────────────────┘            │
//! │                        │                                │
//! │              ┌─────────▼─────────┐                      │
//! │              │   Integration     │                      │
//! │              │      Layer        │                      │
//! │              └───────────────────┘                      │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Example
//!
//! ```rust,no_run
//! use wifi_densepose_mat::{
//!     DisasterResponse, DisasterConfig, DisasterType,
//!     ScanZone, ZoneBounds,
//! };
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Initialize disaster response system
//!     let config = DisasterConfig::builder()
//!         .disaster_type(DisasterType::Earthquake)
//!         .sensitivity(0.8)
//!         .build();
//!
//!     let mut response = DisasterResponse::new(config);
//!
//!     // Define scan zone
//!     let zone = ScanZone::new(
//!         "Building A - North Wing",
//!         ZoneBounds::rectangle(0.0, 0.0, 50.0, 30.0),
//!     );
//!     response.add_zone(zone)?;
//!
//!     // Start scanning
//!     response.start_scanning().await?;
//!
//!     Ok(())
//! }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

pub mod alerting;
pub mod api;
pub mod detection;
pub mod domain;
pub mod integration;
pub mod localization;
pub mod ml;
pub mod tracking;

// Re-export main types
pub use domain::{
    survivor::{Survivor, SurvivorId, SurvivorMetadata, SurvivorStatus},
    disaster_event::{DisasterEvent, DisasterEventId, DisasterType, EventStatus},
    scan_zone::{ScanZone, ScanZoneId, ZoneBounds, ZoneStatus, ScanParameters},
    alert::{Alert, AlertId, AlertPayload, Priority},
    vital_signs::{
        VitalSignsReading, BreathingPattern, BreathingType,
        HeartbeatSignature, MovementProfile, MovementType,
    },
    triage::{TriageStatus, TriageCalculator},
    coordinates::{Coordinates3D, LocationUncertainty, DepthEstimate},
    events::{DetectionEvent, AlertEvent, DomainEvent, EventStore, InMemoryEventStore, TrackingEvent},
};

pub use detection::{
    BreathingDetector, BreathingDetectorConfig,
    HeartbeatDetector, HeartbeatDetectorConfig,
    MovementClassifier, MovementClassifierConfig,
    VitalSignsDetector, DetectionPipeline, DetectionConfig,
    EnsembleClassifier, EnsembleConfig, EnsembleResult,
};

pub use localization::{
    Triangulator, TriangulationConfig,
    DepthEstimator, DepthEstimatorConfig,
    PositionFuser, LocalizationService,
};

pub use alerting::{
    AlertGenerator, AlertDispatcher, AlertConfig,
    TriageService, PriorityCalculator,
};

pub use integration::{
    SignalAdapter, NeuralAdapter, HardwareAdapter,
    AdapterError, IntegrationConfig,
};

pub use api::{
    create_router, AppState,
};

pub use ml::{
    // Core ML types
    MlError, MlResult, MlDetectionConfig, MlDetectionPipeline, MlDetectionResult,
    // Debris penetration model
    DebrisPenetrationModel, DebrisFeatures, DepthEstimate as MlDepthEstimate,
    DebrisModel, DebrisModelConfig, DebrisFeatureExtractor,
    MaterialType, DebrisClassification, AttenuationPrediction,
    // Vital signs classifier
    VitalSignsClassifier, VitalSignsClassifierConfig,
    BreathingClassification, HeartbeatClassification,
    UncertaintyEstimate, ClassifierOutput,
};

pub use tracking::{
    SurvivorTracker, TrackerConfig, TrackId, TrackedSurvivor,
    DetectionObservation, AssociationResult,
    KalmanState, CsiFingerprint,
    TrackState, TrackLifecycle,
};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Common result type for MAT operations
pub type Result<T> = std::result::Result<T, MatError>;

/// Unified error type for MAT operations
#[derive(Debug, thiserror::Error)]
pub enum MatError {
    /// Detection error
    #[error("Detection error: {0}")]
    Detection(String),

    /// Localization error
    #[error("Localization error: {0}")]
    Localization(String),

    /// Alerting error
    #[error("Alerting error: {0}")]
    Alerting(String),

    /// Integration error
    #[error("Integration error: {0}")]
    Integration(#[from] AdapterError),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Domain invariant violation
    #[error("Domain error: {0}")]
    Domain(String),

    /// Repository error
    #[error("Repository error: {0}")]
    Repository(String),

    /// Signal processing error
    #[error("Signal processing error: {0}")]
    Signal(#[from] wifi_densepose_signal::SignalError),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Machine learning error
    #[error("ML error: {0}")]
    Ml(#[from] ml::MlError),
}

/// Configuration for the disaster response system
#[derive(Debug, Clone)]
pub struct DisasterConfig {
    /// Type of disaster event
    pub disaster_type: DisasterType,
    /// Detection sensitivity (0.0-1.0)
    pub sensitivity: f64,
    /// Minimum confidence threshold for survivor detection
    pub confidence_threshold: f64,
    /// Maximum depth to scan (meters)
    pub max_depth: f64,
    /// Scan interval in milliseconds
    pub scan_interval_ms: u64,
    /// Enable continuous monitoring
    pub continuous_monitoring: bool,
    /// Alert configuration
    pub alert_config: AlertConfig,
}

impl Default for DisasterConfig {
    fn default() -> Self {
        Self {
            disaster_type: DisasterType::Unknown,
            sensitivity: 0.8,
            confidence_threshold: 0.5,
            max_depth: 5.0,
            scan_interval_ms: 500,
            continuous_monitoring: true,
            alert_config: AlertConfig::default(),
        }
    }
}

impl DisasterConfig {
    /// Create a new configuration builder
    pub fn builder() -> DisasterConfigBuilder {
        DisasterConfigBuilder::default()
    }
}

/// Builder for DisasterConfig
#[derive(Debug, Default)]
pub struct DisasterConfigBuilder {
    config: DisasterConfig,
}

impl DisasterConfigBuilder {
    /// Set disaster type
    pub fn disaster_type(mut self, disaster_type: DisasterType) -> Self {
        self.config.disaster_type = disaster_type;
        self
    }

    /// Set detection sensitivity
    pub fn sensitivity(mut self, sensitivity: f64) -> Self {
        self.config.sensitivity = sensitivity.clamp(0.0, 1.0);
        self
    }

    /// Set confidence threshold
    pub fn confidence_threshold(mut self, threshold: f64) -> Self {
        self.config.confidence_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set maximum scan depth
    pub fn max_depth(mut self, depth: f64) -> Self {
        self.config.max_depth = depth.max(0.0);
        self
    }

    /// Set scan interval
    pub fn scan_interval_ms(mut self, interval: u64) -> Self {
        self.config.scan_interval_ms = interval.max(100);
        self
    }

    /// Enable/disable continuous monitoring
    pub fn continuous_monitoring(mut self, enabled: bool) -> Self {
        self.config.continuous_monitoring = enabled;
        self
    }

    /// Build the configuration
    pub fn build(self) -> DisasterConfig {
        self.config
    }
}

/// Main disaster response coordinator
pub struct DisasterResponse {
    config: DisasterConfig,
    event: Option<DisasterEvent>,
    detection_pipeline: DetectionPipeline,
    localization_service: LocalizationService,
    alert_dispatcher: AlertDispatcher,
    event_store: std::sync::Arc<dyn domain::events::EventStore>,
    ensemble_classifier: EnsembleClassifier,
    tracker: tracking::SurvivorTracker,
    running: std::sync::atomic::AtomicBool,
}

impl DisasterResponse {
    /// Create a new disaster response system
    pub fn new(config: DisasterConfig) -> Self {
        let detection_config = DetectionConfig::from_disaster_config(&config);
        let detection_pipeline = DetectionPipeline::new(detection_config);

        let localization_service = LocalizationService::new();
        let alert_dispatcher = AlertDispatcher::new(config.alert_config.clone());
        let event_store: std::sync::Arc<dyn domain::events::EventStore> =
            std::sync::Arc::new(InMemoryEventStore::new());
        let ensemble_classifier = EnsembleClassifier::new(EnsembleConfig::default());

        Self {
            config,
            event: None,
            detection_pipeline,
            localization_service,
            alert_dispatcher,
            event_store,
            ensemble_classifier,
            tracker: tracking::SurvivorTracker::with_defaults(),
            running: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Create with a custom event store (e.g. for persistence or testing)
    pub fn with_event_store(
        config: DisasterConfig,
        event_store: std::sync::Arc<dyn domain::events::EventStore>,
    ) -> Self {
        let detection_config = DetectionConfig::from_disaster_config(&config);
        let detection_pipeline = DetectionPipeline::new(detection_config);
        let localization_service = LocalizationService::new();
        let alert_dispatcher = AlertDispatcher::new(config.alert_config.clone());
        let ensemble_classifier = EnsembleClassifier::new(EnsembleConfig::default());

        Self {
            config,
            event: None,
            detection_pipeline,
            localization_service,
            alert_dispatcher,
            event_store,
            ensemble_classifier,
            tracker: tracking::SurvivorTracker::with_defaults(),
            running: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Push CSI data into the detection pipeline for processing.
    ///
    /// This is the primary data ingestion point. Call this with real CSI
    /// amplitude and phase readings from hardware (ESP32, Intel 5300, etc).
    /// Returns an error string if data is invalid.
    pub fn push_csi_data(&self, amplitudes: &[f64], phases: &[f64]) -> Result<()> {
        if amplitudes.len() != phases.len() {
            return Err(MatError::Detection(
                "Amplitude and phase arrays must have equal length".into(),
            ));
        }
        if amplitudes.is_empty() {
            return Err(MatError::Detection("CSI data cannot be empty".into()));
        }
        self.detection_pipeline.add_data(amplitudes, phases);
        Ok(())
    }

    /// Get the event store for querying domain events
    pub fn event_store(&self) -> &std::sync::Arc<dyn domain::events::EventStore> {
        &self.event_store
    }

    /// Get the ensemble classifier
    pub fn ensemble_classifier(&self) -> &EnsembleClassifier {
        &self.ensemble_classifier
    }

    /// Get the detection pipeline (for direct buffer inspection / data push)
    pub fn detection_pipeline(&self) -> &DetectionPipeline {
        &self.detection_pipeline
    }

    /// Get the survivor tracker
    pub fn tracker(&self) -> &tracking::SurvivorTracker {
        &self.tracker
    }

    /// Get mutable access to the tracker (for integration in scan_cycle)
    pub fn tracker_mut(&mut self) -> &mut tracking::SurvivorTracker {
        &mut self.tracker
    }

    /// Initialize a new disaster event
    pub fn initialize_event(
        &mut self,
        location: geo::Point<f64>,
        description: &str,
    ) -> Result<&DisasterEvent> {
        let event = DisasterEvent::new(
            self.config.disaster_type.clone(),
            location,
            description,
        );
        self.event = Some(event);
        self.event.as_ref().ok_or_else(|| MatError::Domain("Failed to create event".into()))
    }

    /// Add a scan zone to the current event
    pub fn add_zone(&mut self, zone: ScanZone) -> Result<()> {
        let event = self.event.as_mut()
            .ok_or_else(|| MatError::Domain("No active disaster event".into()))?;
        event.add_zone(zone);
        Ok(())
    }

    /// Start the scanning process
    pub async fn start_scanning(&mut self) -> Result<()> {
        use std::sync::atomic::Ordering;

        self.running.store(true, Ordering::SeqCst);

        while self.running.load(Ordering::SeqCst) {
            self.scan_cycle().await?;

            if !self.config.continuous_monitoring {
                break;
            }

            tokio::time::sleep(
                std::time::Duration::from_millis(self.config.scan_interval_ms)
            ).await;
        }

        Ok(())
    }

    /// Stop the scanning process
    pub fn stop_scanning(&self) {
        use std::sync::atomic::Ordering;
        self.running.store(false, Ordering::SeqCst);
    }

    /// Execute a single scan cycle.
    ///
    /// Processes all active zones, runs detection pipeline on buffered CSI data,
    /// applies ensemble classification, emits domain events to the EventStore,
    /// and dispatches alerts for newly detected survivors.
    async fn scan_cycle(&mut self) -> Result<()> {
        let scan_start = std::time::Instant::now();

        // Collect detections first to avoid borrowing issues
        let mut detections = Vec::new();

        {
            let event = self.event.as_ref()
                .ok_or_else(|| MatError::Domain("No active disaster event".into()))?;

            for zone in event.zones() {
                if zone.status() != &ZoneStatus::Active {
                    continue;
                }

                // Process buffered CSI data through the detection pipeline
                let detection_result = self.detection_pipeline.process_zone(zone).await?;

                if let Some(vital_signs) = detection_result {
                    // Run ensemble classifier to combine breathing + heartbeat + movement
                    let ensemble_result = self.ensemble_classifier.classify(&vital_signs);

                    // Only proceed if ensemble confidence meets threshold
                    if ensemble_result.confidence >= self.config.confidence_threshold {
                        // Attempt localization
                        let location = self.localization_service
                            .estimate_position(&vital_signs, zone);

                        detections.push((zone.id().clone(), zone.name().to_string(), vital_signs, location, ensemble_result));
                    }
                }

                // Emit zone scan completed event
                let scan_duration = scan_start.elapsed();
                let _ = self.event_store.append(DomainEvent::Zone(
                    domain::events::ZoneEvent::ZoneScanCompleted {
                        zone_id: zone.id().clone(),
                        detections_found: detections.len() as u32,
                        scan_duration_ms: scan_duration.as_millis() as u64,
                        timestamp: chrono::Utc::now(),
                    },
                ));
            }
        }

        // Now process detections with mutable access
        let event = self.event.as_mut()
            .ok_or_else(|| MatError::Domain("No active disaster event".into()))?;

        for (zone_id, _zone_name, vital_signs, location, _ensemble) in detections {
            let survivor = event.record_detection(zone_id.clone(), vital_signs.clone(), location.clone())?;

            // Emit SurvivorDetected domain event
            let _ = self.event_store.append(DomainEvent::Detection(
                DetectionEvent::SurvivorDetected {
                    survivor_id: survivor.id().clone(),
                    zone_id,
                    vital_signs,
                    location,
                    timestamp: chrono::Utc::now(),
                },
            ));

            // Generate and dispatch alert if needed
            if survivor.should_alert() {
                let alert = self.alert_dispatcher.generate_alert(survivor)?;
                let alert_id = alert.id().clone();
                let priority = alert.priority();
                let survivor_id = alert.survivor_id().clone();

                // Emit AlertGenerated domain event
                let _ = self.event_store.append(DomainEvent::Alert(
                    AlertEvent::AlertGenerated {
                        alert_id,
                        survivor_id,
                        priority,
                        timestamp: chrono::Utc::now(),
                    },
                ));

                self.alert_dispatcher.dispatch(alert).await?;
            }
        }

        Ok(())
    }

    /// Get the current disaster event
    pub fn event(&self) -> Option<&DisasterEvent> {
        self.event.as_ref()
    }

    /// Get all detected survivors
    pub fn survivors(&self) -> Vec<&Survivor> {
        self.event.as_ref()
            .map(|e| e.survivors())
            .unwrap_or_default()
    }

    /// Get survivors by triage status
    pub fn survivors_by_triage(&self, status: TriageStatus) -> Vec<&Survivor> {
        self.survivors()
            .into_iter()
            .filter(|s| s.triage_status() == &status)
            .collect()
    }
}

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{
        DisasterConfig, DisasterConfigBuilder, DisasterResponse,
        MatError, Result,
        // Domain types
        Survivor, SurvivorId, DisasterEvent, DisasterType,
        ScanZone, ZoneBounds, TriageStatus,
        VitalSignsReading, BreathingPattern, HeartbeatSignature,
        Coordinates3D, Alert, Priority,
        // Event sourcing
        DomainEvent, EventStore, InMemoryEventStore,
        DetectionEvent, AlertEvent, TrackingEvent,
        // Detection
        DetectionPipeline, VitalSignsDetector,
        EnsembleClassifier, EnsembleConfig, EnsembleResult,
        // Localization
        LocalizationService,
        // Alerting
        AlertDispatcher,
        // ML types
        MlDetectionConfig, MlDetectionPipeline, MlDetectionResult,
        DebrisModel, MaterialType, DebrisClassification,
        VitalSignsClassifier, UncertaintyEstimate,
        // Tracking
        SurvivorTracker, TrackerConfig, TrackId, DetectionObservation, AssociationResult,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = DisasterConfig::builder()
            .disaster_type(DisasterType::Earthquake)
            .sensitivity(0.9)
            .confidence_threshold(0.6)
            .max_depth(10.0)
            .build();

        assert!(matches!(config.disaster_type, DisasterType::Earthquake));
        assert!((config.sensitivity - 0.9).abs() < f64::EPSILON);
        assert!((config.confidence_threshold - 0.6).abs() < f64::EPSILON);
        assert!((config.max_depth - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sensitivity_clamping() {
        let config = DisasterConfig::builder()
            .sensitivity(1.5)
            .build();

        assert!((config.sensitivity - 1.0).abs() < f64::EPSILON);

        let config = DisasterConfig::builder()
            .sensitivity(-0.5)
            .build();

        assert!(config.sensitivity.abs() < f64::EPSILON);
    }

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
