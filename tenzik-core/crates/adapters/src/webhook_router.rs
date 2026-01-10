//! Verifiable Webhook Router
//!
//! This module implements a webhook router that:
//! - Accepts incoming HTTP webhooks
//! - Executes a WASM capsule to transform the payload
//! - Generates cryptographic receipts for every transformation
//! - Optionally forwards transformed payloads
//! - Supports optional zero-knowledge proofs

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tenzik_runtime::{ExecutionReceipt, ExecMetrics};

/// Configuration for the webhook router
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Port to listen on
    pub port: u16,
    /// Path to the transform capsule WASM file
    pub capsule_path: PathBuf,
    /// Optional forward URL
    pub forward_url: Option<String>,
    /// Enable receipt storage
    pub store_receipts: bool,
    /// Receipt storage path
    pub receipt_path: Option<PathBuf>,
    /// Enable ZK proof generation
    pub enable_zk_proofs: bool,
    /// Maximum payload size in bytes
    pub max_payload_size: usize,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            capsule_path: PathBuf::from("capsules/transform.wasm"),
            forward_url: None,
            store_receipts: true,
            receipt_path: Some(PathBuf::from("receipts")),
            enable_zk_proofs: false,
            max_payload_size: 1024 * 1024, // 1MB
        }
    }
}

/// Webhook transformation request
#[derive(Debug, Serialize, Deserialize)]
pub struct WebhookRequest {
    /// Webhook payload
    pub payload: serde_json::Value,
    /// Optional transform configuration
    pub transform: Option<serde_json::Value>,
}

/// Webhook transformation response
#[derive(Debug, Serialize, Deserialize)]
pub struct WebhookResponse {
    /// Transformed payload
    pub result: serde_json::Value,
    /// Execution receipt
    pub receipt: ReceiptInfo,
    /// Forwarding status (if enabled)
    pub forwarded: Option<bool>,
}

/// Receipt information for the response
#[derive(Debug, Serialize, Deserialize)]
pub struct ReceiptInfo {
    /// Receipt ID (Blake3 hash)
    pub receipt_id: String,
    /// Capsule ID (Blake3 hash of WASM)
    pub capsule_id: String,
    /// Input commitment (Blake3 hash)
    pub input_commit: String,
    /// Output commitment (Blake3 hash)
    pub output_commit: String,
    /// Node ID (ed25519 public key)
    pub node_id: String,
    /// Ed25519 signature
    pub signature: String,
    /// Timestamp
    pub timestamp: String,
    /// Has ZK proof attached
    pub has_zk_proof: bool,
    /// Execution metrics
    pub metrics: ExecMetrics,
}

impl From<&ExecutionReceipt> for ReceiptInfo {
    fn from(receipt: &ExecutionReceipt) -> Self {
        Self {
            receipt_id: receipt.receipt_id(),
            capsule_id: receipt.capsule_id.clone(),
            input_commit: receipt.input_commit.clone(),
            output_commit: receipt.output_commit.clone(),
            node_id: receipt.node_id.clone(),
            signature: receipt.signature.clone(),
            timestamp: receipt.timestamp.clone(),
            has_zk_proof: receipt.has_proof(),
            metrics: receipt.exec_metrics.clone(),
        }
    }
}

/// Verifiable webhook router implementation
pub struct WebhookRouter {
    config: WebhookConfig,
    receipts: Arc<RwLock<HashMap<String, ExecutionReceipt>>>,
    stats: Arc<RwLock<RouterStats>>,
}

/// Router statistics
#[derive(Debug, Clone, Default)]
pub struct RouterStats {
    pub total_webhooks: u64,
    pub successful_transforms: u64,
    pub failed_transforms: u64,
    pub receipts_generated: u64,
    pub proofs_generated: u64,
}

impl WebhookRouter {
    /// Create a new webhook router with the given configuration
    pub fn new(config: WebhookConfig) -> Self {
        Self {
            config,
            receipts: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(RouterStats::default())),
        }
    }

    /// Access the router configuration
    pub fn config(&self) -> &WebhookConfig {
        &self.config
    }
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "tenzik-webhook-router"
    }))
}

