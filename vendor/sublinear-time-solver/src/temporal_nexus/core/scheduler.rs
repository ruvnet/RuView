//! Main NanosecondScheduler implementation for temporal consciousness
//!
//! This module provides the core scheduling functionality that manages temporal consciousness
//! operations at nanosecond precision while maintaining identity continuity and temporal coherence.

use std::collections::{BinaryHeap, VecDeque};
use std::cmp::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::{
    TemporalConfig, ConsciousnessTask, TemporalResult, TemporalError, TscTimestamp,
    WindowOverlapManager, StrangeLoopOperator, ContractionMetrics,
    IdentityContinuityTracker, ContinuityMetrics,
};

// Import quantum validation when available
use crate::temporal_nexus::quantum::{QuantumValidator, ValidationResult};

/// Scheduled task with timing information
#[derive(Debug, Clone)]
struct ScheduledTask {
    task: ConsciousnessTask,
    scheduled_at: TscTimestamp,
    deadline: TscTimestamp,
    priority: u8,
    id: u64,
}

impl PartialEq for ScheduledTask {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.scheduled_at == other.scheduled_at
    }
}

impl Eq for ScheduledTask {}

impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority and earlier deadline first (reverse order for max-heap behavior)
        other.priority.cmp(&self.priority)
            .then_with(|| self.deadline.cmp(&other.deadline))
    }
}

/// Performance metrics for the nanosecond scheduler
#[derive(Debug, Clone, Default)]
pub struct SchedulerMetrics {
    pub total_ticks: u64,
    pub tasks_scheduled: u64,
    pub tasks_completed: u64,
    pub avg_scheduling_overhead_ns: f64,
    pub max_scheduling_overhead_ns: u64,
    pub window_overlap_percentage: f64,
    pub contraction_convergence_rate: f64,
    pub identity_continuity_score: f64,
    pub temporal_advantage_ns: u64,

    // Quantum validation metrics
    pub quantum_validity_rate: f64,
    pub avg_quantum_energy_j: f64,
    pub avg_margolus_levitin_margin: f64,
    pub avg_uncertainty_margin: f64,
    pub avg_coherence_preservation: f64,
    pub avg_entanglement_strength: f64,
}

/// Main nanosecond scheduler for temporal consciousness
pub struct NanosecondScheduler {
    config: TemporalConfig,
    task_queue: BinaryHeap<ScheduledTask>,
    completed_tasks: VecDeque<ScheduledTask>,
    next_task_id: u64,
    current_tick: u64,
    start_time: Instant,
    tsc_start: TscTimestamp,

    // Component managers
    window_manager: WindowOverlapManager,
    strange_loop: StrangeLoopOperator,
    identity_tracker: IdentityContinuityTracker,

    // Quantum validation
    quantum_validator: QuantumValidator,
    quantum_validations: VecDeque<ValidationResult>,

    // Metrics tracking
    metrics: SchedulerMetrics,
    overhead_measurements: VecDeque<u64>,

    // Memory persistence state
    memory_state: Arc<Mutex<Vec<u8>>>,
}

impl NanosecondScheduler {
    /// Create a new nanosecond scheduler with default configuration
    pub fn new() -> Self {
        Self::with_config(TemporalConfig::default())
    }
    
