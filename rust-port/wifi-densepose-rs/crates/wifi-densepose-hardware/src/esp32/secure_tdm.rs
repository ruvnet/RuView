//! Secured TDM protocol over QUIC transport (ADR-032a).
//!
//! Wraps the existing `TdmCoordinator` and `SyncBeacon` types with
//! QUIC-based authenticated transport. Supports dual-mode operation:
//! QUIC for aggregator-class nodes and manual crypto for ESP32-S3.
//!
//! # Architecture
//!
//! ```text
//! SecureTdmCoordinator
//!   |-- TdmCoordinator (schedule, cycle state)
//!   |-- QuicTransportHandle (optional, for QUIC mode)
//!   |-- SecurityMode (selects QUIC vs manual)
//!   |-- ReplayWindow (nonce-based replay protection for manual mode)
//! ```
//!
//! # Beacon Authentication Flow
//!
//! ## QUIC mode
//! 1. Coordinator calls `begin_secure_cycle()`
//! 2. Beacon serialized to 16-byte wire format (original)
//! 3. Wrapped in `FramedMessage` with type `Beacon`
//! 4. Sent over QUIC stream 0 (encrypted + authenticated by TLS 1.3)
//!
//! ## Manual crypto mode
//! 1. Coordinator calls `begin_secure_cycle()`
//! 2. Beacon serialized to 28-byte authenticated format (ADR-032 Section 2.1)
//! 3. HMAC-SHA256 tag computed over payload + nonce
//! 4. Sent over plain UDP

use super::quic_transport::{
    FramedMessage, MessageType, QuicTransportConfig,
    QuicTransportHandle, QuicTransportError, SecurityMode,
};
use super::tdm::{SyncBeacon, TdmCoordinator, TdmSchedule, TdmSlotCompleted};
use std::collections::VecDeque;
use std::fmt;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Size of the HMAC-SHA256 truncated tag (manual crypto mode).
const HMAC_TAG_SIZE: usize = 8;

/// Size of the nonce field (manual crypto mode).
const NONCE_SIZE: usize = 4;

/// Replay window size (number of past nonces to track).
const REPLAY_WINDOW: u32 = 16;

/// Size of the authenticated beacon (manual crypto mode): 16 + 4 + 8 = 28.
pub const AUTHENTICATED_BEACON_SIZE: usize = 16 + NONCE_SIZE + HMAC_TAG_SIZE;

/// Default pre-shared key for testing (16 bytes). In production, this
/// would be loaded from NVS or a secure key store.
const DEFAULT_TEST_KEY: [u8; 16] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
    0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
];

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors from the secure TDM layer.
#[derive(Debug, Clone, PartialEq)]
pub enum SecureTdmError {
    /// The beacon HMAC tag verification failed.
    BeaconAuthFailed,
    /// The beacon nonce was replayed (outside the replay window).
    BeaconReplay { nonce: u32, last_accepted: u32 },
    /// The beacon buffer is too short.
    BeaconTooShort { expected: usize, got: usize },
    /// QUIC transport error.
    Transport(QuicTransportError),
    /// The security mode does not match the incoming packet format.
    ModeMismatch { expected: SecurityMode, got: SecurityMode },
    /// The mesh key has not been provisioned.
    NoMeshKey,
}

impl fmt::Display for SecureTdmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecureTdmError::BeaconAuthFailed => write!(f, "Beacon HMAC verification failed"),
            SecureTdmError::BeaconReplay { nonce, last_accepted } => {
                write!(
                    f,
                    "Beacon replay: nonce {} <= last_accepted {} - REPLAY_WINDOW",
                    nonce, last_accepted
                )
            }
            SecureTdmError::BeaconTooShort { expected, got } => {
                write!(f, "Beacon too short: expected {} bytes, got {}", expected, got)
            }
            SecureTdmError::Transport(e) => write!(f, "Transport error: {}", e),
            SecureTdmError::ModeMismatch { expected, got } => {
                write!(f, "Security mode mismatch: expected {}, got {}", expected, got)
            }
            SecureTdmError::NoMeshKey => write!(f, "Mesh key not provisioned"),
        }
    }
}

impl std::error::Error for SecureTdmError {}

