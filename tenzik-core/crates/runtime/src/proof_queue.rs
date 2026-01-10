//! Background Proof Job Queue
//!
//! This module provides a background job queue for generating ZK proofs asynchronously.
//! Proof generation can be computationally expensive, so this allows executions to complete
//! immediately while proofs are generated in the background.

use crate::proofs::{ProofBackend, ProofError, ZkProof};
use crate::receipts::ExecutionReceipt;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use std::collections::HashMap;
use thiserror::Error;

/// Proof job errors
#[derive(Error, Debug)]
pub enum ProofJobError {
    #[error("Job queue full (capacity: {capacity})")]
    QueueFull { capacity: usize },

    #[error("Job not found: {job_id}")]
    JobNotFound { job_id: String },

    #[error("Proof generation failed: {source}")]
    ProofGenerationFailed { source: ProofError },

    #[error("Queue shutdown")]
    QueueShutdown,
}

/// Status of a proof job
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    /// Job is queued but not yet started
    Queued,
    /// Job is currently being processed
    Processing,
    /// Job completed successfully
    Completed,
    /// Job failed with an error
    Failed,
}

/// A proof generation job
#[derive(Debug, Clone)]
pub struct ProofJob {
    /// Unique job identifier
    pub job_id: String,
    /// The receipt to generate a proof for
    pub receipt: ExecutionReceipt,
    /// Current status of the job
    pub status: JobStatus,
    /// The generated proof (if completed)
    pub proof: Option<ZkProof>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Timestamp when job was created (ISO 8601)
    pub created_at: String,
    /// Timestamp when job was completed (ISO 8601)
    pub completed_at: Option<String>,
}

impl ProofJob {
    /// Create a new proof job
    pub fn new(job_id: String, receipt: ExecutionReceipt) -> Self {
        Self {
            job_id,
            receipt,
            status: JobStatus::Queued,
            proof: None,
            error: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
        }
    }

    /// Mark job as processing
    pub fn mark_processing(&mut self) {
        self.status = JobStatus::Processing;
    }

    /// Mark job as completed
    pub fn mark_completed(&mut self, proof: ZkProof) {
        self.status = JobStatus::Completed;
        self.proof = Some(proof);
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark job as failed
    pub fn mark_failed(&mut self, error: String) {
        self.status = JobStatus::Failed;
        self.error = Some(error);
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }
}

/// Background proof job queue
pub struct ProofJobQueue {
    /// Job storage (job_id -> ProofJob)
    jobs: Arc<RwLock<HashMap<String, ProofJob>>>,
    /// Channel for submitting new jobs
    job_tx: mpsc::Sender<ProofJob>,
    /// Maximum queue capacity
    capacity: usize,
    /// Number of worker threads
    worker_count: usize,
}

impl ProofJobQueue {
    /// Create a new proof job queue
    ///
    /// # Arguments
    /// * `backend` - The proof backend to use for generating proofs
    /// * `capacity` - Maximum number of pending jobs
    /// * `worker_count` - Number of worker threads for proof generation
    pub fn new(
        backend: Arc<dyn ProofBackend>,
        capacity: usize,
        worker_count: usize,
    ) -> Self {
        let (job_tx, job_rx) = mpsc::channel(capacity);
        let jobs = Arc::new(RwLock::new(HashMap::new()));

        // Spawn worker tasks
        // We'll use a simple round-robin approach where all workers share the same receiver
        let job_rx = Arc::new(tokio::sync::Mutex::new(job_rx));

        for worker_id in 0..worker_count {
            let backend = Arc::clone(&backend);
            let jobs = Arc::clone(&jobs);
            let job_rx = Arc::clone(&job_rx);

            tokio::spawn(async move {
                Self::worker_loop(worker_id, backend, jobs, job_rx).await;
            });
        }

        Self {
            jobs,
            job_tx,
            capacity,
            worker_count,
        }
    }

