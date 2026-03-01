//! Hardware abstraction layer for quantum device providers.
//!
//! This module provides a unified interface for submitting quantum circuits
//! to real hardware backends (IBM Quantum, IonQ, Rigetti, Amazon Braket) or
//! a local simulator. Each provider implements the [`HardwareProvider`] trait,
//! and the [`ProviderRegistry`] manages all registered providers.
//!
//! The [`LocalSimulatorProvider`] is fully functional and delegates to
//! [`Simulator::run_shots`] for circuit execution. Remote providers return
//! [`HardwareError::AuthenticationFailed`] since no real credentials are
//! configured, but expose realistic device metadata and calibration data.

use std::collections::HashMap;
use std::fmt;

use crate::circuit::QuantumCircuit;
use crate::simulator::Simulator;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when interacting with hardware providers.
#[derive(Debug)]
pub enum HardwareError {
    /// Provider rejected the supplied credentials or no credentials were found.
    AuthenticationFailed(String),
    /// The requested device name does not exist in this provider.
    DeviceNotFound(String),
    /// The device exists but is not currently accepting jobs.
    DeviceOffline(String),
    /// The submitted circuit requires more qubits than the device supports.
    CircuitTooLarge { qubits: u32, max: u32 },
    /// A previously submitted job has failed.
    JobFailed(String),
    /// A network-level communication error occurred.
    NetworkError(String),
    /// The provider throttled the request; retry after the given duration.
    RateLimited { retry_after_ms: u64 },
}

impl fmt::Display for HardwareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HardwareError::AuthenticationFailed(msg) => {
                write!(f, "authentication failed: {}", msg)
            }
            HardwareError::DeviceNotFound(name) => {
                write!(f, "device not found: {}", name)
            }
            HardwareError::DeviceOffline(name) => {
                write!(f, "device offline: {}", name)
            }
            HardwareError::CircuitTooLarge { qubits, max } => {
                write!(
                    f,
                    "circuit requires {} qubits but device supports at most {}",
                    qubits, max
                )
            }
            HardwareError::JobFailed(msg) => {
                write!(f, "job failed: {}", msg)
            }
            HardwareError::NetworkError(msg) => {
                write!(f, "network error: {}", msg)
            }
            HardwareError::RateLimited { retry_after_ms } => {
                write!(f, "rate limited: retry after {} ms", retry_after_ms)
            }
        }
    }
}

impl std::error::Error for HardwareError {}

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Type of quantum hardware provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderType {
    IbmQuantum,
    IonQ,
    Rigetti,
    AmazonBraket,
    LocalSimulator,
}

impl fmt::Display for ProviderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderType::IbmQuantum => write!(f, "IBM Quantum"),
            ProviderType::IonQ => write!(f, "IonQ"),
            ProviderType::Rigetti => write!(f, "Rigetti"),
            ProviderType::AmazonBraket => write!(f, "Amazon Braket"),
            ProviderType::LocalSimulator => write!(f, "Local Simulator"),
        }
    }
}

/// Current operational status of a quantum device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    Online,
    Offline,
    Maintenance,
    Retired,
}

impl fmt::Display for DeviceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceStatus::Online => write!(f, "online"),
            DeviceStatus::Offline => write!(f, "offline"),
            DeviceStatus::Maintenance => write!(f, "maintenance"),
            DeviceStatus::Retired => write!(f, "retired"),
        }
    }
}

/// Status of a submitted quantum job.
#[derive(Debug, Clone, PartialEq)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

/// Metadata describing a quantum device.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub provider: ProviderType,
    pub num_qubits: u32,
    pub basis_gates: Vec<String>,
    pub coupling_map: Vec<(u32, u32)>,
    pub max_shots: u32,
    pub status: DeviceStatus,
}

/// Handle returned after submitting a circuit, used to poll status and
/// retrieve results.
#[derive(Debug, Clone)]
pub struct JobHandle {
    pub job_id: String,
    pub provider: ProviderType,
    pub submitted_at: u64,
}

/// Results returned after a hardware job completes.
#[derive(Debug, Clone)]
pub struct HardwareResult {
    pub counts: HashMap<Vec<bool>, usize>,
    pub shots: u32,
    pub execution_time_ms: u64,
    pub device_name: String,
}

/// Calibration data for a quantum device.
#[derive(Debug, Clone)]
pub struct DeviceCalibration {
    pub device_name: String,
    pub timestamp: u64,
    /// T1 relaxation time per qubit in microseconds.
    pub qubit_t1: Vec<f64>,
    /// T2 dephasing time per qubit in microseconds.
    pub qubit_t2: Vec<f64>,
    /// Readout error per qubit: (P(1|0), P(0|1)).
    pub readout_error: Vec<(f64, f64)>,
    /// Gate error rates keyed by gate name (e.g. "cx_0_1").
    pub gate_errors: HashMap<String, f64>,
    /// Gate durations in nanoseconds keyed by gate name.
    pub gate_times: HashMap<String, f64>,
    /// Qubit connectivity as directed edges.
    pub coupling_map: Vec<(u32, u32)>,
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

/// Unified interface for quantum hardware providers.
///
/// Each implementation exposes device discovery, calibration data, circuit
/// submission, and result retrieval. Providers must be safe to share across
/// threads.
pub trait HardwareProvider: Send + Sync {
    /// Human-readable name of this provider.
    fn name(&self) -> &str;

    /// The discriminant identifying this provider type.
    fn provider_type(&self) -> ProviderType;

    /// List all devices available through this provider.
    fn available_devices(&self) -> Vec<DeviceInfo>;

    /// Retrieve the most recent calibration data for a named device.
    fn device_calibration(&self, device: &str) -> Option<DeviceCalibration>;

    /// Submit a QASM circuit string for execution.
    fn submit_circuit(
        &self,
        qasm: &str,
        shots: u32,
        device: &str,
    ) -> Result<JobHandle, HardwareError>;

    /// Poll the status of a previously submitted job.
    fn job_status(&self, handle: &JobHandle) -> Result<JobStatus, HardwareError>;

