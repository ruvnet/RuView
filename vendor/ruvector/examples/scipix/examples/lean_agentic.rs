//! Lean Agentic integration example
//!
//! This example demonstrates distributed OCR processing using agent coordination.
//! Multiple agents work together to process documents in parallel with fault tolerance.
//!
//! Usage:
//! ```bash
//! cargo run --example lean_agentic -- /path/to/documents
//! ```

use anyhow::{Context, Result};
use ruvector_scipix::{OcrConfig, OcrEngine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OcrTask {
    id: String,
    file_path: String,
    priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OcrTaskResult {
    task_id: String,
    agent_id: String,
    success: bool,
    text: Option<String>,
    latex: Option<String>,
    confidence: Option<f32>,
    processing_time_ms: u64,
    error: Option<String>,
}

#[derive(Debug, Clone)]
struct OcrAgent {
    id: String,
    engine: Arc<OcrEngine>,
    tasks_completed: Arc<RwLock<usize>>,
}

impl OcrAgent {
    async fn new(id: String, config: OcrConfig) -> Result<Self> {
        let engine = OcrEngine::new(config).await?;

        Ok(Self {
            id,
            engine: Arc::new(engine),
            tasks_completed: Arc::new(RwLock::new(0)),
        })
    }

    async fn process_task(&self, task: OcrTask) -> OcrTaskResult {
        let start = std::time::Instant::now();

        println!("[Agent {}] Processing task: {}", self.id, task.id);

        let result = match image::open(&task.file_path) {
            Ok(img) => match self.engine.recognize(&img).await {
                Ok(ocr_result) => {
                    let mut count = self.tasks_completed.write().await;
                    *count += 1;

                    OcrTaskResult {
                        task_id: task.id,
                        agent_id: self.id.clone(),
                        success: true,
                        text: Some(ocr_result.text.clone()),
                        latex: ocr_result
                            .to_format(ruvector_scipix::OutputFormat::LaTeX)
                            .ok(),
                        confidence: Some(ocr_result.confidence),
                        processing_time_ms: start.elapsed().as_millis() as u64,
                        error: None,
                    }
                }
                Err(e) => OcrTaskResult {
                    task_id: task.id,
                    agent_id: self.id.clone(),
                    success: false,
                    text: None,
                    latex: None,
                    confidence: None,
                    processing_time_ms: start.elapsed().as_millis() as u64,
                    error: Some(e.to_string()),
                },
            },
            Err(e) => OcrTaskResult {
                task_id: task.id,
                agent_id: self.id.clone(),
                success: false,
                text: None,
                latex: None,
                confidence: None,
                processing_time_ms: start.elapsed().as_millis() as u64,
                error: Some(e.to_string()),
            },
        };

        println!(
            "[Agent {}] Completed task: {} ({}ms)",
            self.id, result.task_id, result.processing_time_ms
        );

        result
    }

    async fn get_stats(&self) -> usize {
        *self.tasks_completed.read().await
    }
}

struct AgentCoordinator {
    agents: Vec<Arc<OcrAgent>>,
    task_queue: mpsc::Sender<OcrTask>,
    result_queue: mpsc::Receiver<OcrTaskResult>,
    results: Arc<RwLock<HashMap<String, OcrTaskResult>>>,
}

impl AgentCoordinator {
    async fn new(num_agents: usize, config: OcrConfig) -> Result<Self> {
        let mut agents = Vec::new();

        for i in 0..num_agents {
            let agent = OcrAgent::new(format!("agent-{}", i), config.clone()).await?;
            agents.push(Arc::new(agent));
        }

        let (task_tx, task_rx) = mpsc::channel::<OcrTask>(100);
        let (result_tx, result_rx) = mpsc::channel::<OcrTaskResult>(100);

        // Spawn agent workers
        for agent in &agents {
            let agent = Arc::clone(agent);
            let mut task_rx = task_rx.resubscribe();
            let result_tx = result_tx.clone();

            tokio::spawn(async move {
                while let Some(task) = task_rx.recv().await {
                    let result = agent.process_task(task).await;
                    let _ = result_tx.send(result).await;
                }
            });
        }

        Ok(Self {
            agents,
            task_queue: task_tx,
            result_queue: result_rx,
            results: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    async fn submit_task(&self, task: OcrTask) -> Result<()> {
        self.task_queue
            .send(task)
            .await
            .context("Failed to submit task")?;
        Ok(())
    }

    async fn collect_results(&mut self, expected: usize) -> Vec<OcrTaskResult> {
        let mut collected = Vec::new();

        while collected.len() < expected {
            if let Some(result) = self.result_queue.recv().await {
                let mut results = self.results.write().await;
                results.insert(result.task_id.clone(), result.clone());
                collected.push(result);
            }
        }

        collected
    }

    async fn get_agent_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();

        for agent in &self.agents {
            let count = agent.get_stats().await;
            stats.insert(agent.id.clone(), count);
        }

        stats
    }
}

// Note: This is a simplified implementation. In production, you would integrate with
// an actual agent framework like:
// - lean_agentic crate for agent coordination
// - tokio actors for distributed processing
// - Or a custom agent framework

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <documents_directory>", args[0]);
        eprintln!("\nExample:");
        eprintln!("  {} ./documents", args[0]);
        std::process::exit(1);
    }

    let docs_dir = Path::new(&args[1]);

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("🤖 Initializing Agent Swarm...");

    // Create agent coordinator with 4 agents
    let num_agents = 4;
    let config = OcrConfig::default();
    let mut coordinator = AgentCoordinator::new(num_agents, config).await?;

    println!("✅ Spawned {} OCR agents", num_agents);

    // Collect tasks
    let mut tasks = Vec::new();
    for entry in std::fs::read_dir(docs_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_str().unwrap_or("").to_lowercase();
                if ["png", "jpg", "jpeg", "bmp", "tiff", "webp"].contains(&ext_str.as_str()) {
                    let task = OcrTask {
                        id: format!("task-{}", tasks.len()),
                        file_path: path.to_string_lossy().to_string(),
                        priority: 1,
                    };
                    tasks.push(task);
                }
            }
        }
    }

    if tasks.is_empty() {
        eprintln!("No image files found in: {}", docs_dir.display());
        std::process::exit(1);
    }

    println!("📋 Queued {} tasks for processing", tasks.len());

    // Submit all tasks
    for task in &tasks {
        coordinator.submit_task(task.clone()).await?;
    }

    println!("🚀 Processing started...\n");

    let start_time = std::time::Instant::now();

    // Collect results
    let results = coordinator.collect_results(tasks.len()).await;

    let total_time = start_time.elapsed();

    // Calculate statistics
    let successful = results.iter().filter(|r| r.success).count();
    let failed = results.len() - successful;
    let avg_confidence =
        results.iter().filter_map(|r| r.confidence).sum::<f32>() / successful.max(1) as f32;
    let avg_time = results.iter().map(|r| r.processing_time_ms).sum::<u64>() / results.len() as u64;

    // Display results
    println!("\n{}", "=".repeat(80));
    println!("Agent Swarm Results");
    println!("{}", "=".repeat(80));
    println!("Total Tasks: {}", results.len());
    println!(
        "Successful: {} ({:.1}%)",
        successful,
        (successful as f32 / results.len() as f32) * 100.0
    );
    println!("Failed: {}", failed);
    println!("Average Confidence: {:.2}%", avg_confidence * 100.0);
    println!("Average Processing Time: {}ms", avg_time);
    println!("Total Time: {:.2}s", total_time.as_secs_f32());
    println!(
        "Throughput: {:.2} tasks/sec",
        results.len() as f32 / total_time.as_secs_f32()
    );

    // Agent statistics
    println!("\n📊 Agent Statistics:");
    let agent_stats = coordinator.get_agent_stats().await;
    for (agent_id, count) in agent_stats {
        println!("  {}: {} tasks", agent_id, count);
    }

    println!("{}", "=".repeat(80));

    // Save results
    let json = serde_json::to_string_pretty(&results)?;
    std::fs::write("agent_results.json", json)?;
    println!("\nResults saved to: agent_results.json");

    Ok(())
}
