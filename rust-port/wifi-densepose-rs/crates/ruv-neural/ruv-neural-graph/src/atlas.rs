//! Brain atlas definitions with built-in parcellations.
//!
//! Provides the Desikan-Killiany 68-region atlas with anatomical metadata
//! including lobe classification, hemisphere, and MNI centroid coordinates.

use ruv_neural_core::brain::{Atlas, BrainRegion, Hemisphere, Lobe, Parcellation};

/// Supported atlas types for factory loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtlasType {
    /// Desikan-Killiany atlas with 68 cortical regions.
    DesikanKilliany,
}

/// Load a parcellation for the given atlas type.
pub fn load_atlas(atlas_type: AtlasType) -> Parcellation {
    match atlas_type {
        AtlasType::DesikanKilliany => build_desikan_killiany(),
    }
}

/// Region definition used during atlas construction.
struct RegionDef {
    name: &'static str,
    lobe: Lobe,
    /// MNI centroid for the left hemisphere version.
    mni_left: [f64; 3],
}

/// Build the full Desikan-Killiany 68-region parcellation.
///
/// 34 regions per hemisphere. For each region, the left hemisphere uses the
/// original MNI centroid and the right hemisphere mirrors the x-coordinate.
fn build_desikan_killiany() -> Parcellation {
    let region_defs = desikan_killiany_regions();
    let mut regions = Vec::with_capacity(68);
    let mut id = 0;

    // Left hemisphere (indices 0..34)
    for def in &region_defs {
        regions.push(BrainRegion {
            id,
            name: format!("lh_{}", def.name),
            hemisphere: Hemisphere::Left,
            lobe: def.lobe,
            centroid: def.mni_left,
        });
        id += 1;
    }

    // Right hemisphere (indices 34..68) — mirror x-coordinate
    for def in &region_defs {
        regions.push(BrainRegion {
            id,
            name: format!("rh_{}", def.name),
            hemisphere: Hemisphere::Right,
            lobe: def.lobe,
            centroid: [-def.mni_left[0], def.mni_left[1], def.mni_left[2]],
        });
        id += 1;
    }

    Parcellation {
        atlas: Atlas::DesikanKilliany68,
        regions,
    }
}

