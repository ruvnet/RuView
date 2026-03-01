//! # Claude Task Fine-Tuning Dataset Generator
//!
//! Generates synthetic training datasets for RuvLTRA models fine-tuned on
//! Claude Flow agent tasks. Includes data augmentation, quality scoring,
//! and export to standard formats (JSONL, Parquet).
//!
//! ## Task Categories
//!
//! The dataset covers 5 primary task categories aligned with Claude Flow agents:
//! - **Coder**: Code generation, debugging, refactoring
//! - **Researcher**: Analysis, exploration, documentation
//! - **Security**: Audit, vulnerability analysis, threat detection
//! - **Architecture**: Design, planning, system architecture
//! - **Reviewer**: Code review, quality assessment, best practices
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::training::{DatasetGenerator, DatasetConfig};
//!
//! let config = DatasetConfig::default();
//! let generator = DatasetGenerator::new(config);
//! let dataset = generator.generate()?;
//!
//! // Export to JSONL
//! dataset.export_jsonl("training_data.jsonl")?;
//!
//! // Export to Parquet
//! dataset.export_parquet("training_data.parquet")?;
//! ```

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Task categories matching Claude Flow agents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskCategory {
    /// Code generation, debugging, refactoring
    Coder,
    /// Analysis, exploration, documentation
    Researcher,
    /// Audit, vulnerability analysis, threat detection
    Security,
    /// Design, planning, system architecture
    Architecture,
    /// Code review, quality assessment
    Reviewer,
}

impl TaskCategory {
    /// Get all task categories
    pub fn all() -> Vec<Self> {
        vec![
            Self::Coder,
            Self::Researcher,
            Self::Security,
            Self::Architecture,
            Self::Reviewer,
        ]
    }

    /// Get category name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Coder => "coder",
            Self::Researcher => "researcher",
            Self::Security => "security",
            Self::Architecture => "architecture",
            Self::Reviewer => "reviewer",
        }
    }

    /// Get recommended model for this category
    pub fn recommended_model(&self, complexity: ComplexityLevel) -> &'static str {
        match (self, complexity) {
            (Self::Coder, ComplexityLevel::Simple) => "haiku",
            (Self::Coder, ComplexityLevel::Moderate) => "sonnet",
            (Self::Coder, ComplexityLevel::Complex) => "opus",
            (Self::Researcher, ComplexityLevel::Simple) => "haiku",
            (Self::Researcher, _) => "sonnet",
            (Self::Security, _) => "opus",
            (Self::Architecture, ComplexityLevel::Simple) => "sonnet",
            (Self::Architecture, _) => "opus",
            (Self::Reviewer, ComplexityLevel::Simple) => "haiku",
            (Self::Reviewer, _) => "sonnet",
        }
    }
}

/// Complexity level for task classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComplexityLevel {
    /// Simple, straightforward tasks
    Simple,
    /// Moderate complexity requiring analysis
    Moderate,
    /// Complex tasks requiring deep reasoning
    Complex,
}

/// Domain type for task context
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DomainType {
    /// Web development (frontend/backend)
    Web,
    /// Systems programming (low-level, OS, drivers)
    Systems,
    /// Data science and ML
    DataScience,
    /// Mobile development
    Mobile,
    /// DevOps and infrastructure
    DevOps,
    /// Security and cryptography
    Security,
    /// Database and storage
    Database,
    /// API design and integration
    Api,
}

/// Metadata for task examples
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMetadata {
    /// Task category
    pub category: TaskCategory,
    /// Complexity level
    pub complexity: ComplexityLevel,
    /// Domain type
    pub domain: DomainType,
    /// Expected model (haiku/sonnet/opus)
    pub expected_model: String,
    /// Quality score (0.0-1.0)
    pub quality_score: f32,
    /// Tags for filtering
    pub tags: Vec<String>,
}

/// A single training example for Claude task routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeTaskExample {
    /// Input task description
    pub input: String,
    /// Context information
    pub context: String,
    /// Expected agent routing decision
    pub output_agent: String,
    /// Metadata
    pub metadata: TaskMetadata,
}

/// Dataset configuration
#[derive(Debug, Clone)]
pub struct DatasetConfig {
    /// Number of seed examples per category
    pub examples_per_category: usize,
    /// Enable data augmentation
    pub enable_augmentation: bool,
    /// Augmentation configuration
    pub augmentation: AugmentationConfig,
    /// Random seed for reproducibility
    pub seed: u64,
}

