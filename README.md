# Nox (Server)

A lightweight, modular HTTP API server built with Rust and Hyper, designed for maximum extensibility through a plugin architecture. Perfect for mocking APIs, testing, and growing into production scenarios.

## Features

🚀 **High Performance**: Built on Hyper for async HTTP/1.1 and HTTP/2 support  
🔌 **Plugin System**: Extensible architecture with plugin hooks  
📝 **Mock Server**: YAML-configured response templates with dynamic generation  
🔐 **Authentication**: Multiple strategies (Basic, Bearer, API Key)  
🍪 **Session Management**: File, SQLite, Redis, and memory storage options  
📊 **Health Checks**: Built-in health and metrics endpoints  
⚙️ **Configurable**: YAML configuration with hot-reload  
🛠️ **CLI Daemon**: Process management with start/stop/restart/status  
📁 **Static Files**: Serve static content with caching  
🔄 **Proxying**: Load balancing and upstream proxying  
🏗️ **Extensible**: Easy to add custom handlers and middleware  

## Quick Start

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd nox_server

# Build the server
cargo build --release

# Install globally (optional)
cargo install --path .
```

### Basic Usage

```bash
# Start server with default configuration
nox start

# Start in foreground mode (for development)
nox start --foreground

# Stop the server
nox stop

# Check server status
nox status

# Restart server
nox restart

# View logs
nox logs --follow

# Validate configuration
nox config

# Export default configuration
nox config --export --file my-config.yaml
```

### Configuration

Create a `nox.yaml` configuration file:

```yaml
server:
  host: "127.0.0.1"
  port: 8080

logging:
  level: "info"
  format: "text"

plugins:
  enabled:
    - "mock"
    - "health"

mock:
  scenarios:
    - name: "user_api"
      routes:
        - path: "/users/{id}"
          method: "GET"
          response:
            status: 200
            headers:
              content-type: "application/json"
            body: |
              {
                "id": "{{path.id}}",
                "name": "{{fake_data 'name'}}",
                "email": "{{fake_data 'email'}}"
              }
            template: true
```

## Core Concepts

### Plugin System

The server uses a hook-based plugin system with these lifecycle points:

- **OnStartup**: Server initialization
- **OnShutdown**: Server cleanup
- **PreRequest**: Before routing
- **PostRoute**: After route matching
- **PreHandler**: Before handler execution
- **PostHandler**: After handler execution
- **PreResponse**: Before response sent
- **PostResponse**: After response sent
- **OnError**: Error handling

### Mock Server

The mock server supports:

- **Path Parameters**: `/users/{id}` extracts `id` parameter
- **Dynamic Templates**: Use Handlebars templating with helpers
- **Request Matching**: Match by method, headers, query params, body
- **Response Delays**: Simulate network latency
- **Template Helpers**: Built-in helpers for fake data generation

#### Template Helpers

```handlebars
{{uuid}}                          <!-- Generate UUID -->
{{timestamp}}                     <!-- Current timestamp -->
{{timestamp 'iso8601'}}          <!-- Formatted timestamp -->
{{random 'int' 1 100}}           <!-- Random integer -->
{{random 'string' 10}}           <!-- Random string -->
{{fake_data 'name'}}             <!-- Fake person name -->
{{fake_data 'email'}}            <!-- Fake email address -->
{{fake_data 'company'}}          <!-- Fake company name -->
{{base64 'text'}}                <!-- Base64 encode -->
{{url_encode 'text'}}            <!-- URL encode -->
{{json object}}                  <!-- JSON stringify -->
```

### Authentication

Supports multiple authentication strategies:

#### None (Open Access)
```yaml
auth:
  strategy: "none"
```

#### Basic Authentication
```yaml
auth:
  strategy: "basic"
  realm: "API"
  users:
    admin: "secret123"
    user: "password456"
```

#### Bearer Token
```yaml
auth:
  strategy: "bearer"
  users:
    admin: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"
    user: "dXNlci10b2tlbi0xMjM0NTY"
```

#### API Key
```yaml
auth:
  strategy: "api_key"
  header_name: "X-API-Key"
  api_keys:
    - "api-key-12345"
    - "api-key-67890"
```

### Session Management

Multiple storage backends:

#### Memory (Development)
```yaml
session:
  storage: "memory"
  timeout: 3600
```

#### File-based
```yaml
session:
  storage: "file"
  storage_config:
    file_path: "./sessions.json"
