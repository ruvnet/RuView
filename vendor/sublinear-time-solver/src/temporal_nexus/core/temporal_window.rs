//! Temporal Window Management for maintaining consciousness continuity
//!
//! This module manages temporal windows with configurable overlap to ensure
//! consciousness continuity across time boundaries. The window overlap is critical
//! for maintaining temporal coherence and preventing consciousness fragmentation.

use std::collections::VecDeque;
use super::{TemporalResult, TemporalError};

/// Information about a temporal window
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub window_id: u64,
    pub start_tick: u64,
    pub end_tick: u64,
    pub overlap_size: u64,
    pub active: bool,
}

/// Metrics for window overlap management
#[derive(Debug, Clone, Default)]
pub struct WindowMetrics {
    pub total_windows: u64,
    pub average_overlap_percentage: f64,
    pub min_overlap_percentage: f64,
    pub max_overlap_percentage: f64,
    pub continuity_breaks: u64,
    pub optimal_overlap_ratio: f64,
}

/// Manages temporal windows with configurable overlap
pub struct TemporalWindow {
    pub info: WindowInfo,
    pub data: Vec<u8>,
    pub state_snapshot: Vec<f64>,
    pub creation_time: std::time::Instant,
}

impl TemporalWindow {
    /// Create a new temporal window
    pub fn new(window_id: u64, start_tick: u64, end_tick: u64, overlap_size: u64) -> Self {
        Self {
            info: WindowInfo {
                window_id,
                start_tick,
                end_tick,
                overlap_size,
                active: true,
            },
            data: Vec::new(),
            state_snapshot: Vec::new(),
            creation_time: std::time::Instant::now(),
        }
    }
    
    /// Get window size in ticks
    pub fn size(&self) -> u64 {
        self.info.end_tick - self.info.start_tick
    }
    
    /// Check if tick is within this window
    pub fn contains_tick(&self, tick: u64) -> bool {
        tick >= self.info.start_tick && tick <= self.info.end_tick
    }
    
    /// Calculate overlap with another window
    pub fn calculate_overlap(&self, other: &TemporalWindow) -> u64 {
        let overlap_start = self.info.start_tick.max(other.info.start_tick);
        let overlap_end = self.info.end_tick.min(other.info.end_tick);
        
        if overlap_start <= overlap_end {
            overlap_end - overlap_start + 1
        } else {
            0
        }
    }
    
    /// Store state data in the window
    pub fn store_state(&mut self, state: Vec<f64>) {
        self.state_snapshot = state;
    }
    
    /// Store binary data in the window
    pub fn store_data(&mut self, data: Vec<u8>) {
        self.data = data;
    }
    
    /// Deactivate the window
    pub fn deactivate(&mut self) {
        self.info.active = false;
    }
}

/// Manages window overlap to maintain consciousness continuity
pub struct WindowOverlapManager {
    windows: VecDeque<TemporalWindow>,
    next_window_id: u64,
    target_overlap_percent: f64,
    window_size_ticks: u64,
    max_windows: usize,
    metrics: WindowMetrics,
    current_tick: u64,
}

impl WindowOverlapManager {
    /// Create a new window overlap manager
    pub fn new(target_overlap_percent: f64) -> Self {
        Self {
            windows: VecDeque::new(),
            next_window_id: 1,
            target_overlap_percent: target_overlap_percent.clamp(50.0, 100.0),
            window_size_ticks: 1000, // Default 1000 ticks per window
            max_windows: 100,
            metrics: WindowMetrics::default(),
            current_tick: 0,
        }
    }
    
    /// Advance to the next tick and manage windows
    pub fn advance_window(&mut self, tick: u64) -> TemporalResult<()> {
        self.current_tick = tick;
        
        // Create new window if needed
        if self.should_create_new_window()? {
            self.create_new_window()?;
        }
        
        // Cleanup old windows
        self.cleanup_old_windows();
        
        // Update metrics
        self.update_metrics();
        
        // Validate overlap requirements
        self.validate_overlap_requirements()?;
        
        Ok(())
    }
    
    /// Get current window information
    pub fn get_current_window(&self) -> WindowInfo {
        self.windows.back()
            .map(|w| w.info.clone())
            .unwrap_or_else(|| WindowInfo {
                window_id: 0,
                start_tick: self.current_tick,
                end_tick: self.current_tick + self.window_size_ticks,
                overlap_size: 0,
                active: false,
            })
    }
    