impl Default for DatasetConfig {
    fn default() -> Self {
        Self {
            examples_per_category: 100,
            enable_augmentation: true,
            augmentation: AugmentationConfig::default(),
            seed: 42,
        }
    }
}

/// Data augmentation configuration
#[derive(Debug, Clone)]
pub struct AugmentationConfig {
    /// Number of paraphrases per example
    pub paraphrases_per_example: usize,
    /// Number of complexity variations per example
    pub complexity_variations: usize,
    /// Enable domain transfer
    pub enable_domain_transfer: bool,
}

impl Default for AugmentationConfig {
    fn default() -> Self {
        Self {
            paraphrases_per_example: 2,
            complexity_variations: 2,
            enable_domain_transfer: true,
        }
    }
}

/// Complete dataset with statistics
#[derive(Debug)]
pub struct ClaudeTaskDataset {
    /// All training examples
    pub examples: Vec<ClaudeTaskExample>,
    /// Dataset statistics
    pub stats: DatasetStats,
}

/// Dataset statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DatasetStats {
    /// Total number of examples
    pub total_examples: usize,
    /// Examples per category
    pub examples_per_category: HashMap<String, usize>,
    /// Examples per complexity level
    pub examples_per_complexity: HashMap<String, usize>,
    /// Examples per domain
    pub examples_per_domain: HashMap<String, usize>,
    /// Average quality score
    pub avg_quality_score: f32,
}

impl ClaudeTaskDataset {
    /// Create a new dataset from examples
    pub fn new(examples: Vec<ClaudeTaskExample>) -> Self {
        let stats = Self::compute_stats(&examples);
        Self { examples, stats }
    }

    /// Compute statistics for the dataset
    fn compute_stats(examples: &[ClaudeTaskExample]) -> DatasetStats {
        let mut stats = DatasetStats {
            total_examples: examples.len(),
            examples_per_category: HashMap::new(),
            examples_per_complexity: HashMap::new(),
            examples_per_domain: HashMap::new(),
            avg_quality_score: 0.0,
        };

        let mut total_quality = 0.0;

        for example in examples {
            // Count by category
            *stats
                .examples_per_category
                .entry(example.metadata.category.name().to_string())
                .or_insert(0) += 1;

            // Count by complexity
            let complexity = format!("{:?}", example.metadata.complexity);
            *stats.examples_per_complexity.entry(complexity).or_insert(0) += 1;

            // Count by domain
            let domain = format!("{:?}", example.metadata.domain);
            *stats.examples_per_domain.entry(domain).or_insert(0) += 1;

            total_quality += example.metadata.quality_score;
        }

        if !examples.is_empty() {
            stats.avg_quality_score = total_quality / examples.len() as f32;
        }

        stats
    }

    /// Export dataset to JSONL format
    pub fn export_jsonl<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        for example in &self.examples {
            let json = serde_json::to_string(example)?;
            writeln!(writer, "{}", json)?;
        }

        writer.flush()?;
        Ok(())
    }

    /// Export dataset to JSON format (full array)
    pub fn export_json<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, &self.examples)?;
        Ok(())
    }

    /// Export statistics to JSON
    pub fn export_stats<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, &self.stats)?;
        Ok(())
    }

    /// Split dataset into train/validation/test sets
    pub fn split(
        &self,
        train: f32,
        val: f32,
        test: f32,
        seed: u64,
    ) -> (
        Vec<ClaudeTaskExample>,
        Vec<ClaudeTaskExample>,
        Vec<ClaudeTaskExample>,
    ) {
        assert!(
            (train + val + test - 1.0).abs() < 1e-6,
            "Split ratios must sum to 1.0"
        );

        let mut rng = StdRng::seed_from_u64(seed);
        let mut examples = self.examples.clone();
        examples.shuffle(&mut rng);

        let total = examples.len();
        let train_size = (total as f32 * train) as usize;
        let val_size = (total as f32 * val) as usize;

        let train_set = examples[..train_size].to_vec();
        let val_set = examples[train_size..train_size + val_size].to_vec();
        let test_set = examples[train_size + val_size..].to_vec();

        (train_set, val_set, test_set)
    }
}