```

#### SQLite
```yaml
session:
  storage: "sqlite"
  storage_config:
    db_path: "./sessions.db"
```

#### Redis
```yaml
session:
  storage: "redis"
  storage_config:
    redis_url: "redis://127.0.0.1:6379"
```

## CLI Commands

### Server Management

```bash
# Start server as daemon
nox start

# Start in foreground (development)
nox start --foreground

# Stop server gracefully
nox stop

# Force stop server
nox stop --force

# Restart server
nox restart

# Check server status
nox status
```

### Configuration

```bash
# Validate configuration
nox config

# Show effective configuration
nox config --show

# Export default configuration
nox config --export --file default.yaml

# Use custom config file
nox --config custom.yaml start
```

### Monitoring

```bash
# View server logs
nox logs

# Follow logs (like tail -f)
nox logs --follow

# Show last 50 lines
nox logs --lines 50

# Check health status
nox health

# Show detailed health info
nox health --detailed

# List enabled plugins
nox plugins

# Show detailed plugin info
nox plugins --detailed
```

### Session Management

```bash
# List active sessions
nox sessions list

# Show session details
nox sessions show <session-id>

# Delete session
nox sessions delete <session-id>

# Clean up expired sessions
nox sessions cleanup

# Show session statistics
nox sessions stats
```

## Built-in Endpoints

### Health Checks

- `GET /health` - Basic health check
- `GET /health/ready` - Readiness check
- `GET /health/metrics` - Prometheus-style metrics

### Information

- `GET /` - Server information
- `GET /version` - Version information
- `GET /ping` - Simple ping endpoint

## Advanced Usage

### Custom Handlers

Create custom handlers by implementing the `Handler` trait:

```rust
use nox::prelude::*;

struct CustomHandler;

