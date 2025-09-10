#!/bin/bash
# Basic functionality test for nox server

set -e

echo "=== Basic Nox Server Test ==="

# Build and start server
echo "Building server..."
cargo build --release

echo "Starting server..."
./target/release/nox --config mock-config.yaml &
SERVER_PID=$!

# Wait for startup
sleep 3

echo "Testing endpoints..."

# Test health endpoint
echo "Testing /health..."
response=$(curl -s http://127.0.0.1:3000/health)
if [ "$response" = "OK" ]; then
    echo "✅ Health endpoint works"
else
    echo "❌ Health endpoint failed: $response"
fi

# Test handshake endpoint
echo "Testing /nox/handshake..."
response=$(curl -s http://127.0.0.1:3000/nox/handshake)
if echo "$response" | grep -q "kick-nox-v1"; then
    echo "✅ Handshake endpoint works"
else
    echo "❌ Handshake endpoint failed: $response"
fi

# Test mock API
echo "Testing /api/v1/users..."
response=$(curl -s http://127.0.0.1:3000/api/v1/users)
if echo "$response" | grep -q "Alice"; then
    echo "✅ Mock API endpoint works"
else
    echo "❌ Mock API endpoint failed: $response"
fi

# Test with kick client
echo "Testing kick integration..."
cd ../kick
kick_response=$(timeout 10 cargo run --bin kick get http://127.0.0.1:3000/nox/handshake 2>/dev/null | grep -o "kick-nox-v1" || echo "failed")
cd - >/dev/null

if [ "$kick_response" = "kick-nox-v1" ]; then
    echo "✅ Kick integration works"
else
    echo "❌ Kick integration failed"
fi

# Cleanup
echo "Stopping server..."
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null || true

echo "=== Test Complete ==="