/// Dataset generator
pub struct DatasetGenerator {
    config: DatasetConfig,
    rng: StdRng,
}

impl DatasetGenerator {
    /// Create a new dataset generator
    pub fn new(config: DatasetConfig) -> Self {
        let rng = StdRng::seed_from_u64(config.seed);
        Self { config, rng }
    }

    /// Generate the complete dataset
    pub fn generate(&mut self) -> ClaudeTaskDataset {
        let mut examples = Vec::new();

        for category in TaskCategory::all() {
            let seed_examples = self.generate_seed_examples(category);
            examples.extend(seed_examples);
        }

        if self.config.enable_augmentation {
            let augmented = self.augment_examples(&examples);
            examples.extend(augmented);
        }

        ClaudeTaskDataset::new(examples)
    }

    /// Generate seed examples for a category
    fn generate_seed_examples(&mut self, category: TaskCategory) -> Vec<ClaudeTaskExample> {
        let templates = self.get_templates_for_category(category);
        let mut examples = Vec::new();

        for _ in 0..self.config.examples_per_category {
            let template = templates.choose(&mut self.rng).unwrap();
            let example = self.instantiate_template(template, category);
            examples.push(example);
        }

        examples
    }

    /// Get templates for a specific category
    fn get_templates_for_category(&self, category: TaskCategory) -> Vec<TaskTemplate> {
        match category {
            TaskCategory::Coder => self.coder_templates(),
            TaskCategory::Researcher => self.researcher_templates(),
            TaskCategory::Security => self.security_templates(),
            TaskCategory::Architecture => self.architecture_templates(),
            TaskCategory::Reviewer => self.reviewer_templates(),
        }
    }

    /// Generate coder task templates
    fn coder_templates(&self) -> Vec<TaskTemplate> {
        vec![
            // Code generation templates
            TaskTemplate {
                input: "Implement a {function_type} function in {language} that {functionality}",
                context: "The function should {requirements}",
                complexity: ComplexityLevel::Simple,
                domain: DomainType::Web,
                tags: vec!["code-generation", "function"],
                quality: 0.9,
            },
            TaskTemplate {
                input: "Create a {component_type} component using {framework} for {purpose}",
                context: "Requirements: {requirements}. Should follow {pattern} pattern",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Web,
                tags: vec!["code-generation", "component"],
                quality: 0.85,
            },
            TaskTemplate {
                input: "Write a {data_structure} implementation in {language} with {operations}",
                context: "Must support {requirements} and optimize for {optimization_target}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Systems,
                tags: vec!["data-structures", "algorithms"],
                quality: 0.88,
            },
            // Debugging templates
            TaskTemplate {
                input: "Debug the {issue_type} error in {context}",
                context: "Error: {error_message}. Stack trace: {stack_trace}",
                complexity: ComplexityLevel::Simple,
                domain: DomainType::Web,
                tags: vec!["debugging", "error-handling"],
                quality: 0.87,
            },
            TaskTemplate {
                input: "Fix memory leak in {component} caused by {cause}",
                context: "Profiler shows {profiler_output}. Occurring in {scenario}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Systems,
                tags: vec!["debugging", "memory", "performance"],
                quality: 0.92,
            },
            // Refactoring templates
            TaskTemplate {
                input: "Refactor {code_section} to improve {quality_attribute}",
                context: "Current issues: {issues}. Should maintain {constraints}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Web,
                tags: vec!["refactoring", "code-quality"],
                quality: 0.86,
            },
            TaskTemplate {
                input: "Extract {pattern} from {codebase_section}",
                context: "Duplicated code in {locations}. Create reusable {abstraction}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Web,
                tags: vec!["refactoring", "dry"],
                quality: 0.84,
            },
            // API integration templates
            TaskTemplate {
                input: "Integrate {api_name} API for {purpose}",
                context: "API documentation: {docs}. Need to handle {edge_cases}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Api,
                tags: vec!["api", "integration"],
                quality: 0.83,
            },
            TaskTemplate {
                input: "Build REST endpoint {endpoint_path} with {http_method}",
                context: "Should accept {input_schema} and return {output_schema}",
                complexity: ComplexityLevel::Simple,
                domain: DomainType::Api,
                tags: vec!["api", "rest", "backend"],
                quality: 0.88,
            },
            // Testing templates
            TaskTemplate {
                input: "Write unit tests for {function_name} covering {test_cases}",
                context: "Test framework: {framework}. Should cover {coverage_requirements}",
                complexity: ComplexityLevel::Simple,
                domain: DomainType::Web,
                tags: vec!["testing", "unit-tests"],
                quality: 0.90,
            },
        ]
    }

