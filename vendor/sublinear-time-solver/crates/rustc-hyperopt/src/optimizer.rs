//! Core cold start optimization engine

use crate::{
    cache::CacheManager,
    error::Result,
    pattern_db::EcosystemPatternDatabase,
    performance::{OptimizationResult, PerformanceTracker, PerformanceMetrics},
    signature::ProjectSignatureAnalyzer,
};
use std::{sync::Arc, time::Instant};

/// Main cold start optimizer with AI-powered strategies
pub struct ColdStartOptimizer {
    signature_analyzer: Arc<ProjectSignatureAnalyzer>,
    ecosystem_db: Arc<EcosystemPatternDatabase>,
    cache_manager: Arc<CacheManager>,
    performance_tracker: Arc<PerformanceTracker>,
}

impl ColdStartOptimizer {
    /// Create a new cold start optimizer
    pub async fn new() -> Result<Self> {
        let signature_analyzer = Arc::new(ProjectSignatureAnalyzer::new()?);
        let ecosystem_db = Arc::new(EcosystemPatternDatabase::new().await?);
        let cache_manager = Arc::new(CacheManager::new()?);
        let performance_tracker = Arc::new(PerformanceTracker::new());

        Ok(Self {
            signature_analyzer,
            ecosystem_db,
            cache_manager,
            performance_tracker,
        })
    }

    /// Create with custom configuration
    pub async fn with_config(config: OptimizerConfig) -> Result<Self> {
        let signature_analyzer = Arc::new(ProjectSignatureAnalyzer::with_config(config.signature)?);
        let ecosystem_db = Arc::new(EcosystemPatternDatabase::with_config(config.pattern_db).await?);
        let cache_manager = Arc::new(CacheManager::with_config(config.cache)?);
        let performance_tracker = Arc::new(PerformanceTracker::new());

        Ok(Self {
            signature_analyzer,
            ecosystem_db,
            cache_manager,
            performance_tracker,
        })
    }

    /// Optimize compilation with AI-powered strategies
    pub async fn optimize_compilation(&self) -> Result<OptimizationResult> {
        let start_time = Instant::now();

        // Phase 1: Project signature analysis
        let signature = self.signature_analyzer.analyze_project().await?;

        // Phase 2: Ecosystem pattern matching
        let patterns = self.ecosystem_db.find_matching_patterns(&signature).await?;

        // Phase 3: Cache pre-seeding
        self.cache_manager.pre_seed_with_patterns(&patterns).await?;

        // Phase 4: Intelligent cache warming
        let warm_result = self.cache_manager.intelligent_warm().await?;

        // Phase 5: Performance tracking
        let optimization_time = start_time.elapsed();
        let result = self.performance_tracker.record_optimization(
            signature,
            patterns,
            warm_result,
            optimization_time,
        ).await?;

        Ok(result)
    }

    /// Get current performance metrics
    pub async fn get_performance_metrics(&self) -> Result<PerformanceMetrics> {
        self.performance_tracker.get_metrics().await
    }

    /// Clear all caches
    pub async fn clear_caches(&self) -> Result<()> {
        self.cache_manager.clear_all().await
    }
}

/// Configuration for the cold start optimizer
#[derive(Debug, Clone)]
pub struct OptimizerConfig {
    /// Signature analyzer configuration
    pub signature: SignatureConfig,
    /// Pattern database configuration
    pub pattern_db: PatternDbConfig,
    /// Cache manager configuration
    pub cache: CacheConfig,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            signature: SignatureConfig::default(),
            pattern_db: PatternDbConfig::default(),
            cache: CacheConfig::default(),
        }
    }
}

/// Configuration for signature analysis
#[derive(Debug, Clone)]
pub struct SignatureConfig {
    /// Enable dependency analysis
    pub analyze_dependencies: bool,
    /// Enable feature detection
    pub detect_features: bool,
    /// Maximum analysis depth
    pub max_depth: usize,
}

impl Default for SignatureConfig {
    fn default() -> Self {
        Self {
            analyze_dependencies: true,
            detect_features: true,
            max_depth: 10,
        }
    }
}

/// Configuration for pattern database
#[derive(Debug, Clone)]
pub struct PatternDbConfig {
    /// Enable online pattern updates
    pub online_updates: bool,
    /// Maximum patterns to cache
    pub max_patterns: usize,
    /// Pattern confidence threshold
    pub confidence_threshold: f64,
}

impl Default for PatternDbConfig {
    fn default() -> Self {
        Self {
            online_updates: true,
            max_patterns: 10000,
            confidence_threshold: 0.75,
        }
    }
}

/// Configuration for cache management
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Hot cache size in MB
    pub hot_cache_size_mb: usize,
    /// Warm cache size in MB
    pub warm_cache_size_mb: usize,
    /// Cold cache size in MB
    pub cold_cache_size_mb: usize,
    /// Enable intelligent eviction
    pub intelligent_eviction: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            hot_cache_size_mb: 256,
            warm_cache_size_mb: 1024,
            cold_cache_size_mb: 4096,
            intelligent_eviction: true,
        }
    }
}

