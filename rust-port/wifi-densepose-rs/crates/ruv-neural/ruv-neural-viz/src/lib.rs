//! rUv Neural Viz — Brain topology visualization data structures and ASCII rendering.
//!
//! This crate provides:
//! - **Layout algorithms**: Force-directed, anatomical (MNI), and circular layouts
//! - **Color mapping**: Cool-warm, viridis, and module-color schemes
//! - **ASCII rendering**: Terminal-friendly graph, mincut, sparkline, and dashboard views
//! - **Export**: D3.js JSON, Graphviz DOT, GEXF, and CSV timeline formats
//! - **Animation**: Frame generation from temporal brain graph sequences

pub mod animation;
pub mod ascii;
pub mod colormap;
pub mod export;
pub mod layout;

pub use animation::{AnimatedEdge, AnimatedNode, AnimationFrame, AnimationFrames, LayoutType};
pub use colormap::ColorMap;
pub use layout::{AnatomicalLayout, ForceDirectedLayout};