    /// Get current overlap percentage
    pub fn get_current_overlap_percentage(&self) -> f64 {
        if self.windows.len() < 2 {
            return 100.0; // Perfect continuity with single window
        }
        
        let current = &self.windows[self.windows.len() - 1];
        let previous = &self.windows[self.windows.len() - 2];
        
        let overlap_size = current.calculate_overlap(previous);
        let window_size = current.size().min(previous.size());
        
        if window_size > 0 {
            (overlap_size as f64 / window_size as f64) * 100.0
        } else {
            0.0
        }
    }
    
    /// Adjust overlap for a specific window
    pub fn adjust_overlap(&mut self, window_id: u64, new_overlap_target: f64) -> TemporalResult<()> {
        for window in &mut self.windows {
            if window.info.window_id == window_id {
                let new_overlap_size = ((window.size() as f64) * (new_overlap_target / 100.0)) as u64;
                window.info.overlap_size = new_overlap_size;
                
                // Adjust window boundaries if needed
                self.recalculate_window_boundaries(window_id)?;
                return Ok(());
            }
        }
        
        Err(TemporalError::TscTimingError {
            message: format!("Window {} not found", window_id),
        })
    }
    
    /// Get window metrics
    pub fn get_metrics(&self) -> &WindowMetrics {
        &self.metrics
    }
    
    /// Store state in the current window
    pub fn store_current_state(&mut self, state: Vec<f64>) {
        if let Some(current_window) = self.windows.back_mut() {
            current_window.store_state(state);
        }
    }
    
    /// Store data in the current window
    pub fn store_current_data(&mut self, data: Vec<u8>) {
        if let Some(current_window) = self.windows.back_mut() {
            current_window.store_data(data);
        }
    }
    
    /// Get all active windows
    pub fn get_active_windows(&self) -> Vec<&TemporalWindow> {
        self.windows.iter().filter(|w| w.info.active).collect()
    }
    
    /// Find window containing a specific tick
    pub fn find_window_for_tick(&self, tick: u64) -> Option<&TemporalWindow> {
        self.windows.iter().find(|w| w.contains_tick(tick))
    }
    
    // Private helper methods
    
    fn should_create_new_window(&self) -> TemporalResult<bool> {
        if self.windows.is_empty() {
            return Ok(true);
        }
        
        let current_window = self.windows.back().unwrap();
        
        // Create new window when we're near the end of the current one
        let window_progress = (self.current_tick - current_window.info.start_tick) as f64 / current_window.size() as f64;
        let overlap_threshold = 1.0 - (self.target_overlap_percent / 100.0);
        
        Ok(window_progress >= overlap_threshold)
    }
    
    fn create_new_window(&mut self) -> TemporalResult<()> {
        let window_id = self.next_window_id;
        self.next_window_id += 1;
        
        let (start_tick, overlap_size) = if let Some(prev_window) = self.windows.back() {
            // Calculate overlap with previous window
            let overlap_ticks = ((self.window_size_ticks as f64) * (self.target_overlap_percent / 100.0)) as u64;
            let start_tick = prev_window.info.end_tick - overlap_ticks + 1;
            (start_tick, overlap_ticks)
        } else {
            // First window
            (self.current_tick, 0)
        };
        
        let end_tick = start_tick + self.window_size_ticks - 1;
        
        let new_window = TemporalWindow::new(window_id, start_tick, end_tick, overlap_size);
        self.windows.push_back(new_window);
        
        // Ensure we don't exceed max windows
        while self.windows.len() > self.max_windows {
            self.windows.pop_front();
        }
        
        self.metrics.total_windows += 1;
        
        Ok(())
    }
    
    fn cleanup_old_windows(&mut self) {
        // Deactivate windows that are too far in the past
        let cutoff_tick = self.current_tick.saturating_sub(self.window_size_ticks * 2);
        
        for window in &mut self.windows {
            if window.info.end_tick < cutoff_tick {
                window.deactivate();
            }
        }
        
        // Remove very old inactive windows
        let very_old_cutoff = self.current_tick.saturating_sub(self.window_size_ticks * 5);
        self.windows.retain(|w| w.info.end_tick >= very_old_cutoff);
    }
    
