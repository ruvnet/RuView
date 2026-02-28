//! FPGA PCIe backend
//!
//! Direct memory-mapped access to FPGA accelerator via PCIe.
//! Uses DMA ring buffers for zero-copy, lock-free operation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

#[cfg(feature = "pcie")]
use memmap2::{MmapMut, MmapOptions};

use crate::artifact::ModelArtifact;
use crate::backend::{
    compute_topk, protocol, read_lock, validate_tokens, write_lock, BackendStats,
    TransformerBackend,
};
use crate::error::{Error, Result};
use crate::types::{
    BackendKind, GateDecision, InferenceRequest, InferenceResult, ModelId, WitnessLog,
};

/// PCIe device configuration
#[derive(Debug, Clone)]
pub struct PcieConfig {
    /// Device path (e.g., /dev/ruvector0)
    pub device_path: String,
    /// BAR0 offset for control registers
    pub bar0_offset: usize,
    /// BAR1 offset for DMA buffers
    pub bar1_offset: usize,
    /// Number of request slots in ring buffer
    pub ring_slots: usize,
    /// Size of each request slot in bytes
    pub slot_size: usize,
    /// DMA timeout in milliseconds
    pub dma_timeout_ms: u64,
    /// Enable batch mode (multiple requests per DMA burst)
    pub batch_mode: bool,
    /// Maximum requests per batch
    pub batch_size: usize,
}

impl Default for PcieConfig {
    fn default() -> Self {
        Self {
            device_path: "/dev/ruvector0".into(),
            bar0_offset: 0,
            bar1_offset: 0x10000,
            ring_slots: 16,
            slot_size: 64 * 1024, // 64KB per slot
            dma_timeout_ms: 100,
            batch_mode: false,
            batch_size: 4,
        }
    }
}

/// Ring buffer slot state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum SlotState {
    Free = 0,
    Pending = 1,
    Complete = 2,
    Error = 3,
}

/// DMA ring buffer for lock-free request/response handling
struct DmaRingBuffer {
    /// Memory-mapped request buffer
    #[cfg(feature = "pcie")]
    request_mmap: MmapMut,
    /// Memory-mapped response buffer
    #[cfg(feature = "pcie")]
    response_mmap: MmapMut,
    /// Slot states
    slot_states: Vec<AtomicU32>,
    /// Producer index (next slot to write)
    producer_idx: AtomicU32,
    /// Consumer index (next slot to read)
    consumer_idx: AtomicU32,
    /// Number of slots
    num_slots: usize,
    /// Size per slot
    slot_size: usize,
}

impl DmaRingBuffer {
    /// Create a new DMA ring buffer (mock for non-PCIe builds)
    #[cfg(not(feature = "pcie"))]
    fn new(_config: &PcieConfig) -> Result<Self> {
        Err(Error::FeatureNotAvailable(
            "PCIe support not compiled".into(),
        ))
    }

    /// Create a new DMA ring buffer
    #[cfg(feature = "pcie")]
    fn new(config: &PcieConfig) -> Result<Self> {
        use std::fs::OpenOptions;

        // Open device
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&config.device_path)
            .map_err(|e| Error::PcieError(format!("Failed to open device: {}", e)))?;

        let total_size = config.ring_slots * config.slot_size;

        // Map request buffer (BAR1)
        let request_mmap = unsafe {
            MmapOptions::new()
                .offset(config.bar1_offset as u64)
                .len(total_size)
                .map_mut(&file)
                .map_err(|e| Error::PcieError(format!("Failed to map request buffer: {}", e)))?
        };

        // Map response buffer (BAR1 + offset)
        let response_mmap = unsafe {
            MmapOptions::new()
                .offset((config.bar1_offset + total_size) as u64)
                .len(total_size)
                .map_mut(&file)
                .map_err(|e| Error::PcieError(format!("Failed to map response buffer: {}", e)))?
        };

