# Sprint 2: Minimal Federation & DAG

**Duration**: Weeks 3-4  
**Objective**: Implement simple DAG-based federation for receipt exchange  
**Success Criteria**: Two Tenzik nodes exchange receipts end-to-end

## Progress Dashboard

### 🎯 Sprint 2 Gates
- [ ] **FED-003**: Event DAG + local store
- [ ] **FED-004**: NodeAnnouncement + handshake  
- [ ] **FED-005**: Gossip protocol implementation
- [ ] **CLI-102**: `tenzik node` command
- [ ] **DEMO-001**: Two-node receipt exchange

### 📊 Implementation Status

| Component | File | Status | Implementation | Tests | Docs |
|-----------|------|--------|----------------|-------|------|
| Event DAG | `federation/src/storage.rs` | 🔄 In Progress | 0% | 0% | 0% |
| Node Management | `federation/src/node.rs` | ⏳ Planned | 0% | 0% | 0% |
| Gossip Protocol | `federation/src/gossip.rs` | ⏳ Planned | 0% | 0% | 0% |
| CLI Node Command | `cli/src/commands/node.rs` | ⏳ Planned | 0% | 0% | 0% |
| Two-Node Demo | `examples/two-node-demo/` | ⏳ Planned | 0% | 0% | 0% |

## Sprint 2 Implementation Plan

### Week 1: Core Federation Infrastructure

#### Day 1-2: Event DAG (FED-003)
**File**: `crates/federation/src/storage.rs`

**Requirements**:
- Simple DAG structure with Event nodes
- Local sled-based storage backend
- Tip selection algorithm (simple: latest timestamp)
- Event validation and integrity checking
- Receipt storage and retrieval

**Success Criteria**:
```rust
let mut dag = EventDAG::new("./test_db")?;
let event = Event::new_receipt(receipt);
dag.add_event(event)?;
let tips = dag.get_tips()?;
assert!(!tips.is_empty());
```

#### Day 3-4: Node Management (FED-004)  
**File**: `crates/federation/src/node.rs`

**Requirements**:
- TenzikNode struct with identity management
- NodeAnnouncement protocol
- Basic peer discovery and handshake
- Connection management
- Health checking

**Success Criteria**:
```rust
let node = TenzikNode::new(config)?;
node.start().await?;
node.announce_to_peer("127.0.0.1:9001").await?;
let peers = node.get_connected_peers();
assert_eq!(peers.len(), 1);
```

#### Day 5: Gossip Protocol (FED-005)
**File**: `crates/federation/src/gossip.rs`

**Requirements**:
- Simple push-based gossip for receipts
- Periodic sync with peers
- Conflict resolution (prefer earliest timestamp)
- Bandwidth limiting

### Week 2: Integration & Demo

#### Day 6-7: CLI Integration (CLI-102)
**File**: `crates/cli/src/commands/node.rs`

**Requirements**:
- `tenzik node start` command
- Peer connection management
- Status monitoring
- Graceful shutdown

**Success Criteria**:
```bash
tenzik node start --port 9000 --db ./node1
# In another terminal:
tenzik node start --port 9001 --db ./node2 --peer 127.0.0.1:9000
```

#### Day 8-9: Two-Node Demo (DEMO-001)
**File**: `examples/two-node-demo/`

**Requirements**:
- Script to start two nodes
- Execution on node A generates receipt
- Receipt propagates to node B via gossip
- Verification on node B succeeds

#### Day 10: End-to-End Testing
- Integration tests across federation components
- Network partition recovery testing
- Performance measurement
- Documentation updates

## Federation Architecture Design

### Event Structure
```rust
pub struct Event {
    pub id: String,              // Blake3 hash of content
    pub event_type: EventType,   // Receipt, Announce, etc.
    pub content: EventContent,   // Actual data
    pub timestamp: String,       // ISO 8601
    pub parents: Vec<String>,    // Parent event IDs
    pub sequence: u64,           // Local sequence number
    pub node_id: String,         // Creator node ID
    pub signature: String,       // Ed25519 signature
}

pub enum EventType {
    Receipt,          // ExecutionReceipt
    NodeAnnounce,     // Node joining network
    NodeLeave,        // Node leaving network
}
```

### DAG Properties
- **Causally Ordered**: Events reference parents
- **Tamper Evident**: All events signed by creators
- **Eventually Consistent**: Gossip ensures convergence
- **Partition Tolerant**: Nodes can operate independently

### Gossip Protocol
```rust
pub struct GossipProtocol {
    pub sync_interval_ms: u64,     // Default: 5000ms
    pub max_events_per_sync: usize, // Default: 100
    pub peer_timeout_ms: u64,       // Default: 30000ms
}

// Protocol messages
pub enum GossipMessage {
    Sync { since: Option<String> },  // Request events since ID
    Events { events: Vec<Event> },   // Push events to peer
    Ack { count: usize },           // Acknowledge receipt
}
```

## Success Metrics

### Gate A: Basic Federation
**Target**: End of Week 1
- [ ] Events stored and retrieved from DAG
- [ ] Nodes can discover and connect to peers
- [ ] Basic gossip synchronization works
- [ ] CLI can start federation nodes

### Gate B: Two-Node Demo
**Target**: End of Week 2
- [ ] Two nodes exchange receipts automatically
- [ ] Receipt verification works across nodes
- [ ] Network partition recovery demonstrated
- [ ] Performance acceptable (< 5s sync time)

### Quality Gates
- [ ] All federation APIs documented
- [ ] Integration tests cover network scenarios
- [ ] Security review of gossip protocol
- [ ] Performance baseline for federation

## Documentation Links

### Architecture Documents
- [Federation Design](../architecture/federation-design.md) - DAG and gossip architecture
- [Network Protocol](../architecture/network-protocol.md) - Wire format specification
- [Node Discovery](../architecture/node-discovery.md) - Peer discovery mechanisms

### API Documentation  
- [Federation API](../api/federation-api.md) - EventDAG, TenzikNode APIs
- [Gossip API](../api/gossip-api.md) - GossipProtocol specification
- [Storage API](../api/storage-api.md) - Persistent storage interfaces

## Risk Mitigation

### Technical Risks
- **Network partitions**: Design for partition tolerance from start
- **Message ordering**: Use vector clocks or timestamp ordering
- **Peer discovery**: Start simple (manual), add mDNS later

### Performance Risks  
- **DAG growth**: Implement periodic pruning strategy
- **Gossip overhead**: Limit sync frequency and batch size
- **Storage scaling**: Use efficient sled configuration

### Security Risks
- **Message tampering**: All events signed with Ed25519
- **Replay attacks**: Include nonces and timestamps
- **DoS attacks**: Rate limiting and peer validation

## Next Sprint Preview

**Sprint 3: Optional ZK Backend (Weeks 5-6)**
- Pluggable proof backend architecture
- Mock ZK implementation for testing
- Background proof generation queue
- Receipt verification with ZK proofs

The federation layer will provide the foundation for distributed proof verification in Sprint 3.
