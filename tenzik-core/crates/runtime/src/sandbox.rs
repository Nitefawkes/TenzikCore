//! Security Sandbox Module
//!
//! This module provides capability-based access control for WASM capsules.
//! It ensures capsules can only access explicitly granted capabilities through
//! host functions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Capability types that can be granted to WASM capsules
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    /// Access to Blake3 hashing functions
    Hash,
    /// Access to JSON path operations
    Json,
    /// Access to Base64 encoding/decoding
    Base64,
    /// Access to deterministic time functions
    Time,
    /// Access to deterministic random number generation
    Random,
}

impl Capability {
    /// Get the host function prefix for this capability
    pub fn host_function_prefix(&self) -> &'static str {
        match self {
            Capability::Hash => "hash_",
            Capability::Json => "json_",
            Capability::Base64 => "base64_",
            Capability::Time => "time_",
            Capability::Random => "random_",
        }
    }
    
    /// Get all available capabilities
    pub fn all() -> Vec<Capability> {
        vec![
            Capability::Hash,
            Capability::Json,
            Capability::Base64,
            Capability::Time,
            Capability::Random,
        ]
    }
    
    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Capability::Hash => "Blake3 cryptographic hashing",
            Capability::Json => "JSON path extraction and manipulation",
            Capability::Base64 => "Base64 encoding and decoding",
            Capability::Time => "Deterministic timestamp access",
            Capability::Random => "Deterministic random number generation",
        }
    }
}

/// Resource limits for WASM execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum memory allocation in MB
    pub memory_limit_mb: u32,
    /// Maximum execution time in milliseconds
    pub execution_time_ms: u64,
    /// Maximum fuel units for execution (Wasmtime-specific)
    pub fuel_limit: u64,
    /// Allowed capabilities
    pub capabilities: Vec<Capability>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            memory_limit_mb: 32,
            execution_time_ms: 1000,
            fuel_limit: 1_000_000, // 1M fuel units
            capabilities: vec![Capability::Hash, Capability::Json], // Minimal default set
        }
    }
}

impl ResourceLimits {
    /// Create resource limits for development/testing (more permissive)
    pub fn development() -> Self {
        Self {
            memory_limit_mb: 64,
            execution_time_ms: 5000,
            fuel_limit: 10_000_000,
            capabilities: Capability::all(),
        }
    }
    
    /// Create resource limits for production (strict)
    pub fn production() -> Self {
        Self {
            memory_limit_mb: 16,
            execution_time_ms: 500,
            fuel_limit: 500_000,
            capabilities: vec![Capability::Hash], // Only hashing in production
        }
    }
    
    /// Check if a capability is granted
    pub fn has_capability(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
    }
    
    /// Add a capability if not already present
    pub fn add_capability(&mut self, capability: Capability) {
        if !self.has_capability(capability) {
            self.capabilities.push(capability);
        }
    }
    
    /// Remove a capability
    pub fn remove_capability(&mut self, capability: Capability) {
        self.capabilities.retain(|&c| c != capability);
    }
}

/// Security sandbox errors
#[derive(Error, Debug)]
pub enum SandboxError {
    #[error("Capability {capability:?} not granted")]
    CapabilityNotGranted { capability: Capability },
    
    #[error("Host function {function} not allowed")]
    HostFunctionNotAllowed { function: String },
    
    #[error("Resource limit exceeded: {limit_type}")]
    ResourceLimitExceeded { limit_type: String },
    
    #[error("Import {import} not in allowlist")]
    ImportNotAllowed { import: String },
}

/// Access log entry for auditing
#[derive(Debug, Clone)]
pub struct AccessLogEntry {
    /// Timestamp of the access attempt
    pub timestamp: u64,
    /// Capability being accessed
    pub capability: Capability,
    /// Specific action performed
    pub action: String,
    /// Whether the access was allowed
    pub allowed: bool,
    /// Additional context
    pub context: HashMap<String, String>,
}

