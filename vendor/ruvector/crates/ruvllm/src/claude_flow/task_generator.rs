//! Task Generator for Claude Flow Pretraining
//!
//! Generates realistic tasks for pretraining the RuvLTRA model on Claude Flow use cases.
//!
//! ## Task Categories
//!
//! - **Coding Tasks**: implement, fix, refactor, optimize
//! - **Research Tasks**: analyze, investigate, explore
//! - **Review Tasks**: audit, inspect, verify
//! - **Architecture Tasks**: design, structure, plan
//! - **Testing Tasks**: test, validate, coverage
//! - **Security Tasks**: audit security, scan vulnerabilities
//! - **Performance Tasks**: benchmark, profile, optimize
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::claude_flow::task_generator::{TaskGenerator, TaskCategory, TaskComplexity};
//!
//! let generator = TaskGenerator::new();
//!
//! // Generate coding task
//! let task = generator.generate(TaskCategory::Coding, TaskComplexity::Moderate);
//! println!("Task: {}", task.description);
//! println!("Expected agent: {:?}", task.expected_agent);
//!
//! // Generate for specific agent
//! let research_task = generator.generate_for_agent(ClaudeFlowAgent::Researcher, TaskComplexity::Complex);
//! ```

use super::ClaudeFlowAgent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Task category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskCategory {
    /// Code implementation, bug fixes, refactoring
    Coding,
    /// Research, analysis, investigation
    Research,
    /// Code review, quality audit
    Review,
    /// System design, architecture planning
    Architecture,
    /// Testing, validation, QA
    Testing,
    /// Security auditing, vulnerability scanning
    Security,
    /// Performance optimization, benchmarking
    Performance,
    /// ML/AI development
    MachineLearning,
    /// CI/CD, DevOps
    DevOps,
    /// Documentation
    Documentation,
}

impl TaskCategory {
    /// Get all categories
    pub fn all() -> &'static [TaskCategory] {
        &[
            TaskCategory::Coding,
            TaskCategory::Research,
            TaskCategory::Review,
            TaskCategory::Architecture,
            TaskCategory::Testing,
            TaskCategory::Security,
            TaskCategory::Performance,
            TaskCategory::MachineLearning,
            TaskCategory::DevOps,
            TaskCategory::Documentation,
        ]
    }

    /// Get category name
    pub fn name(&self) -> &'static str {
        match self {
            TaskCategory::Coding => "coding",
            TaskCategory::Research => "research",
            TaskCategory::Review => "review",
            TaskCategory::Architecture => "architecture",
            TaskCategory::Testing => "testing",
            TaskCategory::Security => "security",
            TaskCategory::Performance => "performance",
            TaskCategory::MachineLearning => "machine_learning",
            TaskCategory::DevOps => "devops",
            TaskCategory::Documentation => "documentation",
        }
    }

    /// Get expected primary agent for this category
    pub fn primary_agent(&self) -> ClaudeFlowAgent {
        match self {
            TaskCategory::Coding => ClaudeFlowAgent::Coder,
            TaskCategory::Research => ClaudeFlowAgent::Researcher,
            TaskCategory::Review => ClaudeFlowAgent::Reviewer,
            TaskCategory::Architecture => ClaudeFlowAgent::Architect,
            TaskCategory::Testing => ClaudeFlowAgent::Tester,
            TaskCategory::Security => ClaudeFlowAgent::SecurityAuditor,
            TaskCategory::Performance => ClaudeFlowAgent::PerformanceEngineer,
            TaskCategory::MachineLearning => ClaudeFlowAgent::MlDeveloper,
            TaskCategory::DevOps => ClaudeFlowAgent::CicdEngineer,
            TaskCategory::Documentation => ClaudeFlowAgent::Researcher,
        }
    }

    /// Create from agent type
    pub fn from_agent(agent: ClaudeFlowAgent) -> Self {
        match agent {
            ClaudeFlowAgent::Coder | ClaudeFlowAgent::BackendDev => TaskCategory::Coding,
            ClaudeFlowAgent::Researcher => TaskCategory::Research,
            ClaudeFlowAgent::Tester => TaskCategory::Testing,
            ClaudeFlowAgent::Reviewer => TaskCategory::Review,
            ClaudeFlowAgent::Architect => TaskCategory::Architecture,
            ClaudeFlowAgent::SecurityAuditor => TaskCategory::Security,
            ClaudeFlowAgent::PerformanceEngineer => TaskCategory::Performance,
            ClaudeFlowAgent::MlDeveloper => TaskCategory::MachineLearning,
            ClaudeFlowAgent::CicdEngineer => TaskCategory::DevOps,
        }
    }

    /// Get random category
    pub fn random() -> Self {
        let categories = Self::all();
        let idx = (rand_simple() * categories.len() as f32) as usize;
        categories[idx.min(categories.len() - 1)]
    }
}