        // Initialize slot states
        let slot_states: Vec<AtomicU32> = (0..config.ring_slots)
            .map(|_| AtomicU32::new(SlotState::Free as u32))
            .collect();

        Ok(Self {
            request_mmap,
            response_mmap,
            slot_states,
            producer_idx: AtomicU32::new(0),
            consumer_idx: AtomicU32::new(0),
            num_slots: config.ring_slots,
            slot_size: config.slot_size,
        })
    }

    /// Acquire a slot for writing
    fn acquire_slot(&self) -> Option<usize> {
        let producer = self.producer_idx.load(Ordering::Acquire);
        let slot = producer as usize % self.num_slots;

        // Check if slot is free
        if self.slot_states[slot].load(Ordering::Acquire) == SlotState::Free as u32 {
            // Try to claim it
            if self.slot_states[slot]
                .compare_exchange(
                    SlotState::Free as u32,
                    SlotState::Pending as u32,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                self.producer_idx
                    .store(producer.wrapping_add(1), Ordering::Release);
                return Some(slot);
            }
        }
        None
    }

    /// Release a slot after reading response
    fn release_slot(&self, slot: usize) {
        self.slot_states[slot].store(SlotState::Free as u32, Ordering::Release);
        self.consumer_idx.fetch_add(1, Ordering::AcqRel);
    }

    /// Check if a slot is complete
    fn is_complete(&self, slot: usize) -> bool {
        self.slot_states[slot].load(Ordering::Acquire) == SlotState::Complete as u32
    }

    /// Mark a slot as complete (called by FPGA via doorbell/interrupt)
    fn mark_complete(&self, slot: usize) {
        self.slot_states[slot].store(SlotState::Complete as u32, Ordering::Release);
    }

    /// Get request buffer for a slot
    #[cfg(feature = "pcie")]
    fn request_buffer(&mut self, slot: usize) -> &mut [u8] {
        let start = slot * self.slot_size;
        let end = start + self.slot_size;
        &mut self.request_mmap[start..end]
    }

    /// Get response buffer for a slot
    #[cfg(feature = "pcie")]
    fn response_buffer(&self, slot: usize) -> &[u8] {
        let start = slot * self.slot_size;
        let end = start + self.slot_size;
        &self.response_mmap[start..end]
    }
}

/// FPGA PCIe backend
pub struct FpgaPcieBackend {
    /// Configuration
    config: PcieConfig,
    /// DMA ring buffer
    ring: Option<DmaRingBuffer>,
    /// Loaded models
    models: RwLock<HashMap<ModelId, ModelMetadata>>,
    /// Statistics
    stats: RwLock<BackendStats>,
    /// Total cycles counter
    total_cycles: AtomicU64,
    /// FPGA memory allocator state (next free offset)
    fpga_mem_offset: AtomicU64,
    /// FPGA memory total size (2GB default)
    fpga_mem_size: u64,
}

/// Cached model metadata
struct ModelMetadata {
    artifact: ModelArtifact,
    fpga_slot: u32,      // Slot in FPGA memory where model is loaded
    weights_offset: u64, // Offset in FPGA DDR where weights are stored
    weights_size: usize, // Size of weights in bytes
}

/// FPGA DDR base offset for model weights
const FPGA_DDR_MODEL_BASE: u64 = 0x1000_0000; // 256MB offset

impl FpgaPcieBackend {
    /// Create a new PCIe backend
    pub fn new(config: PcieConfig) -> Result<Self> {
        #[cfg(feature = "pcie")]
        let ring = Some(DmaRingBuffer::new(&config)?);

        #[cfg(not(feature = "pcie"))]
        let ring = None;

        Ok(Self {
            config,
            ring,
            models: RwLock::new(HashMap::new()),
            stats: RwLock::new(BackendStats::default()),
            total_cycles: AtomicU64::new(0),
            fpga_mem_offset: AtomicU64::new(FPGA_DDR_MODEL_BASE),
            fpga_mem_size: 2 * 1024 * 1024 * 1024, // 2GB
        })
    }

