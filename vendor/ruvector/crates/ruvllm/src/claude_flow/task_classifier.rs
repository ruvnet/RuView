//! Task Classifier for Claude Flow
//!
//! Classifies tasks into categories for optimal routing and processing.

use super::ClaudeFlowTask;

/// Task type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskType {
    /// Code implementation
    Code,
    /// Research and analysis
    Research,
    /// Testing and QA
    Test,
    /// Code review
    Review,
    /// Documentation
    Docs,
    /// Debugging
    Debug,
    /// Architecture design
    Architecture,
    /// Security audit
    Security,
    /// Performance optimization
    Performance,
    /// Unknown/general
    General,
}

impl From<ClaudeFlowTask> for TaskType {
    fn from(task: ClaudeFlowTask) -> Self {
        match task {
            ClaudeFlowTask::CodeGeneration => TaskType::Code,
            ClaudeFlowTask::CodeReview => TaskType::Review,
            ClaudeFlowTask::Testing => TaskType::Test,
            ClaudeFlowTask::Research => TaskType::Research,
            ClaudeFlowTask::Documentation => TaskType::Docs,
            ClaudeFlowTask::Debugging => TaskType::Debug,
            ClaudeFlowTask::Refactoring => TaskType::Code,
            ClaudeFlowTask::Security => TaskType::Security,
            ClaudeFlowTask::Performance => TaskType::Performance,
            ClaudeFlowTask::Architecture => TaskType::Architecture,
        }
    }
}

/// Classification result with confidence
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    /// Primary task type
    pub task_type: TaskType,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Secondary classifications
    pub secondary: Vec<(TaskType, f32)>,
    /// Detected programming languages
    pub languages: Vec<String>,
    /// Detected frameworks/tools
    pub frameworks: Vec<String>,
    /// Complexity estimate (1-10)
    pub complexity: u8,
    /// Estimated agent count needed
    pub recommended_agents: u8,
}