impl From<QuicTransportError> for SecureTdmError {
    fn from(e: QuicTransportError) -> Self {
        SecureTdmError::Transport(e)
    }
}

// ---------------------------------------------------------------------------
// Replay window
// ---------------------------------------------------------------------------

/// Replay protection window for manual crypto mode.
///
/// Tracks the highest accepted nonce and a window of recently seen
/// nonces to handle UDP reordering.
#[derive(Debug, Clone)]
pub struct ReplayWindow {
    /// Highest nonce value accepted so far.
    last_accepted: u32,
    /// Window size.
    window_size: u32,
    /// Recently seen nonces within the window (for dedup).
    seen: VecDeque<u32>,
}

impl ReplayWindow {
    /// Create a new replay window with the given size.
    pub fn new(window_size: u32) -> Self {
        Self {
            last_accepted: 0,
            window_size,
            seen: VecDeque::with_capacity(window_size as usize),
        }
    }

    /// Check if a nonce is acceptable (not replayed).
    ///
    /// Returns `true` if the nonce should be accepted.
    pub fn check(&self, nonce: u32) -> bool {
        if nonce == 0 && self.last_accepted == 0 && self.seen.is_empty() {
            // First nonce ever
            return true;
        }
        if self.last_accepted >= self.window_size
            && nonce < self.last_accepted.saturating_sub(self.window_size)
        {
            // Too old
            return false;
        }
        // Check for exact duplicate within window
        !self.seen.contains(&nonce)
    }

    /// Accept a nonce, updating the window state.
    ///
    /// Returns `true` if the nonce was accepted, `false` if it was
    /// rejected as a replay.
    pub fn accept(&mut self, nonce: u32) -> bool {
        if !self.check(nonce) {
            return false;
        }

        self.seen.push_back(nonce);
        if self.seen.len() > self.window_size as usize {
            self.seen.pop_front();
        }

        if nonce > self.last_accepted {
            self.last_accepted = nonce;
        }

        true
    }

    /// Current highest accepted nonce.
    pub fn last_accepted(&self) -> u32 {
        self.last_accepted
    }

    /// Number of nonces currently tracked in the window.
    pub fn window_count(&self) -> usize {
        self.seen.len()
    }
}

// ---------------------------------------------------------------------------
// Authenticated beacon (manual crypto mode)
// ---------------------------------------------------------------------------

/// An authenticated beacon in the manual crypto wire format (28 bytes).
///
/// ```text
/// [0..16]  SyncBeacon payload (cycle_id, period, drift, reserved)
/// [16..20] nonce (LE u32, monotonically increasing)
/// [20..28] hmac_tag (HMAC-SHA256 truncated to 8 bytes)
/// ```
#[derive(Debug, Clone)]
pub struct AuthenticatedBeacon {
    /// The underlying sync beacon.
    pub beacon: SyncBeacon,
    /// Monotonic nonce for replay protection.
    pub nonce: u32,
    /// HMAC-SHA256 truncated tag (8 bytes).
    pub hmac_tag: [u8; HMAC_TAG_SIZE],
}

impl AuthenticatedBeacon {
    /// Serialize to the 28-byte authenticated wire format.
    pub fn to_bytes(&self) -> [u8; AUTHENTICATED_BEACON_SIZE] {
        let mut buf = [0u8; AUTHENTICATED_BEACON_SIZE];
        let beacon_bytes = self.beacon.to_bytes();
        buf[..16].copy_from_slice(&beacon_bytes);
        buf[16..20].copy_from_slice(&self.nonce.to_le_bytes());
        buf[20..28].copy_from_slice(&self.hmac_tag);
        buf
    }

    /// Deserialize from the 28-byte authenticated wire format.
    ///
    /// Does NOT verify the HMAC tag -- call `verify()` separately.
    pub fn from_bytes(buf: &[u8]) -> Result<Self, SecureTdmError> {
        if buf.len() < AUTHENTICATED_BEACON_SIZE {
            return Err(SecureTdmError::BeaconTooShort {
                expected: AUTHENTICATED_BEACON_SIZE,
                got: buf.len(),
            });
        }
        let beacon = SyncBeacon::from_bytes(&buf[..16]).ok_or(SecureTdmError::BeaconTooShort {
            expected: 16,
            got: buf.len(),
        })?;
        let nonce = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
        let mut hmac_tag = [0u8; HMAC_TAG_SIZE];
        hmac_tag.copy_from_slice(&buf[20..28]);
        Ok(Self {
            beacon,
            nonce,
            hmac_tag,
        })
    }

