use mini_api_server::prelude::*;
use mini_api_server::cli::CliApp;
use mini_api_server::plugins::{
    mock::MockPlugin,
    health::HealthPlugin,
    auth::{AuthProviderFactory, AuthManager},
};
use mini_api_server::session::{SessionManager, SessionStoreFactory};
use mini_api_server::handlers::{json_handler, text_handler, RouterBuilder};
use mini_api_server::server::{Route, Server};
use mini_api_server::utils::setup_logging;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize CLI application
    let cli_app = CliApp::new()?;
    
    // Check if this is a daemon start command
    if std::env::args().any(|arg| arg == "start" && std::env::args().any(|a| a == "--foreground")) {
        // This is the daemon process, start the actual server
        return start_server().await;
    }
    
    // Run CLI commands
    cli_app.run().await
}

/// Start the actual server (called by daemon or foreground mode)
async fn start_server() -> Result<()> {
    // Load configuration
    let config = Config::load()?;
    
    // Setup logging
    setup_logging(&config.logging)?;
    
    tracing::info!("Starting Mini API Server v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!("Configuration loaded");
    
    // Create server
    let mut server = Server::new(config.clone())?;
    
    // Setup plugins
    setup_plugins(&server, &config).await?;
    
    // Setup authentication
    let auth_manager = setup_authentication(&config)?;
    
    // Setup session management
    let session_manager = setup_sessions(&config).await?;
    
    // Setup routes
    setup_routes(&server, &config, auth_manager, session_manager).await?;
    
    // Setup graceful shutdown
    let shutdown_signal = setup_shutdown_signal();
    
    // Start server
    tracing::info!("Server starting on {}", config.bind_address());
    
    tokio::select! {
        result = server.start() => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
                return Err(e);
            }
        }
        _ = shutdown_signal => {
            tracing::info!("Shutdown signal received");
        }
    }
    
    // Graceful shutdown
    tracing::info!("Shutting down server...");
    server.shutdown().await?;
    tracing::info!("Server shutdown complete");
    
    Ok(())
}

/// Setup plugins based on configuration
async fn setup_plugins(server: &Server, config: &Config) -> Result<()> {
    // Register mock plugin if enabled
    if config.plugins.enabled.contains(&"mock".to_string()) {
        let mut mock_plugin = MockPlugin::new();
        let mock_config = config.plugins.config.get("mock")
            .cloned()
            .unwrap_or_else(|| serde_yaml::to_value(&config.mock).unwrap());
        mock_plugin.initialize(&mock_config).await?;
        server.register_plugin(Arc::new(mock_plugin)).await?;
        tracing::info!("Mock plugin registered");
    }
    
    // Register health plugin if enabled
    if config.plugins.enabled.contains(&"health".to_string()) {
        let mut health_plugin = HealthPlugin::new();
        let health_config = config.plugins.config.get("health")
            .cloned()
            .unwrap_or_else(|| serde_yaml::to_value(&config.health).unwrap());
        health_plugin.initialize(&health_config).await?;
        server.register_plugin(Arc::new(health_plugin)).await?;
        tracing::info!("Health plugin registered");
    }
    
    Ok(())
}

/// Setup authentication based on configuration
fn setup_authentication(config: &Config) -> Result<AuthManager> {
    let auth_manager = AuthProviderFactory::create_manager(&config.auth)?;
    tracing::info!("Authentication configured: {} (required: {})", 
        auth_manager.provider_name(), 
        auth_manager.is_required()
    );
    Ok(auth_manager)
}

/// Setup session management based on configuration
async fn setup_sessions(config: &Config) -> Result<SessionManager> {
    let store = SessionStoreFactory::create_store(
        &config.session.storage,
        &config.session.storage_config,
    )?;
    
    let session_manager = SessionManager::new(
        store,
        config.session_timeout(),
        config.session.cookie_name.clone(),
        config.session.cookie_secure,
        config.session.cookie_http_only,
    );
    
    tracing::info!("Session management configured: {} storage", config.session.storage);
    Ok(session_manager)
}

