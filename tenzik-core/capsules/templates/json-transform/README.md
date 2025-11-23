# JSON Transform Capsule

A versatile Tenzik capsule for transforming JSON data with verifiable execution receipts.

## Features

- **Field Filtering**: Select specific fields from input JSON
- **Field Mapping**: Rename and transform fields
- **Value Transformation**: Apply simple transformations (uppercase, lowercase, truncate)
- **Deterministic**: Guaranteed reproducible results
- **Verifiable**: Every transformation gets a cryptographic receipt
- **Lightweight**: Target <4KB WASM

## Use Cases

- **Webhook Routing**: Transform incoming webhook payloads before forwarding
- **API Adaptation**: Convert between different API formats
- **Data Sanitization**: Filter sensitive fields before logging
- **ETL Pipelines**: Transform data during ingestion

## Input Format

```json
{
  "data": {
    "field1": "value1",
    "field2": "value2",
    "nested": {
      "field3": "value3"
    }
  },
  "transform": {
    "select": ["field1", "nested.field3"],
    "rename": {
      "field1": "name"
    },
    "operations": {
      "name": "uppercase"
    }
  }
}
```

## Output Format

```json
{
  "result": {
    "name": "VALUE1",
    "field3": "value3"
  },
  "metadata": {
    "capsule": "json-transform",
    "fields_processed": 2,
    "timestamp": "2024-12-01"
  }
}
```

## Build

```bash
npm install
npm run build
```

This produces `build/capsule.wasm` optimized for size.

## Test with Tenzik CLI

```bash
# Basic field selection
tenzik test build/capsule.wasm '{
  "data": {"name": "Alice", "age": 30, "email": "alice@example.com"},
  "transform": {"select": ["name", "age"]}
}'

# Field renaming and transformation
tenzik test build/capsule.wasm '{
  "data": {"user_name": "bob", "user_email": "bob@example.com"},
  "transform": {
    "select": ["user_name", "user_email"],
    "rename": {"user_name": "name", "user_email": "email"},
    "operations": {"name": "uppercase"}
  }
}'

# Nested field access
tenzik test build/capsule.wasm '{
  "data": {
    "user": {
      "profile": {
        "name": "Charlie"
      }
    }
  },
  "transform": {"select": ["user.profile.name"]}
}'
```

## Transform Operations

### Selection
- Select specific fields: `{"select": ["field1", "field2"]}`
- Select nested fields: `{"select": ["parent.child.field"]}`

### Renaming
- Rename fields: `{"rename": {"old_name": "new_name"}}`

### Operations
- `uppercase`: Convert string to uppercase
- `lowercase`: Convert string to lowercase
- `truncate_10`: Truncate string to 10 characters
- `hash`: Replace value with Blake3 hash (when host function available)

## Verifiable Execution

Every transformation produces a cryptographic receipt containing:
- Blake3 hash of input JSON
- Blake3 hash of output JSON
- Ed25519 signature from executing node
- Execution metrics (fuel used, memory, duration)
- Optional zero-knowledge proof

This enables:
- **Auditability**: Prove a transformation occurred
- **Reproducibility**: Re-execute and verify same output
- **Compliance**: Meet regulatory requirements for data processing

## Integration Example

```rust
use tenzik_runtime::WasmExecutor;
use tenzik_federation::TenzikNode;

// Load and execute transform capsule
let executor = WasmExecutor::new()?;
let input = r#"{
    "data": {"sensitive_field": "secret", "public_field": "visible"},
    "transform": {"select": ["public_field"]}
}"#;

let (output, receipt) = executor.execute_with_receipt("transform.wasm", input)?;

// Receipt proves the transformation happened
println!("Receipt ID: {}", receipt.receipt_id());
println!("Output: {}", output);

// Optionally broadcast to federation
node.add_receipt(receipt).await?;
```

## Size Optimization

Techniques to stay under 4KB:
- No JSON parser library (custom simple parser)
- Minimal string operations
- No complex data structures
- Inline simple functions

## Security Considerations

- **Determinism**: Pure function, no random values or timestamps in output
- **No External Calls**: Capsule cannot make network requests
- **Resource Limits**: Bounded by fuel and memory limits
- **Input Validation**: Malformed JSON returns error, not crash

## License

Apache-2.0
