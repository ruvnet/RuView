//! SWE-Bench Task Loader
//!
//! Loads evaluation tasks from SWE-Bench format (JSON/JSONL).
//! SWE-Bench is a benchmark for evaluating LLMs on real-world software engineering tasks.
//!
//! ## Supported Formats
//!
//! - SWE-bench JSON (full dataset)
//! - SWE-bench-lite JSON (curated subset)
//! - JSONL (line-delimited JSON)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ruvllm::evaluation::swe_bench::{SweBenchLoader, SweBenchConfig};
//!
//! let loader = SweBenchLoader::new(SweBenchConfig::default());
//!
//! // Load from file
//! let tasks = loader.load_from_file("swe-bench-lite.json")?;
//!
//! // Load from URL (downloads and caches)
//! let tasks = loader.load_from_url(SweBenchLoader::LITE_URL).await?;
//!
//! // Convert to evaluation tasks
//! let eval_tasks: Vec<EvalTask> = tasks.into_iter().map(|t| t.into()).collect();
//! ```

use super::correctness::VerificationLevel;
use super::harness::EvalTask;
use crate::error::{Result, RuvLLMError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// SWE-Bench dataset URLs
pub const SWE_BENCH_LITE_URL: &str =
    "https://raw.githubusercontent.com/princeton-nlp/SWE-bench/main/swe-bench-lite.json";
pub const SWE_BENCH_FULL_URL: &str =
    "https://raw.githubusercontent.com/princeton-nlp/SWE-bench/main/swe-bench.json";

/// Configuration for SWE-Bench loader
#[derive(Debug, Clone)]
pub struct SweBenchConfig {
    /// Cache directory for downloaded datasets
    pub cache_dir: PathBuf,
    /// Maximum number of tasks to load (None = all)
    pub max_tasks: Option<usize>,
    /// Filter by repository (None = all repos)
    pub repo_filter: Option<String>,
    /// Filter by difficulty (easy, medium, hard)
    pub difficulty_filter: Option<String>,
    /// Include only tasks with gold patches
    pub require_gold_patch: bool,
    /// Include test commands
    pub include_tests: bool,
}

impl Default for SweBenchConfig {
    fn default() -> Self {
        Self {
            cache_dir: dirs::cache_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("ruvllm")
                .join("swe-bench"),
            max_tasks: None,
            repo_filter: None,
            difficulty_filter: None,
            require_gold_patch: false,
            include_tests: true,
        }
    }
}

impl SweBenchConfig {
    /// Create config for SWE-bench-lite (smaller, curated dataset)
    pub fn lite() -> Self {
        Self {
            max_tasks: Some(300),
            ..Default::default()
        }
    }

    /// Create config for quick testing (10 tasks)
    pub fn test() -> Self {
        Self {
            max_tasks: Some(10),
            ..Default::default()
        }
    }
}

/// A single SWE-Bench task entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweBenchTask {
    /// Unique instance ID (e.g., "django__django-11099")
    pub instance_id: String,

    /// Repository name (e.g., "django/django")
    #[serde(default)]
    pub repo: String,

    /// Base commit hash
    #[serde(default)]
    pub base_commit: String,

    /// Problem statement / issue description
    #[serde(default)]
    pub problem_statement: String,

    /// Hints provided (optional)
    #[serde(default)]
    pub hints_text: String,

    /// Gold patch (expected solution)
    #[serde(default)]
    pub patch: String,

    /// Test patch (tests to verify solution)
    #[serde(default)]
    pub test_patch: String,

    /// Files that need to be modified
    #[serde(default)]
    pub expected_files: Vec<String>,

    /// Test command to run
    #[serde(default)]
    pub test_cmd: String,

    /// Environment setup command
    #[serde(default)]
    pub env_setup_cmd: String,

    /// Python version required
    #[serde(default)]
    pub version: String,

    /// Difficulty level (if available)
    #[serde(default)]
    pub difficulty: Option<String>,

    /// Additional metadata
    #[serde(default, flatten)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl SweBenchTask {
    /// Parse instance_id to extract repo and issue number
    pub fn parse_instance_id(&self) -> (String, String) {
        // Format: "owner__repo-issue_number"
        let parts: Vec<&str> = self.instance_id.split('-').collect();
        if parts.len() >= 2 {
            let repo_part = parts[0].replace("__", "/");
            let issue = parts[1..].join("-");
            (repo_part, issue)
        } else {
            (self.repo.clone(), self.instance_id.clone())
        }
    }

    /// Get the full repository URL
    pub fn repo_url(&self) -> String {
        let (repo, _) = self.parse_instance_id();
        format!("https://github.com/{}", repo)
    }

    /// Check if this task has a gold patch
    pub fn has_gold_patch(&self) -> bool {
        !self.patch.is_empty()
    }

    /// Get files modified in the gold patch
    pub fn files_in_patch(&self) -> Vec<String> {
        if self.patch.is_empty() {
            return self.expected_files.clone();
        }

        let mut files = Vec::new();
        for line in self.patch.lines() {
            if line.starts_with("--- a/") {
                let file = line.trim_start_matches("--- a/").to_string();
                if !files.contains(&file) {
                    files.push(file);
                }
            } else if line.starts_with("+++ b/") {
                let file = line.trim_start_matches("+++ b/").to_string();
                if !files.contains(&file) {
                    files.push(file);
                }
            }
        }

        if files.is_empty() {
            self.expected_files.clone()
        } else {
            files
        }
    }
}