/// Task complexity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskComplexity {
    /// Simple, straightforward tasks
    Simple,
    /// Moderate complexity
    Moderate,
    /// Complex, multi-step tasks
    Complex,
    /// Expert-level, architectural decisions
    Expert,
}

impl TaskComplexity {
    /// Get all complexity levels
    pub fn all() -> &'static [TaskComplexity] {
        &[
            TaskComplexity::Simple,
            TaskComplexity::Moderate,
            TaskComplexity::Complex,
            TaskComplexity::Expert,
        ]
    }

    /// Get complexity name
    pub fn name(&self) -> &'static str {
        match self {
            TaskComplexity::Simple => "simple",
            TaskComplexity::Moderate => "moderate",
            TaskComplexity::Complex => "complex",
            TaskComplexity::Expert => "expert",
        }
    }

    /// Get numeric level (1-4)
    pub fn level(&self) -> u8 {
        match self {
            TaskComplexity::Simple => 1,
            TaskComplexity::Moderate => 2,
            TaskComplexity::Complex => 3,
            TaskComplexity::Expert => 4,
        }
    }

    /// Get random complexity
    pub fn random() -> Self {
        let levels = Self::all();
        let idx = (rand_simple() * levels.len() as f32) as usize;
        levels[idx.min(levels.len() - 1)]
    }

    /// Get weighted random (prefer simpler tasks)
    pub fn weighted_random() -> Self {
        let r = rand_simple();
        if r < 0.4 {
            TaskComplexity::Simple
        } else if r < 0.7 {
            TaskComplexity::Moderate
        } else if r < 0.9 {
            TaskComplexity::Complex
        } else {
            TaskComplexity::Expert
        }
    }
}

/// Generated task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTask {
    /// Task description
    pub description: String,
    /// Task category
    pub category: TaskCategory,
    /// Task complexity
    pub complexity: TaskComplexity,
    /// Expected agent to handle this task
    pub expected_agent: ClaudeFlowAgent,
    /// Keywords in the task
    pub keywords: Vec<String>,
    /// Optional context/requirements
    pub context: Option<String>,
}

impl GeneratedTask {
    /// Create a new generated task
    pub fn new(
        description: String,
        category: TaskCategory,
        complexity: TaskComplexity,
        expected_agent: ClaudeFlowAgent,
    ) -> Self {
        let keywords = Self::extract_keywords(&description);
        Self {
            description,
            category,
            complexity,
            expected_agent,
            keywords,
            context: None,
        }
    }

    /// Extract keywords from description
    fn extract_keywords(description: &str) -> Vec<String> {
        let keywords_set = [
            "implement",
            "create",
            "build",
            "fix",
            "refactor",
            "optimize",
            "research",
            "analyze",
            "investigate",
            "explore",
            "understand",
            "test",
            "verify",
            "validate",
            "coverage",
            "unit",
            "integration",
            "review",
            "audit",
            "inspect",
            "quality",
            "security",
            "design",
            "architecture",
            "structure",
            "pattern",
            "scalable",
            "performance",
            "benchmark",
            "profile",
            "memory",
            "latency",
            "train",
            "model",
            "neural",
            "embedding",
            "inference",
            "deploy",
            "ci",
            "cd",
            "pipeline",
            "workflow",
            "api",
            "endpoint",
            "database",
            "server",
            "rest",
        ];

        let lower = description.to_lowercase();
        keywords_set
            .iter()
            .filter(|k| lower.contains(*k))
            .map(|k| k.to_string())
            .collect()
    }