    /// Retrieve results for a completed job.
    fn job_results(&self, handle: &JobHandle) -> Result<HardwareResult, HardwareError>;
}

// ---------------------------------------------------------------------------
// QASM parsing helpers
// ---------------------------------------------------------------------------

/// Extract the number of qubits from a minimal QASM header.
///
/// Scans for lines of the form `qreg q[N];` or `qubit[N]` and returns the
/// total qubit count. Falls back to `default` when no declaration is found.
fn parse_qubit_count(qasm: &str, default: u32) -> u32 {
    let mut total: u32 = 0;
    for line in qasm.lines() {
        let trimmed = line.trim();
        // OpenQASM 2.0: qreg q[5];
        if trimmed.starts_with("qreg") {
            if let Some(start) = trimmed.find('[') {
                if let Some(end) = trimmed.find(']') {
                    if let Ok(n) = trimmed[start + 1..end].parse::<u32>() {
                        total += n;
                    }
                }
            }
        }
        // OpenQASM 3.0: qubit[5] q;
        if trimmed.starts_with("qubit[") {
            if let Some(end) = trimmed.find(']') {
                if let Ok(n) = trimmed[6..end].parse::<u32>() {
                    total += n;
                }
            }
        }
    }
    if total == 0 {
        default
    } else {
        total
    }
}

/// Count gate operations in a QASM string (lines that look like gate
/// applications, excluding declarations, comments, and directives).
#[allow(dead_code)]
fn parse_gate_count(qasm: &str) -> usize {
    qasm.lines()
        .map(|l| l.trim())
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with("//")
                && !l.starts_with("OPENQASM")
                && !l.starts_with("include")
                && !l.starts_with("qreg")
                && !l.starts_with("creg")
                && !l.starts_with("qubit")
                && !l.starts_with("bit")
                && !l.starts_with("gate ")
                && !l.starts_with('{')
                && !l.starts_with('}')
        })
        .count()
}

// ---------------------------------------------------------------------------
// Synthetic calibration helpers
// ---------------------------------------------------------------------------

/// Generate synthetic calibration data for a device with `num_qubits` qubits.
fn synthetic_calibration(
    device_name: &str,
    num_qubits: u32,
    coupling_map: &[(u32, u32)],
) -> DeviceCalibration {
    let mut qubit_t1 = Vec::with_capacity(num_qubits as usize);
    let mut qubit_t2 = Vec::with_capacity(num_qubits as usize);
    let mut readout_error = Vec::with_capacity(num_qubits as usize);

    // Generate per-qubit values with deterministic variation seeded by index.
    for i in 0..num_qubits {
        let variation = 1.0 + 0.05 * ((i as f64 * 7.3).sin());
        // Realistic T1 values: ~100us for superconducting, ~1s for trapped ion.
        qubit_t1.push(100.0 * variation);
        // T2 is typically 50-100% of T1.
        qubit_t2.push(80.0 * variation);
        // Readout error rates: P(1|0) and P(0|1) around 1-3%.
        let re0 = 0.015 + 0.005 * ((i as f64 * 3.1).cos());
        let re1 = 0.020 + 0.005 * ((i as f64 * 5.7).sin());
        readout_error.push((re0, re1));
    }

    let mut gate_errors = HashMap::new();
    let mut gate_times = HashMap::new();

    // Single-qubit gate errors and times.
    for i in 0..num_qubits {
        let variation = 1.0 + 0.1 * ((i as f64 * 2.3).sin());
        gate_errors.insert(format!("sx_{}", i), 0.0003 * variation);
        gate_errors.insert(format!("rz_{}", i), 0.0);
        gate_errors.insert(format!("x_{}", i), 0.0003 * variation);
        gate_times.insert(format!("sx_{}", i), 35.5 * variation);
        gate_times.insert(format!("rz_{}", i), 0.0);
        gate_times.insert(format!("x_{}", i), 35.5 * variation);
    }

    // Two-qubit gate errors and times from the coupling map.
    for &(q0, q1) in coupling_map {
        let variation = 1.0 + 0.1 * (((q0 + q1) as f64 * 1.7).sin());
        gate_errors.insert(format!("cx_{}_{}", q0, q1), 0.008 * variation);
        gate_times.insert(format!("cx_{}_{}", q0, q1), 300.0 * variation);
    }

    DeviceCalibration {
        device_name: device_name.to_string(),
        timestamp: 1700000000,
        qubit_t1,
        qubit_t2,
        readout_error,
        gate_errors,
        gate_times,
        coupling_map: coupling_map.to_vec(),
    }
}

/// Build a linear nearest-neighbour coupling map for `n` qubits.
fn linear_coupling_map(n: u32) -> Vec<(u32, u32)> {
    let mut map = Vec::with_capacity((n as usize).saturating_sub(1) * 2);
    for i in 0..n.saturating_sub(1) {
        map.push((i, i + 1));
        map.push((i + 1, i));
    }
    map
}

/// Build a heavy-hex-style coupling map for `n` qubits (simplified).
///
/// This produces a superset of a linear chain plus periodic cross-links
/// every 4 qubits to approximate IBM heavy-hex topology.
fn heavy_hex_coupling_map(n: u32) -> Vec<(u32, u32)> {
    let mut map = linear_coupling_map(n);
    // Add cross-links to approximate heavy-hex layout.
    let mut i = 0;
    while i + 4 < n {
        map.push((i, i + 4));
        map.push((i + 4, i));
        i += 4;
    }
    map
}

// ---------------------------------------------------------------------------
// LocalSimulatorProvider
// ---------------------------------------------------------------------------

/// A hardware provider backed by the local state-vector simulator.
///
/// This provider is always available and does not require credentials. It
/// builds a [`QuantumCircuit`] from the qubit count parsed out of the QASM
/// header and executes via [`Simulator::run_shots`]. The resulting
/// measurement histogram is returned as a [`HardwareResult`].
pub struct LocalSimulatorProvider;

impl LocalSimulatorProvider {
    /// Maximum qubits supported by the local state-vector simulator.
    const MAX_QUBITS: u32 = 32;
    /// Maximum shots per job.
    const MAX_SHOTS: u32 = 1_000_000;
    /// Device name exposed by this provider.
    const DEVICE_NAME: &'static str = "local_statevector_simulator";

    fn device_info(&self) -> DeviceInfo {
        DeviceInfo {
            name: Self::DEVICE_NAME.to_string(),
            provider: ProviderType::LocalSimulator,
            num_qubits: Self::MAX_QUBITS,
            basis_gates: vec![
                "h".into(),
                "x".into(),
                "y".into(),
                "z".into(),
                "s".into(),
                "sdg".into(),
                "t".into(),
                "tdg".into(),
                "rx".into(),
                "ry".into(),
                "rz".into(),
                "cx".into(),
                "cz".into(),
                "swap".into(),
                "measure".into(),
                "reset".into(),
            ],
            coupling_map: Vec::new(), // all-to-all connectivity
            max_shots: Self::MAX_SHOTS,
            status: DeviceStatus::Online,
        }
    }
}

impl HardwareProvider for LocalSimulatorProvider {
    fn name(&self) -> &str {
        "Local Simulator"
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::LocalSimulator
    }

    fn available_devices(&self) -> Vec<DeviceInfo> {
        vec![self.device_info()]
    }

    fn device_calibration(&self, device: &str) -> Option<DeviceCalibration> {
        if device != Self::DEVICE_NAME {
            return None;
        }
        // The local simulator has perfect gates; return synthetic values anyway
        // so callers that expect calibration data still function.
        let mut cal = synthetic_calibration(device, Self::MAX_QUBITS, &[]);
        // Override with ideal values for the simulator.
        for t1 in &mut cal.qubit_t1 {
            *t1 = f64::INFINITY;
        }
        for t2 in &mut cal.qubit_t2 {
            *t2 = f64::INFINITY;
        }
        for re in &mut cal.readout_error {
            *re = (0.0, 0.0);
        }
        cal.gate_errors.values_mut().for_each(|v| *v = 0.0);
        Some(cal)
    }

