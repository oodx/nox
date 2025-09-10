#!/bin/bash
# uat.sh - Nox Server User Acceptance Tests
# Based on China's comprehensive UAT strategy

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test configuration
TEST_CONFIG_FILE="test-config.yaml"
TEST_PORT=3001
NOX_PID=""
FAILED_TESTS=0
TOTAL_TESTS=0

# ==============================================================================
# Test Utilities
# ==============================================================================

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[FAIL]${NC} $1"
    ((FAILED_TESTS++))
}

test_start() {
    ((TOTAL_TESTS++))
    log_info "Test $TOTAL_TESTS: $1"
}

setup_test_environment() {
    log_info "Setting up test environment..."
    
    # Create test config file
    cat > "$TEST_CONFIG_FILE" <<EOF
server:
  host: "127.0.0.1"
  port: $TEST_PORT

mock:
  scenarios:
    - name: "uat_tests"
      routes:
        - path: "/test/users"
          method: "GET"
          response:
            status: 200
            headers:
              Content-Type: "application/json"
            body: '{"users": [{"id": 1, "name": "Test User"}]}'
        
        - path: "/test/users/123"
          method: "GET"
          response:
            status: 200
            headers:
              Content-Type: "application/json"
            body: '{"id": 123, "name": "Test User", "email": "test@example.com"}'
        
        - path: "/test/posts"
          method: "POST"
          response:
            status: 201
            headers:
              Content-Type: "application/json"
            body: '{"id": 456, "title": "Test Post", "created": true}'
        
        - path: "/test/error"
          method: "GET"
          response:
            status: 500
            headers:
              Content-Type: "application/json"
            body: '{"error": "Test error", "code": 500}'
EOF

    # Build the project
    log_info "Building nox server..."
    cargo build --release --quiet
    
    log_success "Test environment setup complete"
}

teardown_test_environment() {
    log_info "Cleaning up test environment..."
    
    # Stop server if running
    stop_nox_server
    
    # Remove test files
    [ -f "$TEST_CONFIG_FILE" ] && rm "$TEST_CONFIG_FILE"
    
    log_success "Test environment cleaned up"
}

start_nox_server() {
    local config_file=${1:-$TEST_CONFIG_FILE}
    log_info "Starting nox server with config: $config_file"
    
    # Start server in background
    ./target/release/nox --config "$config_file" > nox.log 2>&1 &
    NOX_PID=$!
    
    # Wait for server to start
    sleep 2
    
    # Check if server is running
    if kill -0 $NOX_PID 2>/dev/null; then
        log_success "Nox server started (PID: $NOX_PID)"
        return 0
    else
        log_error "Failed to start nox server"
        return 1
    fi
}

stop_nox_server() {
    if [ -n "$NOX_PID" ] && kill -0 $NOX_PID 2>/dev/null; then
        log_info "Stopping nox server (PID: $NOX_PID)"
        kill $NOX_PID
        wait $NOX_PID 2>/dev/null || true
        NOX_PID=""
        log_success "Nox server stopped"
    fi
}

make_http_request() {
    local method="$1"
    local url="$2"
    local data="$3"
    local expected_status="$4"
    
    if [ "$method" = "POST" ] && [ -n "$data" ]; then
        response=$(curl -s -w "\n%{http_code}" -X "$method" \
                       -H "Content-Type: application/json" \
                       -d "$data" "$url" 2>/dev/null)
    else
        response=$(curl -s -w "\n%{http_code}" -X "$method" "$url" 2>/dev/null)
    fi
    
    body=$(echo "$response" | head -n -1)
    status_code=$(echo "$response" | tail -n 1)
    
    if [ "$status_code" = "$expected_status" ]; then
        return 0
    else
        return 1
    fi
}

validate_json_response() {
    local response="$1"
    local expected_field="$2"
    
    # Check if response is valid JSON
    echo "$response" | jq . >/dev/null 2>&1 || return 1
    
    # Check if expected field exists (if provided)
    if [ -n "$expected_field" ]; then
        echo "$response" | jq -e "$expected_field" >/dev/null 2>&1 || return 1
    fi
    
    return 0
}

# ==============================================================================
# Core Functionality Tests
# ==============================================================================

