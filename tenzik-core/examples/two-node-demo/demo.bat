@echo off
REM Two-Node Federation Demo (Windows)
REM This script demonstrates two Tenzik nodes federating and exchanging receipts

echo 🌐 Tenzik Two-Node Federation Demo
echo ==================================
echo.

REM Check if we're in the right directory
if not exist "..\..\Cargo.toml" (
    echo ❌ Please run this script from the examples\two-node-demo directory
    exit /b 1
)

REM Build the workspace first
echo 🔧 Building Tenzik workspace...
cd ..\..
cargo build --release
if %errorlevel% neq 0 (
    echo ❌ Build failed
    exit /b 1
)
cd examples\two-node-demo

echo ✅ Build complete!
echo.

REM Clean up any existing data
echo 🧹 Cleaning up existing node data...
if exist "node1_data" rmdir /s /q node1_data
if exist "node2_data" rmdir /s /q node2_data
mkdir node1_data
mkdir node2_data

echo 📋 Demo Setup:
echo    Node 1: Port 9000, Database: .\node1_data
echo    Node 2: Port 9001, Database: .\node2_data, Peer: 127.0.0.1:9000
echo.

echo 🚀 Starting Node 1 (Bootstrap node)...
start "Tenzik Node 1" cmd /c "cargo run --release -p tenzik-cli -- node --port 9000 --db .\node1_data --name bootstrap-node"

REM Wait for node 1 to start
echo ⏳ Waiting for Node 1 to initialize...
timeout /t 5 /nobreak > nul

echo 🚀 Starting Node 2 (Peer node)...
start "Tenzik Node 2" cmd /c "cargo run --release -p tenzik-cli -- node --port 9001 --db .\node2_data --name peer-node --peer 127.0.0.1:9000"

REM Wait for node 2 to connect
echo ⏳ Waiting for Node 2 to connect...
timeout /t 5 /nobreak > nul

echo ✅ Both nodes should now be running in separate windows!
echo.
echo 📊 Demo Status:
echo    Node 1: Running on port 9000
echo    Node 2: Running on port 9001
echo.

echo 🧪 Next Steps (Manual Testing):
echo    1. In another terminal, test a capsule execution:
echo       cargo run -p tenzik-cli -- test path\to\capsule.wasm "{\"test\":\"data\"}"
echo    2. Check that the receipt appears in both node databases
echo    3. Verify federation is working correctly
echo.

echo 🔄 Demo running in separate windows...
echo 🛑 Close the node windows when you're done testing
pause
