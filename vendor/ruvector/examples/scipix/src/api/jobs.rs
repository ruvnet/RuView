use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use super::requests::PdfRequest;

/// Job status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Job is queued but not started
    Queued,
    /// Job is currently processing
    Processing,
    /// Job completed successfully
    Completed,
    /// Job failed with error
    Failed,
    /// Job was cancelled
    Cancelled,
}

/// PDF processing job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfJob {
    /// Unique job identifier
    pub id: String,

    /// Original request
    pub request: PdfRequest,

    /// Current status
    pub status: JobStatus,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Processing result
    pub result: Option<String>,

    /// Error message (if failed)
    pub error: Option<String>,
}

impl PdfJob {
    /// Create a new PDF job
    pub fn new(request: PdfRequest) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            request,
            status: JobStatus::Queued,
            created_at: now,
            updated_at: now,
            result: None,
            error: None,
        }
    }

    /// Update job status
    pub fn update_status(&mut self, status: JobStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }

    /// Set job result
    pub fn set_result(&mut self, result: String) {
        self.result = Some(result);
        self.status = JobStatus::Completed;
        self.updated_at = Utc::now();
    }

    /// Set job error
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.status = JobStatus::Failed;
        self.updated_at = Utc::now();
    }
}

/// Async job queue with webhook support
pub struct JobQueue {
    /// Job storage
    jobs: Arc<RwLock<HashMap<String, PdfJob>>>,

    /// Job submission channel
    tx: mpsc::Sender<PdfJob>,

    /// Job processing handle
    _handle: Option<tokio::task::JoinHandle<()>>,
}

impl JobQueue {
    /// Create a new job queue
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }

    /// Create a job queue with specific capacity
    pub fn with_capacity(capacity: usize) -> Self {
        let jobs = Arc::new(RwLock::new(HashMap::new()));
        let (tx, rx) = mpsc::channel(capacity);

        let queue_jobs = jobs.clone();
        let handle = tokio::spawn(async move {
            Self::process_jobs(queue_jobs, rx).await;
        });

        Self {
            jobs,
            tx,
            _handle: Some(handle),
        }
    }

    /// Enqueue a new job
    pub async fn enqueue(&self, mut job: PdfJob) -> anyhow::Result<()> {
        job.update_status(JobStatus::Queued);

        // Store job
        {
            let mut jobs = self.jobs.write().await;
            jobs.insert(job.id.clone(), job.clone());
        }

        // Send to processing queue
        self.tx.send(job).await?;

        Ok(())
    }

    /// Get job status
    pub async fn get_status(&self, id: &str) -> Option<JobStatus> {
        let jobs = self.jobs.read().await;
        jobs.get(id).map(|job| job.status.clone())
    }

    /// Get job result
    pub async fn get_result(&self, id: &str) -> Option<String> {
        let jobs = self.jobs.read().await;
        jobs.get(id).and_then(|job| job.result.clone())
    }

    /// Get job error
    pub async fn get_error(&self, id: &str) -> Option<String> {
        let jobs = self.jobs.read().await;
        jobs.get(id).and_then(|job| job.error.clone())
    }

    /// Cancel a job
    pub async fn cancel(&self, id: &str) -> anyhow::Result<()> {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.get_mut(id) {
            job.update_status(JobStatus::Cancelled);
            Ok(())
        } else {
            anyhow::bail!("Job not found")
        }
    }

    /// Background job processor
    async fn process_jobs(
        jobs: Arc<RwLock<HashMap<String, PdfJob>>>,
        mut rx: mpsc::Receiver<PdfJob>,
    ) {
        while let Some(job) = rx.recv().await {
            let job_id = job.id.clone();

            // Update status to processing
            {
                let mut jobs_lock = jobs.write().await;
                if let Some(stored_job) = jobs_lock.get_mut(&job_id) {
                    stored_job.update_status(JobStatus::Processing);
                }
            }

            // Simulate PDF processing
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            // Update with result
            {
                let mut jobs_lock = jobs.write().await;
                if let Some(stored_job) = jobs_lock.get_mut(&job_id) {
                    stored_job.set_result("Processed PDF content".to_string());

                    // Send webhook if specified
                    if let Some(webhook_url) = &stored_job.request.webhook_url {
                        Self::send_webhook(webhook_url, stored_job).await;
                    }
                }
            }
        }
    }

    /// Send webhook notification
    async fn send_webhook(url: &str, job: &PdfJob) {
        let client = reqwest::Client::new();
        let payload = serde_json::json!({
            "job_id": job.id,
            "status": job.status,
            "result": job.result,
            "error": job.error,
        });

        if let Err(e) = client.post(url).json(&payload).send().await {
            tracing::error!("Failed to send webhook to {}: {:?}", url, e);
        }
    }
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::requests::{PdfOptions, RequestMetadata};

    #[tokio::test]
    async fn test_job_creation() {
        let request = PdfRequest {
            url: "https://example.com/test.pdf".to_string(),
            options: PdfOptions::default(),
            webhook_url: None,
            metadata: RequestMetadata::default(),
        };

        let job = PdfJob::new(request);
        assert_eq!(job.status, JobStatus::Queued);
        assert!(job.result.is_none());
        assert!(job.error.is_none());
    }

    #[tokio::test]
    async fn test_job_queue_enqueue() {
        let queue = JobQueue::new();
        let request = PdfRequest {
            url: "https://example.com/test.pdf".to_string(),
            options: PdfOptions::default(),
            webhook_url: None,
            metadata: RequestMetadata::default(),
        };

        let job = PdfJob::new(request);
        let job_id = job.id.clone();

        queue.enqueue(job).await.unwrap();

        let status = queue.get_status(&job_id).await;
        assert!(status.is_some());
    }

    #[tokio::test]
    async fn test_job_cancellation() {
        let queue = JobQueue::new();
        let request = PdfRequest {
            url: "https://example.com/test.pdf".to_string(),
            options: PdfOptions::default(),
            webhook_url: None,
            metadata: RequestMetadata::default(),
        };

        let job = PdfJob::new(request);
        let job_id = job.id.clone();

        queue.enqueue(job).await.unwrap();
        queue.cancel(&job_id).await.unwrap();

        let status = queue.get_status(&job_id).await;
        assert_eq!(status, Some(JobStatus::Cancelled));
    }
}