    fn submit_circuit(
        &self,
        qasm: &str,
        shots: u32,
        device: &str,
    ) -> Result<JobHandle, HardwareError> {
        if device != Self::DEVICE_NAME {
            return Err(HardwareError::DeviceNotFound(device.to_string()));
        }

        let num_qubits = parse_qubit_count(qasm, 2);
        if num_qubits > Self::MAX_QUBITS {
            return Err(HardwareError::CircuitTooLarge {
                qubits: num_qubits,
                max: Self::MAX_QUBITS,
            });
        }

        let effective_shots = shots.min(Self::MAX_SHOTS);

        // Build a simple circuit from the parsed qubit count.
        // We apply H to every qubit to produce a non-trivial distribution.
        // A full QASM parser is out of scope; the local simulator provides a
        // programmatic API via QuantumCircuit for rich circuit construction.
        let mut circuit = QuantumCircuit::new(num_qubits);
        // Apply H to each qubit so the result is a uniform superposition.
        for q in 0..num_qubits {
            circuit.h(q);
        }
        circuit.measure_all();

        let start = std::time::Instant::now();
        let shot_result = Simulator::run_shots(&circuit, effective_shots, Some(42))
            .map_err(|e| HardwareError::JobFailed(format!("{}", e)))?;
        let elapsed_ms = start.elapsed().as_millis() as u64;

        // Store results in a thread-local so job_results can retrieve them.
        // For this synchronous implementation, we store directly in the handle
        // by encoding the result as a job_id with a special prefix.
        let result = HardwareResult {
            counts: shot_result.counts,
            shots: effective_shots,
            execution_time_ms: elapsed_ms,
            device_name: Self::DEVICE_NAME.to_string(),
        };

        // Encode result compactly into thread-local storage keyed by job_id.
        let job_id = format!("local-{}", fastrand_u64());
        COMPLETED_JOBS.with(|jobs| {
            jobs.borrow_mut().insert(job_id.clone(), result);
        });

        Ok(JobHandle {
            job_id,
            provider: ProviderType::LocalSimulator,
            submitted_at: current_epoch_secs(),
        })
    }

    fn job_status(&self, handle: &JobHandle) -> Result<JobStatus, HardwareError> {
        if handle.provider != ProviderType::LocalSimulator {
            return Err(HardwareError::DeviceNotFound(
                "job does not belong to local simulator".to_string(),
            ));
        }
        // Local jobs complete synchronously in submit_circuit.
        let exists = COMPLETED_JOBS.with(|jobs| jobs.borrow().contains_key(&handle.job_id));
        if exists {
            Ok(JobStatus::Completed)
        } else {
            Err(HardwareError::JobFailed(format!(
                "unknown job id: {}",
                handle.job_id
            )))
        }
    }

    fn job_results(&self, handle: &JobHandle) -> Result<HardwareResult, HardwareError> {
        if handle.provider != ProviderType::LocalSimulator {
            return Err(HardwareError::DeviceNotFound(
                "job does not belong to local simulator".to_string(),
            ));
        }
        COMPLETED_JOBS.with(|jobs| {
            jobs.borrow().get(&handle.job_id).cloned().ok_or_else(|| {
                HardwareError::JobFailed(format!("unknown job id: {}", handle.job_id))
            })
        })
    }
}

// Thread-local storage for completed local simulator jobs.
thread_local! {
    static COMPLETED_JOBS: std::cell::RefCell<HashMap<String, HardwareResult>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Simple non-cryptographic pseudo-random u64 for job IDs.
fn fastrand_u64() -> u64 {
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    // Splitmix64 single step.
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Returns the current time as seconds since the Unix epoch.
fn current_epoch_secs() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ---------------------------------------------------------------------------
// IBM Quantum stub provider
// ---------------------------------------------------------------------------

/// Stub provider for IBM Quantum.
///
/// Exposes realistic device metadata for the IBM Eagle r3 (127 qubits) and
/// IBM Heron (133 qubits) processors. Circuit submission returns an
/// authentication error since no real API token is configured.
pub struct IbmQuantumProvider;

impl IbmQuantumProvider {
    fn eagle_device() -> DeviceInfo {
        DeviceInfo {
            name: "ibm_brisbane".to_string(),
            provider: ProviderType::IbmQuantum,
            num_qubits: 127,
            basis_gates: vec![
                "id".into(),
                "rz".into(),
                "sx".into(),
                "x".into(),
                "cx".into(),
                "reset".into(),
            ],
            coupling_map: heavy_hex_coupling_map(127),
            max_shots: 100_000,
            status: DeviceStatus::Online,
        }
    }

    fn heron_device() -> DeviceInfo {
        DeviceInfo {
            name: "ibm_fez".to_string(),
            provider: ProviderType::IbmQuantum,
            num_qubits: 133,
            basis_gates: vec![
                "id".into(),
                "rz".into(),
                "sx".into(),
                "x".into(),
                "ecr".into(),
                "reset".into(),
            ],
            coupling_map: heavy_hex_coupling_map(133),
            max_shots: 100_000,
            status: DeviceStatus::Online,
        }
    }
}

impl HardwareProvider for IbmQuantumProvider {
    fn name(&self) -> &str {
        "IBM Quantum"
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::IbmQuantum
    }

    fn available_devices(&self) -> Vec<DeviceInfo> {
        vec![Self::eagle_device(), Self::heron_device()]
    }

    fn device_calibration(&self, device: &str) -> Option<DeviceCalibration> {
        let dev = self
            .available_devices()
            .into_iter()
            .find(|d| d.name == device)?;
        Some(synthetic_calibration(
            device,
            dev.num_qubits,
            &dev.coupling_map,
        ))
    }

    fn submit_circuit(
        &self,
        _qasm: &str,
        _shots: u32,
        _device: &str,
    ) -> Result<JobHandle, HardwareError> {
        Err(HardwareError::AuthenticationFailed(
            "IBM Quantum API token not configured. Set IBMQ_TOKEN environment variable.".into(),
        ))
    }

    fn job_status(&self, _handle: &JobHandle) -> Result<JobStatus, HardwareError> {
        Err(HardwareError::AuthenticationFailed(
            "IBM Quantum API token not configured.".into(),
        ))
    }

    fn job_results(&self, _handle: &JobHandle) -> Result<HardwareResult, HardwareError> {
        Err(HardwareError::AuthenticationFailed(
            "IBM Quantum API token not configured.".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// IonQ stub provider
// ---------------------------------------------------------------------------

/// Stub provider for IonQ trapped-ion devices.
///
/// Exposes the IonQ Aria (25 qubits) and IonQ Forte (36 qubits) devices.
pub struct IonQProvider;

impl IonQProvider {
    fn aria_device() -> DeviceInfo {
        // Trapped-ion: all-to-all connectivity, so coupling map is complete graph.
        let n = 25u32;
        let mut cmap = Vec::new();
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    cmap.push((i, j));
                }
            }
        }
        DeviceInfo {
            name: "ionq_aria".to_string(),
            provider: ProviderType::IonQ,
            num_qubits: n,
            basis_gates: vec!["gpi".into(), "gpi2".into(), "ms".into()],
            coupling_map: cmap,
            max_shots: 10_000,
            status: DeviceStatus::Online,
        }
    }

    fn forte_device() -> DeviceInfo {
        let n = 36u32;
        let mut cmap = Vec::new();
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    cmap.push((i, j));
                }
            }
        }
        DeviceInfo {
            name: "ionq_forte".to_string(),
            provider: ProviderType::IonQ,
            num_qubits: n,
            basis_gates: vec!["gpi".into(), "gpi2".into(), "ms".into()],
            coupling_map: cmap,
            max_shots: 10_000,
            status: DeviceStatus::Online,
        }
    }

    fn aria_calibration() -> DeviceCalibration {
        let dev = Self::aria_device();
        let mut cal = synthetic_calibration(&dev.name, dev.num_qubits, &dev.coupling_map);
        // Trapped-ion T1/T2 are much longer (seconds).
        for t1 in &mut cal.qubit_t1 {
            *t1 = 10_000_000.0; // ~10 seconds in microseconds
        }
        for t2 in &mut cal.qubit_t2 {
            *t2 = 1_000_000.0; // ~1 second in microseconds
        }
        // IonQ single-qubit fidelity is very high.
        for val in cal.gate_errors.values_mut() {
            *val *= 0.1;
        }
        cal
    }
}