    /// Add context to task
    pub fn with_context(mut self, context: String) -> Self {
        self.context = Some(context);
        self
    }
}

/// Task template for generation
#[derive(Debug, Clone)]
struct TaskTemplate {
    /// Template string with placeholders
    template: &'static str,
    /// Placeholder values
    placeholders: Vec<&'static [&'static str]>,
    /// Complexity level this template is for
    complexity: TaskComplexity,
}

/// Task generator for pretraining
pub struct TaskGenerator {
    /// Templates per category
    templates: HashMap<TaskCategory, Vec<TaskTemplate>>,
    /// Technologies/languages for variation
    technologies: Vec<&'static str>,
    /// Components for variation
    components: Vec<&'static str>,
    /// Frameworks for variation
    frameworks: Vec<&'static str>,
    /// Total tasks generated
    tasks_generated: u64,
}

impl Default for TaskGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskGenerator {
    /// Create a new task generator
    pub fn new() -> Self {
        Self {
            templates: Self::build_templates(),
            technologies: vec![
                "Rust",
                "TypeScript",
                "Python",
                "Go",
                "JavaScript",
                "React",
                "Node.js",
                "PostgreSQL",
                "Redis",
                "MongoDB",
            ],
            components: vec![
                "user service",
                "authentication module",
                "API gateway",
                "payment processor",
                "notification system",
                "data pipeline",
                "caching layer",
                "rate limiter",
                "search engine",
                "analytics service",
            ],
            frameworks: vec![
                "actix-web",
                "tokio",
                "express",
                "fastapi",
                "gin",
                "next.js",
                "django",
                "spring",
                "axum",
                "rocket",
            ],
            tasks_generated: 0,
        }
    }

