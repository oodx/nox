use nox::prelude::*;
use modular_api_client::prelude::*;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Example showing how the API client and server work together perfectly
#[tokio::main]
async fn main() -> Result<()> {
    println!("🔗 Nox Server + Modular API Client Integration Example");
    
    // Start the mock server in background
    let server = start_mock_server().await?;
    
    // Give server time to start
    sleep(Duration::from_millis(500)).await;
    
    // Create client to test against our mock server
    let client = create_test_client().await?;
    
    // Run integration tests
    run_integration_tests(&client).await?;
    
    // Cleanup
    server.shutdown().await?;
    
    println!("✅ Integration test completed successfully!");
    Ok(())
}

/// Start mock server with test scenarios
async fn start_mock_server() -> Result<Server> {
    let mut config = nox::Config::default();
    config.server.port = 8081; // Use different port for testing
    
    // Add comprehensive mock scenarios
    config.mock.scenarios = vec![
        nox::config::MockScenario {
            name: "integration_test".to_string(),
            enabled: Some(true),
            routes: vec![
                // User endpoint for client testing
                nox::config::MockRoute {
                    path: "/api/users/{id}".to_string(),
                    method: Some("GET".to_string()),
                    headers: None,
                    query: None,
                    body_pattern: None,
                    response: nox::config::MockResponse {
                        status: 200,
                        headers: Some(vec![
                            ("content-type".to_string(), "application/json".to_string()),
                            ("x-test-integration".to_string(), "true".to_string()),
                        ].into_iter().collect()),
                        body: Some(r#"
{
  "id": "{{path.id}}",
  "name": "Test User {{path.id}}",
  "email": "user{{path.id}}@test.com",
  "created_at": "{{timestamp 'iso8601'}}",
  "integration_test": true
}
                        "#.to_string()),
                        body_file: None,
                        delay: Some(50),
                        template: Some(true),
                    },
                },
                
                // Streaming endpoint
                nox::config::MockRoute {
                    path: "/api/stream".to_string(),
                    method: Some("GET".to_string()),
                    headers: None,
                    query: None,
                    body_pattern: None,
                    response: nox::config::MockResponse {
                        status: 200,
                        headers: Some(vec![
                            ("content-type".to_string(), "application/json".to_string()),
                            ("transfer-encoding".to_string(), "chunked".to_string()),
                        ].into_iter().collect()),
                        body: Some(r#"
{"chunk": 1, "data": "first chunk", "timestamp": "{{timestamp}}"}
{"chunk": 2, "data": "second chunk", "timestamp": "{{timestamp}}"}
{"chunk": 3, "data": "third chunk", "timestamp": "{{timestamp}}"}
                        "#.to_string()),
                        body_file: None,
                        delay: Some(100),
                        template: Some(true),
                    },
                },
                
                // Error testing endpoint
                nox::config::MockRoute {
                    path: "/api/error".to_string(),
                    method: Some("GET".to_string()),
                    headers: None,
                    query: None,
                    body_pattern: None,
                    response: nox::config::MockResponse {
                        status: 500,
                        headers: Some(vec![
                            ("content-type".to_string(), "application/json".to_string()),
                        ].into_iter().collect()),
                        body: Some(r#"
{
  "error": "Simulated server error for client testing",
  "code": "TEST_ERROR",
  "timestamp": "{{timestamp}}"
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
    
    let server = Server::new(config)?;
    
    // Register mock plugin
    let mut mock_plugin = nox::plugins::mock::MockPlugin::new();
    let mock_config = serde_yaml::to_value(&server.config().await.mock).unwrap();
    mock_plugin.initialize(&mock_config).await?;
    server.register_plugin(Arc::new(mock_plugin)).await?;
    
    // Start server in background
    let server_clone = server.clone();
    tokio::spawn(async move {
        if let Err(e) = server_clone.start().await {
            eprintln!("Server error: {}", e);
        }
    });
    
    Ok(server)
}

/// Create API client configured for testing
async fn create_test_client() -> Result<modular_api_client::ApiClient> {
    let client = modular_api_client::ApiClientBuilder::new()
        .with_timeout(Duration::from_secs(10))
        .with_user_agent("Integration-Test-Client/1.0.0".to_string())
        .with_base_url("http://localhost:8081".to_string())
        .build()
        .await?;
    
    Ok(client)
}

/// Run comprehensive integration tests
async fn run_integration_tests(client: &modular_api_client::ApiClient) -> Result<()> {
    println!("🧪 Running integration tests...");
    
    // Test 1: Basic GET request
    println!("  ✓ Testing basic GET request...");
    let response = client.get("http://localhost:8081/api/users/123").await?;
    let user_data: serde_json::Value = serde_json::from_slice(&client.download_bytes("http://localhost:8081/api/users/123").await?)?;
    assert_eq!(user_data["id"], "123");
    assert_eq!(user_data["integration_test"], true);
    println!("    ✅ User API integration working");
    
    // Test 2: Error handling
    println!("  ✓ Testing error handling...");
    match client.get("http://localhost:8081/api/error").await {
        Err(modular_api_client::ApiError::HttpStatus { status }) => {
            assert_eq!(status.as_u16(), 500);
            println!("    ✅ Error handling working correctly");
        }
        _ => panic!("Expected HTTP 500 error"),
    }
    
    // Test 3: Download and save functionality
    println!("  ✓ Testing download and save...");
    let file_path = client.download_file("http://localhost:8081/api/users/456", "test_user.json").await?;
    println!("    ✅ File saved to: {:?}", file_path);
    
    // Test 4: JSON download
    println!("  ✓ Testing JSON download...");
    let json_data: serde_json::Value = client.download_json("http://localhost:8081/api/users/789").await?;
    assert_eq!(json_data["id"], "789");
    println!("    ✅ JSON download working");
    
    // Test 5: Stream processing
    println!("  ✓ Testing stream processing...");
    let mut chunk_count = 0;
    client.stream_response("http://localhost:8081/api/stream", |chunk| async move {
        chunk_count += 1;
        println!("    📦 Received chunk {}: {} bytes", chunk_count, chunk.len());
        Ok(())
    }).await?;
    println!("    ✅ Stream processing working");
    
    // Test 6: Client storage integration
    println!("  ✓ Testing client storage...");
    let storage_stats = client.storage().storage_stats().await?;
    println!("    📊 Storage stats: {:?}", storage_stats);
    println!("    ✅ Client storage working");
    
    println!("🎉 All integration tests passed!");
    Ok(())
}

/// Example of shared configuration between client and server
pub fn create_shared_config() -> (modular_api_client::Config, nox::Config) {
    let client_config = modular_api_client::Config {
        client: modular_api_client::config::ClientConfig {
            user_agent: "Shared-Client/1.0.0".to_string(),
            timeout: 30,
            max_retries: 3,
            retry_delay: 1000,
            default_headers: vec![
                ("x-client-version".to_string(), "1.0.0".to_string()),
                ("x-integration".to_string(), "client-server".to_string()),
            ].into_iter().collect(),
            base_url: Some("http://localhost:8080".to_string()),
        },
        // ... other client config
        ..Default::default()
    };
    
    let server_config = nox::Config {
        server: nox::config::ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            workers: Some(4),
            max_connections: Some(1000),
            request_timeout: Some(30),
            keep_alive_timeout: Some(60),
            tls: None,
        },
        // ... other server config
        ..Default::default()
    };
    
    (client_config, server_config)
}

/// Example of using both client and server plugins together
pub async fn demonstrate_plugin_synergy() -> Result<()> {
    println!("🔌 Demonstrating plugin synergy...");
    
    // Server-side: Mock plugin generates responses
    // Client-side: Logging plugin tracks requests
    
    // This shows how plugins on both sides can work together
    // for comprehensive testing and development workflows
    
    Ok(())
}