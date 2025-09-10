#!/bin/bash
# deploy.sh - Nox Server Deployment Script
# Similar to kick project deployment patterns

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PROJECT_NAME="nox"
VERSION=$(grep '^version = ' Cargo.toml | cut -d'"' -f2)
BUILD_DIR="target/release"
DEPLOY_DIR="${DEPLOY_DIR:-/opt/nox}"
SERVICE_NAME="nox-server"
CONFIG_FILE="${CONFIG_FILE:-nox.yaml}"

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

show_usage() {
    cat << EOF
Nox Server Deployment Script

Usage: $0 [COMMAND]

Commands:
    build       Build optimized release binary
    test        Run all tests before deployment
    package     Create deployment package
    install     Install to system location
    service     Install systemd service
    start       Start nox service
    stop        Stop nox service
    status      Check service status
    clean       Clean build artifacts
    help        Show this help message

Environment Variables:
    DEPLOY_DIR    Target installation directory (default: /opt/nox)
    CONFIG_FILE   Configuration file name (default: nox.yaml)

Examples:
    $0 build      # Build release binary
    $0 test       # Run comprehensive tests
    $0 install    # Install to /opt/nox
    $0 service    # Install systemd service
EOF
}

check_dependencies() {
    log_info "Checking dependencies..."
    
    # Check Rust toolchain
    if ! command -v cargo &> /dev/null; then
        log_error "Rust/Cargo not found. Please install Rust toolchain."
    fi
    
    # Check required tools
    for tool in jq curl; do
        if ! command -v $tool &> /dev/null; then
            log_warning "$tool not found. Some features may not work."
        fi
    done
    
    log_success "Dependencies check complete"
}

build_release() {
    log_info "Building optimized release binary..."
    
    # Clean previous builds
    cargo clean
    
    # Build with optimizations
    cargo build --release --features config
    
    # Verify binary
    if [ ! -f "$BUILD_DIR/nox" ]; then
        log_error "Build failed - binary not found"
    fi
    
    # Get binary info
    local binary_size=$(du -h "$BUILD_DIR/nox" | cut -f1)
    log_success "Build complete - Binary size: $binary_size"
}

run_tests() {
    log_info "Running comprehensive tests..."
    
    # Run unit tests
    log_info "Running unit tests..."
    cargo test --release
    
    # Run UAT tests
    if [ -f "test-basic.sh" ]; then
        log_info "Running UAT tests..."
        ./test-basic.sh
    else
        log_warning "UAT script not found, skipping integration tests"
    fi
    
    log_success "All tests passed"
}

create_package() {
    log_info "Creating deployment package..."
    
    local package_dir="nox-${VERSION}"
    
    # Create package directory
    mkdir -p "$package_dir"
    
    # Copy binary
    cp "$BUILD_DIR/nox" "$package_dir/"
    
    # Copy configuration files
    if [ -f "mock-config.yaml" ]; then
        cp "mock-config.yaml" "$package_dir/nox.yaml.example"
    fi
    
    # Copy documentation
    cp README.md "$package_dir/" 2>/dev/null || true
    
    # Copy deployment scripts
    cp deploy.sh "$package_dir/" 2>/dev/null || true
    cp uat.sh "$package_dir/" 2>/dev/null || true
    
    # Create archive
    tar czf "${package_dir}.tar.gz" "$package_dir"
    
    # Cleanup
    rm -rf "$package_dir"
    
    log_success "Package created: ${package_dir}.tar.gz"
}

install_system() {
    log_info "Installing nox to system location..."
    
    # Check if running as root for system installation
    if [ "$EUID" -ne 0 ] && [ "$DEPLOY_DIR" = "/opt/nox" ]; then
        log_error "System installation requires root privileges. Use sudo."
    fi
    
    # Create directories
    mkdir -p "$DEPLOY_DIR/bin"
    mkdir -p "$DEPLOY_DIR/config"
    mkdir -p "$DEPLOY_DIR/logs"
    
    # Copy binary
    cp "$BUILD_DIR/nox" "$DEPLOY_DIR/bin/"
    chmod +x "$DEPLOY_DIR/bin/nox"
    
    # Copy example config
    if [ -f "mock-config.yaml" ] && [ ! -f "$DEPLOY_DIR/config/$CONFIG_FILE" ]; then
        cp "mock-config.yaml" "$DEPLOY_DIR/config/$CONFIG_FILE"
    fi
    
    # Create symlink for global access
    if [ "$DEPLOY_DIR" = "/opt/nox" ]; then
        ln -sf "$DEPLOY_DIR/bin/nox" /usr/local/bin/nox
    fi
    
    log_success "Installation complete at $DEPLOY_DIR"
}

install_service() {
    log_info "Installing systemd service..."
    
    if [ "$EUID" -ne 0 ]; then
        log_error "Service installation requires root privileges. Use sudo."
    fi
    
    # Create systemd service file
    cat > "/etc/systemd/system/${SERVICE_NAME}.service" << EOF
[Unit]
Description=Nox Mock Server
After=network.target
Wants=network.target

[Service]
Type=simple
User=nox
Group=nox
WorkingDirectory=${DEPLOY_DIR}
ExecStart=${DEPLOY_DIR}/bin/nox --config ${DEPLOY_DIR}/config/${CONFIG_FILE}
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=${DEPLOY_DIR}/logs

[Install]
WantedBy=multi-user.target
EOF
    
    # Create nox user if it doesn't exist
    if ! id "nox" &>/dev/null; then
        useradd --system --home-dir "$DEPLOY_DIR" --shell /bin/false nox
    fi
    
    # Set permissions
    chown -R nox:nox "$DEPLOY_DIR"
    
    # Reload systemd
    systemctl daemon-reload
    systemctl enable "$SERVICE_NAME"
    
    log_success "Service installed and enabled"
}

service_start() {
    log_info "Starting nox service..."
    systemctl start "$SERVICE_NAME"
    log_success "Service started"
}

service_stop() {
    log_info "Stopping nox service..."
    systemctl stop "$SERVICE_NAME"
    log_success "Service stopped"
}

service_status() {
    log_info "Checking service status..."
    systemctl status "$SERVICE_NAME" --no-pager
}

health_check() {
    log_info "Performing health check..."
    
    local endpoint="http://127.0.0.1:3000/health"
    local max_attempts=5
    local attempt=1
    
    while [ $attempt -le $max_attempts ]; do
        if curl -sf "$endpoint" >/dev/null 2>&1; then
            log_success "Health check passed"
            return 0
        fi
        
        log_info "Attempt $attempt/$max_attempts failed, retrying..."
        sleep 2
        ((attempt++))
    done
    
    log_error "Health check failed after $max_attempts attempts"
}

clean_build() {
    log_info "Cleaning build artifacts..."
    
    cargo clean
    rm -f nox-*.tar.gz
    rm -f nox.log
    rm -f test-config.yaml
    
    log_success "Cleanup complete"
}

# Main command handler
case "${1:-help}" in
    build)
        check_dependencies
        build_release
        ;;
    test)
        check_dependencies
        build_release
        run_tests
        ;;
    package)
        check_dependencies
        build_release
        run_tests
        create_package
        ;;
    install)
        check_dependencies
        build_release
        install_system
        ;;
    service)
        install_service
        ;;
    start)
        service_start
        health_check
        ;;
    stop)
        service_stop
        ;;
    status)
        service_status
        ;;
    clean)
        clean_build
        ;;
    help|--help|-h)
        show_usage
        ;;
    *)
        log_error "Unknown command: $1"
        show_usage
        exit 1
        ;;
esac