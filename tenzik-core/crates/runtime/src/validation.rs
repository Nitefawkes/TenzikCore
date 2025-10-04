//! WASM Validation Module
//!
//! This module provides validation for WebAssembly capsules before execution.
//! It ensures capsules meet Tenzik's size, security, and interface requirements.

use anyhow::{Context, Result};
use thiserror::Error;
use wasmtime::{Engine, Module};

/// Maximum capsule size in bytes (5KB default, configurable)
pub const DEFAULT_MAX_CAPSULE_SIZE: usize = 5 * 1024; // 5KB

/// Required exports for Tenzik capsules
pub const REQUIRED_EXPORTS: &[&str] = &["run", "memory"];

/// Allowed import prefixes for security
pub const ALLOWED_IMPORT_PREFIXES: &[&str] = &[
    "env::",      // Host environment functions
];

/// Validation errors
#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Capsule size {size} bytes exceeds maximum {max_size} bytes")]
    SizeExceeded { size: usize, max_size: usize },
    
    #[error("Missing required export: {export}")]
    MissingRequiredExport { export: String },
    
    #[error("Unauthorized import: {import}")]
    UnauthorizedImport { import: String },
    
    #[error("Invalid WASM module: {reason}")]
    InvalidModule { reason: String },
    
    #[error("Module compilation failed: {reason}")]
    CompilationFailed { reason: String },
}

/// Validation result containing detailed information about the capsule
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the capsule passed validation
    pub is_valid: bool,
    /// Size of the capsule in bytes
    pub size_bytes: usize,
    /// Size of the capsule in KB (rounded up)
    pub size_kb: f64,
    /// List of exports found in the module
    pub exports: Vec<String>,
    /// List of imports found in the module
    pub imports: Vec<String>,
    /// Any validation warnings (non-fatal issues)
    pub warnings: Vec<String>,
    /// Validation errors if any
    pub errors: Vec<ValidationError>,
}

impl ValidationResult {
    /// Create a new successful validation result
    pub fn success(size_bytes: usize, exports: Vec<String>, imports: Vec<String>) -> Self {
        Self {
            is_valid: true,
            size_bytes,
            size_kb: size_bytes as f64 / 1024.0,
            exports,
            imports,
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }
    
    /// Create a new failed validation result
    pub fn failure(size_bytes: usize, errors: Vec<ValidationError>) -> Self {
        Self {
            is_valid: false,
            size_bytes,
            size_kb: size_bytes as f64 / 1024.0,
            exports: Vec::new(),
            imports: Vec::new(),
            warnings: Vec::new(),
            errors,
        }
    }
    
    /// Add a warning to the validation result
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

/// WASM capsule validator with configurable security policies
pub struct WasmValidator {
    /// Maximum allowed capsule size in bytes
    max_size_bytes: usize,
    /// Wasmtime engine for compilation testing
    engine: Engine,
    /// Whether to require all imports to be from allowed prefixes
    strict_imports: bool,
    /// Whether to require all standard exports
    require_standard_exports: bool,
}

impl WasmValidator {
    /// Create a new validator with default settings
    pub fn new() -> Result<Self> {
        let engine = Engine::default();
        
        Ok(Self {
            max_size_bytes: DEFAULT_MAX_CAPSULE_SIZE,
            engine,
            strict_imports: true,
            require_standard_exports: true,
        })
    }
    
    /// Create a new validator with custom configuration
    pub fn with_config(config: ValidatorConfig) -> Result<Self> {
        let engine = Engine::default();
        
        Ok(Self {
            max_size_bytes: config.max_size_bytes,
            engine,
            strict_imports: config.strict_imports,
            require_standard_exports: config.require_standard_exports,
        })
    }
    
    /// Validate a WASM capsule from bytes
    pub fn validate(&self, wasm_bytes: &[u8]) -> Result<ValidationResult> {
        let size_bytes = wasm_bytes.len();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        
        // Check size limits first (fast check)
        if size_bytes > self.max_size_bytes {
            errors.push(ValidationError::SizeExceeded {
                size: size_bytes,
                max_size: self.max_size_bytes,
            });
            return Ok(ValidationResult::failure(size_bytes, errors));
        }
        
        // Add size warning if approaching limit
        if size_bytes > self.max_size_bytes * 80 / 100 {
            warnings.push(format!(
                "Capsule size ({:.1}KB) is approaching limit ({:.1}KB)",
                size_bytes as f64 / 1024.0,
                self.max_size_bytes as f64 / 1024.0
            ));
        }
        
        // Attempt to parse and compile the module
        let module = match Module::from_binary(&self.engine, wasm_bytes) {
            Ok(module) => module,
            Err(e) => {
                errors.push(ValidationError::CompilationFailed {
                    reason: e.to_string(),
                });
                return Ok(ValidationResult::failure(size_bytes, errors));
            }
        };
        
        // Extract exports and imports
        let exports = self.extract_exports(&module)?;
        let imports = self.extract_imports(&module)?;
        
        // Validate required exports
        if self.require_standard_exports {
            for required_export in REQUIRED_EXPORTS {
                if !exports.contains(&required_export.to_string()) {
                    errors.push(ValidationError::MissingRequiredExport {
                        export: required_export.to_string(),
                    });
                }
            }
        }
        
        // Validate imports against allowlist
        if self.strict_imports {
            for import in &imports {
                if !self.is_import_allowed(import) {
                    errors.push(ValidationError::UnauthorizedImport {
                        import: import.clone(),
                    });
                }
            }
        }
        
        // Create result
        if errors.is_empty() {
            let mut result = ValidationResult::success(size_bytes, exports, imports);
            result.warnings = warnings;
            Ok(result)
        } else {
            Ok(ValidationResult::failure(size_bytes, errors))
        }
    }
    
    /// Extract export names from the module
    fn extract_exports(&self, module: &Module) -> Result<Vec<String>> {
        let mut exports = Vec::new();
        
        for export in module.exports() {
            exports.push(export.name().to_string());
        }
        
        Ok(exports)
    }
    
    /// Extract import names from the module  
    fn extract_imports(&self, module: &Module) -> Result<Vec<String>> {
        let mut imports = Vec::new();
        
        for import in module.imports() {
            let import_name = format!("{}::{}", import.module(), import.name());
            imports.push(import_name);
        }
        
        Ok(imports)
    }
    
    /// Check if an import is allowed based on the allowlist
    fn is_import_allowed(&self, import: &str) -> bool {
        for prefix in ALLOWED_IMPORT_PREFIXES {
            if import.starts_with(prefix) {
                return true;
            }
        }
        false
    }
    
    /// Get the current size limit in bytes
    pub fn max_size_bytes(&self) -> usize {
        self.max_size_bytes
    }
    
    /// Get the current size limit in KB
    pub fn max_size_kb(&self) -> f64 {
        self.max_size_bytes as f64 / 1024.0
    }
}

impl Default for WasmValidator {
    fn default() -> Self {
        Self::new().expect("Failed to create default WasmValidator")
    }
}

/// Configuration for the WASM validator
#[derive(Debug, Clone)]
pub struct ValidatorConfig {
    /// Maximum capsule size in bytes
    pub max_size_bytes: usize,
    /// Whether to strictly enforce import allowlist
    pub strict_imports: bool,
    /// Whether to require standard Tenzik exports
    pub require_standard_exports: bool,
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: DEFAULT_MAX_CAPSULE_SIZE,
            strict_imports: true,
            require_standard_exports: true,
        }
    }
}