impl From<SweBenchTask> for EvalTask {
    fn from(task: SweBenchTask) -> Self {
        let (repo, issue) = task.parse_instance_id();
        let expected_files = task.files_in_patch();

        // Determine verification level
        let verification_level = if !task.test_patch.is_empty() {
            VerificationLevel::Automated
        } else {
            VerificationLevel::HumanVerified
        };

        EvalTask {
            id: task.instance_id,
            repo,
            issue: Some(issue),
            description: task.problem_statement,
            reference_patch: if task.patch.is_empty() {
                None
            } else {
                Some(task.patch)
            },
            test_command: if task.test_cmd.is_empty() {
                "pytest".to_string()
            } else {
                task.test_cmd
            },
            expected_files,
            verification_level,
            tags: vec![
                "swe-bench".to_string(),
                task.difficulty.unwrap_or_else(|| "unknown".to_string()),
            ],
        }
    }
}

/// SWE-Bench task loader
pub struct SweBenchLoader {
    config: SweBenchConfig,
}

impl SweBenchLoader {
    /// Create a new loader with configuration
    pub fn new(config: SweBenchConfig) -> Self {
        Self { config }
    }

    /// Load tasks from a local JSON file
    pub fn load_from_file<P: AsRef<Path>>(&self, path: P) -> Result<Vec<SweBenchTask>> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|e| {
            RuvLLMError::Storage(format!("Failed to read {}: {}", path.display(), e))
        })?;

        self.parse_tasks(&content)
    }

    /// Load tasks from a JSONL file (one JSON object per line)
    pub fn load_from_jsonl<P: AsRef<Path>>(&self, path: P) -> Result<Vec<SweBenchTask>> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|e| {
            RuvLLMError::Storage(format!("Failed to read {}: {}", path.display(), e))
        })?;

        let mut tasks = Vec::new();
        for (i, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<SweBenchTask>(line) {
                Ok(task) => tasks.push(task),
                Err(e) => {
                    tracing::warn!("Failed to parse line {}: {}", i + 1, e);
                }
            }
        }

        self.filter_tasks(tasks)
    }

    /// Parse tasks from JSON string (array or single object)
    fn parse_tasks(&self, content: &str) -> Result<Vec<SweBenchTask>> {
        // Try parsing as array first
        let tasks: Vec<SweBenchTask> = match serde_json::from_str(content) {
            Ok(arr) => arr,
            Err(_) => {
                // Try parsing as single object
                let task: SweBenchTask = serde_json::from_str(content).map_err(|e| {
                    RuvLLMError::Serialization(format!("Failed to parse JSON: {}", e))
                })?;
                vec![task]
            }
        };

        self.filter_tasks(tasks)
    }

    /// Apply filters to tasks
    fn filter_tasks(&self, tasks: Vec<SweBenchTask>) -> Result<Vec<SweBenchTask>> {
        let mut filtered: Vec<SweBenchTask> = tasks
            .into_iter()
            .filter(|task| {
                // Repo filter
                if let Some(ref repo_filter) = self.config.repo_filter {
                    if !task.repo.contains(repo_filter) && !task.instance_id.contains(repo_filter) {
                        return false;
                    }
                }

                // Difficulty filter
                if let Some(ref diff_filter) = self.config.difficulty_filter {
                    if let Some(ref difficulty) = task.difficulty {
                        if difficulty != diff_filter {
                            return false;
                        }
                    }
                }

                // Gold patch filter
                if self.config.require_gold_patch && !task.has_gold_patch() {
                    return false;
                }

                true
            })
            .collect();

        // Apply max_tasks limit
        if let Some(max) = self.config.max_tasks {
            filtered.truncate(max);
        }

        Ok(filtered)
    }

    /// Load from cache if available, or return instructions to download
    ///
    /// Since we don't include reqwest as a dependency, users should download manually:
    /// ```bash
    /// curl -o swe-bench-lite.json https://raw.githubusercontent.com/princeton-nlp/SWE-bench/main/swe-bench-lite.json
    /// ```
    pub fn load_from_cache_or_url(&self, url: &str) -> Result<Vec<SweBenchTask>> {
        // Create cache directory if needed
        if !self.config.cache_dir.exists() {
            fs::create_dir_all(&self.config.cache_dir)
                .map_err(|e| RuvLLMError::Storage(format!("Failed to create cache dir: {}", e)))?;
        }

        // Generate cache filename from URL
        let filename = url.split('/').last().unwrap_or("swe-bench.json");
        let cache_path = self.config.cache_dir.join(filename);

        // Check cache
        if cache_path.exists() {
            tracing::info!("Loading from cache: {}", cache_path.display());
            return self.load_from_file(&cache_path);
        }

        // No cache - provide instructions
        Err(RuvLLMError::NotFound(format!(
            "Dataset not cached. Download manually:\n\
             curl -o {} {}\n\
             Or use sample tasks with SweBenchLoader::sample_tasks()",
            cache_path.display(),
            url
        )))
    }

    /// Get the cache path for a given URL
    pub fn cache_path(&self, url: &str) -> PathBuf {
        let filename = url.split('/').last().unwrap_or("swe-bench.json");
        self.config.cache_dir.join(filename)
    }

    /// Create sample tasks for testing (no download required)
    pub fn sample_tasks() -> Vec<SweBenchTask> {
        vec![
            SweBenchTask {
                instance_id: "django__django-11099".to_string(),
                repo: "django/django".to_string(),
                base_commit: "abc123".to_string(),
                problem_statement: "UsernameValidator allows trailing newline in username"
                    .to_string(),
                hints_text: "The regex in UsernameValidator should use \\Z instead of $"
                    .to_string(),
                patch: r#"--- a/django/contrib/auth/validators.py
+++ b/django/contrib/auth/validators.py
@@ -8,7 +8,7 @@ class ASCIIUsernameValidator(validators.RegexValidator):
-    regex = r'^[\w.@+-]+$'
+    regex = r'^[\w.@+-]+\Z'
"#
                .to_string(),
                test_patch: String::new(),
                expected_files: vec!["django/contrib/auth/validators.py".to_string()],
                test_cmd: "python -m pytest django/contrib/auth/tests/test_validators.py"
                    .to_string(),
                env_setup_cmd: String::new(),
                version: "3.8".to_string(),
                difficulty: Some("easy".to_string()),
                metadata: HashMap::new(),
            },
            SweBenchTask {
                instance_id: "requests__requests-4356".to_string(),
                repo: "psf/requests".to_string(),
                base_commit: "def456".to_string(),
                problem_statement: "Session.request does not honor the `json` parameter"
                    .to_string(),
                hints_text: "Check how json parameter is passed in Session.request".to_string(),
                patch: r#"--- a/requests/sessions.py
+++ b/requests/sessions.py
@@ -465,6 +465,7 @@ class Session(SessionRedirectMixin):
         req = Request(
             method=method.upper(),
             url=url,
+            json=json,
             headers=headers,
"#
                .to_string(),
                test_patch: String::new(),
                expected_files: vec!["requests/sessions.py".to_string()],
                test_cmd: "python -m pytest tests/test_requests.py".to_string(),
                env_setup_cmd: String::new(),
                version: "3.9".to_string(),
                difficulty: Some("medium".to_string()),
                metadata: HashMap::new(),
            },
            SweBenchTask {
                instance_id: "flask__flask-4045".to_string(),
                repo: "pallets/flask".to_string(),
                base_commit: "ghi789".to_string(),
                problem_statement: "Add support for async view functions".to_string(),
                hints_text: "Need to detect and await async functions in dispatch".to_string(),
                patch: String::new(), // No gold patch - harder task
                test_patch: String::new(),
                expected_files: vec![
                    "src/flask/app.py".to_string(),
                    "src/flask/views.py".to_string(),
                ],
                test_cmd: "python -m pytest tests/".to_string(),
                env_setup_cmd: String::new(),
                version: "3.10".to_string(),
                difficulty: Some("hard".to_string()),
                metadata: HashMap::new(),
            },
        ]
    }

    /// Get statistics about loaded tasks
    pub fn stats(tasks: &[SweBenchTask]) -> SweBenchStats {
        let mut repos: HashMap<String, usize> = HashMap::new();
        let mut difficulties: HashMap<String, usize> = HashMap::new();
        let mut with_gold_patch = 0;
        let mut with_tests = 0;

        for task in tasks {
            let (repo, _) = task.parse_instance_id();
            *repos.entry(repo).or_insert(0) += 1;

            if let Some(ref diff) = task.difficulty {
                *difficulties.entry(diff.clone()).or_insert(0) += 1;
            }

            if task.has_gold_patch() {
                with_gold_patch += 1;
            }
            if !task.test_cmd.is_empty() {
                with_tests += 1;
            }
        }

        SweBenchStats {
            total_tasks: tasks.len(),
            repos,
            difficulties,
            with_gold_patch,
            with_tests,
        }
    }
}