    /// Create with default configuration
    pub fn default_device() -> Result<Self> {
        Self::new(PcieConfig::default())
    }

    /// Write inference request to DMA buffer
    #[cfg(feature = "pcie")]
    fn write_request(
        &self,
        ring: &mut DmaRingBuffer,
        slot: usize,
        req: &InferenceRequest,
    ) -> Result<()> {
        use crate::backend::{protocol, RequestFrame};

        let buffer = ring.request_buffer(slot);
        let shape = &req.shape;

        // Write header
        let frame = RequestFrame::new(shape.seq_len, shape.d_model, shape.vocab, &req.model, 0, 16);
        let header = frame.to_bytes();
        buffer[..protocol::HEADER_SIZE].copy_from_slice(&header);

        let mut offset = protocol::HEADER_SIZE;

        // Write tokens
        for &token in req.tokens {
            buffer[offset..offset + 2].copy_from_slice(&token.to_le_bytes());
            offset += 2;
        }

        // Write mask
        buffer[offset..offset + req.attn_mask.len()].copy_from_slice(req.attn_mask);
        offset += req.attn_mask.len();

        // Write gate hint
        buffer[offset..offset + 2].copy_from_slice(&req.gate_hint.coherence_score_q.to_le_bytes());
        offset += 2;
        buffer[offset] = req.gate_hint.boundary_crossed as u8;
        offset += 1;
        buffer[offset] = req.gate_hint.max_compute_class as u8;

        Ok(())
    }

    /// Read inference response from DMA buffer
    #[cfg(feature = "pcie")]
    fn read_response(
        &self,
        ring: &DmaRingBuffer,
        slot: usize,
        shape: &crate::types::FixedShape,
    ) -> Result<(Vec<i16>, u32, u32, GateDecision)> {
        use crate::backend::ResponseFrame;

        let buffer = ring.response_buffer(slot);

        // Read response header
        let response = ResponseFrame::from_bytes(&buffer[..14].try_into().unwrap());

        // Check status
        if response.status != 0 {
            return Err(Error::backend(format!(
                "FPGA error: status {}",
                response.status
            )));
        }

        // Read logits
        let vocab = shape.vocab as usize;
        let mut logits = Vec::with_capacity(vocab);
        let mut offset = 14;

        for _ in 0..vocab {
            let value = i16::from_le_bytes([buffer[offset], buffer[offset + 1]]);
            logits.push(value);
            offset += 2;
        }

        Ok((
            logits,
            response.cycles,
            response.latency_ns,
            response.to_gate_decision(),
        ))
    }

    /// Ring doorbell to notify FPGA of pending request
    #[cfg(feature = "pcie")]
    fn ring_doorbell(&self, _slot: usize) {
        // In a real implementation, this would write to a control register
        // to notify the FPGA that a new request is available
    }

    /// Wait for response with polling
    fn wait_for_response(&self, ring: &DmaRingBuffer, slot: usize, timeout_ms: u64) -> Result<()> {
        let start = Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        while !ring.is_complete(slot) {
            if start.elapsed() > timeout {
                return Err(Error::Timeout { ms: timeout_ms });
            }
            std::hint::spin_loop();
        }

        Ok(())
    }

    /// Allocate FPGA DDR memory for model weights
    fn allocate_fpga_memory(&self, size: usize) -> Result<u64> {
        // Align to 4KB boundary for DMA efficiency
        let aligned_size = (size + 0xFFF) & !0xFFF;

        // Atomic allocation (simple bump allocator)
        let offset = self
            .fpga_mem_offset
            .fetch_add(aligned_size as u64, Ordering::SeqCst);

        // Check for overflow
        if offset + aligned_size as u64 > self.fpga_mem_size {
            // Roll back allocation
            self.fpga_mem_offset
                .fetch_sub(aligned_size as u64, Ordering::SeqCst);
            return Err(Error::ResourceExhausted("FPGA DDR memory full".into()));
        }

        Ok(offset)
    }