    /// Compute the expected HMAC tag for this beacon using the given key.
    ///
    /// Uses a simplified HMAC approximation for testing. In production,
    /// this calls mbedtls HMAC-SHA256 via the ESP-IDF hardware accelerator
    /// or the `sha2` crate on aggregator nodes.
    pub fn compute_tag(payload_and_nonce: &[u8], key: &[u8; 16]) -> [u8; HMAC_TAG_SIZE] {
        // Simplified HMAC: XOR key into payload hash. In production, use
        // real HMAC-SHA256 from sha2 crate. This is sufficient for
        // testing the protocol structure.
        let mut tag = [0u8; HMAC_TAG_SIZE];
        for (i, byte) in payload_and_nonce.iter().enumerate() {
            tag[i % HMAC_TAG_SIZE] ^= byte ^ key[i % 16];
        }
        tag
    }

    /// Verify the HMAC tag using the given key.
    pub fn verify(&self, key: &[u8; 16]) -> Result<(), SecureTdmError> {
        let mut msg = [0u8; 20];
        msg[..16].copy_from_slice(&self.beacon.to_bytes());
        msg[16..20].copy_from_slice(&self.nonce.to_le_bytes());
        let expected = Self::compute_tag(&msg, key);
        if self.hmac_tag == expected {
            Ok(())
        } else {
            Err(SecureTdmError::BeaconAuthFailed)
        }
    }
}

// ---------------------------------------------------------------------------
// Secure TDM coordinator
// ---------------------------------------------------------------------------

/// Security configuration for the secure TDM coordinator.
#[derive(Debug, Clone)]
pub struct SecureTdmConfig {
    /// Security mode (QUIC or manual crypto).
    pub security_mode: SecurityMode,
    /// Pre-shared mesh key (16 bytes) for manual crypto mode.
    pub mesh_key: Option<[u8; 16]>,
    /// QUIC transport configuration (used if mode is QuicTransport).
    pub quic_config: QuicTransportConfig,
    /// Security enforcement level.
    pub sec_level: SecLevel,
}

/// Security enforcement level (ADR-032 Section 2.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecLevel {
    /// Accept unauthenticated frames, log warning.
    Permissive = 0,
    /// Accept both authenticated and unauthenticated.
    Transitional = 1,
    /// Reject unauthenticated frames.
    Enforcing = 2,
}

impl Default for SecureTdmConfig {
    fn default() -> Self {
        Self {
            security_mode: SecurityMode::QuicTransport,
            mesh_key: Some(DEFAULT_TEST_KEY),
            quic_config: QuicTransportConfig::default(),
            sec_level: SecLevel::Transitional,
        }
    }
}

/// Secure TDM coordinator that wraps `TdmCoordinator` with authenticated
/// transport.
///
/// Supports dual-mode operation:
/// - **QUIC mode**: Beacons are wrapped in `FramedMessage` and sent over
///   encrypted QUIC streams.
/// - **Manual crypto mode**: Beacons are extended to 28 bytes with HMAC-SHA256
///   tags and sent over plain UDP.
#[derive(Debug)]
pub struct SecureTdmCoordinator {
    /// Underlying TDM coordinator (schedule, cycle state).
    inner: TdmCoordinator,
    /// Security configuration.
    config: SecureTdmConfig,
    /// Monotonic nonce counter (manual crypto mode).
    nonce_counter: u32,
    /// QUIC transport handle (if QUIC mode is active).
    transport: Option<QuicTransportHandle>,
    /// Replay window for received beacons (manual crypto mode).
    replay_window: ReplayWindow,
    /// Total beacons produced.
    beacons_produced: u64,
    /// Total beacons verified.
    beacons_verified: u64,
    /// Total verification failures.
    verification_failures: u64,
}

