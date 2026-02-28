//! Shared Memory Protocol
//!
//! Defines the memory layout and protocol for passing tensor data
//! between the host and WASM kernels.

use crate::kernel::error::KernelError;
use crate::kernel::manifest::{DataType, KernelDescriptor};

/// WASM page size (64KB)
pub const PAGE_SIZE: usize = 65536;

/// Shared memory protocol for kernel invocation
///
/// Manages the layout of tensors and parameters in WASM linear memory.
#[derive(Debug, Clone)]
pub struct SharedMemoryProtocol {
    /// Total memory size in bytes
    total_size: usize,
    /// Current allocation offset
    current_offset: usize,
    /// Memory alignment (typically 8 or 16 bytes)
    alignment: usize,
}

impl SharedMemoryProtocol {
    /// Create a new memory protocol
    ///
    /// # Arguments
    /// * `total_pages` - Number of WASM pages to allocate
    /// * `alignment` - Memory alignment in bytes
    pub fn new(total_pages: usize, alignment: usize) -> Self {
        SharedMemoryProtocol {
            total_size: total_pages * PAGE_SIZE,
            current_offset: 0,
            alignment,
        }
    }

    /// Create with default settings (256 pages = 16MB, 16-byte alignment)
    pub fn default_settings() -> Self {
        Self::new(256, 16)
    }

    /// Reset allocator to beginning
    pub fn reset(&mut self) {
        self.current_offset = 0;
    }

    /// Align offset to boundary
    fn align_offset(&self, offset: usize) -> usize {
        (offset + self.alignment - 1) & !(self.alignment - 1)
    }

    /// Allocate memory region
    ///
    /// # Arguments
    /// * `size` - Size in bytes
    ///
    /// # Returns
    /// * `Ok(offset)` - Starting offset of allocated region
    /// * `Err` - If allocation would exceed total size
    pub fn allocate(&mut self, size: usize) -> Result<usize, KernelError> {
        let aligned_offset = self.align_offset(self.current_offset);
        let end_offset = aligned_offset + size;

        if end_offset > self.total_size {
            return Err(KernelError::AllocationFailed {
                requested_bytes: size,
            });
        }

        self.current_offset = end_offset;
        Ok(aligned_offset)
    }

    /// Get total memory size
    pub fn total_size(&self) -> usize {
        self.total_size
    }

    /// Get total pages
    pub fn total_pages(&self) -> usize {
        self.total_size / PAGE_SIZE
    }

    /// Get current allocation offset
    pub fn current_offset(&self) -> usize {
        self.current_offset
    }

    /// Get remaining available bytes
    pub fn remaining(&self) -> usize {
        self.total_size.saturating_sub(self.current_offset)
    }

    /// Check if a memory region is valid
    pub fn is_valid_region(&self, offset: usize, size: usize) -> bool {
        offset + size <= self.total_size
    }
}

impl Default for SharedMemoryProtocol {
    fn default() -> Self {
        Self::default_settings()
    }
}

/// Kernel invocation descriptor with memory layout
///
/// This is a higher-level wrapper around KernelDescriptor that helps
/// manage memory allocation and data transfer.
#[derive(Debug, Clone)]
pub struct KernelInvocationDescriptor {
    /// Low-level descriptor
    pub descriptor: KernelDescriptor,
    /// Memory protocol
    protocol: SharedMemoryProtocol,
}

impl KernelInvocationDescriptor {
    /// Create a new invocation descriptor
    pub fn new(total_pages: usize) -> Self {
        KernelInvocationDescriptor {
            descriptor: KernelDescriptor::new(),
            protocol: SharedMemoryProtocol::new(total_pages, 16),
        }
    }

    /// Create with default memory size
    pub fn default_size() -> Self {
        Self::new(256)
    }

    /// Allocate space for input tensor A
    pub fn allocate_input_a(&mut self, size: usize) -> Result<u32, KernelError> {
        let offset = self.protocol.allocate(size)?;
        self.descriptor.input_a_offset = offset as u32;
        self.descriptor.input_a_size = size as u32;
        Ok(offset as u32)
    }

    /// Allocate space for input tensor B
    pub fn allocate_input_b(&mut self, size: usize) -> Result<u32, KernelError> {
        let offset = self.protocol.allocate(size)?;
        self.descriptor.input_b_offset = offset as u32;
        self.descriptor.input_b_size = size as u32;
        Ok(offset as u32)
    }