test_core_functionality() {
    log_info "=== CORE FUNCTIONALITY TESTS ==="
    
    # Test 1: Server startup with valid config
    test_start "Server startup with valid config"
    if start_nox_server; then
        log_success "Server started successfully"
    else
        log_error "Server failed to start"
        return 1
    fi
    
    # Test 2: Default health endpoint
    test_start "Default health endpoint"
    if make_http_request "GET" "http://127.0.0.1:$TEST_PORT/health" "" "200"; then
        log_success "Health endpoint responded correctly"
    else
        log_error "Health endpoint failed"
    fi
    
    # Test 3: Root endpoint
    test_start "Root endpoint with X-Server header"
    response=$(curl -s -I "http://127.0.0.1:$TEST_PORT/")
    if echo "$response" | grep -q "X-Server: NOX"; then
        log_success "Root endpoint has correct X-Server header"
    else
        log_error "Root endpoint missing X-Server header"
    fi
    
    # Test 4: Secret handshake endpoint
    test_start "Secret handshake endpoint"
    response=$(curl -s "http://127.0.0.1:$TEST_PORT/nox/handshake")
    if validate_json_response "$response" '.handshake' && \
       echo "$response" | jq -e '.handshake == "kick-nox-v1"' >/dev/null; then
        log_success "Handshake endpoint returned correct response"
    else
        log_error "Handshake endpoint failed"
    fi
    
    # Test 5: Mock GET endpoint
    test_start "Mock GET endpoint"
    response=$(curl -s "http://127.0.0.1:$TEST_PORT/test/users")
    if validate_json_response "$response" '.users' && \
       make_http_request "GET" "http://127.0.0.1:$TEST_PORT/test/users" "" "200"; then
        log_success "Mock GET endpoint working correctly"
    else
        log_error "Mock GET endpoint failed"
    fi
    
    # Test 6: Mock POST endpoint
    test_start "Mock POST endpoint"
    if make_http_request "POST" "http://127.0.0.1:$TEST_PORT/test/posts" '{"test": "data"}' "201"; then
        log_success "Mock POST endpoint working correctly"
    else
        log_error "Mock POST endpoint failed"
    fi
    
    # Test 7: Mock error endpoint
    test_start "Mock error endpoint (500)"
    if make_http_request "GET" "http://127.0.0.1:$TEST_PORT/test/error" "" "500"; then
        log_success "Mock error endpoint returned correct status"
    else
        log_error "Mock error endpoint failed"
    fi
    
    # Test 8: 404 for non-existent routes
    test_start "404 for non-existent routes"
    if make_http_request "GET" "http://127.0.0.1:$TEST_PORT/nonexistent" "" "404"; then
        log_success "Non-existent route returned 404"
    else
        log_error "Non-existent route did not return 404"
    fi
    
    stop_nox_server
}

# ==============================================================================
# Integration Tests
# ==============================================================================

test_integration_kick() {
    log_info "=== KICK INTEGRATION TESTS ==="
    
    start_nox_server
    
    # Test 1: Kick client basic GET request
    test_start "Kick client GET request"
    cd ../kick
    output=$(cargo run --bin kick get "http://127.0.0.1:$TEST_PORT/test/users" 2>/dev/null)
    cd - >/dev/null
    
    if echo "$output" | grep -q "Success"; then
        log_success "Kick client GET request successful"
    else
        log_error "Kick client GET request failed"
    fi
    
    # Test 2: Kick client handshake
    test_start "Kick client handshake"
    cd ../kick
    output=$(cargo run --bin kick get "http://127.0.0.1:$TEST_PORT/nox/handshake" 2>/dev/null)
    cd - >/dev/null
    
    if echo "$output" | grep -q "kick-nox-v1"; then
        log_success "Kick client handshake successful"
    else
        log_error "Kick client handshake failed"
    fi
    
    # Test 3: Kick client POST request
    test_start "Kick client POST request"
    cd ../kick
    output=$(cargo run --bin kick post --data '{"test": "data"}' "http://127.0.0.1:$TEST_PORT/test/posts" 2>/dev/null)
    cd - >/dev/null
    
    if echo "$output" | grep -q "Success"; then
        log_success "Kick client POST request successful"
    else
        log_error "Kick client POST request failed"
    fi
    
    stop_nox_server
}