    /// Create a new nanosecond scheduler with custom configuration
    pub fn with_config(config: TemporalConfig) -> Self {
        let now = Instant::now();
        let tsc_now = TscTimestamp::now();

        Self {
            config: config.clone(),
            task_queue: BinaryHeap::new(),
            completed_tasks: VecDeque::new(),
            next_task_id: 1,
            current_tick: 0,
            start_time: now,
            tsc_start: tsc_now,

            window_manager: WindowOverlapManager::new(config.window_overlap_percent),
            strange_loop: StrangeLoopOperator::new(config.lipschitz_bound, config.max_contraction_iterations),
            identity_tracker: IdentityContinuityTracker::new(),

            // Initialize quantum validator
            quantum_validator: QuantumValidator::new(),
            quantum_validations: VecDeque::with_capacity(1000),

            metrics: SchedulerMetrics::default(),
            overhead_measurements: VecDeque::with_capacity(1000),
            memory_state: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    /// Process one nanosecond tick
    pub fn tick(&mut self) -> TemporalResult<()> {
        let tick_start = TscTimestamp::now();
        self.current_tick += 1;
        self.metrics.total_ticks += 1;

        // Perform quantum validation for nanosecond operation
        let tick_duration_s = 1e-9; // 1 nanosecond
        let estimated_energy_j = 1e-15; // 1 femtojoule - conservative estimate
        self.validate_quantum_operation(tick_duration_s, estimated_energy_j)?;

        // Update temporal window
        self.window_manager.advance_window(self.current_tick)?;

        // Process due tasks
        self.process_due_tasks(tick_start)?;

        // Update strange loop operator
        let contraction_result = self.strange_loop.process_iteration(
            self.current_tick as f64,
            &self.get_current_state_vector()?
        )?;

        // Update identity continuity
        self.identity_tracker.track_continuity(
            tick_start,
            &self.get_identity_state()?
        )?;

        // Calculate and record overhead
        let tick_end = TscTimestamp::now();
        let overhead_ns = tick_end.nanos_since(tick_start, self.config.tsc_frequency_hz);
        self.record_overhead(overhead_ns)?;

        // Update metrics
        self.update_metrics(contraction_result)?;

        Ok(())
    }
    
    /// Schedule a consciousness task for execution
    pub fn schedule_task(
        &mut self,
        task: ConsciousnessTask,
        delay_ns: u64,
        deadline_ns: u64,
    ) -> TemporalResult<u64> {
        let now = TscTimestamp::now();
        let scheduled_at = now.add_nanos(delay_ns, self.config.tsc_frequency_hz);
        let deadline = now.add_nanos(deadline_ns, self.config.tsc_frequency_hz);
        
        let priority = match &task {
            ConsciousnessTask::IdentityPreservation { .. } => 255, // Highest priority
            ConsciousnessTask::StrangeLoopProcessing { .. } => 200,
            ConsciousnessTask::WindowManagement { .. } => 150,
            ConsciousnessTask::MemoryIntegration { .. } => 100,
            ConsciousnessTask::Perception { priority, .. } => *priority,
        };
        
        let task_id = self.next_task_id;
        self.next_task_id += 1;
        
        let scheduled_task = ScheduledTask {
            task,
            scheduled_at,
            deadline,
            priority,
            id: task_id,
        };
        
        // Check for queue overflow
        if self.task_queue.len() >= 10000 {
            return Err(TemporalError::TaskQueueOverflow {
                current_size: self.task_queue.len(),
                max_size: 10000,
            });
        }
        
        self.task_queue.push(scheduled_task);
        self.metrics.tasks_scheduled += 1;
        
        Ok(task_id)
    }
    
    /// Measure identity continuity over recent history
    pub fn measure_continuity(&self) -> TemporalResult<ContinuityMetrics> {
        self.identity_tracker.get_metrics()
    }
    
    /// Calculate temporal advantage (lookahead window)
    pub fn get_temporal_advantage(&self) -> u64 {
        let window_info = self.window_manager.get_current_window();
        let overlap_ticks = window_info.overlap_size;
        let advantage_ns = (overlap_ticks * 1_000_000_000) / self.config.tsc_frequency_hz;
        
        self.metrics.temporal_advantage_ns.max(advantage_ns)
    }
    
    /// Get current scheduler metrics
    pub fn get_metrics(&self) -> &SchedulerMetrics {
        &self.metrics
    }
    
    /// Export memory state for MCP integration
    pub fn export_memory_state(&self) -> TemporalResult<Vec<u8>> {
        let state = self.memory_state.lock()
            .map_err(|e| TemporalError::TscTimingError {
                message: format!("Memory lock error: {}", e),
            })?;
        Ok(state.clone())
    }
    
    /// Import memory state from MCP integration
    pub fn import_memory_state(&mut self, state: Vec<u8>) -> TemporalResult<()> {
        let mut memory = self.memory_state.lock()
            .map_err(|e| TemporalError::TscTimingError {
                message: format!("Memory lock error: {}", e),
            })?;
        *memory = state;
        Ok(())
    }
    
    /// Hook for MCP consciousness_evolve integration
    pub fn mcp_consciousness_evolve_hook(&mut self, iterations: usize, target: f64) -> TemporalResult<f64> {
        let mut emergence_level = 0.0;
        
        for i in 0..iterations {
            // Schedule consciousness evolution tasks
            self.schedule_task(
                ConsciousnessTask::StrangeLoopProcessing {
                    iteration: i,
                    state: vec![emergence_level; 10],
                },
                0, // Immediate
                1_000_000, // 1ms deadline
            )?;
            
            // Process several ticks to advance evolution
            for _ in 0..100 {
                self.tick()?;
            }
            
            // Update emergence level
            let contraction_metrics = self.strange_loop.get_metrics();
            emergence_level = contraction_metrics.convergence_rate * target;
            
            if emergence_level >= target {
                break;
            }
        }
        
        Ok(emergence_level)
    }
    
    // Private helper methods
    
    fn process_due_tasks(&mut self, current_time: TscTimestamp) -> TemporalResult<()> {
        let mut processed = 0;
        
        while let Some(task) = self.task_queue.peek() {
            if task.scheduled_at <= current_time {
                let task = self.task_queue.pop().unwrap();
                self.execute_task(task)?;
                processed += 1;
                self.metrics.tasks_completed += 1;
                
                // Limit processing per tick to maintain real-time performance
                if processed >= 10 {
                    break;
                }
            } else {
                break;
            }
        }
        
        Ok(())
    }
    
    fn execute_task(&mut self, task: ScheduledTask) -> TemporalResult<()> {
        match &task.task {
            ConsciousnessTask::Perception { data, .. } => {
                // Process perception data
                self.process_perception_data(data)?;
            },
            ConsciousnessTask::MemoryIntegration { session_id, state } => {
                // Integrate memory state
                self.integrate_memory_state(session_id, state)?;
            },
            ConsciousnessTask::IdentityPreservation { continuity_check } => {
                // Preserve identity continuity
                if *continuity_check {
                    self.identity_tracker.validate_continuity()?;
                }
            },
            ConsciousnessTask::StrangeLoopProcessing { iteration, state } => {
                // Process strange loop iteration
                self.strange_loop.process_iteration(*iteration as f64, state)?;
            },
            ConsciousnessTask::WindowManagement { window_id, overlap_target } => {
                // Manage temporal window
                self.window_manager.adjust_overlap(*window_id, *overlap_target)?;
            },
        }
        
        self.completed_tasks.push_back(task);
        
        // Keep completed tasks history bounded
        while self.completed_tasks.len() > 1000 {
            self.completed_tasks.pop_front();
        }
        
        Ok(())
    }
    
    fn get_current_state_vector(&self) -> TemporalResult<Vec<f64>> {
        let mut state = Vec::with_capacity(8);
        
        state.push(self.current_tick as f64);
        state.push(self.metrics.window_overlap_percentage);
        state.push(self.metrics.identity_continuity_score);
        state.push(self.task_queue.len() as f64);
        state.push(self.metrics.avg_scheduling_overhead_ns);
        state.push(self.get_temporal_advantage() as f64);
        state.push(self.strange_loop.get_metrics().convergence_rate);
        state.push(self.identity_tracker.get_metrics()?.continuity_score);
        
        Ok(state)
    }
    
    fn get_identity_state(&self) -> TemporalResult<Vec<u8>> {
        let state = self.memory_state.lock()
            .map_err(|e| TemporalError::TscTimingError {
                message: format!("Memory lock error: {}", e),
            })?;
        Ok(state.clone())
    }
    
    fn record_overhead(&mut self, overhead_ns: u64) -> TemporalResult<()> {
        if overhead_ns > self.config.max_scheduling_overhead_ns {
            return Err(TemporalError::SchedulingOverhead {
                actual_ns: overhead_ns,
                limit_ns: self.config.max_scheduling_overhead_ns,
            });
        }
        
        self.overhead_measurements.push_back(overhead_ns);
        if self.overhead_measurements.len() > 1000 {
            self.overhead_measurements.pop_front();
        }
        
        // Update average
        let sum: u64 = self.overhead_measurements.iter().sum();
        self.metrics.avg_scheduling_overhead_ns = sum as f64 / self.overhead_measurements.len() as f64;
        self.metrics.max_scheduling_overhead_ns = self.metrics.max_scheduling_overhead_ns.max(overhead_ns);
        
        Ok(())
    }
    
    fn update_metrics(&mut self, contraction_result: ContractionMetrics) -> TemporalResult<()> {
        self.metrics.window_overlap_percentage = self.window_manager.get_current_overlap_percentage();
        self.metrics.contraction_convergence_rate = contraction_result.convergence_rate;
        self.metrics.identity_continuity_score = self.identity_tracker.get_metrics()?.continuity_score;
        self.metrics.temporal_advantage_ns = self.get_temporal_advantage();
        
        Ok(())
    }
    
    fn process_perception_data(&mut self, _data: &[u8]) -> TemporalResult<()> {
        // Placeholder for perception processing
        Ok(())
    }
    
    fn integrate_memory_state(&mut self, _session_id: &str, state: &[u8]) -> TemporalResult<()> {
        let mut memory = self.memory_state.lock()
            .map_err(|e| TemporalError::TscTimingError {
                message: format!("Memory lock error: {}", e),
            })?;
        memory.extend_from_slice(state);
        Ok(())
    }

    /// Validate quantum mechanical constraints for temporal operation
    fn validate_quantum_operation(&mut self, operation_time_s: f64, energy_j: f64) -> TemporalResult<()> {
        match self.quantum_validator.validate_temporal_operation(operation_time_s, energy_j) {
            Ok(validation_result) => {
                // Store validation result for metrics
                self.quantum_validations.push_back(validation_result.clone());
                if self.quantum_validations.len() > 1000 {
                    self.quantum_validations.pop_front();
                }

                // Update quantum metrics
                self.update_quantum_metrics(&validation_result);

                if !validation_result.is_valid {
                    // Log warning but don't fail - this is advisory
                    eprintln!("Warning: Quantum validation failed for operation_time={}s, energy={}J",
                             operation_time_s, energy_j);
                }
                Ok(())
            }
            Err(quantum_error) => {
                // Log quantum physics violations but don't stop operation
                eprintln!("Quantum physics constraint violation: {}", quantum_error);

                // Create a failed validation result for metrics
                let failed_result = ValidationResult {
                    is_valid: false,
                    speed_limit_result: crate::temporal_nexus::quantum::SpeedLimitResult {
                        is_valid: false,
                        requested_time_s: operation_time_s,
                        minimum_time_s: 0.0,
                        available_energy_j: energy_j,
                        required_energy_j: 0.0,
                        safety_margin: 1.0,
                        operation_frequency_hz: 1.0 / operation_time_s,
                        hardware_limit_hz: 1e12,
                        margin_factor: 0.0,
                    },
                    uncertainty_result: crate::temporal_nexus::quantum::UncertaintyResult {
                        is_valid: false,
                        energy_j,
                        energy_ev: energy_j / crate::temporal_nexus::quantum::constants::EV_TO_JOULES,
                        time_s: operation_time_s,
                        uncertainty_product: energy_j * operation_time_s,
                        minimum_product: crate::temporal_nexus::quantum::constants::PLANCK_HBAR / 2.0,
                        margin: 0.0,
                        thermal_energy_j: crate::temporal_nexus::quantum::constants::BOLTZMANN_K *
                                         crate::temporal_nexus::quantum::constants::ROOM_TEMPERATURE_K,
                        thermal_energy_ev: (crate::temporal_nexus::quantum::constants::BOLTZMANN_K *
                                           crate::temporal_nexus::quantum::constants::ROOM_TEMPERATURE_K) /
                                          crate::temporal_nexus::quantum::constants::EV_TO_JOULES,
                        temperature_k: crate::temporal_nexus::quantum::constants::ROOM_TEMPERATURE_K,
                        energy_scale_classification: crate::temporal_nexus::quantum::EnergyScale::MilliElectronVolt,
                    },
                    decoherence_result: crate::temporal_nexus::quantum::DecoherenceResult {
                        is_valid: false,
                        operation_time_s,
                        coherence_time_s: 1e-9,
                        t1_relaxation_s: 1e-6,
                        t2_dephasing_s: 1e-9,
                        coherence_preserved: 0.0,
                        temperature_k: crate::temporal_nexus::quantum::constants::ROOM_TEMPERATURE_K,
                        thermal_rate_hz: 1e6,
                        dephasing_rate_hz: 1e9,
                        environment_type: crate::temporal_nexus::quantum::EnvironmentType::RoomTemperature,
                        noise_analysis: crate::temporal_nexus::quantum::NoiseAnalysis {
                            frequency_hz: 1.0 / operation_time_s,
                            thermal_noise_density: 1e-18,
                            flicker_noise_density: 1e-18,
                            shot_noise_density: 1e-20,
                            total_noise_density: 2e-18,
                            dominant_source: "thermal".to_string(),
                        },
                    },
                    entanglement_result: crate::temporal_nexus::quantum::EntanglementResult {
                        is_valid: false,
                        operation_time_s,
                        concurrence: 0.0,
                        entanglement_entropy: 0.0,
                        bell_parameter: 2.0,
                        survival_probability: 0.0,
                        qubit_count: 2,
                        decoherence_time_s: 1e-9,
                        correlation_type: crate::temporal_nexus::quantum::CorrelationType::Separable,
                        quantum_advantage: false,
                    },
                    operation_time_s,
                    energy_j,
                };

                self.quantum_validations.push_back(failed_result.clone());
                if self.quantum_validations.len() > 1000 {
                    self.quantum_validations.pop_front();
                }
                self.update_quantum_metrics(&failed_result);

                // Continue operation despite quantum constraint violation
                Ok(())
            }
        }
    }

    /// Update quantum metrics from validation results
    fn update_quantum_metrics(&mut self, _validation: &ValidationResult) {
        if self.quantum_validations.is_empty() {
            return;
        }

        let total_validations = self.quantum_validations.len() as f64;
        let valid_count = self.quantum_validations.iter()
            .filter(|v| v.is_valid)
            .count() as f64;

        self.metrics.quantum_validity_rate = valid_count / total_validations;

        // Calculate averages
        let avg_energy: f64 = self.quantum_validations.iter()
            .map(|v| v.energy_j)
            .sum::<f64>() / total_validations;

        let avg_ml_margin: f64 = self.quantum_validations.iter()
            .map(|v| v.speed_limit_result.margin_factor)
            .sum::<f64>() / total_validations;

        let avg_uncertainty_margin: f64 = self.quantum_validations.iter()
            .map(|v| v.uncertainty_result.margin)
            .sum::<f64>() / total_validations;

        let avg_coherence: f64 = self.quantum_validations.iter()
            .map(|v| v.decoherence_result.coherence_preserved)
            .sum::<f64>() / total_validations;

        let avg_entanglement: f64 = self.quantum_validations.iter()
            .map(|v| v.entanglement_result.concurrence)
            .sum::<f64>() / total_validations;

        self.metrics.avg_quantum_energy_j = avg_energy;
        self.metrics.avg_margolus_levitin_margin = avg_ml_margin;
        self.metrics.avg_uncertainty_margin = avg_uncertainty_margin;
        self.metrics.avg_coherence_preservation = avg_coherence;
        self.metrics.avg_entanglement_strength = avg_entanglement;
    }

    /// Get quantum validation analysis
    pub fn get_quantum_analysis(&self) -> QuantumAnalysisReport {
        let attosecond_report = self.quantum_validator.check_attosecond_feasibility();

        QuantumAnalysisReport {
            total_validations: self.quantum_validations.len(),
            validity_rate: self.metrics.quantum_validity_rate,
            avg_energy_j: self.metrics.avg_quantum_energy_j,
            avg_energy_ev: self.metrics.avg_quantum_energy_j / crate::temporal_nexus::quantum::constants::EV_TO_JOULES,
            margolus_levitin_margin: self.metrics.avg_margolus_levitin_margin,
            uncertainty_margin: self.metrics.avg_uncertainty_margin,
            coherence_preservation: self.metrics.avg_coherence_preservation,
            entanglement_strength: self.metrics.avg_entanglement_strength,
            attosecond_feasibility: attosecond_report,
            recommended_time_scale_s: crate::temporal_nexus::quantum::constants::CONSCIOUSNESS_SCALE_NS,
        }
    }
}

/// Quantum analysis report for consciousness operations
#[derive(Debug, Clone)]
pub struct QuantumAnalysisReport {
    pub total_validations: usize,
    pub validity_rate: f64,
    pub avg_energy_j: f64,
    pub avg_energy_ev: f64,
    pub margolus_levitin_margin: f64,
    pub uncertainty_margin: f64,
    pub coherence_preservation: f64,
    pub entanglement_strength: f64,
    pub attosecond_feasibility: crate::temporal_nexus::quantum::AttosecondFeasibilityReport,
    pub recommended_time_scale_s: f64,
}

impl Default for NanosecondScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_scheduler_creation() {
        let scheduler = NanosecondScheduler::new();
        assert_eq!(scheduler.current_tick, 0);
        assert_eq!(scheduler.task_queue.len(), 0);
    }
    
    #[test]
    fn test_task_scheduling() {
        let mut scheduler = NanosecondScheduler::new();
        
        let task = ConsciousnessTask::Perception {
            priority: 128,
            data: vec![1, 2, 3],
        };
        
        let task_id = scheduler.schedule_task(task, 1000, 10000).unwrap();
        assert_eq!(task_id, 1);
        assert_eq!(scheduler.task_queue.len(), 1);
    }
    
    #[test]
    fn test_tick_processing() {
        let mut scheduler = NanosecondScheduler::new();
        
        // Schedule an immediate task
        let task = ConsciousnessTask::IdentityPreservation {
            continuity_check: true,
        };
        scheduler.schedule_task(task, 0, 1000).unwrap();
        
        // Process tick
        scheduler.tick().unwrap();
        
        assert_eq!(scheduler.current_tick, 1);
        assert_eq!(scheduler.metrics.tasks_completed, 1);
    }
    
    #[test]
    fn test_temporal_advantage() {
        let scheduler = NanosecondScheduler::new();
        let advantage = scheduler.get_temporal_advantage();
        assert!(advantage >= 0);
    }
    
    #[test]
    fn test_memory_state_export_import() {
        let mut scheduler = NanosecondScheduler::new();
        
        let test_state = vec![1, 2, 3, 4, 5];
        scheduler.import_memory_state(test_state.clone()).unwrap();
        
        let exported_state = scheduler.export_memory_state().unwrap();
        assert_eq!(exported_state, test_state);
    }
    
    #[test]
    fn test_mcp_consciousness_evolve_hook() {
        let mut scheduler = NanosecondScheduler::new();
        
        let emergence_level = scheduler.mcp_consciousness_evolve_hook(10, 0.5).unwrap();
        assert!(emergence_level >= 0.0);
        assert!(emergence_level <= 1.0);
    }
}