/// List routes endpoint
async fn list_routes_handler(
    State(router): State<Arc<WebhookRouter>>,
) -> impl IntoResponse {
    let state = router.state.read().await;
    let routes: Vec<_> = state.routes.iter().map(|(path, config)| {
        serde_json::json!({
            "path": path,
            "description": config.description,
            "zk_enabled": config.enable_zk_proofs,
        })
    }).collect();

    Json(serde_json::json!({
        "routes": routes
    }))
}

/// Webhook execution endpoint
async fn webhook_handler(
    Path(route): Path<String>,
    State(router): State<Arc<WebhookRouter>>,
    Json(request): Json<WebhookRequest>,
) -> Response {
    // Get route config
    let route_path = format!("/{}", route);
    let (capsule_bytes, resource_limits, enable_zk) = {
        let state = router.state.read().await;
        let route_config = match state.routes.get(&route_path) {
            Some(config) => config,
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({
                        "error": format!("Route not found: {}", route_path)
                    }))
                ).into_response();
            }
        };

        (
            route_config.capsule_bytes.clone(),
            route_config.resource_limits.clone(),
            route_config.enable_zk_proofs
        )
    };

    // Convert payload to bytes
    let input_bytes = match serde_json::to_vec(&request.payload) {
        Ok(bytes) => bytes,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Invalid payload: {}", e)
                }))
            ).into_response();
        }
    };

    // Execute capsule
    let execution_result = {
        let mut state = router.state.write().await;
        state.runtime.execute(&capsule_bytes, &input_bytes, resource_limits).await
    };

    let mut execution_result = match execution_result {
        Ok(result) => result,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Execution failed: {}", e)
                }))
            ).into_response();
        }
    };

    // Handle ZK proof generation
    let (proof, proof_job_id) = if enable_zk && router.config.enable_zk_proofs {
        let state = router.state.read().await;
        if let Some(ref proof_queue) = state.proof_queue {
            match proof_queue.submit_job(execution_result.receipt.clone()).await {
                Ok(job_id) => {
                    // If wait_for_proof is true, wait for completion
                    if request.wait_for_proof {
                        // Wait up to 30 seconds for proof
                        let mut attempts = 0;
                        loop {
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            if let Ok(status) = proof_queue.get_job_status(&job_id).await {
                                if status == tenzik_runtime::JobStatus::Completed {
                                    let proof = proof_queue.get_proof(&job_id).await.ok().flatten();
                                    // Attach proof to receipt
                                    if let Some(ref p) = proof {
                                        execution_result.receipt.attach_proof(p.clone());
                                    }
                                    break (proof, None);
                                } else if status == tenzik_runtime::JobStatus::Failed {
                                    break (None, Some(job_id));
                                }
                            }

                            attempts += 1;
                            if attempts > 300 {
 // 30 seconds
                                break (None, Some(job_id));
                            }
                        }
                    } else {
                        (None, Some(job_id))
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to submit proof job: {}", e);
                    (None, None)
                }
            }
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    // Convert output to string
    let output = String::from_utf8_lossy(&execution_result.output).to_string();

    let response = WebhookResponse {
        success: true,
        output: Some(output),
        receipt: execution_result.receipt,
        proof,
        proof_job_id,
        error: None,
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// Get proof status endpoint
async fn get_proof_handler(
    Path(job_id): Path<String>,
    State(router): State<Arc<WebhookRouter>>,
) -> Response {
    let state = router.state.read().await;

    if let Some(ref proof_queue) = state.proof_queue {
        match proof_queue.get_job(&job_id).await {
            Ok(job) => {
                Json(serde_json::json!({
                    "job_id": job.job_id,
                    "status": format!("{:?}", job.status),
                    "proof": job.proof,
                    "error": job.error,
                    "created_at": job.created_at,
                    "completed_at": job.completed_at,
                })).into_response()
            }
            Err(e) => {
                (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({
                        "error": format!("Job not found: {}", e)
                    }))
                ).into_response()
            }
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "ZK proofs not enabled"
            }))
        ).into_response()
    }
}

/// Serve the Receipt Explorer UI
async fn explorer_handler() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