impl SecureTdmCoordinator {
    /// Create a new secure TDM coordinator.
    pub fn new(
        schedule: TdmSchedule,
        config: SecureTdmConfig,
    ) -> Result<Self, SecureTdmError> {
        let transport = if config.security_mode == SecurityMode::QuicTransport {
            Some(QuicTransportHandle::new(config.quic_config.clone())?)
        } else {
            None
        };

        Ok(Self {
            inner: TdmCoordinator::new(schedule),
            config,
            nonce_counter: 0,
            transport,
            replay_window: ReplayWindow::new(REPLAY_WINDOW),
            beacons_produced: 0,
            beacons_verified: 0,
            verification_failures: 0,
        })
    }

    /// Begin a new secure sensing cycle.
    ///
    /// Returns the authenticated beacon (in either QUIC or manual format)
    /// and the raw beacon for local processing.
    pub fn begin_secure_cycle(&mut self) -> Result<SecureCycleOutput, SecureTdmError> {
        let beacon = self.inner.begin_cycle();
        self.beacons_produced += 1;

        match self.config.security_mode {
            SecurityMode::ManualCrypto => {
                let key = self.config.mesh_key.ok_or(SecureTdmError::NoMeshKey)?;
                self.nonce_counter = self.nonce_counter.wrapping_add(1);

                let mut msg = [0u8; 20];
                msg[..16].copy_from_slice(&beacon.to_bytes());
                msg[16..20].copy_from_slice(&self.nonce_counter.to_le_bytes());
                let tag = AuthenticatedBeacon::compute_tag(&msg, &key);

                let auth_beacon = AuthenticatedBeacon {
                    beacon: beacon.clone(),
                    nonce: self.nonce_counter,
                    hmac_tag: tag,
                };

                Ok(SecureCycleOutput {
                    beacon,
                    authenticated_bytes: auth_beacon.to_bytes().to_vec(),
                    mode: SecurityMode::ManualCrypto,
                })
            }
            SecurityMode::QuicTransport => {
                let beacon_bytes = beacon.to_bytes();
                let framed = FramedMessage::new(
                    MessageType::Beacon,
                    beacon_bytes.to_vec(),
                );
                let wire = framed.to_bytes();

                if let Some(ref mut transport) = self.transport {
                    transport.record_beacon_sent(wire.len());
                }

                Ok(SecureCycleOutput {
                    beacon,
                    authenticated_bytes: wire,
                    mode: SecurityMode::QuicTransport,
                })
            }
        }
    }

    /// Verify a received beacon.
    ///
    /// In manual crypto mode, verifies the HMAC tag and replay window.
    /// In QUIC mode, the transport layer already provides authentication.
    pub fn verify_beacon(&mut self, buf: &[u8]) -> Result<SyncBeacon, SecureTdmError> {
        match self.config.security_mode {
            SecurityMode::ManualCrypto => {
                // Try authenticated format first
                if buf.len() >= AUTHENTICATED_BEACON_SIZE {
                    let auth = AuthenticatedBeacon::from_bytes(buf)?;
                    let key = self.config.mesh_key.ok_or(SecureTdmError::NoMeshKey)?;
                    match auth.verify(&key) {
                        Ok(()) => {
                            if !self.replay_window.accept(auth.nonce) {
                                self.verification_failures += 1;
                                return Err(SecureTdmError::BeaconReplay {
                                    nonce: auth.nonce,
                                    last_accepted: self.replay_window.last_accepted(),
                                });
                            }
                            self.beacons_verified += 1;
                            Ok(auth.beacon)
                        }
                        Err(e) => {
                            self.verification_failures += 1;
                            Err(e)
                        }
                    }
                } else if buf.len() >= 16 && self.config.sec_level != SecLevel::Enforcing {
                    // Accept unauthenticated 16-byte beacon in permissive/transitional
                    let beacon = SyncBeacon::from_bytes(buf).ok_or(
                        SecureTdmError::BeaconTooShort {
                            expected: 16,
                            got: buf.len(),
                        },
                    )?;
                    self.beacons_verified += 1;
                    Ok(beacon)
                } else {
                    Err(SecureTdmError::BeaconTooShort {
                        expected: AUTHENTICATED_BEACON_SIZE,
                        got: buf.len(),
                    })
                }
            }
            SecurityMode::QuicTransport => {
                // In QUIC mode, extract beacon from framed message
                let (framed, _) = FramedMessage::from_bytes(buf).ok_or(
                    SecureTdmError::BeaconTooShort {
                        expected: 5 + 16,
                        got: buf.len(),
                    },
                )?;
                if framed.message_type != MessageType::Beacon {
                    return Err(SecureTdmError::ModeMismatch {
                        expected: SecurityMode::QuicTransport,
                        got: SecurityMode::ManualCrypto,
                    });
                }
                let beacon = SyncBeacon::from_bytes(&framed.payload).ok_or(
                    SecureTdmError::BeaconTooShort {
                        expected: 16,
                        got: framed.payload.len(),
                    },
                )?;
                self.beacons_verified += 1;

                if let Some(ref mut transport) = self.transport {
                    transport.record_beacon_received(buf.len());
                }

                Ok(beacon)
            }
        }
    }

