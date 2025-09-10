use nox::prelude::*;
use nox::plugins::{
    mock::MockPlugin,
    health::HealthPlugin,
};
use nox::session::{SessionManager, MemorySessionStore};
use nox::auth::{AuthManager, BasicAuthProvider};
use nox::handlers::{json_handler, text_handler, RouterBuilder};
use nox::server::{Route, Server};
use nox::config::*;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::init();
    
    println!("🚀 Starting Nox Server Complete Example");
    
    // Create comprehensive configuration
    let config = create_example_config();
    
    // Create server
    let server = Server::new(config.clone())?;
    
    // Setup plugins
    setup_example_plugins(&server, &config).await?;
    
    // Setup authentication
    let auth_manager = setup_example_auth();
    
    // Setup session management
    let session_manager = setup_example_sessions().await?;
    
    // Setup comprehensive routes
    setup_example_routes(&server, auth_manager, session_manager).await?;
    
    println!("✅ Server configured with:");
    println!("   - Mock API endpoints with templates");
    println!("   - Authentication (Basic Auth)");
    println!("   - Session management");
    println!("   - Health checks");
    println!("   - Custom handlers");
    println!();
    println!("🌐 Server starting on http://{}", config.bind_address());
    println!();
    println!("📍 Try these endpoints:");
    println!("   GET  /                    - Server info");
    println!("   GET  /health              - Health check");
    println!("   GET  /api/users           - List users (mock)");
    println!("   GET  /api/users/123       - Get user by ID (mock)");
    println!("   POST /api/users           - Create user (mock)");
    println!("   GET  /api/slow            - Slow response simulation");
    println!("   GET  /api/error/500       - Error simulation");
    println!("   GET  /custom/time         - Custom handler");
    println!("   GET  /custom/auth         - Authenticated endpoint");
    println!();
    println!("🔐 Authentication:");
    println!("   Username: admin");
    println!("   Password: secret123");
    println!();
    println!("Press Ctrl+C to stop the server");
    
    // Setup graceful shutdown
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
        println!("\n🛑 Shutdown signal received");
        let _ = shutdown_tx.send(());
    });
    
    // Start server in background
    let server_handle = {
        let server_clone = server.clone();
        tokio::spawn(async move {
            if let Err(e) = server_clone.start().await {
                eprintln!("❌ Server error: {}", e);
            }
        })
    };
    
    // Wait for shutdown signal
    let _ = shutdown_rx.await;
    
    println!("🔄 Shutting down server...");
    server.shutdown().await?;
    server_handle.abort();
    
    println!("✅ Server stopped gracefully");
    Ok(())
}

