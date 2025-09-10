use super::{Plugin, PluginContext, PluginHook, PluginResult};
use crate::config::{MockConfig, MockRoute, MockResponse, MockScenario};
use crate::error::{ServerError, Result};
use crate::utils::templates::TemplateEngine;
use async_trait::async_trait;
use handlebars::Handlebars;
use hyper::{Body, Method, Request, Response, StatusCode};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

pub struct MockPlugin {
    config: MockConfig,
    scenarios: Vec<MockScenario>,
    template_engine: TemplateEngine,
    path_patterns: HashMap<String, Regex>,
    request_history: Vec<RecordedRequest>,
}

#[derive(Debug, Clone)]
struct RecordedRequest {
    timestamp: chrono::DateTime<chrono::Utc>,
    method: String,
    path: String,
    headers: HashMap<String, String>,
    query: HashMap<String, String>,
    body: Option<String>,
}

impl MockPlugin {
    pub fn new() -> Self {
        Self {
            config: MockConfig {
                scenarios: Vec::new(),
                default_delay: None,
                record_requests: false,
            },
            scenarios: Vec::new(),
            template_engine: TemplateEngine::new(),
            path_patterns: HashMap::new(),
            request_history: Vec::new(),
        }
    }
    
    /// Find matching route for request
    fn find_matching_route(&self, request: &Request<Body>, context: &PluginContext) -> Option<&MockRoute> {
        for scenario in &self.scenarios {
            if scenario.enabled.unwrap_or(true) {
                for route in &scenario.routes {
                    if self.matches_route(route, request, context) {
                        return Some(route);
                    }
                }
            }
        }
        None
    }
    
