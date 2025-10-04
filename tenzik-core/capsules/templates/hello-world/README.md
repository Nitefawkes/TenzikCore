# Hello World Capsule

A simple Tenzik capsule template demonstrating basic input/output and deterministic execution.

## Features

- **Input**: JSON with optional `name` field
- **Output**: JSON greeting with timestamp and capsule info
- **Size**: Target <3KB when compiled with AssemblyScript + wasm-opt -Oz
- **Deterministic**: Same input always produces same output

## Build

```bash
npm install
npm run build
```

This produces `build/capsule.wasm` ready for testing.

## Test

```bash
# With tenzik CLI (once implemented)
tenzik test build/capsule.wasm '{"name": "Alice"}'

# Expected output:
# {"greeting": "Hello, Alice! Greetings from Tenzik capsule.", "timestamp": "2024-12-01", "capsule": "hello-world"}
```

## Usage Pattern

This template demonstrates the basic capsule interface:

1. **Input**: Read from WASM memory at specified pointer/length
2. **Processing**: Pure function with no side effects  
3. **Output**: Write result to WASM memory, return pointer/length

## Size Optimization

Techniques used to stay under 3-5KB:
- No dynamic allocation patterns
- Simple string operations
- No complex libraries
- Host-provided primitives (when available)

## Host Functions

This template is designed to work with these host-provided functions:
- `hash_commit(bytes)` - Blake3 hashing
- `json_path(bytes, path)` - JSON extraction
- `base64_encode(bytes)` - Base64 encoding

(Currently using native implementations for simplicity)