/// Setup routes based on configuration
async fn setup_routes(
    server: &Server,
    config: &Config,
    _auth_manager: AuthManager,
    _session_manager: SessionManager,
) -> Result<()> {
    let mut builder = RouterBuilder::new();
    
    // Add default routes
    builder = builder
        .get("/", Arc::new(json_handler("root", serde_json::json!({
            "name": "Mini API Server",
            "version": env!("CARGO_PKG_VERSION"),
            "status": "running"
        }))))
        .get("/version", Arc::new(json_handler("version", serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "build_time": env!("BUILD_TIME").unwrap_or("unknown")
        }))))
        .get("/ping", Arc::new(text_handler("ping", "pong")));
    
    // Add static file serving if enabled
    if let Some(static_config) = &config.static_files {
        if static_config.enabled {
            // Note: StaticFileHandler implementation would go here
            tracing::info!("Static file serving would be enabled for: {:?}", static_config.root_dir);
        }
    }
    
    // Add proxy routes if enabled
    if let Some(proxy_config) = &config.proxy {
        if proxy_config.enabled {
            // Note: ProxyHandler implementation would go here
            tracing::info!("Proxy would be enabled for {} upstreams", proxy_config.upstreams.len());
        }
    }
    
    // Apply routes to server
    builder.build_into(server.router).await?;
    
    tracing::info!("Routes configured");
    Ok(())
}

/// Setup graceful shutdown signal handling
async fn setup_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        
        let mut sigterm = signal(SignalKind::terminate()).expect("Failed to setup SIGTERM handler");
        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to setup SIGINT handler");
        
        tokio::select! {
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM");
            }
            _ = sigint.recv() => {
                tracing::info!("Received SIGINT");
            }
        }
    }
    
    #[cfg(windows)]
    {
        tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
        tracing::info!("Received Ctrl+C");
    }
}

/// Example of how to create a custom server with specific configuration
#[allow(dead_code)]
async fn example_custom_server() -> Result<()> {
    // Create custom configuration
    let mut config = Config::default();
    config.server.port = 3000;
    config.server.host = "0.0.0.0".to_string();
    config.logging.level = "debug".to_string();
    
    // Add mock scenarios
    config.mock.scenarios = vec![
        mini_api_server::config::MockScenario {
            name: "user_api".to_string(),
            enabled: Some(true),
            routes: vec![
                mini_api_server::config::MockRoute {
                    path: "/users/{id}".to_string(),
                    method: Some("GET".to_string()),
                    headers: None,
                    query: None,
                    body_pattern: None,
                    response: mini_api_server::config::MockResponse {
                        status: 200,
                        headers: Some(vec![
                            ("content-type".to_string(), "application/json".to_string())
                        ].into_iter().collect()),
                        body: Some(r#"{"id": "{{path.id}}", "name": "User {{path.id}}", "timestamp": "{{timestamp}}"}"#.to_string()),
                        body_file: None,
                        delay: Some(100),
                        template: Some(true),
                    },
                },
            ],
        },
    ];
    
    // Setup authentication
    config.auth.strategy = "bearer".to_string();
    config.auth.users = Some(vec![
        ("admin".to_string(), "secret-token-123".to_string()),
        ("user".to_string(), "user-token-456".to_string()),
    ].into_iter().collect());
    
    // Create and start server
    let server = Server::new(config)?;
    
    // Register plugins
    server.register_plugin(Arc::new(MockPlugin::new())).await?;
    server.register_plugin(Arc::new(HealthPlugin::new())).await?;
    
    // Add custom routes
    server.add_route(
        Route::get("/custom"),
        Arc::new(json_handler("custom", serde_json::json!({
            "message": "This is a custom endpoint",
            "timestamp": chrono::Utc::now().to_rfc3339()
        })))
    ).await?;
    
    // Start server
    server.start().await?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_server_creation() {
        let config = Config::default();
        let server = Server::new(config);
        assert!(server.is_ok());
    }
    
    #[test]
    fn test_config_loading() {
        let config = Config::default();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.auth.strategy, "none");
    }
}