    /// Check if request matches route
    fn matches_route(&self, route: &MockRoute, request: &Request<Body>, context: &PluginContext) -> bool {
        // Check method
        if let Some(method) = &route.method {
            if method.to_uppercase() != request.method().as_str() {
                return false;
            }
        }
        
        // Check path
        if !self.matches_path(&route.path, request.uri().path()) {
            return false;
        }
        
        // Check headers
        if let Some(required_headers) = &route.headers {
            for (name, value) in required_headers {
                match request.headers().get(name) {
                    Some(header_value) => {
                        if header_value.to_str().unwrap_or("") != value {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
        }
        
        // Check query parameters
        if let Some(required_query) = &route.query {
            for (name, value) in required_query {
                match context.query.get(name) {
                    Some(query_value) => {
                        if query_value != value {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
        }
        
        true
    }
    
    /// Check if path matches pattern (supports simple wildcards and parameters)
    fn matches_path(&self, pattern: &str, path: &str) -> bool {
        if let Some(regex) = self.path_patterns.get(pattern) {
            return regex.is_match(path);
        }
        
        // Simple exact match if no pattern compiled
        pattern == path
    }
    
    /// Extract path parameters from matched route
    fn extract_path_params(&self, pattern: &str, path: &str) -> HashMap<String, String> {
        let mut params = HashMap::new();
        
        if let Some(regex) = self.path_patterns.get(pattern) {
            if let Some(captures) = regex.captures(path) {
                // Extract named capture groups
                for name in regex.capture_names().flatten() {
                    if let Some(matched) = captures.name(name) {
                        params.insert(name.to_string(), matched.as_str().to_string());
                    }
                }
            }
        }
        
        params
    }
    
    /// Create template context for response generation
    fn create_template_context(&self, context: &PluginContext, path_params: &HashMap<String, String>) -> Value {
        let mut template_context = serde_json::Map::new();
        
        // Add request information
        template_context.insert("request".to_string(), serde_json::json!({
            "method": context.method,
            "path": context.path,
            "headers": context.headers,
            "query": context.query,
        }));
        
        // Add path parameters
        template_context.insert("path".to_string(), serde_json::to_value(path_params).unwrap_or_default());
        
        // Add query parameters
        template_context.insert("query".to_string(), serde_json::to_value(&context.query).unwrap_or_default());
        
        // Add session information if available
        if let Some(session_id) = &context.session_id {
            template_context.insert("session_id".to_string(), Value::String(session_id.clone()));
        }
        
        // Add user information if available
        if let Some(user_id) = &context.user_id {
            template_context.insert("user_id".to_string(), Value::String(user_id.clone()));
        }
        
        // Add current timestamp
        template_context.insert("timestamp".to_string(), Value::String(
            chrono::Utc::now().to_rfc3339()
        ));
        
        // Add random values for testing
        template_context.insert("random".to_string(), serde_json::json!({
            "uuid": uuid::Uuid::new_v4().to_string(),
            "number": rand::random::<u32>(),
            "boolean": rand::random::<bool>(),
        }));
        
        Value::Object(template_context)
    }
    
    /// Generate mock response
    async fn generate_response(&self, route: &MockRoute, context: &PluginContext) -> Result<Response<Body>> {
        let path_params = self.extract_path_params(&route.path, &context.path);
        let template_context = self.create_template_context(context, &path_params);
        
        // Create response builder
        let mut response_builder = Response::builder()
            .status(StatusCode::from_u16(route.response.status)
                .unwrap_or(StatusCode::OK));
        
        // Add headers
        if let Some(headers) = &route.response.headers {
            for (name, value) in headers {
                let header_value = if route.response.template.unwrap_or(false) {
                    self.template_engine.render_string(value, &template_context)?
                } else {
                    value.clone()
                };
                response_builder = response_builder.header(name, header_value);
            }
        }
        
        // Generate body
        let body = if let Some(body_content) = &route.response.body {
            if route.response.template.unwrap_or(false) {
                self.template_engine.render_string(body_content, &template_context)?
            } else {
                body_content.clone()
            }
        } else if let Some(body_file) = &route.response.body_file {
            tokio::fs::read_to_string(body_file).await
                .map_err(|e| ServerError::handler(format!("Failed to read body file: {}", e)))?
        } else {
            String::new()
        };
        
        // Apply delay if specified
        if let Some(delay) = route.response.delay.or(self.config.default_delay) {
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        
        let response = response_builder
            .body(Body::from(body))
            .map_err(|e| ServerError::handler(format!("Failed to build response: {}", e)))?;
        
        Ok(response)
    }
    
    /// Record request for later analysis
    fn record_request(&mut self, request: &Request<Body>, context: &PluginContext, body: Option<&str>) {
        if self.config.record_requests {
            let recorded = RecordedRequest {
                timestamp: chrono::Utc::now(),
                method: context.method.clone(),
                path: context.path.clone(),
                headers: context.headers.clone(),
                query: context.query.clone(),
                body: body.map(|s| s.to_string()),
            };
            
            self.request_history.push(recorded);
            
            // Keep only last 1000 requests to prevent memory bloat
            if self.request_history.len() > 1000 {
                self.request_history.remove(0);
            }
        }
    }
    
    /// Compile path patterns into regexes
    fn compile_path_patterns(&mut self) -> Result<()> {
        for scenario in &self.scenarios {
            for route in &scenario.routes {
                if !self.path_patterns.contains_key(&route.path) {
                    let regex_pattern = self.path_to_regex(&route.path)?;
                    let regex = Regex::new(&regex_pattern)?;
                    self.path_patterns.insert(route.path.clone(), regex);
                }
            }
        }
        Ok(())
    }
    
    /// Convert path pattern to regex (supports {param} syntax)
    fn path_to_regex(&self, path: &str) -> Result<String> {
        let mut regex_pattern = regex::escape(path);
        
        // Replace {param} with named capture groups
        let param_regex = Regex::new(r"\\\{([^}]+)\\\}")?;
        regex_pattern = param_regex.replace_all(&regex_pattern, r"(?P<$1>[^/]+)").to_string();
        
        // Add anchors
        regex_pattern = format!("^{}$", regex_pattern);
        
        Ok(regex_pattern)
    }
    
    /// Get request history
    pub fn get_request_history(&self) -> &[RecordedRequest] {
        &self.request_history
    }
    
    /// Clear request history
    pub fn clear_request_history(&mut self) {
        self.request_history.clear();
    }
}

#[async_trait]
impl Plugin for MockPlugin {
    fn name(&self) -> &str {
        "mock"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn description(&self) -> &str {
        "Provides configurable mock responses based on YAML scenarios"
    }
    
    async fn initialize(&mut self, config: &serde_yaml::Value) -> Result<()> {
        if let Ok(mock_config) = serde_yaml::from_value::<MockConfig>(config.clone()) {
            self.config = mock_config.clone();
            self.scenarios = mock_config.scenarios;
            self.compile_path_patterns()?;
            tracing::info!("Mock plugin initialized with {} scenarios", self.scenarios.len());
        }
        Ok(())
    }
    
    fn handles_hook(&self, hook: &PluginHook) -> bool {
        matches!(hook, PluginHook::PreHandler)
    }
    
    fn priority(&self) -> i32 {
        50 // Run before most other plugins
    }
    
    async fn pre_handler(
        &self,
        request: &Request<Body>,
        context: &PluginContext,
    ) -> Result<PluginResult> {
        // Record request if enabled
        if self.config.record_requests {
            // Note: In a real implementation, you'd want to safely extract the body
            // This is simplified for demonstration
        }
        
        // Try to find matching route
        if let Some(route) = self.find_matching_route(request, context) {
            tracing::debug!("Mock route matched: {} {}", context.method, context.path);
            
            let response = self.generate_response(route, context).await?;
            return Ok(PluginResult::Response(response));
        }
        
        Ok(PluginResult::Continue)
    }
}