/// Statistics about a SWE-Bench dataset
#[derive(Debug, Clone)]
pub struct SweBenchStats {
    /// Total number of tasks
    pub total_tasks: usize,
    /// Tasks per repository
    pub repos: HashMap<String, usize>,
    /// Tasks per difficulty level
    pub difficulties: HashMap<String, usize>,
    /// Tasks with gold patches
    pub with_gold_patch: usize,
    /// Tasks with test commands
    pub with_tests: usize,
}

impl std::fmt::Display for SweBenchStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "SWE-Bench Dataset Statistics")?;
        writeln!(f, "============================")?;
        writeln!(f, "Total tasks: {}", self.total_tasks)?;
        writeln!(
            f,
            "With gold patches: {} ({:.1}%)",
            self.with_gold_patch,
            self.with_gold_patch as f64 / self.total_tasks as f64 * 100.0
        )?;
        writeln!(
            f,
            "With test commands: {} ({:.1}%)",
            self.with_tests,
            self.with_tests as f64 / self.total_tasks as f64 * 100.0
        )?;

        writeln!(f, "\nBy Repository:")?;
        let mut repos: Vec<_> = self.repos.iter().collect();
        repos.sort_by(|a, b| b.1.cmp(a.1));
        for (repo, count) in repos.iter().take(10) {
            writeln!(f, "  {}: {}", repo, count)?;
        }
        if repos.len() > 10 {
            writeln!(f, "  ... and {} more", repos.len() - 10)?;
        }

        if !self.difficulties.is_empty() {
            writeln!(f, "\nBy Difficulty:")?;
            for (diff, count) in &self.difficulties {
                writeln!(f, "  {}: {}", diff, count)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_instance_id() {
        let task = SweBenchTask {
            instance_id: "django__django-11099".to_string(),
            ..Default::default()
        };

        let (repo, issue) = task.parse_instance_id();
        assert_eq!(repo, "django/django");
        assert_eq!(issue, "11099");
    }

    #[test]
    fn test_files_in_patch() {
        let task = SweBenchTask {
            instance_id: "test".to_string(),
            patch: r#"--- a/file1.py
+++ b/file1.py
@@ -1 +1 @@
-old
+new
--- a/file2.py
+++ b/file2.py
"#
            .to_string(),
            ..Default::default()
        };

        let files = task.files_in_patch();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"file1.py".to_string()));
        assert!(files.contains(&"file2.py".to_string()));
    }

    #[test]
    fn test_sample_tasks() {
        let tasks = SweBenchLoader::sample_tasks();
        assert_eq!(tasks.len(), 3);

        let stats = SweBenchLoader::stats(&tasks);
        assert_eq!(stats.total_tasks, 3);
        assert_eq!(stats.with_gold_patch, 2);
    }

    #[test]
    fn test_convert_to_eval_task() {
        let swe_task = SweBenchTask {
            instance_id: "django__django-11099".to_string(),
            repo: "django/django".to_string(),
            problem_statement: "Fix the validator".to_string(),
            patch: "--- a/file.py\n+++ b/file.py".to_string(),
            test_cmd: "pytest".to_string(),
            ..Default::default()
        };

        let eval_task: EvalTask = swe_task.into();
        assert_eq!(eval_task.id, "django__django-11099");
        assert_eq!(eval_task.repo, "django/django");
        assert!(eval_task.reference_patch.is_some());
    }

    #[test]
    fn test_loader_filter() {
        let config = SweBenchConfig {
            max_tasks: Some(2),
            repo_filter: Some("django".to_string()),
            ..Default::default()
        };

        let loader = SweBenchLoader::new(config);
        let tasks = SweBenchLoader::sample_tasks();
        let filtered = loader.filter_tasks(tasks).unwrap();

        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].instance_id.contains("django"));
    }
}

impl Default for SweBenchTask {
    fn default() -> Self {
        Self {
            instance_id: String::new(),
            repo: String::new(),
            base_commit: String::new(),
            problem_statement: String::new(),
            hints_text: String::new(),
            patch: String::new(),
            test_patch: String::new(),
            expected_files: Vec::new(),
            test_cmd: String::new(),
            env_setup_cmd: String::new(),
            version: String::new(),
            difficulty: None,
            metadata: HashMap::new(),
        }
    }
}