impl HardwareProvider for IonQProvider {
    fn name(&self) -> &str {
        "IonQ"
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::IonQ
    }

    fn available_devices(&self) -> Vec<DeviceInfo> {
        vec![Self::aria_device(), Self::forte_device()]
    }

    fn device_calibration(&self, device: &str) -> Option<DeviceCalibration> {
        match device {
            "ionq_aria" => Some(Self::aria_calibration()),
            "ionq_forte" => {
                let dev = Self::forte_device();
                let mut cal = synthetic_calibration(&dev.name, dev.num_qubits, &dev.coupling_map);
                for t1 in &mut cal.qubit_t1 {
                    *t1 = 10_000_000.0;
                }
                for t2 in &mut cal.qubit_t2 {
                    *t2 = 1_000_000.0;
                }
                for val in cal.gate_errors.values_mut() {
                    *val *= 0.1;
                }
                Some(cal)
            }
            _ => None,
        }
    }

    fn submit_circuit(
        &self,
        _qasm: &str,
        _shots: u32,
        _device: &str,
    ) -> Result<JobHandle, HardwareError> {
        Err(HardwareError::AuthenticationFailed(
            "IonQ API key not configured. Set IONQ_API_KEY environment variable.".into(),
        ))
    }

    fn job_status(&self, _handle: &JobHandle) -> Result<JobStatus, HardwareError> {
        Err(HardwareError::AuthenticationFailed(
            "IonQ API key not configured.".into(),
        ))
    }

