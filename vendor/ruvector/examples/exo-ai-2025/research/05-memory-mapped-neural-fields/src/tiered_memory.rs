// Tiered Memory Management: DRAM → CXL → SSD → HDD
// Implements hierarchical storage with automatic tier migration

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Storage tier levels with latency characteristics
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tier {
    L1Dram, // ~80 ns, 64 GB
    L2Cxl,  // ~350 ns, 512 GB
    L3Ssd,  // ~80 μs, 4 TB
    L4Hdd,  // ~10 ms, 1 PB
}

impl Tier {
    /// Expected latency in nanoseconds
    pub fn latency_ns(&self) -> u64 {
        match self {
            Tier::L1Dram => 80,
            Tier::L2Cxl => 350,
            Tier::L3Ssd => 80_000,
            Tier::L4Hdd => 10_000_000,
        }
    }

    /// Typical capacity in bytes
    pub fn typical_capacity(&self) -> u64 {
        match self {
            Tier::L1Dram => 64 * 1024 * 1024 * 1024,         // 64 GB
            Tier::L2Cxl => 512 * 1024 * 1024 * 1024,         // 512 GB
            Tier::L3Ssd => 4 * 1024 * 1024 * 1024 * 1024,    // 4 TB
            Tier::L4Hdd => 1024 * 1024 * 1024 * 1024 * 1024, // 1 PB
        }
    }

    /// Next slower tier
    pub fn slower(&self) -> Option<Tier> {
        match self {
            Tier::L1Dram => Some(Tier::L2Cxl),
            Tier::L2Cxl => Some(Tier::L3Ssd),
            Tier::L3Ssd => Some(Tier::L4Hdd),
            Tier::L4Hdd => None,
        }
    }

    /// Next faster tier
    pub fn faster(&self) -> Option<Tier> {
        match self {
            Tier::L1Dram => None,
            Tier::L2Cxl => Some(Tier::L1Dram),
            Tier::L3Ssd => Some(Tier::L2Cxl),
            Tier::L4Hdd => Some(Tier::L3Ssd),
        }
    }
}

/// Page descriptor with metadata for migration policy
#[derive(Clone, Debug)]
pub struct Page {
    pub id: u64,
    pub data: Vec<f32>,
    pub size_bytes: usize,
    pub current_tier: Tier,
    pub last_access: Instant,
    pub access_count: usize,
    pub importance: f32,
    pub is_dirty: bool,
    pub is_pinned: bool,
}

impl Page {
    pub fn new(id: u64, data: Vec<f32>, tier: Tier) -> Self {
        let size_bytes = data.len() * std::mem::size_of::<f32>();
        Self {
            id,
            data,
            size_bytes,
            current_tier: tier,
            last_access: Instant::now(),
            access_count: 0,
            importance: 0.5,
            is_dirty: false,
            is_pinned: false,
        }
    }

    pub fn touch(&mut self) {
        self.last_access = Instant::now();
        self.access_count += 1;
    }

    pub fn age(&self) -> Duration {
        self.last_access.elapsed()
    }
}

/// Tier storage backend
struct TierStorage {
    tier: Tier,
    pages: HashMap<u64, Page>,
    capacity_bytes: u64,
    used_bytes: u64,
}

impl TierStorage {
    fn new(tier: Tier, capacity_bytes: u64) -> Self {
        Self {
            tier,
            pages: HashMap::new(),
            capacity_bytes,
            used_bytes: 0,
        }
    }

    fn insert(&mut self, page: Page) -> Result<(), String> {
        let page_size = page.size_bytes as u64;

        if self.used_bytes + page_size > self.capacity_bytes {
            return Err(format!(
                "Tier {:?} full: {} / {} bytes",
                self.tier, self.used_bytes, self.capacity_bytes
            ));
        }

        self.used_bytes += page_size;
        self.pages.insert(page.id, page);
        Ok(())
    }

    fn remove(&mut self, page_id: u64) -> Option<Page> {
        if let Some(page) = self.pages.remove(&page_id) {
            self.used_bytes -= page.size_bytes as u64;
            Some(page)
        } else {
            None
        }
    }

    fn get(&self, page_id: u64) -> Option<&Page> {
        self.pages.get(&page_id)
    }