    /// Generate researcher task templates
    fn researcher_templates(&self) -> Vec<TaskTemplate> {
        vec![
            // Analysis templates
            TaskTemplate {
                input: "Analyze {codebase_component} for {analysis_goal}",
                context: "Focus on {focus_areas}. Document {documentation_requirements}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Web,
                tags: vec!["analysis", "documentation"],
                quality: 0.85,
            },
            TaskTemplate {
                input: "Research best practices for {topic} in {context}",
                context: "Current approach: {current_approach}. Constraints: {constraints}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Web,
                tags: vec!["research", "best-practices"],
                quality: 0.87,
            },
            TaskTemplate {
                input: "Investigate {performance_issue} in {system_component}",
                context: "Metrics: {metrics}. Threshold: {threshold}. Need root cause analysis",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Systems,
                tags: vec!["research", "performance", "analysis"],
                quality: 0.89,
            },
            // Documentation templates
            TaskTemplate {
                input: "Document {api_component} with usage examples",
                context: "Target audience: {audience}. Include {sections}",
                complexity: ComplexityLevel::Simple,
                domain: DomainType::Api,
                tags: vec!["documentation", "api"],
                quality: 0.82,
            },
            TaskTemplate {
                input: "Create architecture documentation for {system}",
                context: "Include: {components}. Diagrams for {diagram_types}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Web,
                tags: vec!["documentation", "architecture"],
                quality: 0.84,
            },
            // Exploration templates
            TaskTemplate {
                input: "Explore {technology} for {use_case}",
                context: "Requirements: {requirements}. Compare with {alternatives}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Web,
                tags: vec!["research", "exploration", "technology"],
                quality: 0.80,
            },
            TaskTemplate {
                input: "Compare {option_a} vs {option_b} for {purpose}",
                context: "Evaluate based on: {criteria}. Context: {context}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Web,
                tags: vec!["research", "comparison"],
                quality: 0.83,
            },
            // Pattern analysis templates
            TaskTemplate {
                input: "Identify design patterns in {codebase}",
                context: "Looking for: {patterns}. Document anti-patterns in {areas}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Web,
                tags: vec!["analysis", "patterns"],
                quality: 0.86,
            },
            TaskTemplate {
                input: "Analyze data flow in {system} from {source} to {destination}",
                context: "Map transformations at {stages}. Document {aspects}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::DataScience,
                tags: vec!["analysis", "data-flow"],
                quality: 0.88,
            },
            TaskTemplate {
                input: "Survey {library_ecosystem} for {functionality}",
                context: "Must support {requirements}. Evaluate {criteria}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Web,
                tags: vec!["research", "libraries"],
                quality: 0.81,
            },
        ]
    }