    /// Build task templates for each category
    fn build_templates() -> HashMap<TaskCategory, Vec<TaskTemplate>> {
        let mut templates = HashMap::new();

        // Coding templates
        templates.insert(
            TaskCategory::Coding,
            vec![
                TaskTemplate {
                    template: "implement a {} function in {}",
                    placeholders: vec![
                        &["sorting", "caching", "validation", "parsing", "formatting"],
                        &["Rust", "TypeScript", "Python"],
                    ],
                    complexity: TaskComplexity::Simple,
                },
                TaskTemplate {
                    template: "create a {} for the {}",
                    placeholders: vec![
                        &[
                            "REST endpoint",
                            "data model",
                            "service class",
                            "helper module",
                        ],
                        &["user service", "payment system", "notification service"],
                    ],
                    complexity: TaskComplexity::Moderate,
                },
                TaskTemplate {
                    template: "refactor the {} to use {} pattern",
                    placeholders: vec![
                        &["authentication module", "database layer", "API handlers"],
                        &["repository", "factory", "strategy", "observer"],
                    ],
                    complexity: TaskComplexity::Complex,
                },
                TaskTemplate {
                    template: "implement {} with {} for {} in a distributed system",
                    placeholders: vec![
                        &["consensus", "leader election", "state synchronization"],
                        &["Raft", "CRDT", "Paxos"],
                        &["the cluster manager", "the data store", "the message queue"],
                    ],
                    complexity: TaskComplexity::Expert,
                },
                TaskTemplate {
                    template: "fix the {} bug in the {}",
                    placeholders: vec![
                        &[
                            "memory leak",
                            "race condition",
                            "null pointer",
                            "off-by-one",
                        ],
                        &["connection pool", "request handler", "cache manager"],
                    ],
                    complexity: TaskComplexity::Moderate,
                },
                TaskTemplate {
                    template: "add {} handling to the {}",
                    placeholders: vec![
                        &["error", "retry", "timeout", "circuit breaker"],
                        &["API client", "database connection", "message consumer"],
                    ],
                    complexity: TaskComplexity::Simple,
                },
            ],
        );

        // Research templates
        templates.insert(
            TaskCategory::Research,
            vec![
                TaskTemplate {
                    template: "research best practices for {} in {}",
                    placeholders: vec![
                        &["authentication", "caching", "logging", "monitoring"],
                        &[
                            "microservices",
                            "serverless",
                            "monolith",
                            "distributed systems",
                        ],
                    ],
                    complexity: TaskComplexity::Simple,
                },
                TaskTemplate {
                    template: "analyze the {} patterns in the codebase",
                    placeholders: vec![
                        &[
                            "error handling",
                            "dependency injection",
                            "state management",
                            "API design",
                        ],
                        &[],
                    ],
                    complexity: TaskComplexity::Moderate,
                },
                TaskTemplate {
                    template: "investigate {} for implementing {}",
                    placeholders: vec![
                        &[
                            "different approaches",
                            "trade-offs",
                            "performance implications",
                        ],
                        &[
                            "real-time notifications",
                            "event sourcing",
                            "data replication",
                        ],
                    ],
                    complexity: TaskComplexity::Complex,
                },
                TaskTemplate {
                    template: "explore {} architectures for {} with {} requirements",
                    placeholders: vec![
                        &["event-driven", "CQRS", "hexagonal", "microkernel"],
                        &[
                            "high-throughput systems",
                            "low-latency applications",
                            "scalable platforms",
                        ],
                        &[
                            "strict consistency",
                            "eventual consistency",
                            "partition tolerance",
                        ],
                    ],
                    complexity: TaskComplexity::Expert,
                },
            ],
        );

        // Review templates
        templates.insert(
            TaskCategory::Review,
            vec![
                TaskTemplate {
                    template: "review the {} for code quality",
                    placeholders: vec![&["pull request", "module", "function", "class"], &[]],
                    complexity: TaskComplexity::Simple,
                },
                TaskTemplate {
                    template: "audit the {} for {} violations",
                    placeholders: vec![
                        &["codebase", "authentication module", "API layer"],
                        &["style guide", "best practice", "SOLID principle"],
                    ],
                    complexity: TaskComplexity::Moderate,
                },
                TaskTemplate {
                    template: "inspect the {} implementation for {} issues",
                    placeholders: vec![
                        &["database access", "API design", "error handling"],
                        &["performance", "security", "maintainability"],
                    ],
                    complexity: TaskComplexity::Complex,
                },
                TaskTemplate {
                    template: "conduct comprehensive code review of {} focusing on {} and {}",
                    placeholders: vec![
                        &[
                            "the entire service",
                            "the core domain",
                            "the infrastructure layer",
                        ],
                        &[
                            "architectural consistency",
                            "security vulnerabilities",
                            "performance bottlenecks",
                        ],
                        &[
                            "test coverage",
                            "documentation completeness",
                            "error handling robustness",
                        ],
                    ],
                    complexity: TaskComplexity::Expert,
                },
            ],
        );

        // Architecture templates
        templates.insert(
            TaskCategory::Architecture,
            vec![
                TaskTemplate {
                    template: "design a {} for the {}",
                    placeholders: vec![
                        &["data model", "API contract", "module structure"],
                        &["user service", "order system", "notification platform"],
                    ],
                    complexity: TaskComplexity::Simple,
                },
                TaskTemplate {
                    template: "design the {} architecture using {} pattern",
                    placeholders: vec![
                        &["service", "module", "system"],
                        &["hexagonal", "clean", "layered", "microservices"],
                    ],
                    complexity: TaskComplexity::Moderate,
                },
                TaskTemplate {
                    template: "architect a {} system with {} and {}",
                    placeholders: vec![
                        &["scalable", "resilient", "high-availability"],
                        &["load balancing", "auto-scaling", "failover"],
                        &["monitoring", "alerting", "self-healing"],
                    ],
                    complexity: TaskComplexity::Complex,
                },
                TaskTemplate {
                    template: "design {} architecture for {} handling {} with {} guarantees",
                    placeholders: vec![
                        &["distributed", "event-driven", "stream processing"],
                        &[
                            "real-time analytics",
                            "transaction processing",
                            "IoT data ingestion",
                        ],
                        &[
                            "millions of events per second",
                            "petabytes of data",
                            "global users",
                        ],
                        &[
                            "exactly-once delivery",
                            "strong consistency",
                            "sub-millisecond latency",
                        ],
                    ],
                    complexity: TaskComplexity::Expert,
                },
            ],
        );

        // Testing templates
        templates.insert(
            TaskCategory::Testing,
            vec![
                TaskTemplate {
                    template: "write unit tests for the {} function",
                    placeholders: vec![
                        &["validation", "parsing", "formatting", "calculation"],
                        &[],
                    ],
                    complexity: TaskComplexity::Simple,
                },
                TaskTemplate {
                    template: "create {} tests for the {}",
                    placeholders: vec![
                        &["integration", "unit", "e2e"],
                        &["user service", "authentication flow", "API endpoints"],
                    ],
                    complexity: TaskComplexity::Moderate,
                },
                TaskTemplate {
                    template: "implement {} testing strategy for {} with {} coverage",
                    placeholders: vec![
                        &["comprehensive", "property-based", "mutation"],
                        &["the core domain", "the API layer", "the data access layer"],
                        &["90%", "95%", "full path"],
                    ],
                    complexity: TaskComplexity::Complex,
                },
                TaskTemplate {
                    template: "design {} test suite for {} including {} and {} scenarios",
                    placeholders: vec![
                        &["chaos engineering", "load", "stress", "security"],
                        &[
                            "the distributed system",
                            "the microservices platform",
                            "the data pipeline",
                        ],
                        &[
                            "failure injection",
                            "network partitions",
                            "resource exhaustion",
                        ],
                        &[
                            "recovery verification",
                            "data integrity checks",
                            "SLA validation",
                        ],
                    ],
                    complexity: TaskComplexity::Expert,
                },
            ],
        );

        // Security templates
        templates.insert(
            TaskCategory::Security,
            vec![
                TaskTemplate {
                    template: "scan the {} for {} vulnerabilities",
                    placeholders: vec![
                        &["codebase", "dependencies", "configuration"],
                        &["known", "common", "critical"],
                    ],
                    complexity: TaskComplexity::Simple,
                },
                TaskTemplate {
                    template: "audit the {} for {} security issues",
                    placeholders: vec![
                        &["authentication module", "API endpoints", "database queries"],
                        &["injection", "authorization", "XSS"],
                    ],
                    complexity: TaskComplexity::Moderate,
                },
                TaskTemplate {
                    template: "perform {} security analysis of {} focusing on {}",
                    placeholders: vec![
                        &["comprehensive", "penetration", "threat modeling"],
                        &[
                            "the authentication system",
                            "the payment processing",
                            "the data storage",
                        ],
                        &["OWASP Top 10", "zero-trust principles", "data protection"],
                    ],
                    complexity: TaskComplexity::Complex,
                },
                TaskTemplate {
                    template: "design {} security architecture for {} with {} and {} compliance",
                    placeholders: vec![
                        &["defense-in-depth", "zero-trust", "secure-by-design"],
                        &[
                            "the enterprise platform",
                            "the financial system",
                            "the healthcare application",
                        ],
                        &["SOC2", "HIPAA", "PCI-DSS"],
                        &["GDPR", "ISO 27001", "FedRAMP"],
                    ],
                    complexity: TaskComplexity::Expert,
                },
            ],
        );

        // Performance templates
        templates.insert(
            TaskCategory::Performance,
            vec![
                TaskTemplate {
                    template: "profile the {} for {} bottlenecks",
                    placeholders: vec![
                        &["function", "module", "service"],
                        &["CPU", "memory", "I/O"],
                    ],
                    complexity: TaskComplexity::Simple,
                },
                TaskTemplate {
                    template: "optimize the {} for {} performance",
                    placeholders: vec![
                        &["database queries", "API endpoints", "data processing"],
                        &["latency", "throughput", "memory"],
                    ],
                    complexity: TaskComplexity::Moderate,
                },
                TaskTemplate {
                    template: "benchmark {} under {} load with {} metrics",
                    placeholders: vec![
                        &["the API", "the service", "the pipeline"],
                        &["high", "sustained", "burst"],
                        &["p99 latency", "throughput", "error rates"],
                    ],
                    complexity: TaskComplexity::Complex,
                },
                TaskTemplate {
                    template: "optimize {} for {} achieving {} with {} constraints",
                    placeholders: vec![
                        &[
                            "the distributed cache",
                            "the message processing",
                            "the ML inference",
                        ],
                        &[
                            "ultra-low latency",
                            "maximum throughput",
                            "optimal resource utilization",
                        ],
                        &[
                            "sub-millisecond p99",
                            "millions of ops/sec",
                            "linear scaling",
                        ],
                        &["memory limits", "cost constraints", "hardware restrictions"],
                    ],
                    complexity: TaskComplexity::Expert,
                },
            ],
        );

        // ML templates
        templates.insert(
            TaskCategory::MachineLearning,
            vec![
                TaskTemplate {
                    template: "implement {} for the {} model",
                    placeholders: vec![
                        &[
                            "data preprocessing",
                            "feature extraction",
                            "evaluation metrics",
                        ],
                        &["classification", "regression", "embedding"],
                    ],
                    complexity: TaskComplexity::Simple,
                },
                TaskTemplate {
                    template: "train a {} model for {}",
                    placeholders: vec![
                        &["neural network", "transformer", "ensemble"],
                        &[
                            "text classification",
                            "entity extraction",
                            "sentiment analysis",
                        ],
                    ],
                    complexity: TaskComplexity::Moderate,
                },
                TaskTemplate {
                    template: "optimize {} inference for {} with {}",
                    placeholders: vec![
                        &["model", "embedding", "transformer"],
                        &["real-time serving", "batch processing", "edge deployment"],
                        &["quantization", "pruning", "distillation"],
                    ],
                    complexity: TaskComplexity::Complex,
                },
                TaskTemplate {
                    template: "design {} ML pipeline for {} with {} and {}",
                    placeholders: vec![
                        &["end-to-end", "continuous learning", "multi-model"],
                        &[
                            "recommendation system",
                            "fraud detection",
                            "personalization engine",
                        ],
                        &["online learning", "A/B testing", "feature store"],
                        &["model versioning", "drift detection", "explainability"],
                    ],
                    complexity: TaskComplexity::Expert,
                },
            ],
        );

        // DevOps templates
        templates.insert(
            TaskCategory::DevOps,
            vec![
                TaskTemplate {
                    template: "create a {} workflow for the {}",
                    placeholders: vec![
                        &["CI", "CD", "build"],
                        &["service", "application", "library"],
                    ],
                    complexity: TaskComplexity::Simple,
                },
                TaskTemplate {
                    template: "set up {} pipeline with {} for {}",
                    placeholders: vec![
                        &["deployment", "testing", "release"],
                        &["GitHub Actions", "GitLab CI", "Jenkins"],
                        &["staging", "production", "multi-environment"],
                    ],
                    complexity: TaskComplexity::Moderate,
                },
                TaskTemplate {
                    template: "implement {} strategy for {} with {}",
                    placeholders: vec![
                        &["blue-green deployment", "canary release", "rolling update"],
                        &["the microservices", "the platform", "the cluster"],
                        &["automated rollback", "health checks", "traffic shifting"],
                    ],
                    complexity: TaskComplexity::Complex,
                },
                TaskTemplate {
                    template: "design {} infrastructure for {} with {} and {}",
                    placeholders: vec![
                        &["GitOps", "platform engineering", "self-service"],
                        &[
                            "multi-cloud deployment",
                            "global distribution",
                            "hybrid cloud",
                        ],
                        &[
                            "infrastructure as code",
                            "policy as code",
                            "security as code",
                        ],
                        &[
                            "observability",
                            "cost optimization",
                            "compliance automation",
                        ],
                    ],
                    complexity: TaskComplexity::Expert,
                },
            ],
        );

        // Documentation templates
        templates.insert(
            TaskCategory::Documentation,
            vec![
                TaskTemplate {
                    template: "document the {} API",
                    placeholders: vec![&["REST", "GraphQL", "gRPC"], &[]],
                    complexity: TaskComplexity::Simple,
                },
                TaskTemplate {
                    template: "create {} documentation for the {}",
                    placeholders: vec![
                        &["technical", "user", "API"],
                        &["authentication flow", "data model", "integration points"],
                    ],
                    complexity: TaskComplexity::Moderate,
                },
                TaskTemplate {
                    template: "write {} guide for {} covering {}",
                    placeholders: vec![
                        &["architecture", "operations", "development"],
                        &["the platform", "the system", "the service"],
                        &["design decisions", "best practices", "troubleshooting"],
                    ],
                    complexity: TaskComplexity::Complex,
                },
                TaskTemplate {
                    template: "create comprehensive {} documentation for {} including {} and {}",
                    placeholders: vec![
                        &["technical", "architectural", "operational"],
                        &[
                            "the entire platform",
                            "the distributed system",
                            "the ML pipeline",
                        ],
                        &["ADRs", "runbooks", "disaster recovery plans"],
                        &[
                            "capacity planning guides",
                            "security protocols",
                            "compliance procedures",
                        ],
                    ],
                    complexity: TaskComplexity::Expert,
                },
            ],
        );

        templates
    }