/// Security sandbox for WASM execution
pub struct SecuritySandbox {
    /// Resource limits for this sandbox
    resource_limits: ResourceLimits,
    /// Access log for auditing
    access_log: Vec<AccessLogEntry>,
    /// Host function allowlist (generated from capabilities)
    host_function_allowlist: HashMap<String, Capability>,
}

impl SecuritySandbox {
    /// Create a new security sandbox with the given resource limits
    pub fn new(resource_limits: ResourceLimits) -> Self {
        let mut sandbox = Self {
            resource_limits,
            access_log: Vec::new(),
            host_function_allowlist: HashMap::new(),
        };
        
        // Generate host function allowlist from capabilities
        sandbox.generate_host_function_allowlist();
        
        sandbox
    }
    
    /// Create a sandbox with default limits
    pub fn default() -> Self {
        Self::new(ResourceLimits::default())
    }
    
    /// Create a sandbox for development
    pub fn development() -> Self {
        Self::new(ResourceLimits::development())
    }
    
    /// Create a sandbox for production
    pub fn production() -> Self {
        Self::new(ResourceLimits::production())
    }
    
    /// Check if a capability is granted
    pub fn has_capability(&self, capability: Capability) -> bool {
        self.resource_limits.has_capability(capability)
    }
    
    /// Check if a host function is allowed
    pub fn allows_host_function(&self, function_name: &str) -> bool {
        self.host_function_allowlist.contains_key(function_name)
    }
    
    /// Check if an import is allowed (for WASM validation)
    pub fn allows_import(&self, import_name: &str) -> bool {
        // Check if it's a host function we recognize
        if import_name.starts_with("env::") {
            let function_name = &import_name[5..]; // Remove "env::" prefix
            return self.allows_host_function(function_name);
        }
        
        // Allow specific system imports
        match import_name {
            "env::memory" => true,
            "env::abort" => true, // AssemblyScript abort function
            _ => false,
        }
    }
    
    /// Log an access attempt
    pub fn log_access(&mut self, capability: Capability, action: String, allowed: bool) {
        self.log_access_with_context(capability, action, allowed, HashMap::new());
    }
    
    /// Log an access attempt with additional context
    pub fn log_access_with_context(
        &mut self,
        capability: Capability,
        action: String,
        allowed: bool,
        context: HashMap<String, String>,
    ) {
        let entry = AccessLogEntry {
            timestamp: self.current_timestamp_ms(),
            capability,
            action,
            allowed,
            context,
        };
        
        self.access_log.push(entry);
    }
    
    /// Get the access log (for auditing)
    pub fn access_log(&self) -> &[AccessLogEntry] {
        &self.access_log
    }
    
    /// Clear the access log
    pub fn clear_access_log(&mut self) {
        self.access_log.clear();
    }
    
    /// Get resource limits
    pub fn resource_limits(&self) -> &ResourceLimits {
        &self.resource_limits
    }
    
    /// Validate a host function call attempt
    pub fn validate_host_function_call(&mut self, function_name: &str) -> Result<Capability, SandboxError> {
        // Check if function is in allowlist
        if let Some(&capability) = self.host_function_allowlist.get(function_name) {
            // Log successful access
            self.log_access(capability, format!("call:{}", function_name), true);
            Ok(capability)
        } else {
            // Log denied access
            // Find the capability this function would belong to
            let capability = self.guess_capability_for_function(function_name);
            self.log_access(capability, format!("call:{}", function_name), false);
            
            Err(SandboxError::HostFunctionNotAllowed {
                function: function_name.to_string(),
            })
        }
    }
    
    /// Generate the host function allowlist based on granted capabilities
    fn generate_host_function_allowlist(&mut self) {
        self.host_function_allowlist.clear();
        
        for &capability in &self.resource_limits.capabilities {
            match capability {
                Capability::Hash => {
                    self.host_function_allowlist.insert("hash_commit".to_string(), capability);
                    self.host_function_allowlist.insert("hash_verify".to_string(), capability);
                }
                Capability::Json => {
                    self.host_function_allowlist.insert("json_path".to_string(), capability);
                    self.host_function_allowlist.insert("json_extract".to_string(), capability);
                }
                Capability::Base64 => {
                    self.host_function_allowlist.insert("base64_encode".to_string(), capability);
                    self.host_function_allowlist.insert("base64_decode".to_string(), capability);
                }
                Capability::Time => {
                    self.host_function_allowlist.insert("time_now_ms".to_string(), capability);
                    self.host_function_allowlist.insert("time_iso8601".to_string(), capability);
                }
                Capability::Random => {
                    self.host_function_allowlist.insert("random_bytes".to_string(), capability);
                    self.host_function_allowlist.insert("random_u32".to_string(), capability);
                }
            }
        }
    }
    
