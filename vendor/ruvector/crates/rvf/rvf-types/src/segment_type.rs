//! Segment type discriminator for the RVF format.

/// Identifies the kind of data stored in a segment.
///
/// Values `0x00` and `0xF0..=0xFF` are reserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum SegmentType {
    /// Not a valid segment (uninitialized / zeroed region).
    Invalid = 0x00,
    /// Raw vector payloads (the actual embeddings).
    Vec = 0x01,
    /// HNSW adjacency lists, entry points, routing tables.
    Index = 0x02,
    /// Graph overlay deltas, partition updates, min-cut witnesses.
    Overlay = 0x03,
    /// Metadata mutations (label changes, deletions, moves).
    Journal = 0x04,
    /// Segment directory, hotset pointers, epoch state.
    Manifest = 0x05,
    /// Quantization dictionaries and codebooks.
    Quant = 0x06,
    /// Arbitrary key-value metadata (tags, provenance, lineage).
    Meta = 0x07,
    /// Temperature-promoted hot data (vectors + neighbors).
    Hot = 0x08,
    /// Access counter sketches for temperature decisions.
    Sketch = 0x09,
    /// Capability manifests, proof of computation, audit trails.
    Witness = 0x0A,
    /// Domain profile declarations (RVDNA, RVText, etc.).
    Profile = 0x0B,
    /// Key material, signature chains, certificate anchors.
    Crypto = 0x0C,
    /// Metadata inverted indexes for filtered search.
    MetaIdx = 0x0D,
    /// Embedded kernel / unikernel image for self-booting.
    Kernel = 0x0E,
    /// Embedded eBPF program for kernel fast path.
    Ebpf = 0x0F,
    /// Embedded WASM bytecode for self-bootstrapping execution.
    ///
    /// A WASM_SEG contains either a WASM microkernel (the RVF query engine
    /// compiled to wasm32) or a minimal WASM interpreter that can execute
    /// the microkernel. When both are present the file becomes fully
    /// self-bootstrapping: any host with raw execution capability can run
    /// the embedded interpreter, which in turn runs the microkernel, which
    /// processes the RVF data segments.
    Wasm = 0x10,
    /// Embedded web dashboard bundle (HTML/JS/CSS assets).
    ///
    /// A DASHBOARD_SEG contains a pre-built web application (e.g. Vite +
    /// Three.js) that can be served by the RVF HTTP server at `/`. The
    /// payload is a 64-byte `DashboardHeader` followed by a file table
    /// and concatenated file contents.
    Dashboard = 0x11,
    /// COW cluster mapping.
    CowMap = 0x20,
    /// Cluster reference counts.
    Refcount = 0x21,
    /// Vector membership filter.
    Membership = 0x22,
    /// Sparse delta patches.
    Delta = 0x23,
    /// Serialized transfer prior (cross-domain posterior summaries + cost EMAs).
    TransferPrior = 0x30,
    /// Policy kernel configuration and performance history.
    PolicyKernel = 0x31,
    /// Cost curve convergence data for acceleration tracking.
    CostCurve = 0x32,
    /// Federated learning export manifest: contributor pseudonym, export timestamp,
    /// included segment IDs, privacy budget spent, format version.
    FederatedManifest = 0x33,
    /// Differential privacy attestation: epsilon/delta values, noise mechanism,
    /// sensitivity bounds, clipping parameters.
    DiffPrivacyProof = 0x34,
    /// PII stripping attestation: redacted fields, rules fired,
    /// hash of pre-redaction content.
    RedactionLog = 0x35,
    /// Federated-averaged SONA weights: aggregated LoRA deltas,
    /// participation count, round number, convergence metrics.
    AggregateWeights = 0x36,
}

impl TryFrom<u8> for SegmentType {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Invalid),
            0x01 => Ok(Self::Vec),
            0x02 => Ok(Self::Index),
            0x03 => Ok(Self::Overlay),
            0x04 => Ok(Self::Journal),
            0x05 => Ok(Self::Manifest),
            0x06 => Ok(Self::Quant),
            0x07 => Ok(Self::Meta),
            0x08 => Ok(Self::Hot),
            0x09 => Ok(Self::Sketch),
            0x0A => Ok(Self::Witness),
            0x0B => Ok(Self::Profile),
            0x0C => Ok(Self::Crypto),
            0x0D => Ok(Self::MetaIdx),
            0x0E => Ok(Self::Kernel),
            0x0F => Ok(Self::Ebpf),
            0x10 => Ok(Self::Wasm),
            0x11 => Ok(Self::Dashboard),
            0x20 => Ok(Self::CowMap),
            0x21 => Ok(Self::Refcount),
            0x22 => Ok(Self::Membership),
            0x23 => Ok(Self::Delta),
            0x30 => Ok(Self::TransferPrior),
            0x31 => Ok(Self::PolicyKernel),
            0x32 => Ok(Self::CostCurve),
            0x33 => Ok(Self::FederatedManifest),
            0x34 => Ok(Self::DiffPrivacyProof),
            0x35 => Ok(Self::RedactionLog),
            0x36 => Ok(Self::AggregateWeights),
            other => Err(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all_variants() {
        let variants = [
            SegmentType::Invalid,
            SegmentType::Vec,
            SegmentType::Index,
            SegmentType::Overlay,
            SegmentType::Journal,
            SegmentType::Manifest,
            SegmentType::Quant,
            SegmentType::Meta,
            SegmentType::Hot,
            SegmentType::Sketch,
            SegmentType::Witness,
            SegmentType::Profile,
            SegmentType::Crypto,
            SegmentType::MetaIdx,
            SegmentType::Kernel,
            SegmentType::Ebpf,
            SegmentType::Wasm,
            SegmentType::Dashboard,
            SegmentType::CowMap,
            SegmentType::Refcount,
            SegmentType::Membership,
            SegmentType::Delta,
            SegmentType::TransferPrior,
            SegmentType::PolicyKernel,
            SegmentType::CostCurve,
            SegmentType::FederatedManifest,
            SegmentType::DiffPrivacyProof,
            SegmentType::RedactionLog,
            SegmentType::AggregateWeights,
        ];
        for v in variants {
            let raw = v as u8;
            assert_eq!(SegmentType::try_from(raw), Ok(v));
        }
    }

    #[test]
    fn invalid_value_returns_err() {
        assert_eq!(SegmentType::try_from(0x12), Err(0x12));
        assert_eq!(SegmentType::try_from(0x37), Err(0x37));
        assert_eq!(SegmentType::try_from(0xF0), Err(0xF0));
        assert_eq!(SegmentType::try_from(0xFF), Err(0xFF));
    }

    #[test]
    fn domain_expansion_discriminants() {
        assert_eq!(SegmentType::TransferPrior as u8, 0x30);
        assert_eq!(SegmentType::PolicyKernel as u8, 0x31);
        assert_eq!(SegmentType::CostCurve as u8, 0x32);
    }

    #[test]
    fn federation_discriminants() {
        assert_eq!(SegmentType::FederatedManifest as u8, 0x33);
        assert_eq!(SegmentType::DiffPrivacyProof as u8, 0x34);
        assert_eq!(SegmentType::RedactionLog as u8, 0x35);
        assert_eq!(SegmentType::AggregateWeights as u8, 0x36);
    }

    #[test]
    fn kernel_ebpf_wasm_discriminants() {
        assert_eq!(SegmentType::Kernel as u8, 0x0E);
        assert_eq!(SegmentType::Ebpf as u8, 0x0F);
        assert_eq!(SegmentType::Wasm as u8, 0x10);
    }
}