    /// Allocate space for output tensor
    pub fn allocate_output(&mut self, size: usize) -> Result<u32, KernelError> {
        let offset = self.protocol.allocate(size)?;
        self.descriptor.output_offset = offset as u32;
        self.descriptor.output_size = size as u32;
        Ok(offset as u32)
    }

    /// Allocate scratch space
    pub fn allocate_scratch(&mut self, size: usize) -> Result<u32, KernelError> {
        let offset = self.protocol.allocate(size)?;
        self.descriptor.scratch_offset = offset as u32;
        self.descriptor.scratch_size = size as u32;
        Ok(offset as u32)
    }

    /// Allocate space for parameters
    pub fn allocate_params(&mut self, size: usize) -> Result<u32, KernelError> {
        let offset = self.protocol.allocate(size)?;
        self.descriptor.params_offset = offset as u32;
        self.descriptor.params_size = size as u32;
        Ok(offset as u32)
    }

    /// Get the low-level descriptor
    pub fn as_descriptor(&self) -> &KernelDescriptor {
        &self.descriptor
    }

    /// Get total allocated memory
    pub fn total_allocated(&self) -> usize {
        self.protocol.current_offset()
    }

    /// Get remaining memory
    pub fn remaining_memory(&self) -> usize {
        self.protocol.remaining()
    }

    /// Required pages for current allocation
    pub fn required_pages(&self) -> usize {
        (self.total_allocated() + PAGE_SIZE - 1) / PAGE_SIZE
    }
}

impl Default for KernelInvocationDescriptor {
    fn default() -> Self {
        Self::default_size()
    }
}

/// Memory region specification
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    /// Start offset in linear memory
    pub offset: u32,
    /// Size in bytes
    pub size: u32,
    /// Whether region is read-only
    pub read_only: bool,
}

impl MemoryRegion {
    /// Create a new memory region
    pub fn new(offset: u32, size: u32, read_only: bool) -> Self {
        MemoryRegion {
            offset,
            size,
            read_only,
        }
    }

    /// Create a read-only region
    pub fn read_only(offset: u32, size: u32) -> Self {
        Self::new(offset, size, true)
    }

    /// Create a writable region
    pub fn writable(offset: u32, size: u32) -> Self {
        Self::new(offset, size, false)
    }

    /// Get end offset (exclusive)
    pub fn end(&self) -> u32 {
        self.offset + self.size
    }

    /// Check if regions overlap
    pub fn overlaps(&self, other: &MemoryRegion) -> bool {
        self.offset < other.end() && other.offset < self.end()
    }
}

/// Calculate tensor size in bytes
///
/// # Arguments
/// * `shape` - Tensor shape (dimensions)
/// * `dtype` - Data type
///
/// # Returns
/// Size in bytes
pub fn tensor_size_bytes(shape: &[usize], dtype: DataType) -> usize {
    let num_elements: usize = shape.iter().product();
    num_elements * dtype.size_bytes()
}

/// Calculate required WASM pages for a given byte size
pub fn required_pages(size_bytes: usize) -> usize {
    (size_bytes + PAGE_SIZE - 1) / PAGE_SIZE
}

/// Memory layout validator
#[derive(Debug, Default)]
pub struct MemoryLayoutValidator {
    /// Registered regions
    regions: Vec<MemoryRegion>,
}

impl MemoryLayoutValidator {
    /// Create a new validator
    pub fn new() -> Self {
        MemoryLayoutValidator {
            regions: Vec::new(),
        }
    }

    /// Add a region to validate
    pub fn add_region(&mut self, region: MemoryRegion) -> Result<(), KernelError> {
        // Check for overlaps with existing regions
        for existing in &self.regions {
            if region.overlaps(existing) {
                return Err(KernelError::InvalidParameters {
                    description: format!(
                        "Memory region overlap: [{}, {}) overlaps [{}, {})",
                        region.offset,
                        region.end(),
                        existing.offset,
                        existing.end()
                    ),
                });
            }
        }

        self.regions.push(region);
        Ok(())
    }