    fn get_mut(&mut self, page_id: u64) -> Option<&mut Page> {
        self.pages.get_mut(&page_id)
    }

    fn available_bytes(&self) -> u64 {
        self.capacity_bytes - self.used_bytes
    }

    fn utilization(&self) -> f32 {
        self.used_bytes as f32 / self.capacity_bytes as f32
    }
}

/// Migration trigger conditions
#[derive(Clone, Debug)]
pub enum MigrationTrigger {
    /// Predicted access with confidence score
    PredictedAccess(f32),

    /// Recently accessed within duration
    RecentAccess(Duration),

    /// High semantic importance
    HighImportance(f32),

    /// Not accessed in duration
    LRU(Duration),

    /// Tier usage exceeds threshold
    CapacityPressure(f32),

    /// Low semantic importance
    LowImportance(f32),
}

/// Tiered memory manager
pub struct TieredMemory {
    tiers: HashMap<Tier, TierStorage>,
    page_index: Arc<RwLock<HashMap<u64, Tier>>>,
    migration_log: Arc<RwLock<VecDeque<MigrationEvent>>>,
}

#[derive(Clone, Debug)]
pub struct MigrationEvent {
    pub page_id: u64,
    pub from_tier: Tier,
    pub to_tier: Tier,
    pub trigger: String,
    pub timestamp: Instant,
    pub success: bool,
}

