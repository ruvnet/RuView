//! Self-bootstrapping loader for WASM_SEG segments.
//!
//! When an RVF file contains embedded WASM modules (WASM_SEG, type 0x10),
//! this module provides the logic to discover and sequence them for
//! self-bootstrapping execution.
//!
//! # Bootstrap Resolution Order
//!
//! 1. Scan all segments for WASM_SEG (type 0x10)
//! 2. Parse the 64-byte WasmHeader from each
//! 3. Sort by `bootstrap_priority` (lower = earlier in chain)
//! 4. Resolve the bootstrap chain:
//!    - If a `Combined` role module exists → single-step bootstrap
//!    - If `Interpreter` + `Microkernel` both exist → two-step bootstrap
//!    - If only `Microkernel` exists → requires host WASM runtime
//!
//! # Self-Bootstrapping Property
//!
//! When an RVF file contains a WASM_SEG with `role = Interpreter` or
//! `role = Combined`, the file is **self-bootstrapping**: any host with
//! raw execution capability (the ability to run native code or interpret
//! bytecode) can execute the file's contents without any external runtime.
//!
//! This makes RVF "run anywhere compute exists."

extern crate alloc;

use alloc::vec::Vec;
use crate::segment::{SegmentInfo, parse_segments};

/// WASM_SEG type discriminant (matches rvf_types::SegmentType::Wasm).
const WASM_SEG_TYPE: u8 = 0x10;

/// WASM_SEG header magic: "RVWM" in little-endian.
const WASM_HEADER_MAGIC: u32 = 0x5256_574D;

/// WASM_SEG header size in bytes.
const WASM_HEADER_SIZE: usize = 64;

/// Role discriminants matching rvf_types::wasm_bootstrap::WasmRole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum WasmRole {
    Microkernel = 0x00,
    Interpreter = 0x01,
    Combined = 0x02,
    Extension = 0x03,
    ControlPlane = 0x04,
}

/// Parsed WASM module descriptor from a WASM_SEG.
#[derive(Clone, Debug)]
pub struct WasmModule {
    /// Role in the bootstrap chain.
    pub role: u8,
    /// Target platform.
    pub target: u8,
    /// Required WASM features bitfield.
    pub required_features: u16,
    /// Number of exports.
    pub export_count: u16,
    /// Uncompressed bytecode size.
    pub bytecode_size: u32,
    /// Bootstrap priority (lower = first).
    pub bootstrap_priority: u8,
    /// Interpreter type (if role=Interpreter).
    pub interpreter_type: u8,
    /// Byte offset of the WASM bytecode within the RVF file.
    pub bytecode_offset: usize,
    /// Length of the WASM bytecode.
    pub bytecode_len: usize,
    /// SHAKE-256-256 hash of the bytecode.
    pub bytecode_hash: [u8; 32],
}

/// Bootstrap chain describing how to execute this RVF file.
#[derive(Clone, Debug)]
pub enum BootstrapChain {
    /// File has no WASM_SEGs — requires external runtime for all processing.
    None,
    /// File contains only a microkernel — requires host WASM runtime.
    HostRequired {
        microkernel: WasmModule,
    },
    /// File contains a combined interpreter+microkernel — single-step bootstrap.
    SelfContained {
        combined: WasmModule,
    },
    /// File contains separate interpreter and microkernel — two-step bootstrap.
    TwoStage {
        interpreter: WasmModule,
        microkernel: WasmModule,
    },
    /// File contains interpreter, microkernel, and extensions.
    Full {
        interpreter: WasmModule,
        microkernel: WasmModule,
        extensions: Vec<WasmModule>,
    },
}

impl BootstrapChain {
    /// Returns true if this file can execute without any external WASM runtime.
    pub fn is_self_bootstrapping(&self) -> bool {
        matches!(
            self,
            BootstrapChain::SelfContained { .. }
                | BootstrapChain::TwoStage { .. }
                | BootstrapChain::Full { .. }
        )
    }
}