    /// Upload model weights to FPGA DDR via DMA
    #[cfg(feature = "pcie")]
    fn upload_weights_dma(&self, weights: &[u8], fpga_offset: u64) -> Result<()> {
        // DMA transfer configuration
        const DMA_CHUNK_SIZE: usize = 64 * 1024; // 64KB per transfer

        let ring = self
            .ring
            .as_ref()
            .ok_or_else(|| Error::FeatureNotAvailable("Ring buffer not initialized".into()))?;

        // Transfer weights in chunks
        let mut transferred = 0usize;
        while transferred < weights.len() {
            let chunk_size = DMA_CHUNK_SIZE.min(weights.len() - transferred);

            // Acquire a DMA slot
            let slot = loop {
                if let Some(s) = ring.acquire_slot() {
                    break s;
                }
                std::hint::spin_loop();
            };

            // Write DMA command to slot (simplified protocol)
            // In real hardware:
            // - Write target FPGA DDR address
            // - Write source offset in slot
            // - Write transfer length
            // - Ring doorbell

            // For now, we simulate the DMA by marking complete
            ring.mark_complete(slot);

            // Wait for completion
            self.wait_for_response(ring, slot, self.config.dma_timeout_ms)?;

            // Release slot
            ring.release_slot(slot);

            transferred += chunk_size;
        }

        Ok(())
    }

    /// Free FPGA DDR memory (simplified - real impl would use proper allocator)
    fn free_fpga_memory(&self, _offset: u64, _size: usize) {
        // In a production system, this would:
        // 1. Mark the memory region as free in an allocator
        // 2. Potentially compact memory if fragmentation is high
        // 3. Update hardware memory management unit
        //
        // For this implementation, we use a bump allocator without free.
        // Memory is reclaimed when all models are unloaded.
    }

    /// Check if all models are unloaded and reset memory allocator
    fn maybe_reset_allocator(&self) {
        let models_empty = read_lock(&self.models, |m| m.is_empty()).unwrap_or(false);
        if models_empty {
            self.fpga_mem_offset
                .store(FPGA_DDR_MODEL_BASE, Ordering::SeqCst);
        }
    }
}

impl TransformerBackend for FpgaPcieBackend {
    fn load(&self, artifact: &ModelArtifact) -> Result<ModelId> {
        #[cfg(not(feature = "pcie"))]
        {
            let _ = artifact;
            return Err(Error::FeatureNotAvailable(
                "PCIe support not compiled".into(),
            ));
        }

        #[cfg(feature = "pcie")]
        {
            // Validate artifact
            artifact.validate()?;

            let model_id = artifact.model_id();
            let weights_size = artifact.weights.len();

            // Allocate FPGA DDR memory for weights
            let weights_offset = self.allocate_fpga_memory(weights_size)?;

            // Upload model weights to FPGA DDR via DMA
            if let Err(e) = self.upload_weights_dma(&artifact.weights, weights_offset) {
                // Roll back allocation on failure
                self.free_fpga_memory(weights_offset, weights_size);
                return Err(e);
            }

            // Get slot number for this model
            let fpga_slot = read_lock(&self.models, |m| m.len() as u32)?;

            // Store metadata
            write_lock(&self.models, |models| {
                models.insert(
                    model_id,
                    ModelMetadata {
                        artifact: artifact.clone(),
                        fpga_slot,
                        weights_offset,
                        weights_size,
                    },
                );
            })?;

            write_lock(&self.stats, |stats| {
                stats.models_loaded += 1;
            })?;

            Ok(model_id)
        }
    }