    /// Generate a task for a category and complexity
    pub fn generate(
        &mut self,
        category: TaskCategory,
        complexity: TaskComplexity,
    ) -> GeneratedTask {
        self.tasks_generated += 1;

        let templates = self.templates.get(&category).unwrap();

        // Filter templates by complexity (allow lower complexity too)
        let matching: Vec<_> = templates
            .iter()
            .filter(|t| t.complexity.level() <= complexity.level())
            .collect();

        let template = if matching.is_empty() {
            &templates[0]
        } else {
            let idx = (rand_simple() * matching.len() as f32) as usize;
            matching[idx.min(matching.len() - 1)]
        };

        // Fill in placeholders
        let description = self.fill_template(template);

        GeneratedTask::new(description, category, complexity, category.primary_agent())
    }

    /// Generate a task for a specific agent
    pub fn generate_for_agent(
        &mut self,
        agent: ClaudeFlowAgent,
        complexity: TaskComplexity,
    ) -> GeneratedTask {
        let category = TaskCategory::from_agent(agent);
        let mut task = self.generate(category, complexity);
        task.expected_agent = agent;
        task
    }

    /// Generate a batch of tasks
    pub fn generate_batch(
        &mut self,
        count: usize,
        category: Option<TaskCategory>,
    ) -> Vec<GeneratedTask> {
        (0..count)
            .map(|_| {
                let cat = category.unwrap_or_else(TaskCategory::random);
                let complexity = TaskComplexity::weighted_random();
                self.generate(cat, complexity)
            })
            .collect()
    }