/// Parse a WasmModule descriptor from a WASM_SEG payload.
fn parse_wasm_module(buf: &[u8], seg_offset: usize) -> Option<WasmModule> {
    let payload_start = seg_offset + rvf_types::constants::SEGMENT_HEADER_SIZE;

    if buf.len() < payload_start + WASM_HEADER_SIZE {
        return None;
    }

    let hdr = &buf[payload_start..payload_start + WASM_HEADER_SIZE];

    // Verify magic
    let magic = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
    if magic != WASM_HEADER_MAGIC {
        return None;
    }

    let role = hdr[0x06];
    let target = hdr[0x07];
    let required_features = u16::from_le_bytes([hdr[0x08], hdr[0x09]]);
    let export_count = u16::from_le_bytes([hdr[0x0A], hdr[0x0B]]);
    let bytecode_size = u32::from_le_bytes([hdr[0x0C], hdr[0x0D], hdr[0x0E], hdr[0x0F]]);
    let bootstrap_priority = hdr[0x38];
    let interpreter_type = hdr[0x39];

    let mut bytecode_hash = [0u8; 32];
    bytecode_hash.copy_from_slice(&hdr[0x18..0x38]);

    let bytecode_offset = payload_start + WASM_HEADER_SIZE;
    let bytecode_len = bytecode_size as usize;

    Some(WasmModule {
        role,
        target,
        required_features,
        export_count,
        bytecode_size,
        bootstrap_priority,
        interpreter_type,
        bytecode_offset,
        bytecode_len,
        bytecode_hash,
    })
}

/// Discover and resolve the bootstrap chain from a raw RVF byte buffer.
///
/// This is the core self-bootstrapping resolver. Given the raw bytes of
/// an RVF file, it:
/// 1. Scans for all WASM_SEG segments
/// 2. Parses their WasmHeaders
/// 3. Sorts by bootstrap_priority
/// 4. Determines the optimal bootstrap strategy
///
/// The returned `BootstrapChain` tells the host exactly what it needs to do:
/// - `None` → use external runtime (file has no embedded WASM)
/// - `HostRequired` → use host's WASM runtime to run microkernel
/// - `SelfContained` → file bootstraps itself in one step
/// - `TwoStage` → file bootstraps: interpreter → microkernel
/// - `Full` → interpreter → microkernel → extensions
pub fn resolve_bootstrap_chain(buf: &[u8]) -> BootstrapChain {
    let segments = parse_segments(buf);

    let mut wasm_modules: Vec<WasmModule> = segments
        .iter()
        .filter(|seg| seg.seg_type == WASM_SEG_TYPE)
        .filter_map(|seg| parse_wasm_module(buf, seg.offset))
        .collect();

    if wasm_modules.is_empty() {
        return BootstrapChain::None;
    }

    // Sort by bootstrap priority (lower = first)
    wasm_modules.sort_by_key(|m| m.bootstrap_priority);

    // Check for combined module (single-step bootstrap)
    if let Some(idx) = wasm_modules.iter().position(|m| m.role == WasmRole::Combined as u8) {
        return BootstrapChain::SelfContained {
            combined: wasm_modules.remove(idx),
        };
    }

    let interpreter_idx = wasm_modules.iter().position(|m| m.role == WasmRole::Interpreter as u8);
    let microkernel_idx = wasm_modules.iter().position(|m| m.role == WasmRole::Microkernel as u8);

    match (interpreter_idx, microkernel_idx) {
        (Some(i_idx), Some(m_idx)) => {
            // Two-stage or full bootstrap
            // Remove in reverse order to preserve indices
            let (first, second) = if i_idx > m_idx {
                let interpreter = wasm_modules.remove(i_idx);
                let microkernel = wasm_modules.remove(m_idx);
                (microkernel, interpreter)
            } else {
                let microkernel = wasm_modules.remove(m_idx);
                let interpreter = wasm_modules.remove(i_idx);
                (interpreter, microkernel)
            };

            let extensions: Vec<WasmModule> = wasm_modules
                .into_iter()
                .filter(|m| m.role == WasmRole::Extension as u8)
                .collect();

            if extensions.is_empty() {
                BootstrapChain::TwoStage {
                    interpreter: first,
                    microkernel: second,
                }
            } else {
                BootstrapChain::Full {
                    interpreter: first,
                    microkernel: second,
                    extensions,
                }
            }
        }
        (None, Some(_)) => {
            // Only microkernel, no interpreter → host provides runtime
            let m_idx = wasm_modules.iter().position(|m| m.role == WasmRole::Microkernel as u8).unwrap();
            BootstrapChain::HostRequired {
                microkernel: wasm_modules.remove(m_idx),
            }
        }
        _ => {
            // No standard bootstrap chain found
            BootstrapChain::None
        }
    }
}