/// Create a comprehensive example configuration
fn create_example_config() -> Config {
    let mut config = Config::default();
    
    // Server settings
    config.server.host = "127.0.0.1".to_string();
    config.server.port = 8080;
    
    // Logging settings
    config.logging.level = "info".to_string();
    config.logging.request_logging = true;
    
    // Enable plugins
    config.plugins.enabled = vec![
        "mock".to_string(),
        "health".to_string(),
    ];
    
    // Mock scenarios with comprehensive examples
    config.mock.scenarios = vec![
        MockScenario {
            name: "user_api".to_string(),
            enabled: Some(true),
            routes: vec![
                // List users
                MockRoute {
                    path: "/api/users".to_string(),
                    method: Some("GET".to_string()),
                    headers: None,
                    query: None,
                    body_pattern: None,
                    response: MockResponse {
                        status: 200,
                        headers: Some(vec![
                            ("content-type".to_string(), "application/json".to_string()),
                            ("x-api-version".to_string(), "v1".to_string()),
                        ].into_iter().collect()),
                        body: Some(r#"
{
  "users": [
    {
      "id": "{{random 'int' 1 100}}",
      "name": "{{fake_data 'name'}}",
      "email": "{{fake_data 'email'}}",
      "created_at": "{{timestamp 'iso8601'}}"
    },
    {
      "id": "{{random 'int' 101 200}}",
      "name": "{{fake_data 'name'}}",
      "email": "{{fake_data 'email'}}",
      "created_at": "{{timestamp 'iso8601'}}"
    }
  ],
  "total": 2,
  "page": "{{query.page}}",
  "limit": "{{query.limit}}",
  "generated_at": "{{timestamp}}"
}
                        "#.to_string()),
                        body_file: None,
                        delay: Some(100),
                        template: Some(true),
                    },
                },
                
                // Get user by ID
                MockRoute {
                    path: "/api/users/{id}".to_string(),
                    method: Some("GET".to_string()),
                    headers: None,
                    query: None,
                    body_pattern: None,
                    response: MockResponse {
                        status: 200,
                        headers: Some(vec![
                            ("content-type".to_string(), "application/json".to_string()),
                        ].into_iter().collect()),
                        body: Some(r#"
{
  "id": "{{path.id}}",
  "name": "{{fake_data 'name'}}",
  "email": "{{fake_data 'email'}}",
  "phone": "{{fake_data 'phone'}}",
  "address": "{{fake_data 'address'}}",
  "company": "{{fake_data 'company'}}",
  "created_at": "{{timestamp 'iso8601'}}",
  "last_login": "{{timestamp}}",
  "is_active": "{{random 'bool'}}",
  "profile_id": "{{uuid}}"
}
                        "#.to_string()),
                        body_file: None,
                        delay: Some(50),
                        template: Some(true),
                    },
                },
                
                // Create user
                MockRoute {
                    path: "/api/users".to_string(),
                    method: Some("POST".to_string()),
                    headers: None,
                    query: None,
                    body_pattern: None,
                    response: MockResponse {
                        status: 201,
                        headers: Some(vec![
                            ("content-type".to_string(), "application/json".to_string()),
                            ("location".to_string(), "/api/users/{{random 'int' 1000 9999}}".to_string()),
                        ].into_iter().collect()),
                        body: Some(r#"
{
  "id": "{{random 'int' 1000 9999}}",
  "name": "{{fake_data 'name'}}",
  "email": "{{fake_data 'email'}}",
  "created_at": "{{timestamp}}",
  "status": "created",
  "message": "User created successfully"
}
                        "#.to_string()),
                        body_file: None,
                        delay: Some(200),
                        template: Some(true),
                    },
                },
            ],
        },
        
        // Error simulation scenarios
        MockScenario {
            name: "error_simulation".to_string(),
            enabled: Some(true),
            routes: vec![
                // Slow response
                MockRoute {
                    path: "/api/slow".to_string(),
                    method: Some("GET".to_string()),
                    headers: None,
                    query: None,
                    body_pattern: None,
                    response: MockResponse {
                        status: 200,
                        headers: Some(vec![
                            ("content-type".to_string(), "application/json".to_string()),
                        ].into_iter().collect()),
                        body: Some(r#"
{
  "message": "This response was intentionally delayed",
  "delay_ms": 2000,
  "timestamp": "{{timestamp}}",
  "request_id": "{{uuid}}"
}
                        "#.to_string()),
                        body_file: None,
                        delay: Some(2000),
                        template: Some(true),
                    },
                },
                
                // Server error
                MockRoute {
                    path: "/api/error/500".to_string(),
                    method: Some("GET".to_string()),
                    headers: None,
                    query: None,
                    body_pattern: None,
                    response: MockResponse {
                        status: 500,
                        headers: Some(vec![
                            ("content-type".to_string(), "application/json".to_string()),
                        ].into_iter().collect()),
                        body: Some(r#"
{
  "error": {
    "code": "INTERNAL_SERVER_ERROR",
    "message": "Something went wrong on our end",
    "timestamp": "{{timestamp}}",
    "request_id": "{{uuid}}"
  }
}
                        "#.to_string()),
                        body_file: None,
                        delay: None,
                        template: Some(true),
                    },
                },
            ],
        },
    ];
    
    // Authentication
    config.auth.strategy = "basic".to_string();
    config.auth.realm = Some("Nox Server Example".to_string());
    config.auth.users = Some(vec![
        ("admin".to_string(), "secret123".to_string()),
        ("user".to_string(), "password456".to_string()),
    ].into_iter().collect());
    
    // Session management
    config.session.storage = "memory".to_string();
    config.session.timeout = 1800; // 30 minutes
    
    // Health checks
    config.health.enabled = true;
    config.health.detailed = true;
    
    config
}

/// Setup example plugins
async fn setup_example_plugins(server: &Server, config: &Config) -> Result<()> {
    // Mock plugin
    let mut mock_plugin = MockPlugin::new();
    let mock_config = serde_yaml::to_value(&config.mock).unwrap();
    mock_plugin.initialize(&mock_config).await?;
    server.register_plugin(Arc::new(mock_plugin)).await?;
    
    // Health plugin
    let mut health_plugin = HealthPlugin::new();
    let health_config = serde_yaml::to_value(&config.health).unwrap();
    health_plugin.initialize(&health_config).await?;
    server.register_plugin(Arc::new(health_plugin)).await?;
    
    Ok(())
}

/// Setup example authentication
fn setup_example_auth() -> AuthManager {
    let mut users = HashMap::new();
    users.insert("admin".to_string(), "secret123".to_string());
    users.insert("user".to_string(), "password456".to_string());
    
    AuthManager::basic(users, Some("Nox Server Example".to_string()))
}

/// Setup example session management
async fn setup_example_sessions() -> Result<SessionManager> {
    let store = Box::new(MemorySessionStore::new());
    let session_manager = SessionManager::new(
        store,
        Duration::from_secs(1800), // 30 minutes
        "example_session".to_string(),
        false, // not secure for HTTP
        true,  // HTTP only
    );
    
    Ok(session_manager)
}

/// Setup comprehensive example routes
async fn setup_example_routes(
    server: &Server,
    _auth_manager: AuthManager,
    _session_manager: SessionManager,
) -> Result<()> {
    let mut builder = RouterBuilder::new();
    
    // Root endpoint with server information
    builder = builder.get(
        "/",
        Arc::new(json_handler("root", serde_json::json!({
            "name": "Nox Server",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "A lightweight, modular HTTP API server",
            "status": "running",
            "features": [
                "Mock responses with templates",
                "Multiple authentication strategies",
                "Session management",
                "Health checks",
                "Plugin system",
                "Hot configuration reload"
            ],
            "endpoints": {
                "health": "/health",
                "users": "/api/users",
                "slow_response": "/api/slow",
                "error_simulation": "/api/error/500"
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        })))
    );
    
    // Custom time endpoint
    builder = builder.get(
        "/custom/time",
        Arc::new(json_handler("current_time", serde_json::json!({
            "current_time": chrono::Utc::now().to_rfc3339(),
            "timezone": "UTC",
            "unix_timestamp": chrono::Utc::now().timestamp(),
            "formatted": chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string()
        })))
    );
    
    // Custom authenticated endpoint (would need auth middleware in real implementation)
    builder = builder.get(
        "/custom/auth",
        Arc::new(json_handler("auth_info", serde_json::json!({
            "message": "This endpoint would require authentication",
            "note": "In a complete implementation, this would be protected by auth middleware",
            "timestamp": chrono::Utc::now().to_rfc3339()
        })))
    );
    
    // Ping endpoint
    builder = builder.get(
        "/ping",
        Arc::new(text_handler("ping", "pong"))
    );
    
    // Version endpoint
    builder = builder.get(
        "/version",
        Arc::new(json_handler("version", serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "build_time": env!("BUILD_TIME").unwrap_or("unknown"),
            "git_commit": env!("GIT_COMMIT").unwrap_or("unknown")
        })))
    );
    
    // Apply all routes to the server
    builder.build_into(&server.router).await?;
    
    Ok(())
}