    /// Guess which capability a function belongs to (for error reporting)
    fn guess_capability_for_function(&self, function_name: &str) -> Capability {
        for capability in Capability::all() {
            if function_name.starts_with(capability.host_function_prefix()) {
                return capability;
            }
        }
        // Default to Hash if we can't guess
        Capability::Hash
    }
    
    /// Get current timestamp in milliseconds (for logging)
    fn current_timestamp_ms(&self) -> u64 {
        // In a real implementation, this might be provided by the host
        // For now, use a simple placeholder
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_capability_basic_functionality() {
        assert_eq!(Capability::Hash.host_function_prefix(), "hash_");
        assert_eq!(Capability::Json.host_function_prefix(), "json_");
        assert!(!Capability::Hash.description().is_empty());
    }
    
    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.memory_limit_mb, 32);
        assert_eq!(limits.execution_time_ms, 1000);
        assert!(limits.has_capability(Capability::Hash));
        assert!(limits.has_capability(Capability::Json));
        assert!(!limits.has_capability(Capability::Random));
    }
    
    #[test]
    fn test_sandbox_capability_checking() {
        let mut limits = ResourceLimits::default();
        limits.add_capability(Capability::Base64);
        
        let sandbox = SecuritySandbox::new(limits);
        
        assert!(sandbox.has_capability(Capability::Hash));
        assert!(sandbox.has_capability(Capability::Base64));
        assert!(!sandbox.has_capability(Capability::Random));
    }
    
    #[test]
    fn test_host_function_allowlist() {
        let limits = ResourceLimits {
            capabilities: vec![Capability::Hash, Capability::Time],
            ..Default::default()
        };
        
        let sandbox = SecuritySandbox::new(limits);
        
        assert!(sandbox.allows_host_function("hash_commit"));
        assert!(sandbox.allows_host_function("time_now_ms"));
        assert!(!sandbox.allows_host_function("json_path"));
        assert!(!sandbox.allows_host_function("random_bytes"));
    }
    
    #[test]
    fn test_import_validation() {
        let limits = ResourceLimits {
            capabilities: vec![Capability::Hash],
            ..Default::default()
        };
        
        let sandbox = SecuritySandbox::new(limits);
        
        assert!(sandbox.allows_import("env::hash_commit"));
        assert!(sandbox.allows_import("env::memory"));
        assert!(sandbox.allows_import("env::abort"));
        assert!(!sandbox.allows_import("env::json_path"));
        assert!(!sandbox.allows_import("unknown::function"));
    }
    
    #[test]
    fn test_access_logging() {
        let mut sandbox = SecuritySandbox::default();
        
        sandbox.log_access(Capability::Hash, "test_action".to_string(), true);
        
        let log = sandbox.access_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].capability, Capability::Hash);
        assert_eq!(log[0].action, "test_action");
        assert!(log[0].allowed);
    }
    
    #[test]
    fn test_host_function_validation() {
        let limits = ResourceLimits {
            capabilities: vec![Capability::Hash],
            ..Default::default()
        };
        
        let mut sandbox = SecuritySandbox::new(limits);
        
        // Should succeed
        let result = sandbox.validate_host_function_call("hash_commit");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Capability::Hash);
        
        // Should fail
        let result = sandbox.validate_host_function_call("json_path");
        assert!(result.is_err());
        
        // Check access log
        let log = sandbox.access_log();
        assert_eq!(log.len(), 2);
        assert!(log[0].allowed);
        assert!(!log[1].allowed);
    }
}
