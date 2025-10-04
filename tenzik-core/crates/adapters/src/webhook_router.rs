//! Webhook router scaffolding.
//!
//! The real implementation will arrive in Sprint 4. For now, this module
//! provides minimal types so that other crates can compile against them.

/// Configuration data for the webhook router.
#[derive(Debug, Clone, Default)]
pub struct WebhookConfig {
    /// TODO: Replace with actual configuration fields in Sprint 4.
    pub placeholder: Option<String>,
}

/// Placeholder webhook router implementation.
pub struct WebhookRouter {
    config: WebhookConfig,
}

impl WebhookRouter {
    /// Creates a new [`WebhookRouter`] using the provided configuration.
    pub fn new(config: WebhookConfig) -> Self {
        Self { config }
    }

    /// Access the configuration used to construct this router.
    pub fn config(&self) -> &WebhookConfig {
        &self.config
    }

    /// TODO: Replace with the real routing logic in Sprint 4.
    pub fn handle_request(&self) {
        // Intentionally left blank until Sprint 4 implementation.
    }
}