/// Returns the 34 unique region definitions for the Desikan-Killiany atlas.
///
/// MNI coordinates are approximate centroids from the FreeSurfer DK atlas.
fn desikan_killiany_regions() -> Vec<RegionDef> {
    vec![
        // Frontal lobe
        RegionDef {
            name: "superiorfrontal",
            lobe: Lobe::Frontal,
            mni_left: [-12.0, 30.0, 48.0],
        },
        RegionDef {
            name: "caudalmiddlefrontal",
            lobe: Lobe::Frontal,
            mni_left: [-37.0, 10.0, 48.0],
        },
        RegionDef {
            name: "rostralmiddlefrontal",
            lobe: Lobe::Frontal,
            mni_left: [-35.0, 38.0, 22.0],
        },
        RegionDef {
            name: "parsopercularis",
            lobe: Lobe::Frontal,
            mni_left: [-48.0, 14.0, 18.0],
        },
        RegionDef {
            name: "parstriangularis",
            lobe: Lobe::Frontal,
            mni_left: [-46.0, 28.0, 8.0],
        },
        RegionDef {
            name: "parsorbitalis",
            lobe: Lobe::Frontal,
            mni_left: [-42.0, 36.0, -10.0],
        },
        RegionDef {
            name: "lateralorbitofrontal",
            lobe: Lobe::Frontal,
            mni_left: [-28.0, 36.0, -14.0],
        },
        RegionDef {
            name: "medialorbitofrontal",
            lobe: Lobe::Frontal,
            mni_left: [-7.0, 44.0, -14.0],
        },
        RegionDef {
            name: "precentral",
            lobe: Lobe::Frontal,
            mni_left: [-38.0, -8.0, 52.0],
        },
        RegionDef {
            name: "paracentral",
            lobe: Lobe::Frontal,
            mni_left: [-8.0, -28.0, 62.0],
        },
        RegionDef {
            name: "frontalpole",
            lobe: Lobe::Frontal,
            mni_left: [-8.0, 64.0, -4.0],
        },
        // Parietal lobe
        RegionDef {
            name: "postcentral",
            lobe: Lobe::Parietal,
            mni_left: [-42.0, -28.0, 54.0],
        },
        RegionDef {
            name: "superiorparietal",
            lobe: Lobe::Parietal,
            mni_left: [-24.0, -56.0, 58.0],
        },
        RegionDef {
            name: "inferiorparietal",
            lobe: Lobe::Parietal,
            mni_left: [-44.0, -54.0, 38.0],
        },
        RegionDef {
            name: "supramarginal",
            lobe: Lobe::Parietal,
            mni_left: [-52.0, -34.0, 34.0],
        },
        RegionDef {
            name: "precuneus",
            lobe: Lobe::Parietal,
            mni_left: [-8.0, -58.0, 42.0],
        },
        // Temporal lobe
        RegionDef {
            name: "superiortemporal",
            lobe: Lobe::Temporal,
            mni_left: [-52.0, -12.0, -4.0],
        },
        RegionDef {
            name: "middletemporal",
            lobe: Lobe::Temporal,
            mni_left: [-56.0, -28.0, -8.0],
        },
        RegionDef {
            name: "inferiortemporal",
            lobe: Lobe::Temporal,
            mni_left: [-50.0, -36.0, -18.0],
        },
        RegionDef {
            name: "bankssts",
            lobe: Lobe::Temporal,
            mni_left: [-52.0, -42.0, 8.0],
        },
        RegionDef {
            name: "fusiform",
            lobe: Lobe::Temporal,
            mni_left: [-36.0, -42.0, -20.0],
        },
        RegionDef {
            name: "transversetemporal",
            lobe: Lobe::Temporal,
            mni_left: [-44.0, -22.0, 10.0],
        },
        RegionDef {
            name: "entorhinal",
            lobe: Lobe::Temporal,
            mni_left: [-24.0, -8.0, -34.0],
        },
        RegionDef {
            name: "temporalpole",
            lobe: Lobe::Temporal,
            mni_left: [-36.0, 12.0, -34.0],
        },
        RegionDef {
            name: "parahippocampal",
            lobe: Lobe::Temporal,
            mni_left: [-22.0, -28.0, -18.0],
        },
        // Occipital lobe
        RegionDef {
            name: "lateraloccipital",
            lobe: Lobe::Occipital,
            mni_left: [-34.0, -80.0, 8.0],
        },
        RegionDef {
            name: "lingual",
            lobe: Lobe::Occipital,
            mni_left: [-12.0, -72.0, -4.0],
        },
        RegionDef {
            name: "cuneus",
            lobe: Lobe::Occipital,
            mni_left: [-8.0, -82.0, 22.0],
        },
        RegionDef {
            name: "pericalcarine",
            lobe: Lobe::Occipital,
            mni_left: [-10.0, -82.0, 6.0],
        },
        // Limbic (cingulate + insula)
        RegionDef {
            name: "posteriorcingulate",
            lobe: Lobe::Limbic,
            mni_left: [-6.0, -30.0, 32.0],
        },
        RegionDef {
            name: "isthmuscingulate",
            lobe: Lobe::Limbic,
            mni_left: [-8.0, -44.0, 24.0],
        },
        RegionDef {
            name: "caudalanteriorcingulate",
            lobe: Lobe::Limbic,
            mni_left: [-6.0, 8.0, 34.0],
        },
        RegionDef {
            name: "rostralanteriorcingulate",
            lobe: Lobe::Limbic,
            mni_left: [-6.0, 30.0, 14.0],
        },
        RegionDef {
            name: "insula",
            lobe: Lobe::Limbic,
            mni_left: [-34.0, 4.0, 2.0],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Hemisphere;

    #[test]
    fn dk68_has_exactly_68_regions() {
        let parcellation = load_atlas(AtlasType::DesikanKilliany);
        assert_eq!(parcellation.num_regions(), 68);
    }

    #[test]
    fn dk68_has_34_per_hemisphere() {
        let parcellation = load_atlas(AtlasType::DesikanKilliany);
        let left = parcellation.regions_in_hemisphere(Hemisphere::Left);
        let right = parcellation.regions_in_hemisphere(Hemisphere::Right);
        assert_eq!(left.len(), 34);
        assert_eq!(right.len(), 34);
    }

    #[test]
    fn dk68_right_hemisphere_mirrors_x() {
        let parcellation = load_atlas(AtlasType::DesikanKilliany);
        // Region 0 (lh) and region 34 (rh) should have mirrored x.
        let lh = &parcellation.regions[0];
        let rh = &parcellation.regions[34];
        assert_eq!(lh.centroid[0], -rh.centroid[0]);
        assert_eq!(lh.centroid[1], rh.centroid[1]);
        assert_eq!(lh.centroid[2], rh.centroid[2]);
    }

    #[test]
    fn dk68_region_names_prefixed() {
        let parcellation = load_atlas(AtlasType::DesikanKilliany);
        assert!(parcellation.regions[0].name.starts_with("lh_"));
        assert!(parcellation.regions[34].name.starts_with("rh_"));
    }

    #[test]
    fn dk68_unique_ids() {
        let parcellation = load_atlas(AtlasType::DesikanKilliany);
        let ids: Vec<usize> = parcellation.regions.iter().map(|r| r.id).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 68);
    }
}
