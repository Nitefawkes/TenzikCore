# Two-Node Federation Demo

This demo shows how two Tenzik nodes can federate and exchange receipts in real-time.

## Overview

The demo creates two independent Tenzik nodes:

1. **Node 1 (Bootstrap)**: Runs on port 9000, acts as the initial node
2. **Node 2 (Peer)**: Runs on port 9001, connects to Node 1 as a peer

When both nodes are running, they will:
- Exchange node announcements
- Sync their local DAGs via gossip protocol
- Share execution receipts automatically
- Maintain federation even during network partitions

## Prerequisites

- Rust toolchain installed
- Tenzik Core workspace built (`cargo build --release`)

## Running the Demo

### Linux/macOS
```bash
cd examples/two-node-demo
chmod +x demo.sh
./demo.sh
```

### Windows
```cmd
cd examples\two-node-demo
demo.bat
```

## What to Expect

1. **Node Startup**: Both nodes will start and display their configuration
2. **Peer Discovery**: Node 2 will connect to Node 1
3. **Federation**: Nodes will exchange announcements and establish gossip
4. **Status**: Each node will show DAG statistics and peer connections

## Testing Federation

Once both nodes are running, you can test the federation by:

1. **Execute a capsule on one node** (generates a receipt):
   ```bash
   # This will add a receipt to the local DAG
   cargo run -p tenzik-cli -- test hello.wasm '{"name":"test"}' --metrics
   ```

2. **Check receipt propagation**:
   - The receipt should appear in both node's DAGs
   - DAG statistics should show the same event count on both nodes
   - Federation ensures eventual consistency

## Architecture Demonstrated

### Event DAG
- Each node maintains a local DAG of events
- Events include: receipts, node announcements, heartbeats
- DAG ensures causal ordering and tamper-evidence

### Gossip Protocol
- Nodes periodically sync their DAGs
- Push-based gossip propagates new events
- Conflict resolution ensures consistency

### Network Resilience
- Nodes can operate independently during partitions
- Automatic reconnection when network heals
- Eventually consistent state across all nodes

## Monitoring

Each node displays:
- **Connected Peers**: Number of active federation connections
- **DAG Events**: Total events in the local DAG
- **Receipt Count**: Number of execution receipts received
- **Node Count**: Number of unique nodes seen in federation

## Troubleshooting

### Port Already in Use
If ports 9000 or 9001 are busy:
```bash
# Check what's using the ports
netstat -tlnp | grep :9000
netstat -tlnp | grep :9001

# Kill any existing processes or use different ports
cargo run -p tenzik-cli -- node --port 9002 --name node1
cargo run -p tenzik-cli -- node --port 9003 --peer 127.0.0.1:9002 --name node2
```

### Connection Issues
- Ensure both nodes are on the same network
- Check firewall settings (allow ports 9000-9001)
- Verify peer address format: `IP:PORT`

### Database Issues
- Clean up: `rm -rf node1_data node2_data`
- Ensure write permissions in demo directory
- Check disk space for database files

## Next Steps

This demo shows basic federation. In production, you might want to:

1. **Add More Nodes**: Scale to 3+ nodes for full mesh federation
2. **Network Security**: Add TLS encryption for peer connections
3. **Authentication**: Implement peer authentication and authorization
4. **Monitoring**: Add metrics collection and dashboards
5. **Persistence**: Configure database backups and recovery

## Code Structure

The demo uses these Tenzik components:

- **`tenzik-federation`**: Node management and gossip protocol
- **`tenzik-runtime`**: WASM execution and receipt generation
- **`tenzik-cli`**: Command-line interface for node control
- **Event DAG**: Distributed state management with sled storage

## Performance Expectations

For this simple demo:
- **Startup Time**: 2-5 seconds per node
- **Connection Time**: 1-2 seconds for peer discovery
- **Sync Latency**: < 100ms for event propagation
- **Memory Usage**: < 50MB per node
- **Storage**: < 10MB for DAG databases

The performance scales well to larger federations with proper configuration.
