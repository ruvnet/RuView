use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BottleneckReport {
    pub component: String,
    pub bottleneck_type: BottleneckType,
    pub severity: Severity,
    pub impact_score: f64, // 0.0 to 1.0
    pub description: String,
    pub recommendations: Vec<String>,
    pub metrics: BottleneckMetrics,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BottleneckType {
    CPUIntensive,
    MemoryPressure,
    IOBound,
    AlgorithmicComplexity,
    ConcurrencyLimitation,
    DataStructureInefficiency,
    CacheInefficiency,
    NetworkLatency,
    SerializationOverhead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BottleneckMetrics {
    pub execution_time: Duration,
    pub memory_usage: Option<usize>,
    pub cpu_utilization: Option<f64>,
    pub cache_hit_rate: Option<f64>,
    pub throughput: Option<f64>,
    pub concurrency_level: Option<u32>,
    pub gc_pressure: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ComponentProfile {
    pub name: String,
    pub execution_times: VecDeque<Duration>,
    pub memory_samples: VecDeque<usize>,
    pub throughput_samples: VecDeque<f64>,
    pub error_count: u64,
    pub last_analysis: Option<std::time::SystemTime>,
}

impl ComponentProfile {
    pub fn new(name: String) -> Self {
        Self {
            name,
            execution_times: VecDeque::with_capacity(1000),
            memory_samples: VecDeque::with_capacity(1000),
            throughput_samples: VecDeque::with_capacity(1000),
            error_count: 0,
            last_analysis: None,
        }
    }

    pub fn add_execution_time(&mut self, duration: Duration) {
        self.execution_times.push_back(duration);
        if self.execution_times.len() > 1000 {
            self.execution_times.pop_front();
        }
    }

    pub fn add_memory_sample(&mut self, memory: usize) {
        self.memory_samples.push_back(memory);
        if self.memory_samples.len() > 1000 {
            self.memory_samples.pop_front();
        }
    }

    pub fn add_throughput_sample(&mut self, throughput: f64) {
        self.throughput_samples.push_back(throughput);
        if self.throughput_samples.len() > 1000 {
            self.throughput_samples.pop_front();
        }
    }

    pub fn increment_errors(&mut self) {
        self.error_count += 1;
    }
}

pub struct BottleneckAnalyzer {
    profiles: HashMap<String, ComponentProfile>,
    analysis_history: Vec<BottleneckReport>,
    thresholds: AnalysisThresholds,
}

#[derive(Debug, Clone)]
pub struct AnalysisThresholds {
    pub cpu_intensive_threshold: Duration,
    pub memory_pressure_threshold: usize,
    pub throughput_degradation_threshold: f64,
    pub error_rate_threshold: f64,
    pub cache_miss_threshold: f64,
}

impl Default for AnalysisThresholds {
    fn default() -> Self {
        Self {
            cpu_intensive_threshold: Duration::from_millis(100),
            memory_pressure_threshold: 100 * 1024 * 1024, // 100MB
            throughput_degradation_threshold: 0.3, // 30% degradation
            error_rate_threshold: 0.05, // 5% error rate
            cache_miss_threshold: 0.7, // 70% cache miss rate
        }
    }
}

impl BottleneckAnalyzer {
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            analysis_history: Vec::new(),
            thresholds: AnalysisThresholds::default(),
        }
    }

    pub fn with_thresholds(thresholds: AnalysisThresholds) -> Self {
        Self {
            profiles: HashMap::new(),
            analysis_history: Vec::new(),
            thresholds,
        }
    }

    pub fn record_execution(&mut self, component: &str, duration: Duration, memory_used: Option<usize>) {
        let profile = self.profiles
            .entry(component.to_string())
            .or_insert_with(|| ComponentProfile::new(component.to_string()));

        profile.add_execution_time(duration);

        if let Some(memory) = memory_used {
            profile.add_memory_sample(memory);
        }
    }

    pub fn record_throughput(&mut self, component: &str, throughput: f64) {
        let profile = self.profiles
            .entry(component.to_string())
            .or_insert_with(|| ComponentProfile::new(component.to_string()));

        profile.add_throughput_sample(throughput);
    }

    pub fn record_error(&mut self, component: &str) {
        let profile = self.profiles
            .entry(component.to_string())
            .or_insert_with(|| ComponentProfile::new(component.to_string()));

        profile.increment_errors();
    }

    pub fn analyze_component(&mut self, component: &str) -> Vec<BottleneckReport> {
        let mut reports = Vec::new();

        if let Some(profile) = self.profiles.get_mut(component) {
            // Analyze execution time patterns
            reports.extend(self.analyze_execution_times(profile));

            // Analyze memory usage patterns
            reports.extend(self.analyze_memory_usage(profile));

            // Analyze throughput patterns
            reports.extend(self.analyze_throughput(profile));

            // Analyze error patterns
            reports.extend(self.analyze_error_patterns(profile));

            profile.last_analysis = Some(std::time::SystemTime::now());
        }

        // Store analysis results
        self.analysis_history.extend(reports.clone());

        // Keep history bounded
        if self.analysis_history.len() > 10000 {
            self.analysis_history.drain(..1000);
        }

        reports
    }

    fn analyze_execution_times(&self, profile: &ComponentProfile) -> Vec<BottleneckReport> {
        let mut reports = Vec::new();

        if profile.execution_times.len() < 10 {
            return reports;
        }

        let times: Vec<Duration> = profile.execution_times.iter().cloned().collect();

        // Calculate statistics
        let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
        let max_time = times.iter().max().copied().unwrap_or_default();
        let min_time = times.iter().min().copied().unwrap_or_default();

        // Check for CPU-intensive operations
        if avg_time > self.thresholds.cpu_intensive_threshold {
            let impact_score = (avg_time.as_millis() as f64 / self.thresholds.cpu_intensive_threshold.as_millis() as f64).min(1.0);

            reports.push(BottleneckReport {
                component: profile.name.clone(),
                bottleneck_type: BottleneckType::CPUIntensive,
                severity: self.calculate_severity(impact_score),
                impact_score,
                description: format!(
                    "Component {} has high average execution time: {:?} (threshold: {:?})",
                    profile.name, avg_time, self.thresholds.cpu_intensive_threshold
                ),
                recommendations: vec![
                    "Consider algorithmic optimizations".to_string(),
                    "Profile hotspots with flamegraph".to_string(),
                    "Evaluate parallel processing opportunities".to_string(),
                    "Consider caching frequently computed results".to_string(),
                ],
                metrics: BottleneckMetrics {
                    execution_time: avg_time,
                    memory_usage: None,
                    cpu_utilization: Some(impact_score * 100.0),
                    cache_hit_rate: None,
                    throughput: None,
                    concurrency_level: None,
                    gc_pressure: None,
                },
                timestamp: std::time::SystemTime::now(),
            });
        }

        // Check for high variance (inconsistent performance)
        let variance = self.calculate_duration_variance(&times, avg_time);
        if variance > avg_time.as_millis() as f64 * 0.5 {
            reports.push(BottleneckReport {
                component: profile.name.clone(),
                bottleneck_type: BottleneckType::CacheInefficiency,
                severity: Severity::Medium,
                impact_score: 0.6,
                description: format!(
                    "Component {} shows inconsistent performance with high variance: {:.2}ms",
                    profile.name, variance
                ),
                recommendations: vec![
                    "Investigate performance variance causes".to_string(),
                    "Check for memory allocation patterns".to_string(),
                    "Consider data structure optimization".to_string(),
                    "Analyze cache efficiency".to_string(),
                ],
                metrics: BottleneckMetrics {
                    execution_time: avg_time,
                    memory_usage: None,
                    cpu_utilization: None,
                    cache_hit_rate: None,
                    throughput: None,
                    concurrency_level: None,
                    gc_pressure: Some(variance / avg_time.as_millis() as f64),
                },
                timestamp: std::time::SystemTime::now(),
            });
        }

        reports
    }

    fn analyze_memory_usage(&self, profile: &ComponentProfile) -> Vec<BottleneckReport> {
        let mut reports = Vec::new();

        if profile.memory_samples.len() < 10 {
            return reports;
        }

        let memory_samples: Vec<usize> = profile.memory_samples.iter().cloned().collect();
        let avg_memory = memory_samples.iter().sum::<usize>() / memory_samples.len();
        let max_memory = memory_samples.iter().max().copied().unwrap_or(0);

        // Check for memory pressure
        if avg_memory > self.thresholds.memory_pressure_threshold {
            let impact_score = (avg_memory as f64 / self.thresholds.memory_pressure_threshold as f64).min(1.0);

            reports.push(BottleneckReport {
                component: profile.name.clone(),
                bottleneck_type: BottleneckType::MemoryPressure,
                severity: self.calculate_severity(impact_score),
                impact_score,
                description: format!(
                    "Component {} uses excessive memory: {}MB (threshold: {}MB)",
                    profile.name,
                    avg_memory / (1024 * 1024),
                    self.thresholds.memory_pressure_threshold / (1024 * 1024)
                ),
                recommendations: vec![
                    "Implement object pooling".to_string(),
                    "Reduce allocation frequency".to_string(),
                    "Consider streaming for large datasets".to_string(),
                    "Use more memory-efficient data structures".to_string(),
                ],
                metrics: BottleneckMetrics {
                    execution_time: Duration::default(),
                    memory_usage: Some(avg_memory),
                    cpu_utilization: None,
                    cache_hit_rate: None,
                    throughput: None,
                    concurrency_level: None,
                    gc_pressure: Some(max_memory as f64 / avg_memory as f64),
                },
                timestamp: std::time::SystemTime::now(),
            });
        }

        // Check for memory growth patterns
        if memory_samples.len() >= 50 {
            let first_half = &memory_samples[..memory_samples.len() / 2];
            let second_half = &memory_samples[memory_samples.len() / 2..];

            let first_avg = first_half.iter().sum::<usize>() / first_half.len();
            let second_avg = second_half.iter().sum::<usize>() / second_half.len();

            let growth_rate = (second_avg as f64 - first_avg as f64) / first_avg as f64;

            if growth_rate > 0.2 { // 20% growth
                reports.push(BottleneckReport {
                    component: profile.name.clone(),
                    bottleneck_type: BottleneckType::MemoryPressure,
                    severity: if growth_rate > 0.5 { Severity::High } else { Severity::Medium },
                    impact_score: growth_rate.min(1.0),
                    description: format!(
                        "Component {} shows memory growth trend: {:.1}% increase",
                        profile.name, growth_rate * 100.0
                    ),
                    recommendations: vec![
                        "Investigate potential memory leaks".to_string(),
                        "Implement proper cleanup in long-running operations".to_string(),
                        "Consider memory-bounded caches".to_string(),
                        "Profile memory allocation patterns".to_string(),
                    ],
                    metrics: BottleneckMetrics {
                        execution_time: Duration::default(),
                        memory_usage: Some(second_avg),
                        cpu_utilization: None,
                        cache_hit_rate: None,
                        throughput: None,
                        concurrency_level: None,
                        gc_pressure: Some(growth_rate),
                    },
                    timestamp: std::time::SystemTime::now(),
                });
            }
        }

        reports
    }

    fn analyze_throughput(&self, profile: &ComponentProfile) -> Vec<BottleneckReport> {
        let mut reports = Vec::new();

        if profile.throughput_samples.len() < 20 {
            return reports;
        }

        let samples: Vec<f64> = profile.throughput_samples.iter().cloned().collect();
        let recent_samples = &samples[samples.len().saturating_sub(10)..];
        let baseline_samples = &samples[..samples.len().saturating_sub(10)];

        if baseline_samples.is_empty() || recent_samples.is_empty() {
            return reports;
        }

        let recent_avg = recent_samples.iter().sum::<f64>() / recent_samples.len() as f64;
        let baseline_avg = baseline_samples.iter().sum::<f64>() / baseline_samples.len() as f64;

        let degradation = (baseline_avg - recent_avg) / baseline_avg;

        if degradation > self.thresholds.throughput_degradation_threshold {
            reports.push(BottleneckReport {
                component: profile.name.clone(),
                bottleneck_type: BottleneckType::AlgorithmicComplexity,
                severity: self.calculate_severity(degradation),
                impact_score: degradation.min(1.0),
                description: format!(
                    "Component {} shows throughput degradation: {:.1}% decrease",
                    profile.name, degradation * 100.0
                ),
                recommendations: vec![
                    "Analyze for O(n²) or worse complexity".to_string(),
                    "Consider algorithmic improvements".to_string(),
                    "Implement parallel processing".to_string(),
                    "Optimize data access patterns".to_string(),
                ],
                metrics: BottleneckMetrics {
                    execution_time: Duration::default(),
                    memory_usage: None,
                    cpu_utilization: None,
                    cache_hit_rate: None,
                    throughput: Some(recent_avg),
                    concurrency_level: None,
                    gc_pressure: Some(degradation),
                },
                timestamp: std::time::SystemTime::now(),
            });
        }

        reports
    }

    fn analyze_error_patterns(&self, profile: &ComponentProfile) -> Vec<BottleneckReport> {
        let mut reports = Vec::new();

        let total_operations = profile.execution_times.len() as u64;
        if total_operations == 0 {
            return reports;
        }

        let error_rate = profile.error_count as f64 / total_operations as f64;

        if error_rate > self.thresholds.error_rate_threshold {
            reports.push(BottleneckReport {
                component: profile.name.clone(),
                bottleneck_type: BottleneckType::ConcurrencyLimitation,
                severity: self.calculate_severity(error_rate * 5.0), // Scale up for severity
                impact_score: (error_rate * 5.0).min(1.0),
                description: format!(
                    "Component {} has high error rate: {:.1}% ({} errors out of {} operations)",
                    profile.name, error_rate * 100.0, profile.error_count, total_operations
                ),
                recommendations: vec![
                    "Investigate error causes".to_string(),
                    "Implement better error handling".to_string(),
                    "Add retry mechanisms with exponential backoff".to_string(),
                    "Consider circuit breaker pattern".to_string(),
                ],
                metrics: BottleneckMetrics {
                    execution_time: Duration::default(),
                    memory_usage: None,
                    cpu_utilization: None,
                    cache_hit_rate: None,
                    throughput: None,
                    concurrency_level: None,
                    gc_pressure: Some(error_rate),
                },
                timestamp: std::time::SystemTime::now(),
            });
        }

        reports
    }

    fn calculate_duration_variance(&self, durations: &[Duration], mean: Duration) -> f64 {
        if durations.len() < 2 {
            return 0.0;
        }

        let mean_ms = mean.as_millis() as f64;
        let variance = durations.iter()
            .map(|d| {
                let diff = d.as_millis() as f64 - mean_ms;
                diff * diff
            })
            .sum::<f64>() / (durations.len() - 1) as f64;

        variance.sqrt()
    }

    fn calculate_severity(&self, impact_score: f64) -> Severity {
        match impact_score {
            x if x >= 0.8 => Severity::Critical,
            x if x >= 0.6 => Severity::High,
            x if x >= 0.3 => Severity::Medium,
            _ => Severity::Low,
        }
    }

    pub fn analyze_all_components(&mut self) -> Vec<BottleneckReport> {
        let components: Vec<String> = self.profiles.keys().cloned().collect();
        let mut all_reports = Vec::new();

        for component in components {
            all_reports.extend(self.analyze_component(&component));
        }

        all_reports
    }

    pub fn get_top_bottlenecks(&self, limit: usize) -> Vec<&BottleneckReport> {
        let mut reports: Vec<&BottleneckReport> = self.analysis_history.iter().collect();

        reports.sort_by(|a, b| {
            b.impact_score.partial_cmp(&a.impact_score).unwrap_or(std::cmp::Ordering::Equal)
        });

        reports.into_iter().take(limit).collect()
    }

    pub fn generate_optimization_plan(&self) -> String {
        let top_bottlenecks = self.get_top_bottlenecks(10);

        let mut plan = String::from("# Performance Optimization Plan\n\n");

        plan.push_str("## Critical Bottlenecks (Immediate Action Required)\n\n");

        let critical: Vec<_> = top_bottlenecks.iter()
            .filter(|r| matches!(r.severity, Severity::Critical))
            .collect();

        for (i, report) in critical.iter().enumerate() {
            plan.push_str(&format!("### {}. {} - {}\n\n", i + 1, report.component, report.description));
            plan.push_str("**Recommendations:**\n");
            for rec in &report.recommendations {
                plan.push_str(&format!("- {}\n", rec));
            }
            plan.push_str(&format!("**Impact Score:** {:.2}\n\n", report.impact_score));
        }

        plan.push_str("## High Priority Optimizations\n\n");

        let high_priority: Vec<_> = top_bottlenecks.iter()
            .filter(|r| matches!(r.severity, Severity::High))
            .collect();

        for (i, report) in high_priority.iter().enumerate() {
            plan.push_str(&format!("### {}. {} - {}\n\n", i + 1, report.component, report.description));
            plan.push_str("**Primary Recommendation:** ");
            if let Some(rec) = report.recommendations.first() {
                plan.push_str(&format!("{}\n\n", rec));
            }
        }

        plan.push_str("## Implementation Priority\n\n");
        plan.push_str("1. **Immediate (Critical):** Address critical bottlenecks that severely impact performance\n");
        plan.push_str("2. **Short-term (High):** Implement high-impact optimizations within 1-2 weeks\n");
        plan.push_str("3. **Medium-term (Medium):** Architectural improvements over 1-2 months\n");
        plan.push_str("4. **Long-term (Low):** Nice-to-have optimizations for future releases\n\n");

        plan
    }

    pub fn export_analysis(&self, format: &str) -> Result<String, Box<dyn std::error::Error>> {
        match format {
            "json" => Ok(serde_json::to_string_pretty(&self.analysis_history)?),
            "csv" => {
                let mut csv = String::from("component,bottleneck_type,severity,impact_score,description,timestamp\n");
                for report in &self.analysis_history {
                    csv.push_str(&format!(
                        "{},{:?},{:?},{:.3},{},{:?}\n",
                        report.component,
                        report.bottleneck_type,
                        report.severity,
                        report.impact_score,
                        report.description.replace(',', ';'),
                        report.timestamp
                    ));
                }
                Ok(csv)
            }
            _ => Err("Unsupported format".into()),
        }
    }
}

impl Default for BottleneckAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_bottleneck_detection() {
        let mut analyzer = BottleneckAnalyzer::new();

        // Record slow operations
        for _ in 0..20 {
            analyzer.record_execution("slow_component", Duration::from_millis(200), Some(50 * 1024 * 1024));
        }

        let reports = analyzer.analyze_component("slow_component");
        assert!(!reports.is_empty());

        let cpu_bottleneck = reports.iter()
            .find(|r| matches!(r.bottleneck_type, BottleneckType::CPUIntensive));
        assert!(cpu_bottleneck.is_some());
    }

    #[test]
    fn test_memory_growth_detection() {
        let mut analyzer = BottleneckAnalyzer::new();

        // Simulate memory growth
        for i in 0..60 {
            let memory = 10 * 1024 * 1024 + (i * 1024 * 1024); // Growing memory usage
            analyzer.record_execution("growing_component", Duration::from_millis(10), Some(memory));
        }

        let reports = analyzer.analyze_component("growing_component");
        let memory_growth = reports.iter()
            .find(|r| r.description.contains("growth trend"));
        assert!(memory_growth.is_some());
    }

    #[test]
    fn test_throughput_degradation() {
        let mut analyzer = BottleneckAnalyzer::new();

        // Baseline throughput
        for _ in 0..30 {
            analyzer.record_throughput("degrading_component", 1000.0);
        }

        // Degraded throughput
        for _ in 0..15 {
            analyzer.record_throughput("degrading_component", 600.0);
        }

        let reports = analyzer.analyze_component("degrading_component");
        let throughput_issue = reports.iter()
            .find(|r| r.description.contains("throughput degradation"));
        assert!(throughput_issue.is_some());
    }
}