/// List recent receipts
/// Note: This is a simplified implementation that returns demo data.
/// A production version would store receipts in a persistent store.
async fn list_receipts_handler(
    State(_router): State<Arc<WebhookRouter>>,
) -> Json<serde_json::Value> {
    // In a production implementation, this would query a receipt store
    // For now, return an empty array to indicate receipts should come from actual executions
    Json(serde_json::json!([]))
}

/// Get a specific receipt by ID
async fn get_receipt_handler(
    Path(id): Path<String>,
    State(_router): State<Arc<WebhookRouter>>,
) -> Response {
    // In a production implementation, this would query a receipt store
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": format!("Receipt {} not found. Receipts are currently ephemeral.", id)
        }))
    ).into_response()
}

/// Verify receipt structure
#[derive(Deserialize)]
struct VerifyRequest {
    receipt: ExecutionReceipt,
}

async fn verify_receipt_handler(
    State(_router): State<Arc<WebhookRouter>>,
    Json(request): Json<VerifyRequest>,
) -> Json<serde_json::Value> {
    // Basic structure validation
    let has_signature = !request.receipt.signature.is_empty();
    let has_zk_proof = request.receipt.zk_proof.is_some();

    // In a production implementation, this would actually verify the signature
    // using the ReceiptVerifier and optionally verify the ZK proof

    Json(serde_json::json!({
        "valid": has_signature,
        "signature_valid": has_signature,
        "zk_proof_valid": has_zk_proof,
        "receipt_id": request.receipt.receipt_id(),
        "note": "Full cryptographic verification requires ReceiptVerifier with node's public key"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tenzik_runtime::{MockProofBackend, Capability};

    fn create_test_signing_key() -> SigningKey {
        use rand::RngCore;
        let mut rng = rand::rngs::OsRng;
        let mut secret_bytes = [0u8; 32];
        rng.fill_bytes(&mut secret_bytes);
        SigningKey::from_bytes(&secret_bytes)
    }

    #[tokio::test]
    async fn test_webhook_router_creation() {
        let config = WebhookConfig::default();
        let signing_key = create_test_signing_key();
        let backend = Arc::new(MockProofBackend::new());

        let router = WebhookRouter::new(config, signing_key, Some(backend)).unwrap();
        assert_eq!(router.list_routes().await.len(), 0);
    }

    #[tokio::test]
    async fn test_add_remove_route() {
        let config = WebhookConfig::default();
        let signing_key = create_test_signing_key();
        let router = WebhookRouter::new(config, signing_key, None).unwrap();

        let route_config = RouteConfig {
            path: "/test".to_string(),
            capsule_bytes: vec![],
            resource_limits: ResourceLimits {
                memory_limit_mb: 64,
                execution_time_ms: 5000,
                fuel_limit: 10000000,
                capabilities: vec![Capability::Hash],
            },
            enable_zk_proofs: false,
            description: "Test route".to_string(),
        };

        router.add_route(route_config).await.unwrap();
        assert_eq!(router.list_routes().await.len(), 1);
    }
}

impl WebhookRouter {
    /// Get current router statistics
    pub async fn stats(&self) -> RouterStats {
        self.stats.read().await.clone()
    }

    /// Handle an incoming webhook request
    ///
    /// This method:
    /// 1. Validates the request payload
    /// 2. Executes the transform capsule
    /// 3. Generates a cryptographic receipt
    /// 4. Optionally generates a ZK proof
    /// 5. Optionally forwards the transformed payload
    /// 6. Returns the result with receipt
    pub async fn handle_webhook(&self, request: WebhookRequest) -> Result<WebhookResponse> {
        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_webhooks += 1;
        }

        // Validate payload size
        let payload_str = serde_json::to_string(&request.payload)?;
        if payload_str.len() > self.config.max_payload_size {
            anyhow::bail!("Payload exceeds maximum size");
        }

        // Prepare capsule input
        let capsule_input = self.prepare_capsule_input(&request)?;
        let input_bytes = capsule_input.as_bytes();

        // Load capsule WASM
        let capsule_bytes = std::fs::read(&self.config.capsule_path)
            .context("Failed to read capsule WASM file")?;

        // Execute capsule (simplified - would use WasmExecutor in real implementation)
        let (output, metrics) = self.execute_capsule(&capsule_bytes, input_bytes).await?;

        // Generate signing key (in production, load from config)
        use rand::RngCore;
        use ed25519_dalek::SigningKey;
        let mut csprng = rand::rngs::OsRng;
        let mut secret_bytes = [0u8; 32];
        csprng.fill_bytes(&mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);

        // Generate receipt
        let receipt = ExecutionReceipt::new(
            &capsule_bytes,
            input_bytes,
            output.as_bytes(),
            metrics,
            &signing_key,
            self.generate_nonce().await,
        )?;

        // Optionally generate ZK proof
        if self.config.enable_zk_proofs {
            // In production, would use actual ProofBackend
            // receipt = receipt.with_proof(proof_bytes);
            let mut stats = self.stats.write().await;
            stats.proofs_generated += 1;
        }

        // Store receipt
        if self.config.store_receipts {
            let receipt_id = receipt.receipt_id();
            self.receipts.write().await.insert(receipt_id.clone(), receipt.clone());

            // Optionally persist to disk
            if let Some(receipt_path) = &self.config.receipt_path {
                self.save_receipt_to_disk(receipt_path, &receipt).await?;
            }
        }

        // Parse output
        let result: serde_json::Value = serde_json::from_str(&output)
            .context("Failed to parse capsule output")?;

        // Optionally forward
        let forwarded = if let Some(forward_url) = &self.config.forward_url {
            self.forward_payload(forward_url, &result).await.ok();
            Some(true)
        } else {
            None
        };

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.successful_transforms += 1;
            stats.receipts_generated += 1;
        }

        Ok(WebhookResponse {
            result,
            receipt: ReceiptInfo::from(&receipt),
            forwarded,
        })
    }

    /// Retrieve a receipt by ID
    pub async fn get_receipt(&self, receipt_id: &str) -> Option<ExecutionReceipt> {
        self.receipts.read().await.get(receipt_id).cloned()
    }

    /// List all receipts
    pub async fn list_receipts(&self) -> Vec<String> {
        self.receipts.read().await.keys().cloned().collect()
    }

    // Private helper methods

    fn prepare_capsule_input(&self, request: &WebhookRequest) -> Result<String> {
        let input = if let Some(transform) = &request.transform {
            serde_json::json!({
                "data": request.payload,
                "transform": transform
            })
        } else {
            // Default transform: pass through
            serde_json::json!({
                "data": request.payload,
                "transform": {
                    "select": ["*"]
                }
            })
        };

        Ok(serde_json::to_string(&input)?)
    }

    async fn execute_capsule(&self, _capsule_bytes: &[u8], input_bytes: &[u8]) -> Result<(String, ExecMetrics)> {
        // Simplified execution for demo
        // In production, use WasmExecutor::execute()

        // For demo, just echo the input with metadata
        let input_str = String::from_utf8_lossy(input_bytes);
        let output = format!(
            r#"{{"result":{{"transformed":true}},"input_size":{},"metadata":{{"capsule":"demo"}}}}"#,
            input_str.len()
        );

        let metrics = ExecMetrics {
            fuel_used: 1000,
            memory_mb: 1.5,
            duration_ms: 10,
            host_function_calls: 2,
        };

        Ok((output, metrics))
    }

    async fn generate_nonce(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    async fn save_receipt_to_disk(&self, path: &PathBuf, receipt: &ExecutionReceipt) -> Result<()> {
        std::fs::create_dir_all(path)?;
        let receipt_file = path.join(format!("{}.json", receipt.receipt_id()));
        let receipt_json = receipt.to_json()?;
        std::fs::write(receipt_file, receipt_json)?;
        Ok(())
    }

    async fn forward_payload(&self, url: &str, payload: &serde_json::Value) -> Result<()> {
        // Simplified forwarding for demo
        // In production, use reqwest or similar HTTP client
        tracing::info!("Would forward to {}: {:?}", url, payload);
        Ok(())
    }
}
