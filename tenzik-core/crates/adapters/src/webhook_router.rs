//! Verifiable Webhook Router
//!
//! This module provides a webhook router that executes WASM capsules in response
//! to HTTP requests and returns verifiable execution receipts.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tenzik_runtime::{
    ExecutionReceipt, ExecutionResult, ResourceLimits, WasmRuntime,
    ProofBackend, ProofJobQueue, ZkProof,
};
use ed25519_dalek::SigningKey;
use thiserror::Error;

/// Webhook router errors
#[derive(Error, Debug)]
pub enum WebhookError {
    #[error("Route not found: {route}")]
    RouteNotFound { route: String },

    #[error("Capsule not loaded for route: {route}")]
    CapsuleNotLoaded { route: String },

    #[error("Execution failed: {reason}")]
    ExecutionFailed { reason: String },

    #[error("Invalid request: {reason}")]
    InvalidRequest { reason: String },

    #[error("Internal error: {reason}")]
    InternalError { reason: String },
}

/// Webhook route configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    /// Route path (e.g., "/transform")
    pub path: String,
    /// WASM capsule bytes
    #[serde(skip)]
    pub capsule_bytes: Vec<u8>,
    /// Resource limits for this route
    pub resource_limits: ResourceLimits,
    /// Whether to generate ZK proofs for this route
    pub enable_zk_proofs: bool,
    /// Description of what this route does
    pub description: String,
}

/// Webhook request
#[derive(Debug, Deserialize)]
pub struct WebhookRequest {
    /// JSON payload to pass to the capsule
    pub payload: serde_json::Value,
    /// Whether to wait for ZK proof generation
    #[serde(default)]
    pub wait_for_proof: bool,
}

/// Webhook response
#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    /// Success status
    pub success: bool,
    /// Output from the capsule
    pub output: Option<String>,
    /// Execution receipt
    pub receipt: ExecutionReceipt,
    /// ZK proof (if requested and completed)
    pub proof: Option<ZkProof>,
    /// Proof job ID (if ZK proof requested but not yet completed)
    pub proof_job_id: Option<String>,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Router configuration
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    /// Server bind address
    pub bind_address: String,
    /// Server port
    pub port: u16,
    /// Whether to enable ZK proof generation
    pub enable_zk_proofs: bool,
    /// Proof queue worker count
    pub proof_workers: usize,
    /// Proof queue capacity
    pub proof_queue_capacity: usize,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
            enable_zk_proofs: true,
            proof_workers: 2,
            proof_queue_capacity: 100,
        }
    }
}

/// Shared router state
struct RouterState {
    /// Routes mapped to their configurations
    routes: HashMap<String, RouteConfig>,
    /// WASM runtime
    runtime: WasmRuntime,
    /// Proof job queue (optional)
    proof_queue: Option<Arc<ProofJobQueue>>,
}

/// Verifiable Webhook Router
pub struct WebhookRouter {
    config: WebhookConfig,
    state: Arc<RwLock<RouterState>>,
}

impl WebhookRouter {
    /// Create a new webhook router
    pub fn new(
        config: WebhookConfig,
        signing_key: SigningKey,
        proof_backend: Option<Arc<dyn ProofBackend>>,
    ) -> Result<Self, WebhookError> {
        let runtime = WasmRuntime::new(signing_key).map_err(|e| WebhookError::InternalError {
            reason: format!("Failed to create runtime: {}", e),
        })?;

        // Create proof queue if ZK proofs are enabled
        let proof_queue = if config.enable_zk_proofs {
            proof_backend.map(|backend| {
                Arc::new(ProofJobQueue::new(
                    backend,
                    config.proof_queue_capacity,
                    config.proof_workers,
                ))
            })
        } else {
            None
        };

        let state = Arc::new(RwLock::new(RouterState {
            routes: HashMap::new(),
            runtime,
            proof_queue,
        }));

        Ok(Self { config, state })
    }

    /// Add a route to the router
    pub async fn add_route(&self, route_config: RouteConfig) -> Result<(), WebhookError> {
        let mut state = self.state.write().await;
        state.routes.insert(route_config.path.clone(), route_config);
        Ok(())
    }

    /// Remove a route from the router
    pub async fn remove_route(&self, path: &str) -> Result<(), WebhookError> {
        let mut state = self.state.write().await;
        state.routes.remove(path)
            .ok_or_else(|| WebhookError::RouteNotFound { route: path.to_string() })?;
        Ok(())
    }

    /// List all routes
    pub async fn list_routes(&self) -> Vec<String> {
        let state = self.state.read().await;
        state.routes.keys().cloned().collect()
    }

    /// Build the Axum router
    pub fn build_router(self: Arc<Self>) -> Router {
        Router::new()
            .route("/health", get(health_check))
            .route("/routes", get(list_routes_handler))
            .route("/webhook/:route", post(webhook_handler))
            .route("/proof/:job_id", get(get_proof_handler))
            .with_state(self)
    }

    /// Start the webhook server
    pub async fn serve(self: Arc<Self>) -> Result<(), WebhookError> {
        let addr = format!("{}:{}", self.config.bind_address, self.config.port);
        let router = self.clone().build_router();

        tracing::info!("Starting webhook router on {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| WebhookError::InternalError {
                reason: format!("Failed to bind to {}: {}", addr, e),
            })?;

        axum::serve(listener, router)
            .await
            .map_err(|e| WebhookError::InternalError {
                reason: format!("Server error: {}", e),
            })?;

        Ok(())
    }

    /// Get the router configuration
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

        router.remove_route("/test").await.unwrap();
        assert_eq!(router.list_routes().await.len(), 0);
    }
}