    /// Submit a new proof generation job
    ///
    /// Returns the job ID for tracking
    pub async fn submit_job(&self, receipt: ExecutionReceipt) -> Result<String, ProofJobError> {
        // Generate unique job ID
        let job_id = format!(
            "job_{}_{}",
            chrono::Utc::now().timestamp_millis(),
            uuid::Uuid::new_v4().simple()
        );

        let job = ProofJob::new(job_id.clone(), receipt);

        // Store job
        {
            let mut jobs = self.jobs.write().await;
            jobs.insert(job_id.clone(), job.clone());
        }

        // Submit to queue
        self.job_tx
            .send(job)
            .await
            .map_err(|_| ProofJobError::QueueShutdown)?;

        Ok(job_id)
    }

    /// Get the status of a proof job
    pub async fn get_job_status(&self, job_id: &str) -> Result<JobStatus, ProofJobError> {
        let jobs = self.jobs.read().await;
        jobs.get(job_id)
            .map(|job| job.status.clone())
            .ok_or_else(|| ProofJobError::JobNotFound {
                job_id: job_id.to_string(),
            })
    }

    /// Get a completed proof
    pub async fn get_proof(&self, job_id: &str) -> Result<Option<ZkProof>, ProofJobError> {
        let jobs = self.jobs.read().await;
        let job = jobs.get(job_id).ok_or_else(|| ProofJobError::JobNotFound {
            job_id: job_id.to_string(),
        })?;

        Ok(job.proof.clone())
    }

    /// Get complete job information
    pub async fn get_job(&self, job_id: &str) -> Result<ProofJob, ProofJobError> {
        let jobs = self.jobs.read().await;
        jobs.get(job_id)
            .cloned()
            .ok_or_else(|| ProofJobError::JobNotFound {
                job_id: job_id.to_string(),
            })
    }

    /// List all jobs (returns job IDs and their statuses)
    pub async fn list_jobs(&self) -> Vec<(String, JobStatus)> {
        let jobs = self.jobs.read().await;
        jobs.iter()
            .map(|(id, job)| (id.clone(), job.status.clone()))
            .collect()
    }

    /// Remove old completed/failed jobs
    ///
    /// Removes jobs older than the specified age in seconds
    pub async fn cleanup_old_jobs(&self, max_age_seconds: u64) -> usize {
        let mut jobs = self.jobs.write().await;
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(max_age_seconds as i64);

        let to_remove: Vec<String> = jobs
            .iter()
            .filter(|(_, job)| {
                if job.status == JobStatus::Queued || job.status == JobStatus::Processing {
                    return false;
                }

                if let Some(completed_at) = &job.completed_at {
                    if let Ok(completed_time) = chrono::DateTime::parse_from_rfc3339(completed_at) {
                        return completed_time.with_timezone(&chrono::Utc) < cutoff;
                    }
                }

                false
            })
            .map(|(id, _)| id.clone())
            .collect();

        let count = to_remove.len();
        for id in to_remove {
            jobs.remove(&id);
        }

        count
    }

    /// Worker loop for processing proof jobs
    async fn worker_loop(
        worker_id: usize,
        backend: Arc<dyn ProofBackend>,
        jobs: Arc<RwLock<HashMap<String, ProofJob>>>,
        job_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<ProofJob>>>,
    ) {
        tracing::info!("Proof worker {} started", worker_id);

        loop {
            let mut job = {
                let mut rx = job_rx.lock().await;
                match rx.recv().await {
                    Some(job) => job,
                    None => break, // Channel closed
                }
            };
            tracing::debug!("Worker {} processing job {}", worker_id, job.job_id);

            // Mark as processing
            job.mark_processing();
            {
                let mut jobs_map = jobs.write().await;
                jobs_map.insert(job.job_id.clone(), job.clone());
            }

            // Generate proof
            match backend.generate_proof(&job.receipt).await {
                Ok(proof) => {
                    tracing::info!("Worker {} completed job {}", worker_id, job.job_id);
                    job.mark_completed(proof);
                }
                Err(e) => {
                    tracing::error!("Worker {} failed job {}: {}", worker_id, job.job_id, e);
                    job.mark_failed(e.to_string());
                }
            }

            // Update job status
            {
                let mut jobs_map = jobs.write().await;
                jobs_map.insert(job.job_id.clone(), job);
            }
        }

        tracing::info!("Proof worker {} shutting down", worker_id);
    }

