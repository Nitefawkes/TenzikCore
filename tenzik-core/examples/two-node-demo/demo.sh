#!/bin/bash

# Two-Node Federation Demo
# This script demonstrates two Tenzik nodes federating and exchanging receipts

set -e

echo "🌐 Tenzik Two-Node Federation Demo"
echo "=================================="
echo ""

# Check if we're in the right directory
if [ ! -f "../../Cargo.toml" ]; then
    echo "❌ Please run this script from the examples/two-node-demo directory"
    exit 1
fi

# Build the workspace first
echo "🔧 Building Tenzik workspace..."
cd ../..
cargo build --release
cd examples/two-node-demo

echo "✅ Build complete!"
echo ""

# Clean up any existing data
echo "🧹 Cleaning up existing node data..."
rm -rf ./node1_data ./node2_data
mkdir -p node1_data node2_data

echo "📋 Demo Setup:"
echo "   Node 1: Port 9000, Database: ./node1_data"
echo "   Node 2: Port 9001, Database: ./node2_data, Peer: 127.0.0.1:9000"
echo ""

# Function to cleanup background processes
cleanup() {
    echo ""
    echo "🛑 Shutting down nodes..."
    kill $NODE1_PID $NODE2_PID 2>/dev/null || true
    wait 2>/dev/null || true
    echo "✅ Cleanup complete"
}

# Set up cleanup trap
trap cleanup EXIT

echo "🚀 Starting Node 1 (Bootstrap node)..."
cargo run --release -p tenzik-cli -- node --port 9000 --db ./node1_data --name "bootstrap-node" &
NODE1_PID=$!

# Wait for node 1 to start
echo "⏳ Waiting for Node 1 to initialize..."
sleep 5

echo "🚀 Starting Node 2 (Peer node)..."
cargo run --release -p tenzik-cli -- node --port 9001 --db ./node2_data --name "peer-node" --peer 127.0.0.1:9000 &
NODE2_PID=$!

# Wait for node 2 to connect
echo "⏳ Waiting for Node 2 to connect..."
sleep 5

echo "✅ Both nodes should now be running and connected!"
echo ""
echo "📊 Demo Status:"
echo "   Node 1: http://127.0.0.1:9000 (PID: $NODE1_PID)"
echo "   Node 2: http://127.0.0.1:9001 (PID: $NODE2_PID)"
echo ""

# TODO: Add capsule execution demo when runtime integration is complete
echo "🧪 Next Steps (Manual Testing):"
echo "   1. In another terminal, test a capsule execution:"
echo "      cargo run -p tenzik-cli -- test path/to/capsule.wasm '{\"test\":\"data\"}'"
echo "   2. Check that the receipt appears in both node databases"
echo "   3. Verify federation is working correctly"
echo ""

echo "🔄 Demo running... Press Ctrl+C to stop both nodes"

# Wait for user to stop
wait $NODE1_PID $NODE2_PID