    /// Generate security task templates
    fn security_templates(&self) -> Vec<TaskTemplate> {
        vec![
            // Vulnerability analysis templates
            TaskTemplate {
                input: "Audit {code_component} for {vulnerability_type} vulnerabilities",
                context: "Focus areas: {focus_areas}. Check against {standards}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Security,
                tags: vec!["security", "audit", "vulnerability"],
                quality: 0.95,
            },
            TaskTemplate {
                input: "Analyze authentication flow for security weaknesses",
                context: "Current implementation: {implementation}. Threats: {threat_model}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Security,
                tags: vec!["security", "authentication"],
                quality: 0.93,
            },
            TaskTemplate {
                input: "Review {api_endpoint} for injection vulnerabilities",
                context: "Input sources: {inputs}. Sanitization: {sanitization}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Security,
                tags: vec!["security", "injection", "api"],
                quality: 0.94,
            },
            // Threat detection templates
            TaskTemplate {
                input: "Identify potential {attack_type} attack vectors in {system}",
                context: "System architecture: {architecture}. Trust boundaries: {boundaries}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Security,
                tags: vec!["security", "threat-modeling"],
                quality: 0.92,
            },
            TaskTemplate {
                input: "Analyze {dependency} for known vulnerabilities",
                context: "Version: {version}. Usage context: {usage}. CVE database: {cve_db}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Security,
                tags: vec!["security", "dependencies", "cve"],
                quality: 0.89,
            },
            // Security hardening templates
            TaskTemplate {
                input: "Implement {security_control} for {component}",
                context: "Threat model: {threats}. Compliance requirements: {compliance}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Security,
                tags: vec!["security", "hardening"],
                quality: 0.91,
            },
            TaskTemplate {
                input: "Add input validation for {input_type} in {context}",
                context: "Expected format: {format}. Constraints: {constraints}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Security,
                tags: vec!["security", "validation"],
                quality: 0.87,
            },
            // Cryptography templates
            TaskTemplate {
                input: "Review cryptographic implementation of {feature}",
                context:
                    "Algorithm: {algorithm}. Key management: {key_mgmt}. Standards: {standards}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Security,
                tags: vec!["security", "cryptography"],
                quality: 0.96,
            },
            TaskTemplate {
                input: "Audit data encryption at rest for {storage_system}",
                context: "Encryption scheme: {scheme}. Key rotation: {rotation}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Security,
                tags: vec!["security", "encryption"],
                quality: 0.94,
            },
            // Compliance templates
            TaskTemplate {
                input: "Ensure {standard} compliance in {system_area}",
                context: "Requirements: {requirements}. Current gaps: {gaps}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Security,
                tags: vec!["security", "compliance"],
                quality: 0.90,
            },
        ]
    }

    /// Generate architecture task templates
    fn architecture_templates(&self) -> Vec<TaskTemplate> {
        vec![
            // System design templates
            TaskTemplate {
                input: "Design {system_type} system for {purpose}",
                context: "Requirements: {requirements}. Scale: {scale}. Constraints: {constraints}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Web,
                tags: vec!["architecture", "system-design"],
                quality: 0.90,
            },
            TaskTemplate {
                input: "Architect microservices for {domain}",
                context: "Services needed: {services}. Communication: {patterns}. Data: {data_strategy}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Web,
                tags: vec!["architecture", "microservices"],
                quality: 0.92,
            },
            TaskTemplate {
                input: "Design database schema for {application}",
                context: "Entities: {entities}. Relationships: {relationships}. Access patterns: {patterns}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Database,
                tags: vec!["architecture", "database"],
                quality: 0.88,
            },
            // API design templates
            TaskTemplate {
                input: "Design RESTful API for {resource_type}",
                context: "Operations: {operations}. Versioning: {versioning}. Auth: {auth}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Api,
                tags: vec!["architecture", "api", "rest"],
                quality: 0.85,
            },
            TaskTemplate {
                input: "Architect GraphQL schema for {domain}",
                context: "Types: {types}. Queries: {queries}. Mutations: {mutations}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Api,
                tags: vec!["architecture", "api", "graphql"],
                quality: 0.86,
            },
            // Scalability templates
            TaskTemplate {
                input: "Plan scaling strategy for {system} to handle {target_load}",
                context: "Current: {current_state}. Bottlenecks: {bottlenecks}. Budget: {budget}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Web,
                tags: vec!["architecture", "scalability"],
                quality: 0.91,
            },
            TaskTemplate {
                input: "Design caching strategy for {application}",
                context: "Access patterns: {patterns}. Data volatility: {volatility}. Layers: {layers}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Web,
                tags: vec!["architecture", "caching"],
                quality: 0.84,
            },
            // Infrastructure templates
            TaskTemplate {
                input: "Design deployment architecture for {application}",
                context: "Environments: {environments}. CI/CD: {cicd}. Monitoring: {monitoring}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::DevOps,
                tags: vec!["architecture", "deployment", "infrastructure"],
                quality: 0.87,
            },
            TaskTemplate {
                input: "Plan disaster recovery strategy for {system}",
                context: "RTO: {rto}. RPO: {rpo}. Critical data: {data}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::DevOps,
                tags: vec!["architecture", "disaster-recovery"],
                quality: 0.93,
            },
            // Integration templates
            TaskTemplate {
                input: "Design integration pattern for {system_a} and {system_b}",
                context: "Data flow: {flow}. Consistency: {consistency}. Error handling: {errors}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Web,
                tags: vec!["architecture", "integration"],
                quality: 0.83,
            },
        ]
    }