#[async_trait]
impl Handler for CustomHandler {
    async fn handle(&self, request: &Request<Body>, context: &PluginContext) -> Result<HandlerResult> {
        let response = Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .body(Body::from(r#"{"message": "Hello from custom handler!"}"#))
            .unwrap();
        
        Ok(HandlerResult::Response(response))
    }
    
    fn name(&self) -> &str {
        "custom"
    }
}
```

### Custom Plugins

Create plugins by implementing the `Plugin` trait:

```rust
use nox::prelude::*;

struct MetricsPlugin {
    request_count: AtomicU64,
}

#[async_trait]
impl Plugin for MetricsPlugin {
    fn name(&self) -> &str {
        "metrics"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn description(&self) -> &str {
        "Collects request metrics"
    }
    
    async fn initialize(&mut self, config: &serde_yaml::Value) -> Result<()> {
        Ok(())
    }
    
    fn handles_hook(&self, hook: &PluginHook) -> bool {
        matches!(hook, PluginHook::PreRequest)
    }
    
    async fn pre_request(
        &self,
        request: &mut Request<Body>,
        context: &PluginContext,
    ) -> Result<PluginResult> {
        self.request_count.fetch_add(1, Ordering::Relaxed);
        Ok(PluginResult::Continue)
    }
}
```

### Programmatic Usage

```rust
use nox::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let mut config = Config::default();
    config.server.port = 3000;
    
    let server = Server::new(config)?;
    
    // Register plugins
    server.register_plugin(Arc::new(MockPlugin::new())).await?;
    server.register_plugin(Arc::new(HealthPlugin::new())).await?;
    
    // Add custom routes
    server.add_route(
        Route::get("/custom"),
        Arc::new(JsonHandler::new("custom", serde_json::json!({
            "message": "Custom endpoint"
        })))
    ).await?;
    
    // Start server
    server.start().await?;
    
    Ok(())
}
```

## Configuration Reference

### Server Settings

```yaml
server:
  host: "127.0.0.1"           # Bind address
  port: 8080                  # Port number
  workers: 4                  # Number of worker threads
  max_connections: 1000       # Maximum concurrent connections
  request_timeout: 30         # Request timeout in seconds
  keep_alive_timeout: 60      # Keep-alive timeout in seconds
  tls:                        # Optional TLS configuration
    cert_path: "cert.pem"
    key_path: "key.pem"
```

### Logging Settings

```yaml
logging:
  level: "info"               # trace, debug, info, warn, error
  format: "text"              # text or json
  file: "server.log"          # Optional log file
  request_logging: true       # Log HTTP requests
```

### Plugin Configuration

```yaml
plugins:
  enabled:                    # List of enabled plugins
    - "mock"
    - "health"
    - "auth"
  config:                     # Plugin-specific configuration
    mock:
      default_delay: 100
    auth:
      strategy: "bearer"
```

## Docker Usage

```dockerfile
FROM rust:alpine AS builder
COPY . /app
WORKDIR /app
RUN cargo build --release

FROM alpine:latest
RUN apk add --no-cache ca-certificates
COPY --from=builder /app/target/release/nox /usr/local/bin/
COPY nox.yaml /etc/nox/
EXPOSE 8080
CMD ["nox", "--config", "/etc/nox/nox.yaml", "start", "--foreground"]
```

```bash
# Build and run
docker build -t nox .
docker run -p 8080:8080 -v $(pwd)/config.yaml:/etc/nox/nox.yaml nox
```

## Development

### Building from Source

```bash
# Clone repository
git clone <repository-url>
cd nox_server

# Build with all features
cargo build --features sqlite,redis

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run -- start --foreground

# Check code formatting
cargo fmt --check

# Run linter
cargo clippy
```

### Project Structure

```
src/
├── lib.rs              # Library exports
├── server/             # Core HTTP server
│   ├── mod.rs
│   └── router.rs
├── plugins/            # Plugin system
│   ├── mod.rs
│   ├── manager.rs
│   ├── mock.rs
│   ├── health.rs
│   └── auth.rs
├── session/            # Session management
│   ├── mod.rs
│   ├── memory.rs
│   ├── file.rs
│   └── sqlite.rs
├── auth/               # Authentication
│   ├── mod.rs
│   ├── basic.rs
│   ├── bearer.rs
│   └── api_key.rs
├── handlers/           # Request handlers
│   ├── mod.rs
│   ├── static_files.rs
│   └── proxy.rs
├── utils/              # Utilities
│   ├── mod.rs
│   ├── templates.rs
│   └── logging.rs
├── cli/                # CLI interface
│   ├── mod.rs
│   ├── daemon.rs
│   └── commands.rs
├── config.rs           # Configuration
├── error.rs            # Error types
└── main.rs             # Entry point
```

## Use Cases

### 1. API Mocking for Development

Perfect for frontend development when backend APIs aren't ready:

```yaml
mock:
  scenarios:
    - name: "user_service"
      routes:
        - path: "/api/users"
          response:
            body: '[{"id": 1, "name": "John"}, {"id": 2, "name": "Jane"}]'
```

### 2. Testing HTTP Clients

Test how your HTTP clients handle various scenarios:

```yaml
mock:
  scenarios:
    - name: "error_testing"
      routes:
        - path: "/api/error/500"
          response:
            status: 500
            delay: 1000
        - path: "/api/error/timeout"
          response:
            status: 200
            delay: 30000
```

### 3. Service Prototyping

Quickly prototype API designs:

```yaml
mock:
  scenarios:
    - name: "product_api"
      routes:
        - path: "/products/{category}"
          response:
            body: |
              {
                "category": "{{path.category}}",
                "products": [
                  {"id": "{{random 'int' 1000 9999}}", "name": "{{fake_data 'name'}}"}
                ]
              }
            template: true
```

### 4. Load Testing Target

Use as a target for load testing tools:

```bash
# Start server optimized for load testing
nox --config load-test.yaml start
```

### 5. Integration Testing

Use in CI/CD pipelines for integration tests:

```bash
# Start mock server in background
nox start &
sleep 2

# Run integration tests
npm test

# Stop server
nox stop
```

## Performance

- **Async Architecture**: Built on Tokio for excellent concurrency
- **Low Memory**: Minimal memory footprint
- **Fast Startup**: Starts in milliseconds
- **High Throughput**: Handles thousands of requests per second

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Run `cargo test` and `cargo clippy`
6. Submit a pull request

## License

MIT License - see LICENSE file for details.

## Roadmap

- [ ] WebSocket support
- [ ] GraphQL integration
- [ ] gRPC support
- [ ] OAuth2 implementation
- [ ] Rate limiting
- [ ] Circuit breaker
- [ ] Request/response transformation
- [ ] Plugin marketplace
- [ ] Web UI for configuration
- [ ] Kubernetes operator
- [ ] Terraform provider

This Nox server provides a solid foundation that can grow from simple mocking to complex production scenarios while maintaining simplicity and extensibility!