# ==============================================================================
# Configuration Tests
# ==============================================================================

test_yaml_configuration() {
    log_info "=== YAML CONFIGURATION TESTS ==="
    
    # Test 1: Invalid YAML handling
    test_start "Invalid YAML handling"
    echo "invalid: yaml: content: [" > invalid-config.yaml
    
    ./target/release/nox --config invalid-config.yaml > error.log 2>&1 &
    local pid=$!
    sleep 1
    
    if kill -0 $pid 2>/dev/null; then
        kill $pid
        log_error "Server started with invalid YAML (should have failed)"
    else
        log_success "Server correctly rejected invalid YAML"
    fi
    
    rm -f invalid-config.yaml error.log
    
    # Test 2: Missing config file
    test_start "Missing config file handling"
    ./target/release/nox --config nonexistent.yaml > error.log 2>&1 &
    local pid=$!
    sleep 1
    
    if kill -0 $pid 2>/dev/null; then
        kill $pid
        log_error "Server started with missing config (should have failed)"
    else
        log_success "Server correctly handled missing config file"
    fi
    
    rm -f error.log
    
    # Test 3: Default config (no --config flag)
    test_start "Default configuration"
    ./target/release/nox > default.log 2>&1 &
    local pid=$!
    sleep 2
    
    if kill -0 $pid 2>/dev/null; then
        kill $pid
        log_success "Server started with default configuration"
    else
        log_error "Server failed to start with default configuration"
    fi
    
    rm -f default.log
}

# ==============================================================================
# Edge Case Tests
# ==============================================================================

test_edge_cases() {
    log_info "=== EDGE CASE TESTS ==="
    
    start_nox_server
    
    # Test 1: Very long URL
    test_start "Very long URL handling"
    long_path="/test/$(printf 'a%.0s' {1..1000})"
    if make_http_request "GET" "http://127.0.0.1:$TEST_PORT$long_path" "" "404"; then
        log_success "Long URL handled correctly"
    else
        log_error "Long URL handling failed"
    fi
    
    # Test 2: Large POST data
    test_start "Large POST data handling"
    large_data='{"data": "'$(printf 'x%.0s' {1..1000})'"}'
    if curl -s -X POST -d "$large_data" "http://127.0.0.1:$TEST_PORT/test/posts" >/dev/null 2>&1; then
        log_success "Large POST data handled"
    else
        log_error "Large POST data handling failed"
    fi
    
    # Test 3: Concurrent requests
    test_start "Concurrent request handling"
    concurrent_success=0
    for i in {1..5}; do
        curl -s "http://127.0.0.1:$TEST_PORT/test/users" >/dev/null 2>&1 &
    done
    wait
    
    # If we get here without the server crashing, it's a success
    if kill -0 $NOX_PID 2>/dev/null; then
        log_success "Concurrent requests handled successfully"
    else
        log_error "Server crashed under concurrent load"
    fi
    
    stop_nox_server
}

# ==============================================================================
# Main Test Runner
# ==============================================================================

print_summary() {
    echo
    log_info "=== TEST SUMMARY ==="
    echo "Total tests: $TOTAL_TESTS"
    echo "Failed tests: $FAILED_TESTS"
    echo "Passed tests: $((TOTAL_TESTS - FAILED_TESTS))"
    
    if [ $FAILED_TESTS -eq 0 ]; then
        log_success "ALL TESTS PASSED! 🎉"
        echo
        log_info "Nox server is ready for production deployment!"
        return 0
    else
        log_error "$FAILED_TESTS tests failed"
        echo
        log_error "Please fix failing tests before deployment"
        return 1
    fi
}

main() {
    log_info "Starting Nox Server UAT Tests..."
    echo
    
    # Setup
    setup_test_environment
    
    # Run test suites
    test_core_functionality
    test_integration_kick  
    test_yaml_configuration
    test_edge_cases
    
    # Cleanup and summary
    teardown_test_environment
    print_summary
}

# Run tests if script is executed directly
if [ "${BASH_SOURCE[0]}" == "${0}" ]; then
    main "$@"
fi