//! Model card generation for HuggingFace Hub

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Model task type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskType {
    TextGeneration,
    ConversationalAi,
    CodeCompletion,
    QuestionAnswering,
    Summarization,
}

/// ML framework
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Framework {
    Gguf,
    PyTorch,
    TensorFlow,
    Onnx,
}

/// Model license
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum License {
    Mit,
    Apache20,
    Gpl30,
    Bsd3Clause,
    CreativemlOpenrailM,
    Llama2,
    Other(String),
}

impl std::str::FromStr for License {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mit" => Ok(Self::Mit),
            "apache-2.0" | "apache2.0" => Ok(Self::Apache20),
            "gpl-3.0" | "gpl3.0" => Ok(Self::Gpl30),
            "bsd-3-clause" => Ok(Self::Bsd3Clause),
            "creativeml-openrail-m" => Ok(Self::CreativemlOpenrailM),
            "llama2" => Ok(Self::Llama2),
            other => Ok(Self::Other(other.to_string())),
        }
    }
}

/// Dataset information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetInfo {
    /// Dataset name/identifier
    pub name: String,
    /// Dataset description
    pub description: Option<String>,
}

/// Metric result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricResult {
    /// Metric name (e.g., "perplexity", "accuracy")
    pub name: String,
    /// Metric value
    pub value: f64,
    /// Dataset used for evaluation
    pub dataset: Option<String>,
}

/// Model card for HuggingFace Hub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCard {
    /// Model name
    pub name: String,
    /// Short description
    pub description: Option<String>,
    /// Task type
    pub task: TaskType,
    /// Framework
    pub framework: Framework,
    /// Architecture (e.g., "llama", "qwen2")
    pub architecture: String,
    /// Model license
    pub license: License,
    /// Number of parameters
    pub parameters: u64,
    /// Context window size
    pub context_length: usize,
    /// Training datasets
    pub datasets: Vec<DatasetInfo>,
    /// Evaluation metrics
    pub metrics: Vec<MetricResult>,
    /// Model tags
    pub tags: Vec<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl ModelCard {
    /// Convert model card to YAML frontmatter + markdown
    pub fn to_markdown(&self) -> String {
        let mut content = String::new();

        // YAML frontmatter
        content.push_str("---\n");
        content.push_str(&format!("language: en\n"));
        content.push_str(&format!("license: {}\n", self.license_str()));
        content.push_str(&format!("library_name: ruvltra\n"));

        if !self.tags.is_empty() {
            content.push_str("tags:\n");
            for tag in &self.tags {
                content.push_str(&format!("- {}\n", tag));
            }
        }

        content.push_str("---\n\n");

        // Model description
        content.push_str(&format!("# {}\n\n", self.name));

        if let Some(desc) = &self.description {
            content.push_str(&format!("{}\n\n", desc));
        }

        // Model details
        content.push_str("## Model Details\n\n");
        content.push_str(&format!("- **Architecture**: {}\n", self.architecture));
        content.push_str(&format!(
            "- **Parameters**: {}\n",
            format_params(self.parameters)
        ));
        content.push_str(&format!(
            "- **Context Length**: {} tokens\n",
            self.context_length
        ));
        content.push_str(&format!("- **Framework**: {:?}\n", self.framework));
        content.push_str(&format!("- **Task**: {:?}\n\n", self.task));

        // Training data
        if !self.datasets.is_empty() {
            content.push_str("## Training Data\n\n");
            for dataset in &self.datasets {
                content.push_str(&format!("- **{}**", dataset.name));
                if let Some(desc) = &dataset.description {
                    content.push_str(&format!(": {}", desc));
                }
                content.push_str("\n");
            }
            content.push_str("\n");
        }

        // Evaluation metrics
        if !self.metrics.is_empty() {
            content.push_str("## Evaluation\n\n");
            content.push_str("| Metric | Value | Dataset |\n");
            content.push_str("|--------|-------|----------|\n");
            for metric in &self.metrics {
                content.push_str(&format!(
                    "| {} | {:.2} | {} |\n",
                    metric.name,
                    metric.value,
                    metric.dataset.as_deref().unwrap_or("N/A")
                ));
            }
            content.push_str("\n");
        }

        // Usage
        content.push_str("## Usage\n\n");
        content.push_str("```bash\n");
        content.push_str("# Download using ruvllm CLI\n");
        content.push_str(&format!("ruvllm pull {}\n", self.name.to_lowercase()));
        content.push_str("```\n\n");

        content.push_str("```rust\n");
        content.push_str("use ruvllm::hub::ModelDownloader;\n\n");
        content.push_str("let downloader = ModelDownloader::new();\n");
        content.push_str(&format!(
            "let path = downloader.download_by_id(\"{}\")?;\n",
            self.name.to_lowercase()
        ));
        content.push_str("```\n\n");

        // Additional metadata
        if !self.metadata.is_empty() {
            content.push_str("## Additional Information\n\n");
            for (key, value) in &self.metadata {
                content.push_str(&format!("- **{}**: {}\n", key, value));
            }
            content.push_str("\n");
        }

        // Footer
        content.push_str("---\n\n");
        content.push_str("*This model card was generated automatically by RuvLLM*\n");

        content
    }

    /// Get license as string
    fn license_str(&self) -> &str {
        match &self.license {
            License::Mit => "mit",
            License::Apache20 => "apache-2.0",
            License::Gpl30 => "gpl-3.0",
            License::Bsd3Clause => "bsd-3-clause",
            License::CreativemlOpenrailM => "creativeml-openrail-m",
            License::Llama2 => "llama2",
            License::Other(s) => s,
        }
    }
}

