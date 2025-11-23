//! Proof job queue for background proof generation
//!
//! This module provides a priority queue for managing proof generation jobs,
//! allowing proofs to be generated asynchronously in the background.

use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::backend::{ProofBackend, ProofError, ProofRequest, ProofResponse};

/// Unique identifier for a proof job
pub type JobId = String;

/// Status of a proof job in the queue
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofJobStatus {
    /// Job is queued and waiting to be processed
    Queued,
    /// Job is currently being processed
    Processing,
    /// Job completed successfully
    Completed,
    /// Job failed with error
    Failed { error: String },
}

/// A proof generation job
#[derive(Debug, Clone)]
pub struct ProofJob {
    /// Unique job identifier
    pub id: JobId,
    /// The proof request
    pub request: ProofRequest,
    /// Current status
    pub status: ProofJobStatus,
    /// When the job was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When the job was completed (if completed)
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// The proof response (if completed)
    pub response: Option<ProofResponse>,
}

impl ProofJob {
    /// Create a new proof job
    pub fn new(id: JobId, request: ProofRequest) -> Self {
        Self {
            id,
            request,
            status: ProofJobStatus::Queued,
            created_at: chrono::Utc::now(),
            completed_at: None,
            response: None,
        }
    }
}

/// Internal job wrapper for priority queue
#[derive(Debug, Clone)]
struct PriorityJob {
    job: ProofJob,
    priority: u8,
}

impl PartialEq for PriorityJob {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for PriorityJob {}

impl PartialOrd for PriorityJob {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityJob {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher priority comes first
        self.priority.cmp(&other.priority)
    }
}

/// Message for queue operations
enum QueueMessage {
    /// Submit a new job
    Submit(ProofJob),
    /// Get job status
    GetStatus(JobId, tokio::sync::oneshot::Sender<Option<ProofJob>>),
    /// Shutdown the queue
    Shutdown,
}

/// Background proof job queue
pub struct ProofJobQueue {
    /// Channel for submitting jobs
    tx: mpsc::UnboundedSender<QueueMessage>,
    /// Worker task handle
    worker: Option<JoinHandle<()>>,
}

impl ProofJobQueue {
    /// Create a new proof job queue
    pub fn new<B: ProofBackend + 'static>(backend: Arc<B>, max_concurrent: usize) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        let worker = tokio::spawn(Self::run_queue(backend, rx, max_concurrent));

        Self {
            tx,
            worker: Some(worker),
        }
    }

    /// Submit a proof job to the queue
    pub async fn submit(&self, job: ProofJob) -> Result<(), ProofError> {
        self.tx
            .send(QueueMessage::Submit(job))
            .map_err(|_| ProofError::GenerationFailed {
                reason: "Queue channel closed".to_string(),
            })?;
        Ok(())
    }