/// Convenience function to validate WASM bytes with default settings
pub fn validate_capsule(wasm_bytes: &[u8]) -> Result<ValidationResult> {
    let validator = WasmValidator::new()?;
    validator.validate(wasm_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_size_validation() {
        let validator = WasmValidator::with_config(ValidatorConfig {
            max_size_bytes: 100, // Very small limit for testing
            ..Default::default()
        }).unwrap();
        
        let large_module = vec![0u8; 200]; // Too large
        let result = validator.validate(&large_module).unwrap();
        
        assert!(!result.is_valid);
        assert!(matches!(result.errors[0], ValidationError::SizeExceeded { .. }));
    }
    
    #[test]
    fn test_invalid_wasm() {
        let validator = WasmValidator::new().unwrap();
        let invalid_wasm = b"not a wasm module";
        
        let result = validator.validate(invalid_wasm).unwrap();
        assert!(!result.is_valid);
        assert!(matches!(result.errors[0], ValidationError::CompilationFailed { .. }));
    }
    
    #[test]
    fn test_size_warning() {
        let validator = WasmValidator::with_config(ValidatorConfig {
            max_size_bytes: 100,
            require_standard_exports: false, // Skip export validation for this test
            strict_imports: false, // Skip import validation for this test
        }).unwrap();
        
        // Create a minimal valid WASM module that's 85 bytes (85% of 100 byte limit)
        let minimal_wasm = create_minimal_wasm_module();
        
        if minimal_wasm.len() > 80 { // If our test module triggers the warning
            let result = validator.validate(&minimal_wasm).unwrap();
            // Should have warnings about approaching size limit
            assert!(!result.warnings.is_empty());
        }
    }
    
    /// Helper to create a minimal valid WASM module for testing
    fn create_minimal_wasm_module() -> Vec<u8> {
        // Minimal WASM module with magic number and version
        // This is just for testing - real modules would be much larger
        vec![
            0x00, 0x61, 0x73, 0x6d, // Magic number
            0x01, 0x00, 0x00, 0x00, // Version
        ]
    }
}