/// Model card builder
pub struct ModelCardBuilder {
    name: String,
    description: Option<String>,
    task: TaskType,
    framework: Framework,
    architecture: String,
    license: License,
    parameters: u64,
    context_length: usize,
    datasets: Vec<DatasetInfo>,
    metrics: Vec<MetricResult>,
    tags: Vec<String>,
    metadata: HashMap<String, String>,
}

impl ModelCardBuilder {
    /// Create a new model card builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            task: TaskType::TextGeneration,
            framework: Framework::Gguf,
            architecture: "llama".to_string(),
            license: License::Mit,
            parameters: 0,
            context_length: 4096,
            datasets: Vec::new(),
            metrics: Vec::new(),
            tags: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Set description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set task type
    pub fn task(mut self, task: TaskType) -> Self {
        self.task = task;
        self
    }

    /// Set framework
    pub fn framework(mut self, framework: Framework) -> Self {
        self.framework = framework;
        self
    }

    /// Set architecture
    pub fn architecture(mut self, arch: impl Into<String>) -> Self {
        self.architecture = arch.into();
        self
    }

    /// Set license
    pub fn license(mut self, license: License) -> Self {
        self.license = license;
        self
    }

    /// Set parameter count
    pub fn parameters(mut self, params: u64) -> Self {
        self.parameters = params;
        self
    }

    /// Set context length
    pub fn context_length(mut self, length: usize) -> Self {
        self.context_length = length;
        self
    }

    /// Add a dataset
    pub fn add_dataset(mut self, name: impl Into<String>, desc: Option<String>) -> Self {
        self.datasets.push(DatasetInfo {
            name: name.into(),
            description: desc,
        });
        self
    }

    /// Add a metric
    pub fn add_metric(
        mut self,
        name: impl Into<String>,
        value: f64,
        dataset: Option<String>,
    ) -> Self {
        self.metrics.push(MetricResult {
            name: name.into(),
            value,
            dataset,
        });
        self
    }

    /// Add a tag
    pub fn add_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add metadata
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Build the model card
    pub fn build(self) -> ModelCard {
        ModelCard {
            name: self.name,
            description: self.description,
            task: self.task,
            framework: self.framework,
            architecture: self.architecture,
            license: self.license,
            parameters: self.parameters,
            context_length: self.context_length,
            datasets: self.datasets,
            metrics: self.metrics,
            tags: self.tags,
            metadata: self.metadata,
        }
    }
}

/// Format parameter count as human-readable string
fn format_params(params: u64) -> String {
    const B: u64 = 1_000_000_000;
    const M: u64 = 1_000_000;
    const K: u64 = 1_000;

    if params >= B {
        format!("{:.1}B", params as f64 / B as f64)
    } else if params >= M {
        format!("{:.0}M", params as f64 / M as f64)
    } else if params >= K {
        format!("{:.0}K", params as f64 / K as f64)
    } else {
        format!("{}", params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_card_builder() {
        let card = ModelCardBuilder::new("Test Model")
            .description("A test model")
            .architecture("llama")
            .parameters(500_000_000)
            .context_length(4096)
            .add_tag("test")
            .build();

        assert_eq!(card.name, "Test Model");
        assert_eq!(card.parameters, 500_000_000);
        assert_eq!(card.tags.len(), 1);
    }

    #[test]
    fn test_model_card_markdown() {
        let card = ModelCardBuilder::new("RuvLTRA Small")
            .description("Compact model")
            .parameters(500_000_000)
            .add_dataset("dataset1", Some("Training data".to_string()))
            .add_metric("perplexity", 5.2, Some("test-set".to_string()))
            .build();

        let markdown = card.to_markdown();
        assert!(markdown.contains("# RuvLTRA Small"));
        assert!(markdown.contains("0.5B"));
        assert!(markdown.contains("dataset1"));
        assert!(markdown.contains("perplexity"));
    }

    #[test]
    fn test_format_params() {
        assert_eq!(format_params(500), "500");
        assert_eq!(format_params(5_000), "5K");
        assert_eq!(format_params(5_000_000), "5M");
        assert_eq!(format_params(500_000_000), "0.5B");
        assert_eq!(format_params(3_000_000_000), "3.0B");
    }

    #[test]
    fn test_license_from_str() {
        use std::str::FromStr;

        assert_eq!(License::from_str("mit").unwrap(), License::Mit);
        assert_eq!(License::from_str("apache-2.0").unwrap(), License::Apache20);

        match License::from_str("custom-license").unwrap() {
            License::Other(s) => assert_eq!(s, "custom-license"),
            _ => panic!("Expected Other variant"),
        }
    }
}
