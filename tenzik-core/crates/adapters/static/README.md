# Tenzik Receipt Explorer

A web-based interface for viewing and verifying execution receipts from Tenzik WASM capsules.

## Features

- 📋 **View Recent Receipts**: Browse execution receipts with full details
- 🔐 **Verify Signatures**: Check cryptographic signatures (Ed25519)
- ✨ **ZK Proof Status**: View and verify zero-knowledge proofs
- 📊 **Execution Metrics**: Inspect gas usage, memory, duration, and capsule size
- 🔍 **Search & Filter**: Find receipts by ID, capsule hash, or node ID

## Usage

### Starting the Receipt Explorer

The Receipt Explorer is integrated into the Webhook Router. Start it with:

```rust
use tenzik_adapters::{WebhookRouter, WebhookConfig};
use ed25519_dalek::SigningKey;

#[tokio::main]
async fn main() {
    let config = WebhookConfig {
        bind_address: "127.0.0.1".to_string(),
        port: 8080,
        ..Default::default()
    };

    let signing_key = /* your signing key */;
    let router = WebhookRouter::new(config, signing_key, None).unwrap();

    // Start the server
    let router = Arc::new(router);
    router.serve().await.unwrap();
}
```

### Accessing the UI

Once the server is running, navigate to:

```
http://localhost:8080/explorer
```

## API Endpoints

The Receipt Explorer provides these REST endpoints:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/explorer` | GET | Receipt Explorer web UI |
| `/api/receipts` | GET | List recent receipts |
| `/api/receipts/:id` | GET | Get specific receipt |
| `/api/verify` | POST | Verify receipt signature |
| `/proof/:job_id` | GET | Check ZK proof status |
| `/static/*` | GET | Static assets (CSS, JS) |

## API Examples

### List Receipts

```bash
curl http://localhost:8080/api/receipts
```

Response:
```json
[
  {
    "receipt_id": "rcpt_1234567890abcdef",
    "capsule_hash": "blake3:a1b2c3d4e5f6...",
    "timestamp": "2024-12-01T12:00:00Z",
    "node_id": "node_001",
    "has_zk_proof": true,
    "verified": true,
    "metrics": {
      "gas_used": 12500,
      "memory_used": 245000,
      "duration_ms": 42,
      "capsule_size": 3200
    }
  }
]
```

### Verify Receipt

```bash
curl -X POST http://localhost:8080/api/verify \
  -H "Content-Type: application/json" \
  -d '{"receipt": { ... receipt JSON ... }}'
```

Response:
```json
{
  "valid": true,
  "signature_valid": true,
  "zk_proof_valid": false,
  "receipt_id": "rcpt_1234567890abcdef",
  "note": "Full cryptographic verification requires ReceiptVerifier with node's public key"
}
```

### Check Proof Status

```bash
curl http://localhost:8080/proof/job_1234567890
```

Response:
```json
{
  "job_id": "job_1234567890",
  "status": "Completed",
  "proof": { ... ZK proof data ... },
  "error": null,
  "created_at": "2024-12-01T12:00:00Z",
  "completed_at": "2024-12-01T12:00:05Z"
}
```

## UI Components

### Tabs

1. **Recent Receipts**: Browse and search execution receipts
2. **Verify Receipt**: Paste receipt JSON for verification
3. **About**: Information about the Receipt Explorer

### Receipt Cards

Each receipt is displayed as a card showing:

- Receipt ID (unique identifier)
- Capsule hash (Blake3)
- Node ID (executing node)
- Timestamp
- Verification status
- ZK proof presence

Click on a receipt card to expand and view:

- Full execution metrics
- Complete receipt JSON
- Detailed verification information

## Development Notes

### File Structure

```
crates/adapters/static/
├── index.html      # Main UI structure
├── styles.css      # Styling and layout
├── app.js          # Frontend logic and API integration
└── README.md       # This file
```

### Integration with Webhook Router

The Receipt Explorer is served by the Webhook Router using Axum and tower-http:

- Static files served from `/static/*`
- HTML embedded using `include_str!` macro
- API endpoints share the same router state

### Future Enhancements

Potential improvements for the Receipt Explorer:

1. **Persistent Storage**: Store receipts in a database (e.g., sled, PostgreSQL)
2. **Real-time Updates**: WebSocket support for live receipt streaming
3. **Advanced Filtering**: Date ranges, execution status, gas usage
4. **Batch Verification**: Verify multiple receipts at once
5. **Export**: Download receipts as JSON or CSV
6. **Pagination**: Handle large numbers of receipts efficiently
7. **Charts**: Visualize execution metrics over time
8. **Proof Details**: Display detailed ZK proof structure

## Security Considerations

The Receipt Explorer provides:

- ✅ Basic receipt structure validation
- ✅ Signature presence checking
- ⚠️ Limited cryptographic verification (requires ReceiptVerifier integration)

For production deployments:

- Use HTTPS/TLS for all connections
- Implement rate limiting on API endpoints
- Add authentication/authorization for sensitive operations
- Integrate full ReceiptVerifier for signature validation
- Validate ZK proofs against expected proof systems

## Browser Compatibility

The Receipt Explorer UI works in modern browsers:

- ✅ Chrome/Edge 90+
- ✅ Firefox 88+
- ✅ Safari 14+

JavaScript features used:
- ES6+ syntax (async/await, arrow functions)
- Fetch API
- CSS Grid and Flexbox

## License

Apache-2.0 (same as parent project)
