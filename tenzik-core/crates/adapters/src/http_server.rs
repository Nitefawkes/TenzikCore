//! HTTP server scaffolding.
//!
//! The actual server implementation is scheduled for Sprint 4. The current
//! types are placeholders so the crate compiles and downstream code can depend
//! on stable interfaces.

/// Configuration for the placeholder HTTP server.
#[derive(Debug, Clone, Default)]
pub struct ServerConfig {
    /// TODO: Flesh out real server configuration options in Sprint 4.
    pub placeholder: Option<String>,
}

/// Minimal HTTP server stub.
pub struct HttpServer {
    config: ServerConfig,
}

impl HttpServer {
    /// Creates a new [`HttpServer`] instance from the given configuration.
    pub fn new(config: ServerConfig) -> Self {
        Self { config }
    }

    /// Access the configuration associated with this server.
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// TODO: Replace with real HTTP server startup logic in Sprint 4.
    pub async fn run(&self) {
        // Intentionally left empty until Sprint 4 implementation.
    }
}