    /// Validate a descriptor's memory layout
    pub fn validate_descriptor(
        &self,
        desc: &KernelDescriptor,
        total_memory: usize,
    ) -> Result<(), KernelError> {
        // Check all regions are within bounds
        let regions = [
            ("input_a", desc.input_a_offset, desc.input_a_size),
            ("input_b", desc.input_b_offset, desc.input_b_size),
            ("output", desc.output_offset, desc.output_size),
            ("scratch", desc.scratch_offset, desc.scratch_size),
            ("params", desc.params_offset, desc.params_size),
        ];

        for (name, offset, size) in regions {
            if size > 0 {
                let end = (offset as usize) + (size as usize);
                if end > total_memory {
                    return Err(KernelError::MemoryAccessViolation { offset, size });
                }
            }
        }

        // Check for overlaps between output and inputs
        let output = MemoryRegion::writable(desc.output_offset, desc.output_size);

        if desc.input_a_size > 0 {
            let input_a = MemoryRegion::read_only(desc.input_a_offset, desc.input_a_size);
            if output.overlaps(&input_a) {
                return Err(KernelError::InvalidParameters {
                    description: "Output overlaps with input_a".to_string(),
                });
            }
        }

        if desc.input_b_size > 0 {
            let input_b = MemoryRegion::read_only(desc.input_b_offset, desc.input_b_size);
            if output.overlaps(&input_b) {
                return Err(KernelError::InvalidParameters {
                    description: "Output overlaps with input_b".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Clear all regions
    pub fn clear(&mut self) {
        self.regions.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_protocol() {
        let mut protocol = SharedMemoryProtocol::new(1, 16); // 1 page = 64KB

        let offset1 = protocol.allocate(1024).unwrap();
        assert_eq!(offset1, 0);

        let offset2 = protocol.allocate(2048).unwrap();
        assert!(offset2 >= 1024);
        assert_eq!(offset2 % 16, 0); // Aligned

        assert!(protocol.remaining() < PAGE_SIZE);
    }

    #[test]
    fn test_allocation_failure() {
        let mut protocol = SharedMemoryProtocol::new(1, 16);

        // Try to allocate more than available
        let result = protocol.allocate(PAGE_SIZE + 1);
        assert!(matches!(result, Err(KernelError::AllocationFailed { .. })));
    }

    #[test]
    fn test_invocation_descriptor() {
        let mut desc = KernelInvocationDescriptor::new(4); // 4 pages

        desc.allocate_input_a(1024).unwrap();
        desc.allocate_input_b(1024).unwrap();
        desc.allocate_output(1024).unwrap();
        desc.allocate_scratch(512).unwrap();
        desc.allocate_params(64).unwrap();

        assert!(desc.total_allocated() > 3600); // With alignment
        assert_eq!(desc.descriptor.input_a_size, 1024);
    }

    #[test]
    fn test_tensor_size() {
        let shape = [1, 512, 32, 128]; // batch, seq, heads, dim
        let size = tensor_size_bytes(&shape, DataType::F32);
        assert_eq!(size, 1 * 512 * 32 * 128 * 4); // 8MB
    }

    #[test]
    fn test_required_pages() {
        assert_eq!(required_pages(0), 0);
        assert_eq!(required_pages(1), 1);
        assert_eq!(required_pages(PAGE_SIZE), 1);
        assert_eq!(required_pages(PAGE_SIZE + 1), 2);
    }

    #[test]
    fn test_memory_region_overlap() {
        let r1 = MemoryRegion::new(0, 100, false);
        let r2 = MemoryRegion::new(50, 100, false);
        let r3 = MemoryRegion::new(100, 100, false);

        assert!(r1.overlaps(&r2));
        assert!(!r1.overlaps(&r3));
    }

    #[test]
    fn test_layout_validator() {
        let mut validator = MemoryLayoutValidator::new();

        // Add non-overlapping regions
        validator
            .add_region(MemoryRegion::new(0, 100, false))
            .unwrap();
        validator
            .add_region(MemoryRegion::new(100, 100, false))
            .unwrap();

        // Try to add overlapping region
        let result = validator.add_region(MemoryRegion::new(50, 100, false));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_descriptor() {
        let validator = MemoryLayoutValidator::new();
        let mut desc = KernelDescriptor::new();

        desc.input_a_offset = 0;
        desc.input_a_size = 1024;
        desc.output_offset = 1024;
        desc.output_size = 1024;

        // Should pass - no overlap
        assert!(validator.validate_descriptor(&desc, PAGE_SIZE).is_ok());

        // Should fail - output overlaps input
        desc.output_offset = 512;
        assert!(validator.validate_descriptor(&desc, PAGE_SIZE).is_err());
    }

    #[test]
    fn test_validate_bounds() {
        let validator = MemoryLayoutValidator::new();
        let mut desc = KernelDescriptor::new();

        desc.input_a_offset = 0;
        desc.input_a_size = PAGE_SIZE as u32 + 1; // Too big

        let result = validator.validate_descriptor(&desc, PAGE_SIZE);
        assert!(matches!(
            result,
            Err(KernelError::MemoryAccessViolation { .. })
        ));
    }
}
