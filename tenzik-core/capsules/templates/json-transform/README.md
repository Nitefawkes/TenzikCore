# JSON Transform Capsule

A Tenzik capsule template demonstrating JSON transformation and field extraction using path notation.

## Features

- **Input**: JSON with nested objects and fields
- **Processing**: Extract fields using path notation (e.g., `user.name`)
- **Transform**: Map input structure to output structure
- **Output**: Transformed JSON with enriched data
- **Size**: Target <3KB when compiled with AssemblyScript + wasm-opt -Oz
- **Deterministic**: Same input always produces same output

## Use Cases

- **Webhook Transformation**: Convert webhook payloads between formats
- **Data Mapping**: Extract and reshape API responses
- **Event Processing**: Transform event streams
- **Field Extraction**: Pull specific fields from complex JSON

## Build

```bash
npm install
npm run build
```

This produces `build/capsule.wasm` ready for testing.

## Test

```bash
# Example 1: User login event transformation
tenzik test build/capsule.wasm '{
  "user": {"name": "Alice", "age": 30},
  "action": "login",
  "timestamp": 1234567890
}'

# Expected output:
# {
#   "event": "notification",
#   "message": "User Alice performed login",
#   "user": "Alice",
#   "action": "login",
#   "severity": "info",
#   "timestamp": 1234567890,
#   "processed_by": "json-transform-capsule"
# }

# Example 2: Different action
tenzik test build/capsule.wasm '{
  "user": {"name": "Bob", "age": 25},
  "action": "logout",
  "timestamp": 1234567900
}'

# Expected output:
# {
#   "event": "notification",
#   "message": "User Bob performed logout",
#   "user": "Bob",
#   "action": "logout",
#   "severity": "warning",
#   "timestamp": 1234567900,
#   "processed_by": "json-transform-capsule"
# }
```

## Implementation Details

### JSON Path Extraction

The capsule implements simple JSON path extraction:

- `"fieldName"` - Extract top-level field
- `"object.field"` - Extract nested field from object

Examples:
```typescript
extractJsonValue(json, "action")        // → "login"
extractJsonValue(json, "user.name")     // → "Alice"
extractJsonValue(json, "timestamp")     // → "1234567890"
```

### Value Type Handling

The extractor handles multiple JSON value types:
- **Strings**: `"field": "value"` → returns `value`
- **Numbers**: `"field": 123` → returns `123`
- **Objects**: `"field": {...}` → returns the complete object JSON
- **Null/Missing**: Returns `"null"` for missing fields

### Transformation Logic

The template demonstrates a common transformation pattern:

1. **Extract** relevant fields from input
2. **Enrich** with computed values (e.g., severity based on action)
3. **Reshape** into target format
4. **Add metadata** (e.g., processed_by field)

## Customization

To adapt this template for your use case:

1. **Modify field extraction**: Update the field names in the `run()` function
2. **Change transformation logic**: Edit `buildJsonOutput()` to create your desired output structure
3. **Add validation**: Insert checks for required fields
4. **Extend path syntax**: Add support for arrays, deeper nesting, etc.

## Size Optimization

Techniques used to stay under 3-5KB:

- **No external JSON library**: Custom lightweight parsers
- **Simple string operations**: Avoid complex regex or parsing
- **Minimal allocations**: Reuse buffers where possible
- **No runtime**: Compiled with `--runtime none`

## Host Functions (Future)

This template could leverage host-provided functions for better performance:

- `json_path(bytes, path)` - Native JSON extraction with full JSONPath support
- `hash_commit(bytes)` - For data integrity verification
- `base64_encode(bytes)` - For encoding binary data in output

Currently uses native implementations for maximum portability.

## Integration Example

Use with webhook router:

```rust
// Add route with JSON transform capsule
router.add_route(RouteConfig {
    path: "/webhook/github",
    capsule_bytes: fs::read("json-transform/build/capsule.wasm")?,
    resource_limits: ResourceLimits::default(),
    enable_zk_proofs: true,
    description: "Transform GitHub webhooks to internal notification format".to_string(),
}).await?;

// POST /webhook/github with GitHub payload
// → Capsule transforms to internal format
// → Returns receipt + optional ZK proof
```

## Next Steps

- Add support for array indexing (`items[0]`)
- Implement filtering logic (e.g., only process certain actions)
- Add field validation and error messages
- Support multiple transformation rules
