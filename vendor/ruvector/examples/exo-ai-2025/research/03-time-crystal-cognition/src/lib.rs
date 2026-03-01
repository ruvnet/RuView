// Time Crystal Cognition Library
// Cognitive Time Crystals: Discrete Time Translation Symmetry Breaking in Working Memory

pub mod discrete_time_crystal;
pub mod floquet_cognition;
pub mod simd_optimizations;
pub mod temporal_memory;

// Re-export main types
pub use discrete_time_crystal::{DTCConfig, DiscreteTimeCrystal};
pub use floquet_cognition::{
    FloquetCognitiveSystem, FloquetConfig, FloquetTrajectory, PhaseDiagram,
};
pub use simd_optimizations::{
    HierarchicalTimeCrystal, SimdDTC, SimdFloquet, TopologicalTimeCrystal,
};
pub use temporal_memory::{
    MemoryItem, MemoryStats, TemporalMemory, TemporalMemoryConfig, WorkingMemoryTask,
};