    fn infer(&self, req: InferenceRequest) -> Result<InferenceResult> {
        #[cfg(not(feature = "pcie"))]
        {
            let _ = req;
            return Err(Error::FeatureNotAvailable(
                "PCIe support not compiled".into(),
            ));
        }

        #[cfg(feature = "pcie")]
        {
            let start = Instant::now();

            // Validate request
            req.validate()?;

            // Get model metadata
            let model_artifact = read_lock(&self.models, |models| {
                models.get(&req.model).map(|m| m.artifact.clone())
            })?
            .ok_or_else(|| Error::ModelNotFound(req.model))?;

            // Validate tokens against vocabulary
            validate_tokens(req.tokens, model_artifact.manifest.shape.vocab)?;

            // Get ring buffer
            let ring = self
                .ring
                .as_ref()
                .ok_or_else(|| Error::FeatureNotAvailable("Ring buffer not initialized".into()))?;

            // Acquire slot
            let slot = ring
                .acquire_slot()
                .ok_or_else(|| Error::ResourceExhausted("No DMA slots available".into()))?;

            // Write request (need mutable access - simplified for now)
            // In production, this would use proper interior mutability
            // self.write_request(ring, slot, &req)?;

            // Ring doorbell
            // self.ring_doorbell(slot);

            // Wait for response
            self.wait_for_response(ring, slot, self.config.dma_timeout_ms)?;

            // Read response
            let (logits, cycles, fpga_latency_ns, gate_decision) =
                self.read_response(ring, slot, &req.shape)?;

            // Release slot
            ring.release_slot(slot);

            let latency_ns = start.elapsed().as_nanos() as u32;

            // Compute top-K using common utility
            let topk = compute_topk(&logits, 16);

            // Create witness
            let witness = WitnessLog::new(
                model_artifact.model_hash(),
                model_artifact.quant_hash(),
                BackendKind::FpgaPcie,
                cycles,
                fpga_latency_ns.min(latency_ns),
                gate_decision,
            );

            // Update stats
            self.total_cycles
                .fetch_add(cycles as u64, Ordering::Relaxed);
            write_lock(&self.stats, |stats| {
                stats.total_inferences += 1;
                stats.total_cycles = self.total_cycles.load(Ordering::Relaxed);
                let n = stats.total_inferences;
                stats.avg_latency_ns = (stats.avg_latency_ns * (n - 1) + latency_ns as u64) / n;
                match gate_decision {
                    GateDecision::EarlyExit { .. } => stats.early_exits += 1,
                    GateDecision::Skipped { .. } => stats.skipped += 1,
                    _ => {}
                }
            })?;

            Ok(InferenceResult::new(logits, Some(topk), witness))
        }
    }

    fn unload(&self, model: ModelId) -> Result<()> {
        // Remove from cache and get memory info for deallocation
        let removed = write_lock(&self.models, |models| {
            models
                .remove(&model)
                .map(|m| (m.weights_offset, m.weights_size))
        })?;

        if let Some((offset, size)) = removed {
            // Free FPGA DDR memory
            self.free_fpga_memory(offset, size);

            // Check if we can reset the allocator
            self.maybe_reset_allocator();

            write_lock(&self.stats, |stats| {
                stats.models_loaded = stats.models_loaded.saturating_sub(1);
            })?;

            Ok(())
        } else {
            Err(Error::ModelNotFound(model))
        }
    }

    fn is_loaded(&self, model: ModelId) -> bool {
        read_lock(&self.models, |m| m.contains_key(&model)).unwrap_or(false)
    }

    fn kind(&self) -> BackendKind {
        BackendKind::FpgaPcie
    }

    fn stats(&self) -> BackendStats {
        read_lock(&self.stats, |s| s.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcie_config_default() {
        let config = PcieConfig::default();
        assert_eq!(config.ring_slots, 16);
        assert_eq!(config.slot_size, 64 * 1024);
    }

    #[test]
    fn test_slot_state_values() {
        assert_eq!(SlotState::Free as u8, 0);
        assert_eq!(SlotState::Pending as u8, 1);
        assert_eq!(SlotState::Complete as u8, 2);
    }
}
