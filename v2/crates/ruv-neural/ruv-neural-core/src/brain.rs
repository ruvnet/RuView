//! Brain region and atlas types for parcellation.

use serde::{Deserialize, Serialize};

/// Brain atlas defining a parcellation scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Atlas {
    /// Desikan-Killiany atlas (68 cortical regions).
    DesikanKilliany68,
    /// Destrieux atlas (148 cortical regions).
    Destrieux148,
    /// Schaefer 100-parcel atlas.
    Schaefer100,
    /// Schaefer 200-parcel atlas.
    Schaefer200,
    /// Schaefer 400-parcel atlas.
    Schaefer400,
    /// Custom atlas with a specified number of regions.
    Custom(usize),
}

impl Atlas {
    /// Number of regions in this atlas.
    pub fn num_regions(&self) -> usize {
        match self {
            Atlas::DesikanKilliany68 => 68,
            Atlas::Destrieux148 => 148,
            Atlas::Schaefer100 => 100,
            Atlas::Schaefer200 => 200,
            Atlas::Schaefer400 => 400,
            Atlas::Custom(n) => *n,
        }
    }
}

/// Cerebral hemisphere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Hemisphere {
    Left,
    Right,
    Midline,
}

/// Brain lobe classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Lobe {
    Frontal,
    Parietal,
    Temporal,
    Occipital,
    Limbic,
    Subcortical,
    Cerebellar,
}

/// A single brain region (parcel) within an atlas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainRegion {
    /// Region index within the atlas.
    pub id: usize,
    /// Human-readable name (e.g., "superiorfrontal").
    pub name: String,
    /// Hemisphere.
    pub hemisphere: Hemisphere,
    /// Lobe classification.
    pub lobe: Lobe,
    /// Centroid in MNI coordinates (x, y, z in mm).
    pub centroid: [f64; 3],
}

/// A full brain parcellation (atlas + all regions).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parcellation {
    /// Atlas used.
    pub atlas: Atlas,
    /// All regions in the parcellation.
    pub regions: Vec<BrainRegion>,
}

impl Parcellation {
    /// Number of regions.
    pub fn num_regions(&self) -> usize {
        self.regions.len()
    }

    /// Get a region by its id.
    pub fn get_region(&self, id: usize) -> Option<&BrainRegion> {
        self.regions.iter().find(|r| r.id == id)
    }

    /// Get all regions in a given hemisphere.
    pub fn regions_in_hemisphere(&self, hemisphere: Hemisphere) -> Vec<&BrainRegion> {
        self.regions
            .iter()
            .filter(|r| r.hemisphere == hemisphere)
            .collect()
    }

    /// Get all regions in a given lobe.
    pub fn regions_in_lobe(&self, lobe: Lobe) -> Vec<&BrainRegion> {
        self.regions.iter().filter(|r| r.lobe == lobe).collect()
    }
}