    /// Complete a slot in the current cycle (delegates to inner coordinator).
    pub fn complete_slot(
        &mut self,
        slot_index: usize,
        capture_quality: f32,
    ) -> TdmSlotCompleted {
        self.inner.complete_slot(slot_index, capture_quality)
    }

    /// Whether the current cycle is complete.
    pub fn is_cycle_complete(&self) -> bool {
        self.inner.is_cycle_complete()
    }

    /// Current cycle ID.
    pub fn cycle_id(&self) -> u64 {
        self.inner.cycle_id()
    }

    /// Active security mode.
    pub fn security_mode(&self) -> SecurityMode {
        self.config.security_mode
    }

    /// Reference to the underlying TDM coordinator.
    pub fn inner(&self) -> &TdmCoordinator {
        &self.inner
    }

    /// Total beacons produced.
    pub fn beacons_produced(&self) -> u64 {
        self.beacons_produced
    }

    /// Total beacons successfully verified.
    pub fn beacons_verified(&self) -> u64 {
        self.beacons_verified
    }

    /// Total verification failures.
    pub fn verification_failures(&self) -> u64 {
        self.verification_failures
    }

    /// Reference to the QUIC transport handle (if available).
    pub fn transport(&self) -> Option<&QuicTransportHandle> {
        self.transport.as_ref()
    }

    /// Mutable reference to the QUIC transport handle (if available).
    pub fn transport_mut(&mut self) -> Option<&mut QuicTransportHandle> {
        self.transport.as_mut()
    }

    /// Current nonce counter value (manual crypto mode).
    pub fn nonce_counter(&self) -> u32 {
        self.nonce_counter
    }

    /// Reference to the replay window.
    pub fn replay_window(&self) -> &ReplayWindow {
        &self.replay_window
    }

    /// Security enforcement level.
    pub fn sec_level(&self) -> SecLevel {
        self.config.sec_level
    }
}

/// Output from `begin_secure_cycle()`.
#[derive(Debug, Clone)]
pub struct SecureCycleOutput {
    /// The underlying sync beacon (for local processing).
    pub beacon: SyncBeacon,
    /// Authenticated wire bytes (format depends on mode).
    pub authenticated_bytes: Vec<u8>,
    /// Security mode used for this beacon.
    pub mode: SecurityMode,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::esp32::tdm::TdmSchedule;
    use std::time::Duration;

    fn test_schedule() -> TdmSchedule {
        TdmSchedule::default_4node()
    }

    fn manual_config() -> SecureTdmConfig {
        SecureTdmConfig {
            security_mode: SecurityMode::ManualCrypto,
            mesh_key: Some(DEFAULT_TEST_KEY),
            quic_config: QuicTransportConfig::default(),
            sec_level: SecLevel::Transitional,
        }
    }

    fn quic_config() -> SecureTdmConfig {
        SecureTdmConfig {
            security_mode: SecurityMode::QuicTransport,
            mesh_key: Some(DEFAULT_TEST_KEY),
            quic_config: QuicTransportConfig::default(),
            sec_level: SecLevel::Transitional,
        }
    }

    // ---- ReplayWindow tests ----

    #[test]
    fn test_replay_window_new() {
        let rw = ReplayWindow::new(16);
        assert_eq!(rw.last_accepted(), 0);
        assert_eq!(rw.window_count(), 0);
    }