    fn update_metrics(&mut self) {
        if self.windows.len() < 2 {
            return;
        }
        
        let mut overlap_percentages = Vec::new();
        
        for i in 1..self.windows.len() {
            let current = &self.windows[i];
            let previous = &self.windows[i - 1];
            
            let overlap_size = current.calculate_overlap(previous);
            let window_size = current.size().min(previous.size());
            
            if window_size > 0 {
                let overlap_percent = (overlap_size as f64 / window_size as f64) * 100.0;
                overlap_percentages.push(overlap_percent);
            }
        }
        
        if !overlap_percentages.is_empty() {
            self.metrics.average_overlap_percentage = overlap_percentages.iter().sum::<f64>() / overlap_percentages.len() as f64;
            self.metrics.min_overlap_percentage = overlap_percentages.iter().cloned().fold(f64::INFINITY, f64::min);
            self.metrics.max_overlap_percentage = overlap_percentages.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            
            // Calculate optimal overlap ratio (how close we are to target)
            self.metrics.optimal_overlap_ratio = self.metrics.average_overlap_percentage / self.target_overlap_percent;
        }
    }
    
    fn validate_overlap_requirements(&mut self) -> TemporalResult<()> {
        let current_overlap = self.get_current_overlap_percentage();
        let min_required = self.target_overlap_percent * 0.8; // Allow 20% tolerance
        
        if current_overlap < min_required {
            self.metrics.continuity_breaks += 1;
            return Err(TemporalError::WindowOverlapTooLow {
                actual: current_overlap,
                required: min_required,
            });
        }
        
        Ok(())
    }
    
    fn recalculate_window_boundaries(&mut self, window_id: u64) -> TemporalResult<()> {
        // Find the window and recalculate its boundaries based on new overlap
        for i in 0..self.windows.len() {
            if self.windows[i].info.window_id == window_id {
                let overlap_size = self.windows[i].info.overlap_size;
                
                // Adjust start time if there's a previous window
                if i > 0 {
                    let prev_end = self.windows[i - 1].info.end_tick;
                    self.windows[i].info.start_tick = prev_end - overlap_size + 1;
                }
                
                // Adjust end time
                self.windows[i].info.end_tick = self.windows[i].info.start_tick + self.window_size_ticks - 1;
                
                // Adjust next window if exists
                if i + 1 < self.windows.len() {
                    let next_overlap = self.windows[i + 1].info.overlap_size;
                    self.windows[i + 1].info.start_tick = self.windows[i].info.end_tick - next_overlap + 1;
                }
                
                break;
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_temporal_window_creation() {
        let window = TemporalWindow::new(1, 0, 999, 100);
        assert_eq!(window.info.window_id, 1);
        assert_eq!(window.size(), 1000);
        assert!(window.contains_tick(500));
        assert!(!window.contains_tick(1500));
    }
    
    #[test]
    fn test_window_overlap_calculation() {
        let window1 = TemporalWindow::new(1, 0, 999, 100);
        let window2 = TemporalWindow::new(2, 900, 1899, 100);
        
        let overlap = window1.calculate_overlap(&window2);
        assert_eq!(overlap, 100); // Overlap from 900 to 999
    }
    
    #[test]
    fn test_window_overlap_manager() {
        let mut manager = WindowOverlapManager::new(75.0);
        
        // Advance through multiple ticks to create windows
        for tick in 0..2000 {
            manager.advance_window(tick).unwrap();
        }
        
        assert!(manager.windows.len() > 0);
        let overlap_percent = manager.get_current_overlap_percentage();
        assert!(overlap_percent >= 50.0); // Should maintain reasonable overlap
    }
    
    #[test]
    fn test_overlap_adjustment() {
        let mut manager = WindowOverlapManager::new(75.0);
        manager.advance_window(0).unwrap();
        manager.advance_window(500).unwrap();
        
        let window_id = manager.get_current_window().window_id;
        manager.adjust_overlap(window_id, 80.0).unwrap();
        
        // Verify adjustment was applied
        let window = manager.windows.iter().find(|w| w.info.window_id == window_id).unwrap();
        assert!(window.info.overlap_size > 0);
    }
    
    #[test]
    fn test_window_state_storage() {
        let mut manager = WindowOverlapManager::new(75.0);
        manager.advance_window(0).unwrap();
        
        let test_state = vec![1.0, 2.0, 3.0, 4.0];
        manager.store_current_state(test_state.clone());
        
        let current_window = manager.windows.back().unwrap();
        assert_eq!(current_window.state_snapshot, test_state);
    }
    
    #[test]
    fn test_window_cleanup() {
        let mut manager = WindowOverlapManager::new(75.0);
        
        // Create many windows
        for tick in 0..10000 {
            manager.advance_window(tick).unwrap();
        }
        
        // Should not exceed max windows
        assert!(manager.windows.len() <= manager.max_windows);
    }
}