    /// Get queue statistics
    pub async fn get_stats(&self) -> QueueStats {
        let jobs = self.jobs.read().await;

        let mut queued = 0;
        let mut processing = 0;
        let mut completed = 0;
        let mut failed = 0;

        for job in jobs.values() {
            match job.status {
                JobStatus::Queued => queued += 1,
                JobStatus::Processing => processing += 1,
                JobStatus::Completed => completed += 1,
                JobStatus::Failed => failed += 1,
            }
        }

        QueueStats {
            total_jobs: jobs.len(),
            queued,
            processing,
            completed,
            failed,
            capacity: self.capacity,
            worker_count: self.worker_count,
        }
    }
}

/// Queue statistics
#[derive(Debug, Clone)]
pub struct QueueStats {
    pub total_jobs: usize,
    pub queued: usize,
    pub processing: usize,
    pub completed: usize,
    pub failed: usize,
    pub capacity: usize,
    pub worker_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proofs::MockProofBackend;
    use crate::receipts::{ExecMetrics, generate_test_signing_key};

    fn create_test_receipt() -> ExecutionReceipt {
        let signing_key = generate_test_signing_key();
        ExecutionReceipt::new(
            b"test capsule",
            b"input",
            b"output",
            ExecMetrics::default(),
            &signing_key,
            42,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_queue_creation() {
        let backend = Arc::new(MockProofBackend::new());
        let queue = ProofJobQueue::new(backend, 100, 2);

        let stats = queue.get_stats().await;
        assert_eq!(stats.total_jobs, 0);
        assert_eq!(stats.capacity, 100);
        assert_eq!(stats.worker_count, 2);
    }

    #[tokio::test]
    async fn test_submit_and_complete_job() {
        let backend = Arc::new(MockProofBackend::with_delay(10));
        let queue = ProofJobQueue::new(backend, 100, 2);

        let receipt = create_test_receipt();
        let job_id = queue.submit_job(receipt).await.unwrap();

        // Initially should be queued
        let status = queue.get_job_status(&job_id).await.unwrap();
        assert!(matches!(status, JobStatus::Queued | JobStatus::Processing));

        // Wait for completion
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let status = queue.get_job_status(&job_id).await.unwrap();
        assert_eq!(status, JobStatus::Completed);

        // Get the proof
        let proof = queue.get_proof(&job_id).await.unwrap();
        assert!(proof.is_some());
    }

    #[tokio::test]
    async fn test_multiple_jobs() {
        let backend = Arc::new(MockProofBackend::with_delay(5));
        let queue = ProofJobQueue::new(backend, 100, 2);

        // Submit 5 jobs
        let mut job_ids = Vec::new();
        for _ in 0..5 {
            let receipt = create_test_receipt();
            let job_id = queue.submit_job(receipt).await.unwrap();
            job_ids.push(job_id);
        }

        // Wait for all to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Check all completed
        for job_id in &job_ids {
            let status = queue.get_job_status(job_id).await.unwrap();
            assert_eq!(status, JobStatus::Completed);
        }

        let stats = queue.get_stats().await;
        assert_eq!(stats.completed, 5);
    }

    #[tokio::test]
    async fn test_job_not_found() {
        let backend = Arc::new(MockProofBackend::new());
        let queue = ProofJobQueue::new(backend, 100, 1);

        let result = queue.get_job_status("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cleanup_old_jobs() {
        let backend = Arc::new(MockProofBackend::new());
        let queue = ProofJobQueue::new(backend, 100, 1);

        // Submit and complete a job
        let receipt = create_test_receipt();
        let job_id = queue.submit_job(receipt).await.unwrap();

        // Wait for completion
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Cleanup (with 0 age - should remove all completed)
        let removed = queue.cleanup_old_jobs(0).await;
        assert!(removed > 0);

        // Job should be gone
        let result = queue.get_job_status(&job_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_jobs() {
        let backend = Arc::new(MockProofBackend::with_delay(5));
        let queue = ProofJobQueue::new(backend, 100, 1);

        // Submit 3 jobs
        for _ in 0..3 {
            let receipt = create_test_receipt();
            queue.submit_job(receipt).await.unwrap();
        }

        let jobs = queue.list_jobs().await;
        assert_eq!(jobs.len(), 3);
    }
}
