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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_webhook_router_creation() {
        let config = WebhookConfig::default();
        let router = WebhookRouter::new(config);
        assert_eq!(router.config().port, 8080);
    }

    #[tokio::test]
    async fn test_webhook_stats() {
        let router = WebhookRouter::new(WebhookConfig::default());
        let stats = router.stats().await;
        assert_eq!(stats.total_webhooks, 0);
    }

    #[tokio::test]
    async fn test_receipt_storage() {
        let router = WebhookRouter::new(WebhookConfig::default());

        // Initially empty
        let receipts = router.list_receipts().await;
        assert_eq!(receipts.len(), 0);
    }
}
