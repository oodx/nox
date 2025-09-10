use super::{Plugin, PluginContext, PluginHook, PluginResult};
use crate::config::HealthConfig;
use crate::error::Result;
use async_trait::async_trait;
use hyper::{Body, Request, Response, StatusCode};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct HealthPlugin {
    config: HealthConfig,
    startup_time: SystemTime,
    request_count: std::sync::atomic::AtomicU64,
    error_count: std::sync::atomic::AtomicU64,
}

impl HealthPlugin {
    pub fn new() -> Self {
        Self {
            config: HealthConfig {
                enabled: true,
                path: "/health".to_string(),
                detailed: false,
            },
            startup_time: SystemTime::now(),
            request_count: std::sync::atomic::AtomicU64::new(0),
            error_count: std::sync::atomic::AtomicU64::new(0),
        }
    }
    
    fn create_health_response(&self) -> Result<Response<Body>> {
        let uptime = self.startup_time
            .elapsed()
            .unwrap_or_default()
            .as_secs();
        
        let health_data = if self.config.detailed {
            json!({
                "status": "healthy",
                "timestamp": SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                "uptime_seconds": uptime,
                "version": env!("CARGO_PKG_VERSION"),
                "metrics": {
                    "requests_total": self.request_count.load(std::sync::atomic::Ordering::Relaxed),
                    "errors_total": self.error_count.load(std::sync::atomic::Ordering::Relaxed)
                },
                "checks": {
                    "server": "ok",
                    "memory": "ok"
                }
            })
        } else {
            json!({
                "status": "healthy",
                "timestamp": SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            })
        };
        
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .header("cache-control", "no-cache")
            .body(Body::from(serde_json::to_string_pretty(&health_data)?))
            .map_err(|e| crate::error::ServerError::handler(format!("Failed to build health response: {}", e)))?;
        
        Ok(response)
    }
    
    fn create_readiness_response(&self) -> Result<Response<Body>> {
        // In a real implementation, you'd check database connections, 
        // external service availability, etc.
        let readiness_data = json!({
            "status": "ready",
            "timestamp": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            "checks": {
                "database": "ok",
                "external_services": "ok"
            }
        });
        
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .header("cache-control", "no-cache")
            .body(Body::from(serde_json::to_string_pretty(&readiness_data)?))
            .map_err(|e| crate::error::ServerError::handler(format!("Failed to build readiness response: {}", e)))?;
        
        Ok(response)
    }
    
    fn create_metrics_response(&self) -> Result<Response<Body>> {
        let uptime = self.startup_time
            .elapsed()
            .unwrap_or_default()
            .as_secs();
        
        // Prometheus-style metrics
        let metrics = format!(
            "# HELP http_requests_total Total number of HTTP requests\n\
             # TYPE http_requests_total counter\n\
             http_requests_total {}\n\
             \n\
             # HELP http_errors_total Total number of HTTP errors\n\
             # TYPE http_errors_total counter\n\
             http_errors_total {}\n\
             \n\
             # HELP uptime_seconds Server uptime in seconds\n\
             # TYPE uptime_seconds gauge\n\
             uptime_seconds {}\n",
            self.request_count.load(std::sync::atomic::Ordering::Relaxed),
            self.error_count.load(std::sync::atomic::Ordering::Relaxed),
            uptime
        );
        
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/plain; version=0.0.4")
            .body(Body::from(metrics))
            .map_err(|e| crate::error::ServerError::handler(format!("Failed to build metrics response: {}", e)))?;
        
        Ok(response)
    }
}

#[async_trait]
impl Plugin for HealthPlugin {
    fn name(&self) -> &str {
        "health"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn description(&self) -> &str {
        "Provides health check, readiness, and metrics endpoints"
    }
    
    async fn initialize(&mut self, config: &serde_yaml::Value) -> Result<()> {
        if let Ok(health_config) = serde_yaml::from_value::<HealthConfig>(config.clone()) {
            self.config = health_config;
            tracing::info!("Health plugin initialized with path: {}", self.config.path);
        }
        Ok(())
    }
    
    fn handles_hook(&self, hook: &PluginHook) -> bool {
        matches!(hook, PluginHook::PreHandler | PluginHook::PostResponse | PluginHook::OnError)
    }
    
    fn priority(&self) -> i32 {
        10 // Run early for health checks
    }
    
    async fn pre_handler(
        &self,
        _request: &Request<Body>,
        context: &PluginContext,
    ) -> Result<PluginResult> {
        if !self.config.enabled {
            return Ok(PluginResult::Continue);
        }
        
        // Increment request counter
        self.request_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        match context.path.as_str() {
            path if path == self.config.path => {
                let response = self.create_health_response()?;
                Ok(PluginResult::Response(response))
            }
            "/health/ready" | "/ready" => {
                let response = self.create_readiness_response()?;
                Ok(PluginResult::Response(response))
            }
            "/health/metrics" | "/metrics" => {
                let response = self.create_metrics_response()?;
                Ok(PluginResult::Response(response))
            }
            _ => Ok(PluginResult::Continue),
        }
    }
    
    async fn post_response(
        &self,
        _response: &Response<Body>,
        _context: &PluginContext,
    ) -> Result<PluginResult> {
        // Track successful responses
        Ok(PluginResult::Continue)
    }
    
    async fn on_error(
        &self,
        _error: &crate::error::ServerError,
        _context: &PluginContext,
    ) -> Result<PluginResult> {
        // Increment error counter
        self.error_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(PluginResult::Continue)
    }
}