//! Protocol adapters (HTTP, webhooks, etc)
//!
//! This crate provides adapters to bridge external protocols with Tenzik,
//! starting with the Verifiable Webhook Router for the MVP demo.

pub mod webhook_router;
pub mod http_server;

pub use webhook_router::{WebhookRouter, WebhookConfig};
pub use http_server::{HttpServer, ServerConfig};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