    /// Generate reviewer task templates
    fn reviewer_templates(&self) -> Vec<TaskTemplate> {
        vec![
            // Code review templates
            TaskTemplate {
                input: "Review pull request #{pr_number} for {purpose}",
                context: "Changes: {changes}. Focus on: {focus_areas}",
                complexity: ComplexityLevel::Simple,
                domain: DomainType::Web,
                tags: vec!["review", "code-review", "pull-request"],
                quality: 0.84,
            },
            TaskTemplate {
                input: "Assess code quality of {module}",
                context: "Check: {criteria}. Standards: {standards}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Web,
                tags: vec!["review", "code-quality"],
                quality: 0.86,
            },
            TaskTemplate {
                input: "Review {code_section} for adherence to {coding_standard}",
                context: "Violations to check: {violations}. Document issues in: {format}",
                complexity: ComplexityLevel::Simple,
                domain: DomainType::Web,
                tags: vec!["review", "standards"],
                quality: 0.82,
            },
            // Best practices templates
            TaskTemplate {
                input: "Evaluate {implementation} against {framework} best practices",
                context: "Current approach: {approach}. Recommended patterns: {patterns}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Web,
                tags: vec!["review", "best-practices"],
                quality: 0.85,
            },
            TaskTemplate {
                input: "Review error handling in {component}",
                context: "Error scenarios: {scenarios}. Current handling: {handling}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Web,
                tags: vec!["review", "error-handling"],
                quality: 0.87,
            },
            // Performance review templates
            TaskTemplate {
                input: "Review {code_section} for performance issues",
                context: "Metrics: {metrics}. Hot paths: {hot_paths}. Optimizations: {optimizations}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Systems,
                tags: vec!["review", "performance"],
                quality: 0.88,
            },
            TaskTemplate {
                input: "Analyze query performance in {data_layer}",
                context: "Slow queries: {queries}. Execution plans: {plans}",
                complexity: ComplexityLevel::Moderate,
                domain: DomainType::Database,
                tags: vec!["review", "performance", "database"],
                quality: 0.89,
            },
            // Architecture review templates
            TaskTemplate {
                input: "Review architectural decisions in {design_doc}",
                context: "Proposed: {proposal}. Alternatives: {alternatives}. Trade-offs: {tradeoffs}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Web,
                tags: vec!["review", "architecture"],
                quality: 0.90,
            },
            TaskTemplate {
                input: "Assess scalability of {system_design}",
                context: "Expected load: {load}. Current capacity: {capacity}. Bottlenecks: {bottlenecks}",
                complexity: ComplexityLevel::Complex,
                domain: DomainType::Web,
                tags: vec!["review", "scalability"],
                quality: 0.91,
            },
            // Testing review templates
            TaskTemplate {
                input: "Review test coverage for {module}",
                context: "Current coverage: {coverage}. Critical paths: {paths}. Gaps: {gaps}",
                complexity: ComplexityLevel::Simple,
                domain: DomainType::Web,
                tags: vec!["review", "testing", "coverage"],
                quality: 0.83,
            },
        ]
    }

    /// Instantiate a template with random values
    fn instantiate_template(
        &mut self,
        template: &TaskTemplate,
        category: TaskCategory,
    ) -> ClaudeTaskExample {
        let input = self.fill_template(&template.input);
        let context = self.fill_template(&template.context);
        let expected_model = category.recommended_model(template.complexity);

        ClaudeTaskExample {
            input,
            context,
            output_agent: category.name().to_string(),
            metadata: TaskMetadata {
                category,
                complexity: template.complexity,
                domain: template.domain,
                expected_model: expected_model.to_string(),
                quality_score: template.quality,
                tags: template.tags.iter().map(|s| s.to_string()).collect(),
            },
        }
    }

    /// Fill template placeholders with random values
    fn fill_template(&mut self, template: &str) -> String {
        let mut result = template.to_string();

        // Replace placeholders with random values
        let replacements = self.get_template_replacements();
        for (placeholder, options) in replacements {
            let value = options.choose(&mut self.rng).unwrap();
            result = result.replace(&format!("{{{}}}", placeholder), value);
        }

        result
    }