/// Task classifier using RuvLTRA embeddings
pub struct TaskClassifier {
    /// Language detection patterns
    language_patterns: Vec<(String, Vec<&'static str>)>,
    /// Framework detection patterns
    framework_patterns: Vec<(String, Vec<&'static str>)>,
}

impl TaskClassifier {
    /// Create a new task classifier
    pub fn new() -> Self {
        Self {
            language_patterns: Self::build_language_patterns(),
            framework_patterns: Self::build_framework_patterns(),
        }
    }

    fn build_language_patterns() -> Vec<(String, Vec<&'static str>)> {
        vec![
            (
                "rust".to_string(),
                vec!["rust", "cargo", ".rs", "tokio", "async-std", "serde"],
            ),
            (
                "typescript".to_string(),
                vec!["typescript", "ts", ".tsx", "deno", "bun"],
            ),
            (
                "javascript".to_string(),
                vec!["javascript", "js", "node", "npm", "react", "vue"],
            ),
            (
                "python".to_string(),
                vec!["python", "pip", ".py", "django", "flask", "pytorch"],
            ),
            ("go".to_string(), vec!["golang", "go ", ".go", "goroutine"]),
        ]
    }

    fn build_framework_patterns() -> Vec<(String, Vec<&'static str>)> {
        vec![
            (
                "react".to_string(),
                vec!["react", "jsx", "tsx", "next.js", "nextjs"],
            ),
            (
                "express".to_string(),
                vec!["express", "middleware", "router"],
            ),
            (
                "tokio".to_string(),
                vec!["tokio", "async", "await", "spawn"],
            ),
            ("actix".to_string(), vec!["actix", "actix-web"]),
            (
                "jest".to_string(),
                vec!["jest", "describe", "it(", "expect("],
            ),
            ("pytest".to_string(), vec!["pytest", "test_", "fixture"]),
        ]
    }

    /// Classify a task description
    pub fn classify(&self, description: &str) -> ClassificationResult {
        let lower = description.to_lowercase();

        // Detect task type
        let (task_type, confidence, secondary) = self.detect_task_type(&lower);

        // Detect languages
        let languages = self.detect_languages(&lower);

        // Detect frameworks
        let frameworks = self.detect_frameworks(&lower);

        // Estimate complexity
        let complexity = self.estimate_complexity(&lower, &languages);

        // Recommend agent count
        let recommended_agents = self.recommend_agent_count(complexity, &secondary);

        ClassificationResult {
            task_type,
            confidence,
            secondary,
            languages,
            frameworks,
            complexity,
            recommended_agents,
        }
    }

    fn detect_task_type(&self, lower: &str) -> (TaskType, f32, Vec<(TaskType, f32)>) {
        let mut scores: Vec<(TaskType, f32)> = vec![
            (TaskType::Code, self.score_code(lower)),
            (TaskType::Research, self.score_research(lower)),
            (TaskType::Test, self.score_test(lower)),
            (TaskType::Review, self.score_review(lower)),
            (TaskType::Docs, self.score_docs(lower)),
            (TaskType::Debug, self.score_debug(lower)),
            (TaskType::Architecture, self.score_architecture(lower)),
            (TaskType::Security, self.score_security(lower)),
            (TaskType::Performance, self.score_performance(lower)),
        ];

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let primary = scores[0];
        let secondary: Vec<(TaskType, f32)> = scores[1..4]
            .iter()
            .filter(|(_, s)| *s > 0.1)
            .cloned()
            .collect();

        (primary.0, primary.1, secondary)
    }

    fn score_code(&self, s: &str) -> f32 {
        let keywords = [
            "implement",
            "create",
            "build",
            "code",
            "write",
            "function",
            "class",
            "module",
        ];
        self.keyword_score(s, &keywords)
    }

    fn score_research(&self, s: &str) -> f32 {
        let keywords = [
            "research",
            "analyze",
            "investigate",
            "explore",
            "find",
            "understand",
            "learn",
        ];
        self.keyword_score(s, &keywords)
    }

    fn score_test(&self, s: &str) -> f32 {
        let keywords = [
            "test",
            "verify",
            "validate",
            "assert",
            "coverage",
            "unit",
            "integration",
            "e2e",
        ];
        self.keyword_score(s, &keywords)
    }

    fn score_review(&self, s: &str) -> f32 {
        let keywords = [
            "review", "audit", "inspect", "check", "quality", "lint", "pr",
        ];
        self.keyword_score(s, &keywords)
    }

    fn score_docs(&self, s: &str) -> f32 {
        let keywords = [
            "document", "readme", "api docs", "comment", "explain", "describe",
        ];
        self.keyword_score(s, &keywords)
    }

    fn score_debug(&self, s: &str) -> f32 {
        let keywords = [
            "debug",
            "fix",
            "error",
            "bug",
            "issue",
            "crash",
            "exception",
            "trace",
        ];
        self.keyword_score(s, &keywords)
    }

    fn score_architecture(&self, s: &str) -> f32 {
        let keywords = [
            "architecture",
            "design",
            "structure",
            "pattern",
            "system",
            "scalable",
            "modular",
        ];
        self.keyword_score(s, &keywords)
    }

    fn score_security(&self, s: &str) -> f32 {
        let keywords = [
            "security",
            "vulnerability",
            "cve",
            "injection",
            "auth",
            "encrypt",
            "xss",
            "csrf",
        ];
        self.keyword_score(s, &keywords)
    }

    fn score_performance(&self, s: &str) -> f32 {
        let keywords = [
            "performance",
            "optimize",
            "speed",
            "memory",
            "benchmark",
            "profile",
            "latency",
            "throughput",
        ];
        self.keyword_score(s, &keywords)
    }

    fn keyword_score(&self, text: &str, keywords: &[&str]) -> f32 {
        let matches: f32 = keywords.iter().filter(|k| text.contains(*k)).count() as f32;
        (matches / keywords.len() as f32).min(1.0)
    }

    fn detect_languages(&self, lower: &str) -> Vec<String> {
        self.language_patterns
            .iter()
            .filter(|(_, patterns)| patterns.iter().any(|p| lower.contains(p)))
            .map(|(lang, _)| lang.clone())
            .collect()
    }

    fn detect_frameworks(&self, lower: &str) -> Vec<String> {
        self.framework_patterns
            .iter()
            .filter(|(_, patterns)| patterns.iter().any(|p| lower.contains(p)))
            .map(|(fw, _)| fw.clone())
            .collect()
    }

    fn estimate_complexity(&self, lower: &str, languages: &[String]) -> u8 {
        let mut complexity: u8 = 3; // Base complexity

        // Multi-language increases complexity
        complexity += (languages.len() as u8).saturating_sub(1);

        // Certain keywords indicate higher complexity
        if lower.contains("distributed") || lower.contains("concurrent") {
            complexity += 2;
        }
        if lower.contains("migration") || lower.contains("refactor") {
            complexity += 1;
        }
        if lower.contains("security") || lower.contains("authentication") {
            complexity += 1;
        }

        // Cap at 10
        complexity.min(10)
    }

    fn recommend_agent_count(&self, complexity: u8, secondary: &[(TaskType, f32)]) -> u8 {
        let base = match complexity {
            1..=3 => 1,
            4..=6 => 2,
            7..=8 => 3,
            _ => 4,
        };

        // Add agents for secondary task types
        let secondary_count = secondary.iter().filter(|(_, score)| *score > 0.3).count() as u8;

        (base + secondary_count.min(2)).min(6)
    }
}

impl Default for TaskClassifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classification() {
        let classifier = TaskClassifier::new();

        let result = classifier.classify("implement a REST API endpoint in Rust using actix-web");
        assert_eq!(result.task_type, TaskType::Code);
        assert!(result.languages.contains(&"rust".to_string()));
        assert!(result.frameworks.contains(&"actix".to_string()));
    }

    #[test]
    fn test_complexity() {
        let classifier = TaskClassifier::new();

        let simple = classifier.classify("fix a typo");
        let complex =
            classifier.classify("implement distributed authentication with security audit");

        assert!(complex.complexity > simple.complexity);
    }
}