    /// Get the status of a job
    pub async fn get_status(&self, job_id: JobId) -> Option<ProofJob> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(QueueMessage::GetStatus(job_id, tx)).ok()?;
        rx.await.ok()?
    }

    /// Shutdown the queue gracefully
    pub async fn shutdown(mut self) {
        let _ = self.tx.send(QueueMessage::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.await;
        }
    }

    /// Main queue processing loop
    async fn run_queue<B: ProofBackend + 'static>(
        backend: Arc<B>,
        mut rx: mpsc::UnboundedReceiver<QueueMessage>,
        max_concurrent: usize,
    ) {
        let mut queue: BinaryHeap<PriorityJob> = BinaryHeap::new();
        let jobs: Arc<RwLock<HashMap<JobId, ProofJob>>> = Arc::new(RwLock::new(HashMap::new()));
        let mut active_workers = 0;
        let (worker_done_tx, mut worker_done_rx) = mpsc::unbounded_channel();

        info!("Proof job queue started (max_concurrent: {})", max_concurrent);

        loop {
            tokio::select! {
                // Handle incoming messages
                msg = rx.recv() => {
                    match msg {
                        Some(QueueMessage::Submit(job)) => {
                            debug!("Queuing proof job: {}", job.id);
                            let priority = job.request.priority;
                            let id = job.id.clone();
                            queue.push(PriorityJob {
                                job: job.clone(),
                                priority,
                            });
                            jobs.write().await.insert(id, job);
                        }
                        Some(QueueMessage::GetStatus(job_id, resp_tx)) => {
                            let _ = resp_tx.send(jobs.read().await.get(&job_id).cloned());
                        }
                        Some(QueueMessage::Shutdown) => {
                            info!("Shutting down proof job queue");
                            break;
                        }
                        None => {
                            warn!("Queue channel closed");
                            break;
                        }
                    }
                }

                // Handle worker completion
                Some(_) = worker_done_rx.recv() => {
                    active_workers -= 1;
                    debug!("Worker completed, active workers: {}", active_workers);
                }

                // Process jobs from the queue
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)), if !queue.is_empty() && active_workers < max_concurrent => {
                    if let Some(priority_job) = queue.pop() {
                        let mut job = priority_job.job;
                        job.status = ProofJobStatus::Processing;
                        let job_id = job.id.clone();

                        // Update job status
                        {
                            let mut jobs_write = jobs.write().await;
                            if let Some(stored_job) = jobs_write.get_mut(&job_id) {
                                stored_job.status = ProofJobStatus::Processing;
                            }
                        }

                        // Spawn worker
                        let backend_clone = backend.clone();
                        let jobs_clone = jobs.clone();
                        let done_tx = worker_done_tx.clone();

                        active_workers += 1;
                        tokio::spawn(async move {
                            let result = backend_clone.generate_proof(job.request.clone()).await;

                            let mut jobs = jobs_clone.write().await;
                            if let Some(stored_job) = jobs.get_mut(&job_id) {
                                match result {
                                    Ok(response) => {
                                        stored_job.status = ProofJobStatus::Completed;
                                        stored_job.response = Some(response);
                                        stored_job.completed_at = Some(chrono::Utc::now());
                                        info!("Proof job completed: {}", job_id);
                                    }
                                    Err(e) => {
                                        stored_job.status = ProofJobStatus::Failed {
                                            error: e.to_string(),
                                        };
                                        stored_job.completed_at = Some(chrono::Utc::now());
                                        error!("Proof job failed: {} - {}", job_id, e);
                                    }
                                }
                            }

                            // Signal completion
                            let _ = done_tx.send(());
                        });
                    }
                }
            }
        }

        info!("Proof job queue stopped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockProofBackend;
    use tenzik_runtime::ExecutionReceipt;

    fn create_test_receipt() -> ExecutionReceipt {
        use tenzik_runtime::ExecMetrics;

        ExecutionReceipt {
            capsule_id: "test_capsule_hash".to_string(),
            input_commit: "input_hash".to_string(),
            output_commit: "output_hash".to_string(),
            exec_metrics: ExecMetrics {
                fuel_used: 1000,
                memory_mb: 1.0,
                duration_ms: 100,
                host_function_calls: 0,
            },
            node_id: "test_node".to_string(),
            nonce: 1,
            signature: "test_sig".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: "1.0".to_string(),
            zk_proof: None,
        }
    }

    #[tokio::test]
    async fn test_job_queue_creation() {
        let mut backend = MockProofBackend::new();
        backend.initialize().await.unwrap();

        let queue = ProofJobQueue::new(Arc::new(backend), 2);
        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_job_submission() {
        let mut backend = MockProofBackend::with_delays(10, 5);
        backend.initialize().await.unwrap();

        let queue = ProofJobQueue::new(Arc::new(backend), 2);

        let receipt = create_test_receipt();
        let request = ProofRequest {
            receipt,
            timeout_ms: None,
            priority: 5,
        };

        let job = ProofJob::new("job_1".to_string(), request);
        queue.submit(job).await.unwrap();

        // Give it time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let status = queue.get_status("job_1".to_string()).await;
        assert!(status.is_some());

        queue.shutdown().await;
    }
}
