//! ROS3 Real-Time Execution
//!
//! Dual runtime architecture combining Tokio (soft RT) and RTIC (hard RT)

pub mod executor;
pub mod scheduler;
pub mod latency;

pub use executor::{ROS3Executor, Priority, Deadline};
pub use scheduler::PriorityScheduler;
pub use latency::LatencyTracker;


/// Real-time task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RTPriority {
    /// Lowest priority (background tasks)
    Background = 0,
    /// Low priority
    Low = 1,
    /// Normal priority
    Normal = 2,
    /// High priority
    High = 3,
    /// Critical priority (hard real-time)
    Critical = 4,
}

impl From<u8> for RTPriority {
    fn from(value: u8) -> Self {
        match value {
            0 => RTPriority::Background,
            1 => RTPriority::Low,
            2 => RTPriority::Normal,
            3 => RTPriority::High,
            _ => RTPriority::Critical,
        }
    }
}

impl From<RTPriority> for u8 {
    fn from(priority: RTPriority) -> Self {
        priority as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_conversion() {
        let priority = RTPriority::High;
        let value: u8 = priority.into();
        assert_eq!(value, 3);

        let converted: RTPriority = value.into();
        assert_eq!(converted, RTPriority::High);
    }
}