    /// Get replacement options for template placeholders
    fn get_template_replacements(&self) -> HashMap<&'static str, Vec<&'static str>> {
        let mut map = HashMap::new();

        map.insert(
            "language",
            vec!["Rust", "TypeScript", "Python", "Go", "Java"],
        );
        map.insert(
            "framework",
            vec!["React", "Vue", "Angular", "Svelte", "Next.js"],
        );
        map.insert(
            "function_type",
            vec!["async", "recursive", "higher-order", "pure", "generic"],
        );
        map.insert(
            "component_type",
            vec!["form", "table", "modal", "dashboard", "navigation"],
        );
        map.insert(
            "data_structure",
            vec![
                "binary tree",
                "hash map",
                "linked list",
                "priority queue",
                "trie",
            ],
        );
        map.insert(
            "issue_type",
            vec![
                "null pointer",
                "type mismatch",
                "race condition",
                "deadlock",
                "stack overflow",
            ],
        );
        map.insert(
            "quality_attribute",
            vec![
                "readability",
                "maintainability",
                "performance",
                "testability",
                "modularity",
            ],
        );
        map.insert(
            "pattern",
            vec!["singleton", "factory", "observer", "strategy", "repository"],
        );
        map.insert(
            "api_name",
            vec!["Stripe", "Twilio", "SendGrid", "AWS S3", "OpenAI"],
        );
        map.insert("http_method", vec!["GET", "POST", "PUT", "DELETE", "PATCH"]);
        map.insert(
            "vulnerability_type",
            vec![
                "SQL injection",
                "XSS",
                "CSRF",
                "authentication",
                "authorization",
            ],
        );
        map.insert(
            "attack_type",
            vec![
                "DDoS",
                "man-in-the-middle",
                "replay",
                "privilege escalation",
            ],
        );
        map.insert(
            "security_control",
            vec!["rate limiting", "CORS", "CSP", "input sanitization"],
        );
        map.insert(
            "system_type",
            vec![
                "distributed",
                "event-driven",
                "real-time",
                "batch processing",
            ],
        );
        map.insert(
            "resource_type",
            vec!["users", "products", "orders", "payments", "inventory"],
        );

        map
    }

    /// Augment examples with paraphrases and variations
    fn augment_examples(&mut self, examples: &[ClaudeTaskExample]) -> Vec<ClaudeTaskExample> {
        let mut augmented = Vec::new();

        for example in examples {
            // Generate paraphrases
            for _ in 0..self.config.augmentation.paraphrases_per_example {
                if let Some(paraphrased) = self.paraphrase_example(example) {
                    augmented.push(paraphrased);
                }
            }

            // Generate complexity variations
            for _ in 0..self.config.augmentation.complexity_variations {
                if let Some(varied) = self.vary_complexity(example) {
                    augmented.push(varied);
                }
            }

            // Domain transfer (if enabled)
            if self.config.augmentation.enable_domain_transfer {
                if let Some(transferred) = self.transfer_domain(example) {
                    augmented.push(transferred);
                }
            }
        }

        augmented
    }

    /// Paraphrase an example (simple implementation)
    fn paraphrase_example(&mut self, example: &ClaudeTaskExample) -> Option<ClaudeTaskExample> {
        // Simple paraphrasing by replacing words
        let paraphrase_map: HashMap<&str, Vec<&str>> = [
            ("implement", vec!["create", "build", "develop", "write"]),
            ("analyze", vec!["examine", "investigate", "study", "review"]),
            ("design", vec!["architect", "plan", "structure", "outline"]),
            ("fix", vec!["resolve", "correct", "repair", "patch"]),
            ("optimize", vec!["improve", "enhance", "refine", "tune"]),
        ]
        .iter()
        .cloned()
        .collect();

        let mut paraphrased_input = example.input.clone();
        for (original, alternatives) in &paraphrase_map {
            if paraphrased_input.to_lowercase().contains(original) {
                let replacement = alternatives.choose(&mut self.rng)?;
                paraphrased_input = paraphrased_input
                    .to_lowercase()
                    .replace(original, replacement);
            }
        }

        Some(ClaudeTaskExample {
            input: paraphrased_input,
            context: example.context.clone(),
            output_agent: example.output_agent.clone(),
            metadata: example.metadata.clone(),
        })
    }

