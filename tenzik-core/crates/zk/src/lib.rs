//! Zero-knowledge proof backend for Tenzik
//!
//! This crate provides an abstraction layer for generating and verifying
//! zero-knowledge proofs of execution receipts.

pub mod backend;
pub mod mock;
pub mod queue;

pub use backend::{ProofBackend, ProofError, ProofRequest, ProofResponse, ProofStatus};
pub use mock::MockProofBackend;
pub use queue::{ProofJob, ProofJobQueue, ProofJobStatus};