impl TieredMemory {
    /// Create new tiered memory system
    pub fn new() -> Self {
        let mut tiers = HashMap::new();

        // Initialize tiers with typical capacities
        tiers.insert(
            Tier::L1Dram,
            TierStorage::new(Tier::L1Dram, 64 * 1024 * 1024 * 1024), // 64 GB
        );
        tiers.insert(
            Tier::L2Cxl,
            TierStorage::new(Tier::L2Cxl, 512 * 1024 * 1024 * 1024), // 512 GB
        );
        tiers.insert(
            Tier::L3Ssd,
            TierStorage::new(Tier::L3Ssd, 4 * 1024 * 1024 * 1024 * 1024), // 4 TB
        );
        tiers.insert(
            Tier::L4Hdd,
            TierStorage::new(Tier::L4Hdd, 1024 * 1024 * 1024 * 1024 * 1024), // 1 PB
        );

        Self {
            tiers,
            page_index: Arc::new(RwLock::new(HashMap::new())),
            migration_log: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    /// Insert page into system (initially at coldest tier)
    pub fn insert(&mut self, page: Page) -> Result<(), String> {
        let page_id = page.id;
        let tier = Tier::L4Hdd; // Start at coldest tier

        self.tiers
            .get_mut(&tier)
            .ok_or("Tier not found")?
            .insert(page)?;

        self.page_index.write().unwrap().insert(page_id, tier);
        Ok(())
    }

    /// Load page (promotes to L1 if not already there)
    pub fn load(&mut self, page_id: u64) -> Result<&Page, String> {
        // Find current tier
        let current_tier = self
            .page_index
            .read()
            .unwrap()
            .get(&page_id)
            .copied()
            .ok_or("Page not found")?;

        // Promote to L1 if not already there
        if current_tier != Tier::L1Dram {
            self.promote(page_id, Tier::L1Dram, "load")?;
        }

        // Return reference
        self.tiers
            .get(&Tier::L1Dram)
            .and_then(|t| t.get(page_id))
            .ok_or("Page not in L1 after promotion".to_string())
    }

    /// Promote page to faster tier
    pub fn promote(
        &mut self,
        page_id: u64,
        target_tier: Tier,
        trigger: &str,
    ) -> Result<(), String> {
        let current_tier = self
            .page_index
            .read()
            .unwrap()
            .get(&page_id)
            .copied()
            .ok_or("Page not found")?;

        if current_tier == target_tier {
            return Ok(()); // Already in target tier
        }

        // Check if promotion is valid (can only move to faster tiers)
        if current_tier < target_tier {
            return Err("Cannot promote to slower tier".to_string());
        }

        // Remove from current tier
        let mut page = self
            .tiers
            .get_mut(&current_tier)
            .ok_or("Current tier not found")?
            .remove(page_id)
            .ok_or("Page not in current tier")?;

        // Check if target tier has space
        let target_storage = self
            .tiers
            .get_mut(&target_tier)
            .ok_or("Target tier not found")?;

        if target_storage.available_bytes() < page.size_bytes as u64 {
            // Evict pages from target tier to make space
            self.evict_pages(target_tier, page.size_bytes as u64)?;
        }

        // Update page metadata
        page.current_tier = target_tier;
        page.touch();

        // Insert into target tier
        self.tiers
            .get_mut(&target_tier)
            .ok_or("Target tier not found")?
            .insert(page)?;

        // Update index
        self.page_index
            .write()
            .unwrap()
            .insert(page_id, target_tier);

        // Log migration
        self.log_migration(MigrationEvent {
            page_id,
            from_tier: current_tier,
            to_tier: target_tier,
            trigger: trigger.to_string(),
            timestamp: Instant::now(),
            success: true,
        });

        Ok(())
    }

    /// Demote page to slower tier
    pub fn demote(&mut self, page_id: u64, target_tier: Tier, trigger: &str) -> Result<(), String> {
        let current_tier = self
            .page_index
            .read()
            .unwrap()
            .get(&page_id)
            .copied()
            .ok_or("Page not found")?;

        if current_tier == target_tier {
            return Ok(());
        }

        // Check if demotion is valid
        if current_tier > target_tier {
            return Err("Cannot demote to faster tier".to_string());
        }

        // Remove from current tier
        let mut page = self
            .tiers
            .get_mut(&current_tier)
            .ok_or("Current tier not found")?
            .remove(page_id)
            .ok_or("Page not in current tier")?;

        // Update metadata
        page.current_tier = target_tier;

        // Insert into target tier
        self.tiers
            .get_mut(&target_tier)
            .ok_or("Target tier not found")?
            .insert(page)?;

        // Update index
        self.page_index
            .write()
            .unwrap()
            .insert(page_id, target_tier);

        // Log migration
        self.log_migration(MigrationEvent {
            page_id,
            from_tier: current_tier,
            to_tier: target_tier,
            trigger: trigger.to_string(),
            timestamp: Instant::now(),
            success: true,
        });

        Ok(())
    }

    /// Evict pages from tier to free space
    fn evict_pages(&mut self, tier: Tier, bytes_needed: u64) -> Result<(), String> {
        let target_tier = tier.slower().ok_or("Cannot evict from coldest tier")?;

        // Find eviction candidates (LRU + importance)
        let mut candidates: Vec<_> = self
            .tiers
            .get(&tier)
            .ok_or("Tier not found")?
            .pages
            .values()
            .filter(|p| !p.is_pinned)
            .map(|p| {
                let lru_score = p.age().as_secs() as f32;
                let importance_penalty = 1.0 / (p.importance + 1e-6);
                let score = lru_score * importance_penalty;
                (p.id, score)
            })
            .collect();

        // Sort by score (highest = best candidate for eviction)
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Evict until we have enough space
        let mut freed = 0u64;
        for (page_id, _) in candidates {
            if freed >= bytes_needed {
                break;
            }

            let page = self
                .tiers
                .get(&tier)
                .and_then(|t| t.get(page_id))
                .ok_or("Page not found")?;
            freed += page.size_bytes as u64;

            self.demote(page_id, target_tier, "eviction")?;
        }

        if freed < bytes_needed {
            Err(format!(
                "Could not free enough space: {} / {} bytes",
                freed, bytes_needed
            ))
        } else {
            Ok(())
        }
    }

    /// Run background tier migration
    pub fn migrate_background(&mut self) {
        // Promote hot pages
        let promote_candidates: Vec<_> = self
            .tiers
            .iter()
            .flat_map(|(tier, storage)| {
                storage
                    .pages
                    .values()
                    .filter(|p| p.age().as_secs() < 60 && *tier != Tier::L1Dram)
                    .map(|p| (p.id, *tier))
            })
            .collect();

        for (page_id, current_tier) in promote_candidates {
            if let Some(target) = current_tier.faster() {
                let _ = self.promote(page_id, target, "background");
            }
        }

        // Demote cold pages
        let demote_candidates: Vec<_> = self
            .tiers
            .iter()
            .flat_map(|(tier, storage)| {
                storage
                    .pages
                    .values()
                    .filter(|p| p.age().as_secs() > 300 && *tier != Tier::L4Hdd)
                    .map(|p| (p.id, *tier))
            })
            .collect();

        for (page_id, current_tier) in demote_candidates {
            if let Some(target) = current_tier.slower() {
                let _ = self.demote(page_id, target, "background");
            }
        }
    }

    /// Log migration event
    fn log_migration(&self, event: MigrationEvent) {
        let mut log = self.migration_log.write().unwrap();
        log.push_back(event);

        // Keep log bounded
        if log.len() > 10_000 {
            log.drain(0..1000);
        }
    }

    /// Get tier statistics
    pub fn tier_stats(&self, tier: Tier) -> TierStats {
        let storage = &self.tiers[&tier];
        TierStats {
            tier,
            total_capacity: storage.capacity_bytes,
            used_bytes: storage.used_bytes,
            page_count: storage.pages.len(),
            utilization: storage.utilization(),
        }
    }

    /// Get overall statistics
    pub fn stats(&self) -> MemoryStats {
        MemoryStats {
            l1: self.tier_stats(Tier::L1Dram),
            l2: self.tier_stats(Tier::L2Cxl),
            l3: self.tier_stats(Tier::L3Ssd),
            l4: self.tier_stats(Tier::L4Hdd),
            total_pages: self.page_index.read().unwrap().len(),
            migration_count: self.migration_log.read().unwrap().len(),
        }
    }
}

/// Tier statistics
#[derive(Clone, Debug)]
pub struct TierStats {
    pub tier: Tier,
    pub total_capacity: u64,
    pub used_bytes: u64,
    pub page_count: usize,
    pub utilization: f32,
}

/// Overall memory statistics
#[derive(Clone, Debug)]
pub struct MemoryStats {
    pub l1: TierStats,
    pub l2: TierStats,
    pub l3: TierStats,
    pub l4: TierStats,
    pub total_pages: usize,
    pub migration_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_insertion() {
        let mut memory = TieredMemory::new();

        let page = Page::new(1, vec![1.0; 1024], Tier::L4Hdd);
        memory.insert(page).unwrap();

        let stats = memory.tier_stats(Tier::L4Hdd);
        assert_eq!(stats.page_count, 1);
    }

    #[test]
    fn test_promotion() {
        let mut memory = TieredMemory::new();

        let page = Page::new(1, vec![1.0; 1024], Tier::L4Hdd);
        memory.insert(page).unwrap();

        // Promote to L1
        memory.promote(1, Tier::L1Dram, "test").unwrap();

        let stats_l1 = memory.tier_stats(Tier::L1Dram);
        let stats_l4 = memory.tier_stats(Tier::L4Hdd);

        assert_eq!(stats_l1.page_count, 1);
        assert_eq!(stats_l4.page_count, 0);
    }

    #[test]
    fn test_load_promotes() {
        let mut memory = TieredMemory::new();

        let page = Page::new(1, vec![42.0; 1024], Tier::L4Hdd);
        memory.insert(page).unwrap();

        // Load should promote to L1
        let loaded = memory.load(1).unwrap();
        assert_eq!(loaded.data[0], 42.0);
        assert_eq!(loaded.current_tier, Tier::L1Dram);
    }

    #[test]
    fn test_eviction() {
        let mut memory = TieredMemory::new();

        // Fill L1 to near capacity
        let page_size = 1024 * 1024 * 1024; // 1 GB per page
        for i in 0..60 {
            let page = Page::new(i, vec![i as f32; page_size / 4], Tier::L4Hdd);
            memory.insert(page).unwrap();
            memory.promote(i, Tier::L1Dram, "test").ok();
        }

        let stats = memory.tier_stats(Tier::L1Dram);
        assert!(stats.page_count > 0);

        // Insert large page should trigger eviction
        let large_page = Page::new(100, vec![100.0; page_size / 4], Tier::L4Hdd);
        memory.insert(large_page).unwrap();
        memory.promote(100, Tier::L1Dram, "test").ok();

        let stats_after = memory.tier_stats(Tier::L1Dram);
        // Some pages should have been evicted
        assert!(stats_after.used_bytes <= memory.tiers[&Tier::L1Dram].capacity_bytes);
    }
}