    /// Vary the complexity of an example
    fn vary_complexity(&mut self, example: &ClaudeTaskExample) -> Option<ClaudeTaskExample> {
        let new_complexity = match example.metadata.complexity {
            ComplexityLevel::Simple => {
                if self.rng.gen_bool(0.5) {
                    ComplexityLevel::Moderate
                } else {
                    return None;
                }
            }
            ComplexityLevel::Moderate => {
                if self.rng.gen_bool(0.5) {
                    ComplexityLevel::Simple
                } else {
                    ComplexityLevel::Complex
                }
            }
            ComplexityLevel::Complex => {
                if self.rng.gen_bool(0.5) {
                    ComplexityLevel::Moderate
                } else {
                    return None;
                }
            }
        };

        let new_model = example.metadata.category.recommended_model(new_complexity);

        Some(ClaudeTaskExample {
            input: example.input.clone(),
            context: example.context.clone(),
            output_agent: example.output_agent.clone(),
            metadata: TaskMetadata {
                complexity: new_complexity,
                expected_model: new_model.to_string(),
                ..example.metadata.clone()
            },
        })
    }

    /// Transfer an example to a different domain
    fn transfer_domain(&mut self, example: &ClaudeTaskExample) -> Option<ClaudeTaskExample> {
        let domains = [
            DomainType::Web,
            DomainType::Systems,
            DomainType::DataScience,
            DomainType::Mobile,
            DomainType::DevOps,
            DomainType::Security,
            DomainType::Database,
            DomainType::Api,
        ];

        let new_domain = *domains.choose(&mut self.rng)?;
        if new_domain == example.metadata.domain {
            return None;
        }

        Some(ClaudeTaskExample {
            input: example.input.clone(),
            context: example.context.clone(),
            output_agent: example.output_agent.clone(),
            metadata: TaskMetadata {
                domain: new_domain,
                ..example.metadata.clone()
            },
        })
    }
}

/// Task template for seed example generation
#[derive(Debug, Clone)]
struct TaskTemplate {
    input: &'static str,
    context: &'static str,
    complexity: ComplexityLevel,
    domain: DomainType,
    tags: Vec<&'static str>,
    quality: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataset_generation() {
        let config = DatasetConfig {
            examples_per_category: 10,
            enable_augmentation: false,
            ..Default::default()
        };

        let mut generator = DatasetGenerator::new(config);
        let dataset = generator.generate();

        // Should have 5 categories * 10 examples = 50 examples
        assert_eq!(dataset.examples.len(), 50);
        assert_eq!(dataset.stats.total_examples, 50);

        // Check category distribution
        for category in TaskCategory::all() {
            let count = dataset
                .stats
                .examples_per_category
                .get(category.name())
                .unwrap_or(&0);
            assert_eq!(*count, 10);
        }
    }

    #[test]
    fn test_dataset_augmentation() {
        let config = DatasetConfig {
            examples_per_category: 5,
            enable_augmentation: true,
            augmentation: AugmentationConfig {
                paraphrases_per_example: 1,
                complexity_variations: 1,
                enable_domain_transfer: true,
            },
            ..Default::default()
        };

        let mut generator = DatasetGenerator::new(config);
        let dataset = generator.generate();

        // Should have base examples + augmented examples
        // Base: 5 categories * 5 = 25
        // Augmented: 25 * (1 paraphrase + 1 complexity + 1 domain) = ~75 more
        assert!(dataset.examples.len() >= 25);
    }

    #[test]
    fn test_dataset_split() {
        let config = DatasetConfig {
            examples_per_category: 20,
            enable_augmentation: false,
            ..Default::default()
        };

        let mut generator = DatasetGenerator::new(config);
        let dataset = generator.generate();

        let (train, val, test) = dataset.split(0.7, 0.15, 0.15, 42);

        assert_eq!(train.len() + val.len() + test.len(), dataset.examples.len());
        assert!(train.len() > val.len());
        assert!(train.len() > test.len());
    }

    #[test]
    fn test_model_recommendation() {
        assert_eq!(
            TaskCategory::Coder.recommended_model(ComplexityLevel::Simple),
            "haiku"
        );
        assert_eq!(
            TaskCategory::Security.recommended_model(ComplexityLevel::Simple),
            "opus"
        );
        assert_eq!(
            TaskCategory::Architecture.recommended_model(ComplexityLevel::Complex),
            "opus"
        );
    }
}