/// Get the raw WASM bytecode for a module from the RVF buffer.
///
/// Returns a slice into `buf` containing the WASM bytecode for the given
/// module. This avoids copying — the caller can feed the slice directly
/// to a WASM runtime.
pub fn get_bytecode<'a>(buf: &'a [u8], module: &WasmModule) -> Option<&'a [u8]> {
    let end = module.bytecode_offset.checked_add(module.bytecode_len)?;
    if end > buf.len() {
        return None;
    }
    Some(&buf[module.bytecode_offset..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal WASM_SEG for testing.
    fn build_wasm_seg(role: u8, priority: u8, bytecode: &[u8]) -> Vec<u8> {
        let seg_header_size = rvf_types::constants::SEGMENT_HEADER_SIZE;
        let payload_len = WASM_HEADER_SIZE + bytecode.len();

        let mut seg = Vec::with_capacity(seg_header_size + payload_len);

        // Segment header (64 bytes)
        seg.extend_from_slice(&rvf_types::constants::SEGMENT_MAGIC.to_le_bytes());
        seg.push(1); // version
        seg.push(WASM_SEG_TYPE); // seg_type
        seg.extend_from_slice(&[0, 0]); // flags
        seg.extend_from_slice(&1u64.to_le_bytes()); // segment_id
        seg.extend_from_slice(&(payload_len as u64).to_le_bytes()); // payload_length
        // Fill remaining header bytes to reach 64
        while seg.len() < seg_header_size {
            seg.push(0);
        }

        // WasmHeader (64 bytes)
        let mut wasm_hdr = [0u8; 64];
        wasm_hdr[0..4].copy_from_slice(&WASM_HEADER_MAGIC.to_le_bytes());
        wasm_hdr[0x04..0x06].copy_from_slice(&1u16.to_le_bytes()); // version
        wasm_hdr[0x06] = role;
        wasm_hdr[0x07] = 0x00; // target: Wasm32
        wasm_hdr[0x0C..0x10].copy_from_slice(&(bytecode.len() as u32).to_le_bytes());
        wasm_hdr[0x38] = priority;
        seg.extend_from_slice(&wasm_hdr);

        // Bytecode
        seg.extend_from_slice(bytecode);

        seg
    }

    /// Build a minimal MANIFEST_SEG so the file is structurally valid.
    fn build_manifest_seg(seg_id: u64) -> Vec<u8> {
        let seg_header_size = rvf_types::constants::SEGMENT_HEADER_SIZE;
        let payload = [0u8; 4]; // minimal payload
        let mut seg = Vec::with_capacity(seg_header_size + payload.len());

        seg.extend_from_slice(&rvf_types::constants::SEGMENT_MAGIC.to_le_bytes());
        seg.push(1);
        seg.push(0x05); // Manifest
        seg.extend_from_slice(&[0, 0]);
        seg.extend_from_slice(&seg_id.to_le_bytes());
        seg.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        while seg.len() < seg_header_size {
            seg.push(0);
        }
        seg.extend_from_slice(&payload);
        seg
    }

    #[test]
    fn resolve_no_wasm_segments() {
        let buf = build_manifest_seg(1);
        let chain = resolve_bootstrap_chain(&buf);
        assert!(matches!(chain, BootstrapChain::None));
        assert!(!chain.is_self_bootstrapping());
    }

    #[test]
    fn resolve_microkernel_only() {
        let fake_bytecode = b"\x00asm\x01\x00\x00\x00_microkernel_";
        let mut buf = build_wasm_seg(WasmRole::Microkernel as u8, 1, fake_bytecode);
        buf.extend_from_slice(&build_manifest_seg(2));

        let chain = resolve_bootstrap_chain(&buf);
        assert!(matches!(chain, BootstrapChain::HostRequired { .. }));
        assert!(!chain.is_self_bootstrapping());

        if let BootstrapChain::HostRequired { microkernel } = &chain {
            assert_eq!(microkernel.role, WasmRole::Microkernel as u8);
            assert_eq!(microkernel.bytecode_len, fake_bytecode.len());
        }
    }

    #[test]
    fn resolve_combined_self_bootstrap() {
        let fake_bytecode = b"\x00asm\x01\x00\x00\x00_combined_interp_plus_kernel_";
        let mut buf = build_wasm_seg(WasmRole::Combined as u8, 0, fake_bytecode);
        buf.extend_from_slice(&build_manifest_seg(2));

        let chain = resolve_bootstrap_chain(&buf);
        assert!(chain.is_self_bootstrapping());
        assert!(matches!(chain, BootstrapChain::SelfContained { .. }));
    }

    #[test]
    fn resolve_two_stage_bootstrap() {
        let interp_bytecode = b"\x00asm\x01\x00\x00\x00_interpreter_runtime_";
        let kernel_bytecode = b"\x00asm\x01\x00\x00\x00_microkernel_";

        let mut buf = build_wasm_seg(WasmRole::Interpreter as u8, 0, interp_bytecode);
        // Adjust segment_id for second segment
        let mut kernel_seg = build_wasm_seg(WasmRole::Microkernel as u8, 1, kernel_bytecode);
        // Fix the segment_id to 2
        kernel_seg[8..16].copy_from_slice(&2u64.to_le_bytes());
        buf.extend_from_slice(&kernel_seg);
        buf.extend_from_slice(&build_manifest_seg(3));

        let chain = resolve_bootstrap_chain(&buf);
        assert!(chain.is_self_bootstrapping());
        assert!(matches!(chain, BootstrapChain::TwoStage { .. }));

        if let BootstrapChain::TwoStage { interpreter, microkernel } = &chain {
            assert_eq!(interpreter.role, WasmRole::Interpreter as u8);
            assert_eq!(microkernel.role, WasmRole::Microkernel as u8);
        }
    }

    #[test]
    fn get_bytecode_returns_correct_slice() {
        let fake_bytecode = b"\x00asm\x01\x00\x00\x00_test_module_";
        let buf = build_wasm_seg(WasmRole::Microkernel as u8, 0, fake_bytecode);

        let segments = parse_segments(&buf);
        assert!(!segments.is_empty());

        let module = parse_wasm_module(&buf, segments[0].offset).unwrap();
        let extracted = get_bytecode(&buf, &module).unwrap();
        assert_eq!(extracted, fake_bytecode);
    }

    #[test]
    fn bootstrap_priority_ordering() {
        // Create two modules with reversed priorities
        let high_priority = b"\x00asm\x01\x00\x00\x00_hi_";
        let low_priority = b"\x00asm\x01\x00\x00\x00_lo_";

        // Microkernel at priority 10, interpreter at priority 0
        let mut buf = build_wasm_seg(WasmRole::Microkernel as u8, 10, high_priority);
        let mut interp_seg = build_wasm_seg(WasmRole::Interpreter as u8, 0, low_priority);
        interp_seg[8..16].copy_from_slice(&2u64.to_le_bytes());
        buf.extend_from_slice(&interp_seg);
        buf.extend_from_slice(&build_manifest_seg(3));

        let chain = resolve_bootstrap_chain(&buf);
        assert!(chain.is_self_bootstrapping());

        // The interpreter should have lower priority (comes first)
        if let BootstrapChain::TwoStage { interpreter, microkernel } = &chain {
            assert_eq!(interpreter.bootstrap_priority, 0);
            assert_eq!(microkernel.bootstrap_priority, 10);
        }
    }
}
