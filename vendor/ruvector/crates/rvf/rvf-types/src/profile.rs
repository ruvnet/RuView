//! Hardware and domain profile identifiers.

/// Hardware profile ID (stored in root manifest `profile_id` for hardware tier).
///
/// Determines the runtime behaviour profile (memory budget, tier policy, etc.).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum ProfileId {
    /// Generic / minimal profile.
    Generic = 0,
    /// Core profile (moderate resources).
    Core = 1,
    /// Hot profile (high-performance, memory-rich).
    Hot = 2,
    /// Full profile (all features enabled).
    Full = 3,
}

impl TryFrom<u8> for ProfileId {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Generic),
            1 => Ok(Self::Core),
            2 => Ok(Self::Hot),
            3 => Ok(Self::Full),
            other => Err(other),
        }
    }
}

/// Domain profile discriminator (semantic overlay on the RVF substrate).
///
/// Stored in the root manifest `profile_id` field and declared in PROFILE_SEG.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum DomainProfile {
    /// Generic / unspecified domain.
    Generic = 0,
    /// Genomics (RVDNA) -- codon, k-mer, motif, structure embeddings.
    Rvdna = 1,
    /// Language / text (RVText) -- sentence, paragraph, document embeddings.
    RvText = 2,
    /// Graph / network (RVGraph) -- node, edge, subgraph embeddings.
    RvGraph = 3,
    /// Vision / imagery (RVVision) -- patch, image, object embeddings.
    RvVision = 4,
}

impl DomainProfile {
    /// The 4-byte magic number associated with each domain profile.
    pub const fn magic(self) -> u32 {
        match self {
            Self::Generic => 0x0000_0000,
            Self::Rvdna => 0x5244_4E41,    // "RDNA"
            Self::RvText => 0x5254_5854,   // "RTXT"
            Self::RvGraph => 0x5247_5248,  // "RGRH"
            Self::RvVision => 0x5256_4953, // "RVIS"
        }
    }
}

impl DomainProfile {
    /// The canonical file extension for this domain profile.
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Generic => "rvf",
            Self::Rvdna => "rvdna",
            Self::RvText => "rvtext",
            Self::RvGraph => "rvgraph",
            Self::RvVision => "rvvis",
        }
    }

    /// Look up a domain profile from a file extension (case-insensitive).
    pub fn from_extension(ext: &str) -> Option<Self> {
        // Manual case-insensitive comparison for no_std compatibility
        let ext_bytes = ext.as_bytes();
        if eq_ignore_ascii_case(ext_bytes, b"rvf") {
            Some(Self::Generic)
        } else if eq_ignore_ascii_case(ext_bytes, b"rvdna") {
            Some(Self::Rvdna)
        } else if eq_ignore_ascii_case(ext_bytes, b"rvtext") {
            Some(Self::RvText)
        } else if eq_ignore_ascii_case(ext_bytes, b"rvgraph") {
            Some(Self::RvGraph)
        } else if eq_ignore_ascii_case(ext_bytes, b"rvvis") {
            Some(Self::RvVision)
        } else {
            None
        }
    }
}

/// Case-insensitive ASCII byte comparison.
fn eq_ignore_ascii_case(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| x.eq_ignore_ascii_case(y))
}

impl TryFrom<u8> for DomainProfile {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Generic),
            1 => Ok(Self::Rvdna),
            2 => Ok(Self::RvText),
            3 => Ok(Self::RvGraph),
            4 => Ok(Self::RvVision),
            other => Err(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_id_round_trip() {
        for raw in 0..=3u8 {
            let p = ProfileId::try_from(raw).unwrap();
            assert_eq!(p as u8, raw);
        }
        assert_eq!(ProfileId::try_from(4), Err(4));
    }

    #[test]
    fn domain_profile_round_trip() {
        for raw in 0..=4u8 {
            let d = DomainProfile::try_from(raw).unwrap();
            assert_eq!(d as u8, raw);
        }
        assert_eq!(DomainProfile::try_from(5), Err(5));
    }

    #[test]
    fn domain_extension_round_trip() {
        let profiles = [
            DomainProfile::Generic,
            DomainProfile::Rvdna,
            DomainProfile::RvText,
            DomainProfile::RvGraph,
            DomainProfile::RvVision,
        ];
        for p in profiles {
            let ext = p.extension();
            let back = DomainProfile::from_extension(ext).unwrap();
            assert_eq!(back, p, "round-trip failed for {ext}");
        }
    }

    #[test]
    fn domain_extension_case_insensitive() {
        assert_eq!(
            DomainProfile::from_extension("RVDNA"),
            Some(DomainProfile::Rvdna)
        );
        assert_eq!(
            DomainProfile::from_extension("RvF"),
            Some(DomainProfile::Generic)
        );
        assert_eq!(
            DomainProfile::from_extension("RvText"),
            Some(DomainProfile::RvText)
        );
    }

    #[test]
    fn domain_extension_unknown() {
        assert_eq!(DomainProfile::from_extension("txt"), None);
        assert_eq!(DomainProfile::from_extension(""), None);
    }

    #[test]
    fn domain_magic_values() {
        assert_eq!(&DomainProfile::Rvdna.magic().to_be_bytes(), b"RDNA");
        assert_eq!(&DomainProfile::RvText.magic().to_be_bytes(), b"RTXT");
        assert_eq!(&DomainProfile::RvGraph.magic().to_be_bytes(), b"RGRH");
        assert_eq!(&DomainProfile::RvVision.magic().to_be_bytes(), b"RVIS");
    }
}