    /// Generate balanced batch (equal across categories)
    pub fn generate_balanced_batch(&mut self, per_category: usize) -> Vec<GeneratedTask> {
        let mut tasks = Vec::new();

        for category in TaskCategory::all() {
            for complexity in TaskComplexity::all() {
                let count = per_category / TaskComplexity::all().len();
                for _ in 0..count {
                    tasks.push(self.generate(*category, *complexity));
                }
            }
        }

        tasks
    }

    /// Fill in template placeholders
    fn fill_template(&self, template: &TaskTemplate) -> String {
        let mut result = template.template.to_string();

        for placeholders in &template.placeholders {
            if placeholders.is_empty() {
                continue;
            }
            let idx = (rand_simple() * placeholders.len() as f32) as usize;
            let replacement = placeholders[idx.min(placeholders.len() - 1)];

            if let Some(pos) = result.find("{}") {
                result.replace_range(pos..pos + 2, replacement);
            }
        }

        // Add variation with technology/component names
        if rand_simple() > 0.5 && result.contains("the ") {
            let component =
                self.components[(rand_simple() * self.components.len() as f32) as usize];
            result = result.replace("the service", &format!("the {}", component));
        }

        result
    }

    /// Get total tasks generated
    pub fn tasks_generated(&self) -> u64 {
        self.tasks_generated
    }