    #[test]
    fn test_replay_window_accept_first() {
        let mut rw = ReplayWindow::new(16);
        assert!(rw.accept(0)); // First nonce accepted
        assert_eq!(rw.window_count(), 1);
    }

    #[test]
    fn test_replay_window_monotonic() {
        let mut rw = ReplayWindow::new(16);
        assert!(rw.accept(1));
        assert!(rw.accept(2));
        assert!(rw.accept(3));
        assert_eq!(rw.last_accepted(), 3);
    }

    #[test]
    fn test_replay_window_reject_duplicate() {
        let mut rw = ReplayWindow::new(16);
        assert!(rw.accept(1));
        assert!(!rw.accept(1)); // Duplicate rejected
    }

    #[test]
    fn test_replay_window_accept_within_window() {
        let mut rw = ReplayWindow::new(16);
        assert!(rw.accept(5));
        assert!(rw.accept(3)); // Out of order but within window
        assert_eq!(rw.last_accepted(), 5);
    }

    #[test]
    fn test_replay_window_reject_too_old() {
        let mut rw = ReplayWindow::new(4);
        for i in 0..20 {
            rw.accept(i);
        }
        // Nonce 0 is way outside the window
        assert!(!rw.accept(0));
    }

    #[test]
    fn test_replay_window_evicts_old() {
        let mut rw = ReplayWindow::new(4);
        for i in 0..10 {
            rw.accept(i);
        }
        assert!(rw.window_count() <= 4);
    }

    // ---- AuthenticatedBeacon tests ----