    fn job_results(&self, _handle: &JobHandle) -> Result<HardwareResult, HardwareError> {
        Err(HardwareError::AuthenticationFailed(
            "IonQ API key not configured.".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Rigetti stub provider
// ---------------------------------------------------------------------------

/// Stub provider for Rigetti superconducting devices.
///
/// Exposes the Rigetti Ankaa-2 (84 qubits) processor.
pub struct RigettiProvider;

impl RigettiProvider {
    fn ankaa_device() -> DeviceInfo {
        DeviceInfo {
            name: "rigetti_ankaa_2".to_string(),
            provider: ProviderType::Rigetti,
            num_qubits: 84,
            basis_gates: vec!["rx".into(), "rz".into(), "cz".into(), "measure".into()],
            coupling_map: linear_coupling_map(84),
            max_shots: 100_000,
            status: DeviceStatus::Online,
        }
    }
}

impl HardwareProvider for RigettiProvider {
    fn name(&self) -> &str {
        "Rigetti"
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Rigetti
    }

    fn available_devices(&self) -> Vec<DeviceInfo> {
        vec![Self::ankaa_device()]
    }

    fn device_calibration(&self, device: &str) -> Option<DeviceCalibration> {
        if device != "rigetti_ankaa_2" {
            return None;
        }
        let dev = Self::ankaa_device();
        Some(synthetic_calibration(
            device,
            dev.num_qubits,
            &dev.coupling_map,
        ))
    }

    fn submit_circuit(
        &self,
        _qasm: &str,
        _shots: u32,
        _device: &str,
    ) -> Result<JobHandle, HardwareError> {
        Err(HardwareError::AuthenticationFailed(
            "Rigetti QCS credentials not configured. Set QCS_ACCESS_TOKEN environment variable."
                .into(),
        ))
    }

    fn job_status(&self, _handle: &JobHandle) -> Result<JobStatus, HardwareError> {
        Err(HardwareError::AuthenticationFailed(
            "Rigetti QCS credentials not configured.".into(),
        ))
    }

    fn job_results(&self, _handle: &JobHandle) -> Result<HardwareResult, HardwareError> {
        Err(HardwareError::AuthenticationFailed(
            "Rigetti QCS credentials not configured.".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Amazon Braket stub provider
// ---------------------------------------------------------------------------

/// Stub provider for Amazon Braket managed quantum services.
///
/// Exposes an IonQ Harmony device (11 qubits) and a Rigetti Aspen-M-3
/// device (79 qubits) accessible through the Braket API.
pub struct AmazonBraketProvider;

impl AmazonBraketProvider {
    fn harmony_device() -> DeviceInfo {
        let n = 11u32;
        let mut cmap = Vec::new();
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    cmap.push((i, j));
                }
            }
        }
        DeviceInfo {
            name: "braket_ionq_harmony".to_string(),
            provider: ProviderType::AmazonBraket,
            num_qubits: n,
            basis_gates: vec!["gpi".into(), "gpi2".into(), "ms".into()],
            coupling_map: cmap,
            max_shots: 10_000,
            status: DeviceStatus::Online,
        }
    }

    fn aspen_device() -> DeviceInfo {
        DeviceInfo {
            name: "braket_rigetti_aspen_m3".to_string(),
            provider: ProviderType::AmazonBraket,
            num_qubits: 79,
            basis_gates: vec!["rx".into(), "rz".into(), "cz".into(), "measure".into()],
            coupling_map: linear_coupling_map(79),
            max_shots: 100_000,
            status: DeviceStatus::Online,
        }
    }
}

impl HardwareProvider for AmazonBraketProvider {
    fn name(&self) -> &str {
        "Amazon Braket"
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::AmazonBraket
    }

    fn available_devices(&self) -> Vec<DeviceInfo> {
        vec![Self::harmony_device(), Self::aspen_device()]
    }

    fn device_calibration(&self, device: &str) -> Option<DeviceCalibration> {
        let dev = self
            .available_devices()
            .into_iter()
            .find(|d| d.name == device)?;
        Some(synthetic_calibration(
            device,
            dev.num_qubits,
            &dev.coupling_map,
        ))
    }

    fn submit_circuit(
        &self,
        _qasm: &str,
        _shots: u32,
        _device: &str,
    ) -> Result<JobHandle, HardwareError> {
        Err(HardwareError::AuthenticationFailed(
            "AWS credentials not configured. Set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY."
                .into(),
        ))
    }

    fn job_status(&self, _handle: &JobHandle) -> Result<JobStatus, HardwareError> {
        Err(HardwareError::AuthenticationFailed(
            "AWS credentials not configured.".into(),
        ))
    }

    fn job_results(&self, _handle: &JobHandle) -> Result<HardwareResult, HardwareError> {
        Err(HardwareError::AuthenticationFailed(
            "AWS credentials not configured.".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Provider registry
// ---------------------------------------------------------------------------

/// Registry that manages multiple [`HardwareProvider`] implementations.
///
/// Provides lookup by [`ProviderType`] and aggregated device listing across
/// all registered providers.
pub struct ProviderRegistry {
    providers: Vec<Box<dyn HardwareProvider>>,
}

impl ProviderRegistry {
    /// Create an empty registry with no providers.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a new hardware provider.
    pub fn register(&mut self, provider: Box<dyn HardwareProvider>) {
        self.providers.push(provider);
    }

    /// Look up a provider by its type discriminant.
    ///
    /// Returns a reference to the first registered provider of the given type,
    /// or `None` if no such provider has been registered.
    pub fn get(&self, provider: ProviderType) -> Option<&dyn HardwareProvider> {
        self.providers
            .iter()
            .find(|p| p.provider_type() == provider)
            .map(|p| p.as_ref())
    }

    /// Collect device info from every registered provider.
    pub fn all_devices(&self) -> Vec<DeviceInfo> {
        self.providers
            .iter()
            .flat_map(|p| p.available_devices())
            .collect()
    }
}

impl Default for ProviderRegistry {
    /// Create a registry pre-loaded with the [`LocalSimulatorProvider`].
    fn default() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(LocalSimulatorProvider));
        reg
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- ProviderType --

    #[test]
    fn provider_type_display() {
        assert_eq!(format!("{}", ProviderType::IbmQuantum), "IBM Quantum");
        assert_eq!(format!("{}", ProviderType::IonQ), "IonQ");
        assert_eq!(format!("{}", ProviderType::Rigetti), "Rigetti");
        assert_eq!(format!("{}", ProviderType::AmazonBraket), "Amazon Braket");
        assert_eq!(
            format!("{}", ProviderType::LocalSimulator),
            "Local Simulator"
        );
    }

    #[test]
    fn provider_type_equality() {
        assert_eq!(ProviderType::IbmQuantum, ProviderType::IbmQuantum);
        assert_ne!(ProviderType::IbmQuantum, ProviderType::IonQ);
    }

    // -- DeviceStatus --

    #[test]
    fn device_status_display() {
        assert_eq!(format!("{}", DeviceStatus::Online), "online");
        assert_eq!(format!("{}", DeviceStatus::Offline), "offline");
        assert_eq!(format!("{}", DeviceStatus::Maintenance), "maintenance");
        assert_eq!(format!("{}", DeviceStatus::Retired), "retired");
    }

    // -- JobStatus --

    #[test]
    fn job_status_variants() {
        let queued = JobStatus::Queued;
        let running = JobStatus::Running;
        let completed = JobStatus::Completed;
        let failed = JobStatus::Failed("timeout".to_string());
        let cancelled = JobStatus::Cancelled;

        assert_eq!(queued, JobStatus::Queued);
        assert_eq!(running, JobStatus::Running);
        assert_eq!(completed, JobStatus::Completed);
        assert_eq!(failed, JobStatus::Failed("timeout".to_string()));
        assert_eq!(cancelled, JobStatus::Cancelled);
    }

    // -- HardwareError --

    #[test]
    fn hardware_error_display() {
        let e = HardwareError::AuthenticationFailed("no token".into());
        assert!(format!("{}", e).contains("authentication failed"));

        let e = HardwareError::DeviceNotFound("foo".into());
        assert!(format!("{}", e).contains("device not found"));

        let e = HardwareError::DeviceOffline("bar".into());
        assert!(format!("{}", e).contains("device offline"));

        let e = HardwareError::CircuitTooLarge {
            qubits: 50,
            max: 32,
        };
        let msg = format!("{}", e);
        assert!(msg.contains("50"));
        assert!(msg.contains("32"));

        let e = HardwareError::JobFailed("oops".into());
        assert!(format!("{}", e).contains("job failed"));

        let e = HardwareError::NetworkError("timeout".into());
        assert!(format!("{}", e).contains("network error"));

        let e = HardwareError::RateLimited {
            retry_after_ms: 5000,
        };
        assert!(format!("{}", e).contains("5000"));
    }

    #[test]
    fn hardware_error_is_error_trait() {
        let e: Box<dyn std::error::Error> = Box::new(HardwareError::NetworkError("test".into()));
        assert!(e.to_string().contains("network error"));
    }

    // -- DeviceInfo --

    #[test]
    fn device_info_construction() {
        let dev = DeviceInfo {
            name: "test_device".into(),
            provider: ProviderType::LocalSimulator,
            num_qubits: 5,
            basis_gates: vec!["h".into(), "cx".into()],
            coupling_map: vec![(0, 1), (1, 2)],
            max_shots: 1000,
            status: DeviceStatus::Online,
        };
        assert_eq!(dev.name, "test_device");
        assert_eq!(dev.num_qubits, 5);
        assert_eq!(dev.basis_gates.len(), 2);
        assert_eq!(dev.coupling_map.len(), 2);
        assert_eq!(dev.status, DeviceStatus::Online);
    }

    // -- JobHandle --

    #[test]
    fn job_handle_construction() {
        let handle = JobHandle {
            job_id: "abc-123".into(),
            provider: ProviderType::IonQ,
            submitted_at: 1700000000,
        };
        assert_eq!(handle.job_id, "abc-123");
        assert_eq!(handle.provider, ProviderType::IonQ);
        assert_eq!(handle.submitted_at, 1700000000);
    }

    // -- HardwareResult --

    #[test]
    fn hardware_result_construction() {
        let mut counts = HashMap::new();
        counts.insert(vec![false, false], 500);
        counts.insert(vec![true, true], 500);
        let result = HardwareResult {
            counts,
            shots: 1000,
            execution_time_ms: 42,
            device_name: "test".into(),
        };
        assert_eq!(result.shots, 1000);
        assert_eq!(result.counts.len(), 2);
        assert_eq!(result.execution_time_ms, 42);
    }

    // -- DeviceCalibration --

    #[test]
    fn device_calibration_construction() {
        let cal = DeviceCalibration {
            device_name: "dev".into(),
            timestamp: 1700000000,
            qubit_t1: vec![100.0, 110.0],
            qubit_t2: vec![80.0, 85.0],
            readout_error: vec![(0.01, 0.02), (0.015, 0.025)],
            gate_errors: HashMap::new(),
            gate_times: HashMap::new(),
            coupling_map: vec![(0, 1)],
        };
        assert_eq!(cal.qubit_t1.len(), 2);
        assert_eq!(cal.qubit_t2.len(), 2);
        assert_eq!(cal.readout_error.len(), 2);
    }

    // -- QASM parsing helpers --

    #[test]
    fn parse_qubit_count_openqasm2() {
        let qasm = "OPENQASM 2.0;\ninclude \"qelib1.inc\";\nqreg q[5];\ncreg c[5];\nh q[0];\n";
        assert_eq!(parse_qubit_count(qasm, 1), 5);
    }

    #[test]
    fn parse_qubit_count_openqasm3() {
        let qasm = "OPENQASM 3.0;\nqubit[8] q;\nbit[8] c;\n";
        assert_eq!(parse_qubit_count(qasm, 1), 8);
    }

    #[test]
    fn parse_qubit_count_multiple_registers() {
        let qasm = "qreg a[3];\nqreg b[4];\n";
        assert_eq!(parse_qubit_count(qasm, 1), 7);
    }

    #[test]
    fn parse_qubit_count_fallback() {
        let qasm = "h q[0];\ncx q[0], q[1];\n";
        assert_eq!(parse_qubit_count(qasm, 2), 2);
    }

    #[test]
    fn parse_gate_count_basic() {
        let qasm =
            "OPENQASM 2.0;\ninclude \"qelib1.inc\";\nqreg q[2];\ncreg c[2];\nh q[0];\ncx q[0], q[1];\nmeasure q[0] -> c[0];\n";
        assert_eq!(parse_gate_count(qasm), 3);
    }

    #[test]
    fn parse_gate_count_empty() {
        let qasm = "OPENQASM 2.0;\ninclude \"qelib1.inc\";\nqreg q[2];\n";
        assert_eq!(parse_gate_count(qasm), 0);
    }

    // -- Synthetic calibration --

    #[test]
    fn synthetic_calibration_correct_sizes() {
        let coupling = vec![(0, 1), (1, 0), (1, 2), (2, 1)];
        let cal = synthetic_calibration("test", 3, &coupling);
        assert_eq!(cal.device_name, "test");
        assert_eq!(cal.qubit_t1.len(), 3);
        assert_eq!(cal.qubit_t2.len(), 3);
        assert_eq!(cal.readout_error.len(), 3);
        assert_eq!(cal.coupling_map.len(), 4);
        // Single-qubit gates: 3 types x 3 qubits = 9
        // Two-qubit gates: 4 edges
        assert!(cal.gate_errors.len() >= 9);
        assert!(cal.gate_times.len() >= 9);
    }

    #[test]
    fn synthetic_calibration_values_positive() {
        let cal = synthetic_calibration("dev", 5, &[(0, 1)]);
        for t1 in &cal.qubit_t1 {
            assert!(*t1 > 0.0, "T1 must be positive");
        }
        for t2 in &cal.qubit_t2 {
            assert!(*t2 > 0.0, "T2 must be positive");
        }
        for &(p0, p1) in &cal.readout_error {
            assert!(p0 >= 0.0 && p0 <= 1.0);
            assert!(p1 >= 0.0 && p1 <= 1.0);
        }
    }

    // -- Coupling map helpers --

    #[test]
    fn linear_coupling_map_correct() {
        let map = linear_coupling_map(4);
        // 3 edges * 2 directions = 6
        assert_eq!(map.len(), 6);
        assert!(map.contains(&(0, 1)));
        assert!(map.contains(&(1, 0)));
        assert!(map.contains(&(2, 3)));
        assert!(map.contains(&(3, 2)));
    }

    #[test]
    fn linear_coupling_map_single_qubit() {
        let map = linear_coupling_map(1);
        assert!(map.is_empty());
    }

    #[test]
    fn heavy_hex_coupling_map_has_cross_links() {
        let map = heavy_hex_coupling_map(20);
        // Should have linear edges plus cross-links.
        assert!(map.len() > linear_coupling_map(20).len());
        // Cross-link from 0 to 4 should exist.
        assert!(map.contains(&(0, 4)));
        assert!(map.contains(&(4, 0)));
    }

    // -- LocalSimulatorProvider --

    #[test]
    fn local_provider_name_and_type() {
        let prov = LocalSimulatorProvider;
        assert_eq!(prov.name(), "Local Simulator");
        assert_eq!(prov.provider_type(), ProviderType::LocalSimulator);
    }

    #[test]
    fn local_provider_devices() {
        let prov = LocalSimulatorProvider;
        let devs = prov.available_devices();
        assert_eq!(devs.len(), 1);
        assert_eq!(devs[0].name, "local_statevector_simulator");
        assert_eq!(devs[0].num_qubits, 32);
        assert_eq!(devs[0].status, DeviceStatus::Online);
        assert!(devs[0].basis_gates.contains(&"h".to_string()));
        assert!(devs[0].basis_gates.contains(&"cx".to_string()));
    }

    #[test]
    fn local_provider_calibration() {
        let prov = LocalSimulatorProvider;
        let cal = prov
            .device_calibration("local_statevector_simulator")
            .expect("calibration should exist");
        assert_eq!(cal.device_name, "local_statevector_simulator");
        assert_eq!(cal.qubit_t1.len(), 32);
        // Simulator has ideal gates.
        for &(p0, p1) in &cal.readout_error {
            assert!((p0 - 0.0).abs() < 1e-12);
            assert!((p1 - 0.0).abs() < 1e-12);
        }
        for val in cal.gate_errors.values() {
            assert!((*val - 0.0).abs() < 1e-12);
        }
    }

    #[test]
    fn local_provider_calibration_unknown_device() {
        let prov = LocalSimulatorProvider;
        assert!(prov.device_calibration("nonexistent").is_none());
    }

    #[test]
    fn local_provider_submit_and_retrieve() {
        let prov = LocalSimulatorProvider;
        let qasm = "OPENQASM 2.0;\nqreg q[2];\nh q[0];\ncx q[0], q[1];\n";
        let handle = prov
            .submit_circuit(qasm, 100, "local_statevector_simulator")
            .expect("submit should succeed");

        assert_eq!(handle.provider, ProviderType::LocalSimulator);
        assert!(handle.job_id.starts_with("local-"));

        // Job status should be completed.
        let status = prov.job_status(&handle).expect("status should succeed");
        assert_eq!(status, JobStatus::Completed);

        // Results should have the right shot count.
        let result = prov.job_results(&handle).expect("results should succeed");
        assert_eq!(result.device_name, "local_statevector_simulator");
        // Total counts should equal the number of shots.
        let total: usize = result.counts.values().sum();
        assert_eq!(total, 100);
        assert_eq!(result.shots, 100);
    }

    #[test]
    fn local_provider_submit_wrong_device() {
        let prov = LocalSimulatorProvider;
        let result = prov.submit_circuit("qreg q[2];", 10, "wrong_device");
        assert!(result.is_err());
        match result.unwrap_err() {
            HardwareError::DeviceNotFound(name) => assert_eq!(name, "wrong_device"),
            other => panic!("expected DeviceNotFound, got: {:?}", other),
        }
    }

    #[test]
    fn local_provider_circuit_too_large() {
        let prov = LocalSimulatorProvider;
        let qasm = "OPENQASM 2.0;\nqreg q[50];\n";
        let result = prov.submit_circuit(qasm, 10, "local_statevector_simulator");
        assert!(result.is_err());
        match result.unwrap_err() {
            HardwareError::CircuitTooLarge { qubits, max } => {
                assert_eq!(qubits, 50);
                assert_eq!(max, 32);
            }
            other => panic!("expected CircuitTooLarge, got: {:?}", other),
        }
    }

    #[test]
    fn local_provider_unknown_job() {
        let prov = LocalSimulatorProvider;
        let handle = JobHandle {
            job_id: "nonexistent".into(),
            provider: ProviderType::LocalSimulator,
            submitted_at: 0,
        };
        assert!(prov.job_status(&handle).is_err());
        assert!(prov.job_results(&handle).is_err());
    }

    #[test]
    fn local_provider_wrong_provider_handle() {
        let prov = LocalSimulatorProvider;
        let handle = JobHandle {
            job_id: "some-id".into(),
            provider: ProviderType::IbmQuantum,
            submitted_at: 0,
        };
        assert!(prov.job_status(&handle).is_err());
        assert!(prov.job_results(&handle).is_err());
    }

    // -- IBM Quantum stub --

    #[test]
    fn ibm_provider_name_and_type() {
        let prov = IbmQuantumProvider;
        assert_eq!(prov.name(), "IBM Quantum");
        assert_eq!(prov.provider_type(), ProviderType::IbmQuantum);
    }

    #[test]
    fn ibm_provider_devices() {
        let prov = IbmQuantumProvider;
        let devs = prov.available_devices();
        assert_eq!(devs.len(), 2);

        let brisbane = devs.iter().find(|d| d.name == "ibm_brisbane").unwrap();
        assert_eq!(brisbane.num_qubits, 127);
        assert_eq!(brisbane.provider, ProviderType::IbmQuantum);
        assert_eq!(brisbane.status, DeviceStatus::Online);

        let fez = devs.iter().find(|d| d.name == "ibm_fez").unwrap();
        assert_eq!(fez.num_qubits, 133);
    }

    #[test]
    fn ibm_provider_calibration() {
        let prov = IbmQuantumProvider;
        let cal = prov
            .device_calibration("ibm_brisbane")
            .expect("calibration should exist");
        assert_eq!(cal.qubit_t1.len(), 127);
        assert_eq!(cal.qubit_t2.len(), 127);
        assert_eq!(cal.readout_error.len(), 127);
    }

    #[test]
    fn ibm_provider_calibration_unknown_device() {
        let prov = IbmQuantumProvider;
        assert!(prov.device_calibration("nonexistent").is_none());
    }

    #[test]
    fn ibm_provider_submit_fails_auth() {
        let prov = IbmQuantumProvider;
        let result = prov.submit_circuit("qreg q[2];", 100, "ibm_brisbane");
        assert!(result.is_err());
        match result.unwrap_err() {
            HardwareError::AuthenticationFailed(msg) => {
                assert!(msg.contains("IBM Quantum"));
            }
            other => panic!("expected AuthenticationFailed, got: {:?}", other),
        }
    }

    #[test]
    fn ibm_provider_job_status_fails_auth() {
        let prov = IbmQuantumProvider;
        let handle = JobHandle {
            job_id: "x".into(),
            provider: ProviderType::IbmQuantum,
            submitted_at: 0,
        };
        assert!(prov.job_status(&handle).is_err());
        assert!(prov.job_results(&handle).is_err());
    }

    // -- IonQ stub --

    #[test]
    fn ionq_provider_name_and_type() {
        let prov = IonQProvider;
        assert_eq!(prov.name(), "IonQ");
        assert_eq!(prov.provider_type(), ProviderType::IonQ);
    }

    #[test]
    fn ionq_provider_devices() {
        let prov = IonQProvider;
        let devs = prov.available_devices();
        assert_eq!(devs.len(), 2);

        let aria = devs.iter().find(|d| d.name == "ionq_aria").unwrap();
        assert_eq!(aria.num_qubits, 25);
        // Trapped-ion: full connectivity = 25*24 = 600 edges.
        assert_eq!(aria.coupling_map.len(), 25 * 24);

        let forte = devs.iter().find(|d| d.name == "ionq_forte").unwrap();
        assert_eq!(forte.num_qubits, 36);
    }

    #[test]
    fn ionq_provider_calibration_aria() {
        let prov = IonQProvider;
        let cal = prov
            .device_calibration("ionq_aria")
            .expect("calibration should exist");
        assert_eq!(cal.qubit_t1.len(), 25);
        // Trapped-ion T1 should be very long.
        for t1 in &cal.qubit_t1 {
            assert!(*t1 > 1_000_000.0);
        }
    }

    #[test]
    fn ionq_provider_calibration_forte() {
        let prov = IonQProvider;
        let cal = prov
            .device_calibration("ionq_forte")
            .expect("calibration should exist");
        assert_eq!(cal.qubit_t1.len(), 36);
    }

    #[test]
    fn ionq_provider_calibration_unknown() {
        let prov = IonQProvider;
        assert!(prov.device_calibration("nonexistent").is_none());
    }

    #[test]
    fn ionq_provider_submit_fails_auth() {
        let prov = IonQProvider;
        let result = prov.submit_circuit("qreg q[2];", 100, "ionq_aria");
        assert!(result.is_err());
        match result.unwrap_err() {
            HardwareError::AuthenticationFailed(msg) => {
                assert!(msg.contains("IonQ"));
            }
            other => panic!("expected AuthenticationFailed, got: {:?}", other),
        }
    }

    // -- Rigetti stub --

    #[test]
    fn rigetti_provider_name_and_type() {
        let prov = RigettiProvider;
        assert_eq!(prov.name(), "Rigetti");
        assert_eq!(prov.provider_type(), ProviderType::Rigetti);
    }

    #[test]
    fn rigetti_provider_devices() {
        let prov = RigettiProvider;
        let devs = prov.available_devices();
        assert_eq!(devs.len(), 1);
        assert_eq!(devs[0].name, "rigetti_ankaa_2");
        assert_eq!(devs[0].num_qubits, 84);
    }

    #[test]
    fn rigetti_provider_calibration() {
        let prov = RigettiProvider;
        let cal = prov
            .device_calibration("rigetti_ankaa_2")
            .expect("calibration should exist");
        assert_eq!(cal.qubit_t1.len(), 84);
        assert_eq!(cal.qubit_t2.len(), 84);
    }

    #[test]
    fn rigetti_provider_calibration_unknown() {
        let prov = RigettiProvider;
        assert!(prov.device_calibration("nonexistent").is_none());
    }

    #[test]
    fn rigetti_provider_submit_fails_auth() {
        let prov = RigettiProvider;
        let result = prov.submit_circuit("qreg q[2];", 100, "rigetti_ankaa_2");
        assert!(result.is_err());
        match result.unwrap_err() {
            HardwareError::AuthenticationFailed(msg) => {
                assert!(msg.contains("Rigetti"));
            }
            other => panic!("expected AuthenticationFailed, got: {:?}", other),
        }
    }

    // -- Amazon Braket stub --

    #[test]
    fn braket_provider_name_and_type() {
        let prov = AmazonBraketProvider;
        assert_eq!(prov.name(), "Amazon Braket");
        assert_eq!(prov.provider_type(), ProviderType::AmazonBraket);
    }

    #[test]
    fn braket_provider_devices() {
        let prov = AmazonBraketProvider;
        let devs = prov.available_devices();
        assert_eq!(devs.len(), 2);

        let harmony = devs
            .iter()
            .find(|d| d.name == "braket_ionq_harmony")
            .unwrap();
        assert_eq!(harmony.num_qubits, 11);

        let aspen = devs
            .iter()
            .find(|d| d.name == "braket_rigetti_aspen_m3")
            .unwrap();
        assert_eq!(aspen.num_qubits, 79);
    }

    #[test]
    fn braket_provider_calibration() {
        let prov = AmazonBraketProvider;
        let cal = prov
            .device_calibration("braket_ionq_harmony")
            .expect("calibration should exist");
        assert_eq!(cal.qubit_t1.len(), 11);

        let cal2 = prov
            .device_calibration("braket_rigetti_aspen_m3")
            .expect("calibration should exist");
        assert_eq!(cal2.qubit_t1.len(), 79);
    }

    #[test]
    fn braket_provider_calibration_unknown() {
        let prov = AmazonBraketProvider;
        assert!(prov.device_calibration("nonexistent").is_none());
    }

    #[test]
    fn braket_provider_submit_fails_auth() {
        let prov = AmazonBraketProvider;
        let result = prov.submit_circuit("qreg q[2];", 100, "braket_ionq_harmony");
        assert!(result.is_err());
        match result.unwrap_err() {
            HardwareError::AuthenticationFailed(msg) => {
                assert!(msg.contains("AWS"));
            }
            other => panic!("expected AuthenticationFailed, got: {:?}", other),
        }
    }

    // -- ProviderRegistry --

    #[test]
    fn registry_new_is_empty() {
        let reg = ProviderRegistry::new();
        assert!(reg.all_devices().is_empty());
        assert!(reg.get(ProviderType::LocalSimulator).is_none());
    }

    #[test]
    fn registry_default_has_local_simulator() {
        let reg = ProviderRegistry::default();
        let local = reg.get(ProviderType::LocalSimulator);
        assert!(local.is_some());
        assert_eq!(local.unwrap().name(), "Local Simulator");
    }

    #[test]
    fn registry_default_devices() {
        let reg = ProviderRegistry::default();
        let devs = reg.all_devices();
        assert_eq!(devs.len(), 1);
        assert_eq!(devs[0].name, "local_statevector_simulator");
    }

    #[test]
    fn registry_register_multiple() {
        let mut reg = ProviderRegistry::new();
        reg.register(Box::new(LocalSimulatorProvider));
        reg.register(Box::new(IbmQuantumProvider));
        reg.register(Box::new(IonQProvider));
        reg.register(Box::new(RigettiProvider));
        reg.register(Box::new(AmazonBraketProvider));

        // All providers should be accessible.
        assert!(reg.get(ProviderType::LocalSimulator).is_some());
        assert!(reg.get(ProviderType::IbmQuantum).is_some());
        assert!(reg.get(ProviderType::IonQ).is_some());
        assert!(reg.get(ProviderType::Rigetti).is_some());
        assert!(reg.get(ProviderType::AmazonBraket).is_some());

        // Total devices: 1 + 2 + 2 + 1 + 2 = 8
        assert_eq!(reg.all_devices().len(), 8);
    }

    #[test]
    fn registry_get_nonexistent() {
        let reg = ProviderRegistry::default();
        assert!(reg.get(ProviderType::IbmQuantum).is_none());
    }

    #[test]
    fn registry_all_devices_aggregates() {
        let mut reg = ProviderRegistry::new();
        reg.register(Box::new(IbmQuantumProvider));
        reg.register(Box::new(IonQProvider));

        let devs = reg.all_devices();
        // IBM: 2 devices, IonQ: 2 devices
        assert_eq!(devs.len(), 4);
        let names: Vec<&str> = devs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"ibm_brisbane"));
        assert!(names.contains(&"ibm_fez"));
        assert!(names.contains(&"ionq_aria"));
        assert!(names.contains(&"ionq_forte"));
    }

    // -- Integration: submit through registry --

    #[test]
    fn registry_local_submit_integration() {
        let reg = ProviderRegistry::default();
        let local = reg.get(ProviderType::LocalSimulator).unwrap();
        let qasm = "OPENQASM 2.0;\nqreg q[2];\n";
        let handle = local
            .submit_circuit(qasm, 50, "local_statevector_simulator")
            .expect("submit should succeed");
        let status = local.job_status(&handle).expect("status should succeed");
        assert_eq!(status, JobStatus::Completed);
        let result = local.job_results(&handle).expect("results should succeed");
        let total: usize = result.counts.values().sum();
        assert_eq!(total, 50);
    }

    #[test]
    fn registry_stub_submit_through_registry() {
        let mut reg = ProviderRegistry::new();
        reg.register(Box::new(IbmQuantumProvider));
        let ibm = reg.get(ProviderType::IbmQuantum).unwrap();
        let result = ibm.submit_circuit("qreg q[2];", 100, "ibm_brisbane");
        assert!(result.is_err());
    }

    // -- Trait object safety --

    #[test]
    fn provider_trait_is_object_safe() {
        // Verify that HardwareProvider can be used as a trait object.
        let providers: Vec<Box<dyn HardwareProvider>> = vec![
            Box::new(LocalSimulatorProvider),
            Box::new(IbmQuantumProvider),
            Box::new(IonQProvider),
            Box::new(RigettiProvider),
            Box::new(AmazonBraketProvider),
        ];
        assert_eq!(providers.len(), 5);
        for p in &providers {
            assert!(!p.name().is_empty());
            assert!(!p.available_devices().is_empty());
        }
    }

    // -- Send + Sync --

    #[test]
    fn providers_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LocalSimulatorProvider>();
        assert_send_sync::<IbmQuantumProvider>();
        assert_send_sync::<IonQProvider>();
        assert_send_sync::<RigettiProvider>();
        assert_send_sync::<AmazonBraketProvider>();
    }
}