    /// Reset generator state
    pub fn reset(&mut self) {
        self.tasks_generated = 0;
    }
}

/// Simple pseudo-random number generator
fn rand_simple() -> f32 {
    use std::cell::RefCell;

    thread_local! {
        static STATE: RefCell<u64> = RefCell::new(12345);
    }

    STATE.with(|state| {
        let mut s = state.borrow_mut();
        *s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (*s >> 33) as f32 / u32::MAX as f32
    })
}

/// Seed the random number generator
pub fn seed_rng(seed: u64) {
    use std::cell::RefCell;

    thread_local! {
        static STATE: RefCell<u64> = RefCell::new(12345);
    }

    STATE.with(|state| {
        *state.borrow_mut() = seed;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_category_all() {
        assert_eq!(TaskCategory::all().len(), 10);
    }

    #[test]
    fn test_task_complexity_level() {
        assert_eq!(TaskComplexity::Simple.level(), 1);
        assert_eq!(TaskComplexity::Expert.level(), 4);
    }

    #[test]
    fn test_task_generation() {
        let mut generator = TaskGenerator::new();
        let task = generator.generate(TaskCategory::Coding, TaskComplexity::Simple);

        assert_eq!(task.category, TaskCategory::Coding);
        assert_eq!(task.complexity, TaskComplexity::Simple);
        assert_eq!(task.expected_agent, ClaudeFlowAgent::Coder);
        assert!(!task.description.is_empty());
    }

    #[test]
    fn test_generate_for_agent() {
        let mut generator = TaskGenerator::new();
        let task =
            generator.generate_for_agent(ClaudeFlowAgent::Researcher, TaskComplexity::Moderate);

        assert_eq!(task.expected_agent, ClaudeFlowAgent::Researcher);
        assert_eq!(task.category, TaskCategory::Research);
    }

    #[test]
    fn test_batch_generation() {
        let mut generator = TaskGenerator::new();
        let tasks = generator.generate_batch(10, None);

        assert_eq!(tasks.len(), 10);
        assert!(generator.tasks_generated() >= 10);
    }

    #[test]
    fn test_balanced_batch() {
        let mut generator = TaskGenerator::new();
        let tasks = generator.generate_balanced_batch(4);

        assert!(!tasks.is_empty());
        // Should have tasks from multiple categories
        let categories: std::collections::HashSet<_> = tasks.iter().map(|t| t.category).collect();
        assert!(categories.len() > 1);
    }

    #[test]
    fn test_keyword_extraction() {
        let task = GeneratedTask::new(
            "implement a validation function for the authentication module".to_string(),
            TaskCategory::Coding,
            TaskComplexity::Simple,
            ClaudeFlowAgent::Coder,
        );

        assert!(task.keywords.contains(&"implement".to_string()));
        assert!(task.keywords.contains(&"validation".to_string()));
    }

    #[test]
    fn test_category_from_agent() {
        assert_eq!(
            TaskCategory::from_agent(ClaudeFlowAgent::Coder),
            TaskCategory::Coding
        );
        assert_eq!(
            TaskCategory::from_agent(ClaudeFlowAgent::Researcher),
            TaskCategory::Research
        );
        assert_eq!(
            TaskCategory::from_agent(ClaudeFlowAgent::SecurityAuditor),
            TaskCategory::Security
        );
    }

    #[test]
    fn test_primary_agent() {
        assert_eq!(TaskCategory::Coding.primary_agent(), ClaudeFlowAgent::Coder);
        assert_eq!(
            TaskCategory::Testing.primary_agent(),
            ClaudeFlowAgent::Tester
        );
        assert_eq!(
            TaskCategory::Security.primary_agent(),
            ClaudeFlowAgent::SecurityAuditor
        );
    }

    #[test]
    fn test_all_categories_have_templates() {
        let generator = TaskGenerator::new();

        for category in TaskCategory::all() {
            assert!(
                generator.templates.contains_key(category),
                "Missing templates for category: {:?}",
                category
            );
        }
    }

    #[test]
    fn test_expert_complexity_tasks() {
        let mut generator = TaskGenerator::new();

        for category in TaskCategory::all() {
            let task = generator.generate(*category, TaskComplexity::Expert);
            assert!(!task.description.is_empty());
            assert!(task.description.len() > 10); // Expert tasks should be descriptive
        }
    }
}