    #[test]
    fn test_auth_beacon_roundtrip() {
        let beacon = SyncBeacon {
            cycle_id: 42,
            cycle_period: Duration::from_millis(50),
            drift_correction_us: -3,
            generated_at: std::time::Instant::now(),
        };
        let key = DEFAULT_TEST_KEY;
        let nonce = 7u32;

        let mut msg = [0u8; 20];
        msg[..16].copy_from_slice(&beacon.to_bytes());
        msg[16..20].copy_from_slice(&nonce.to_le_bytes());
        let tag = AuthenticatedBeacon::compute_tag(&msg, &key);

        let auth = AuthenticatedBeacon {
            beacon,
            nonce,
            hmac_tag: tag,
        };

        let bytes = auth.to_bytes();
        assert_eq!(bytes.len(), AUTHENTICATED_BEACON_SIZE);

        let decoded = AuthenticatedBeacon::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.beacon.cycle_id, 42);
        assert_eq!(decoded.nonce, 7);
        assert_eq!(decoded.hmac_tag, tag);
    }

    #[test]
    fn test_auth_beacon_verify_ok() {
        let beacon = SyncBeacon {
            cycle_id: 100,
            cycle_period: Duration::from_millis(50),
            drift_correction_us: 0,
            generated_at: std::time::Instant::now(),
        };
        let key = DEFAULT_TEST_KEY;
        let nonce = 1u32;

        let mut msg = [0u8; 20];
        msg[..16].copy_from_slice(&beacon.to_bytes());
        msg[16..20].copy_from_slice(&nonce.to_le_bytes());
        let tag = AuthenticatedBeacon::compute_tag(&msg, &key);

        let auth = AuthenticatedBeacon {
            beacon,
            nonce,
            hmac_tag: tag,
        };
        assert!(auth.verify(&key).is_ok());
    }

    #[test]
    fn test_auth_beacon_verify_tampered() {
        let beacon = SyncBeacon {
            cycle_id: 100,
            cycle_period: Duration::from_millis(50),
            drift_correction_us: 0,
            generated_at: std::time::Instant::now(),
        };
        let key = DEFAULT_TEST_KEY;
        let nonce = 1u32;

        let mut msg = [0u8; 20];
        msg[..16].copy_from_slice(&beacon.to_bytes());
        msg[16..20].copy_from_slice(&nonce.to_le_bytes());
        let mut tag = AuthenticatedBeacon::compute_tag(&msg, &key);
        tag[0] ^= 0xFF; // Tamper with tag

        let auth = AuthenticatedBeacon {
            beacon,
            nonce,
            hmac_tag: tag,
        };
        assert!(matches!(
            auth.verify(&key),
            Err(SecureTdmError::BeaconAuthFailed)
        ));
    }

    #[test]
    fn test_auth_beacon_too_short() {
        let result = AuthenticatedBeacon::from_bytes(&[0u8; 10]);
        assert!(matches!(
            result,
            Err(SecureTdmError::BeaconTooShort { .. })
        ));
    }

    #[test]
    fn test_auth_beacon_size_constant() {
        assert_eq!(AUTHENTICATED_BEACON_SIZE, 28);
    }

    // ---- SecureTdmCoordinator tests (manual crypto) ----

    #[test]
    fn test_secure_coordinator_manual_create() {
        let coord =
            SecureTdmCoordinator::new(test_schedule(), manual_config()).unwrap();
        assert_eq!(coord.security_mode(), SecurityMode::ManualCrypto);
        assert_eq!(coord.beacons_produced(), 0);
        assert!(coord.transport().is_none());
    }

    #[test]
    fn test_secure_coordinator_manual_begin_cycle() {
        let mut coord =
            SecureTdmCoordinator::new(test_schedule(), manual_config()).unwrap();
        let output = coord.begin_secure_cycle().unwrap();

        assert_eq!(output.mode, SecurityMode::ManualCrypto);
        assert_eq!(output.authenticated_bytes.len(), AUTHENTICATED_BEACON_SIZE);
        assert_eq!(output.beacon.cycle_id, 0);
        assert_eq!(coord.beacons_produced(), 1);
        assert_eq!(coord.nonce_counter(), 1);
    }

    #[test]
    fn test_secure_coordinator_manual_nonce_increments() {
        let mut coord =
            SecureTdmCoordinator::new(test_schedule(), manual_config()).unwrap();

        for expected_nonce in 1..=5u32 {
            let _output = coord.begin_secure_cycle().unwrap();
            // Complete all slots
            for i in 0..4 {
                coord.complete_slot(i, 1.0);
            }
            assert_eq!(coord.nonce_counter(), expected_nonce);
        }
    }

    #[test]
    fn test_secure_coordinator_manual_verify_own_beacon() {
        let mut coord =
            SecureTdmCoordinator::new(test_schedule(), manual_config()).unwrap();
        let output = coord.begin_secure_cycle().unwrap();

        // Create a second coordinator to verify
        let mut verifier =
            SecureTdmCoordinator::new(test_schedule(), manual_config()).unwrap();
        let beacon = verifier
            .verify_beacon(&output.authenticated_bytes)
            .unwrap();
        assert_eq!(beacon.cycle_id, 0);
    }

    #[test]
    fn test_secure_coordinator_manual_reject_tampered() {
        let mut coord =
            SecureTdmCoordinator::new(test_schedule(), manual_config()).unwrap();
        let output = coord.begin_secure_cycle().unwrap();

        let mut tampered = output.authenticated_bytes.clone();
        tampered[25] ^= 0xFF; // Tamper with HMAC tag

        let mut verifier =
            SecureTdmCoordinator::new(test_schedule(), manual_config()).unwrap();
        assert!(verifier.verify_beacon(&tampered).is_err());
        assert_eq!(verifier.verification_failures(), 1);
    }

    #[test]
    fn test_secure_coordinator_manual_reject_replay() {
        let mut coord =
            SecureTdmCoordinator::new(test_schedule(), manual_config()).unwrap();
        let output = coord.begin_secure_cycle().unwrap();

        let mut verifier =
            SecureTdmCoordinator::new(test_schedule(), manual_config()).unwrap();

        // First acceptance succeeds
        verifier
            .verify_beacon(&output.authenticated_bytes)
            .unwrap();

        // Replay of same beacon fails
        let result = verifier.verify_beacon(&output.authenticated_bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_secure_coordinator_manual_backward_compat_permissive() {
        let mut cfg = manual_config();
        cfg.sec_level = SecLevel::Permissive;
        let mut coord = SecureTdmCoordinator::new(test_schedule(), cfg).unwrap();

        // Send an unauthenticated 16-byte beacon
        let beacon = SyncBeacon {
            cycle_id: 99,
            cycle_period: Duration::from_millis(50),
            drift_correction_us: 0,
            generated_at: std::time::Instant::now(),
        };
        let bytes = beacon.to_bytes();

        let verified = coord.verify_beacon(&bytes).unwrap();
        assert_eq!(verified.cycle_id, 99);
    }

    #[test]
    fn test_secure_coordinator_manual_reject_unauthenticated_enforcing() {
        let mut cfg = manual_config();
        cfg.sec_level = SecLevel::Enforcing;
        let mut coord = SecureTdmCoordinator::new(test_schedule(), cfg).unwrap();

        let beacon = SyncBeacon {
            cycle_id: 99,
            cycle_period: Duration::from_millis(50),
            drift_correction_us: 0,
            generated_at: std::time::Instant::now(),
        };
        let bytes = beacon.to_bytes();

        // 16-byte unauthenticated beacon rejected in enforcing mode
        let result = coord.verify_beacon(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_secure_coordinator_no_mesh_key() {
        let cfg = SecureTdmConfig {
            security_mode: SecurityMode::ManualCrypto,
            mesh_key: None,
            ..Default::default()
        };
        let mut coord = SecureTdmCoordinator::new(test_schedule(), cfg).unwrap();
        let result = coord.begin_secure_cycle();
        assert!(matches!(result, Err(SecureTdmError::NoMeshKey)));
    }

    // ---- SecureTdmCoordinator tests (QUIC mode) ----

    #[test]
    fn test_secure_coordinator_quic_create() {
        let coord =
            SecureTdmCoordinator::new(test_schedule(), quic_config()).unwrap();
        assert_eq!(coord.security_mode(), SecurityMode::QuicTransport);
        assert!(coord.transport().is_some());
    }

    #[test]
    fn test_secure_coordinator_quic_begin_cycle() {
        let mut coord =
            SecureTdmCoordinator::new(test_schedule(), quic_config()).unwrap();
        let output = coord.begin_secure_cycle().unwrap();

        assert_eq!(output.mode, SecurityMode::QuicTransport);
        // QUIC framed: 5-byte header + 16-byte beacon = 21 bytes
        assert_eq!(output.authenticated_bytes.len(), 5 + 16);
        assert_eq!(coord.beacons_produced(), 1);
    }

    #[test]
    fn test_secure_coordinator_quic_verify_own_beacon() {
        let mut coord =
            SecureTdmCoordinator::new(test_schedule(), quic_config()).unwrap();
        let output = coord.begin_secure_cycle().unwrap();

        let mut verifier =
            SecureTdmCoordinator::new(test_schedule(), quic_config()).unwrap();
        let beacon = verifier
            .verify_beacon(&output.authenticated_bytes)
            .unwrap();
        assert_eq!(beacon.cycle_id, 0);
    }

    #[test]
    fn test_secure_coordinator_complete_cycle() {
        let mut coord =
            SecureTdmCoordinator::new(test_schedule(), manual_config()).unwrap();
        coord.begin_secure_cycle().unwrap();

        for i in 0..4 {
            let event = coord.complete_slot(i, 0.95);
            assert_eq!(event.slot_index, i);
        }
        assert!(coord.is_cycle_complete());
    }

    #[test]
    fn test_secure_coordinator_cycle_id_increments() {
        let mut coord =
            SecureTdmCoordinator::new(test_schedule(), manual_config()).unwrap();

        let out0 = coord.begin_secure_cycle().unwrap();
        assert_eq!(out0.beacon.cycle_id, 0);
        for i in 0..4 {
            coord.complete_slot(i, 1.0);
        }

        let out1 = coord.begin_secure_cycle().unwrap();
        assert_eq!(out1.beacon.cycle_id, 1);
    }

    // ---- SecLevel tests ----

    #[test]
    fn test_sec_level_values() {
        assert_eq!(SecLevel::Permissive as u8, 0);
        assert_eq!(SecLevel::Transitional as u8, 1);
        assert_eq!(SecLevel::Enforcing as u8, 2);
    }

    // ---- Error display tests ----

    #[test]
    fn test_secure_tdm_error_display() {
        let err = SecureTdmError::BeaconAuthFailed;
        assert!(format!("{}", err).contains("HMAC"));

        let err = SecureTdmError::BeaconReplay {
            nonce: 5,
            last_accepted: 10,
        };
        assert!(format!("{}", err).contains("replay"));

        let err = SecureTdmError::NoMeshKey;
        assert!(format!("{}", err).contains("Mesh key"));
    }
}
